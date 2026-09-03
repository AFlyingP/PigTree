//! Integration tests for pigtree-scan-worker public seam, Win32 traversal, and streaming protocol.

use pigtree_protocol::{
    DirectoryObservation, FileObservation, ObservationReader, ObservationRecord, ObservationWriter,
    RunOutcome, TerminalObservation, ValueKnowledge,
};
use pigtree_scan_worker::{
    filetime_to_unix_ms, parse_worker_args, scan_directory, ArgsError, Cancellation, CreateEventW,
    NoCancellation, PipeWriter, Win32EventCancellation, WorkerArgs, BOOL, FILETIME,
    INVALID_HANDLE_VALUE, LPCWSTR, TRUE,
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

/// Sums every emitted directory + file allocated size that carries evidence.
/// Special objects and coverage gaps contribute nothing.
fn sum_emitted_allocated_sizes(records: &[ObservationRecord]) -> u64 {
    records
        .iter()
        .map(|rec| match rec {
            ObservationRecord::Directory(d) => d.allocated_size.unwrap_or(0),
            ObservationRecord::File(f) => f.allocated_size.unwrap_or(0),
            _ => 0,
        })
        .sum()
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
    assert_eq!(term.coverage_gap_count, 0);

    // Decode and verify all emitted observation records
    let mut reader = ObservationReader::new(Cursor::new(&stream_buf)).unwrap();
    assert_eq!(reader.target_path(), root.to_str().unwrap());

    let mut records = Vec::new();
    while let Some(rec) = reader.read_record().unwrap() {
        records.push(rec);
    }

    assert_eq!(records.len(), 9); // root dir + 4 files + 3 subdirs + 1 terminal

    // Terminal must equal the exact sum of emitted allocated sizes (legacy
    // enumeration emits no allocation evidence, so both sides are 0 there).
    assert_eq!(
        term.total_allocated_bytes,
        sum_emitted_allocated_sizes(&records),
        "terminal total_allocated_bytes must equal the sum of emitted allocated sizes"
    );

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

    // Allocated-size evidence follows the enumeration path: handle-based
    // enumeration (NTFS/ReFS) provides it for every file and directory;
    // the legacy FindFirstFileExW fallback does not.
    let handle_based = files.iter().any(|f| f.allocated_size.is_some());
    if handle_based {
        for f in &files {
            assert!(
                f.allocated_size.is_some(),
                "handle-based enumeration must carry allocated_size for file {}",
                f.name
            );
        }
        for d in &dirs {
            assert!(
                d.allocated_size.is_some(),
                "handle-based enumeration must carry allocated_size for directory {}",
                d.name
            );
        }
    } else {
        println!("Conditional note: legacy enumeration path active; allocated_size stays None");
    }

    // Verify files have correct sizes and allocation evidence matches the path
    let file1 = files
        .iter()
        .find(|f| f.name == "file1.txt")
        .expect("file1.txt found");
    assert_eq!(file1.logical_size, 5);
    assert_eq!(file1.allocated_size.is_some(), handle_based);
    assert_eq!(file1.parent_id, 1);

    let file2 = files
        .iter()
        .find(|f| f.name == "file2.bin")
        .expect("file2.bin found");
    assert_eq!(file2.logical_size, 1000);
    assert_eq!(file2.allocated_size.is_some(), handle_based);
    assert_eq!(file2.parent_id, 1);

    let dir_sub_a = dirs.iter().find(|d| d.name == "subA").expect("subA found");
    assert_eq!(dir_sub_a.parent_id, 1);

    let subfile_a1 = files
        .iter()
        .find(|f| f.name == "subfileA1.txt")
        .expect("subfileA1.txt found");
    assert_eq!(subfile_a1.logical_size, 12);
    assert_eq!(subfile_a1.allocated_size.is_some(), handle_based);
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
    assert_eq!(deep_file.allocated_size.is_some(), handle_based);
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

#[test]
fn test_hard_link_aliases_share_object_identity() {
    #[link(name = "kernel32")]
    extern "system" {
        fn CreateHardLinkW(
            lpFileName: LPCWSTR,
            lpExistingFileName: LPCWSTR,
            lpSecurityAttributes: *mut std::ffi::c_void,
        ) -> BOOL;
    }

    let temp = TempTestDir::new("hardlink_identity");
    let root = temp.path();

    // 4 KiB so the allocation is a full, non-trivial NTFS cluster-backed size
    let payload = vec![0x5Au8; 4096];
    let original = root.join("original.dat");
    fs::write(&original, &payload).unwrap();
    let alias = root.join("alias.dat");

    let original_w = pigtree_scan_worker::to_wide_null(original.to_str().unwrap());
    let alias_w = pigtree_scan_worker::to_wide_null(alias.to_str().unwrap());
    // SAFETY: valid null-terminated wide paths for hard link creation.
    let ok = unsafe { CreateHardLinkW(alias_w.as_ptr(), original_w.as_ptr(), null_mut()) };
    if ok != TRUE {
        let err = unsafe { pigtree_scan_worker::GetLastError() };
        println!(
            "Conditionally skipping hard link test: CreateHardLinkW failed with error {err} \
             (hard links require NTFS/ReFS)"
        );
        return;
    }

    let mut stream_buf = Vec::new();
    let mut writer = ObservationWriter::new(&mut stream_buf, root.to_str().unwrap()).unwrap();
    let term = scan_directory(root, &mut writer, &NoCancellation).expect("scan_directory succeeds");
    assert_eq!(term.outcome, RunOutcome::Finished);
    assert_eq!(term.total_files, 2); // original.dat + alias.dat

    let mut reader = ObservationReader::new(Cursor::new(&stream_buf)).unwrap();
    let mut records = Vec::new();
    while let Some(rec) = reader.read_record().unwrap() {
        records.push(rec);
    }

    // Terminal sum invariant must hold for hard-linked trees too
    assert_eq!(
        term.total_allocated_bytes,
        sum_emitted_allocated_sizes(&records),
        "terminal total_allocated_bytes must equal the sum of emitted allocated sizes"
    );

    let files: Vec<&FileObservation> = records
        .iter()
        .filter_map(|rec| match rec {
            ObservationRecord::File(f) => Some(f),
            _ => None,
        })
        .collect();
    assert_eq!(files.len(), 2);

    let original_rec = files
        .iter()
        .find(|f| f.name == "original.dat")
        .expect("original.dat observed");
    let alias_rec = files
        .iter()
        .find(|f| f.name == "alias.dat")
        .expect("alias.dat observed");

    // Hard links only exist on hard-link-capable filesystems; there the worker
    // must attach canonical identity evidence from batched enumeration.
    match (original_rec.object_id, alias_rec.object_id) {
        (Some(id_a), Some(id_b)) => {
            assert_eq!(
                id_a, id_b,
                "both directory entries pointing at the same filesystem object \
                 must carry the same ObjectIdentity"
            );
        }
        (a, b) => panic!(
            "hard-link-capable volume (CreateHardLinkW succeeded) must emit object identity \
             evidence for both aliases; got original={a:?} alias={b:?}"
        ),
    }

    // Link knowledge: directory enumeration never provides link counts on
    // NTFS/ReFS (totals stay NotObserved on default scans), never zero.
    assert_eq!(original_rec.total_link_count, ValueKnowledge::NotObserved);
    assert_eq!(alias_rec.total_link_count, ValueKnowledge::NotObserved);

    // Batched enumeration provides allocation evidence for both aliases
    let alloc_a = original_rec
        .allocated_size
        .expect("original.dat must carry allocated_size from batched enumeration");
    let alloc_b = alias_rec
        .allocated_size
        .expect("alias.dat must carry allocated_size from batched enumeration");
    assert_eq!(alloc_a, alloc_b, "aliases share one object's allocation");
    assert!(
        alloc_a > 0,
        "4096-byte file must have non-zero physical allocation on NTFS"
    );
}

#[test]
fn test_allocated_size_and_terminal_sum_invariant() {
    let temp = TempTestDir::new("alloc_sum");
    let root = temp.path();

    fs::write(root.join("small.txt"), b"abc").unwrap();
    fs::write(root.join("large.bin"), vec![0xCD; 8192]).unwrap();
    let sub = root.join("sub");
    fs::create_dir_all(&sub).unwrap();
    fs::write(sub.join("nested.txt"), b"xyz").unwrap();

    let mut stream_buf = Vec::new();
    let mut writer = ObservationWriter::new(&mut stream_buf, root.to_str().unwrap()).unwrap();
    let term = scan_directory(root, &mut writer, &NoCancellation).expect("scan_directory succeeds");
    assert_eq!(term.outcome, RunOutcome::Finished);

    let mut reader = ObservationReader::new(Cursor::new(&stream_buf)).unwrap();
    let mut records = Vec::new();
    while let Some(rec) = reader.read_record().unwrap() {
        records.push(rec);
    }

    // (c) Terminal sum invariant: holds on every filesystem and enumeration path.
    assert_eq!(
        term.total_allocated_bytes,
        sum_emitted_allocated_sizes(&records),
        "terminal total_allocated_bytes must equal the sum of emitted allocated sizes"
    );

    let dirs: Vec<&DirectoryObservation> = records
        .iter()
        .filter_map(|rec| match rec {
            ObservationRecord::Directory(d) => Some(d),
            _ => None,
        })
        .collect();
    let files: Vec<&FileObservation> = records
        .iter()
        .filter_map(|rec| match rec {
            ObservationRecord::File(f) => Some(f),
            _ => None,
        })
        .collect();
    assert_eq!(dirs.len(), 2); // root + sub
    assert_eq!(files.len(), 3);

    // Detect the environment from the emitted stream itself. NotApplicable link
    // knowledge means a FAT-family volume (hard links unsupported); no
    // allocation evidence at all means the legacy enumeration path is active.
    let fat_family = files
        .iter()
        .all(|f| f.total_link_count == ValueKnowledge::NotApplicable);
    if fat_family {
        println!("Conditionally skipping: test volume is FAT-family (no hard-link evidence)");
        return;
    }

    let handle_based = files.iter().any(|f| f.allocated_size.is_some());
    if !handle_based {
        println!("Conditionally skipping: legacy enumeration path active (no allocation evidence)");
        return;
    }

    // (b) Batched enumeration evidence: every file AND directory (root included)
    // carries physical allocation, and file identity evidence is attached on
    // hard-link-capable volumes with the extended ID class.
    for f in &files {
        assert!(
            f.allocated_size.is_some(),
            "file {} must carry allocated_size",
            f.name
        );
    }
    for d in &dirs {
        assert!(
            d.allocated_size.is_some(),
            "directory {} must carry allocated_size",
            d.name
        );
    }
    // Root directory (entry_id 1, parent 0) must also carry real evidence
    let root_rec = dirs.iter().find(|d| d.parent_id == 0).expect("root found");
    assert!(
        root_rec.allocated_size.is_some(),
        "root directory must carry allocated_size from handle evidence"
    );

    let identity_based = files.iter().any(|f| f.object_id.is_some());
    if identity_based {
        for f in &files {
            assert!(
                f.object_id.is_some(),
                "file {} must carry object identity on extended-ID enumeration",
                f.name
            );
        }
        // Identity must be non-degenerate: distinct files get distinct identities
        let ids: Vec<_> = files.iter().filter_map(|f| f.object_id).collect();
        let distinct: std::collections::HashSet<_> = ids.iter().collect();
        assert_eq!(
            distinct.len(),
            ids.len(),
            "distinct filesystem objects must have distinct identities"
        );
        for f in &files {
            assert_eq!(
                f.total_link_count,
                ValueKnowledge::NotObserved,
                "link totals stay NotObserved on default scans (never zero)"
            );
        }
    } else {
        println!(
            "Conditional note: allocation evidence present but object identity unavailable \
             (volume GUID or extended ID class unresolved)"
        );
    }
}

#[repr(C)]
#[derive(Default)]
struct TestFileStandardInfo {
    allocation_size: i64,
    end_of_file: i64,
    number_of_links: u32,
    delete_pending: u8,
    directory: u8,
    _pad: [u8; 2],
}

mod test_win32_ffi {
    use super::*;

    #[link(name = "kernel32")]
    extern "system" {
        pub fn CreateFileW(
            lpFileName: LPCWSTR,
            dwDesiredAccess: u32,
            dwShareMode: u32,
            lpSecurityAttributes: *mut std::ffi::c_void,
            dwCreationDisposition: u32,
            dwFlagsAndAttributes: u32,
            hTemplateFile: *mut std::ffi::c_void,
        ) -> *mut std::ffi::c_void;

        pub fn GetFileInformationByHandleEx(
            hFile: *mut std::ffi::c_void,
            FileInformationClass: i32,
            lpFileInformation: *mut std::ffi::c_void,
            dwBufferSize: u32,
        ) -> BOOL;

        pub fn CloseHandle(hObject: *mut std::ffi::c_void) -> BOOL;

        pub fn GetVolumeInformationW(
            lpRootPathName: LPCWSTR,
            lpVolumeNameBuffer: *mut u16,
            nVolumeNameSize: u32,
            lpVolumeSerialNumber: *mut u32,
            lpMaximumComponentLength: *mut u32,
            lpFileSystemFlags: *mut u32,
            lpFileSystemNameBuffer: *mut u16,
            nFileSystemNameSize: u32,
        ) -> BOOL;

        pub fn GetVolumePathNameW(
            lpszFileName: LPCWSTR,
            lpszVolumePathName: *mut u16,
            cchBufferLength: u32,
        ) -> BOOL;
    }
}

#[test]
fn test_windows_scan_worker_tracer_issue_20() {
    // Windows-gated black-box tracer test for scan-worker vertical slice (issue #20).
    // Pre-agreed public seam: pigtree_scan_worker::scan_directory -> ObservationWriter stream -> ObservationReader.
    // Test fixture with spaced parent directories to verify robust quoting on cmd.exe /C
    let temp = TempTestDir::new("tracer root with spaces");
    let root = temp.path();

    // Determine volume filesystem name
    let root_wide = pigtree_scan_worker::to_wide_null(root.to_str().unwrap());
    let mut vol_path = [0u16; 512];
    let ok = unsafe {
        test_win32_ffi::GetVolumePathNameW(root_wide.as_ptr(), vol_path.as_mut_ptr(), 512)
    };
    assert_ne!(ok, 0, "GetVolumePathNameW failed");
    let mut fs_name_buf = [0u16; 64];
    let ok = unsafe {
        test_win32_ffi::GetVolumeInformationW(
            vol_path.as_ptr(),
            null_mut(),
            0,
            null_mut(),
            null_mut(),
            null_mut(),
            fs_name_buf.as_mut_ptr(),
            64,
        )
    };
    assert_ne!(ok, 0, "GetVolumeInformationW failed");
    let fs_name_len = fs_name_buf
        .iter()
        .position(|&c| c == 0)
        .unwrap_or(fs_name_buf.len());
    let fs_name = String::from_utf16_lossy(&fs_name_buf[..fs_name_len]);
    let is_ntfs_or_refs =
        fs_name.eq_ignore_ascii_case("NTFS") || fs_name.eq_ignore_ascii_case("ReFS");

    // Create outside directory (outside scan root, with spaces in path) with marker file
    let outside_temp = TempTestDir::new("tracer outside with spaces");
    let outside_dir = outside_temp.path();
    let outside_marker = outside_dir.join("outside_marker.txt");
    fs::write(
        &outside_marker,
        b"outside sentinel file: must not be traversed",
    )
    .unwrap();

    // Create files and subdirectories inside the scan root
    let tracer_path = root.join("tracer.bin");
    // Write 5000 bytes: on standard NTFS cluster size (4096), logical size is 5000 and allocation is 8192
    fs::write(&tracer_path, vec![0x42u8; 5000]).unwrap();

    let sub_dir = root.join("sub_dir");
    fs::create_dir_all(&sub_dir).unwrap();
    let sub_file = sub_dir.join("sub_file.txt");
    fs::write(&sub_file, b"inside sub_dir").unwrap();

    // Create junction with spaces inside root pointing to outside_dir
    let junction_path = root.join("junction link with spaces");
    struct JunctionGuard(PathBuf);
    impl Drop for JunctionGuard {
        fn drop(&mut self) {
            let _ = fs::remove_dir(&self.0);
        }
    }

    let junction_cmd = std::process::Command::new("cmd")
        .args([
            "/C",
            "mklink",
            "/J",
            junction_path.to_str().unwrap(),
            outside_dir.to_str().unwrap(),
        ])
        .output()
        .expect("spawn mklink /J command");

    assert!(
        junction_cmd.status.success(),
        "mklink /J failed: stdout={}, stderr={}",
        String::from_utf8_lossy(&junction_cmd.stdout),
        String::from_utf8_lossy(&junction_cmd.stderr)
    );
    let _junction_guard = JunctionGuard(junction_path.clone());

    // Query ground-truth sizes on tracer.bin itself via test-owned documented Win32 FFI
    let (ground_truth_eof, ground_truth_alloc) = {
        const FILE_READ_ATTRIBUTES: u32 = 0x0080;
        const FILE_SHARE_READ: u32 = 1;
        const FILE_SHARE_WRITE: u32 = 2;
        const FILE_SHARE_DELETE: u32 = 4;
        const OPEN_EXISTING: u32 = 3;
        const FILE_ATTRIBUTE_NORMAL: u32 = 0x80;
        const FILE_STANDARD_INFO_CLASS: i32 = 1;

        let tracer_wide = pigtree_scan_worker::to_wide_null(tracer_path.to_str().unwrap());
        let handle = unsafe {
            test_win32_ffi::CreateFileW(
                tracer_wide.as_ptr(),
                FILE_READ_ATTRIBUTES,
                FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
                null_mut(),
                OPEN_EXISTING,
                FILE_ATTRIBUTE_NORMAL,
                null_mut(),
            )
        };
        assert!(
            !handle.is_null() && handle != pigtree_scan_worker::INVALID_HANDLE_VALUE,
            "failed to open tracer.bin for ground-truth query"
        );

        let mut std_info = TestFileStandardInfo::default();
        let ok = unsafe {
            test_win32_ffi::GetFileInformationByHandleEx(
                handle,
                FILE_STANDARD_INFO_CLASS,
                &mut std_info as *mut _ as *mut std::ffi::c_void,
                std::mem::size_of::<TestFileStandardInfo>() as u32,
            )
        };
        unsafe {
            test_win32_ffi::CloseHandle(handle);
        }
        assert_ne!(
            ok, 0,
            "GetFileInformationByHandleEx(FileStandardInfo) failed"
        );
        (std_info.end_of_file as u64, std_info.allocation_size as u64)
    };

    // Execute scan through pre-agreed public seam
    let mut stream_buf = Vec::new();
    let mut writer = ObservationWriter::new(&mut stream_buf, root.to_str().unwrap()).unwrap();
    let term = scan_directory(root, &mut writer, &NoCancellation).expect("scan_directory succeeds");
    assert_eq!(term.outcome, RunOutcome::Finished);

    let mut reader = ObservationReader::new(Cursor::new(&stream_buf)).unwrap();
    let mut records = Vec::new();
    while let Some(rec) = reader.read_record().unwrap() {
        records.push(rec);
    }

    if is_ntfs_or_refs {
        // Do not silently skip batched traversal on supported filesystems
        // 1. Exact non-empty names and sound parentage
        let mut dirs = Vec::new();
        let mut files = Vec::new();
        for rec in &records {
            match rec {
                ObservationRecord::Directory(d) => {
                    assert!(!d.name.is_empty(), "directory name must not be empty");
                    dirs.push(d);
                }
                ObservationRecord::File(f) => {
                    assert!(!f.name.is_empty(), "file name must not be empty");
                    files.push(f);
                }
                ObservationRecord::CoverageGap(gap) => {
                    panic!("unexpected coverage gap encountered: {:?}", gap);
                }
                _ => {}
            }
        }

        // 2. No unexpected coverage gap
        assert_eq!(term.coverage_gap_count, 0, "coverage_gap_count must be 0");

        // Exact counts
        assert_eq!(
            term.total_directories, 3,
            "root, sub_dir, junction link with spaces"
        );
        assert_eq!(term.total_files, 2, "tracer.bin, sub_file.txt");
        assert_eq!(dirs.len(), 3);
        assert_eq!(files.len(), 2);

        // Verify root directory parentage
        let root_dir = dirs
            .iter()
            .find(|d| d.parent_id == 0)
            .expect("root directory present");
        assert_eq!(root_dir.entry_id, 1);

        let sub_dir_rec = dirs
            .iter()
            .find(|d| d.name == "sub_dir")
            .expect("sub_dir present");
        assert_eq!(sub_dir_rec.parent_id, root_dir.entry_id);

        let junction_dir_rec = dirs
            .iter()
            .find(|d| d.name == "junction link with spaces")
            .expect("junction link present");
        assert_eq!(junction_dir_rec.parent_id, root_dir.entry_id);

        let tracer_file_rec = files
            .iter()
            .find(|f| f.name == "tracer.bin")
            .expect("tracer.bin present");
        assert_eq!(tracer_file_rec.parent_id, root_dir.entry_id);

        let sub_file_rec = files
            .iter()
            .find(|f| f.name == "sub_file.txt")
            .expect("sub_file.txt present");
        assert_eq!(sub_file_rec.parent_id, sub_dir_rec.entry_id);

        // 3. File and directory ObjectIdentity presence (non-zero ID presence;
        // canonical identity alignment between FileIdInfo and FileIdExtdDirectoryInfo
        // is independently verified in test_root_and_child_identity_alignment_issue_20).
        for d in &dirs {
            let id = d
                .object_id
                .unwrap_or_else(|| panic!("directory {} must carry ObjectIdentity", d.name));
            assert_ne!(
                id.file_id, 0,
                "directory {} file_id must be non-zero",
                d.name
            );
        }
        for f in &files {
            let id = f
                .object_id
                .unwrap_or_else(|| panic!("file {} must carry ObjectIdentity", f.name));
            assert_ne!(id.file_id, 0, "file {} file_id must be non-zero", f.name);
        }

        // 4. Observed allocation evidence
        for d in &dirs {
            assert!(
                d.allocated_size.is_some(),
                "directory {} must carry allocated_size evidence",
                d.name
            );
        }
        for f in &files {
            assert!(
                f.allocated_size.is_some(),
                "file {} must carry allocated_size evidence",
                f.name
            );
        }
        assert_eq!(
            term.total_allocated_bytes,
            sum_emitted_allocated_sizes(&records),
            "terminal total_allocated_bytes must match sum of emitted allocated sizes"
        );

        // Total link count verification: directories are NotApplicable, default files are NotObserved
        for d in &dirs {
            assert_eq!(
                d.total_link_count,
                ValueKnowledge::NotApplicable,
                "directory {} total_link_count must be NotApplicable",
                d.name
            );
        }
        for f in &files {
            assert_eq!(
                f.total_link_count,
                ValueKnowledge::NotObserved,
                "file {} total_link_count must retain NotObserved by default",
                f.name
            );
        }

        // 5. IO_REPARSE_TAG_MOUNT_POINT preservation through junction
        const IO_REPARSE_TAG_MOUNT_POINT: u32 = 0xA000_0003;
        assert_ne!(
            junction_dir_rec.file_attributes & pigtree_scan_worker::FILE_ATTRIBUTE_REPARSE_POINT,
            0,
            "junction_link must have FILE_ATTRIBUTE_REPARSE_POINT"
        );
        assert_eq!(
            junction_dir_rec.reparse_tag, IO_REPARSE_TAG_MOUNT_POINT,
            "junction_link reparse_tag must be IO_REPARSE_TAG_MOUNT_POINT"
        );

        // 6. No traversal across that reparse boundary
        assert!(
            !files.iter().any(|f| f.name == "outside_marker.txt"),
            "outside_marker.txt must not be traversed across junction"
        );
        assert!(
            !dirs
                .iter()
                .any(|d| d.parent_id == junction_dir_rec.entry_id),
            "no directory may be traversed inside junction"
        );
        assert!(
            !files
                .iter()
                .any(|f| f.parent_id == junction_dir_rec.entry_id),
            "no file may be traversed inside junction"
        );

        // 7 & 8. Ground-truth sizes on tracer.bin and conditional logical-vs-allocation discrimination
        if ground_truth_eof != ground_truth_alloc {
            assert_eq!(
                tracer_file_rec.logical_size, ground_truth_eof,
                "logical_size must equal ground-truth EndOfFile, not AllocationSize"
            );
            assert_eq!(
                tracer_file_rec.allocated_size,
                Some(ground_truth_alloc),
                "allocated_size must equal ground-truth AllocationSize, not EndOfFile"
            );
        }
    }
}

#[test]
fn test_root_and_child_identity_alignment_issue_20() {
    // Windows-gated public-seam tracer test:
    // Verify that a directory scanned as a root (identity from FileIdInfo)
    // matches the same directory's ObjectIdentity when enumerated as a child
    // entry by its parent (identity from FileIdExtdDirectoryInfo).
    let temp = TempTestDir::new("id_align");
    let parent = temp.path();
    let child = parent.join("child_dir");
    fs::create_dir_all(&child).unwrap();

    // Check if filesystem is NTFS or ReFS
    let parent_wide = pigtree_scan_worker::to_wide_null(parent.to_str().unwrap());
    let mut vol_path = [0u16; 512];
    let ok = unsafe {
        test_win32_ffi::GetVolumePathNameW(parent_wide.as_ptr(), vol_path.as_mut_ptr(), 512)
    };
    assert_ne!(ok, 0, "GetVolumePathNameW failed");
    let mut fs_name_buf = [0u16; 64];
    let ok = unsafe {
        test_win32_ffi::GetVolumeInformationW(
            vol_path.as_ptr(),
            null_mut(),
            0,
            null_mut(),
            null_mut(),
            null_mut(),
            fs_name_buf.as_mut_ptr(),
            64,
        )
    };
    assert_ne!(ok, 0, "GetVolumeInformationW failed");
    let fs_name_len = fs_name_buf
        .iter()
        .position(|&c| c == 0)
        .unwrap_or(fs_name_buf.len());
    let fs_name = String::from_utf16_lossy(&fs_name_buf[..fs_name_len]);
    let is_ntfs_or_refs =
        fs_name.eq_ignore_ascii_case("NTFS") || fs_name.eq_ignore_ascii_case("ReFS");

    // Scan 1: Scan child directory as root (exercises FileIdInfo)
    let mut child_stream = Vec::new();
    let mut child_writer =
        ObservationWriter::new(&mut child_stream, child.to_str().unwrap()).unwrap();
    let child_term =
        scan_directory(&child, &mut child_writer, &NoCancellation).expect("child scan succeeds");
    assert_eq!(child_term.outcome, RunOutcome::Finished);

    let mut child_reader = ObservationReader::new(Cursor::new(&child_stream)).unwrap();
    let mut child_root_rec: Option<DirectoryObservation> = None;
    while let Some(rec) = child_reader.read_record().unwrap() {
        if let ObservationRecord::Directory(d) = rec {
            if d.parent_id == 0 {
                child_root_rec = Some(d);
            }
        }
    }
    let child_as_root = child_root_rec.expect("child root directory observation present");

    // Scan 2: Scan parent directory (exercises FileIdExtdDirectoryInfo for child)
    let mut parent_stream = Vec::new();
    let mut parent_writer =
        ObservationWriter::new(&mut parent_stream, parent.to_str().unwrap()).unwrap();
    let parent_term =
        scan_directory(parent, &mut parent_writer, &NoCancellation).expect("parent scan succeeds");
    assert_eq!(parent_term.outcome, RunOutcome::Finished);

    let mut parent_reader = ObservationReader::new(Cursor::new(&parent_stream)).unwrap();
    let mut child_entry_rec: Option<DirectoryObservation> = None;
    while let Some(rec) = parent_reader.read_record().unwrap() {
        if let ObservationRecord::Directory(d) = rec {
            if d.name == "child_dir" {
                child_entry_rec = Some(d);
            }
        }
    }
    let child_as_entry =
        child_entry_rec.expect("child_dir entry observation present in parent scan");

    if is_ntfs_or_refs {
        // Root identity from FileIdInfo must match child entry identity from FileIdExtdDirectoryInfo
        let root_id = child_as_root
            .object_id
            .expect("child as root must have ObjectIdentity on NTFS/ReFS");
        let entry_id = child_as_entry
            .object_id
            .expect("child as entry must have ObjectIdentity on NTFS/ReFS");

        assert_eq!(
            root_id, entry_id,
            "root ObjectIdentity (from FileIdInfo) must match child entry ObjectIdentity (from FileIdExtdDirectoryInfo)"
        );

        // Keep directory link knowledge NotApplicable per issue semantics (not Known(1))
        assert_eq!(
            child_as_root.total_link_count,
            ValueKnowledge::NotApplicable,
            "root directory link knowledge must be NotApplicable"
        );
        assert_eq!(
            child_as_entry.total_link_count,
            ValueKnowledge::NotApplicable,
            "child directory link knowledge must be NotApplicable"
        );
    }
}

#[test]
fn test_default_scan_never_emits_content_stream_records() {
    // ADR 0001 / issue #20 Q4: the default profile leaves secondary content
    // streams Not Observed. Even a file with ADS present must produce no
    // ContentStream records; the stream seam only opens for explicit
    // profiles and enrichments.
    let temp = TempTestDir::new("no_ads_records");
    let root = temp.path();

    fs::write(root.join("plain.txt"), b"hello").unwrap();

    let mut stream_buf = Vec::new();
    let mut writer = ObservationWriter::new(&mut stream_buf, root.to_str().unwrap()).unwrap();
    let term = scan_directory(root, &mut writer, &NoCancellation).expect("scan should succeed");
    assert_eq!(term.outcome, RunOutcome::Finished);

    let mut reader = ObservationReader::new(Cursor::new(&stream_buf)).unwrap();
    while let Some(rec) = reader.read_record().unwrap() {
        assert!(
            !matches!(rec, ObservationRecord::ContentStream(_)),
            "default scan must never emit a ContentStream record, got {rec:?}"
        );
    }
}
