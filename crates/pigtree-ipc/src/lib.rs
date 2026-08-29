//! Windows Named Pipe IPC, process lifecycle, Job Object confinement, and security controls.

pub mod bootstrap;
pub mod client;
pub mod error;
pub mod job;
pub mod pipe;
pub mod process;
pub mod security;
pub mod server;
pub mod transport;
pub mod validator;
pub mod win32;

pub use bootstrap::{read_bootstrap_nonce, spawn_engine, BootstrapPipe, ChildProcessGuard};
pub use client::{EngineClientSession, ScanCallOutcome};
pub use error::IpcError;
pub use job::JobObject;
pub use pipe::{format_pipe_name, NamedPipeClient, NamedPipeServer, PipeRole, PipeStream};
pub use process::{
    build_windows_command_line, quote_windows_arg, AnonymousPipe, CancelHandle, PipeReader,
};
pub use security::{
    build_pipe_sddl, constant_time_eq, constant_time_eq_32, create_pipe_security_attributes,
    derive_channel_key, generate_nonce, get_current_token_restricted_sids, get_current_user_sid,
    get_process_creation_time, is_broad_sid, SecurityDescriptorGuard,
};
pub use server::EngineServerSession;
pub use transport::{FrameReadiness, FramedSession};
pub use validator::{
    is_lexical_unc, validate_scan_target, DriveKind, FileSystemKind, TargetValidationError,
    ValidatedScanTarget,
};
