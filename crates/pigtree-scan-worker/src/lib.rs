//! PigTree scan worker library seam and Win32 directory traversal implementation.
#![allow(non_camel_case_types, non_snake_case, non_upper_case_globals)]

#[cfg(not(windows))]
compile_error!("pigtree-scan-worker targets Windows only");

use pigtree_protocol::{
    CoverageGapObservation, DirectoryObservation, FileObservation, ObjectIdentity,
    ObservationWriter, RunOutcome, SpecialObservation, TerminalObservation, ValueKnowledge,
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
pub const ERROR_INVALID_DATA: DWORD = 13;
pub const ERROR_INSUFFICIENT_BUFFER: DWORD = 122;

pub const WAIT_OBJECT_0: DWORD = 0;
pub const WAIT_TIMEOUT: DWORD = 258;
pub const WAIT_FAILED: DWORD = 0xFFFFFFFF;

pub const FindExInfoStandard: i32 = 0;
pub const FindExInfoBasic: i32 = 1;

pub const FindExSearchNameMatch: i32 = 0;
pub const FindExSearchLimitToDirectories: i32 = 1;

pub const FIND_FIRST_EX_LARGE_FETCH: DWORD = 2;
pub const GetFileExInfoStandard: i32 = 0;

// File access / disposition / flag constants for CreateFileW directory handles.
pub const FILE_LIST_DIRECTORY: DWORD = 0x00000001;
pub const FILE_READ_ATTRIBUTES: DWORD = 0x0080;
pub const FILE_SHARE_READ: DWORD = 0x00000001;
pub const FILE_SHARE_WRITE: DWORD = 0x00000002;
pub const FILE_SHARE_DELETE: DWORD = 0x00000004;
pub const OPEN_EXISTING: DWORD = 3;
pub const FILE_FLAG_BACKUP_SEMANTICS: DWORD = 0x02000000;

// FILE_INFO_LEVELS classes for GetFileInformationByHandleEx.
pub const FileStandardInfo: i32 = 1; // FILE_STANDARD_INFO
pub const FileIdBothDirectoryInfo: i32 = 10; // FILE_ID_BOTH_DIR_INFO
pub const FileIdInfo: i32 = 18; // FILE_ID_INFO
pub const FileIdExtdDirectoryInfo: i32 = 19; // FILE_ID_EXTD_DIR_INFO

// Growable directory-enumeration buffer policy.
pub const ENUM_BUFFER_INITIAL_BYTES: usize = 64 * 1024;
pub const ENUM_BUFFER_MAX_BYTES: usize = 16 * 1024 * 1024;

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
    pub fn CreateFileW(
        lpFileName: LPCWSTR,
        dwDesiredAccess: DWORD,
        dwShareMode: DWORD,
        lpSecurityAttributes: *mut c_void,
        dwCreationDisposition: DWORD,
        dwFlagsAndAttributes: DWORD,
        hTemplateFile: HANDLE,
    ) -> HANDLE;
    pub fn GetFileInformationByHandleEx(
        hFile: HANDLE,
        FileInformationClass: i32,
        lpFileInformation: *mut c_void,
        dwBufferSize: DWORD,
    ) -> BOOL;
    pub fn GetVolumePathNameW(
        lpszFileName: LPCWSTR,
        lpszVolumePathName: LPWSTR,
        cchBufferLength: DWORD,
    ) -> BOOL;
    pub fn GetVolumeNameForVolumeMountPointW(
        lpszVolumeMountPoint: LPCWSTR,
        lpszVolumeName: LPWSTR,
        cchBufferLength: DWORD,
    ) -> BOOL;
    #[allow(clippy::too_many_arguments)]
    pub fn GetVolumeInformationW(
        lpRootPathName: LPCWSTR,
        lpVolumeNameBuffer: LPWSTR,
        nVolumeNameSize: DWORD,
        lpVolumeSerialNumber: *mut DWORD,
        lpMaximumComponentLength: *mut DWORD,
        lpFileSystemFlags: *mut DWORD,
        lpFileSystemNameBuffer: LPWSTR,
        nFileSystemNameSize: DWORD,
    ) -> BOOL;
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

/// Convert a raw FILETIME (as i64) to milliseconds since the Unix epoch.
/// Timestamps at or before the epoch clamp to 0, matching `filetime_to_unix_ms`.
fn filetime_i64_to_unix_ms(ft: i64) -> u64 {
    const WINDOWS_EPOCH_DIFFERENCE: i64 = 116_444_736_000_000_000;
    if ft > WINDOWS_EPOCH_DIFFERENCE {
        ((ft - WINDOWS_EPOCH_DIFFERENCE) as u64) / 10_000
    } else {
        0
    }
}

#[allow(dead_code)]
fn read_u16_le(buf: &[u8], off: usize) -> Option<u16> {
    if off + 2 > buf.len() {
        return None;
    }
    Some(u16::from_le_bytes(buf[off..off + 2].try_into().unwrap()))
}

fn read_u32_le(buf: &[u8], off: usize) -> Option<u32> {
    if off + 4 > buf.len() {
        return None;
    }
    Some(u32::from_le_bytes(buf[off..off + 4].try_into().unwrap()))
}

fn read_u64_le(buf: &[u8], off: usize) -> Option<u64> {
    if off + 8 > buf.len() {
        return None;
    }
    Some(u64::from_le_bytes(buf[off..off + 8].try_into().unwrap()))
}

fn read_i64_le(buf: &[u8], off: usize) -> Option<i64> {
    read_u64_le(buf, off).map(|v| v as i64)
}

fn read_u128_le(buf: &[u8], off: usize) -> Option<u128> {
    if off + 16 > buf.len() {
        return None;
    }
    Some(u128::from_le_bytes(buf[off..off + 16].try_into().unwrap()))
}

/// Volume context resolved once per scan: canonical volume identity evidence and
/// the filesystem's hard-link capability class (per ADR 0001, unsupported
/// filesystem features are Not Applicable, never zero).
struct VolumeContext {
    guid: Option<[u8; 16]>,
    hardlink_capable: bool,
}

impl VolumeContext {
    /// Canonical object identity for the given 128-bit filesystem File ID, or
    /// None when the volume GUID is unavailable.
    fn identity(&self, file_id: u128) -> Option<ObjectIdentity> {
        self.guid.map(|guid| ObjectIdentity::new(guid, file_id))
    }

    /// Link-count knowledge for a fresh directory enumeration record: the
    /// default traversal never issues per-file handle queries, so totals stay
    /// NotObserved on hard-link-capable filesystems and NotApplicable where
    /// hard links cannot exist.
    fn link_knowledge(&self) -> ValueKnowledge<u32> {
        if self.hardlink_capable {
            ValueKnowledge::NotObserved
        } else {
            ValueKnowledge::NotApplicable
        }
    }
}

/// Filesystems on which hard links do not exist (per issue #20 / ADR 0001).
/// Unknown filesystem names are treated as hard-link-capable (conservative).
fn is_fat_family(fs_name: &str) -> bool {
    matches!(
        fs_name.to_ascii_lowercase().as_str(),
        "fat" | "fat12" | "fat16" | "fat32" | "exfat" | "cdfs"
    )
}

/// Parse the 32 hex digits of a `{GUID}` volume path into 16 identity bytes.
/// The mapping is opaque but deterministic: hex pairs in string order.
fn parse_volume_guid(volume_path: &str) -> Option<[u8; 16]> {
    let start = volume_path.find('{')? + 1;
    let end = volume_path[start..].find('}')? + start;
    let hex: String = volume_path[start..end]
        .chars()
        .filter(|c| *c != '-')
        .collect();
    if hex.len() != 32 {
        return None;
    }
    let mut guid = [0u8; 16];
    for (i, byte) in guid.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).ok()?;
    }
    Some(guid)
}

/// Resolve the volume context for a scan target. On any resolution failure the
/// context degrades honestly: no identity evidence, conservative link knowledge.
fn resolve_volume_context(extended_target: &str) -> VolumeContext {
    let target_wide = to_wide_null(extended_target);
    let mut volume_path = [0u16; 512];
    // SAFETY: valid wide path pointers with declared buffer lengths.
    let got_volume_path =
        unsafe { GetVolumePathNameW(target_wide.as_ptr(), volume_path.as_mut_ptr(), 512) };
    if got_volume_path == 0 {
        return VolumeContext {
            guid: None,
            hardlink_capable: true,
        };
    }

    let mut volume_name = [0u16; 128];
    // SAFETY: valid wide pointers with declared buffer lengths.
    let got_guid = unsafe {
        GetVolumeNameForVolumeMountPointW(volume_path.as_ptr(), volume_name.as_mut_ptr(), 128)
    };
    let guid = if got_guid != 0 {
        parse_volume_guid(&wide_slice_to_string(&volume_name))
    } else {
        None
    };

    let mut fs_name = [0u16; 64];
    // SAFETY: valid wide pointers; volume name/serial/length outputs unused.
    let got_fs = unsafe {
        GetVolumeInformationW(
            volume_path.as_ptr(),
            null_mut(),
            0,
            null_mut(),
            null_mut(),
            null_mut(),
            fs_name.as_mut_ptr(),
            64,
        )
    };
    let hardlink_capable = if got_fs != 0 {
        !is_fat_family(&wide_slice_to_string(&fs_name))
    } else {
        true
    };

    VolumeContext {
        guid,
        hardlink_capable,
    }
}

/// RAII wrapper closing a raw Win32 handle on drop.
struct RawHandleGuard(HANDLE);

impl Drop for RawHandleGuard {
    fn drop(&mut self) {
        if !self.0.is_null() && self.0 != INVALID_HANDLE_VALUE {
            // SAFETY: Closing a valid kernel handle with CloseHandle.
            unsafe {
                CloseHandle(self.0);
            }
        }
    }
}

/// Open a directory handle for attribute queries and batched enumeration.
/// Requires FILE_LIST_DIRECTORY for enumeration and FILE_READ_ATTRIBUTES for
/// querying FileStandardInfo (sizes) and FileIdInfo (128-bit identity) via
/// GetFileInformationByHandleEx per Microsoft Win32 documentation.
/// If access is denied by permissions, opening fails cleanly and traversal
/// records an honest coverage gap without guessing.
fn open_directory_handle(extended_path: &str) -> Option<HANDLE> {
    let wide = to_wide_null(extended_path);
    // SAFETY: Opening an existing directory with list and read-attributes access.
    let handle = unsafe {
        CreateFileW(
            wide.as_ptr(),
            FILE_LIST_DIRECTORY | FILE_READ_ATTRIBUTES,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            null_mut(),
            OPEN_EXISTING,
            FILE_FLAG_BACKUP_SEMANTICS,
            null_mut(),
        )
    };
    if handle.is_null() || handle == INVALID_HANDLE_VALUE {
        None
    } else {
        Some(handle)
    }
}

/// Directory enumeration strategy for a scan. The class is decided once per scan
/// from the first successfully opened directory handle so that File ID
/// representations stay consistent across the whole observation stream: mixed
/// classes could present the same object under two different IDs and break
/// hard-link alias unification downstream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EnumClass {
    /// Documented batched `FileIdExtdDirectoryInfo` enumeration carrying
    /// 128-bit File IDs and AllocationSize evidence (Windows 10+ NTFS/ReFS).
    Extd,
    /// Legacy `FindFirstFileExW` enumeration without identity or allocation
    /// evidence beyond logical sizes.
    Legacy,
}

/// Probe whether the filesystem backing an open directory handle supports the
/// extended batched directory information class.
fn probe_enum_class(handle: HANDLE) -> EnumClass {
    let mut buf = vec![0u8; 4096];
    // SAFETY: Buffer is allocated and sized correctly for the query.
    let ok = unsafe {
        GetFileInformationByHandleEx(
            handle,
            FileIdExtdDirectoryInfo,
            buf.as_mut_ptr() as *mut c_void,
            buf.len() as DWORD,
        )
    };
    if ok != 0 {
        return EnumClass::Extd;
    }
    let err = unsafe { GetLastError() };
    // A single record larger than the probe buffer or an exhausted/empty enumeration
    // (ERROR_NO_MORE_FILES or ERROR_FILE_NOT_FOUND) proves the class itself is supported.
    if err == ERROR_INSUFFICIENT_BUFFER || err == ERROR_NO_MORE_FILES || err == ERROR_FILE_NOT_FOUND
    {
        return EnumClass::Extd;
    }
    EnumClass::Legacy
}

/// Per-entry facts collected by either enumeration backend before emission.
struct EntryFacts {
    name: String,
    file_attributes: DWORD,
    reparse_tag: DWORD,
    creation_time_utc_ms: u64,
    last_write_time_utc_ms: u64,
    last_access_time_utc_ms: u64,
    logical_size: u64,
    allocated_size: Option<u64>,
    object_id: Option<ObjectIdentity>,
    total_link_count: ValueKnowledge<u32>,
}

fn find_data_to_facts(find_data: &WIN32_FIND_DATAW, volume: &VolumeContext) -> EntryFacts {
    let is_reparse = (find_data.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT) != 0;
    EntryFacts {
        name: wide_slice_to_string(&find_data.cFileName),
        file_attributes: find_data.dwFileAttributes,
        reparse_tag: if is_reparse { find_data.dwReserved0 } else { 0 },
        creation_time_utc_ms: filetime_to_unix_ms(&find_data.ftCreationTime),
        last_write_time_utc_ms: filetime_to_unix_ms(&find_data.ftLastWriteTime),
        last_access_time_utc_ms: filetime_to_unix_ms(&find_data.ftLastAccessTime),
        logical_size: ((find_data.nFileSizeHigh as u64) << 32) | (find_data.nFileSizeLow as u64),
        allocated_size: None,
        object_id: None,
        total_link_count: volume.link_knowledge(),
    }
}

/// Outcome of enumerating one directory with a backend.
enum DirEnumeration {
    /// Directory enumerated (possibly zero entries).
    Success(Vec<EntryFacts>),
    /// The directory could not be opened; carry the Win32 error code.
    OpenFailed { error_code: DWORD },
    /// Entries observed before a mid-enumeration failure; the failure must be
    /// recorded as a coverage gap after emitting the partial facts.
    Partial {
        facts: Vec<EntryFacts>,
        error_code: DWORD,
    },
    /// Cooperative cancellation observed mid-enumeration.
    Cancelled,
}

/// `FILE_ID_EXTD_DIR_INFO` field offsets (x64/any alignment: the struct is a
/// packed C layout with a 1-aligned FILE_ID_128 at offset 72 and the name at 88).
const EXTD_OFF_CREATION: usize = 8;
const EXTD_OFF_LAST_ACCESS: usize = 16;
const EXTD_OFF_LAST_WRITE: usize = 24;
const EXTD_OFF_EOF: usize = 40;
const EXTD_OFF_ALLOC: usize = 48;
const EXTD_OFF_ATTRS: usize = 56;
const EXTD_OFF_NAME_LEN: usize = 60;
const EXTD_OFF_REPARSE_TAG: usize = 68;
const EXTD_OFF_FILE_ID: usize = 72;
const EXTD_OFF_NAME: usize = 88;
const EXTD_MIN_RECORD: usize = 88;

/// Parse one `GetFileInformationByHandleEx(FileIdExtdDirectoryInfo)` buffer into
/// entry facts. Returns None on any structurally malformed record: the caller
/// must then fail the directory closed rather than emit unvalidated data.
fn parse_extd_entries(buf: &[u8], volume: &VolumeContext) -> Option<Vec<EntryFacts>> {
    let mut facts = Vec::new();
    let mut off = 0usize;
    loop {
        // Each record requires at least the fixed header size (up to FileName offset).
        let record_header_end = off.checked_add(EXTD_MIN_RECORD)?;
        if record_header_end > buf.len() {
            // Offset 0 means the buffer itself was empty (clean end); anywhere
            // else the record chain is truncated and the batch is invalid.
            return if off == 0 { Some(facts) } else { None };
        }

        let next = read_u32_le(buf, off)? as usize;
        // Validate NextEntryOffset bounds and alignment when not the terminal record:
        // non-zero next must be at least the header size, 8-byte aligned, and advance
        // within the supplied buffer boundary.
        let record_end = if next != 0 {
            if next < EXTD_MIN_RECORD || !next.is_multiple_of(8) {
                return None;
            }
            let end = off.checked_add(next)?;
            if end > buf.len() {
                return None;
            }
            end
        } else {
            buf.len()
        };

        let attrs = read_u32_le(buf, off + EXTD_OFF_ATTRS)?;
        let is_reparse = (attrs & FILE_ATTRIBUTE_REPARSE_POINT) != 0;
        let reparse_tag = if is_reparse {
            read_u32_le(buf, off + EXTD_OFF_REPARSE_TAG)?
        } else {
            0
        };

        let name_len = read_u32_le(buf, off + EXTD_OFF_NAME_LEN)? as usize;
        // Non-empty, even UTF-16 byte length strictly contained within this record boundary.
        if name_len == 0 || !name_len.is_multiple_of(2) {
            return None;
        }
        let name_start = off.checked_add(EXTD_OFF_NAME)?;
        let name_end = name_start.checked_add(name_len)?;
        if name_end > record_end {
            return None;
        }

        let (chunks, _) = buf[name_start..name_end].as_chunks::<2>();
        let name_u16: Vec<u16> = chunks.iter().map(|c| u16::from_le_bytes(*c)).collect();
        let creation = read_i64_le(buf, off + EXTD_OFF_CREATION)?;
        let last_access = read_i64_le(buf, off + EXTD_OFF_LAST_ACCESS)?;
        let last_write = read_i64_le(buf, off + EXTD_OFF_LAST_WRITE)?;
        let alloc_raw = read_i64_le(buf, off + EXTD_OFF_ALLOC)?;
        let eof_raw = read_i64_le(buf, off + EXTD_OFF_EOF)?;
        let file_id = read_u128_le(buf, off + EXTD_OFF_FILE_ID)?;

        facts.push(EntryFacts {
            name: String::from_utf16_lossy(&name_u16),
            file_attributes: attrs,
            reparse_tag,
            creation_time_utc_ms: filetime_i64_to_unix_ms(creation),
            last_write_time_utc_ms: filetime_i64_to_unix_ms(last_write),
            last_access_time_utc_ms: filetime_i64_to_unix_ms(last_access),
            // Cloud placeholders and sparse/compressed files legitimately report
            // less allocation than logical size (including 0); negative values clamp to 0.
            logical_size: eof_raw.max(0) as u64,
            allocated_size: if alloc_raw >= 0 {
                Some(alloc_raw as u64)
            } else {
                None
            },
            object_id: volume.identity(file_id),
            total_link_count: volume.link_knowledge(),
        });

        if next == 0 {
            return Some(facts);
        }
        off = record_end;
    }
}

/// Enumerate one directory with the documented batched `FileIdExtdDirectoryInfo`
/// class: identity, allocation, sizes, attributes, timestamps, and reparse tags
/// in batched kernel transfers with zero per-file handle opens.
fn enumerate_directory_batched<C: Cancellation>(
    extended_path: &str,
    volume: &VolumeContext,
    cancellation: &C,
) -> DirEnumeration {
    let Some(handle) = open_directory_handle(extended_path) else {
        let error_code = unsafe { GetLastError() };
        return DirEnumeration::OpenFailed { error_code };
    };
    let _guard = RawHandleGuard(handle);

    let mut buf = vec![0u8; ENUM_BUFFER_INITIAL_BYTES];
    let mut facts: Vec<EntryFacts> = Vec::new();
    loop {
        if cancellation.is_cancelled() {
            return DirEnumeration::Cancelled;
        }
        // SAFETY: Buffer is allocated and sized correctly for the query.
        let ok = unsafe {
            GetFileInformationByHandleEx(
                handle,
                FileIdExtdDirectoryInfo,
                buf.as_mut_ptr() as *mut c_void,
                buf.len() as DWORD,
            )
        };
        if ok == 0 {
            let err = unsafe { GetLastError() };
            match err {
                ERROR_INSUFFICIENT_BUFFER if buf.len() < ENUM_BUFFER_MAX_BYTES => {
                    buf.resize(buf.len() * 2, 0);
                    continue;
                }
                ERROR_NO_MORE_FILES | ERROR_FILE_NOT_FOUND => {
                    return DirEnumeration::Success(facts)
                }
                other => {
                    return DirEnumeration::Partial {
                        facts,
                        error_code: other,
                    }
                }
            }
        }
        match parse_extd_entries(&buf, volume) {
            Some(batch) => {
                let parsed_empty = batch.is_empty();
                facts.extend(batch);
                if parsed_empty {
                    // Defensive guard against a malformed empty success buffer.
                    return DirEnumeration::Success(facts);
                }
            }
            None => {
                return DirEnumeration::Partial {
                    facts,
                    error_code: ERROR_INVALID_DATA,
                }
            }
        }
    }
}

/// Enumerate one directory with the legacy `FindFirstFileExW` backend, preserving
/// the exact pre-existing traversal semantics without identity/allocation evidence.
fn enumerate_directory_legacy<C: Cancellation>(
    extended_path: &str,
    volume: &VolumeContext,
    cancellation: &C,
) -> DirEnumeration {
    let pattern = if extended_path.ends_with('\\') {
        format!("{}*", extended_path)
    } else {
        format!("{}\\*", extended_path)
    };
    let pattern_w = to_wide_null(&pattern);

    let mut find_data: WIN32_FIND_DATAW = unsafe { std::mem::zeroed() };
    // SAFETY: Calling FindFirstFileExW to enumerate entries within the directory.
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
        let error_code = unsafe { GetLastError() };
        if error_code == ERROR_FILE_NOT_FOUND || error_code == ERROR_NO_MORE_FILES {
            // Empty directory: normal completion for this folder.
            return DirEnumeration::Success(Vec::new());
        }
        return DirEnumeration::OpenFailed { error_code };
    }
    let find_guard = FindHandle(h_find);

    let mut facts: Vec<EntryFacts> = Vec::new();
    loop {
        if cancellation.is_cancelled() {
            return DirEnumeration::Cancelled;
        }
        let name = wide_slice_to_string(&find_data.cFileName);
        if name != "." && name != ".." {
            facts.push(find_data_to_facts(&find_data, volume));
        }
        // SAFETY: Advancing the active search handle.
        let next_ok = unsafe { FindNextFileW(find_guard.0, &mut find_data) };
        if next_ok == 0 {
            let err = unsafe { GetLastError() };
            if err != ERROR_NO_MORE_FILES && err != ERROR_FILE_NOT_FOUND {
                return DirEnumeration::Partial {
                    facts,
                    error_code: err,
                };
            }
            return DirEnumeration::Success(facts);
        }
    }
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
    let mut total_allocated_bytes: u64 = 0;
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

    // Resolve volume context once: canonical identity evidence and hard-link
    // capability classification for this scan target's volume.
    let volume = resolve_volume_context(&extended_target);

    // Root directory evidence from a single handle: allocation, link count, and
    // File ID identity (only for non-reparse roots; a reparse root's handle
    // would resolve to the link target, not the observed entry). The same
    // handle probes the per-scan enumeration class.
    let mut root_object_id: Option<ObjectIdentity> = None;
    let mut root_allocated_size: Option<u64> = None;
    let root_total_link_count = ValueKnowledge::NotApplicable;
    let mut enum_class: Option<EnumClass> = None;

    if root_reparse_tag == 0 {
        if let Some(handle) = open_directory_handle(&extended_target) {
            let _guard = RawHandleGuard(handle);

            enum_class = Some(probe_enum_class(handle));

            // FILE_STANDARD_INFO: AllocationSize (i64 @ 0), NumberOfLinks (u32 @ 16).
            let mut std_buf = [0u8; 32];
            // SAFETY: Stack buffer sized to FILE_STANDARD_INFO.
            if unsafe {
                GetFileInformationByHandleEx(
                    handle,
                    FileStandardInfo,
                    std_buf.as_mut_ptr() as *mut c_void,
                    std_buf.len() as DWORD,
                )
            } != 0
            {
                let alloc = read_i64_le(&std_buf, 0).unwrap_or(-1);
                if alloc >= 0 {
                    root_allocated_size = Some(alloc as u64);
                }
            }

            // FILE_ID_INFO: 64-bit VolumeSerialNumber (@ 0), 128-bit FileId (@ 8).
            if volume.hardlink_capable && volume.guid.is_some() {
                let mut id_buf = [0u8; 24];
                // SAFETY: Stack buffer sized to FILE_ID_INFO.
                if unsafe {
                    GetFileInformationByHandleEx(
                        handle,
                        FileIdInfo,
                        id_buf.as_mut_ptr() as *mut c_void,
                        id_buf.len() as DWORD,
                    )
                } != 0
                {
                    if let Some(file_id) = read_u128_le(&id_buf, 8) {
                        root_object_id = volume.identity(file_id);
                    }
                }
            }
        }
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
        object_id: root_object_id,
        allocated_size: root_allocated_size,
        total_link_count: root_total_link_count,
    })?;
    if let Some(alloc) = root_allocated_size {
        total_allocated_bytes += alloc;
    }

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

        let class = match enum_class {
            Some(class) => class,
            None => {
                // The root handle never resolved a class (reparse root or open
                // failure); probe lazily on the first successfully opened child.
                match open_directory_handle(&current_dir.extended_path) {
                    Some(handle) => {
                        let class = {
                            let _guard = RawHandleGuard(handle);
                            probe_enum_class(handle)
                        };
                        enum_class = Some(class);
                        class
                    }
                    None => EnumClass::Legacy,
                }
            }
        };

        let enumeration = match class {
            EnumClass::Extd => {
                enumerate_directory_batched(&current_dir.extended_path, &volume, cancellation)
            }
            EnumClass::Legacy => {
                enumerate_directory_legacy(&current_dir.extended_path, &volume, cancellation)
            }
        };

        let facts = match enumeration {
            DirEnumeration::Cancelled => {
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
            DirEnumeration::OpenFailed { error_code } => {
                if error_code != ERROR_FILE_NOT_FOUND && error_code != ERROR_NO_MORE_FILES {
                    // Access denied or enumeration error: record coverage gap
                    // and continue with accessible sibling folders.
                    coverage_gap_count += 1;
                    writer.write_coverage_gap(&CoverageGapObservation {
                        path: current_dir.display_path.clone(),
                        error_code,
                        error_message: format_win32_error(error_code),
                    })?;
                }
                continue;
            }
            DirEnumeration::Partial { facts, error_code } => {
                // Entries observed before the failure remain valid; the failure
                // itself is recorded as a coverage gap for this directory.
                coverage_gap_count += 1;
                writer.write_coverage_gap(&CoverageGapObservation {
                    path: current_dir.display_path.clone(),
                    error_code,
                    error_message: format_win32_error(error_code),
                })?;
                facts
            }
            DirEnumeration::Success(facts) => facts,
        };

        for fact in facts {
            if fact.name == "." || fact.name == ".." {
                continue;
            }

            let entry_id = next_entry_id;
            next_entry_id += 1;

            let is_dir = (fact.file_attributes & FILE_ATTRIBUTE_DIRECTORY) != 0;
            let is_reparse = (fact.file_attributes & FILE_ATTRIBUTE_REPARSE_POINT) != 0;
            let is_device = (fact.file_attributes & FILE_ATTRIBUTE_DEVICE) != 0;

            if is_device {
                writer.write_special(&SpecialObservation {
                    entry_id,
                    parent_id: current_dir.entry_id,
                    name: fact.name,
                    file_attributes: fact.file_attributes,
                    reparse_tag: fact.reparse_tag,
                    creation_time_utc_ms: fact.creation_time_utc_ms,
                    last_write_time_utc_ms: fact.last_write_time_utc_ms,
                    last_access_time_utc_ms: fact.last_access_time_utc_ms,
                    object_id: fact.object_id,
                })?;
            } else if is_dir {
                total_directories += 1;
                writer.write_directory(&DirectoryObservation {
                    entry_id,
                    parent_id: current_dir.entry_id,
                    name: fact.name.clone(),
                    file_attributes: fact.file_attributes,
                    reparse_tag: fact.reparse_tag,
                    creation_time_utc_ms: fact.creation_time_utc_ms,
                    last_write_time_utc_ms: fact.last_write_time_utc_ms,
                    last_access_time_utc_ms: fact.last_access_time_utc_ms,
                    object_id: fact.object_id,
                    allocated_size: fact.allocated_size,
                    total_link_count: ValueKnowledge::NotApplicable,
                })?;
                if let Some(alloc) = fact.allocated_size {
                    total_allocated_bytes += alloc;
                }

                // Only descend into non-reparse subdirectories
                if !is_reparse {
                    let child_ext = if current_dir.extended_path.ends_with('\\') {
                        format!("{}{}", current_dir.extended_path, fact.name)
                    } else {
                        format!("{}\\{}", current_dir.extended_path, fact.name)
                    };
                    let child_disp = if current_dir.display_path.ends_with('\\') {
                        format!("{}{}", current_dir.display_path, fact.name)
                    } else {
                        format!("{}\\{}", current_dir.display_path, fact.name)
                    };
                    stack.push(DirectoryStackItem {
                        entry_id,
                        extended_path: child_ext,
                        display_path: child_disp,
                    });
                }
            } else {
                total_files += 1;
                total_logical_bytes += fact.logical_size;

                writer.write_file(&FileObservation {
                    entry_id,
                    parent_id: current_dir.entry_id,
                    name: fact.name,
                    logical_size: fact.logical_size,
                    allocated_size: fact.allocated_size,
                    file_attributes: fact.file_attributes,
                    reparse_tag: fact.reparse_tag,
                    creation_time_utc_ms: fact.creation_time_utc_ms,
                    last_write_time_utc_ms: fact.last_write_time_utc_ms,
                    last_access_time_utc_ms: fact.last_access_time_utc_ms,
                    object_id: fact.object_id,
                    total_link_count: fact.total_link_count,
                })?;
                if let Some(alloc) = fact.allocated_size {
                    total_allocated_bytes += alloc;
                }
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
