//! Anonymous bootstrap pipe creation, handle inheritance confinement, and engine process spawning.

use crate::error::IpcError;
use crate::job::JobObject;
use crate::security::get_process_creation_time;
use crate::win32::*;
use std::ffi::c_void;
use std::path::Path;
use std::ptr::null_mut;

#[derive(Debug)]
pub struct ChildProcessGuard {
    pub h_process: HANDLE,
    pub pid: u32,
    pub creation_time: u64,
}

impl ChildProcessGuard {
    pub fn wait_for_exit(&self, timeout_ms: u32) -> Result<u32, IpcError> {
        unsafe {
            let res = WaitForSingleObject(self.h_process, timeout_ms);
            if res == 0 {
                let mut exit_code: DWORD = 0;
                if GetExitCodeProcess(self.h_process, &mut exit_code) != 0 {
                    return Ok(exit_code);
                }
            }
            Err(IpcError::Timeout)
        }
    }

    pub fn terminate(&self, exit_code: u32) -> Result<(), IpcError> {
        unsafe {
            if TerminateProcess(self.h_process, exit_code) == 0 {
                return Err(IpcError::Win32 {
                    code: GetLastError(),
                    message: "TerminateProcess failed".to_string(),
                });
            }
            Ok(())
        }
    }
}

impl Drop for ChildProcessGuard {
    fn drop(&mut self) {
        if !self.h_process.is_null() && self.h_process != INVALID_HANDLE_VALUE {
            unsafe {
                CloseHandle(self.h_process);
            }
        }
    }
}

unsafe impl Send for ChildProcessGuard {}
unsafe impl Sync for ChildProcessGuard {}

/// Anonymous bootstrap pipe for passing secrets out-of-band to child engine processes.
pub struct BootstrapPipe {
    h_read: HANDLE,
    h_write: HANDLE,
}

impl BootstrapPipe {
    pub fn create() -> Result<Self, IpcError> {
        let mut h_read: HANDLE = null_mut();
        let mut h_write: HANDLE = null_mut();

        let mut sa = SECURITY_ATTRIBUTES {
            nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as DWORD,
            lpSecurityDescriptor: null_mut(),
            bInheritHandle: TRUE,
        };

        if unsafe { CreatePipe(&mut h_read, &mut h_write, &mut sa, 4096) } == 0 {
            return Err(IpcError::Win32 {
                code: unsafe { GetLastError() },
                message: "CreatePipe failed".to_string(),
            });
        }

        // Only the read handle should be inheritable
        if unsafe { SetHandleInformation(h_write, HANDLE_FLAG_INHERIT, 0) } == 0 {
            let err = unsafe { GetLastError() };
            unsafe {
                CloseHandle(h_read);
                CloseHandle(h_write);
            }
            return Err(IpcError::Win32 {
                code: err,
                message: "SetHandleInformation on write pipe failed".to_string(),
            });
        }

        Ok(Self { h_read, h_write })
    }

    pub fn write_nonce(&mut self, nonce: &[u8; 32]) -> Result<(), IpcError> {
        let mut written: DWORD = 0;
        let success = unsafe {
            WriteFile(
                self.h_write,
                nonce.as_ptr() as *const c_void,
                32,
                &mut written,
                null_mut(),
            )
        };
        if success == 0 || written != 32 {
            return Err(IpcError::Win32 {
                code: unsafe { GetLastError() },
                message: "WriteFile failed writing bootstrap nonce".to_string(),
            });
        }
        // Close write handle so child gets EOF after reading
        unsafe {
            CloseHandle(self.h_write);
            self.h_write = null_mut();
        }
        Ok(())
    }

    pub fn read_handle(&self) -> HANDLE {
        self.h_read
    }

    pub fn into_read_handle(mut self) -> HANDLE {
        let h = self.h_read;
        self.h_read = null_mut();
        h
    }
}

impl Drop for BootstrapPipe {
    fn drop(&mut self) {
        unsafe {
            if !self.h_read.is_null() && self.h_read != INVALID_HANDLE_VALUE {
                CloseHandle(self.h_read);
            }
            if !self.h_write.is_null() && self.h_write != INVALID_HANDLE_VALUE {
                CloseHandle(self.h_write);
            }
        }
    }
}

/// Reads the bootstrap nonce from an inherited handle (used by engine process).
///
/// # Safety
/// The caller must ensure `h_read` is a valid, readable Win32 pipe handle.
pub unsafe fn read_bootstrap_nonce(h_read: HANDLE) -> Result<[u8; 32], IpcError> {
    let mut nonce = [0u8; 32];
    let mut bytes_read: DWORD = 0;
    let success = ReadFile(
        h_read,
        nonce.as_mut_ptr() as *mut c_void,
        32,
        &mut bytes_read,
        null_mut(),
    );
    if success == 0 || bytes_read != 32 {
        return Err(IpcError::Win32 {
            code: GetLastError(),
            message: "ReadFile failed reading bootstrap nonce from inherited handle".to_string(),
        });
    }
    CloseHandle(h_read);
    Ok(nonce)
}

/// Spawns the dedicated engine process with handle confinement and Job Object assignment.
pub fn spawn_engine(
    engine_exe: &Path,
    pipe_name: &str,
    session_id: &str,
    bootstrap_pipe: &BootstrapPipe,
    job_object: &JobObject,
) -> Result<ChildProcessGuard, IpcError> {
    let h_read = bootstrap_pipe.read_handle();
    let handle_val = h_read as usize;

    let command_line = format!(
        r#""{}" --pipe-name "{}" --session-id "{}" --bootstrap-handle {}"#,
        engine_exe.display(),
        pipe_name,
        session_id,
        handle_val
    );

    let mut wide_cmd: Vec<u16> = command_line
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();

    unsafe {
        let mut size: SIZE_T = 0;
        InitializeProcThreadAttributeList(null_mut(), 1, 0, &mut size);

        let count = size.div_ceil(std::mem::size_of::<usize>());
        let mut attr_buf: Vec<usize> = vec![0; count];
        let p_attr = attr_buf.as_mut_ptr() as *mut c_void;

        if InitializeProcThreadAttributeList(p_attr, 1, 0, &mut size) == 0 {
            return Err(IpcError::Win32 {
                code: GetLastError(),
                message: "InitializeProcThreadAttributeList failed".to_string(),
            });
        }

        let mut inherit_handles = [h_read];
        if UpdateProcThreadAttribute(
            p_attr,
            0,
            PROC_THREAD_ATTRIBUTE_HANDLE_LIST,
            inherit_handles.as_mut_ptr() as *mut c_void,
            std::mem::size_of::<HANDLE>(),
            null_mut(),
            null_mut(),
        ) == 0
        {
            let err = GetLastError();
            DeleteProcThreadAttributeList(p_attr);
            return Err(IpcError::Win32 {
                code: err,
                message: "UpdateProcThreadAttribute failed".to_string(),
            });
        }

        let mut siex: STARTUPINFOEXW = std::mem::zeroed();
        siex.StartupInfo.cb = std::mem::size_of::<STARTUPINFOEXW>() as DWORD;
        siex.lpAttributeList = p_attr;

        let mut pi: PROCESS_INFORMATION = std::mem::zeroed();

        let success = CreateProcessW(
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
        );

        DeleteProcThreadAttributeList(p_attr);

        if success == 0 {
            return Err(IpcError::Win32 {
                code: GetLastError(),
                message: format!(
                    "CreateProcessW failed for engine binary: {}",
                    engine_exe.display()
                ),
            });
        }

        // Assign to Job Object before resuming to guarantee process cannot run uncontained
        if let Err(e) = job_object.assign_process(pi.hProcess) {
            TerminateProcess(pi.hProcess, 1);
            CloseHandle(pi.hProcess);
            CloseHandle(pi.hThread);
            return Err(e);
        }

        // Resume suspended engine main thread
        if ResumeThread(pi.hThread) == u32::MAX {
            let err = GetLastError();
            TerminateProcess(pi.hProcess, 1);
            CloseHandle(pi.hProcess);
            CloseHandle(pi.hThread);
            return Err(IpcError::Win32 {
                code: err,
                message: "ResumeThread failed".to_string(),
            });
        }

        // Close thread handle immediately after successful resume
        CloseHandle(pi.hThread);

        let creation_time = match get_process_creation_time(pi.hProcess) {
            Ok(t) => t,
            Err(e) => {
                TerminateProcess(pi.hProcess, 1);
                CloseHandle(pi.hProcess);
                return Err(e);
            }
        };

        Ok(ChildProcessGuard {
            h_process: pi.hProcess,
            pid: pi.dwProcessId,
            creation_time,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bootstrap_pipe_write_read_no_deadlock() {
        let mut pipe = BootstrapPipe::create().expect("Create pipe failed");
        let nonce = [42u8; 32];
        pipe.write_nonce(&nonce).expect("Write nonce failed");
        let h_read = pipe.into_read_handle();
        let read_nonce = unsafe { read_bootstrap_nonce(h_read) }.expect("Read nonce failed");
        assert_eq!(nonce, read_nonce);
    }
}
