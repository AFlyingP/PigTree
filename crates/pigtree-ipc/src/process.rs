//! Reusable Windows process execution, argument quoting, cancellation event, and pipe primitives.

use crate::error::IpcError;
use crate::win32::*;
use std::ffi::c_void;
use std::io::{self, Read};
use std::ptr::null_mut;
use std::sync::Arc;

/// Correctly quotes a single command-line argument for Windows processes per standard rules.
pub fn quote_windows_arg(arg: &str) -> String {
    if arg.is_empty() {
        return "\"\"".to_string();
    }
    if !arg.contains([' ', '\t', '\n', '\x0B', '"']) {
        return arg.to_string();
    }

    let mut res = String::with_capacity(arg.len() + 2);
    res.push('"');
    let mut backslashes = 0;

    for c in arg.chars() {
        if c == '\\' {
            backslashes += 1;
        } else if c == '"' {
            for _ in 0..(backslashes * 2 + 1) {
                res.push('\\');
            }
            res.push('"');
            backslashes = 0;
        } else {
            for _ in 0..backslashes {
                res.push('\\');
            }
            backslashes = 0;
            res.push(c);
        }
    }

    // Escape any trailing backslashes before the closing quote
    for _ in 0..(backslashes * 2) {
        res.push('\\');
    }
    res.push('"');
    res
}

/// Builds a full Windows command line string from an iterator of arguments.
pub fn build_windows_command_line<I, S>(args: I) -> String
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut cmd = String::new();
    for (i, arg) in args.into_iter().enumerate() {
        if i > 0 {
            cmd.push(' ');
        }
        cmd.push_str(&quote_windows_arg(arg.as_ref()));
    }
    cmd
}

/// Thread-safe RAII cancellation handle backed by a manual-reset Win32 event.
#[derive(Clone, Debug)]
pub struct CancelHandle {
    inner: Arc<CancelInner>,
}

#[derive(Debug)]
struct CancelInner {
    handle: HANDLE,
    owns_handle: bool,
}

impl CancelHandle {
    /// Creates a new manual-reset cancellation event with inheritable handle permissions.
    pub fn new() -> Result<Self, IpcError> {
        let mut sa = SECURITY_ATTRIBUTES {
            nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as DWORD,
            lpSecurityDescriptor: null_mut(),
            bInheritHandle: TRUE,
        };

        let h_event = unsafe { CreateEventW(&mut sa, TRUE, FALSE, null_mut()) };
        if h_event.is_null() || h_event == INVALID_HANDLE_VALUE {
            return Err(IpcError::Win32 {
                code: unsafe { GetLastError() },
                message: "CreateEventW failed".to_string(),
            });
        }

        Ok(Self {
            inner: Arc::new(CancelInner {
                handle: h_event,
                owns_handle: true,
            }),
        })
    }

    /// Wraps a raw Win32 event handle without closing it on drop.
    ///
    /// # Safety
    /// Caller must ensure `handle` is a valid Win32 event handle for the lifetime of this object.
    pub unsafe fn from_raw(handle: HANDLE) -> Self {
        Self {
            inner: Arc::new(CancelInner {
                handle,
                owns_handle: false,
            }),
        }
    }

    /// Signals cancellation by setting the underlying manual-reset Win32 event.
    ///
    /// This method is thread-safe, idempotent, and non-blocking.
    pub fn cancel(&self) {
        if !self.inner.handle.is_null() && self.inner.handle != INVALID_HANDLE_VALUE {
            unsafe {
                SetEvent(self.inner.handle);
            }
        }
    }

    /// Returns `true` if the cancellation event has been signaled.
    pub fn is_cancelled(&self) -> bool {
        if self.inner.handle.is_null() || self.inner.handle == INVALID_HANDLE_VALUE {
            return false;
        }
        unsafe { WaitForSingleObject(self.inner.handle, 0) == WAIT_OBJECT_0 }
    }

    /// Returns the raw Win32 event HANDLE for inheritance or low-level API calls.
    pub fn raw_handle(&self) -> HANDLE {
        self.inner.handle
    }
}

impl Drop for CancelInner {
    fn drop(&mut self) {
        if self.owns_handle && !self.handle.is_null() && self.handle != INVALID_HANDLE_VALUE {
            unsafe {
                CloseHandle(self.handle);
            }
        }
    }
}

unsafe impl Send for CancelInner {}
unsafe impl Sync for CancelInner {}

/// Synchronous RAII reader for reading from a Win32 pipe HANDLE implementing `std::io::Read`.
pub struct PipeReader {
    handle: HANDLE,
    owns_handle: bool,
}

impl PipeReader {
    /// Borrow a raw pipe HANDLE without closing it on drop.
    pub fn from_raw(handle: HANDLE) -> Self {
        Self {
            handle,
            owns_handle: false,
        }
    }

    /// Take ownership of a raw pipe HANDLE to close via CloseHandle on drop.
    pub fn from_owned(handle: HANDLE) -> Self {
        Self {
            handle,
            owns_handle: true,
        }
    }

    pub fn raw_handle(&self) -> HANDLE {
        self.handle
    }
}

impl Read for PipeReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.handle.is_null() || self.handle == INVALID_HANDLE_VALUE {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Invalid pipe handle",
            ));
        }
        if buf.is_empty() {
            return Ok(0);
        }

        let mut bytes_read: DWORD = 0;
        let success = unsafe {
            ReadFile(
                self.handle,
                buf.as_mut_ptr() as *mut c_void,
                buf.len() as DWORD,
                &mut bytes_read,
                null_mut(),
            )
        };

        if success == 0 {
            let err = unsafe { GetLastError() };
            if err == ERROR_BROKEN_PIPE || err == 38
            /* ERROR_HANDLE_EOF */
            {
                Ok(0) // Clean EOF
            } else {
                Err(io::Error::from_raw_os_error(err as i32))
            }
        } else {
            Ok(bytes_read as usize)
        }
    }
}

impl Drop for PipeReader {
    fn drop(&mut self) {
        if self.owns_handle && !self.handle.is_null() && self.handle != INVALID_HANDLE_VALUE {
            unsafe {
                CloseHandle(self.handle);
            }
        }
    }
}

unsafe impl Send for PipeReader {}
unsafe impl Sync for PipeReader {}

/// Anonymous pipe creation helper for child process communication.
pub struct AnonymousPipe;

impl AnonymousPipe {
    /// Creates an anonymous pipe with an inheritable write handle and a non-inheritable read handle.
    ///
    /// Returns `(PipeReader, raw_write_handle)`.
    pub fn create_inheritable_write() -> Result<(PipeReader, HANDLE), IpcError> {
        let mut h_read: HANDLE = null_mut();
        let mut h_write: HANDLE = null_mut();

        let mut sa = SECURITY_ATTRIBUTES {
            nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as DWORD,
            lpSecurityDescriptor: null_mut(),
            bInheritHandle: TRUE,
        };

        if unsafe { CreatePipe(&mut h_read, &mut h_write, &mut sa, 64 * 1024) } == 0 {
            return Err(IpcError::Win32 {
                code: unsafe { GetLastError() },
                message: "CreatePipe failed".to_string(),
            });
        }

        // Read end should NOT be inherited by child process
        if unsafe { SetHandleInformation(h_read, HANDLE_FLAG_INHERIT, 0) } == 0 {
            let err = unsafe { GetLastError() };
            unsafe {
                CloseHandle(h_read);
                CloseHandle(h_write);
            }
            return Err(IpcError::Win32 {
                code: err,
                message: "SetHandleInformation on read pipe failed".to_string(),
            });
        }

        Ok((PipeReader::from_owned(h_read), h_write))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_windows_arg_quoting() {
        assert_eq!(quote_windows_arg("simple"), "simple");
        assert_eq!(quote_windows_arg(""), "\"\"");
        assert_eq!(quote_windows_arg("with space"), "\"with space\"");
        assert_eq!(
            quote_windows_arg(r"C:\Program Files\"),
            r#""C:\Program Files\\""#
        );
        assert_eq!(
            quote_windows_arg(r#"word "with" quotes"#),
            "\"word \\\"with\\\" quotes\""
        );
    }

    #[test]
    fn test_build_windows_command_line() {
        let cmd = build_windows_command_line(["app.exe", "--target", r"C:\my path\", "--flag"]);
        assert_eq!(cmd, r#"app.exe --target "C:\my path\\" --flag"#);
    }

    #[test]
    fn test_cancel_handle_lifecycle() {
        let handle = CancelHandle::new().expect("CancelHandle::new failed");
        assert!(!handle.is_cancelled());

        let handle_clone = handle.clone();
        handle_clone.cancel();

        assert!(handle.is_cancelled());
        assert!(handle_clone.is_cancelled());
    }

    #[test]
    fn test_anonymous_pipe_read_write_eof() {
        let (mut reader, write_handle) =
            AnonymousPipe::create_inheritable_write().expect("create pipe failed");

        // Write some bytes to the write handle
        let data = b"test observation byte stream";
        let mut written: DWORD = 0;
        let success = unsafe {
            WriteFile(
                write_handle,
                data.as_ptr() as *const c_void,
                data.len() as DWORD,
                &mut written,
                null_mut(),
            )
        };
        assert_ne!(success, 0);
        assert_eq!(written as usize, data.len());

        // Close write handle to simulate child exit / EOF
        unsafe {
            CloseHandle(write_handle);
        }

        let mut read_buf = Vec::new();
        reader
            .read_to_end(&mut read_buf)
            .expect("read_to_end failed");
        assert_eq!(read_buf, data);
    }
}
