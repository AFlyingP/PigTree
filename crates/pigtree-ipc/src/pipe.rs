//! Win32 Named Pipe server, client, and stream implementations.

use crate::error::IpcError;
use crate::security::create_pipe_security_attributes;
use crate::win32::*;
use std::ffi::c_void;
use std::io::{self, Read, Write};
use std::ptr::null_mut;

/// Formats a canonical Win32 named pipe path for a private engine session.
pub fn format_pipe_name(session_id: &str) -> String {
    format!(r#"\\.\pipe\pigtree-engine-{session_id}"#)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PipeRole {
    Server,
    Client,
}

#[derive(Debug)]
pub struct PipeStream {
    handle: HANDLE,
    role: PipeRole,
}

impl PipeStream {
    /// Constructs a `PipeStream` from a raw Win32 handle and an endpoint role.
    ///
    /// # Safety
    /// The caller must ensure `handle` is a valid open Win32 pipe handle matching `role`.
    pub unsafe fn from_raw_handle(handle: HANDLE, role: PipeRole) -> Self {
        Self { handle, role }
    }

    pub fn raw_handle(&self) -> HANDLE {
        self.handle
    }

    pub fn role(&self) -> PipeRole {
        self.role
    }

    pub fn get_client_pid(&self) -> Result<u32, IpcError> {
        let mut pid: ULONG = 0;
        unsafe {
            if GetNamedPipeClientProcessId(self.handle, &mut pid) == 0 {
                return Err(IpcError::Win32 {
                    code: GetLastError(),
                    message: "GetNamedPipeClientProcessId failed".to_string(),
                });
            }
        }
        Ok(pid)
    }

    pub fn get_server_pid(&self) -> Result<u32, IpcError> {
        let mut pid: ULONG = 0;
        unsafe {
            if GetNamedPipeServerProcessId(self.handle, &mut pid) == 0 {
                return Err(IpcError::Win32 {
                    code: GetLastError(),
                    message: "GetNamedPipeServerProcessId failed".to_string(),
                });
            }
        }
        Ok(pid)
    }

    pub fn get_client_session_id(&self) -> Result<u32, IpcError> {
        let mut session_id: ULONG = 0;
        unsafe {
            if GetNamedPipeClientSessionId(self.handle, &mut session_id) == 0 {
                return Err(IpcError::Win32 {
                    code: GetLastError(),
                    message: "GetNamedPipeClientSessionId failed".to_string(),
                });
            }
        }
        Ok(session_id)
    }

    pub fn get_server_session_id(&self) -> Result<u32, IpcError> {
        let mut session_id: ULONG = 0;
        unsafe {
            if GetNamedPipeServerSessionId(self.handle, &mut session_id) == 0 {
                return Err(IpcError::Win32 {
                    code: GetLastError(),
                    message: "GetNamedPipeServerSessionId failed".to_string(),
                });
            }
        }
        Ok(session_id)
    }

    pub fn has_incoming_data(&self) -> Result<bool, IpcError> {
        let mut bytes_avail: DWORD = 0;
        let res = unsafe {
            PeekNamedPipe(
                self.handle,
                null_mut(),
                0,
                null_mut(),
                &mut bytes_avail,
                null_mut(),
            )
        };
        if res == 0 {
            let err = unsafe { GetLastError() };
            if err == ERROR_BROKEN_PIPE {
                return Ok(false);
            }
            return Err(IpcError::Win32 {
                code: err,
                message: "PeekNamedPipe failed".to_string(),
            });
        }
        Ok(bytes_avail > 0)
    }

    pub fn read_overlapped(
        &mut self,
        buf: &mut [u8],
        cancel_event: Option<HANDLE>,
        timeout_ms: Option<u32>,
    ) -> Result<usize, IpcError> {
        if buf.is_empty() {
            return Ok(0);
        }

        let h_event = unsafe { CreateEventW(null_mut(), TRUE, FALSE, null_mut()) };
        if h_event.is_null() || h_event == INVALID_HANDLE_VALUE {
            return Err(IpcError::Win32 {
                code: unsafe { GetLastError() },
                message: "CreateEventW failed".to_string(),
            });
        }

        struct EventGuard(HANDLE);
        impl Drop for EventGuard {
            fn drop(&mut self) {
                unsafe {
                    CloseHandle(self.0);
                }
            }
        }
        let _guard = EventGuard(h_event);

        let mut overlapped: OVERLAPPED = unsafe { std::mem::zeroed() };
        overlapped.hEvent = h_event;

        let mut bytes_read: DWORD = 0;
        let success = unsafe {
            ReadFile(
                self.handle,
                buf.as_mut_ptr() as *mut c_void,
                buf.len() as DWORD,
                &mut bytes_read,
                &mut overlapped,
            )
        };

        if success != 0 {
            return Ok(bytes_read as usize);
        }

        let err = unsafe { GetLastError() };
        if err == ERROR_BROKEN_PIPE {
            return Ok(0); // Clean EOF
        }
        if err != ERROR_IO_PENDING {
            return Err(IpcError::Win32 {
                code: err,
                message: "ReadFile failed".to_string(),
            });
        }

        let mut handles = [h_event, null_mut()];
        let count = if let Some(ce) = cancel_event {
            handles[1] = ce;
            2
        } else {
            1
        };

        let wait_res = unsafe {
            WaitForMultipleObjects(
                count,
                handles.as_ptr(),
                FALSE,
                timeout_ms.unwrap_or(INFINITE),
            )
        };

        if wait_res == WAIT_OBJECT_0 {
            let mut transferred: DWORD = 0;
            let res = unsafe {
                GetOverlappedResult(self.handle, &mut overlapped, &mut transferred, FALSE)
            };
            if res != 0 {
                Ok(transferred as usize)
            } else {
                let err = unsafe { GetLastError() };
                if err == ERROR_BROKEN_PIPE {
                    Ok(0)
                } else {
                    Err(IpcError::Win32 {
                        code: err,
                        message: "GetOverlappedResult failed".to_string(),
                    })
                }
            }
        } else if count == 2 && wait_res == WAIT_OBJECT_0 + 1 {
            unsafe {
                CancelIoEx(self.handle, &mut overlapped);
                let mut dummy: DWORD = 0;
                GetOverlappedResult(self.handle, &mut overlapped, &mut dummy, TRUE);
            }
            Err(IpcError::Cancelled)
        } else if wait_res == WAIT_TIMEOUT {
            unsafe {
                CancelIoEx(self.handle, &mut overlapped);
                let mut dummy: DWORD = 0;
                GetOverlappedResult(self.handle, &mut overlapped, &mut dummy, TRUE);
            }
            Err(IpcError::Timeout)
        } else {
            let err = unsafe { GetLastError() };
            unsafe {
                CancelIoEx(self.handle, &mut overlapped);
                let mut dummy: DWORD = 0;
                GetOverlappedResult(self.handle, &mut overlapped, &mut dummy, TRUE);
            }
            Err(IpcError::Win32 {
                code: err,
                message: "WaitForMultipleObjects failed".to_string(),
            })
        }
    }

    pub fn read_exact_interruptible(
        &mut self,
        mut buf: &mut [u8],
        cancel_event: Option<HANDLE>,
        timeout_ms: Option<u32>,
    ) -> Result<(), IpcError> {
        while !buf.is_empty() {
            let n = self.read_overlapped(buf, cancel_event, timeout_ms)?;
            if n == 0 {
                return Err(IpcError::Protocol(
                    pigtree_protocol::FrameParseError::PrematureEof,
                ));
            }
            let tmp = buf;
            buf = &mut tmp[n..];
        }
        Ok(())
    }

    pub fn write_overlapped(
        &mut self,
        buf: &[u8],
        _cancel_event: Option<HANDLE>,
        _timeout_ms: Option<u32>,
    ) -> Result<usize, IpcError> {
        if buf.is_empty() {
            return Ok(0);
        }

        let h_event = unsafe { CreateEventW(null_mut(), TRUE, FALSE, null_mut()) };
        if h_event.is_null() || h_event == INVALID_HANDLE_VALUE {
            return Err(IpcError::Win32 {
                code: unsafe { GetLastError() },
                message: "CreateEventW failed".to_string(),
            });
        }

        struct EventGuard(HANDLE);
        impl Drop for EventGuard {
            fn drop(&mut self) {
                unsafe {
                    CloseHandle(self.0);
                }
            }
        }
        let _guard = EventGuard(h_event);

        let mut overlapped: OVERLAPPED = unsafe { std::mem::zeroed() };
        overlapped.hEvent = h_event;

        let mut written: DWORD = 0;
        let success = unsafe {
            WriteFile(
                self.handle,
                buf.as_ptr() as *const c_void,
                buf.len() as DWORD,
                &mut written,
                &mut overlapped,
            )
        };

        if success != 0 {
            return Ok(written as usize);
        }

        let err = unsafe { GetLastError() };
        if err == ERROR_IO_PENDING {
            let mut transferred: DWORD = 0;
            let res = unsafe {
                GetOverlappedResult(self.handle, &mut overlapped, &mut transferred, TRUE)
            };
            if res != 0 {
                Ok(transferred as usize)
            } else {
                let err = unsafe { GetLastError() };
                if err == ERROR_BROKEN_PIPE {
                    return Err(IpcError::Io(io::Error::new(
                        io::ErrorKind::BrokenPipe,
                        "broken pipe",
                    )));
                }
                Err(IpcError::Win32 {
                    code: err,
                    message: "GetOverlappedResult write failed".to_string(),
                })
            }
        } else if err == ERROR_BROKEN_PIPE {
            Err(IpcError::Io(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "broken pipe",
            )))
        } else {
            Err(IpcError::Win32 {
                code: err,
                message: "WriteFile failed".to_string(),
            })
        }
    }
}

impl Read for PipeStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.read_overlapped(buf, None, None).map_err(|e| match e {
            IpcError::Io(io_err) => io_err,
            other => io::Error::other(other.to_string()),
        })
    }
}

impl Write for PipeStream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.write_overlapped(buf, None, None).map_err(|e| match e {
            IpcError::Io(io_err) => io_err,
            other => io::Error::other(other.to_string()),
        })
    }

    fn flush(&mut self) -> io::Result<()> {
        let success = unsafe { FlushFileBuffers(self.handle) };
        if success != 0 {
            Ok(())
        } else {
            let err = unsafe { GetLastError() };
            if err == ERROR_BROKEN_PIPE {
                Ok(())
            } else {
                Err(io::Error::from_raw_os_error(err as i32))
            }
        }
    }
}

impl Drop for PipeStream {
    fn drop(&mut self) {
        if !self.handle.is_null() && self.handle != INVALID_HANDLE_VALUE {
            unsafe {
                if self.role == PipeRole::Server {
                    DisconnectNamedPipe(self.handle);
                }
                CloseHandle(self.handle);
            }
        }
    }
}

unsafe impl Send for PipeStream {}
unsafe impl Sync for PipeStream {}

pub struct NamedPipeServer {
    handle: HANDLE,
}

impl NamedPipeServer {
    pub fn create(pipe_name: &str) -> Result<Self, IpcError> {
        let (_guard, mut sa) = create_pipe_security_attributes()?;
        let wide_name: Vec<u16> = pipe_name.encode_utf16().chain(std::iter::once(0)).collect();

        let h_pipe = unsafe {
            CreateNamedPipeW(
                wide_name.as_ptr(),
                PIPE_ACCESS_DUPLEX | FILE_FLAG_FIRST_PIPE_INSTANCE | FILE_FLAG_OVERLAPPED,
                PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
                1,     // Max 1 instance
                65536, // Output buffer 64 KiB
                65536, // Input buffer 64 KiB
                5000,  // Default timeout 5000ms
                &mut sa,
            )
        };

        if h_pipe == INVALID_HANDLE_VALUE || h_pipe.is_null() {
            return Err(IpcError::Win32 {
                code: unsafe { GetLastError() },
                message: format!("CreateNamedPipeW failed for pipe {pipe_name}"),
            });
        }

        Ok(Self { handle: h_pipe })
    }

    pub fn accept(self) -> Result<PipeStream, IpcError> {
        let h_event = unsafe { CreateEventW(null_mut(), TRUE, FALSE, null_mut()) };
        if h_event.is_null() || h_event == INVALID_HANDLE_VALUE {
            return Err(IpcError::Win32 {
                code: unsafe { GetLastError() },
                message: "CreateEventW failed".to_string(),
            });
        }

        struct EventGuard(HANDLE);
        impl Drop for EventGuard {
            fn drop(&mut self) {
                unsafe {
                    CloseHandle(self.0);
                }
            }
        }
        let _guard = EventGuard(h_event);

        let mut overlapped: OVERLAPPED = unsafe { std::mem::zeroed() };
        overlapped.hEvent = h_event;

        let success =
            unsafe { ConnectNamedPipe(self.handle, &mut overlapped as *mut _ as *mut c_void) };
        if success != 0 {
            let stream = PipeStream {
                handle: self.handle,
                role: PipeRole::Server,
            };
            std::mem::forget(self);
            Ok(stream)
        } else {
            let err = unsafe { GetLastError() };
            if err == ERROR_PIPE_CONNECTED {
                let stream = PipeStream {
                    handle: self.handle,
                    role: PipeRole::Server,
                };
                std::mem::forget(self);
                Ok(stream)
            } else if err == ERROR_IO_PENDING {
                let wait_res = unsafe { WaitForSingleObject(h_event, 10000) };
                if wait_res == WAIT_OBJECT_0 {
                    let mut transferred: DWORD = 0;
                    let res = unsafe {
                        GetOverlappedResult(self.handle, &mut overlapped, &mut transferred, FALSE)
                    };
                    if res != 0 || unsafe { GetLastError() } == ERROR_PIPE_CONNECTED {
                        let stream = PipeStream {
                            handle: self.handle,
                            role: PipeRole::Server,
                        };
                        std::mem::forget(self);
                        Ok(stream)
                    } else {
                        let err = unsafe { GetLastError() };
                        Err(IpcError::Win32 {
                            code: err,
                            message: "ConnectNamedPipe overlapped result failed".to_string(),
                        })
                    }
                } else {
                    unsafe {
                        CancelIoEx(self.handle, &mut overlapped);
                        let mut dummy: DWORD = 0;
                        GetOverlappedResult(self.handle, &mut overlapped, &mut dummy, TRUE);
                    }
                    if wait_res == WAIT_TIMEOUT {
                        Err(IpcError::Timeout)
                    } else {
                        Err(IpcError::Win32 {
                            code: unsafe { GetLastError() },
                            message: "WaitForSingleObject failed on ConnectNamedPipe".to_string(),
                        })
                    }
                }
            } else {
                Err(IpcError::Win32 {
                    code: err,
                    message: "ConnectNamedPipe failed".to_string(),
                })
            }
        }
    }
}

impl Drop for NamedPipeServer {
    fn drop(&mut self) {
        if !self.handle.is_null() && self.handle != INVALID_HANDLE_VALUE {
            unsafe {
                CloseHandle(self.handle);
            }
        }
    }
}

pub struct NamedPipeClient;

impl NamedPipeClient {
    pub fn connect(pipe_name: &str, timeout_ms: u32) -> Result<PipeStream, IpcError> {
        let wide_name: Vec<u16> = pipe_name.encode_utf16().chain(std::iter::once(0)).collect();
        let start = std::time::Instant::now();
        let timeout = std::time::Duration::from_millis(timeout_ms as u64);

        loop {
            let h_file = unsafe {
                CreateFileW(
                    wide_name.as_ptr(),
                    GENERIC_READ | GENERIC_WRITE,
                    0, // No sharing
                    null_mut(),
                    OPEN_EXISTING,
                    SECURITY_SQOS_PRESENT | SECURITY_IDENTIFICATION | FILE_FLAG_OVERLAPPED,
                    null_mut(),
                )
            };

            if h_file != INVALID_HANDLE_VALUE && !h_file.is_null() {
                let mode: DWORD = PIPE_READMODE_BYTE;
                unsafe {
                    SetNamedPipeHandleState(h_file, &mode, null_mut(), null_mut());
                }
                return Ok(PipeStream {
                    handle: h_file,
                    role: PipeRole::Client,
                });
            }

            let err = unsafe { GetLastError() };
            if err == ERROR_PIPE_BUSY {
                unsafe {
                    WaitNamedPipeW(wide_name.as_ptr(), 100);
                }
            } else if err == ERROR_FILE_NOT_FOUND {
                std::thread::sleep(std::time::Duration::from_millis(10));
            } else {
                return Err(IpcError::Win32 {
                    code: err,
                    message: format!("CreateFileW failed for pipe {pipe_name}"),
                });
            }

            if start.elapsed() >= timeout {
                return Err(IpcError::Timeout);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_pipe_name() {
        let sess_id = format!("sess-{}", std::process::id());
        let expected = format!(r#"\\.\pipe\pigtree-engine-{sess_id}"#);
        assert_eq!(format_pipe_name(&sess_id), expected);
    }

    #[test]
    fn test_pipe_stream_roles() {
        let dummy_handle = INVALID_HANDLE_VALUE;
        let server_stream = unsafe { PipeStream::from_raw_handle(dummy_handle, PipeRole::Server) };
        assert_eq!(server_stream.role(), PipeRole::Server);
        drop(server_stream);

        let client_stream = unsafe { PipeStream::from_raw_handle(dummy_handle, PipeRole::Client) };
        assert_eq!(client_stream.role(), PipeRole::Client);
        drop(client_stream);
    }

    #[test]
    fn test_named_pipe_roundtrip() {
        let nonce = crate::security::generate_nonce();
        let hex: String = nonce[0..8].iter().map(|b| format!("{b:02x}")).collect();
        let sess_id = format!("roundtrip-{}-{hex}", std::process::id());
        let pipe_name = format_pipe_name(&sess_id);
        let server = NamedPipeServer::create(&pipe_name).expect("server create");

        let pipe_name_clone = pipe_name.clone();
        let client_thread = std::thread::spawn(move || {
            let mut client =
                NamedPipeClient::connect(&pipe_name_clone, 3000).expect("client connect");
            assert_eq!(client.role(), PipeRole::Client);
            client.write_all(b"PING").expect("client write");
            client.flush().expect("client flush");
            let mut resp = [0u8; 4];
            client.read_exact(&mut resp).expect("client read");
            assert_eq!(&resp, b"PONG");
        });

        let mut server_stream = server.accept().expect("server accept");
        assert_eq!(server_stream.role(), PipeRole::Server);
        let mut req = [0u8; 4];
        server_stream.read_exact(&mut req).expect("server read");
        assert_eq!(&req, b"PING");
        server_stream.write_all(b"PONG").expect("server write");
        server_stream.flush().expect("server flush");

        client_thread.join().expect("client thread join");
    }
}
