//! Integration tests for pigtree-scan-worker public seam, Win32 traversal, and streaming protocol.

use pigtree_protocol::{
    DirectoryObservation, FileObservation, ObservationReader, ObservationRecord, ObservationWriter,
    RunOutcome, TerminalObservation,
};
use pigtree_scan_worker::{
    filetime_to_unix_ms, parse_worker_args, scan_directory, ArgsError, Cancellation, CreateEventW,
    NoCancellation, PipeWriter, Win32EventCancellation, WorkerArgs, FILETIME, INVALID_HANDLE_VALUE,
    TRUE,
};
use std::fs;
use std::io::Cursor;
use std::os::windows::fs::symlink_dir;
use std::path::{Path, PathBuf};
use std::ptr::null_mut;

struct TempTestDir {
    path: PathBuf,
}

impl TempTestDir {
    fn new(name: &str) -> Self {
        let unique = format!(
            "pigtree_test_{name}_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let path = std::env::temp_dir().join(unique);
        fs::create_dir_all(&path).expect("failed to create temp test directory");
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempTestDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

#[test]
fn test_scan_temp_hierarchy_and_decode_stream() {
    let temp = TempTestDir::new("hierarchy");
    let root = temp.path();

    // Setup hierarchy:
    // root/
    //   file1.txt (5 bytes)
    //   file2.bin (1000 bytes)
    //   subA/
    //     subfileA1.txt (12 bytes)
    //     subsub/
    //       deep.txt (7 bytes)
    //   subB/
    //     (empty)

    fs::write(root.join("file1.txt"), b"12345").unwrap();
    fs::write(root.join("file2.bin"), vec![0xAB; 1000]).unwrap();

    let sub_a = root.join("subA");
    fs::create_dir_all(&sub_a).unwrap();
    fs::write(sub_a.join("subfileA1.txt"), b"Hello World!").unwrap();

    let subsub = sub_a.join("subsub");
    fs::create_dir_all(&subsub).unwrap();
    fs::write(subsub.join("deep.txt"), b"deep123").unwrap();

    let sub_b = root.join("subB");
    fs::create_dir_all(&sub_b).unwrap();

    let mut stream_buf = Vec::new();
    let mut writer = ObservationWriter::new(&mut stream_buf, root.to_str().unwrap()).unwrap();
    let term =
        scan_directory(root, &mut writer, &NoCancellation).expect("scan_directory should succeed");

    assert_eq!(term.outcome, RunOutcome::Finished);
    assert_eq!(term.total_directories, 4); // root, subA, subsub, subB
    assert_eq!(term.total_files, 4); // file1.txt, file2.bin, subfileA1.txt, deep.txt
    assert_eq!(term.total_logical_bytes, 5 + 1000 + 12 + 7);
    assert_eq!(term.total_allocated_bytes, 0);
    assert_eq!(term.coverage_gap_count, 0);

    // Decode and verify all emitted observation records
    let mut reader = ObservationReader::new(Cursor::new(&stream_buf)).unwrap();
    assert_eq!(reader.target_path(), root.to_str().unwrap());

    let mut records = Vec::new();
    while let Some(rec) = reader.read_record().unwrap() {
        records.push(rec);
    }

    assert_eq!(records.len(), 9); // root dir + 4 files + 3 subdirs + 1 terminal

    // First record is root directory (entry_id 1, parent_id 0)
    let root_rec = match &records[0] {
        ObservationRecord::Directory(d) => d,
        other => panic!("expected root Directory record, got {other:?}"),
    };
    assert_eq!(root_rec.entry_id, 1);
    assert_eq!(root_rec.parent_id, 0);
    assert_eq!(root_rec.name, root.to_str().unwrap());
    assert_ne!(root_rec.file_attributes, 0);

    // Collect all directory, file, and terminal records
    let mut dirs: Vec<&DirectoryObservation> = Vec::new();
    let mut files: Vec<&FileObservation> = Vec::new();
    let mut terminals: Vec<&TerminalObservation> = Vec::new();

    for rec in &records {
        match rec {
            ObservationRecord::Directory(d) => dirs.push(d),
            ObservationRecord::File(f) => files.push(f),
            ObservationRecord::Terminal(t) => terminals.push(t),
            other => panic!("unexpected record type in clean hierarchy: {other:?}"),
        }
    }

    assert_eq!(dirs.len(), 4);
    assert_eq!(files.len(), 4);
    assert_eq!(terminals.len(), 1);

    // Validate monotonic IDs and parent relationships
    for w in records.windows(2) {
        let id1 = match &w[0] {
            ObservationRecord::Directory(d) => d.entry_id,
            ObservationRecord::File(f) => f.entry_id,
            ObservationRecord::Special(s) => s.entry_id,
            _ => 0,
        };
        let id2 = match &w[1] {
            ObservationRecord::Directory(d) => d.entry_id,
            ObservationRecord::File(f) => f.entry_id,
            ObservationRecord::Special(s) => s.entry_id,
            _ => 0,
        };
        if id1 > 0 && id2 > 0 {
            assert!(
                id2 > id1,
                "entry IDs must be strictly monotonic: {id1} -> {id2}"
            );
        }
    }

    // Verify files have correct sizes and allocated_size is None
    let file1 = files
        .iter()
        .find(|f| f.name == "file1.txt")
        .expect("file1.txt found");
    assert_eq!(file1.logical_size, 5);
    assert_eq!(file1.allocated_size, None);
    assert_eq!(file1.parent_id, 1);

    let file2 = files
        .iter()
        .find(|f| f.name == "file2.bin")
        .expect("file2.bin found");
    assert_eq!(file2.logical_size, 1000);
    assert_eq!(file2.allocated_size, None);
    assert_eq!(file2.parent_id, 1);

    let dir_sub_a = dirs.iter().find(|d| d.name == "subA").expect("subA found");
    assert_eq!(dir_sub_a.parent_id, 1);

    let subfile_a1 = files
        .iter()
        .find(|f| f.name == "subfileA1.txt")
        .expect("subfileA1.txt found");
    assert_eq!(subfile_a1.logical_size, 12);
    assert_eq!(subfile_a1.allocated_size, None);
    assert_eq!(subfile_a1.parent_id, dir_sub_a.entry_id);

    let dir_subsub = dirs
        .iter()
        .find(|d| d.name == "subsub")
        .expect("subsub found");
    assert_eq!(dir_subsub.parent_id, dir_sub_a.entry_id);

    let deep_file = files
        .iter()
        .find(|f| f.name == "deep.txt")
        .expect("deep.txt found");
    assert_eq!(deep_file.logical_size, 7);
    assert_eq!(deep_file.allocated_size, None);
    assert_eq!(deep_file.parent_id, dir_subsub.entry_id);
}

#[test]
fn test_already_signaled_cancellation_via_real_create_event_w() {
    let temp = TempTestDir::new("cancel");
    let root = temp.path();

    fs::write(root.join("file1.txt"), b"test").unwrap();
    let sub = root.join("sub");
    fs::create_dir_all(&sub).unwrap();
    fs::write(sub.join("file2.txt"), b"test2").unwrap();

    // Create real Win32 manual-reset event initially signaled (bInitialState = TRUE)
    let h_event = unsafe { CreateEventW(null_mut(), TRUE, TRUE, null_mut()) };
    assert_ne!(h_event, null_mut());
    assert_ne!(h_event, INVALID_HANDLE_VALUE);

    let cancellation = Win32EventCancellation::from_owned(h_event);
    assert!(cancellation.is_cancelled());

    let mut stream_buf = Vec::new();
    let mut writer = ObservationWriter::new(&mut stream_buf, root.to_str().unwrap()).unwrap();
    let term = scan_directory(root, &mut writer, &cancellation)
        .expect("scan_directory should return terminal Cancelled");

    assert_eq!(term.outcome, RunOutcome::Cancelled);

    // Verify stream was cleanly terminated
    let mut reader = ObservationReader::new(Cursor::new(&stream_buf)).unwrap();
    let mut last_rec = None;
    while let Some(rec) = reader.read_record().unwrap() {
        last_rec = Some(rec);
    }
    match last_rec {
        Some(ObservationRecord::Terminal(t)) => {
            assert_eq!(t.outcome, RunOutcome::Cancelled);
        }
        other => panic!("expected terminal record, got {other:?}"),
    }
}

#[test]
fn test_directory_symlink_or_junction_no_descent() {
    let temp = TempTestDir::new("symlink");
    let root = temp.path();

    let target_sub = root.join("real_dir");
    fs::create_dir_all(&target_sub).unwrap();
    fs::write(target_sub.join("nested.txt"), b"nested content").unwrap();

    let link_path = root.join("symlink_dir");

    // Attempt to create a directory symlink
    match symlink_dir(&target_sub, &link_path) {
        Ok(()) => {
            println!("Created directory symlink successfully; verifying reparse non-descent");
            let mut stream_buf = Vec::new();
            let mut writer =
                ObservationWriter::new(&mut stream_buf, root.to_str().unwrap()).unwrap();
            let term = scan_directory(root, &mut writer, &NoCancellation).unwrap();

            assert_eq!(term.outcome, RunOutcome::Finished);
            // real_dir + root + symlink_dir = 3 directories
            assert_eq!(term.total_directories, 3);
            // nested.txt should be counted once from real_dir, but NOT duplicated by descending into symlink_dir
            assert_eq!(term.total_files, 1);

            let mut reader = ObservationReader::new(Cursor::new(&stream_buf)).unwrap();
            let mut dirs = Vec::new();
            while let Some(rec) = reader.read_record().unwrap() {
                if let ObservationRecord::Directory(d) = rec {
                    dirs.push(d);
                }
            }

            let symlink_dir_rec = dirs
                .iter()
                .find(|d| d.name == "symlink_dir")
                .expect("symlink_dir observed");
            assert_ne!(
                symlink_dir_rec.file_attributes & pigtree_scan_worker::FILE_ATTRIBUTE_REPARSE_POINT,
                0,
                "reparse attribute must be set on symlink directory"
            );
        }
        Err(e) => {
            // Under Windows standard user without Developer Mode, SeCreateSymbolicLinkPrivilege is not held.
            // Explicitly report conditional skip without hanging.
            println!("Conditionally skipping symlink test: symlink creation requires elevated privilege or Developer Mode ({e})");
        }
    }
}

#[test]
fn test_parse_worker_args_valid_and_invalid() {
    // Valid args
    let valid_args = vec![
        "--target".to_string(),
        r"C:\test\dir".to_string(),
        "--pipe-handle".to_string(),
        "12345".to_string(),
        "--cancel-event-handle".to_string(),
        "67890".to_string(),
    ];
    let parsed = parse_worker_args(valid_args).expect("valid args should parse");
    assert_eq!(
        parsed,
        WorkerArgs {
            target: r"C:\test\dir".to_string(),
            pipe_handle: 12345,
            cancel_event_handle: 67890,
        }
    );

    // Missing target
    let missing_target = vec![
        "--pipe-handle".to_string(),
        "12345".to_string(),
        "--cancel-event-handle".to_string(),
        "67890".to_string(),
    ];
    assert_eq!(
        parse_worker_args(missing_target),
        Err(ArgsError::MissingArgument("--target"))
    );

    // Invalid handle integer
    let bad_handle = vec![
        "--target".to_string(),
        r"C:\test\dir".to_string(),
        "--pipe-handle".to_string(),
        "not_an_int".to_string(),
        "--cancel-event-handle".to_string(),
        "67890".to_string(),
    ];
    match parse_worker_args(bad_handle) {
        Err(ArgsError::InvalidHandle(flag, val)) => {
            assert_eq!(flag, "--pipe-handle");
            assert_eq!(val, "not_an_int");
        }
        other => panic!("expected InvalidHandle, got {other:?}"),
    }

    // Unexpected flag
    let unexpected = vec![
        "--target".to_string(),
        r"C:\test\dir".to_string(),
        "--pipe-handle".to_string(),
        "12345".to_string(),
        "--cancel-event-handle".to_string(),
        "67890".to_string(),
        "--unknown-flag".to_string(),
    ];
    assert_eq!(
        parse_worker_args(unexpected),
        Err(ArgsError::UnexpectedArgument("--unknown-flag".to_string()))
    );
}

#[test]
fn test_scan_direct_unc_target_rejects_before_observation() {
    let mut stream_buf = Vec::new();
    let mut writer = ObservationWriter::new(&mut stream_buf, r"\\server\share").unwrap();
    let result = scan_directory(Path::new(r"\\server\share"), &mut writer, &NoCancellation);
    assert!(
        result.is_err(),
        "Direct UNC scan must fail before observation"
    );
    assert_eq!(result.unwrap_err().kind(), std::io::ErrorKind::InvalidInput);

    let mut reader = ObservationReader::new(Cursor::new(&stream_buf)).unwrap();
    assert!(
        reader.read_record().unwrap().is_none(),
        "No observation records must be emitted for rejected UNC"
    );
}

#[test]
fn test_scan_nonexistent_directory_fails_before_observation() {
    let non_existent = std::env::temp_dir().join(format!(
        "pigtree_nonexistent_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));

    let mut stream_buf = Vec::new();
    let mut writer =
        ObservationWriter::new(&mut stream_buf, non_existent.to_str().unwrap()).unwrap();
    let result = scan_directory(&non_existent, &mut writer, &NoCancellation);
    assert!(
        result.is_err(),
        "Nonexistent scan target must be rejected before observation"
    );
    assert_eq!(result.unwrap_err().kind(), std::io::ErrorKind::InvalidInput);

    let mut reader = ObservationReader::new(Cursor::new(&stream_buf)).unwrap();
    assert!(
        reader.read_record().unwrap().is_none(),
        "No observation records must be emitted for nonexistent target"
    );
}

#[test]
fn test_filetime_to_unix_ms_accuracy() {
    // Zero FILETIME -> 0 ms
    let ft_zero = FILETIME {
        dwLowDateTime: 0,
        dwHighDateTime: 0,
    };
    assert_eq!(filetime_to_unix_ms(&ft_zero), 0);

    // Unix epoch: 1970-01-01 00:00:00 UTC = 116,444,736,000,000,000 100-ns intervals
    // dwHighDateTime = (116444736000000000 >> 32) = 27111902
    // dwLowDateTime = (116444736000000000 & 0xFFFFFFFF) = 3577643008
    let ft_epoch = FILETIME {
        dwLowDateTime: (116_444_736_000_000_000u64 & 0xFFFF_FFFF) as u32,
        dwHighDateTime: (116_444_736_000_000_000u64 >> 32) as u32,
    };
    assert_eq!(filetime_to_unix_ms(&ft_epoch), 0);

    // Unix epoch + 1 second (10,000,000 intervals) = 1,000 ms
    let val_1s = 116_444_736_000_000_000u64 + 10_000_000;
    let ft_1s = FILETIME {
        dwLowDateTime: (val_1s & 0xFFFF_FFFF) as u32,
        dwHighDateTime: (val_1s >> 32) as u32,
    };
    assert_eq!(filetime_to_unix_ms(&ft_1s), 1000);
}
#[test]
fn test_pipe_writer_with_real_win32_pipe() {
    let temp = TempTestDir::new("pipe_test");
    let root = temp.path();
    fs::write(root.join("testfile.dat"), vec![0x42; 256]).unwrap();

    // Create a real Win32 anonymous pipe
    let mut h_read: pigtree_scan_worker::HANDLE = null_mut();
    let mut h_write: pigtree_scan_worker::HANDLE = null_mut();

    #[link(name = "kernel32")]
    extern "system" {
        fn CreatePipe(
            hReadPipe: *mut pigtree_scan_worker::HANDLE,
            hWritePipe: *mut pigtree_scan_worker::HANDLE,
            lpPipeAttributes: *mut c_void,
            nSize: DWORD,
        ) -> pigtree_scan_worker::BOOL;
    }
    use pigtree_scan_worker::DWORD;
    use std::ffi::c_void;

    let ok = unsafe { CreatePipe(&mut h_read, &mut h_write, null_mut(), 0) };
    assert_ne!(ok, 0);

    let pipe_writer = PipeWriter::from_owned(h_write);

    // Spawn a thread to read from h_read
    let h_read_usize = h_read as usize;
    let handle_reader_thread = std::thread::spawn(move || {
        let h_read = h_read_usize as pigtree_scan_worker::HANDLE;
        struct ReadPipeGuard(pigtree_scan_worker::HANDLE);
        impl Drop for ReadPipeGuard {
            fn drop(&mut self) {
                unsafe {
                    pigtree_scan_worker::CloseHandle(self.0);
                }
            }
        }
        let _guard = ReadPipeGuard(h_read);

        struct PipeReader(pigtree_scan_worker::HANDLE);
        impl std::io::Read for PipeReader {
            fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
                #[link(name = "kernel32")]
                extern "system" {
                    fn ReadFile(
                        hFile: pigtree_scan_worker::HANDLE,
                        lpBuffer: *mut c_void,
                        nNumberOfBytesToRead: DWORD,
                        lpNumberOfBytesRead: *mut DWORD,
                        lpOverlapped: *mut c_void,
                    ) -> pigtree_scan_worker::BOOL;
                }
                let mut read: DWORD = 0;
                let ok = unsafe {
                    ReadFile(
                        self.0,
                        buf.as_mut_ptr() as *mut c_void,
                        buf.len() as DWORD,
                        &mut read,
                        null_mut(),
                    )
                };
                if ok == 0 {
                    let err = unsafe { pigtree_scan_worker::GetLastError() };
                    // ERROR_BROKEN_PIPE = 109
                    if err == 109 {
                        return Ok(0);
                    }
                    Err(std::io::Error::from_raw_os_error(err as i32))
                } else {
                    Ok(read as usize)
                }
            }
        }

        let reader_stream = PipeReader(h_read);
        let mut obs_reader = ObservationReader::new(reader_stream).expect("reader stream init");
        let mut records = Vec::new();
        while let Some(rec) = obs_reader.read_record().expect("read record") {
            records.push(rec);
        }
        records
    });

    let mut writer = ObservationWriter::new(pipe_writer, root.to_str().unwrap()).unwrap();
    let term = scan_directory(root, &mut writer, &NoCancellation).unwrap();
    assert_eq!(term.outcome, RunOutcome::Finished);
    drop(writer); // Closes h_write pipe handle so reader gets EOF (ERROR_BROKEN_PIPE)

    let records = handle_reader_thread.join().expect("reader thread join");
    assert_eq!(records.len(), 3); // root dir + 1 file + terminal
}
