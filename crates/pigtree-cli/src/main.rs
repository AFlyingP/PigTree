//! Command-line interface and automation client (pigtree.exe).

use pigtree_ipc::client::EngineClientSession;
use pigtree_ipc::win32::*;
use pigtree_protocol::json::{
    format_cancelled_envelope, format_diagnostic, format_echo_response, format_error_envelope,
    format_health_response, format_ping_response, format_status_response, format_success_envelope,
    format_version_response,
};
use std::env;
use std::ffi::c_void;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::ptr::null_mut;
use std::sync::atomic::{AtomicBool, AtomicPtr, Ordering};

static CANCELLED: AtomicBool = AtomicBool::new(false);
static CANCEL_EVENT: AtomicPtr<c_void> = AtomicPtr::new(null_mut());

unsafe extern "system" fn ctrl_handler(ctrl_type: DWORD) -> BOOL {
    if ctrl_type == CTRL_C_EVENT || ctrl_type == CTRL_BREAK_EVENT {
        CANCELLED.store(true, Ordering::SeqCst);
        let h_event = CANCEL_EVENT.load(Ordering::SeqCst);
        if !h_event.is_null() && h_event != INVALID_HANDLE_VALUE {
            SetEvent(h_event);
        }
        return 1; // TRUE: Handled
    }
    0
}

fn find_engine_binary(override_path: Option<&str>) -> Result<PathBuf, String> {
    if let Some(p) = override_path {
        let path = PathBuf::from(p);
        if path.exists() {
            return Ok(path);
        } else {
            return Err(format!("Specified engine binary not found: {p}"));
        }
    }

    // 1. Sibling to current executable
    if let Ok(current_exe) = env::current_exe() {
        if let Some(parent) = current_exe.parent() {
            let candidate = parent.join("pigtree-engine.exe");
            if candidate.exists() {
                return Ok(candidate);
            }
        }
    }

    // 2. Target directory check (dev mode)
    for target_dir in &["target/debug", "target/release", "."] {
        let candidate = Path::new(target_dir).join("pigtree-engine.exe");
        if candidate.exists() {
            return Ok(candidate);
        }
    }

    Err("Could not locate pigtree-engine.exe binary. Specify via --engine-path".to_string())
}

fn print_help() {
    println!(
        "PigTree CLI - Disk Space Analysis and Guarded Storage Remediation

USAGE:
    pigtree.exe [OPTIONS] <SUBCOMMAND>

SUBCOMMANDS:
    health [--include-memory]  Query session host engine health, uptime, and resource usage
    ping                       Send timestamp ping request to engine session
    echo <TEXT>                Echo text payload through engine session
    status                     Query engine session runtime state
    version                    Inspect engine and protocol wire versions
    shutdown                   Cleanly terminate private session host
    help                       Print this help message

OPTIONS:
    --engine-path <PATH>       Path to pigtree-engine.exe binary
    --format <json|ndjson>     Output format (default: json)
    -h, --help                 Print help information
    -V, --version              Print CLI version
"
    );
}

fn handle_rpc_result<T>(
    verb: &str,
    session: &mut EngineClientSession,
    op: impl FnOnce(&mut EngineClientSession) -> Result<T, pigtree_ipc::IpcError>,
    formatter: impl FnOnce(&T) -> String,
) -> u8 {
    match op(session) {
        Ok(resp) => {
            let data_json = formatter(&resp);
            println!("{}", format_success_envelope(verb, &data_json));
            0
        }
        Err(pigtree_ipc::IpcError::Cancelled) => {
            let _ = session.cancel_request(verb);
            eprintln!(
                "{}",
                format_diagnostic("WARN", "pigtree_cli", "Operation cancelled by user")
            );
            println!(
                "{}",
                format_cancelled_envelope(verb, "Operation cancelled by user")
            );
            3
        }
        Err(err) => {
            let msg = format!("{verb} failed: {err}");
            eprintln!("{}", format_diagnostic("ERROR", "pigtree_cli", &msg));
            println!("{}", format_error_envelope(verb, "OPERATION_FAILED", &msg));
            1
        }
    }
}

fn main() -> ExitCode {
    ExitCode::from(run())
}

fn run() -> u8 {
    let h_cancel = unsafe { CreateEventW(null_mut(), TRUE, FALSE, null_mut()) };
    if !h_cancel.is_null() && h_cancel != INVALID_HANDLE_VALUE {
        CANCEL_EVENT.store(h_cancel, Ordering::SeqCst);
    }

    unsafe {
        SetConsoleCtrlHandler(Some(ctrl_handler), 1);
    }

    struct EventCleanup(HANDLE);
    impl Drop for EventCleanup {
        fn drop(&mut self) {
            CANCEL_EVENT.store(null_mut(), Ordering::SeqCst);
            if !self.0.is_null() && self.0 != INVALID_HANDLE_VALUE {
                unsafe {
                    CloseHandle(self.0);
                }
            }
        }
    }
    let _event_cleanup = EventCleanup(h_cancel);

    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!(
            "{}",
            format_diagnostic("ERROR", "pigtree_cli", "No subcommand specified")
        );
        println!(
            "{}",
            format_error_envelope("cli", "COMMAND_ERROR", "No subcommand specified")
        );
        return 2;
    }

    let mut engine_path_override: Option<String> = None;
    let mut _format = "json".to_string();
    let mut subcommand: Option<String> = None;
    let mut sub_args: Vec<String> = Vec::new();
    let mut test_delay_ms: u32 = env::var("PIGTREE_TEST_DELAY_MS")
        .ok()
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(0);

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "-h" | "--help" | "help" => {
                print_help();
                return 0;
            }
            "-V" | "--version" => {
                let ver_json = format!(
                    r#"{{"cli_version":"{}","protocol_version":1}}"#,
                    env!("CARGO_PKG_VERSION")
                );
                println!("{}", format_success_envelope("cli-version", &ver_json));
                return 0;
            }
            "--engine-path" => {
                if i + 1 < args.len() && !args[i + 1].starts_with("--") {
                    engine_path_override = Some(args[i + 1].clone());
                    i += 1;
                } else {
                    eprintln!(
                        "{}",
                        format_diagnostic(
                            "ERROR",
                            "pigtree_cli",
                            "Missing value for --engine-path"
                        )
                    );
                    println!(
                        "{}",
                        format_error_envelope(
                            "cli",
                            "COMMAND_ERROR",
                            "Missing value for --engine-path"
                        )
                    );
                    return 2;
                }
            }
            "--test-delay-ms" => {
                if i + 1 < args.len() && !args[i + 1].starts_with("--") {
                    if let Ok(d) = args[i + 1].parse::<u32>() {
                        test_delay_ms = d;
                    }
                    i += 1;
                } else {
                    eprintln!(
                        "{}",
                        format_diagnostic(
                            "ERROR",
                            "pigtree_cli",
                            "Missing value for --test-delay-ms"
                        )
                    );
                    println!(
                        "{}",
                        format_error_envelope(
                            "cli",
                            "COMMAND_ERROR",
                            "Missing value for --test-delay-ms"
                        )
                    );
                    return 2;
                }
            }
            "--format" => {
                if i + 1 < args.len() && !args[i + 1].starts_with("--") {
                    let fmt_val = &args[i + 1];
                    if fmt_val != "json" && fmt_val != "ndjson" {
                        let msg = format!(
                            "Invalid value for --format: '{fmt_val}'. Expected 'json' or 'ndjson'"
                        );
                        eprintln!("{}", format_diagnostic("ERROR", "pigtree_cli", &msg));
                        println!("{}", format_error_envelope("cli", "COMMAND_ERROR", &msg));
                        return 2;
                    }
                    _format = fmt_val.clone();
                    i += 1;
                } else {
                    eprintln!(
                        "{}",
                        format_diagnostic("ERROR", "pigtree_cli", "Missing value for --format")
                    );
                    println!(
                        "{}",
                        format_error_envelope("cli", "COMMAND_ERROR", "Missing value for --format")
                    );
                    return 2;
                }
            }
            "--json" => {
                _format = "json".to_string();
            }
            other if !other.starts_with('-') && subcommand.is_none() => {
                subcommand = Some(other.to_string());
            }
            other => {
                sub_args.push(other.to_string());
            }
        }
        i += 1;
    }

    let cmd = match subcommand {
        Some(c) => c,
        None => {
            eprintln!(
                "{}",
                format_diagnostic("ERROR", "pigtree_cli", "No subcommand specified")
            );
            println!(
                "{}",
                format_error_envelope("cli", "COMMAND_ERROR", "No subcommand specified")
            );
            return 2;
        }
    };

    let valid_subcommands = ["ping", "echo", "health", "status", "version", "shutdown"];
    if !valid_subcommands.contains(&cmd.as_str()) {
        let msg = format!("Unknown subcommand: {cmd}");
        eprintln!("{}", format_diagnostic("ERROR", "pigtree_cli", &msg));
        println!("{}", format_error_envelope("cli", "COMMAND_ERROR", &msg));
        return 2;
    }

    if CANCELLED.load(Ordering::SeqCst) {
        eprintln!(
            "{}",
            format_diagnostic("WARN", "pigtree_cli", "Operation cancelled by user")
        );
        println!(
            "{}",
            format_cancelled_envelope(&cmd, "Operation cancelled by user")
        );
        return 3;
    }

    let engine_binary = match find_engine_binary(engine_path_override.as_deref()) {
        Ok(b) => b,
        Err(err) => {
            eprintln!("{}", format_diagnostic("ERROR", "pigtree_cli", &err));
            println!("{}", format_error_envelope("cli", "OPERATION_FAILED", &err));
            return 1;
        }
    };

    eprintln!(
        "{}",
        format_diagnostic(
            "INFO",
            "pigtree_cli",
            &format!("Spawning engine session from {}", engine_binary.display())
        )
    );

    let mut session = match EngineClientSession::launch(&engine_binary) {
        Ok(s) => s,
        Err(err) => {
            let msg = format!("Failed to spawn private engine host: {err}");
            eprintln!("{}", format_diagnostic("ERROR", "pigtree_cli", &msg));
            println!("{}", format_error_envelope("cli", "OPERATION_FAILED", &msg));
            return 1;
        }
    };

    eprintln!(
        "{}",
        format_diagnostic(
            "INFO",
            "pigtree_cli",
            &format!(
                "Established authenticated session ID {} (Engine PID {})",
                session.session_id(),
                session.engine_pid()
            )
        )
    );

    if CANCELLED.load(Ordering::SeqCst) {
        let _ = session.cancel_request(&cmd);
        let _ = session.shutdown();
        eprintln!(
            "{}",
            format_diagnostic("WARN", "pigtree_cli", "Operation cancelled by user")
        );
        println!(
            "{}",
            format_cancelled_envelope(&cmd, "Operation cancelled by user")
        );
        return 3;
    }

    let cancel_opt = if !h_cancel.is_null() && h_cancel != INVALID_HANDLE_VALUE {
        Some(h_cancel)
    } else {
        None
    };

    match cmd.as_str() {
        "ping" => handle_rpc_result(
            "ping",
            &mut session,
            |s| s.ping_with_options(test_delay_ms, cancel_opt),
            format_ping_response,
        ),
        "echo" => {
            let payload = sub_args.join(" ");
            handle_rpc_result(
                "echo",
                &mut session,
                |s| s.echo_with_options(&payload, test_delay_ms, cancel_opt),
                format_echo_response,
            )
        }
        "health" => {
            let include_memory = sub_args.iter().any(|a| a == "--include-memory");
            handle_rpc_result(
                "health",
                &mut session,
                |s| s.health_with_options(include_memory, test_delay_ms, cancel_opt),
                format_health_response,
            )
        }
        "status" => handle_rpc_result(
            "status",
            &mut session,
            |s| s.status_with_options(test_delay_ms, cancel_opt),
            format_status_response,
        ),
        "version" => handle_rpc_result(
            "version",
            &mut session,
            |s| s.version_with_options(test_delay_ms, cancel_opt),
            format_version_response,
        ),
        "shutdown" => match session.shutdown() {
            Ok(()) => {
                println!(
                    "{}",
                    format_success_envelope("shutdown", r#"{"shutdown":true}"#)
                );
                0
            }
            Err(err) => {
                let msg = format!("Shutdown failed: {err}");
                eprintln!("{}", format_diagnostic("ERROR", "pigtree_cli", &msg));
                println!(
                    "{}",
                    format_error_envelope("shutdown", "OPERATION_FAILED", &msg)
                );
                1
            }
        },
        unknown => {
            let msg = format!("Unknown subcommand: {unknown}");
            eprintln!("{}", format_diagnostic("ERROR", "pigtree_cli", &msg));
            println!("{}", format_error_envelope("cli", "COMMAND_ERROR", &msg));
            2
        }
    }
}
