//! Engine scan worker process launch and observation stream orchestration.

use crate::builder::GraphBuilder;
use crate::error::GraphBuildError;
use crate::graph::DirectoryGraph;
use pigtree_ipc::win32::*;
use pigtree_ipc::{
    build_windows_command_line, AnonymousPipe, CancelHandle, ChildProcessGuard, IpcError, JobObject,
};
use pigtree_protocol::protobuf::ScanProgress;
use pigtree_protocol::{ObservationReader, RunOutcome};
use std::ffi::c_void;
use std::fmt;
use std::path::{Path, PathBuf};
use std::ptr::null_mut;

/// Resolves the scan worker executable (`pigtree-scan-worker.exe`) from the engine executable's sibling
/// directory, with a narrow internal environment override seam for testing.
pub fn resolve_scan_worker_exe() -> Option<PathBuf> {
    if let Ok(val) = std::env::var("PIGTREE_SCAN_WORKER_EXE") {
        let p = PathBuf::from(val);
        if p.exists() {
            return Some(p);
        }
    }
    if let Ok(val) = std::env::var("PIGTREE_WORKER_PATH") {
        let p = PathBuf::from(val);
        if p.exists() {
            return Some(p);
        }
    }

    if let Ok(cur_exe) = std::env::current_exe() {
        if let Some(parent) = cur_exe.parent() {
            let sibling = parent.join("pigtree-scan-worker.exe");
            if sibling.exists() {
                return Some(sibling);
            }
            if let Some(grandparent) = parent.parent() {
                let sibling = grandparent.join("pigtree-scan-worker.exe");
                if sibling.exists() {
                    return Some(sibling);
                }
            }
        }
    }

    for candidate in &[
        "target/debug/pigtree-scan-worker.exe",
        "target/release/pigtree-scan-worker.exe",
        "../../target/debug/pigtree-scan-worker.exe",
        "../target/debug/pigtree-scan-worker.exe",
    ] {
        let p = PathBuf::from(candidate);
        if p.exists() {
            return p.canonicalize().ok().or(Some(p));
        }
    }

    None
}

#[derive(Debug)]
pub enum ScanRunnerError {
    Spawn(String),
    Ipc(IpcError),
    Io(std::io::Error),
    Graph(GraphBuildError),
    WorkerExitInconsistent {
        exit_code: u32,
        terminal_outcome: Option<RunOutcome>,
    },
    WorkerTimeout,
}

impl fmt::Display for ScanRunnerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ScanRunnerError::Spawn(msg) => write!(f, "failed to spawn scan worker: {msg}"),
            ScanRunnerError::Ipc(e) => write!(f, "IPC error during worker execution: {e}"),
            ScanRunnerError::Io(e) => write!(f, "I/O error during worker execution: {e}"),
            ScanRunnerError::Graph(e) => write!(f, "graph build error: {e}"),
            ScanRunnerError::WorkerExitInconsistent {
                exit_code,
                terminal_outcome,
            } => {
                write!(
                    f,
                    "worker exit code {exit_code} is inconsistent with terminal outcome {terminal_outcome:?}"
                )
            }
            ScanRunnerError::WorkerTimeout => {
                write!(f, "worker process timed out waiting for exit")
            }
        }
    }
}

impl std::error::Error for ScanRunnerError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ScanRunnerError::Ipc(e) => Some(e),
            ScanRunnerError::Io(e) => Some(e),
            ScanRunnerError::Graph(e) => Some(e),
            _ => None,
        }
    }
}

impl From<IpcError> for ScanRunnerError {
    fn from(e: IpcError) -> Self {
        ScanRunnerError::Ipc(e)
    }
}

impl From<std::io::Error> for ScanRunnerError {
    fn from(e: std::io::Error) -> Self {
        ScanRunnerError::Io(e)
    }
}

impl From<GraphBuildError> for ScanRunnerError {
    fn from(e: GraphBuildError) -> Self {
        ScanRunnerError::Graph(e)
    }
}

/// Launches a dedicated scan worker subprocess, confines it via Job Object and handle inheritance,
/// and drives its binary observation stream into a DirectoryGraph.
pub fn launch_scan_worker(
    worker_exe: &Path,
    target: &Path,
    cancel_handle: &CancelHandle,
) -> Result<DirectoryGraph, ScanRunnerError> {
    launch_scan_worker_with_progress(
        worker_exe,
        target,
        cancel_handle,
        "",
        None::<fn(ScanProgress)>,
    )
}

/// Launches a dedicated scan worker subprocess, confines it via Job Object and handle inheritance,
/// and drives its binary observation stream into a DirectoryGraph with progress callbacks.
pub fn launch_scan_worker_with_progress<F>(
    worker_exe: &Path,
    target: &Path,
    cancel_handle: &CancelHandle,
    operation_id: &str,
    on_progress: Option<F>,
) -> Result<DirectoryGraph, ScanRunnerError>
where
    F: FnMut(ScanProgress),
{
    // 1. Create anonymous observation pipe with inheritable write handle
    let (pipe_reader, h_write) = AnonymousPipe::create_inheritable_write()?;

    // 2. Prepare handle whitelist for inheritance (write pipe + cancellation event)
    let mut inherit_handles = [h_write, cancel_handle.raw_handle()];

    // 3. Build command line with proper Windows argument escaping
    let cmd = build_windows_command_line([
        worker_exe.to_string_lossy().as_ref(),
        "--target",
        target.to_string_lossy().as_ref(),
        "--pipe-handle",
        &format!("{}", h_write as usize),
        "--cancel-event-handle",
        &format!("{}", cancel_handle.raw_handle() as usize),
    ]);

    let mut wide_cmd: Vec<u16> = cmd.encode_utf16().chain(std::iter::once(0)).collect();

    // 4. Initialize PROC_THREAD_ATTRIBUTE_HANDLE_LIST in STARTUPINFOEXW
    let mut size: SIZE_T = 0;
    unsafe {
        InitializeProcThreadAttributeList(null_mut(), 1, 0, &mut size);
    }
    let count = size.div_ceil(std::mem::size_of::<usize>());
    let mut attr_buf: Vec<usize> = vec![0; count];
    let p_attr = attr_buf.as_mut_ptr() as *mut c_void;

    if unsafe { InitializeProcThreadAttributeList(p_attr, 1, 0, &mut size) } == 0 {
        let err = unsafe { GetLastError() };
        unsafe {
            CloseHandle(h_write);
        }
        return Err(ScanRunnerError::Ipc(IpcError::Win32 {
            code: err,
            message: "InitializeProcThreadAttributeList failed".to_string(),
        }));
    }

    if unsafe {
        UpdateProcThreadAttribute(
            p_attr,
            0,
            PROC_THREAD_ATTRIBUTE_HANDLE_LIST,
            inherit_handles.as_mut_ptr() as *mut c_void,
            inherit_handles.len() * std::mem::size_of::<HANDLE>(),
            null_mut(),
            null_mut(),
        )
    } == 0
    {
        let err = unsafe { GetLastError() };
        unsafe {
            DeleteProcThreadAttributeList(p_attr);
            CloseHandle(h_write);
        }
        return Err(ScanRunnerError::Ipc(IpcError::Win32 {
            code: err,
            message: "UpdateProcThreadAttribute failed".to_string(),
        }));
    }

    let mut siex: STARTUPINFOEXW = unsafe { std::mem::zeroed() };
    siex.StartupInfo.cb = std::mem::size_of::<STARTUPINFOEXW>() as DWORD;
    siex.lpAttributeList = p_attr;

    let mut pi: PROCESS_INFORMATION = unsafe { std::mem::zeroed() };

    // 5. Spawn child process suspended
    let success = unsafe {
        CreateProcessW(
            null_mut(),
            wide_cmd.as_mut_ptr(),
            null_mut(),
            null_mut(),
            TRUE, // inherit only whitelisted handles
            EXTENDED_STARTUPINFO_PRESENT | CREATE_NO_WINDOW | CREATE_SUSPENDED,
            null_mut(),
            null_mut(),
            &mut siex.StartupInfo,
            &mut pi,
        )
    };

    // Immediately clean up attribute list and close parent's write pipe copy
    unsafe {
        DeleteProcThreadAttributeList(p_attr);
        CloseHandle(h_write);
    }

    if success == 0 {
        let err = unsafe { GetLastError() };
        return Err(ScanRunnerError::Spawn(format!(
            "CreateProcessW failed for worker '{}': {}",
            worker_exe.display(),
            format_win32_error(err)
        )));
    }

    // 6. Confine child process in kill-on-close Job Object before resuming
    let job_object = match JobObject::create_kill_on_close() {
        Ok(job) => job,
        Err(e) => {
            unsafe {
                TerminateProcess(pi.hProcess, 1);
                CloseHandle(pi.hProcess);
                CloseHandle(pi.hThread);
            }
            return Err(ScanRunnerError::Ipc(e));
        }
    };
    if let Err(e) = unsafe { job_object.assign_process(pi.hProcess) } {
        unsafe {
            TerminateProcess(pi.hProcess, 1);
            CloseHandle(pi.hProcess);
            CloseHandle(pi.hThread);
        }
        return Err(ScanRunnerError::Ipc(e));
    }

    // 7. Resume child thread
    if unsafe { ResumeThread(pi.hThread) } == u32::MAX {
        let err = unsafe { GetLastError() };
        unsafe {
            TerminateProcess(pi.hProcess, 1);
            CloseHandle(pi.hProcess);
            CloseHandle(pi.hThread);
        }
        return Err(ScanRunnerError::Ipc(IpcError::Win32 {
            code: err,
            message: "ResumeThread failed".to_string(),
        }));
    }

    // Promptly close child thread handle
    unsafe {
        CloseHandle(pi.hThread);
    }

    let child_guard = ChildProcessGuard {
        h_process: pi.hProcess,
        pid: pi.dwProcessId,
        creation_time: 0,
    };

    // 8. Drain observation stream from pipe
    let reader = match ObservationReader::new(pipe_reader) {
        Ok(r) => r,
        Err(e) => {
            let _ = child_guard.wait_for_exit(5000);
            return Err(ScanRunnerError::Graph(GraphBuildError::Decode(e)));
        }
    };

    let graph_res =
        GraphBuilder::build_from_reader_with_progress(reader, operation_id, on_progress);

    // 9. Wait for child process to exit (bounded 5s wait)
    let exit_code = match child_guard.wait_for_exit(5000) {
        Ok(code) => code,
        Err(_) => {
            let _ = child_guard.terminate(1);
            return Err(ScanRunnerError::WorkerTimeout);
        }
    };

    // 10. Check consistency between exit code and graph outcome
    match graph_res {
        Ok(graph) => {
            let expected_exit_code = match graph.terminal().outcome {
                RunOutcome::Finished => 0,
                RunOutcome::Cancelled => 3,
                RunOutcome::Failed => 1,
            };
            if exit_code != expected_exit_code {
                return Err(ScanRunnerError::WorkerExitInconsistent {
                    exit_code,
                    terminal_outcome: Some(graph.terminal().outcome),
                });
            }
            Ok(graph)
        }
        Err(e) => Err(ScanRunnerError::Graph(e)),
    }
}
