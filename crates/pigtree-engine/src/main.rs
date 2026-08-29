//! Dedicated private session host engine (pigtree-engine.exe).

use pigtree_ipc::server::EngineServerSession;
use pigtree_ipc::win32::*;
use pigtree_protocol::protobuf::{
    command_request, command_response, CancelResponse, CommandResponse, EchoResponse,
    HealthResponse, PingResponse, ShutdownResponse, StatusResponse, VersionResponse,
};
use std::env;
use std::process::ExitCode;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

fn main() -> ExitCode {
    ExitCode::from(run())
}

fn run() -> u8 {
    let args: Vec<String> = env::args().collect();
    let mut pipe_name = String::new();
    let mut session_id = String::new();
    let mut bootstrap_handle_val: Option<usize> = None;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--pipe-name" if i + 1 < args.len() => {
                pipe_name = args[i + 1].clone();
                i += 1;
            }
            "--session-id" if i + 1 < args.len() => {
                session_id = args[i + 1].clone();
                i += 1;
            }
            "--bootstrap-handle" if i + 1 < args.len() => {
                bootstrap_handle_val = args[i + 1].parse::<usize>().ok();
                i += 1;
            }
            _ => {}
        }
        i += 1;
    }

    if pipe_name.is_empty() || bootstrap_handle_val.is_none() {
        eprintln!(
            "Usage: pigtree-engine.exe --pipe-name <pipe> --session-id <session> --bootstrap-handle <handle>"
        );
        return 2;
    }

    let h_bootstrap = bootstrap_handle_val.unwrap() as HANDLE;
    let bootstrap_nonce = match unsafe { pigtree_ipc::read_bootstrap_nonce(h_bootstrap) } {
        Ok(nonce) => nonce,
        Err(err) => {
            eprintln!("Failed to read bootstrap nonce: {err}");
            return 1;
        }
    };

    let start_instant = Instant::now();

    let mut session = match EngineServerSession::accept(&pipe_name, bootstrap_nonce) {
        Ok(s) => s,
        Err(err) => {
            eprintln!("Failed to establish engine server session: {err}");
            return 1;
        }
    };

    // Main command processing loop
    loop {
        let (_channel_tag, cmd) = match session.recv_command() {
            Ok(Some(c)) => c,
            Ok(None) => {
                // Client disconnected cleanly (EOF between frames)
                break;
            }
            Err(pigtree_ipc::IpcError::Io(_)) => {
                // Pipe closed
                break;
            }
            Err(err) => {
                eprintln!("Engine error reading command: {err}");
                break;
            }
        };

        // Check for test delay
        let env_delay = env::var("PIGTREE_TEST_DELAY_MS")
            .ok()
            .and_then(|v| v.parse::<u32>().ok())
            .unwrap_or(0);

        let req_delay = match &cmd.request {
            Some(command_request::Request::Ping(p)) => p.delay_ms,
            Some(command_request::Request::Echo(e)) => e.delay_ms,
            Some(command_request::Request::Health(h)) => h.delay_ms,
            Some(command_request::Request::Status(s)) => s.delay_ms,
            Some(command_request::Request::Version(v)) => v.delay_ms,
            _ => 0,
        };

        let effective_delay = req_delay.max(env_delay);
        if effective_delay > 0 {
            let start = Instant::now();
            let delay = std::time::Duration::from_millis(effective_delay as u64);
            let mut cancelled = false;
            let mut cancel_request_id = String::new();

            while start.elapsed() < delay {
                if session.has_incoming_data().unwrap_or(false) {
                    match session.recv_command() {
                        Ok(Some((_tag, incoming_cmd))) => {
                            if let Some(command_request::Request::Cancel(cancel)) =
                                incoming_cmd.request
                            {
                                cancelled = true;
                                cancel_request_id = if incoming_cmd.request_id.is_empty() {
                                    cancel.target_request_id
                                } else {
                                    incoming_cmd.request_id
                                };
                                break;
                            }
                        }
                        Ok(None) => return 0,
                        Err(_) => return 0,
                    }
                }
                std::thread::sleep(std::time::Duration::from_millis(10));
            }

            if cancelled {
                let cancel_resp = CommandResponse {
                    request_id: cancel_request_id,
                    status: 0,
                    error_code: String::new(),
                    error_message: String::new(),
                    response: Some(command_response::Response::Cancel(CancelResponse {
                        cancelled: true,
                        message: "Operation cancelled by client".to_string(),
                    })),
                };
                let _ = session.send_response(&cancel_resp);
                continue;
            }
        }

        let mut resp = CommandResponse {
            request_id: cmd.request_id.clone(),
            status: 0,
            error_code: String::new(),
            error_message: String::new(),
            response: None,
        };

        let mut is_shutdown = false;

        match cmd.request {
            Some(command_request::Request::Ping(ping)) => {
                let now_ms = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64;
                resp.response = Some(command_response::Response::Ping(PingResponse {
                    timestamp_utc_ms: ping.timestamp_utc_ms,
                    echo_timestamp_utc_ms: now_ms,
                }));
            }
            Some(command_request::Request::Echo(echo)) => {
                resp.response = Some(command_response::Response::Echo(EchoResponse {
                    payload: echo.payload,
                }));
            }
            Some(command_request::Request::Health(health)) => {
                let uptime_ms = start_instant.elapsed().as_millis() as u64;
                let mut handle_count: DWORD = 0;
                unsafe {
                    GetProcessHandleCount(GetCurrentProcess(), &mut handle_count);
                }

                let mut mem_private_bytes: u64 = 0;
                if health.include_memory {
                    let mut pmc: PROCESS_MEMORY_COUNTERS = unsafe { std::mem::zeroed() };
                    pmc.cb = std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as DWORD;
                    if unsafe { K32GetProcessMemoryInfo(GetCurrentProcess(), &mut pmc, pmc.cb) }
                        != 0
                    {
                        mem_private_bytes = pmc.PagefileUsage as u64;
                    }
                }

                resp.response = Some(command_response::Response::Health(HealthResponse {
                    status: "HEALTHY".to_string(),
                    uptime_ms,
                    memory_private_bytes: mem_private_bytes,
                    handle_count,
                }));
            }
            Some(command_request::Request::Status(_)) => {
                resp.response = Some(command_response::Response::StatusPayload(StatusResponse {
                    state: "IDLE".to_string(),
                    active_runs: 0,
                    total_observations: 0,
                    session_id: session_id.clone(),
                }));
            }
            Some(command_request::Request::Version(_)) => {
                resp.response = Some(command_response::Response::Version(VersionResponse {
                    engine_version: env!("CARGO_PKG_VERSION").to_string(),
                    protocol_version: 1,
                    build_date: env!("PIGTREE_BUILD_DATE").to_string(),
                    commit_hash: env!("PIGTREE_COMMIT_HASH").to_string(),
                    capabilities: vec!["scan".to_string(), "remediation".to_string()],
                }));
            }
            Some(command_request::Request::Cancel(cancel)) => {
                resp.response = Some(command_response::Response::Cancel(CancelResponse {
                    cancelled: true,
                    message: format!("Request {} cancelled", cancel.target_request_id),
                }));
            }
            Some(command_request::Request::Shutdown(_)) => {
                is_shutdown = true;
                resp.response = Some(command_response::Response::Shutdown(ShutdownResponse {
                    status: 0,
                }));
            }
            _ => {
                resp.status = 1;
                resp.error_code = "UNKNOWN_COMMAND".to_string();
                resp.error_message = "Requested command verb is not supported".to_string();
            }
        }

        if let Err(err) = session.send_response(&resp) {
            eprintln!("Failed to send response: {err}");
            break;
        }

        if is_shutdown {
            break;
        }
    }

    0
}
