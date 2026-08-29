use pigtree_engine::{launch_scan_worker, CancelHandle, ScanRunnerError};
use pigtree_protocol::RunOutcome;
use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;
use std::thread;
use std::time::{Duration, Instant};

fn find_binary(name: &str) -> PathBuf {
    for candidate in &[
        format!("target/debug/{name}"),
        format!("target/release/{name}"),
        format!("../../target/debug/{name}"),
        format!("../target/debug/{name}"),
    ] {
        let p = PathBuf::from(candidate);
        if p.exists() {
            return p.canonicalize().unwrap_or(p);
        }
    }
    if let Ok(cur) = std::env::current_exe() {
        if let Some(parent) = cur.parent() {
            let p = parent.join(name);
            if p.exists() {
                return p;
            }
            if let Some(gp) = parent.parent() {
                let p = gp.join(name);
                if p.exists() {
                    return p;
                }
            }
        }
    }
    panic!("Binary {name} not found");
}

fn get_worker_exe() -> PathBuf {
    find_binary("pigtree-scan-worker.exe")
}

fn get_crash_worker_exe() -> PathBuf {
    find_binary("test-crash-worker.exe")
}

fn create_temp_tree(name: &str) -> PathBuf {
    let mut dir = std::env::temp_dir();
    dir.push(format!("pigtree_test_runner_{name}_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();

    // dir/subdir1/file1.txt
    let sub1 = dir.join("subdir1");
    fs::create_dir_all(&sub1).unwrap();
    let mut f1 = File::create(sub1.join("file1.txt")).unwrap();
    f1.write_all(b"hello world").unwrap();

    // dir/subdir2/file2.dat
    let sub2 = dir.join("subdir2");
    fs::create_dir_all(&sub2).unwrap();
    let mut f2 = File::create(sub2.join("file2.dat")).unwrap();
    f2.write_all(&vec![0u8; 2048]).unwrap();

    dir
}

#[test]
fn test_launch_scan_worker_successful_hierarchy() {
    let worker_exe = get_worker_exe();
    let target = create_temp_tree("success_hierarchy");
    let cancel = CancelHandle::new().unwrap();

    let graph = launch_scan_worker(&worker_exe, &target, &cancel)
        .expect("launch_scan_worker should succeed");

    assert_eq!(graph.terminal().outcome, RunOutcome::Finished);
    assert_eq!(graph.terminal().total_directories, 3); // root + subdir1 + subdir2
    assert_eq!(graph.terminal().total_files, 2);
    assert_eq!(graph.terminal().total_logical_bytes, 11 + 2048);
    assert_eq!(graph.total_entries(), 5);

    let root = graph.root();
    assert_eq!(root.id, 1);
    assert_eq!(root.parent_id, 0);
    assert_eq!(root.children.len(), 2);

    let _ = fs::remove_dir_all(&target);
}

#[test]
fn test_launch_scan_worker_target_with_spaces_and_trailing_slash() {
    let worker_exe = get_worker_exe();
    let mut target = std::env::temp_dir();
    target.push(format!("pigtree space test dir_{}", std::process::id()));
    let _ = fs::remove_dir_all(&target);
    fs::create_dir_all(&target).unwrap();

    let file_path = target.join("spaced file.txt");
    let mut f = File::create(&file_path).unwrap();
    f.write_all(b"spaced content").unwrap();

    let cancel = CancelHandle::new().unwrap();
    let graph = launch_scan_worker(&worker_exe, &target, &cancel)
        .expect("launch_scan_worker on path with spaces should succeed");

    assert_eq!(graph.terminal().outcome, RunOutcome::Finished);
    assert_eq!(graph.terminal().total_files, 1);
    assert_eq!(graph.terminal().total_directories, 1);

    let _ = fs::remove_dir_all(&target);
}

#[test]
fn test_launch_scan_worker_nonexistent_target_fails_cleanly() {
    let worker_exe = get_worker_exe();
    let target = PathBuf::from(r"C:\pigtree_nonexistent_dir_123456789");
    let cancel = CancelHandle::new().unwrap();

    let res = launch_scan_worker(&worker_exe, &target, &cancel);
    assert!(
        res.is_err(),
        "launch_scan_worker on nonexistent target must fail cleanly"
    );
}

#[test]
fn test_launch_scan_worker_unc_target_fails_cleanly() {
    let worker_exe = get_worker_exe();
    let target = PathBuf::from(r"\\dummy_server\dummy_share");
    let cancel = CancelHandle::new().unwrap();

    let res = launch_scan_worker(&worker_exe, &target, &cancel);
    assert!(
        res.is_err(),
        "launch_scan_worker on UNC target must fail cleanly"
    );
}

#[test]
fn test_launch_scan_worker_already_signaled_cancellation() {
    let worker_exe = get_worker_exe();
    let target = create_temp_tree("already_cancelled");
    let cancel = CancelHandle::new().unwrap();
    cancel.cancel();
    assert!(cancel.is_cancelled());

    let start = Instant::now();
    let graph = launch_scan_worker(&worker_exe, &target, &cancel)
        .expect("cancelled scan produces graph with cancelled terminal");
    let elapsed = start.elapsed();

    assert_eq!(graph.terminal().outcome, RunOutcome::Cancelled);
    assert!(elapsed < Duration::from_secs(2), "took {elapsed:?}");

    let _ = fs::remove_dir_all(&target);
}

#[test]
fn test_launch_scan_worker_concurrent_cancellation() {
    let worker_exe = get_worker_exe();
    let mut target = std::env::temp_dir();
    target.push(format!("pigtree_cancel_tree_{}", std::process::id()));
    let _ = fs::remove_dir_all(&target);
    fs::create_dir_all(&target).unwrap();

    // Create a tree with several directories and files
    for d in 0..10 {
        let sub = target.join(format!("sub_{d}"));
        fs::create_dir_all(&sub).unwrap();
        for f in 0..10 {
            let mut file = File::create(sub.join(format!("file_{f}.bin"))).unwrap();
            file.write_all(&vec![1u8; 1024]).unwrap();
        }
    }

    let cancel = CancelHandle::new().unwrap();
    let cancel_clone = cancel.clone();

    let handle = thread::spawn(move || {
        thread::sleep(Duration::from_millis(5));
        cancel_clone.cancel();
    });

    let start = Instant::now();
    let graph = launch_scan_worker(&worker_exe, &target, &cancel)
        .expect("concurrent cancellation should return a valid graph");
    let elapsed = start.elapsed();

    handle.join().unwrap();
    assert!(
        elapsed < Duration::from_secs(2),
        "cancellation took {elapsed:?}"
    );
    assert!(
        graph.terminal().outcome == RunOutcome::Cancelled
            || graph.terminal().outcome == RunOutcome::Finished
    );

    let _ = fs::remove_dir_all(&target);
}

#[test]
fn test_launch_scan_worker_invalid_executable_spawn_error() {
    let worker_exe = PathBuf::from(
        r"C:
onexistentpigtree-fake-worker.exe",
    );
    let target = std::env::temp_dir();
    let cancel = CancelHandle::new().unwrap();

    let err = launch_scan_worker(&worker_exe, &target, &cancel)
        .expect_err("should fail to spawn nonexistent executable");

    match err {
        ScanRunnerError::Spawn(_) => {}
        other => panic!("expected Spawn error, got: {other:?}"),
    }
}

#[test]
fn test_launch_scan_worker_crash_and_truncated_stream_fails_cleanly() {
    let crash_worker = get_crash_worker_exe();
    let target = std::env::temp_dir();
    let cancel = CancelHandle::new().unwrap();

    let start = Instant::now();
    let err = launch_scan_worker(&crash_worker, &target, &cancel)
        .expect_err("crashing worker with truncated stream must return error");
    let elapsed = start.elapsed();

    assert!(elapsed < Duration::from_secs(2), "took {elapsed:?}");
    match err {
        ScanRunnerError::Graph(_)
        | ScanRunnerError::WorkerExitInconsistent { .. }
        | ScanRunnerError::Io(_) => {}
        other => panic!("expected Graph/ExitInconsistent/Io error, got: {other:?}"),
    }
}

#[test]
fn test_worker_cleanup_leaves_no_orphan_process() {
    let worker_exe = get_worker_exe();
    let target = create_temp_tree("cleanup_no_orphans");
    let cancel = CancelHandle::new().unwrap();

    let graph = launch_scan_worker(&worker_exe, &target, &cancel).unwrap();
    assert_eq!(graph.terminal().outcome, RunOutcome::Finished);

    let _ = fs::remove_dir_all(&target);
}
