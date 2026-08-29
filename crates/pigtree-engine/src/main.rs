//! Dedicated private session host engine (pigtree-engine.exe).

use pigtree_ipc::server::EngineServerSession;
use pigtree_ipc::win32::*;
use pigtree_ipc::FrameReadiness;
use pigtree_protocol::protobuf::{
    command_request, command_response, CancelResponse, CommandResponse, CoverageGapReport,
    EchoResponse, ErrorResponse, HealthResponse, PingResponse, ScanResponse, ScanRunOutcome,
    ScopeCoverage, ShutdownResponse, StatusResponse, VersionResponse,
};
use std::env;
use std::process::ExitCode;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

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

    let mut idle_partial_start: Option<Instant> = None;

    // Main command processing loop
    loop {
        let cmd = match session.peek_frame_readiness() {
            Ok(FrameReadiness::Empty) => {
                idle_partial_start = None;
                std::thread::sleep(Duration::from_millis(5));
                continue;
            }
            Ok(FrameReadiness::Partial) => {
                let pstart = *idle_partial_start.get_or_insert_with(Instant::now);
                if pstart.elapsed() >= Duration::from_millis(250) {
                    eprintln!(
                        "Inbound frame stayed partial > 250ms while idle; treating as corrupt"
                    );
                    break;
                }
                std::thread::sleep(Duration::from_millis(5));
                continue;
            }
            Ok(FrameReadiness::Complete) => {
                idle_partial_start = None;
                match session.recv_command() {
                    Ok(Some((_channel_tag, c))) => c,
                    Ok(None) => break, // Client disconnected cleanly
                    Err(pigtree_ipc::IpcError::Io(_)) => break,
                    Err(err) => {
                        eprintln!("Engine error reading command: {err}");
                        break;
                    }
                }
            }
            Err(_) => break,
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
            let delay = Duration::from_millis(effective_delay as u64);
            let mut cancelled = false;
            let mut cancel_request_id = String::new();
            let mut partial_start: Option<Instant> = None;

            while start.elapsed() < delay {
                match session.peek_frame_readiness() {
                    Ok(FrameReadiness::Empty) => {
                        partial_start = None;
                        std::thread::sleep(Duration::from_millis(5));
                    }
                    Ok(FrameReadiness::Partial) => {
                        let pstart = *partial_start.get_or_insert_with(Instant::now);
                        if pstart.elapsed() >= Duration::from_millis(250) {
                            eprintln!("Inbound frame stayed partial > 250ms during delay");
                            return 0;
                        }
                        std::thread::sleep(Duration::from_millis(5));
                    }
                    Ok(FrameReadiness::Complete) => {
                        partial_start = None;
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
                    Err(_) => return 0,
                }
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
            Some(command_request::Request::Scan(scan_req)) => {
                let validated_target = match pigtree_ipc::validator::validate_scan_target(
                    &scan_req.target_path,
                ) {
                    Ok(v) => v,
                    Err(val_err) => {
                        let (error_code, error_msg) = match &val_err {
                            pigtree_ipc::validator::TargetValidationError::UnsupportedFileSystem { .. } => {
                                ("UNSUPPORTED_FILESYSTEM", val_err.to_string())
                            }
                            _ => ("INVALID_TARGET", val_err.to_string()),
                        };
                        resp.status = 1;
                        resp.error_code = error_code.to_string();
                        resp.error_message = error_msg.clone();
                        resp.response = Some(command_response::Response::Error(ErrorResponse {
                            code: error_code.to_string(),
                            message: error_msg,
                            details: String::new(),
                        }));
                        if let Err(err) = session.send_response(&resp) {
                            eprintln!("Failed to send response: {err}");
                            break;
                        }
                        continue;
                    }
                };

                let worker_exe = match pigtree_engine::resolve_scan_worker_exe() {
                    Some(exe) => exe,
                    None => {
                        let error_msg =
                            "Scan worker executable 'pigtree-scan-worker.exe' not found"
                                .to_string();
                        resp.status = 1;
                        resp.error_code = "WORKER_NOT_FOUND".to_string();
                        resp.error_message = error_msg.clone();
                        resp.response = Some(command_response::Response::Error(ErrorResponse {
                            code: "WORKER_NOT_FOUND".to_string(),
                            message: error_msg,
                            details: String::new(),
                        }));
                        if let Err(err) = session.send_response(&resp) {
                            eprintln!("Failed to send response: {err}");
                            break;
                        }
                        continue;
                    }
                };

                let cancel_handle = match pigtree_ipc::CancelHandle::new() {
                    Ok(h) => h,
                    Err(err) => {
                        let error_msg = format!("Failed to create cancel handle: {err}");
                        resp.status = 1;
                        resp.error_code = "INTERNAL_ERROR".to_string();
                        resp.error_message = error_msg.clone();
                        resp.response = Some(command_response::Response::Error(ErrorResponse {
                            code: "INTERNAL_ERROR".to_string(),
                            message: error_msg,
                            details: String::new(),
                        }));
                        if let Err(err) = session.send_response(&resp) {
                            eprintln!("Failed to send response: {err}");
                            break;
                        }
                        continue;
                    }
                };

                let (progress_tx, progress_rx) = std::sync::mpsc::channel();
                let worker_exe_clone = worker_exe.clone();
                let target_clone = validated_target.canonical_path.clone();
                let cancel_clone = cancel_handle.clone();
                let op_id = if scan_req.operation_id.is_empty() {
                    cmd.request_id.clone()
                } else {
                    scan_req.operation_id.clone()
                };
                let op_id_thread = op_id.clone();
                let started_time = Instant::now();
                let started_iso = pigtree_engine::format_utc_iso(SystemTime::now());

                let mut runner_handle = Some(std::thread::spawn(move || {
                    pigtree_engine::launch_scan_worker_with_progress(
                        &worker_exe_clone,
                        &target_clone,
                        &cancel_clone,
                        &op_id_thread,
                        Some(move |p| {
                            let _ = progress_tx.send(p);
                        }),
                    )
                }));

                let active_req_id = cmd.request_id.clone();
                let active_op_id = op_id.clone();
                let active_target_str = scan_req.target_path.clone();

                let mut scan_result = None;
                let mut last_progress_send = Instant::now() - Duration::from_millis(100);
                let mut pending_progress = None;
                let mut partial_start: Option<Instant> = None;

                while scan_result.is_none() {
                    // Drain and coalesce progress to latest
                    while let Ok(progress) = progress_rx.try_recv() {
                        pending_progress = Some(progress);
                    }

                    // Rate limit progress sending: send at most every 100ms
                    if let Some(progress) = pending_progress.take() {
                        if last_progress_send.elapsed() >= Duration::from_millis(100) {
                            let progress_resp = CommandResponse {
                                request_id: active_req_id.clone(),
                                status: 0,
                                error_code: String::new(),
                                error_message: String::new(),
                                response: Some(command_response::Response::ScanProgress(progress)),
                            };
                            if let Err(err) = session.send_progress(&progress_resp) {
                                eprintln!("Failed to send progress: {err}");
                                cancel_handle.cancel();
                                if let Some(h) = runner_handle.take() {
                                    let _ = h.join();
                                }
                                return 0;
                            }
                            last_progress_send = Instant::now();
                        } else {
                            pending_progress = Some(progress);
                        }
                    }

                    // Poll frame readiness non-consumingly
                    match session.peek_frame_readiness() {
                        Ok(FrameReadiness::Empty) => {
                            partial_start = None;
                        }
                        Ok(FrameReadiness::Partial) => {
                            let pstart = *partial_start.get_or_insert_with(Instant::now);
                            if pstart.elapsed() >= Duration::from_millis(250) {
                                eprintln!(
                                    "Inbound frame stayed partial > 250ms during scan; treating as corrupt"
                                );
                                cancel_handle.cancel();
                                if let Some(h) = runner_handle.take() {
                                    let _ = h.join();
                                }
                                return 0;
                            }
                        }
                        Ok(FrameReadiness::Complete) => {
                            partial_start = None;
                            match session.recv_command() {
                                Ok(Some((_tag, incoming_cmd))) => {
                                    match incoming_cmd.request {
                                        Some(command_request::Request::Cancel(cancel)) => {
                                            if cancel.target_request_id.is_empty()
                                                || cancel.target_request_id == active_req_id
                                                || cancel.target_request_id == active_op_id
                                                || cancel.target_request_id == "scan"
                                            {
                                                // Signal worker cancellation; do NOT send separate CancelResponse.
                                                // Exactly one terminal ScanResponse settles a valid cancellation.
                                                cancel_handle.cancel();
                                            }
                                        }
                                        Some(command_request::Request::Status(_)) => {
                                            let status_resp = CommandResponse {
                                                request_id: incoming_cmd.request_id,
                                                status: 0,
                                                error_code: String::new(),
                                                error_message: String::new(),
                                                response: Some(
                                                    command_response::Response::StatusPayload(
                                                        StatusResponse {
                                                            state: "SCANNING".to_string(),
                                                            active_runs: 1,
                                                            total_observations: 0,
                                                            session_id: session_id.clone(),
                                                        },
                                                    ),
                                                ),
                                            };
                                            let _ = session.send_response(&status_resp);
                                        }
                                        _ => {
                                            let busy_resp = CommandResponse {
                                                request_id: incoming_cmd.request_id,
                                                status: 1,
                                                error_code: "BUSY".to_string(),
                                                error_message:
                                                    "Engine is busy executing an active scan operation"
                                                        .to_string(),
                                                response: Some(command_response::Response::Error(
                                                    ErrorResponse {
                                                        code: "BUSY".to_string(),
                                                        message:
                                                            "Engine is busy executing an active scan operation"
                                                                .to_string(),
                                                        details: String::new(),
                                                    },
                                                )),
                                            };
                                            let _ = session.send_response(&busy_resp);
                                        }
                                    }
                                }
                                Ok(None) => {
                                    cancel_handle.cancel();
                                    if let Some(h) = runner_handle.take() {
                                        let _ = h.join();
                                    }
                                    return 0;
                                }
                                Err(err) => {
                                    eprintln!("Error reading complete command: {err}");
                                    cancel_handle.cancel();
                                    if let Some(h) = runner_handle.take() {
                                        let _ = h.join();
                                    }
                                    return 0;
                                }
                            }
                        }
                        Err(err) => {
                            eprintln!("Fatal error on frame readiness check: {err}");
                            cancel_handle.cancel();
                            if let Some(h) = runner_handle.take() {
                                let _ = h.join();
                            }
                            return 0;
                        }
                    }

                    if let Some(h) = runner_handle.as_ref() {
                        if h.is_finished() {
                            scan_result = Some(runner_handle.take().unwrap().join());
                        } else {
                            std::thread::sleep(Duration::from_millis(5));
                        }
                    }
                }

                // Ensure runner thread is joined
                if let Some(h) = runner_handle.take() {
                    let res = h.join();
                    if scan_result.is_none() {
                        scan_result = Some(res);
                    }
                }

                // Drain any final pending progress event
                while let Ok(progress) = progress_rx.try_recv() {
                    pending_progress = Some(progress);
                }
                if let Some(progress) = pending_progress {
                    let progress_resp = CommandResponse {
                        request_id: active_req_id.clone(),
                        status: 0,
                        error_code: String::new(),
                        error_message: String::new(),
                        response: Some(command_response::Response::ScanProgress(progress)),
                    };
                    let _ = session.send_progress(&progress_resp);
                }

                let completed_iso = pigtree_engine::format_utc_iso(SystemTime::now());
                let duration_ms = started_time.elapsed().as_millis() as u64;

                let scan_response = match scan_result {
                    Some(Ok(Ok(graph))) => {
                        let outcome = match graph.terminal().outcome {
                            pigtree_protocol::RunOutcome::Finished => ScanRunOutcome::Finished,
                            pigtree_protocol::RunOutcome::Cancelled => ScanRunOutcome::Cancelled,
                            pigtree_protocol::RunOutcome::Failed => ScanRunOutcome::Failed,
                        };
                        let scope_coverage = if outcome == ScanRunOutcome::Finished {
                            if graph.gaps().is_empty() {
                                ScopeCoverage::Complete
                            } else {
                                ScopeCoverage::Partial
                            }
                        } else {
                            ScopeCoverage::Partial
                        };

                        let coverage_gaps: Vec<CoverageGapReport> = graph
                            .gaps()
                            .iter()
                            .map(|g| CoverageGapReport {
                                display_path: g.path.clone(),
                                status_code: format!("ERROR_{}", g.error_code),
                                native_error: g.error_code,
                                error_message: g.error_message.clone(),
                            })
                            .collect();

                        ScanResponse {
                            operation_id: active_op_id,
                            target_path: active_target_str,
                            run_outcome: outcome as i32,
                            observation_started_iso: started_iso,
                            observation_completed_iso: completed_iso,
                            scope_coverage: scope_coverage as i32,
                            directory_count: graph.terminal().total_directories,
                            file_count: graph.terminal().total_files,
                            special_count: 0,
                            logical_bytes: graph.terminal().total_logical_bytes,
                            allocated_bytes: graph.terminal().total_allocated_bytes,
                            allocated_bytes_known: graph.allocated_bytes_known(),
                            coverage_gaps,
                            duration_ms,
                        }
                    }
                    Some(Ok(Err(_err))) => {
                        let outcome = if cancel_handle.is_cancelled() {
                            ScanRunOutcome::Cancelled
                        } else {
                            ScanRunOutcome::Failed
                        };
                        ScanResponse {
                            operation_id: active_op_id,
                            target_path: active_target_str,
                            run_outcome: outcome as i32,
                            observation_started_iso: started_iso,
                            observation_completed_iso: completed_iso,
                            scope_coverage: ScopeCoverage::Indeterminate as i32,
                            directory_count: 0,
                            file_count: 0,
                            special_count: 0,
                            logical_bytes: 0,
                            allocated_bytes: 0,
                            allocated_bytes_known: false,
                            coverage_gaps: vec![],
                            duration_ms,
                        }
                    }
                    _ => {
                        let outcome = if cancel_handle.is_cancelled() {
                            ScanRunOutcome::Cancelled
                        } else {
                            ScanRunOutcome::Failed
                        };
                        ScanResponse {
                            operation_id: active_op_id,
                            target_path: active_target_str,
                            run_outcome: outcome as i32,
                            observation_started_iso: started_iso,
                            observation_completed_iso: completed_iso,
                            scope_coverage: ScopeCoverage::Indeterminate as i32,
                            directory_count: 0,
                            file_count: 0,
                            special_count: 0,
                            logical_bytes: 0,
                            allocated_bytes: 0,
                            allocated_bytes_known: false,
                            coverage_gaps: vec![],
                            duration_ms,
                        }
                    }
                };

                resp.response = Some(command_response::Response::ScanResponse(scan_response));
                if let Err(err) = session.send_response(&resp) {
                    eprintln!("Failed to send scan terminal response: {err}");
                    break;
                }
                continue;
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
