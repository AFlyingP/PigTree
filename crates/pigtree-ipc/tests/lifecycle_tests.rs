use pigtree_ipc::bootstrap::{read_bootstrap_nonce, spawn_engine, BootstrapPipe};
use pigtree_ipc::client::EngineClientSession;
use pigtree_ipc::error::IpcError;
use pigtree_ipc::job::JobObject;
use pigtree_ipc::pipe::{format_pipe_name, NamedPipeClient, NamedPipeServer};
use pigtree_ipc::security::generate_nonce;
use pigtree_ipc::server::EngineServerSession;
use pigtree_ipc::transport::FramedSession;
use pigtree_ipc::win32::*;
use pigtree_protocol::frame::{ChannelTag, FrameFlags};
use pigtree_protocol::protobuf::{AuthHandshakeRequest, AuthHandshakeResponse};
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
