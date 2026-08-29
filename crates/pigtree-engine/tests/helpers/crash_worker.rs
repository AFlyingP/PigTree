//! Test helper executable simulating a crashing worker with truncated stream output.
#![allow(clippy::upper_case_acronyms)]

use pigtree_ipc::win32::{CloseHandle, WriteFile, DWORD, HANDLE};
use std::env;
use std::ffi::c_void;
use std::ptr::null_mut;

#[link(name = "kernel32")]
extern "system" {
    fn ExitProcess(uExitCode: u32) -> !;
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let mut pipe_handle: Option<usize> = None;

    let mut iter = args.into_iter().skip(1);
    while let Some(arg) = iter.next() {
        if arg == "--pipe-handle" {
            if let Some(val) = iter.next() {
                pipe_handle = val.parse::<usize>().ok();
            }
        }
    }

    if let Some(handle_val) = pipe_handle {
        let h_pipe = handle_val as HANDLE;
        // Write incomplete/truncated stream header (only 2 bytes instead of 4 magic + 2 version)
        let partial_header = [0x50u8, 0x54u8];
        let mut written: DWORD = 0;
        unsafe {
            WriteFile(
                h_pipe,
                partial_header.as_ptr() as *const c_void,
                partial_header.len() as DWORD,
                &mut written,
                null_mut(),
            );
            CloseHandle(h_pipe);
        }
    }

    // Exit with non-zero crash exit code
    unsafe {
        ExitProcess(42);
    }
}
