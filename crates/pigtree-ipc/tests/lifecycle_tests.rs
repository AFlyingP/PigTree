use pigtree_ipc::bootstrap::{read_bootstrap_nonce, spawn_engine, BootstrapPipe};
use pigtree_ipc::client::{EngineClientSession, ScanCallOutcome};
use pigtree_ipc::error::IpcError;
use pigtree_ipc::job::JobObject;
use pigtree_ipc::pipe::{format_pipe_name, NamedPipeClient, NamedPipeServer};
use pigtree_ipc::security::generate_nonce;
use pigtree_ipc::server::EngineServerSession;
use pigtree_ipc::transport::{FrameReadiness, FramedSession};
use pigtree_ipc::win32::*;
use pigtree_protocol::frame::{ChannelTag, FrameFlags};
use pigtree_protocol::protobuf::{AuthHandshakeRequest, AuthHandshakeResponse};
use pigtree_protocol::Message;
use std::path::PathBuf;
use std::time::Duration;

fn get_engine_exe() -> PathBuf {
    for candidate in &[
        "target/debug/pigtree-engine.exe",
        "../../target/debug/pigtree-engine.exe",
        "../target/debug/pigtree-engine.exe",
    ] {
        let p = PathBuf::from(candidate);
        if p.exists() {
            return p.canonicalize().unwrap_or(p);
        }
    }
    if let Ok(cur) = std::env::current_exe() {
        if let Some(parent) = cur.parent() {
            let p = parent.join("pigtree-engine.exe");
            if p.exists() {
                return p;
            }
            if let Some(grandparent) = parent.parent() {
                let p = grandparent.join("pigtree-engine.exe");
                if p.exists() {
                    return p;
                }
            }
        }
    }
    panic!("pigtree-engine.exe not found; build target first");
}

fn unique_session_id(prefix: &str) -> String {
    let nonce = generate_nonce();
    let hex: String = nonce[0..8].iter().map(|b| format!("{b:02x}")).collect();
    format!("{prefix}-{}-{hex}", std::process::id())
}

#[test]
fn test_bootstrap_pipe_write_read_no_deadlock() {
    let mut bootstrap = BootstrapPipe::create().expect("create bootstrap pipe");
    let nonce = generate_nonce();
    bootstrap.write_nonce(&nonce).expect("write nonce");
    let h_read = bootstrap.into_read_handle();
    let read_nonce = unsafe { read_bootstrap_nonce(h_read) }.expect("read nonce");
    assert_eq!(nonce, read_nonce);
}

#[test]
fn test_spawn_and_connect() {
    let engine_exe = get_engine_exe();

    let job = JobObject::create_kill_on_close().expect("create job");
    let nonce = generate_nonce();
    let mut bootstrap = BootstrapPipe::create().expect("create bootstrap pipe");
    bootstrap.write_nonce(&nonce).expect("write nonce");

    let sess_id = unique_session_id("spawn");
    let pipe_name = format_pipe_name(&sess_id);

    let child =
        spawn_engine(&engine_exe, &pipe_name, &sess_id, &bootstrap, &job).expect("spawn engine");

    std::thread::sleep(Duration::from_millis(50));
    let client = NamedPipeClient::connect(&pipe_name, 5000).expect("connect client");
    assert_eq!(client.get_server_pid().unwrap(), child.pid);

    child.terminate(0).expect("terminate child");
}

#[test]
fn test_pipe_connect_in_proc() {
    let sess_id = unique_session_id("inproc");
    let pipe_name = format_pipe_name(&sess_id);
    let server = NamedPipeServer::create(&pipe_name).expect("create server");

    let client_handle = std::thread::spawn({
        let pipe_name = pipe_name.clone();
        move || NamedPipeClient::connect(&pipe_name, 5000).expect("connect client")
    });

    let _stream = server.accept().expect("server accept");
    let client_stream = client_handle.join().expect("join");
    assert_eq!(client_stream.role(), pigtree_ipc::pipe::PipeRole::Client);
}

#[test]
fn test_engine_client_session_full_lifecycle() {
    let engine_exe = get_engine_exe();

    let mut session = EngineClientSession::launch(&engine_exe).expect("launch engine session");
    assert!(session.engine_pid() > 0);
    assert!(!session.session_id().is_empty());

    let ping_resp = session.ping().expect("ping");
    assert!(ping_resp.timestamp_utc_ms > 0);

    let echo_resp = session.echo("Lifecycle Test").expect("echo");
    assert_eq!(echo_resp.payload, "Lifecycle Test");

    let health_resp = session.health(true).expect("health");
    assert_eq!(health_resp.status, "HEALTHY");

    let status_resp = session.status().expect("status");
    assert_eq!(status_resp.state, "IDLE");

    let ver_resp = session.version().expect("version");
    assert_eq!(ver_resp.engine_version, "0.1.0");
    assert_eq!(ver_resp.protocol_version, 1);

    session.shutdown().expect("shutdown");
}

#[test]
fn test_auth_handshake_failure_wrong_nonce() {
    let sess_id = unique_session_id("authfail");
    let pipe_name = format_pipe_name(&sess_id);
    let correct_nonce = generate_nonce();
    let wrong_nonce = [99u8; 32];

    let server_handle = std::thread::spawn({
        let pipe_name = pipe_name.clone();
        move || {
            let res = EngineServerSession::accept(&pipe_name, correct_nonce);
            assert!(res.is_err(), "Server must reject wrong bootstrap nonce");
        }
    });

    std::thread::sleep(Duration::from_millis(50));
    let stream = NamedPipeClient::connect(&pipe_name, 3000).expect("connect client");
    let mut framed = FramedSession::new(stream);
    let client_nonce = generate_nonce();
    let client_pid = unsafe { GetCurrentProcessId() };

    let req = AuthHandshakeRequest {
        bootstrap_nonce: wrong_nonce.to_vec(),
        client_nonce: client_nonce.to_vec(),
        client_pid,
        client_session_id: 0,
    };
    framed
        .send_message(ChannelTag::Command, FrameFlags::empty(), &req)
        .expect("send req");

    let res: Result<Option<(_, AuthHandshakeResponse)>, _> = framed.recv_message();
    let (_, resp) = res
        .expect("recv_message should succeed")
        .expect("response frame should be present");
    assert_eq!(resp.status, 1, "Server must return unauthorized status 1");
    assert!(
        resp.error_message.contains("Invalid bootstrap") || resp.error_message.contains("mismatch"),
        "Error message must describe rejection reason: {}",
        resp.error_message
    );

    server_handle.join().expect("join server");
}

#[test]
fn test_auth_handshake_failure_wrong_pid() {
    let sess_id = unique_session_id("pidfail");
    let pipe_name = format_pipe_name(&sess_id);
    let bootstrap_nonce = generate_nonce();

    let server_handle = std::thread::spawn({
        let pipe_name = pipe_name.clone();
        move || {
            let res = EngineServerSession::accept(&pipe_name, bootstrap_nonce);
            assert!(res.is_err(), "Server must reject mismatched client PID");
        }
    });

    std::thread::sleep(Duration::from_millis(50));
    let stream = NamedPipeClient::connect(&pipe_name, 3000).expect("connect client");
    let mut framed = FramedSession::new(stream);
    let client_nonce = generate_nonce();
    let bogus_pid = 999999;

    let req = AuthHandshakeRequest {
        bootstrap_nonce: bootstrap_nonce.to_vec(),
        client_nonce: client_nonce.to_vec(),
        client_pid: bogus_pid,
        client_session_id: 0,
    };
    framed
        .send_message(ChannelTag::Command, FrameFlags::empty(), &req)
        .expect("send req");

    let res: Result<Option<(_, AuthHandshakeResponse)>, _> = framed.recv_message();
    let (_, resp) = res
        .expect("recv_message should succeed")
        .expect("response frame should be present");
    assert_eq!(
        resp.status, 1,
        "Server must return unauthorized status 1 on PID mismatch"
    );
    assert!(
        resp.error_message.contains("PID mismatch"),
        "Error message must describe PID mismatch: {}",
        resp.error_message
    );

    server_handle.join().expect("join server");
}

#[test]
fn test_client_in_flight_cancellation_settles() {
    let engine_exe = get_engine_exe();

    let mut session = EngineClientSession::launch(&engine_exe).expect("launch engine session");
    let h_cancel = unsafe { CreateEventW(std::ptr::null_mut(), TRUE, FALSE, std::ptr::null_mut()) };
    let h_cancel_val = h_cancel as usize;

    // Trigger cancel event in background after 50ms
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(50));
        unsafe {
            SetEvent(h_cancel_val as HANDLE);
        }
    });

    // Ping with 2000ms delay and cancel event
    let ping_res = session.ping_with_options(2000, Some(h_cancel));
    assert!(
        matches!(ping_res, Err(IpcError::Cancelled)),
        "Operation must return Cancelled"
    );

    // Transmit CancelRequest
    let cancel_res = session
        .cancel_request("ping")
        .expect("cancel request acknowledged");
    assert!(cancel_res.cancelled);

    unsafe {
        CloseHandle(h_cancel);
    }
}

fn create_temp_scan_tree(name: &str) -> PathBuf {
    let mut dir = std::env::temp_dir();
    dir.push(format!(
        "pigtree_ipc_scan_test_{name}_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let sub_a = dir.join("subA");
    std::fs::create_dir_all(&sub_a).unwrap();
    let mut f1 = std::fs::File::create(sub_a.join("file1.txt")).unwrap();
    std::io::Write::write_all(&mut f1, b"hello world 12345").unwrap(); // 17 bytes

    let mut f2 = std::fs::File::create(sub_a.join("file2.txt")).unwrap();
    std::io::Write::write_all(&mut f2, b"another test file").unwrap(); // 17 bytes

    let sub_b = dir.join("subB");
    std::fs::create_dir_all(&sub_b).unwrap();
    let mut f3 = std::fs::File::create(sub_b.join("file3.bin")).unwrap();
    std::io::Write::write_all(&mut f3, &vec![0u8; 1000]).unwrap(); // 1000 bytes

    dir
}

#[test]
fn test_client_scan_success_with_progress_and_terminal() {
    let engine_exe = get_engine_exe();
    let temp_tree = create_temp_scan_tree("success");
    let target_str = temp_tree.to_str().unwrap();

    let mut session = EngineClientSession::launch(&engine_exe).expect("launch engine session");

    let mut progress_events = Vec::new();
    let scan_resp = session
        .scan_with_progress(
            target_str,
            Some(|p: pigtree_protocol::protobuf::ScanProgress| {
                progress_events.push(p);
            }),
            None,
        )
        .expect("scan should succeed");

    assert_eq!(
        scan_resp.run_outcome,
        pigtree_protocol::protobuf::ScanRunOutcome::Finished as i32
    );
    assert_eq!(
        scan_resp.scope_coverage,
        pigtree_protocol::protobuf::ScopeCoverage::Complete as i32
    );
    assert_eq!(scan_resp.directory_count, 3); // root, subA, subB
    assert_eq!(scan_resp.file_count, 3); // file1, file2, file3
    assert_eq!(scan_resp.logical_bytes, 17 + 17 + 1000);
    assert!(scan_resp.allocated_bytes_known);
    assert!(scan_resp.coverage_gaps.is_empty());
    assert!(!scan_resp.observation_started_iso.is_empty());
    assert!(!scan_resp.observation_completed_iso.is_empty());

    let mut last_seq = 0;
    for p in &progress_events {
        assert_eq!(p.operation_id, scan_resp.operation_id);
        assert!(p.sequence_number > last_seq);
        last_seq = p.sequence_number;
        assert!(p.timestamp_iso.ends_with('Z'));
        assert!(!p.current_phase.is_empty());
        assert!(!p.current_directory.is_empty());
        let norm_cur = p
            .current_directory
            .strip_prefix("\\\\?\\")
            .unwrap_or(&p.current_directory);
        let norm_target = target_str.strip_prefix("\\\\?\\").unwrap_or(target_str);
        assert!(
            norm_cur.starts_with(norm_target),
            "current_directory '{}' must be under target '{}'",
            p.current_directory,
            target_str
        );
    }

    session.shutdown().expect("shutdown");
    let _ = std::fs::remove_dir_all(&temp_tree);
}

#[test]
fn test_client_scan_target_with_spaces() {
    let engine_exe = get_engine_exe();
    let mut dir = std::env::temp_dir();
    dir.push(format!(
        "pigtree spaced dir name test_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let mut f = std::fs::File::create(dir.join("spaced file name.txt")).unwrap();
    std::io::Write::write_all(&mut f, b"content with spaces").unwrap();
    drop(f);

    let mut session = EngineClientSession::launch(&engine_exe).expect("launch engine session");
    let scan_resp = session
        .scan(dir.to_str().unwrap())
        .expect("scan spaced path");

    assert_eq!(
        scan_resp.run_outcome,
        pigtree_protocol::protobuf::ScanRunOutcome::Finished as i32
    );
    assert_eq!(scan_resp.directory_count, 1);
    assert_eq!(scan_resp.file_count, 1);
    assert_eq!(scan_resp.logical_bytes, 19);

    session.shutdown().expect("shutdown");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_client_scan_invalid_target_rejected_command_error() {
    let engine_exe = get_engine_exe();
    let mut session = EngineClientSession::launch(&engine_exe).expect("launch engine session");

    // 1. Nonexistent path rejected
    let invalid_path = r"C:
onexistent_pigtree_dir_test_404_abc";
    let res = session.scan(invalid_path);
    assert!(res.is_err(), "Invalid target path must be rejected");
    match res.err().unwrap() {
        IpcError::CommandError { code, message } => {
            assert_eq!(code, "INVALID_TARGET");
            assert!(message.contains("does not exist") || message.contains("Target"));
        }
        other => panic!("expected CommandError, got {:?}", other),
    }

    // Engine remains healthy
    let ping_resp = session
        .ping()
        .expect("ping after rejected nonexistent scan");
    assert!(ping_resp.timestamp_utc_ms > 0);

    // 2. Standard UNC path rejected
    let unc_path = "\\\\dummy_server\\dummy_share";
    let res_unc = session.scan(unc_path);
    assert!(res_unc.is_err(), "UNC path must be rejected");
    match res_unc.err().unwrap() {
        IpcError::CommandError { code, message } => {
            assert_eq!(code, "INVALID_TARGET");
            assert!(message.contains("UNC") || message.contains("network"));
        }
        other => panic!("expected CommandError, got {:?}", other),
    }

    // Engine remains healthy
    let ping_resp = session.ping().expect("ping after rejected UNC scan");
    assert!(ping_resp.timestamp_utc_ms > 0);

    // 3. Extended UNC path rejected
    let ext_unc_path = "\\\\?\\UNC\\dummy_server\\dummy_share";
    let res_ext_unc = session.scan(ext_unc_path);
    assert!(res_ext_unc.is_err(), "Extended UNC path must be rejected");
    match res_ext_unc.err().unwrap() {
        IpcError::CommandError { code, message } => {
            assert_eq!(code, "INVALID_TARGET");
            assert!(message.contains("UNC") || message.contains("network"));
        }
        other => panic!("expected CommandError, got {:?}", other),
    }

    // Engine remains healthy
    let ping_resp = session
        .ping()
        .expect("ping after rejected extended UNC scan");
    assert!(ping_resp.timestamp_utc_ms > 0);

    session.shutdown().expect("shutdown");
}

#[test]
fn test_client_scan_cancellation_under_2s() {
    let engine_exe = get_engine_exe();
    let temp_tree = create_temp_scan_tree("cancel");
    let target_str = temp_tree.to_str().unwrap();

    let mut session = EngineClientSession::launch(&engine_exe).expect("launch engine session");

    let h_cancel = unsafe { CreateEventW(std::ptr::null_mut(), TRUE, FALSE, std::ptr::null_mut()) };
    let h_cancel_val = h_cancel as usize;

    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(20));
        unsafe {
            SetEvent(h_cancel_val as HANDLE);
        }
    });

    let start = std::time::Instant::now();
    let res = session.scan_with_progress(
        target_str,
        None::<fn(pigtree_protocol::protobuf::ScanProgress)>,
        Some(h_cancel),
    );

    let elapsed = start.elapsed();
    assert!(
        elapsed < Duration::from_secs(2),
        "Cancellation must settle in under 2 seconds, took {:?}",
        elapsed
    );

    assert!(matches!(res, Err(IpcError::Cancelled)));

    // Verify engine remains immediately healthy and usable without stale frames or extra cancel calls
    let ping = session.ping().expect("ping after scan cancellation");
    assert!(ping.timestamp_utc_ms > 0);

    unsafe {
        CloseHandle(h_cancel);
    }
    session.shutdown().expect("shutdown");
    let _ = std::fs::remove_dir_all(&temp_tree);
}

#[test]
fn test_partial_inbound_frame_not_consumed_and_peek_readiness() {
    use std::io::{Read, Write};

    let sess_id = unique_session_id("peek_readiness");
    let pipe_name = format_pipe_name(&sess_id);
    let server = NamedPipeServer::create(&pipe_name).expect("server create");

    let pipe_name_clone = pipe_name.clone();
    let client_thread = std::thread::spawn(move || {
        let mut client = NamedPipeClient::connect(&pipe_name_clone, 3000).expect("client connect");
        // Write 1 byte
        client.write_all(b"P").expect("client write 1 byte");
        client.flush().expect("client flush");
        std::thread::sleep(Duration::from_millis(50));
        // Write rest of 4-byte ping message payload
        client.write_all(b"ING!").expect("client write rest");
        client.flush().expect("client flush");
    });

    let mut server_stream = server.accept().expect("server accept");

    // Wait bounded time for 1 byte to arrive
    let mut readiness1 = FrameReadiness::Empty;
    let start = std::time::Instant::now();
    while readiness1 == FrameReadiness::Empty && start.elapsed() < Duration::from_millis(500) {
        readiness1 = server_stream.peek_frame_readiness().expect("peek 1");
        if readiness1 == FrameReadiness::Empty {
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    assert_eq!(readiness1, FrameReadiness::Partial);

    // Peeking again must STILL report Partial without consuming the byte!
    let readiness2 = server_stream.peek_frame_readiness().expect("peek 2");
    assert_eq!(readiness2, FrameReadiness::Partial);

    // Read the 1 byte directly to prove it wasn't consumed
    let mut buf = [0u8; 1];
    server_stream.read_exact(&mut buf).expect("read byte");
    assert_eq!(&buf, b"P");

    let mut rest = [0u8; 4];
    server_stream.read_exact(&mut rest).expect("read rest");
    assert_eq!(&rest, b"ING!");

    client_thread.join().expect("client thread join");
}

#[test]
fn test_partial_inbound_frame_persistent_causes_bounded_engine_closure() {
    use std::io::Write;

    let engine_exe = get_engine_exe();
    let sess_id = unique_session_id("persistent_partial");
    let pipe_name = format_pipe_name(&sess_id);
    let job = JobObject::create_kill_on_close().expect("create job");
    let nonce = generate_nonce();
    let mut bootstrap = BootstrapPipe::create().expect("create bootstrap pipe");
    bootstrap.write_nonce(&nonce).expect("write nonce");

    let child =
        spawn_engine(&engine_exe, &pipe_name, &sess_id, &bootstrap, &job).expect("spawn engine");

    std::thread::sleep(Duration::from_millis(50));
    let stream = NamedPipeClient::connect(&pipe_name, 5000).expect("connect client");
    let mut framed = FramedSession::new(stream);

    // Handshake
    let client_nonce = generate_nonce();
    let client_pid = unsafe { GetCurrentProcessId() };
    let handshake_req = pigtree_protocol::protobuf::AuthHandshakeRequest {
        bootstrap_nonce: nonce.to_vec(),
        client_nonce: client_nonce.to_vec(),
        client_pid,
        client_session_id: 0,
    };
    framed
        .send_message(ChannelTag::Command, FrameFlags::empty(), &handshake_req)
        .expect("send handshake");
    let (_hdr, _hresp): (_, pigtree_protocol::protobuf::AuthHandshakeResponse) = framed
        .recv_message()
        .expect("recv handshake")
        .expect("handshake response");

    // Write only 1 byte (incomplete frame) and stop
    let mut raw_stream = framed.into_inner();
    raw_stream.write_all(b"P").expect("write 1 byte");
    raw_stream.flush().expect("flush 1 byte");

    // Wait for engine to detect persistent partial frame (> 250ms) and exit cleanly
    let exit_res = child.wait_for_exit(3000);
    assert!(
        exit_res.is_ok(),
        "Engine must exit boundedly when inbound frame is persistently partial"
    );
}

#[test]
fn test_engine_busy_rejection_during_active_scan() {
    let engine_exe = get_engine_exe();
    let temp_tree = create_temp_scan_tree("busy");
    let target_str = temp_tree.to_str().unwrap().to_string();

    let sess_id = unique_session_id("busy");
    let pipe_name = format_pipe_name(&sess_id);
    let job = JobObject::create_kill_on_close().expect("create job");
    let nonce = generate_nonce();
    let mut bootstrap = BootstrapPipe::create().expect("create bootstrap pipe");
    bootstrap.write_nonce(&nonce).expect("write nonce");

    let child =
        spawn_engine(&engine_exe, &pipe_name, &sess_id, &bootstrap, &job).expect("spawn engine");

    std::thread::sleep(Duration::from_millis(50));
    let stream = NamedPipeClient::connect(&pipe_name, 5000).expect("connect client");
    let mut framed = FramedSession::new(stream);

    // Handshake
    let client_nonce = generate_nonce();
    let client_pid = unsafe { GetCurrentProcessId() };
    let handshake_req = pigtree_protocol::protobuf::AuthHandshakeRequest {
        bootstrap_nonce: nonce.to_vec(),
        client_nonce: client_nonce.to_vec(),
        client_pid,
        client_session_id: 0,
    };
    framed
        .send_message(ChannelTag::Command, FrameFlags::empty(), &handshake_req)
        .expect("send handshake");
    let (_hdr, _hresp): (_, pigtree_protocol::protobuf::AuthHandshakeResponse) = framed
        .recv_message()
        .expect("recv handshake")
        .expect("handshake response");

    // Send Scan request with delay to test busy rejection
    let scan_req = pigtree_protocol::protobuf::CommandRequest {
        request_id: "scan-busy-1".to_string(),
        request: Some(pigtree_protocol::protobuf::command_request::Request::Scan(
            pigtree_protocol::protobuf::ScanRequest {
                operation_id: "scan-busy-1".to_string(),
                target_path: target_str,
            },
        )),
    };
    framed
        .send_message(ChannelTag::Command, FrameFlags::empty(), &scan_req)
        .expect("send scan");

    // Immediately send a GetChildren request while scan is active
    let gc_req = pigtree_protocol::protobuf::CommandRequest {
        request_id: "gc-while-busy".to_string(),
        request: Some(
            pigtree_protocol::protobuf::command_request::Request::GetChildren(
                pigtree_protocol::protobuf::GetChildrenRequest {
                    operation_id: "scan-busy-1".to_string(),
                    parent_id: 1,
                    offset: 0,
                    limit: 100,
                },
            ),
        ),
    };
    framed
        .send_message(ChannelTag::Command, FrameFlags::empty(), &gc_req)
        .expect("send get_children");

    // Read messages from pipe until scan completes
    let mut received_busy = false;
    let mut received_scan_response = false;

    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while !received_scan_response && std::time::Instant::now() < deadline {
        match framed.peek_frame_readiness().expect("peek frame") {
            FrameReadiness::Complete => {
                let frame = framed
                    .recv_frame()
                    .expect("recv frame")
                    .expect("frame present");
                let resp = pigtree_protocol::protobuf::CommandResponse::decode(&frame.payload[..])
                    .expect("decode response");

                if resp.request_id == "gc-while-busy" {
                    if resp.status == 1
                        && (resp.error_code == "BUSY" || resp.error_message.contains("busy"))
                    {
                        received_busy = true;
                    }
                } else if resp.request_id == "scan-busy-1" {
                    if let Some(
                        pigtree_protocol::protobuf::command_response::Response::ScanResponse(_),
                    ) = resp.response
                    {
                        received_scan_response = true;
                    }
                }
            }
            FrameReadiness::Empty | FrameReadiness::Partial => {
                std::thread::sleep(Duration::from_millis(5));
            }
        }
    }

    assert!(
        received_busy,
        "GetChildren during active scan must be rejected with BUSY"
    );
    assert!(
        received_scan_response,
        "Active scan should complete and send ScanResponse"
    );

    let _ = child.terminate(0);
    let _ = std::fs::remove_dir_all(&temp_tree);
}

#[test]
fn test_partial_inbound_frame_during_active_scan_causes_cleanup() {
    use std::io::Write;

    let engine_exe = get_engine_exe();
    let temp_tree = create_temp_scan_tree("partial_active_scan");
    let target_str = temp_tree.to_str().unwrap().to_string();

    let sess_id = unique_session_id("partial_active");
    let pipe_name = format_pipe_name(&sess_id);
    let job = JobObject::create_kill_on_close().expect("create job");
    let nonce = generate_nonce();
    let mut bootstrap = BootstrapPipe::create().expect("create bootstrap pipe");
    bootstrap.write_nonce(&nonce).expect("write nonce");

    let child =
        spawn_engine(&engine_exe, &pipe_name, &sess_id, &bootstrap, &job).expect("spawn engine");

    std::thread::sleep(Duration::from_millis(50));
    let stream = NamedPipeClient::connect(&pipe_name, 5000).expect("connect client");
    let mut framed = FramedSession::new(stream);

    // Handshake
    let client_nonce = generate_nonce();
    let client_pid = unsafe { GetCurrentProcessId() };
    let handshake_req = pigtree_protocol::protobuf::AuthHandshakeRequest {
        bootstrap_nonce: nonce.to_vec(),
        client_nonce: client_nonce.to_vec(),
        client_pid,
        client_session_id: 0,
    };
    framed
        .send_message(ChannelTag::Command, FrameFlags::empty(), &handshake_req)
        .expect("send handshake");
    let (_hdr, _hresp): (_, pigtree_protocol::protobuf::AuthHandshakeResponse) = framed
        .recv_message()
        .expect("recv handshake")
        .expect("handshake response");

    // Send Scan request
    let scan_req = pigtree_protocol::protobuf::CommandRequest {
        request_id: "scan-partial-active".to_string(),
        request: Some(pigtree_protocol::protobuf::command_request::Request::Scan(
            pigtree_protocol::protobuf::ScanRequest {
                operation_id: "scan-partial-active".to_string(),
                target_path: target_str,
            },
        )),
    };
    framed
        .send_message(ChannelTag::Command, FrameFlags::empty(), &scan_req)
        .expect("send scan");

    // Send 1 byte (partial frame)
    let mut raw_stream = framed.into_inner();
    raw_stream.write_all(b"X").expect("write 1 byte");

    // Engine must detect > 250ms partial frame during scan, cancel worker, and exit boundedly (< 3s)
    let exit_res = child.wait_for_exit(3000);
    assert!(
        exit_res.is_ok(),
        "Engine must exit boundedly when inbound frame is partial during active scan"
    );

    let _ = std::fs::remove_dir_all(&temp_tree);
}

#[test]
fn test_progress_backpressure_never_yields_corruption() {
    let engine_exe = get_engine_exe();
    let mut dir = std::env::temp_dir();
    dir.push(format!("pigtree_backpressure_test_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    // Create a hierarchy with multiple folders and files
    for d in 0..10 {
        let sub = dir.join(format!("dir_{d}"));
        std::fs::create_dir_all(&sub).unwrap();
        for f in 0..10 {
            let file_p = sub.join(format!("file_{f}.bin"));
            std::fs::write(&file_p, vec![0xAB; 200]).unwrap();
        }
    }

    let mut session = EngineClientSession::launch(&engine_exe).expect("launch engine session");

    let mut progress_count = 0;
    let mut last_seq = 0;
    let scan_resp = session
        .scan_with_progress(
            dir.to_str().unwrap(),
            Some(|p: pigtree_protocol::protobuf::ScanProgress| {
                progress_count += 1;
                assert!(p.sequence_number > last_seq);
                last_seq = p.sequence_number;
                assert!(!p.current_directory.is_empty());
                let target_path_str = dir.to_str().unwrap();
                let norm_cur = p
                    .current_directory
                    .strip_prefix("\\\\?\\")
                    .unwrap_or(&p.current_directory);
                let norm_target = target_path_str
                    .strip_prefix("\\\\?\\")
                    .unwrap_or(target_path_str);
                assert!(
                    norm_cur.starts_with(norm_target),
                    "current_directory '{}' must be under target '{}'",
                    p.current_directory,
                    target_path_str
                );
                // Add simulated client processing delay to exercise backpressure
                std::thread::sleep(Duration::from_millis(5));
            }),
            None,
        )
        .expect("scan with backpressure should succeed without framing or checksum corruption");

    assert_eq!(
        scan_resp.run_outcome,
        pigtree_protocol::protobuf::ScanRunOutcome::Finished as i32
    );
    assert_eq!(scan_resp.file_count, 100);
    assert_eq!(scan_resp.directory_count, 11);
    assert_eq!(scan_resp.logical_bytes, 100 * 200);

    let ping = session.ping().expect("ping after backpressure scan");
    assert!(ping.timestamp_utc_ms > 0);

    session.shutdown().expect("shutdown");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_get_children_pagination_and_error_lifecycle() {
    let engine_exe = get_engine_exe();
    let temp_tree = create_temp_scan_tree("gc_lifecycle");

    // Add subdirectories and files for rich ordering checks
    let sub_a = temp_tree.join("DirAlpha");
    std::fs::create_dir(&sub_a).unwrap();
    std::fs::write(sub_a.join("inner.txt"), vec![0x11; 500]).unwrap();

    let sub_b = temp_tree.join("dirbeta");
    std::fs::create_dir(&sub_b).unwrap();
    std::fs::write(sub_b.join("inner2.txt"), vec![0x22; 1000]).unwrap();

    let file_top = temp_tree.join("top_file.bin");
    std::fs::write(&file_top, vec![0x33; 2000]).unwrap();

    let mut session = EngineClientSession::launch(&engine_exe).expect("launch engine session");

    // 0. Test query before any scan has settled returns STALE_OPERATION
    let err_pre_scan = session
        .get_children("op-not-run-yet", 0, 0, 100)
        .expect_err("pre-scan get_children should fail");
    match err_pre_scan {
        IpcError::CommandError { code, message } => {
            assert_eq!(code, "STALE_OPERATION");
            assert!(message.contains("op-not-run-yet"));
        }
        other => panic!(
            "expected CommandError with STALE_OPERATION, got {:?}",
            other
        ),
    }

    let scan_resp = session
        .scan(temp_tree.to_str().unwrap())
        .expect("scan success");

    let op_id = &scan_resp.operation_id;

    // 1. Query root via virtual parent 0 with default limit 0 (should default to 100)
    let gc_root = session
        .get_children(op_id, 0, 0, 0)
        .expect("get root child with limit 0 default");
    assert_eq!(gc_root.total_children, 1);
    assert_eq!(gc_root.nodes.len(), 1);
    assert_eq!(gc_root.nodes[0].id, 1);
    assert_eq!(gc_root.nodes[0].entry_kind, 1);

    // 2. Query root's children (parent_id = 1) with max limit 500
    let gc_children = session
        .get_children(op_id, 1, 0, 500)
        .expect("get root children with max limit 500");
    // Should have: DirAlpha, dirbeta, top_file.bin, plus sub1/sub2/file1/file2 from create_temp_scan_tree
    assert!(gc_children.total_children > 0);
    // Verify directories come first
    let mut seen_file = false;
    let mut file_node_id = None;
    for node in &gc_children.nodes {
        if node.entry_kind == 1 {
            assert!(!seen_file, "Directories must appear before files");
        } else {
            seen_file = true;
            if file_node_id.is_none() {
                file_node_id = Some(node.id);
            }
        }
    }

    // 3. Test limit > 500 returns INVALID_LIMIT
    let err_limit = session
        .get_children(op_id, 1, 0, 501)
        .expect_err("limit > 500 should fail");
    match err_limit {
        IpcError::CommandError { code, message } => {
            assert_eq!(code, "INVALID_LIMIT");
            assert!(message.contains("500"));
        }
        other => panic!("expected CommandError with INVALID_LIMIT, got {:?}", other),
    }

    // 4. Test invalid parent_id returns INVALID_PARENT
    let err_parent = session
        .get_children(op_id, 99999, 0, 50)
        .expect_err("invalid parent should fail");
    match err_parent {
        IpcError::CommandError { code, message } => {
            assert_eq!(code, "INVALID_PARENT");
            assert!(message.contains("99999"));
        }
        other => panic!("expected CommandError with INVALID_PARENT, got {:?}", other),
    }

    // 5. Test parent_id pointing to a file returns INVALID_PARENT
    if let Some(file_id) = file_node_id {
        let err_file_parent = session
            .get_children(op_id, file_id, 0, 50)
            .expect_err("file as parent should fail");
        match err_file_parent {
            IpcError::CommandError { code, message } => {
                assert_eq!(code, "INVALID_PARENT");
                assert!(message.contains("not a directory"));
            }
            other => panic!("expected CommandError with INVALID_PARENT, got {:?}", other),
        }
    }

    // 6. Test stale operation_id returns STALE_OPERATION
    let err_stale = session
        .get_children("stale-op-id-999", 1, 0, 50)
        .expect_err("stale operation should fail");
    match err_stale {
        IpcError::CommandError { code, message } => {
            assert_eq!(code, "STALE_OPERATION");
            assert!(message.contains("stale-op-id-999"));
        }
        other => panic!(
            "expected CommandError with STALE_OPERATION, got {:?}",
            other
        ),
    }

    session.shutdown().expect("shutdown");
    let _ = std::fs::remove_dir_all(&temp_tree);
}

#[test]
fn test_get_children_on_cancelled_settled_scan() {
    let engine_exe = get_engine_exe();
    let temp_tree = create_temp_scan_tree("gc_cancel");

    // Create a multi-level directory structure so traversal takes sufficient time
    for i in 0..25 {
        let sub = temp_tree.join(format!("dir_{i:02}"));
        std::fs::create_dir(&sub).unwrap();
        for j in 0..10 {
            let sub2 = sub.join(format!("nested_{j:02}"));
            std::fs::create_dir(&sub2).unwrap();
            for f in 0..5 {
                std::fs::write(sub2.join(format!("f_{f:02}.dat")), vec![0x55; 50]).unwrap();
            }
        }
    }

    let mut session = EngineClientSession::launch(&engine_exe).expect("launch engine session");

    let h_cancel = unsafe { CreateEventW(std::ptr::null_mut(), TRUE, FALSE, std::ptr::null_mut()) };
    assert!(!h_cancel.is_null() && h_cancel != INVALID_HANDLE_VALUE);
    let h_cancel_val = h_cancel as usize;

    struct CancelWatchdogGuard {
        h_cancel: HANDLE,
        disarm_tx: Option<std::sync::mpsc::Sender<()>>,
        watchdog_handle: Option<std::thread::JoinHandle<()>>,
    }

    impl Drop for CancelWatchdogGuard {
        fn drop(&mut self) {
            if let Some(tx) = self.disarm_tx.take() {
                let _ = tx.send(());
            }
            if let Some(handle) = self.watchdog_handle.take() {
                let _ = handle.join();
            }
            if !self.h_cancel.is_null() && self.h_cancel != INVALID_HANDLE_VALUE {
                unsafe {
                    CloseHandle(self.h_cancel);
                }
                self.h_cancel = std::ptr::null_mut();
            }
        }
    }

    let (watchdog_tx, watchdog_rx) = std::sync::mpsc::channel();
    let h_cancel_thread = h_cancel_val;
    let watchdog_handle = std::thread::spawn(move || {
        if let Err(std::sync::mpsc::RecvTimeoutError::Timeout) =
            watchdog_rx.recv_timeout(Duration::from_secs(5))
        {
            unsafe {
                SetEvent(h_cancel_thread as HANDLE);
            }
        }
    });

    let cancel_guard = CancelWatchdogGuard {
        h_cancel,
        disarm_tx: Some(watchdog_tx),
        watchdog_handle: Some(watchdog_handle),
    };

    let scan_outcome = session.scan_with_progress_outcome(
        temp_tree.to_str().unwrap(),
        Some(move |p: pigtree_protocol::protobuf::ScanProgress| {
            if p.observed_directories >= 1 {
                unsafe {
                    SetEvent(h_cancel_val as HANDLE);
                }
            }
        }),
        Some(h_cancel),
    );

    match scan_outcome {
        Ok(ScanCallOutcome::Cancelled(sr)) => {
            assert_eq!(
                sr.run_outcome,
                pigtree_protocol::protobuf::ScanRunOutcome::Cancelled as i32
            );
            assert_eq!(
                sr.scope_coverage,
                pigtree_protocol::protobuf::ScopeCoverage::Partial as i32
            );
            let op_id = &sr.operation_id;

            // Root query via virtual parent 0
            let gc_root = session
                .get_children(op_id, 0, 0, 50)
                .expect("GetChildren on root of cancelled scan should succeed");
            assert_eq!(gc_root.total_children, 1);
            assert_eq!(gc_root.nodes.len(), 1);
            let root_id = gc_root.nodes[0].id;
            assert_eq!(root_id, 1);

            // Query root's children from the partial scan
            let limit = 100;
            let offset = 0;
            let gc_children = session
                .get_children(op_id, root_id, offset, limit)
                .expect("GetChildren on root children of cancelled scan should succeed");
            assert_eq!(gc_children.parent_id, root_id);
            assert_eq!(gc_children.offset, offset);
            assert!(gc_children.nodes.len() <= limit as usize);
            assert!(gc_children.nodes.len() as u32 <= gc_children.total_children);
            if gc_children.total_children <= limit {
                assert_eq!(gc_children.nodes.len() as u32, gc_children.total_children);
            } else {
                assert_eq!(gc_children.nodes.len(), limit as usize);
            }

            // Pick an observed child directory if any and query its children
            let child_dir = gc_children.nodes.iter().find(|n| n.entry_kind == 1);
            if let Some(dir) = child_dir {
                let gc_sub = session.get_children(op_id, dir.id, 0, 50);
                assert!(
                    gc_sub.is_ok(),
                    "GetChildren on observed sub-directory should succeed"
                );
            }
        }
        Ok(ScanCallOutcome::Finished(_)) => {
            panic!("scan should have been cancelled");
        }
        Err(e) => {
            panic!("scan failed unexpectedly: {:?}", e);
        }
    }

    drop(cancel_guard);
    session.shutdown().expect("shutdown");
    let _ = std::fs::remove_dir_all(&temp_tree);
}

#[test]
fn test_get_children_directory_subtree_aggregates_ipc() {
    let engine_exe = get_engine_exe();
    let temp_tree = std::env::temp_dir().join(unique_session_id("repro_tree"));
    let _ = std::fs::remove_dir_all(&temp_tree);
    std::fs::create_dir_all(&temp_tree).unwrap();

    let nested_folder = temp_tree.join("nested_folder");
    std::fs::create_dir_all(&nested_folder).unwrap();
    std::fs::write(nested_folder.join("inner_file.bin"), vec![0xAB; 5000]).unwrap();

    std::fs::write(temp_tree.join("known_file.dat"), vec![0xCD; 1024]).unwrap();
    std::fs::write(temp_tree.join("empty_file.txt"), b"").unwrap();

    let mut session = EngineClientSession::launch(&engine_exe).expect("launch engine session");

    let scan_resp = session
        .scan(temp_tree.to_str().unwrap())
        .expect("scan success");
    let op_id = &scan_resp.operation_id;

    // Root query via virtual parent 0
    let gc_root = session
        .get_children(op_id, 0, 0, 10)
        .expect("get root child");
    assert_eq!(gc_root.total_children, 1);
    assert_eq!(gc_root.nodes[0].logical_bytes, 6024);

    // Root's immediate children query
    let gc_children = session
        .get_children(op_id, 1, 0, 10)
        .expect("get root children");
    assert_eq!(gc_children.total_children, 3);

    let nested_node = gc_children
        .nodes
        .iter()
        .find(|n| n.name == "nested_folder")
        .expect("nested_folder node must exist");
    assert_eq!(nested_node.entry_kind, 1);
    assert_eq!(nested_node.logical_bytes, 5000);

    let known_node = gc_children
        .nodes
        .iter()
        .find(|n| n.name == "known_file.dat")
        .expect("known_file.dat node must exist");
    assert_eq!(known_node.entry_kind, 2);
    assert_eq!(known_node.logical_bytes, 1024);

    let empty_node = gc_children
        .nodes
        .iter()
        .find(|n| n.name == "empty_file.txt")
        .expect("empty_file.txt node must exist");
    assert_eq!(empty_node.entry_kind, 2);
    assert_eq!(empty_node.logical_bytes, 0);

    session.shutdown().expect("shutdown");
    let _ = std::fs::remove_dir_all(&temp_tree);
}
