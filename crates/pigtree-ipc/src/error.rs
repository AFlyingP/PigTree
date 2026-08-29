//! Error types for PigTree IPC subsystem.

use pigtree_protocol::FrameParseError;
use pigtree_protocol::ProtoError;
use std::fmt;
use std::io;

#[derive(Debug)]
pub enum IpcError {
    Io(io::Error),
    Protocol(FrameParseError),
    Protobuf(ProtoError),
    Win32 { code: u32, message: String },
    AuthenticationFailed(String),
    CommandError { code: String, message: String },
    IdentityMismatch { expected: String, actual: String },
    SessionNotFound,
    Timeout,
    Cancelled,
    ProcessExited(u32),
    ServerNotFound,
}

impl fmt::Display for IpcError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IpcError::Io(e) => write!(f, "I/O error: {e}"),
            IpcError::Protocol(e) => write!(f, "protocol framing error: {e}"),
            IpcError::Protobuf(e) => write!(f, "protobuf decoding error: {e}"),
            IpcError::Win32 { code, message } => {
                write!(f, "Win32 error (code {code}): {message}")
            }
            IpcError::AuthenticationFailed(msg) => write!(f, "authentication failed: {msg}"),
            IpcError::CommandError { code, message } => {
                write!(f, "command error ({code}): {message}")
            }
            IpcError::IdentityMismatch { expected, actual } => {
                write!(
                    f,
                    "mutual process identity mismatch: expected {expected}, actual {actual}"
                )
            }
            IpcError::SessionNotFound => write!(f, "session not found"),
            IpcError::Timeout => write!(f, "IPC operation timed out"),
            IpcError::Cancelled => write!(f, "IPC operation cancelled"),
            IpcError::ProcessExited(code) => {
                write!(f, "child process exited prematurely with code {code}")
            }
            IpcError::ServerNotFound => write!(f, "engine server named pipe not found"),
        }
    }
}

impl std::error::Error for IpcError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            IpcError::Io(e) => Some(e),
            IpcError::Protocol(e) => Some(e),
            IpcError::Protobuf(e) => Some(e),
            _ => None,
        }
    }
}

impl From<io::Error> for IpcError {
    fn from(e: io::Error) -> Self {
        IpcError::Io(e)
    }
}

impl From<FrameParseError> for IpcError {
    fn from(e: FrameParseError) -> Self {
        IpcError::Protocol(e)
    }
}

impl From<ProtoError> for IpcError {
    fn from(e: ProtoError) -> Self {
        IpcError::Protobuf(e)
    }
}
