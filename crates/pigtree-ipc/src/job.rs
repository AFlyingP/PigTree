//! Windows Job Object management with JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE.

use crate::error::IpcError;
use crate::win32::*;
use std::ffi::c_void;
use std::ptr::null_mut;

#[derive(Debug)]
pub struct JobObject {
    handle: HANDLE,
}

impl JobObject {
    /// Creates a new anonymous Job Object configured with JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE.
    pub fn create_kill_on_close() -> Result<Self, IpcError> {
        unsafe {
            let h_job = CreateJobObjectW(null_mut(), std::ptr::null());
            if h_job.is_null() {
                return Err(IpcError::Win32 {
                    code: GetLastError(),
                    message: "CreateJobObjectW failed".to_string(),
                });
            }

            let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
            info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;

            let res = SetInformationJobObject(
                h_job,
                JOB_OBJECT_EXTENDED_LIMIT_INFORMATION,
                &info as *const _ as *const c_void,
                std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as DWORD,
            );

            if res == 0 {
                let err = GetLastError();
                CloseHandle(h_job);
                return Err(IpcError::Win32 {
                    code: err,
                    message: "SetInformationJobObject failed to set KILL_ON_JOB_CLOSE".to_string(),
                });
            }

            Ok(Self { handle: h_job })
        }
    }

    /// Assigns a running process handle to this Job Object.
    ///
    /// # Safety
    /// The caller must ensure `h_process` is a valid Win32 process handle.
    pub unsafe fn assign_process(&self, h_process: HANDLE) -> Result<(), IpcError> {
        if AssignProcessToJobObject(self.handle, h_process) == 0 {
            return Err(IpcError::Win32 {
                code: GetLastError(),
                message: "AssignProcessToJobObject failed".to_string(),
            });
        }
        Ok(())
    }

    /// Returns raw job HANDLE.
    pub fn raw_handle(&self) -> HANDLE {
        self.handle
    }
}

impl Drop for JobObject {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            unsafe {
                CloseHandle(self.handle);
            }
        }
    }
}

unsafe impl Send for JobObject {}
unsafe impl Sync for JobObject {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_job_object_creation_and_handle_validity() {
        let job = JobObject::create_kill_on_close().expect("should create job object");
        assert!(!job.raw_handle().is_null());
        assert_ne!(job.raw_handle(), INVALID_HANDLE_VALUE);
    }
}
