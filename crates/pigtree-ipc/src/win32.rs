//! Win32 FFI bindings for process management, security, Named Pipes, and Job Objects.
#![allow(non_camel_case_types, non_snake_case, dead_code)]

#[cfg(not(target_arch = "x86_64"))]
compile_error!("PigTree targets 64-bit Windows (x86_64) only");

use std::ffi::c_void;
use std::ptr::null_mut;

pub type HANDLE = *mut c_void;
pub type BOOL = i32;
pub type DWORD = u32;
pub type ULONG = u32;
pub type SIZE_T = usize;
pub type LPWSTR = *mut u16;
pub type LPCWSTR = *const u16;

pub const TRUE: BOOL = 1;
pub const FALSE: BOOL = 0;
pub const INVALID_HANDLE_VALUE: HANDLE = -1isize as HANDLE;

// Job Object Limit Flags
pub const JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE: DWORD = 0x00002000;
pub const JOB_OBJECT_EXTENDED_LIMIT_INFORMATION: u32 = 9;

// Pipe flags
pub const PIPE_ACCESS_DUPLEX: DWORD = 0x00000003;
pub const FILE_FLAG_FIRST_PIPE_INSTANCE: DWORD = 0x00080000;
pub const PIPE_TYPE_BYTE: DWORD = 0x00000000;
pub const PIPE_READMODE_BYTE: DWORD = 0x00000000;
pub const PIPE_WAIT: DWORD = 0x00000000;
pub const PIPE_REJECT_REMOTE_CLIENTS: DWORD = 0x00000008;

// File access, attributes & sharing
pub const GENERIC_READ: DWORD = 0x80000000;
pub const GENERIC_WRITE: DWORD = 0x40000000;
pub const OPEN_EXISTING: DWORD = 3;
pub const FILE_ATTRIBUTE_NORMAL: DWORD = 0x00000080;
pub const FILE_FLAG_OVERLAPPED: DWORD = 0x40000000;
pub const SECURITY_SQOS_PRESENT: DWORD = 0x00100000;
pub const SECURITY_IDENTIFICATION: DWORD = 0x00010000;

// Drive types
pub const DRIVE_UNKNOWN: DWORD = 0;
pub const DRIVE_NO_ROOT_DIR: DWORD = 1;
pub const DRIVE_REMOVABLE: DWORD = 2;
pub const DRIVE_FIXED: DWORD = 3;
pub const DRIVE_REMOTE: DWORD = 4;
pub const DRIVE_CDROM: DWORD = 5;
pub const DRIVE_RAMDISK: DWORD = 6;

// Win32 Error codes
pub const ERROR_FILE_NOT_FOUND: DWORD = 2;
pub const ERROR_INVALID_HANDLE: DWORD = 6;
pub const ERROR_HANDLE_EOF: DWORD = 38;
pub const ERROR_BROKEN_PIPE: DWORD = 109;
pub const ERROR_PIPE_BUSY: DWORD = 231;
pub const ERROR_PIPE_NOT_CONNECTED: DWORD = 233;
pub const ERROR_PIPE_CONNECTED: DWORD = 535;
pub const ERROR_OPERATION_ABORTED: DWORD = 995;
pub const ERROR_IO_PENDING: DWORD = 997;

// Wait constants
pub const WAIT_OBJECT_0: DWORD = 0;
pub const WAIT_TIMEOUT: DWORD = 258;
pub const WAIT_FAILED: DWORD = 0xFFFFFFFF;
pub const INFINITE: DWORD = 0xFFFFFFFF;

// Console control events
pub const CTRL_C_EVENT: DWORD = 0;
pub const CTRL_BREAK_EVENT: DWORD = 1;

// Handle inheritance
pub const HANDLE_FLAG_INHERIT: DWORD = 0x00000001;

// Process creation flags
pub const EXTENDED_STARTUPINFO_PRESENT: DWORD = 0x00080000;
pub const CREATE_NO_WINDOW: DWORD = 0x08000000;
pub const CREATE_SUSPENDED: DWORD = 0x00000004;
pub const CREATE_NEW_PROCESS_GROUP: DWORD = 0x00000200;

// Proc Thread Attributes
pub const PROC_THREAD_ATTRIBUTE_HANDLE_LIST: usize = 0x00020002;

// Token Information
pub const TOKEN_USER: u32 = 1;
pub const TOKEN_RESTRICTED_SIDS: u32 = 11;
pub const TOKEN_QUERY: DWORD = 0x0008;

// BCrypt flags
pub const BCRYPT_USE_SYSTEM_PREFERRED_RNG: DWORD = 0x00000002;

#[repr(C)]
pub struct OVERLAPPED {
    pub Internal: usize,
    pub InternalHigh: usize,
    pub Offset: DWORD,
    pub OffsetHigh: DWORD,
    pub hEvent: HANDLE,
}

#[repr(C)]
pub struct SECURITY_ATTRIBUTES {
    pub nLength: DWORD,
    pub lpSecurityDescriptor: *mut c_void,
    pub bInheritHandle: BOOL,
}

#[repr(C)]
pub struct STARTUPINFOW {
    pub cb: DWORD,
    pub lpReserved: LPWSTR,
    pub lpDesktop: LPWSTR,
    pub lpTitle: LPWSTR,
    pub dwX: DWORD,
    pub dwY: DWORD,
    pub dwXSize: DWORD,
    pub dwYSize: DWORD,
    pub dwXCountChars: DWORD,
    pub dwYCountChars: DWORD,
    pub dwFillAttribute: DWORD,
    pub dwFlags: DWORD,
    pub wShowWindow: u16,
    pub cbReserved2: u16,
    pub lpReserved2: *mut u8,
    pub hStdInput: HANDLE,
    pub hStdOutput: HANDLE,
    pub hStdError: HANDLE,
}

#[repr(C)]
pub struct STARTUPINFOEXW {
    pub StartupInfo: STARTUPINFOW,
    pub lpAttributeList: *mut c_void,
}

#[repr(C)]
pub struct PROCESS_INFORMATION {
    pub hProcess: HANDLE,
    pub hThread: HANDLE,
    pub dwProcessId: DWORD,
    pub dwThreadId: DWORD,
}

#[repr(C)]
pub struct IO_COUNTERS {
    pub ReadOperationCount: u64,
    pub WriteOperationCount: u64,
    pub OtherOperationCount: u64,
    pub ReadTransferCount: u64,
    pub WriteTransferCount: u64,
    pub OtherTransferCount: u64,
}

#[repr(C)]
pub struct JOBOBJECT_BASIC_LIMIT_INFORMATION {
    pub PerProcessUserTimeLimit: i64,
    pub PerJobUserTimeLimit: i64,
    pub LimitFlags: DWORD,
    pub MinimumWorkingSetSize: SIZE_T,
    pub MaximumWorkingSetSize: SIZE_T,
    pub ActiveProcessLimit: DWORD,
    pub Affinity: usize,
    pub PriorityClass: DWORD,
    pub SchedulingClass: DWORD,
}

#[repr(C)]
pub struct JOBOBJECT_EXTENDED_LIMIT_INFORMATION {
    pub BasicLimitInformation: JOBOBJECT_BASIC_LIMIT_INFORMATION,
    pub IoInfo: IO_COUNTERS,
    pub ProcessMemoryLimit: SIZE_T,
    pub JobMemoryLimit: SIZE_T,
    pub PeakProcessMemoryLimit: SIZE_T,
    pub PeakJobMemoryLimit: SIZE_T,
}

#[repr(C)]
pub struct FILETIME {
    pub dwLowDateTime: DWORD,
    pub dwHighDateTime: DWORD,
}

impl FILETIME {
    pub fn to_u64(&self) -> u64 {
        ((self.dwHighDateTime as u64) << 32) | (self.dwLowDateTime as u64)
    }
}

#[repr(C)]
pub struct PROCESS_MEMORY_COUNTERS {
    pub cb: DWORD,
    pub PageFaultCount: DWORD,
    pub PeakWorkingSetSize: SIZE_T,
    pub WorkingSetSize: SIZE_T,
    pub QuotaPeakPagedPoolUsage: SIZE_T,
    pub QuotaPagedPoolUsage: SIZE_T,
    pub QuotaPeakNonPagedPoolUsage: SIZE_T,
    pub QuotaNonPagedPoolUsage: SIZE_T,
    pub PagefileUsage: SIZE_T,
    pub PeakPagefileUsage: SIZE_T,
}

#[repr(C)]
pub struct SID_AND_ATTRIBUTES {
    pub Sid: *mut c_void,
    pub Attributes: DWORD,
}

#[repr(C)]
pub struct TOKEN_USER {
    pub User: SID_AND_ATTRIBUTES,
}

#[repr(C)]
pub struct TOKEN_GROUPS {
    pub GroupCount: DWORD,
    pub Groups: [SID_AND_ATTRIBUTES; 1],
}

#[link(name = "kernel32")]
extern "system" {
    pub fn GetLastError() -> DWORD;
    pub fn CloseHandle(hObject: HANDLE) -> BOOL;
    pub fn GetCurrentProcess() -> HANDLE;
    pub fn GetCurrentProcessId() -> DWORD;

    pub fn FormatMessageW(
        dwFlags: DWORD,
        lpSource: *const c_void,
        dwMessageId: DWORD,
        dwLanguageId: DWORD,
        lpBuffer: LPWSTR,
        nSize: DWORD,
        Arguments: *mut c_void,
    ) -> DWORD;

    pub fn CreateEventW(
        lpEventAttributes: *mut SECURITY_ATTRIBUTES,
        bManualReset: BOOL,
        bInitialState: BOOL,
        lpName: LPCWSTR,
    ) -> HANDLE;
    pub fn SetEvent(hEvent: HANDLE) -> BOOL;
    pub fn ResetEvent(hEvent: HANDLE) -> BOOL;

    pub fn WaitForSingleObject(hHandle: HANDLE, dwMilliseconds: DWORD) -> DWORD;
    pub fn WaitForMultipleObjects(
        nCount: DWORD,
        lpHandles: *const HANDLE,
        bWaitAll: BOOL,
        dwMilliseconds: DWORD,
    ) -> DWORD;

    pub fn CreateJobObjectW(lpJobAttributes: *mut SECURITY_ATTRIBUTES, lpName: LPCWSTR) -> HANDLE;

    pub fn SetInformationJobObject(
        hJob: HANDLE,
        JobObjectInformationClass: u32,
        lpJobObjectInformation: *const c_void,
        cbJobObjectInformationLength: DWORD,
    ) -> BOOL;

    pub fn AssignProcessToJobObject(hJob: HANDLE, hProcess: HANDLE) -> BOOL;

    pub fn CreateNamedPipeW(
        lpName: LPCWSTR,
        dwOpenMode: DWORD,
        dwPipeMode: DWORD,
        nMaxInstances: DWORD,
        nOutBufferSize: DWORD,
        nInBufferSize: DWORD,
        nDefaultTimeOut: DWORD,
        lpSecurityAttributes: *mut SECURITY_ATTRIBUTES,
    ) -> HANDLE;

    pub fn ConnectNamedPipe(hNamedPipe: HANDLE, lpOverlapped: *mut c_void) -> BOOL;
    pub fn DisconnectNamedPipe(hNamedPipe: HANDLE) -> BOOL;
    pub fn WaitNamedPipeW(lpNamedPipeName: LPCWSTR, nTimeOut: DWORD) -> BOOL;
    pub fn SetNamedPipeHandleState(
        hNamedPipe: HANDLE,
        lpMode: *const DWORD,
        lpMaxCollectionCount: *const DWORD,
        lpCollectDataTimeout: *const DWORD,
    ) -> BOOL;

    pub fn GetNamedPipeClientProcessId(Pipe: HANDLE, ClientProcessId: *mut ULONG) -> BOOL;
    pub fn GetNamedPipeServerProcessId(Pipe: HANDLE, ServerProcessId: *mut ULONG) -> BOOL;
    pub fn GetNamedPipeClientSessionId(Pipe: HANDLE, ClientSessionId: *mut ULONG) -> BOOL;
    pub fn GetNamedPipeServerSessionId(Pipe: HANDLE, ServerSessionId: *mut ULONG) -> BOOL;

    pub fn PeekNamedPipe(
        hNamedPipe: HANDLE,
        lpBuffer: *mut c_void,
        nBufferSize: DWORD,
        lpBytesRead: *mut DWORD,
        lpTotalBytesAvail: *mut DWORD,
        lpBytesLeftThisMessage: *mut DWORD,
    ) -> BOOL;

    pub fn CreateFileW(
        lpFileName: LPCWSTR,
        dwDesiredAccess: DWORD,
        dwShareMode: DWORD,
        lpSecurityAttributes: *mut SECURITY_ATTRIBUTES,
        dwCreationDisposition: DWORD,
        dwFlagsAndAttributes: DWORD,
        hTemplateFile: HANDLE,
    ) -> HANDLE;

    pub fn ReadFile(
        hFile: HANDLE,
        lpBuffer: *mut c_void,
        nNumberOfBytesToRead: DWORD,
        lpNumberOfBytesRead: *mut DWORD,
        lpOverlapped: *mut OVERLAPPED,
    ) -> BOOL;

    pub fn WriteFile(
        hFile: HANDLE,
        lpBuffer: *const c_void,
        nNumberOfBytesToWrite: DWORD,
        lpNumberOfBytesWritten: *mut DWORD,
        lpOverlapped: *mut OVERLAPPED,
    ) -> BOOL;

    pub fn FlushFileBuffers(hFile: HANDLE) -> BOOL;

    pub fn GetVolumePathNameW(
        lpszFileName: LPCWSTR,
        lpszVolumePathName: LPWSTR,
        cchBufferLength: DWORD,
    ) -> BOOL;

    pub fn GetDriveTypeW(lpRootPathName: LPCWSTR) -> DWORD;

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

    pub fn GetOverlappedResult(
        hFile: HANDLE,
        lpOverlapped: *mut OVERLAPPED,
        lpNumberOfBytesTransferred: *mut DWORD,
        bWait: BOOL,
    ) -> BOOL;

    pub fn CancelIoEx(hFile: HANDLE, lpOverlapped: *mut OVERLAPPED) -> BOOL;

    pub fn CreatePipe(
        hReadPipe: *mut HANDLE,
        hWritePipe: *mut HANDLE,
        lpPipeAttributes: *mut SECURITY_ATTRIBUTES,
        nSize: DWORD,
    ) -> BOOL;

    pub fn SetHandleInformation(hObject: HANDLE, dwMask: DWORD, dwFlags: DWORD) -> BOOL;

    pub fn InitializeProcThreadAttributeList(
        lpAttributeList: *mut c_void,
        dwAttributeCount: DWORD,
        dwFlags: DWORD,
        lpSize: *mut SIZE_T,
    ) -> BOOL;

    pub fn UpdateProcThreadAttribute(
        lpAttributeList: *mut c_void,
        dwFlags: DWORD,
        Attribute: usize,
        lpValue: *mut c_void,
        cbSize: SIZE_T,
        lpPreviousValue: *mut c_void,
        lpReturnSize: *mut SIZE_T,
    ) -> BOOL;

    pub fn DeleteProcThreadAttributeList(lpAttributeList: *mut c_void);

    pub fn CreateProcessW(
        lpApplicationName: LPCWSTR,
        lpCommandLine: LPWSTR,
        lpProcessAttributes: *mut SECURITY_ATTRIBUTES,
        lpThreadAttributes: *mut SECURITY_ATTRIBUTES,
        bInheritHandles: BOOL,
        dwCreationFlags: DWORD,
        lpEnvironment: *mut c_void,
        lpCurrentDirectory: LPCWSTR,
        lpStartupInfo: *mut STARTUPINFOW,
        lpProcessInformation: *mut PROCESS_INFORMATION,
    ) -> BOOL;

    pub fn GetProcessTimes(
        hProcess: HANDLE,
        lpCreationTime: *mut FILETIME,
        lpExitTime: *mut FILETIME,
        lpKernelTime: *mut FILETIME,
        lpUserTime: *mut FILETIME,
    ) -> BOOL;

    pub fn GetProcessHandleCount(hProcess: HANDLE, pdwHandleCount: *mut DWORD) -> BOOL;
    pub fn LocalFree(hMem: *mut c_void) -> *mut c_void;
    pub fn TerminateProcess(hProcess: HANDLE, uExitCode: u32) -> BOOL;
    pub fn ResumeThread(hThread: HANDLE) -> DWORD;
    pub fn GetExitCodeProcess(hProcess: HANDLE, lpExitCode: *mut DWORD) -> BOOL;

    pub fn GenerateConsoleCtrlEvent(dwCtrlEvent: DWORD, dwProcessGroupId: DWORD) -> BOOL;
    pub fn SetConsoleCtrlHandler(
        HandlerRoutine: Option<unsafe extern "system" fn(DWORD) -> BOOL>,
        Add: BOOL,
    ) -> BOOL;
}

#[link(name = "advapi32")]
extern "system" {
    pub fn ConvertStringSecurityDescriptorToSecurityDescriptorW(
        StringSecurityDescriptor: LPCWSTR,
        StringSDRevision: DWORD,
        SecurityDescriptor: *mut *mut c_void,
        SecurityDescriptorSize: *mut ULONG,
    ) -> BOOL;

    pub fn OpenProcessToken(
        ProcessHandle: HANDLE,
        DesiredAccess: DWORD,
        TokenHandle: *mut HANDLE,
    ) -> BOOL;

    pub fn GetTokenInformation(
        TokenHandle: HANDLE,
        TokenInformationClass: u32,
        TokenInformation: *mut c_void,
        TokenInformationLength: DWORD,
        ReturnLength: *mut DWORD,
    ) -> BOOL;

    pub fn ConvertSidToStringSidW(Sid: *mut c_void, StringSid: *mut LPWSTR) -> BOOL;
}

#[link(name = "psapi")]
extern "system" {
    pub fn K32GetProcessMemoryInfo(
        Process: HANDLE,
        ppsmemCounters: *mut PROCESS_MEMORY_COUNTERS,
        cb: DWORD,
    ) -> BOOL;
}

#[link(name = "bcrypt")]
extern "system" {
    pub fn BCryptGenRandom(
        hAlgorithm: usize,
        pbBuffer: *mut u8,
        cbBuffer: u32,
        dwFlags: u32,
    ) -> i32;
}
// Process query access
pub const PROCESS_QUERY_INFORMATION: DWORD = 0x0400;
pub const PROCESS_VM_READ: DWORD = 0x0010;

#[repr(C)]
pub struct UNICODE_STRING {
    pub Length: u16,
    pub MaximumLength: u16,
    pub Buffer: *mut u16,
}

#[repr(C)]
pub struct PROCESS_BASIC_INFORMATION {
    pub ExitStatus: u32,
    pub PebBaseAddress: *mut c_void,
    pub AffinityMask: usize,
    pub BasePriority: i32,
    pub UniqueProcessId: usize,
    pub InheritedFromUniqueProcessId: usize,
}

#[link(name = "ntdll")]
extern "system" {
    pub fn NtQueryInformationProcess(
        ProcessHandle: HANDLE,
        ProcessInformationClass: u32,
        ProcessInformation: *mut c_void,
        ProcessInformationLength: u32,
        ReturnLength: *mut u32,
    ) -> i32;
}

#[link(name = "kernel32")]
extern "system" {
    pub fn ReadProcessMemory(
        hProcess: HANDLE,
        lpBaseAddress: *const c_void,
        lpBuffer: *mut c_void,
        nSize: SIZE_T,
        lpNumberOfBytesRead: *mut SIZE_T,
    ) -> BOOL;

    pub fn OpenProcess(dwDesiredAccess: DWORD, bInheritHandle: BOOL, dwProcessId: DWORD) -> HANDLE;
}

/// Helper to query process command line given a process ID.
pub fn get_process_command_line(pid: u32) -> Result<String, crate::error::IpcError> {
    unsafe {
        let h_proc = OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, FALSE, pid);
        if h_proc.is_null() || h_proc == INVALID_HANDLE_VALUE {
            return Err(crate::error::IpcError::Win32 {
                code: GetLastError(),
                message: format!("OpenProcess failed for PID {pid}"),
            });
        }

        struct HandleGuard(HANDLE);
        impl Drop for HandleGuard {
            fn drop(&mut self) {
                unsafe {
                    CloseHandle(self.0);
                }
            }
        }
        let _guard = HandleGuard(h_proc);

        let mut pbi: PROCESS_BASIC_INFORMATION = std::mem::zeroed();
        let mut ret_len: u32 = 0;
        let status = NtQueryInformationProcess(
            h_proc,
            0,
            &mut pbi as *mut _ as *mut c_void,
            std::mem::size_of::<PROCESS_BASIC_INFORMATION>() as u32,
            &mut ret_len,
        );
        if status != 0 {
            return Err(crate::error::IpcError::Win32 {
                code: status as u32,
                message: "NtQueryInformationProcess failed".to_string(),
            });
        }

        if pbi.PebBaseAddress.is_null() {
            return Ok(String::new());
        }

        // Read ProcessParameters pointer at offset 0x20 in x64 PEB
        let peb_addr = pbi.PebBaseAddress as usize;
        let mut proc_params_ptr: usize = 0;
        let mut read: SIZE_T = 0;
        if ReadProcessMemory(
            h_proc,
            (peb_addr + 0x20) as *const c_void,
            &mut proc_params_ptr as *mut _ as *mut c_void,
            std::mem::size_of::<usize>(),
            &mut read,
        ) == 0
        {
            return Err(crate::error::IpcError::Win32 {
                code: GetLastError(),
                message: "ReadProcessMemory for PEB ProcessParameters failed".to_string(),
            });
        }

        if proc_params_ptr == 0 {
            return Ok(String::new());
        }

        // Read CommandLine UNICODE_STRING at offset 0x70 in x64 RTL_USER_PROCESS_PARAMETERS
        let mut cmd_str: UNICODE_STRING = std::mem::zeroed();
        if ReadProcessMemory(
            h_proc,
            (proc_params_ptr + 0x70) as *const c_void,
            &mut cmd_str as *mut _ as *mut c_void,
            std::mem::size_of::<UNICODE_STRING>(),
            &mut read,
        ) == 0
        {
            return Err(crate::error::IpcError::Win32 {
                code: GetLastError(),
                message: "ReadProcessMemory for CommandLine UNICODE_STRING failed".to_string(),
            });
        }

        if cmd_str.Length == 0 || cmd_str.Buffer.is_null() {
            return Ok(String::new());
        }

        let char_count = (cmd_str.Length / 2) as usize;
        let mut wide_buf: Vec<u16> = vec![0; char_count];
        if ReadProcessMemory(
            h_proc,
            cmd_str.Buffer as *const c_void,
            wide_buf.as_mut_ptr() as *mut c_void,
            cmd_str.Length as SIZE_T,
            &mut read,
        ) == 0
        {
            return Err(crate::error::IpcError::Win32 {
                code: GetLastError(),
                message: "ReadProcessMemory for CommandLine buffer failed".to_string(),
            });
        }

        Ok(String::from_utf16_lossy(&wide_buf))
    }
}

/// Helper to format a Win32 system error code into a human-readable message.
pub fn format_win32_error(code: u32) -> String {
    const FORMAT_MESSAGE_FROM_SYSTEM: DWORD = 0x00001000;
    const FORMAT_MESSAGE_IGNORE_INSERTS: DWORD = 0x00000200;
    let mut buf = [0u16; 512];
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
