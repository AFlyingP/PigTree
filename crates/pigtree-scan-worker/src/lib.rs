//! PigTree scan worker library seam and Win32 directory traversal implementation.
#![allow(non_camel_case_types, non_snake_case, non_upper_case_globals)]

#[cfg(not(windows))]
compile_error!("pigtree-scan-worker targets Windows only");

use pigtree_protocol::{
    CoverageGapObservation, DirectoryObservation, FileObservation, ObservationWriter, RunOutcome,
    SpecialObservation, TerminalObservation,
};
use std::ffi::c_void;
use std::fmt;
use std::io::Write;
use std::path::Path;
use std::ptr::null_mut;
use std::time::Instant;

pub type HANDLE = *mut c_void;
pub type BOOL = i32;
pub type DWORD = u32;
pub type LPCWSTR = *const u16;
pub type LPWSTR = *mut u16;

pub const TRUE: BOOL = 1;
pub const FALSE: BOOL = 0;
pub const INVALID_HANDLE_VALUE: HANDLE = -1isize as HANDLE;

pub const FILE_ATTRIBUTE_READONLY: DWORD = 0x00000001;
pub const FILE_ATTRIBUTE_HIDDEN: DWORD = 0x00000002;
pub const FILE_ATTRIBUTE_SYSTEM: DWORD = 0x00000004;
pub const FILE_ATTRIBUTE_DIRECTORY: DWORD = 0x00000010;
pub const FILE_ATTRIBUTE_ARCHIVE: DWORD = 0x00000020;
pub const FILE_ATTRIBUTE_DEVICE: DWORD = 0x00000040;
pub const FILE_ATTRIBUTE_NORMAL: DWORD = 0x00000080;
pub const FILE_ATTRIBUTE_TEMPORARY: DWORD = 0x00000100;
pub const FILE_ATTRIBUTE_SPARSE_FILE: DWORD = 0x00000200;
pub const FILE_ATTRIBUTE_REPARSE_POINT: DWORD = 0x00000400;
pub const FILE_ATTRIBUTE_COMPRESSED: DWORD = 0x00000800;
pub const FILE_ATTRIBUTE_OFFLINE: DWORD = 0x00001000;
pub const FILE_ATTRIBUTE_NOT_CONTENT_INDEXED: DWORD = 0x00002000;
pub const FILE_ATTRIBUTE_ENCRYPTED: DWORD = 0x00004000;
pub const INVALID_FILE_ATTRIBUTES: DWORD = 0xFFFFFFFF;

pub const ERROR_FILE_NOT_FOUND: DWORD = 2;
pub const ERROR_PATH_NOT_FOUND: DWORD = 3;
pub const ERROR_ACCESS_DENIED: DWORD = 5;
pub const ERROR_NO_MORE_FILES: DWORD = 18;

pub const WAIT_OBJECT_0: DWORD = 0;
pub const WAIT_TIMEOUT: DWORD = 258;
pub const WAIT_FAILED: DWORD = 0xFFFFFFFF;

pub const FindExInfoStandard: i32 = 0;
pub const FindExInfoBasic: i32 = 1;

pub const FindExSearchNameMatch: i32 = 0;
pub const FindExSearchLimitToDirectories: i32 = 1;

pub const FIND_FIRST_EX_LARGE_FETCH: DWORD = 2;
pub const GetFileExInfoStandard: i32 = 0;

#[repr(C)]
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub struct FILETIME {
    pub dwLowDateTime: DWORD,
    pub dwHighDateTime: DWORD,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct WIN32_FIND_DATAW {
    pub dwFileAttributes: DWORD,
    pub ftCreationTime: FILETIME,
    pub ftLastAccessTime: FILETIME,
    pub ftLastWriteTime: FILETIME,
    pub nFileSizeHigh: DWORD,
    pub nFileSizeLow: DWORD,
    pub dwReserved0: DWORD,
    pub dwReserved1: DWORD,
    pub cFileName: [u16; 260],
    pub cAlternateFileName: [u16; 14],
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct WIN32_FILE_ATTRIBUTE_DATA {
    pub dwFileAttributes: DWORD,
    pub ftCreationTime: FILETIME,
    pub ftLastAccessTime: FILETIME,
    pub ftLastWriteTime: FILETIME,
    pub nFileSizeHigh: DWORD,
    pub nFileSizeLow: DWORD,
}

#[link(name = "kernel32")]
extern "system" {
    pub fn GetLastError() -> DWORD;
    pub fn CloseHandle(hObject: HANDLE) -> BOOL;
    pub fn FindFirstFileExW(
        lpFileName: LPCWSTR,
        fInfoLevelId: i32,
        lpFindFileData: *mut WIN32_FIND_DATAW,
        fSearchOp: i32,
        lpSearchFilter: *mut c_void,
        dwAdditionalFlags: DWORD,
    ) -> HANDLE;
    pub fn FindNextFileW(hFindFile: HANDLE, lpFindFileData: *mut WIN32_FIND_DATAW) -> BOOL;
    pub fn FindClose(hFindFile: HANDLE) -> BOOL;
    pub fn GetFileAttributesExW(
        lpFileName: LPCWSTR,
        fInfoLevelId: i32,
        lpFileInformation: *mut c_void,
    ) -> BOOL;
    pub fn WaitForSingleObject(hHandle: HANDLE, dwMilliseconds: DWORD) -> DWORD;
    pub fn WriteFile(
        hFile: HANDLE,
        lpBuffer: *const c_void,
        nNumberOfBytesToWrite: DWORD,
        lpNumberOfBytesWritten: *mut DWORD,
        lpOverlapped: *mut c_void,
    ) -> BOOL;
    pub fn FlushFileBuffers(hFile: HANDLE) -> BOOL;
    pub fn CreateEventW(
        lpEventAttributes: *mut c_void,
        bManualReset: BOOL,
        bInitialState: BOOL,
        lpName: LPCWSTR,
    ) -> HANDLE;
    pub fn SetEvent(hEvent: HANDLE) -> BOOL;
    pub fn ResetEvent(hEvent: HANDLE) -> BOOL;
    pub fn FormatMessageW(
        dwFlags: DWORD,
        lpSource: *const c_void,
        dwMessageId: DWORD,
        dwLanguageId: DWORD,
        lpBuffer: LPWSTR,
        nSize: DWORD,
        Arguments: *mut c_void,
    ) -> DWORD;
}

/// Convert a Windows FILETIME to milliseconds elapsed since Unix epoch (1970-01-01 00:00:00 UTC).
pub fn filetime_to_unix_ms(ft: &FILETIME) -> u64 {
    let ft_u64 = ((ft.dwHighDateTime as u64) << 32) | (ft.dwLowDateTime as u64);
    // Number of 100-nanosecond intervals between 1601-01-01 and 1970-01-01 UTC:
    // 134,774 days * 86,400 seconds/day * 10,000,000 intervals/second = 116,444,736,000,000,000
    const WINDOWS_EPOCH_DIFFERENCE: u64 = 116_444_736_000_000_000;
    if ft_u64 > WINDOWS_EPOCH_DIFFERENCE {
        (ft_u64 - WINDOWS_EPOCH_DIFFERENCE) / 10_000
    } else {
        0
    }
}

/// Helper to format a Win32 system error code into a human-readable message.
pub fn format_win32_error(code: u32) -> String {
    const FORMAT_MESSAGE_FROM_SYSTEM: DWORD = 0x00001000;
    const FORMAT_MESSAGE_IGNORE_INSERTS: DWORD = 0x00000200;
    let mut buf = [0u16; 512];
    // SAFETY: Calling FormatMessageW with stack buffer of 512 u16 elements.
    unsafe {
        let len = FormatMessageW(
            FORMAT_MESSAGE_FROM_SYSTEM | FORMAT_MESSAGE_IGNORE_INSERTS,
            null_mut(),
            code,
            0,
            buf.as_mut_ptr(),
            buf.len() as DWORD,
            null_mut(),
        );
        if len > 0 {
            let msg = String::from_utf16_lossy(&buf[..len as usize]);
            msg.trim().to_string()
        } else {
            format!("Win32 error {code}")
        }
    }
}

/// Convert a string/path to null-terminated UTF-16 for Win32 API calls.
pub fn to_wide_null(s: &str) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    std::ffi::OsStr::new(s)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

/// Convert a null-terminated UTF-16 slice from WIN32_FIND_DATAW.cFileName to a String.
pub fn wide_slice_to_string(slice: &[u16]) -> String {
    let len = slice.iter().position(|&c| c == 0).unwrap_or(slice.len());
    String::from_utf16_lossy(&slice[..len])
}

/// RAII wrapper for FindFirstFileExW / FindNextFileW search handle.
pub struct FindHandle(pub HANDLE);

impl Drop for FindHandle {
    fn drop(&mut self) {
        if !self.0.is_null() && self.0 != INVALID_HANDLE_VALUE {
            // SAFETY: Closing valid search handle with FindClose.
            unsafe {
                FindClose(self.0);
            }
        }
    }
}

/// Abstraction for cooperative scan cancellation polling.
pub trait Cancellation: Send + Sync {
    fn is_cancelled(&self) -> bool;
}

/// Cancellation token that is never cancelled (used in non-cancelling contexts and tests).
#[derive(Debug, Default, Clone, Copy)]
pub struct NoCancellation;

impl Cancellation for NoCancellation {
    fn is_cancelled(&self) -> bool {
        false
    }
}

/// Cancellation token backed by an atomic boolean.
pub struct AtomicCancellation<'a>(pub &'a std::sync::atomic::AtomicBool);

impl<'a> Cancellation for AtomicCancellation<'a> {
    fn is_cancelled(&self) -> bool {
        self.0.load(std::sync::atomic::Ordering::Relaxed)
    }
}

/// Cancellation abstraction backed by a Win32 manual-reset event.
pub struct Win32EventCancellation {
    handle: HANDLE,
    owns_handle: bool,
}

impl Win32EventCancellation {
    /// Wrap an unowned borrowed Win32 event HANDLE without closing it on drop.
    pub fn from_raw(handle: HANDLE) -> Self {
        Self {
            handle,
            owns_handle: false,
        }
    }

    /// Wrap an owned Win32 event HANDLE that will be closed via CloseHandle on drop.
    pub fn from_owned(handle: HANDLE) -> Self {
        Self {
            handle,
            owns_handle: true,
        }
    }

    pub fn handle(&self) -> HANDLE {
        self.handle
    }
}

impl Cancellation for Win32EventCancellation {
    fn is_cancelled(&self) -> bool {
        if self.handle.is_null() || self.handle == INVALID_HANDLE_VALUE {
            return false;
        }
        // SAFETY: Non-blocking zero-millisecond wait on the event handle.
        unsafe { WaitForSingleObject(self.handle, 0) == WAIT_OBJECT_0 }
    }
}

impl Drop for Win32EventCancellation {
    fn drop(&mut self) {
        if self.owns_handle && !self.handle.is_null() && self.handle != INVALID_HANDLE_VALUE {
            // SAFETY: Closing owned handle with CloseHandle.
            unsafe {
                CloseHandle(self.handle);
            }
        }
    }
}

// SAFETY: Win32EventCancellation holds an opaque kernel event handle queried thread-safely via WaitForSingleObject.
unsafe impl Send for Win32EventCancellation {}
unsafe impl Sync for Win32EventCancellation {}

/// RAII-safe pipe writer wrapping a raw Win32 pipe HANDLE implementing `std::io::Write`.
pub struct PipeWriter {
    handle: HANDLE,
    owns_handle: bool,
}

impl PipeWriter {
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

    pub fn handle(&self) -> HANDLE {
        self.handle
    }
}

impl Write for PipeWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if self.handle.is_null() || self.handle == INVALID_HANDLE_VALUE {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Invalid pipe handle",
            ));
        }
        let mut written: DWORD = 0;
        // SAFETY: Writing buffer to the Win32 pipe handle.
        let ok = unsafe {
            WriteFile(
                self.handle,
                buf.as_ptr() as *const c_void,
                buf.len() as DWORD,
                &mut written,
                null_mut(),
            )
        };
        if ok == 0 {
            let err = unsafe { GetLastError() };
            Err(std::io::Error::from_raw_os_error(err as i32))
        } else {
            Ok(written as usize)
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        if self.handle.is_null() || self.handle == INVALID_HANDLE_VALUE {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Invalid pipe handle",
            ));
        }
        // SAFETY: Flushing buffers for the Win32 pipe handle.
        let ok = unsafe { FlushFileBuffers(self.handle) };
        if ok == 0 {
            let err = unsafe { GetLastError() };
            // FlushFileBuffers on anonymous pipes or certain handles may return ERROR_INVALID_FUNCTION (1); ignore if harmless.
            if err != 1 {
                return Err(std::io::Error::from_raw_os_error(err as i32));
            }
        }
        Ok(())
    }
}

impl Drop for PipeWriter {
    fn drop(&mut self) {
        if self.owns_handle && !self.handle.is_null() && self.handle != INVALID_HANDLE_VALUE {
            // SAFETY: Closing owned handle with CloseHandle.
            unsafe {
                CloseHandle(self.handle);
            }
        }
    }
}

// SAFETY: PipeWriter writes to an OS pipe handle without shared mutable state.
unsafe impl Send for PipeWriter {}
unsafe impl Sync for PipeWriter {}

/// Parsed command line arguments for the scan worker binary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkerArgs {
    pub target: String,
    pub pipe_handle: usize,
    pub cancel_event_handle: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArgsError {
    MissingArgument(&'static str),
    InvalidHandle(&'static str, String),
    UnexpectedArgument(String),
}

impl fmt::Display for ArgsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ArgsError::MissingArgument(arg) => write!(f, "missing required argument: {arg}"),
            ArgsError::InvalidHandle(arg, val) => {
                write!(f, "invalid handle integer for {arg}: {val}")
            }
            ArgsError::UnexpectedArgument(arg) => write!(f, "unexpected argument: {arg}"),
        }
    }
}

impl std::error::Error for ArgsError {}

/// Parse scan worker command line arguments.
pub fn parse_worker_args<I>(args: I) -> Result<WorkerArgs, ArgsError>
where
    I: IntoIterator<Item = String>,
{
    let mut target = None;
    let mut pipe_handle = None;
    let mut cancel_event_handle = None;

    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--target" => {
                let val = iter.next().ok_or(ArgsError::MissingArgument("--target"))?;
                target = Some(val);
            }
            "--pipe-handle" => {
                let val = iter
                    .next()
                    .ok_or(ArgsError::MissingArgument("--pipe-handle"))?;
                let handle = val
                    .parse::<usize>()
                    .map_err(|_| ArgsError::InvalidHandle("--pipe-handle", val))?;
                pipe_handle = Some(handle);
            }
            "--cancel-event-handle" => {
                let val = iter
                    .next()
                    .ok_or(ArgsError::MissingArgument("--cancel-event-handle"))?;
                let handle = val
                    .parse::<usize>()
                    .map_err(|_| ArgsError::InvalidHandle("--cancel-event-handle", val))?;
                cancel_event_handle = Some(handle);
            }
            other if other.starts_with("--") => {
                return Err(ArgsError::UnexpectedArgument(other.to_string()));
            }
            _ => {
                // Ignore non-flag arguments (like executable name if passed)
            }
        }
    }

    Ok(WorkerArgs {
        target: target.ok_or(ArgsError::MissingArgument("--target"))?,
        pipe_handle: pipe_handle.ok_or(ArgsError::MissingArgument("--pipe-handle"))?,
        cancel_event_handle: cancel_event_handle
            .ok_or(ArgsError::MissingArgument("--cancel-event-handle"))?,
    })
}

/// Convert a path to an extended local path (`\\?\...`) for long path support.
pub fn normalize_to_extended_path(p: &Path) -> String {
    let path_str = p.to_string_lossy().to_string();
    if path_str.starts_with("\\\\?\\") || path_str.starts_with(r"\\?\") {
        return path_str;
    }
    if let Ok(canon) = std::fs::canonicalize(p) {
        let s = canon.to_string_lossy().to_string();
        if s.starts_with(r"\\?\") || s.starts_with("\\\\?\\") {
            return s;
        }
    }
    format!(r"\\?\\{}", path_str)
}

struct DirectoryStackItem {
    entry_id: u32,
    extended_path: String,
    display_path: String,
}

/// Primary public library seam for traversing a directory and streaming observation records.
pub fn scan_directory<W: Write, C: Cancellation>(
    target: &Path,
    writer: &mut ObservationWriter<W>,
    cancellation: &C,
) -> std::io::Result<TerminalObservation> {
    // Validate target defensively: reject UNC / remote / unsupported filesystem before any observation
    let validated = pigtree_ipc::validator::validate_scan_target(target)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e.to_string()))?;

    let start_time = Instant::now();
    let mut total_directories: u64 = 0;
    let mut total_files: u64 = 0;
    let mut total_logical_bytes: u64 = 0;
    let total_allocated_bytes: u64 = 0;
    let mut coverage_gap_count: u32 = 0;
    let mut next_entry_id: u32 = 1;

    let target_display = validated.display_path;

    // Check cancellation immediately
    if cancellation.is_cancelled() {
        let duration_ms = start_time.elapsed().as_millis() as u64;
        let term = TerminalObservation {
            outcome: RunOutcome::Cancelled,
            total_directories,
            total_files,
            total_logical_bytes,
            total_allocated_bytes,
            coverage_gap_count,
            duration_ms,
        };
        writer.write_terminal(&term)?;
        writer.flush()?;
        return Ok(term);
    }

    let extended_target = validated.extended_path;
    let extended_target_w = to_wide_null(&extended_target);

    // Retrieve target root attributes and timestamps
    let mut root_find_data: WIN32_FIND_DATAW = unsafe { std::mem::zeroed() };
    // SAFETY: Calling FindFirstFileExW to query target root attributes.
    let h_root_find = unsafe {
        FindFirstFileExW(
            extended_target_w.as_ptr(),
            FindExInfoBasic,
            &mut root_find_data,
            FindExSearchNameMatch,
            null_mut(),
            0,
        )
    };

    let (root_attrs, root_reparse_tag, root_creation, root_last_write, root_last_access) =
        if h_root_find != INVALID_HANDLE_VALUE {
            let _guard = FindHandle(h_root_find);
            let is_reparse = (root_find_data.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT) != 0;
            let reparse_tag = if is_reparse {
                root_find_data.dwReserved0
            } else {
                0
            };
            (
                root_find_data.dwFileAttributes,
                reparse_tag,
                filetime_to_unix_ms(&root_find_data.ftCreationTime),
                filetime_to_unix_ms(&root_find_data.ftLastWriteTime),
                filetime_to_unix_ms(&root_find_data.ftLastAccessTime),
            )
        } else {
            // For root volume drives (e.g. C:\) FindFirstFileExW without wildcard may fail; fallback to GetFileAttributesExW
            let mut attr_data: WIN32_FILE_ATTRIBUTE_DATA = unsafe { std::mem::zeroed() };
            // SAFETY: Calling GetFileAttributesExW with valid wide string and output buffer.
            let ok = unsafe {
                GetFileAttributesExW(
                    extended_target_w.as_ptr(),
                    GetFileExInfoStandard,
                    &mut attr_data as *mut _ as *mut c_void,
                )
            };
            if ok != 0 {
                (
                    attr_data.dwFileAttributes,
                    0,
                    filetime_to_unix_ms(&attr_data.ftCreationTime),
                    filetime_to_unix_ms(&attr_data.ftLastWriteTime),
                    filetime_to_unix_ms(&attr_data.ftLastAccessTime),
                )
            } else {
                let err = unsafe { GetLastError() };
                coverage_gap_count += 1;
                writer.write_coverage_gap(&CoverageGapObservation {
                    path: target_display.clone(),
                    error_code: err,
                    error_message: format_win32_error(err),
                })?;
                let duration_ms = start_time.elapsed().as_millis() as u64;
                let term = TerminalObservation {
                    outcome: RunOutcome::Failed,
                    total_directories: 0,
                    total_files: 0,
                    total_logical_bytes: 0,
                    total_allocated_bytes: 0,
                    coverage_gap_count,
                    duration_ms,
                };
                writer.write_terminal(&term)?;
                writer.flush()?;
                return Ok(term);
            }
        };

    // Validate that target is a directory
    if (root_attrs & FILE_ATTRIBUTE_DIRECTORY) == 0 {
        coverage_gap_count += 1;
        writer.write_coverage_gap(&CoverageGapObservation {
            path: target_display.clone(),
            error_code: ERROR_PATH_NOT_FOUND,
            error_message: "Target path is not a directory".to_string(),
        })?;
        let duration_ms = start_time.elapsed().as_millis() as u64;
        let term = TerminalObservation {
            outcome: RunOutcome::Failed,
            total_directories: 0,
            total_files: 0,
            total_logical_bytes: 0,
            total_allocated_bytes: 0,
            coverage_gap_count,
            duration_ms,
        };
        writer.write_terminal(&term)?;
        writer.flush()?;
        return Ok(term);
    }

    // Emit root directory observation (root observation id 1 parent 0)
    let root_id = next_entry_id;
    next_entry_id += 1;
    total_directories += 1;

    writer.write_directory(&DirectoryObservation {
        entry_id: root_id,
        parent_id: 0,
        name: target_display.clone(),
        file_attributes: root_attrs,
        reparse_tag: root_reparse_tag,
        creation_time_utc_ms: root_creation,
        last_write_time_utc_ms: root_last_write,
        last_access_time_utc_ms: root_last_access,
    })?;

    // Traversal iterative stack
    let mut stack: Vec<DirectoryStackItem> = Vec::new();
    stack.push(DirectoryStackItem {
        entry_id: root_id,
        extended_path: extended_target,
        display_path: target_display,
    });

    while let Some(current_dir) = stack.pop() {
        if cancellation.is_cancelled() {
            let duration_ms = start_time.elapsed().as_millis() as u64;
            let term = TerminalObservation {
                outcome: RunOutcome::Cancelled,
                total_directories,
                total_files,
                total_logical_bytes,
                total_allocated_bytes,
                coverage_gap_count,
                duration_ms,
            };
            writer.write_terminal(&term)?;
            writer.flush()?;
            return Ok(term);
        }

        let pattern = if current_dir.extended_path.ends_with('\\') {
            format!("{}*", current_dir.extended_path)
        } else {
            format!("{}\\*", current_dir.extended_path)
        };
        let pattern_w = to_wide_null(&pattern);

        let mut find_data: WIN32_FIND_DATAW = unsafe { std::mem::zeroed() };
        // SAFETY: Calling FindFirstFileExW to enumerate entries within the current directory.
        let h_find = unsafe {
            FindFirstFileExW(
                pattern_w.as_ptr(),
                FindExInfoBasic,
                &mut find_data,
                FindExSearchNameMatch,
                null_mut(),
                FIND_FIRST_EX_LARGE_FETCH,
            )
        };

        if h_find == INVALID_HANDLE_VALUE {
            let err = unsafe { GetLastError() };
            if err == ERROR_FILE_NOT_FOUND || err == ERROR_NO_MORE_FILES {
                // Empty directory - normal completion for this folder
                continue;
            } else {
                // Access denied or enumeration error: record coverage gap and continue with siblings
                coverage_gap_count += 1;
                writer.write_coverage_gap(&CoverageGapObservation {
                    path: current_dir.display_path.clone(),
                    error_code: err,
                    error_message: format_win32_error(err),
                })?;
                continue;
            }
        }

        let find_guard = FindHandle(h_find);

        loop {
            if cancellation.is_cancelled() {
                drop(find_guard);
                let duration_ms = start_time.elapsed().as_millis() as u64;
                let term = TerminalObservation {
                    outcome: RunOutcome::Cancelled,
                    total_directories,
                    total_files,
                    total_logical_bytes,
                    total_allocated_bytes,
                    coverage_gap_count,
                    duration_ms,
                };
                writer.write_terminal(&term)?;
                writer.flush()?;
                return Ok(term);
            }

            let entry_name = wide_slice_to_string(&find_data.cFileName);
            if entry_name != "." && entry_name != ".." {
                let entry_id = next_entry_id;
                next_entry_id += 1;

                let is_dir = (find_data.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY) != 0;
                let is_reparse = (find_data.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT) != 0;
                let is_device = (find_data.dwFileAttributes & FILE_ATTRIBUTE_DEVICE) != 0;
                let reparse_tag = if is_reparse { find_data.dwReserved0 } else { 0 };

                let creation_time = filetime_to_unix_ms(&find_data.ftCreationTime);
                let last_write_time = filetime_to_unix_ms(&find_data.ftLastWriteTime);
                let last_access_time = filetime_to_unix_ms(&find_data.ftLastAccessTime);

                if is_device {
                    writer.write_special(&SpecialObservation {
                        entry_id,
                        parent_id: current_dir.entry_id,
                        name: entry_name,
                        file_attributes: find_data.dwFileAttributes,
                        reparse_tag,
                        creation_time_utc_ms: creation_time,
                        last_write_time_utc_ms: last_write_time,
                        last_access_time_utc_ms: last_access_time,
                    })?;
                } else if is_dir {
                    total_directories += 1;
                    writer.write_directory(&DirectoryObservation {
                        entry_id,
                        parent_id: current_dir.entry_id,
                        name: entry_name.clone(),
                        file_attributes: find_data.dwFileAttributes,
                        reparse_tag,
                        creation_time_utc_ms: creation_time,
                        last_write_time_utc_ms: last_write_time,
                        last_access_time_utc_ms: last_access_time,
                    })?;

                    // Only descend into non-reparse subdirectories
                    if !is_reparse {
                        let child_ext = if current_dir.extended_path.ends_with('\\') {
                            format!("{}{}", current_dir.extended_path, entry_name)
                        } else {
                            format!("{}\\{}", current_dir.extended_path, entry_name)
                        };
                        let child_disp = if current_dir.display_path.ends_with('\\') {
                            format!("{}{}", current_dir.display_path, entry_name)
                        } else {
                            format!("{}\\{}", current_dir.display_path, entry_name)
                        };
                        stack.push(DirectoryStackItem {
                            entry_id,
                            extended_path: child_ext,
                            display_path: child_disp,
                        });
                    }
                } else {
                    total_files += 1;
                    let logical_size =
                        ((find_data.nFileSizeHigh as u64) << 32) | (find_data.nFileSizeLow as u64);
                    total_logical_bytes += logical_size;

                    writer.write_file(&FileObservation {
                        entry_id,
                        parent_id: current_dir.entry_id,
                        name: entry_name,
                        logical_size,
                        allocated_size: None,
                        file_attributes: find_data.dwFileAttributes,
                        reparse_tag,
                        creation_time_utc_ms: creation_time,
                        last_write_time_utc_ms: last_write_time,
                        last_access_time_utc_ms: last_access_time,
                    })?;
                }
            }

            // Advance to next entry
            // SAFETY: Calling FindNextFileW with active search handle.
            let next_ok = unsafe { FindNextFileW(find_guard.0, &mut find_data) };
            if next_ok == 0 {
                let err = unsafe { GetLastError() };
                if err != ERROR_NO_MORE_FILES && err != ERROR_FILE_NOT_FOUND {
                    coverage_gap_count += 1;
                    writer.write_coverage_gap(&CoverageGapObservation {
                        path: current_dir.display_path.clone(),
                        error_code: err,
                        error_message: format_win32_error(err),
                    })?;
                }
                break;
            }
        }
    }

    let duration_ms = start_time.elapsed().as_millis() as u64;
    let term = TerminalObservation {
        outcome: RunOutcome::Finished,
        total_directories,
        total_files,
        total_logical_bytes,
        total_allocated_bytes,
        coverage_gap_count,
        duration_ms,
    };
    writer.write_terminal(&term)?;
    writer.flush()?;
    Ok(term)
}
