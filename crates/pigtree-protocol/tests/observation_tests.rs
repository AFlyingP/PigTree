use pigtree_protocol::observation::*;
use std::io::{Cursor, ErrorKind};

#[test]
fn test_observation_stream_header_roundtrip() {
    let mut buf = Vec::new();
    let target = "C:\\Users\\Test\\Folder";
    {
        let _writer = ObservationWriter::new(&mut buf, target).expect("create writer");
    }

    let mut cursor = Cursor::new(buf);
    let reader = ObservationReader::new(&mut cursor).expect("create reader");
    assert_eq!(reader.target_path(), target);
}

#[test]
fn test_directory_observation_roundtrip() {
    let mut buf = Vec::new();
    let dir = DirectoryObservation {
        entry_id: 1,
        parent_id: 0,
        name: "test_dir".to_string(),
        file_attributes: 0x10, // FILE_ATTRIBUTE_DIRECTORY
        reparse_tag: 0,
        creation_time_utc_ms: 1700000000000,
        last_write_time_utc_ms: 1700000001000,
        last_access_time_utc_ms: 1700000002000,
    };

    {
        let mut writer = ObservationWriter::new(&mut buf, "C:\\Target").expect("writer");
        writer.write_directory(&dir).expect("write dir");
        writer.flush().expect("flush");
    }

    let mut cursor = Cursor::new(buf);
    let mut reader = ObservationReader::new(&mut cursor).expect("reader");
    let rec = reader
        .read_record()
        .expect("read record")
        .expect("record present");

    match rec {
        ObservationRecord::Directory(d) => {
            assert_eq!(d, dir);
        }
        other => panic!("expected Directory record, got {:?}", other),
    }

    // Clean EOF
    let eof = reader.read_record().expect("read eof");
    assert!(eof.is_none());
}

#[test]
fn test_file_observation_known_and_unavailable_allocated_size() {
    let mut buf = Vec::new();
    let file_known = FileObservation {
        entry_id: 2,
        parent_id: 1,
        name: "file_known.txt".to_string(),
        logical_size: 1048576,
        allocated_size: Some(1052672),
        file_attributes: 0x20, // FILE_ATTRIBUTE_ARCHIVE
        reparse_tag: 0,
        creation_time_utc_ms: 1700000000000,
        last_write_time_utc_ms: 1700000001000,
        last_access_time_utc_ms: 1700000002000,
    };

    let file_unavail = FileObservation {
        entry_id: 3,
        parent_id: 1,
        name: "file_unavail.bin".to_string(),
        logical_size: 512,
        allocated_size: None,
        file_attributes: 0x80, // FILE_ATTRIBUTE_NORMAL
        reparse_tag: 0,
        creation_time_utc_ms: 1700000003000,
        last_write_time_utc_ms: 1700000004000,
        last_access_time_utc_ms: 1700000005000,
    };

    {
        let mut writer = ObservationWriter::new(&mut buf, "C:\\Target").expect("writer");
        writer.write_file(&file_known).expect("write file_known");
        writer
            .write_file(&file_unavail)
            .expect("write file_unavail");
        writer.flush().expect("flush");
    }

    let mut cursor = Cursor::new(buf);
    let mut reader = ObservationReader::new(&mut cursor).expect("reader");

    let rec1 = reader
        .read_record()
        .expect("read rec1")
        .expect("rec1 present");
    match rec1 {
        ObservationRecord::File(f) => assert_eq!(f, file_known),
        other => panic!("expected File, got {:?}", other),
    }

    let rec2 = reader
        .read_record()
        .expect("read rec2")
        .expect("rec2 present");
    match rec2 {
        ObservationRecord::File(f) => assert_eq!(f, file_unavail),
        other => panic!("expected File, got {:?}", other),
    }

    assert!(reader.read_record().expect("eof").is_none());
}

#[test]
fn test_special_observation_roundtrip() {
    let mut buf = Vec::new();
    let special = SpecialObservation {
        entry_id: 10,
        parent_id: 1,
        name: "symlink_or_socket".to_string(),
        file_attributes: 0x400,  // FILE_ATTRIBUTE_REPARSE_POINT
        reparse_tag: 0xA000000C, // IO_REPARSE_TAG_SYMLINK
        creation_time_utc_ms: 1700000010000,
        last_write_time_utc_ms: 1700000020000,
        last_access_time_utc_ms: 1700000030000,
    };

    {
        let mut writer = ObservationWriter::new(&mut buf, "C:\\Target").expect("writer");
        writer.write_special(&special).expect("write special");
        writer.flush().expect("flush");
    }

    let mut cursor = Cursor::new(buf);
    let mut reader = ObservationReader::new(&mut cursor).expect("reader");

    match reader
        .read_record()
        .expect("read special")
        .expect("record present")
    {
        ObservationRecord::Special(s) => assert_eq!(s, special),
        other => panic!("expected Special, got {:?}", other),
    }

    assert!(reader.read_record().expect("eof").is_none());
}

#[test]
fn test_coverage_gap_and_terminal_outcomes_roundtrip() {
    let mut buf = Vec::new();
    let gap = CoverageGapObservation {
        path: "C:\\Target\\System Volume Information".to_string(),
        error_code: 5, // ERROR_ACCESS_DENIED
        error_message: "Access is denied".to_string(),
    };

    let term_finished = TerminalObservation {
        outcome: RunOutcome::Finished,
        total_directories: 42,
        total_files: 100,
        total_logical_bytes: 9999999,
        total_allocated_bytes: 10000000,
        coverage_gap_count: 1,
        duration_ms: 1250,
    };

    let term_cancelled = TerminalObservation {
        outcome: RunOutcome::Cancelled,
        total_directories: 10,
        total_files: 20,
        total_logical_bytes: 5000,
        total_allocated_bytes: 8192,
        coverage_gap_count: 0,
        duration_ms: 300,
    };

    let term_failed = TerminalObservation {
        outcome: RunOutcome::Failed,
        total_directories: 2,
        total_files: 1,
        total_logical_bytes: 100,
        total_allocated_bytes: 4096,
        coverage_gap_count: 5,
        duration_ms: 50,
    };

    {
        let mut writer = ObservationWriter::new(&mut buf, "C:\\Target").expect("writer");
        writer.write_coverage_gap(&gap).expect("write gap");
        writer
            .write_terminal(&term_finished)
            .expect("write term finished");
        writer
            .write_terminal(&term_cancelled)
            .expect("write term cancelled");
        writer
            .write_terminal(&term_failed)
            .expect("write term failed");
    }

    let mut cursor = Cursor::new(buf);
    let mut reader = ObservationReader::new(&mut cursor).expect("reader");

    match reader
        .read_record()
        .expect("read gap")
        .expect("gap present")
    {
        ObservationRecord::CoverageGap(g) => assert_eq!(g, gap),
        other => panic!("expected CoverageGap, got {:?}", other),
    }

    match reader
        .read_record()
        .expect("read finished")
        .expect("present")
    {
        ObservationRecord::Terminal(t) => {
            assert_eq!(t, term_finished);
            assert_eq!(t.outcome.as_str(), "finished");
        }
        other => panic!("expected Terminal, got {:?}", other),
    }

    match reader
        .read_record()
        .expect("read cancelled")
        .expect("present")
    {
        ObservationRecord::Terminal(t) => {
            assert_eq!(t, term_cancelled);
            assert_eq!(t.outcome.as_str(), "cancelled");
        }
        other => panic!("expected Terminal, got {:?}", other),
    }

    match reader.read_record().expect("read failed").expect("present") {
        ObservationRecord::Terminal(t) => {
            assert_eq!(t, term_failed);
            assert_eq!(t.outcome.as_str(), "failed");
        }
        other => panic!("expected Terminal, got {:?}", other),
    }

    assert!(reader.read_record().expect("eof").is_none());
}

#[test]
fn test_mixed_stream_full_lifecycle() {
    let mut buf = Vec::new();
    let root = DirectoryObservation {
        entry_id: 1,
        parent_id: 0,
        name: "root".to_string(),
        file_attributes: 0x10,
        reparse_tag: 0,
        creation_time_utc_ms: 1000,
        last_write_time_utc_ms: 2000,
        last_access_time_utc_ms: 3000,
    };
    let file1 = FileObservation {
        entry_id: 2,
        parent_id: 1,
        name: "data.db".to_string(),
        logical_size: 409600,
        allocated_size: Some(409600),
        file_attributes: 0x20,
        reparse_tag: 0,
        creation_time_utc_ms: 1100,
        last_write_time_utc_ms: 2100,
        last_access_time_utc_ms: 3100,
    };
    let file2 = FileObservation {
        entry_id: 3,
        parent_id: 1,
        name: "sparse.bin".to_string(),
        logical_size: 1000000,
        allocated_size: None,
        file_attributes: 0x200, // FILE_ATTRIBUTE_SPARSE_FILE
        reparse_tag: 0,
        creation_time_utc_ms: 1200,
        last_write_time_utc_ms: 2200,
        last_access_time_utc_ms: 3200,
    };
    let special = SpecialObservation {
        entry_id: 4,
        parent_id: 1,
        name: "device_link".to_string(),
        file_attributes: 0x400,
        reparse_tag: 0xA000000C,
        creation_time_utc_ms: 1300,
        last_write_time_utc_ms: 2300,
        last_access_time_utc_ms: 3300,
    };
    let gap = CoverageGapObservation {
        path: "C:\\Target\\Locked".to_string(),
        error_code: 32, // ERROR_SHARING_VIOLATION
        error_message:
            "The process cannot access the file because it is being used by another process."
                .to_string(),
    };
    let term = TerminalObservation {
        outcome: RunOutcome::Finished,
        total_directories: 1,
        total_files: 2,
        total_logical_bytes: 1409600,
        total_allocated_bytes: 409600,
        coverage_gap_count: 1,
        duration_ms: 840,
    };

    {
        let mut writer = ObservationWriter::new(&mut buf, "C:\\Target").expect("writer");
        writer.write_directory(&root).expect("write root");
        writer.write_file(&file1).expect("write file1");
        writer.write_file(&file2).expect("write file2");
        writer.write_special(&special).expect("write special");
        writer.write_coverage_gap(&gap).expect("write gap");
        writer.write_terminal(&term).expect("write term");
    }

    let mut cursor = Cursor::new(buf);
    let mut reader = ObservationReader::new(&mut cursor).expect("reader");
    assert_eq!(reader.target_path(), "C:\\Target");

    assert_eq!(
        reader.read_record().expect("rec1").expect("rec1 present"),
        ObservationRecord::Directory(root)
    );
    assert_eq!(
        reader.read_record().expect("rec2").expect("rec2 present"),
        ObservationRecord::File(file1)
    );
    assert_eq!(
        reader.read_record().expect("rec3").expect("rec3 present"),
        ObservationRecord::File(file2)
    );
    assert_eq!(
        reader.read_record().expect("rec4").expect("rec4 present"),
        ObservationRecord::Special(special)
    );
    assert_eq!(
        reader.read_record().expect("rec5").expect("rec5 present"),
        ObservationRecord::CoverageGap(gap)
    );
    assert_eq!(
        reader.read_record().expect("rec6").expect("rec6 present"),
        ObservationRecord::Terminal(term)
    );

    assert!(reader.read_record().expect("clean eof").is_none());
}

#[test]
fn test_fail_closed_on_corrupt_stream_magic() {
    let mut buf = Vec::new();
    {
        let _writer = ObservationWriter::new(&mut buf, "C:\\Target").unwrap();
    }
    buf[0] = b'X'; // corrupt magic

    let mut cursor = Cursor::new(buf);
    match ObservationReader::new(&mut cursor) {
        Err(ObservationDecodeError::InvalidMagic(m)) => {
            assert_eq!(m, [b'X', b'T', b'W', b'O']);
        }
        other => panic!("expected InvalidMagic, got {:?}", other.err()),
    }
}

#[test]
fn test_fail_closed_on_unsupported_version() {
    let mut buf = Vec::new();
    {
        let _writer = ObservationWriter::new(&mut buf, "C:\\Target").unwrap();
    }
    buf[4] = 0x99; // corrupt version
    buf[5] = 0x99;

    let mut cursor = Cursor::new(buf);
    match ObservationReader::new(&mut cursor) {
        Err(ObservationDecodeError::UnsupportedVersion(0x9999)) => {}
        other => panic!("expected UnsupportedVersion, got {:?}", other.err()),
    }
}

#[test]
fn test_fail_closed_on_invalid_record_tag() {
    let mut buf = Vec::new();
    {
        let _writer = ObservationWriter::new(&mut buf, "C:\\Target").unwrap();
    }
    buf.push(0xEE); // Invalid record tag

    let mut cursor = Cursor::new(buf);
    let mut reader = ObservationReader::new(&mut cursor).unwrap();

    match reader.read_record() {
        Err(ObservationDecodeError::InvalidRecordTag(0xEE)) => {}
        other => panic!("expected InvalidRecordTag(0xEE), got {:?}", other),
    }
}

#[test]
fn test_fail_closed_on_invalid_run_outcome() {
    let mut buf = Vec::new();
    let term = TerminalObservation {
        outcome: RunOutcome::Finished,
        total_directories: 0,
        total_files: 0,
        total_logical_bytes: 0,
        total_allocated_bytes: 0,
        coverage_gap_count: 0,
        duration_ms: 0,
    };
    {
        let mut writer = ObservationWriter::new(&mut buf, "C:\\Target").unwrap();
        writer.write_terminal(&term).unwrap();
    }

    // Corrupt outcome byte (after header and tag: header = 4 magic + 2 ver + 2 path_len + 9 target = 17; tag = 18th byte; outcome = 19th byte at index 18)
    let header_and_tag_len = 4 + 2 + 2 + "C:\\Target".len() + 1;
    buf[header_and_tag_len] = 0x77; // Invalid outcome

    let mut cursor = Cursor::new(buf);
    let mut reader = ObservationReader::new(&mut cursor).unwrap();

    match reader.read_record() {
        Err(ObservationDecodeError::InvalidOutcome(0x77)) => {}
        other => panic!("expected InvalidOutcome(0x77), got {:?}", other),
    }
}

#[test]
fn test_fail_closed_on_invalid_utf8_in_header_and_records() {
    // 1. Invalid UTF-8 in header target_path
    let mut buf = Vec::new();
    buf.extend_from_slice(&WORKER_MAGIC);
    buf.extend_from_slice(&WORKER_STREAM_VERSION.to_le_bytes());
    buf.extend_from_slice(&2u16.to_le_bytes());
    buf.push(0xFF); // Invalid UTF-8 byte
    buf.push(0xFF);

    let mut cursor = Cursor::new(&buf);
    match ObservationReader::new(&mut cursor) {
        Err(ObservationDecodeError::InvalidUtf8(_)) => {}
        other => panic!("expected InvalidUtf8 in header, got {:?}", other.err()),
    }

    // 2. Invalid UTF-8 in directory name
    let mut buf2 = Vec::new();
    let dir = DirectoryObservation {
        entry_id: 1,
        parent_id: 0,
        name: "valid_name".to_string(),
        file_attributes: 0x10,
        reparse_tag: 0,
        creation_time_utc_ms: 100,
        last_write_time_utc_ms: 200,
        last_access_time_utc_ms: 300,
    };
    {
        let mut writer = ObservationWriter::new(&mut buf2, "C:\\Target").unwrap();
        writer.write_directory(&dir).unwrap();
    }
    // Corrupt the name string bytes at the end
    let last_idx = buf2.len() - 1;
    buf2[last_idx] = 0xFF;

    let mut cursor2 = Cursor::new(buf2);
    let mut reader2 = ObservationReader::new(&mut cursor2).unwrap();
    match reader2.read_record() {
        Err(ObservationDecodeError::InvalidUtf8(_)) => {}
        other => panic!("expected InvalidUtf8 in directory name, got {:?}", other),
    }
}

#[test]
fn test_fail_closed_on_premature_eof_and_truncations() {
    // 1. Completely empty stream
    let empty_buf = Vec::new();
    let mut cursor = Cursor::new(empty_buf);
    match ObservationReader::new(&mut cursor) {
        Err(ObservationDecodeError::PrematureEof) => {}
        other => panic!(
            "expected PrematureEof on empty header, got {:?}",
            other.err()
        ),
    }

    // 2. Partial magic
    let mut cursor_partial_magic = Cursor::new(vec![0x50, 0x54]);
    match ObservationReader::new(&mut cursor_partial_magic) {
        Err(ObservationDecodeError::PrematureEof) => {}
        other => panic!(
            "expected PrematureEof on partial magic, got {:?}",
            other.err()
        ),
    }

    // 3. Partial version
    let mut buf = Vec::new();
    buf.extend_from_slice(&WORKER_MAGIC);
    buf.push(0x01); // 1 byte of version instead of 2
    let mut cursor_partial_ver = Cursor::new(buf);
    match ObservationReader::new(&mut cursor_partial_ver) {
        Err(ObservationDecodeError::PrematureEof) => {}
        other => panic!(
            "expected PrematureEof on partial version, got {:?}",
            other.err()
        ),
    }

    // 4. Truncation at record fixed buffer
    let mut buf_rec = Vec::new();
    let dir = DirectoryObservation {
        entry_id: 1,
        parent_id: 0,
        name: "test".to_string(),
        file_attributes: 0x10,
        reparse_tag: 0,
        creation_time_utc_ms: 100,
        last_write_time_utc_ms: 200,
        last_access_time_utc_ms: 300,
    };
    {
        let mut writer = ObservationWriter::new(&mut buf_rec, "C:\\Target").unwrap();
        writer.write_directory(&dir).unwrap();
    }

    // Truncate at various offsets
    let header_len = 4 + 2 + 2 + "C:\\Target".len();
    for cut in 1..dir.name.len() + 40 {
        let truncated = &buf_rec[..header_len + cut];
        let mut cursor_trunc = Cursor::new(truncated);
        let mut reader_trunc = ObservationReader::new(&mut cursor_trunc).unwrap();
        match reader_trunc.read_record() {
            Err(ObservationDecodeError::PrematureEof) => {}
            other => panic!("expected PrematureEof at cut {}, got {:?}", cut, other),
        }
    }
}

#[test]
fn test_oversized_string_rejection() {
    let huge_str = "x".repeat(65536); // exceeds u16::MAX (65535)

    // 1. Oversized target path in header
    let mut buf = Vec::new();
    let err = ObservationWriter::new(&mut buf, &huge_str).unwrap_err();
    assert_eq!(err.kind(), ErrorKind::InvalidInput);

    // 2. Oversized directory name
    let mut writer = ObservationWriter::new(&mut buf, "C:\\Target").unwrap();
    let oversized_dir = DirectoryObservation {
        entry_id: 1,
        parent_id: 0,
        name: huge_str.clone(),
        file_attributes: 0x10,
        reparse_tag: 0,
        creation_time_utc_ms: 0,
        last_write_time_utc_ms: 0,
        last_access_time_utc_ms: 0,
    };
    let dir_err = writer.write_directory(&oversized_dir).unwrap_err();
    assert_eq!(dir_err.kind(), ErrorKind::InvalidInput);

    // 3. Oversized file name
    let oversized_file = FileObservation {
        entry_id: 2,
        parent_id: 1,
        name: huge_str.clone(),
        logical_size: 0,
        allocated_size: None,
        file_attributes: 0x20,
        reparse_tag: 0,
        creation_time_utc_ms: 0,
        last_write_time_utc_ms: 0,
        last_access_time_utc_ms: 0,
    };
    let file_err = writer.write_file(&oversized_file).unwrap_err();
    assert_eq!(file_err.kind(), ErrorKind::InvalidInput);

    // 4. Oversized special name
    let oversized_special = SpecialObservation {
        entry_id: 3,
        parent_id: 1,
        name: huge_str.clone(),
        file_attributes: 0x400,
        reparse_tag: 0,
        creation_time_utc_ms: 0,
        last_write_time_utc_ms: 0,
        last_access_time_utc_ms: 0,
    };
    let spec_err = writer.write_special(&oversized_special).unwrap_err();
    assert_eq!(spec_err.kind(), ErrorKind::InvalidInput);

    // 5. Oversized coverage gap path and error_message
    let oversized_gap_path = CoverageGapObservation {
        path: huge_str.clone(),
        error_code: 5,
        error_message: "short".to_string(),
    };
    let gap_path_err = writer.write_coverage_gap(&oversized_gap_path).unwrap_err();
    assert_eq!(gap_path_err.kind(), ErrorKind::InvalidInput);

    let oversized_gap_msg = CoverageGapObservation {
        path: "short".to_string(),
        error_code: 5,
        error_message: huge_str.clone(),
    };
    let gap_msg_err = writer.write_coverage_gap(&oversized_gap_msg).unwrap_err();
    assert_eq!(gap_msg_err.kind(), ErrorKind::InvalidInput);
}
