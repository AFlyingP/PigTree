//! Client orchestration, session spawning, and typed RPC commands over Named Pipes.

use crate::bootstrap::{spawn_engine, BootstrapPipe, ChildProcessGuard};
use crate::error::IpcError;
use crate::job::JobObject;
use crate::pipe::{format_pipe_name, NamedPipeClient};
use crate::security::{constant_time_eq, derive_channel_key, generate_nonce};
use crate::transport::FramedSession;
use crate::win32::*;
use pigtree_protocol::frame::{ChannelTag, FrameFlags};
use pigtree_protocol::protobuf::{
    command_request, command_response, AuthHandshakeRequest, AuthHandshakeResponse, CancelRequest,
    CancelResponse, CommandRequest, CommandResponse, EchoRequest, EchoResponse, HealthRequest,
    HealthResponse, PingRequest, PingResponse, ShutdownRequest, StatusRequest, StatusResponse,
    VersionRequest, VersionResponse,
};
use std::path::Path;

pub struct EngineClientSession {
    job_object: JobObject,
    child_process: ChildProcessGuard,
    framed: FramedSession<crate::pipe::PipeStream>,
    session_id: String,
    channel_key: [u8; 32],
}

impl EngineClientSession {
    /// Launches a dedicated private session host engine and establishes an authenticated IPC connection.
    pub fn launch(engine_exe: &Path) -> Result<Self, IpcError> {
        let job_object = JobObject::create_kill_on_close()?;

        let bootstrap_nonce = generate_nonce();
        let mut bootstrap_pipe = BootstrapPipe::create()?;
        bootstrap_pipe.write_nonce(&bootstrap_nonce)?;

        let session_nonce = generate_nonce();
        let session_id: String = session_nonce[0..8]
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        let pipe_name = format_pipe_name(&session_id);

        let child_process = spawn_engine(
            engine_exe,
            &pipe_name,
            &session_id,
            &bootstrap_pipe,
            &job_object,
        )?;

        // Connect to Named Pipe with timeout
        let stream = match NamedPipeClient::connect(&pipe_name, 5000) {
            Ok(s) => s,
            Err(e) => {
                let _ = child_process.terminate(1);
                return Err(e);
            }
        };

        // Mutual process identity verification:
        // 1. Verify Named Pipe server PID matches spawned child process PID
        let server_pid = stream.get_server_pid()?;
        if server_pid != child_process.pid {
            let _ = child_process.terminate(1);
            return Err(IpcError::IdentityMismatch {
                expected: child_process.pid.to_string(),
                actual: server_pid.to_string(),
            });
        }

        let mut framed = FramedSession::new(stream);

        // Perform mutual authentication handshake
        let client_nonce = generate_nonce();
        let client_pid = unsafe { GetCurrentProcessId() };

        let handshake_req = AuthHandshakeRequest {
            bootstrap_nonce: bootstrap_nonce.to_vec(),
            client_nonce: client_nonce.to_vec(),
            client_pid,
            client_session_id: 0,
        };

        framed.send_message(ChannelTag::Command, FrameFlags::empty(), &handshake_req)?;

        let (resp_header, handshake_resp): (_, AuthHandshakeResponse) =
            framed.recv_message()?.ok_or(IpcError::Protocol(
                pigtree_protocol::FrameParseError::PrematureEof,
            ))?;
        if resp_header.channel_tag != ChannelTag::Command {
            let _ = child_process.terminate(1);
            return Err(IpcError::AuthenticationFailed(
                "Handshake response on invalid channel".to_string(),
            ));
        }

        if handshake_resp.status != 0 {
            let _ = child_process.terminate(1);
            return Err(IpcError::AuthenticationFailed(format!(
                "Server rejected handshake: {}",
                handshake_resp.error_message
            )));
        }

        if handshake_resp.server_pid != child_process.pid {
            let _ = child_process.terminate(1);
            return Err(IpcError::IdentityMismatch {
                expected: child_process.pid.to_string(),
                actual: handshake_resp.server_pid.to_string(),
            });
        }

        let expected_channel_key = derive_channel_key(
            &bootstrap_nonce,
            &client_nonce,
            &handshake_resp.server_nonce,
        );

        if !constant_time_eq(&handshake_resp.channel_key_hash, &expected_channel_key) {
            let _ = child_process.terminate(1);
            return Err(IpcError::AuthenticationFailed(
                "Derived channel key hash mismatch".to_string(),
            ));
        }

        Ok(Self {
            job_object,
            child_process,
            framed,
            session_id,
            channel_key: expected_channel_key,
        })
    }

    pub fn engine_pid(&self) -> u32 {
        self.child_process.pid
    }

    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    pub fn channel_key(&self) -> &[u8; 32] {
        &self.channel_key
    }

    pub fn child_process(&self) -> &ChildProcessGuard {
        &self.child_process
    }

    pub fn job_object(&self) -> &JobObject {
        &self.job_object
    }

    pub fn send_command(&mut self, req: CommandRequest) -> Result<CommandResponse, IpcError> {
        self.send_command_interruptible(req, None)
    }

    pub fn send_command_interruptible(
        &mut self,
        req: CommandRequest,
        cancel_event: Option<HANDLE>,
    ) -> Result<CommandResponse, IpcError> {
        self.framed
            .send_message(ChannelTag::Command, FrameFlags::empty(), &req)?;
        let (_, resp) = self
            .framed
            .recv_message_interruptible::<CommandResponse>(cancel_event, None)?
            .ok_or(IpcError::Protocol(
                pigtree_protocol::FrameParseError::PrematureEof,
            ))?;
        Ok(resp)
    }

    pub fn cancel_request(&mut self, target_request_id: &str) -> Result<CancelResponse, IpcError> {
        let cancel_req_id = format!("cancel-{}", self.framed.next_seq());
        let req = CommandRequest {
            request_id: cancel_req_id,
            request: Some(command_request::Request::Cancel(CancelRequest {
                target_request_id: target_request_id.to_string(),
                reason: "Operation cancelled by user".to_string(),
            })),
        };

        // Send cancel request on CancellationHeartbeat channel
        self.framed
            .send_message(ChannelTag::CancellationHeartbeat, FrameFlags::empty(), &req)?;

        // Wait bounded time for acknowledgment (up to 3000ms)
        let (_, resp) = self
            .framed
            .recv_message_interruptible::<CommandResponse>(None, Some(3000))?
            .ok_or(IpcError::Protocol(
                pigtree_protocol::FrameParseError::PrematureEof,
            ))?;

        match resp.response {
            Some(command_response::Response::Cancel(cancel_resp)) => Ok(cancel_resp),
            _ => Ok(CancelResponse {
                cancelled: true,
                message: "Request cancelled".to_string(),
            }),
        }
    }

    fn send_rpc_request<R>(
        &mut self,
        req_verb: &str,
        request: command_request::Request,
        cancel_event: Option<HANDLE>,
        extract: impl FnOnce(command_response::Response) -> Option<R>,
    ) -> Result<R, IpcError> {
        let request_id = format!("{req_verb}-{}", self.framed.next_seq());
        let req = CommandRequest {
            request_id,
            request: Some(request),
        };

        let resp = self.send_command_interruptible(req, cancel_event)?;
        if resp.status != 0 {
            return Err(IpcError::AuthenticationFailed(resp.error_message));
        }
        match resp.response {
            Some(r) => extract(r).ok_or(IpcError::Protocol(
                pigtree_protocol::FrameParseError::PrematureEof,
            )),
            None => Err(IpcError::Protocol(
                pigtree_protocol::FrameParseError::PrematureEof,
            )),
        }
    }

    pub fn ping(&mut self) -> Result<PingResponse, IpcError> {
        self.ping_with_options(0, None)
    }

    pub fn ping_with_options(
        &mut self,
        delay_ms: u32,
        cancel_event: Option<HANDLE>,
    ) -> Result<PingResponse, IpcError> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        self.send_rpc_request(
            "ping",
            command_request::Request::Ping(PingRequest {
                timestamp_utc_ms: now,
                delay_ms,
            }),
            cancel_event,
            |r| match r {
                command_response::Response::Ping(p) => Some(p),
                _ => None,
            },
        )
    }

    pub fn echo(&mut self, payload: &str) -> Result<EchoResponse, IpcError> {
        self.echo_with_options(payload, 0, None)
    }

    pub fn echo_with_options(
        &mut self,
        payload: &str,
        delay_ms: u32,
        cancel_event: Option<HANDLE>,
    ) -> Result<EchoResponse, IpcError> {
        self.send_rpc_request(
            "echo",
            command_request::Request::Echo(EchoRequest {
                payload: payload.to_string(),
                delay_ms,
            }),
            cancel_event,
            |r| match r {
                command_response::Response::Echo(e) => Some(e),
                _ => None,
            },
        )
    }

    pub fn health(&mut self, include_memory: bool) -> Result<HealthResponse, IpcError> {
        self.health_with_options(include_memory, 0, None)
    }

    pub fn health_with_options(
        &mut self,
        include_memory: bool,
        delay_ms: u32,
        cancel_event: Option<HANDLE>,
    ) -> Result<HealthResponse, IpcError> {
        self.send_rpc_request(
            "health",
            command_request::Request::Health(HealthRequest {
                include_memory,
                delay_ms,
            }),
            cancel_event,
            |r| match r {
                command_response::Response::Health(h) => Some(h),
                _ => None,
            },
        )
    }

    pub fn status(&mut self) -> Result<StatusResponse, IpcError> {
        self.status_with_options(0, None)
    }

    pub fn status_with_options(
        &mut self,
        delay_ms: u32,
        cancel_event: Option<HANDLE>,
    ) -> Result<StatusResponse, IpcError> {
        self.send_rpc_request(
            "status",
            command_request::Request::Status(StatusRequest { delay_ms }),
            cancel_event,
            |r| match r {
                command_response::Response::StatusPayload(s) => Some(s),
                _ => None,
            },
        )
    }

    pub fn version(&mut self) -> Result<VersionResponse, IpcError> {
        self.version_with_options(0, None)
    }

    pub fn version_with_options(
        &mut self,
        delay_ms: u32,
        cancel_event: Option<HANDLE>,
    ) -> Result<VersionResponse, IpcError> {
        self.send_rpc_request(
            "version",
            command_request::Request::Version(VersionRequest { delay_ms }),
            cancel_event,
            |r| match r {
                command_response::Response::Version(v) => Some(v),
                _ => None,
            },
        )
    }

    pub fn shutdown(&mut self) -> Result<(), IpcError> {
        let request_id = format!("shutdown-{}", self.framed.next_seq());
        let req = CommandRequest {
            request_id,
            request: Some(command_request::Request::Shutdown(ShutdownRequest {})),
        };

        let _ = self.send_command(req);
        Ok(())
    }
}
