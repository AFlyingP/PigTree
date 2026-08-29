//! Engine server IPC lifecycle, authentication, and mutual identity verification.

use crate::error::IpcError;
use crate::pipe::NamedPipeServer;
use crate::security::{constant_time_eq, derive_channel_key, generate_nonce};
use crate::transport::FramedSession;
use crate::win32::*;
use pigtree_protocol::frame::{ChannelTag, FrameFlags};
use pigtree_protocol::protobuf::{
    AuthHandshakeRequest, AuthHandshakeResponse, CommandRequest, CommandResponse,
};

pub struct EngineServerSession {
    framed: FramedSession<crate::pipe::PipeStream>,
    client_pid: u32,
    client_session_id: u32,
    channel_key: [u8; 32],
}

impl EngineServerSession {
    /// Binds the Named Pipe server and accepts/authenticates the incoming client connection.
    pub fn accept(pipe_name: &str, bootstrap_nonce: [u8; 32]) -> Result<Self, IpcError> {
        let server = NamedPipeServer::create(pipe_name)?;
        let stream = server.accept()?;

        // Query and verify connected client's PID and session ID
        let client_pid = stream.get_client_pid()?;
        let client_session_id = stream.get_client_session_id()?;

        let mut framed = FramedSession::new(stream);

        // Receive AuthHandshakeRequest
        let (header, handshake_req): (_, AuthHandshakeRequest) = framed.recv_message()?.ok_or(
            IpcError::Protocol(pigtree_protocol::FrameParseError::PrematureEof),
        )?;
        if header.channel_tag != ChannelTag::Command {
            return Err(IpcError::AuthenticationFailed(
                "Handshake request must be sent on Command channel".to_string(),
            ));
        }

        // Mutual identity & nonce verification
        if !constant_time_eq(&handshake_req.bootstrap_nonce, &bootstrap_nonce) {
            let resp = AuthHandshakeResponse {
                status: 1, // UNAUTHORIZED
                error_message: "Invalid bootstrap launch nonce".to_string(),
                ..Default::default()
            };
            let _ = framed.send_message(ChannelTag::Command, FrameFlags::empty(), &resp);
            return Err(IpcError::AuthenticationFailed(
                "Bootstrap nonce mismatch".to_string(),
            ));
        }

        if handshake_req.client_pid != client_pid {
            let resp = AuthHandshakeResponse {
                status: 1,
                error_message: "Client PID mismatch".to_string(),
                ..Default::default()
            };
            let _ = framed.send_message(ChannelTag::Command, FrameFlags::empty(), &resp);
            return Err(IpcError::IdentityMismatch {
                expected: client_pid.to_string(),
                actual: handshake_req.client_pid.to_string(),
            });
        }

        // Generate server nonce and derive ephemeral channel key
        let server_nonce = generate_nonce();
        let channel_key =
            derive_channel_key(&bootstrap_nonce, &handshake_req.client_nonce, &server_nonce);

        let my_pid = unsafe { GetCurrentProcessId() };
        let resp = AuthHandshakeResponse {
            status: 0, // OK
            server_nonce: server_nonce.to_vec(),
            server_pid: my_pid,
            channel_key_hash: channel_key.to_vec(),
            error_message: String::new(),
        };

        framed.send_message(ChannelTag::Command, FrameFlags::empty(), &resp)?;

        Ok(Self {
            framed,
            client_pid,
            client_session_id,
            channel_key,
        })
    }

    pub fn client_pid(&self) -> u32 {
        self.client_pid
    }

    pub fn client_session_id(&self) -> u32 {
        self.client_session_id
    }

    pub fn channel_key(&self) -> &[u8; 32] {
        &self.channel_key
    }

    pub fn has_incoming_data(&self) -> Result<bool, IpcError> {
        self.framed.has_incoming_data()
    }

    pub fn recv_command(&mut self) -> Result<Option<(ChannelTag, CommandRequest)>, IpcError> {
        match self.framed.recv_message::<CommandRequest>()? {
            Some((header, cmd)) => Ok(Some((header.channel_tag, cmd))),
            None => Ok(None),
        }
    }

    pub fn send_response(&mut self, resp: &CommandResponse) -> Result<(), IpcError> {
        self.framed
            .send_message(ChannelTag::Command, FrameFlags::empty(), resp)?;
        Ok(())
    }
}
