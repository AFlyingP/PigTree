use pigtree_engine::{build_graph_from_reader, EntryKind, GraphBuildError, GraphBuilder};
use pigtree_protocol::{
    CoverageGapObservation, DirectoryObservation, FileObservation, ObservationReader,
    ObservationRecord, ObservationWriter, RunOutcome, SpecialObservation, TerminalObservation,
};
use std::io::Cursor;

#[test]
fn test_valid_hierarchy_gap_and_counts() {
    let mut buf = Vec::new();
    let mut writer = ObservationWriter::new(&mut buf, r"C:TestRoot").unwrap();

    // 1. Root directory (id=1, parent=0)
    writer
        .write_directory(&DirectoryObservation {
            entry_id: 1,
            parent_id: 0,
            name: "Root".to_string(),
            file_attributes: 0x10,
            reparse_tag: 0,
            creation_time_utc_ms: 100,
            last_write_time_utc_ms: 200,
            last_access_time_utc_ms: 300,
        })
        .unwrap();

    // 2. Subdirectory (id=2, parent=1)
    writer
        .write_directory(&DirectoryObservation {
            entry_id: 2,
            parent_id: 1,
            name: "SubDir".to_string(),
            file_attributes: 0x10,
            reparse_tag: 0,
            creation_time_utc_ms: 101,
            last_write_time_utc_ms: 201,
            last_access_time_utc_ms: 301,
        })
        .unwrap();

    // 3. File in Root (id=3, parent=1) with known allocated size
    writer
        .write_file(&FileObservation {
            entry_id: 3,
            parent_id: 1,
            name: "file1.txt".to_string(),
            logical_size: 1000,
            allocated_size: Some(4096),
            file_attributes: 0x20,
            reparse_tag: 0,
            creation_time_utc_ms: 102,
            last_write_time_utc_ms: 202,
            last_access_time_utc_ms: 302,
        })
        .unwrap();

    // 4. File in SubDir (id=4, parent=2) with unavailable (None) allocated size
    writer
        .write_file(&FileObservation {
            entry_id: 4,
            parent_id: 2,
            name: "file2.dat".to_string(),
            logical_size: 500,
            allocated_size: None,
            file_attributes: 0x20,
            reparse_tag: 0,
            creation_time_utc_ms: 103,
            last_write_time_utc_ms: 203,
            last_access_time_utc_ms: 303,
        })
        .unwrap();

    // 5. Special object in SubDir (id=5, parent=2)
    writer
        .write_special(&SpecialObservation {
            entry_id: 5,
            parent_id: 2,
            name: "junction_link".to_string(),
            file_attributes: 0x400,
            reparse_tag: 0xA0000003,
            creation_time_utc_ms: 104,
            last_write_time_utc_ms: 204,
            last_access_time_utc_ms: 304,
        })
        .unwrap();

    // 6. Coverage Gap
    writer
        .write_coverage_gap(&CoverageGapObservation {
            path: r"C:TestRootSubDirLocked".to_string(),
            error_code: 5,
            error_message: "Access is denied".to_string(),
        })
        .unwrap();

    // 7. Terminal observation
    writer
        .write_terminal(&TerminalObservation {
            outcome: RunOutcome::Finished,
            total_directories: 2,
            total_files: 2,
            total_logical_bytes: 1500,
            total_allocated_bytes: 4096,
            coverage_gap_count: 1,
            duration_ms: 150,
        })
        .unwrap();

    let reader = ObservationReader::new(Cursor::new(buf)).unwrap();
    let graph = build_graph_from_reader(reader).expect("graph build should succeed");

    assert_eq!(graph.root_target(), r"C:TestRoot");
    assert_eq!(graph.root_id(), 1);
    assert_eq!(graph.total_entries(), 5);

    // Root entry checks
    let root = graph.root();
    assert_eq!(root.id, 1);
    assert_eq!(root.parent_id, 0);
    assert_eq!(root.name, "Root");
    assert_eq!(root.kind, EntryKind::Directory);
    assert_eq!(root.children, vec![2, 3]);

    // Subdir entry checks
    let subdir = graph.entry(2).expect("entry 2 must exist");
    assert_eq!(subdir.id, 2);
    assert_eq!(subdir.parent_id, 1);
    assert_eq!(subdir.name, "SubDir");
    assert_eq!(subdir.kind, EntryKind::Directory);
    assert_eq!(subdir.children, vec![4, 5]);

    // File 1 checks (known allocated size)
    let f1 = graph.entry(3).expect("entry 3 must exist");
    assert_eq!(f1.id, 3);
    assert_eq!(f1.parent_id, 1);
    assert_eq!(f1.name, "file1.txt");
    assert_eq!(f1.kind, EntryKind::File);
    assert_eq!(f1.logical_size, Some(1000));
    assert_eq!(f1.allocated_size, Some(4096));
    assert!(f1.children.is_empty());

    // File 2 checks (unavailable allocated size)
    let f2 = graph.entry(4).expect("entry 4 must exist");
    assert_eq!(f2.id, 4);
    assert_eq!(f2.parent_id, 2);
    assert_eq!(f2.name, "file2.dat");
    assert_eq!(f2.kind, EntryKind::File);
    assert_eq!(f2.logical_size, Some(500));
    assert_eq!(f2.allocated_size, None);

    // Special object checks
    let sp = graph.entry(5).expect("entry 5 must exist");
    assert_eq!(sp.id, 5);
    assert_eq!(sp.parent_id, 2);
    assert_eq!(sp.name, "junction_link");
    assert_eq!(sp.kind, EntryKind::Special);
    assert_eq!(sp.reparse_tag, 0xA0000003);

    // Coverage gaps check
    assert_eq!(graph.gaps().len(), 1);
    assert_eq!(graph.gaps()[0].path, r"C:TestRootSubDirLocked");
    assert_eq!(graph.gaps()[0].error_code, 5);

    // Terminal observation check
    assert_eq!(graph.terminal().outcome, RunOutcome::Finished);
    assert_eq!(graph.terminal().total_directories, 2);
    assert_eq!(graph.terminal().total_files, 2);
    assert_eq!(graph.terminal().total_logical_bytes, 1500);
    assert_eq!(graph.terminal().total_allocated_bytes, 4096);
    assert_eq!(graph.terminal().coverage_gap_count, 1);
    assert_eq!(graph.terminal().duration_ms, 150);
    // Mixed Some/None means allocated_bytes_known is false
    assert!(!graph.allocated_bytes_known());
}

#[test]
fn test_duplicate_id_rejected() {
    let mut buf = Vec::new();
    let mut writer = ObservationWriter::new(&mut buf, r"C:TestRoot").unwrap();

    writer
        .write_directory(&DirectoryObservation {
            entry_id: 1,
            parent_id: 0,
            name: "Root".to_string(),
            file_attributes: 0x10,
            reparse_tag: 0,
            creation_time_utc_ms: 0,
            last_write_time_utc_ms: 0,
            last_access_time_utc_ms: 0,
        })
        .unwrap();

    writer
        .write_file(&FileObservation {
            entry_id: 2,
            parent_id: 1,
            name: "first.txt".to_string(),
            logical_size: 10,
            allocated_size: Some(10),
            file_attributes: 0x20,
            reparse_tag: 0,
            creation_time_utc_ms: 0,
            last_write_time_utc_ms: 0,
            last_access_time_utc_ms: 0,
        })
        .unwrap();

    // Duplicate ID 2
    writer
        .write_file(&FileObservation {
            entry_id: 2,
            parent_id: 1,
            name: "duplicate.txt".to_string(),
            logical_size: 20,
            allocated_size: Some(20),
            file_attributes: 0x20,
            reparse_tag: 0,
            creation_time_utc_ms: 0,
            last_write_time_utc_ms: 0,
            last_access_time_utc_ms: 0,
        })
        .unwrap();

    let reader = ObservationReader::new(Cursor::new(buf)).unwrap();
    let err = build_graph_from_reader(reader).unwrap_err();
    match err {
        GraphBuildError::DuplicateEntryId(id) => assert_eq!(id, 2),
        other => panic!("expected DuplicateEntryId(2), got {:?}", other),
    }
}

#[test]
fn test_missing_or_out_of_order_parent_rejected() {
    let mut buf = Vec::new();
    let mut writer = ObservationWriter::new(&mut buf, r"C:TestRoot").unwrap();

    writer
        .write_directory(&DirectoryObservation {
            entry_id: 1,
            parent_id: 0,
            name: "Root".to_string(),
            file_attributes: 0x10,
            reparse_tag: 0,
            creation_time_utc_ms: 0,
            last_write_time_utc_ms: 0,
            last_access_time_utc_ms: 0,
        })
        .unwrap();

    // Child with parent_id=99 before parent 99 exists
    writer
        .write_file(&FileObservation {
            entry_id: 2,
            parent_id: 99,
            name: "orphan.txt".to_string(),
            logical_size: 10,
            allocated_size: Some(10),
            file_attributes: 0x20,
            reparse_tag: 0,
            creation_time_utc_ms: 0,
            last_write_time_utc_ms: 0,
            last_access_time_utc_ms: 0,
        })
        .unwrap();

    let reader = ObservationReader::new(Cursor::new(buf)).unwrap();
    let err = build_graph_from_reader(reader).unwrap_err();
    match err {
        GraphBuildError::MissingParent {
            entry_id,
            parent_id,
        } => {
            assert_eq!(entry_id, 2);
            assert_eq!(parent_id, 99);
        }
        other => panic!("expected MissingParent, got {:?}", other),
    }
}

#[test]
fn test_invalid_root_rejected() {
    // 1. Root with entry_id != 1
    let mut buf = Vec::new();
    let mut writer = ObservationWriter::new(&mut buf, r"C:TestRoot").unwrap();

    writer
        .write_directory(&DirectoryObservation {
            entry_id: 2,
            parent_id: 0,
            name: "Root".to_string(),
            file_attributes: 0x10,
            reparse_tag: 0,
            creation_time_utc_ms: 0,
            last_write_time_utc_ms: 0,
            last_access_time_utc_ms: 0,
        })
        .unwrap();

    let reader = ObservationReader::new(Cursor::new(buf)).unwrap();
    let err = build_graph_from_reader(reader).unwrap_err();
    assert!(matches!(
        err,
        GraphBuildError::InvalidRoot {
            entry_id: 2,
            parent_id: 0
        }
    ));

    // 2. Root with parent_id != 0
    let mut buf2 = Vec::new();
    let mut writer2 = ObservationWriter::new(&mut buf2, r"C:TestRoot").unwrap();

    writer2
        .write_directory(&DirectoryObservation {
            entry_id: 1,
            parent_id: 5,
            name: "Root".to_string(),
            file_attributes: 0x10,
            reparse_tag: 0,
            creation_time_utc_ms: 0,
            last_write_time_utc_ms: 0,
            last_access_time_utc_ms: 0,
        })
        .unwrap();

    let reader2 = ObservationReader::new(Cursor::new(buf2)).unwrap();
    let err2 = build_graph_from_reader(reader2).unwrap_err();
    assert!(matches!(
        err2,
        GraphBuildError::InvalidRoot {
            entry_id: 1,
            parent_id: 5
        }
    ));
}

#[test]
fn test_parent_not_directory_rejected() {
    let mut buf = Vec::new();
    let mut writer = ObservationWriter::new(&mut buf, r"C:TestRoot").unwrap();

    writer
        .write_directory(&DirectoryObservation {
            entry_id: 1,
            parent_id: 0,
            name: "Root".to_string(),
            file_attributes: 0x10,
            reparse_tag: 0,
            creation_time_utc_ms: 0,
            last_write_time_utc_ms: 0,
            last_access_time_utc_ms: 0,
        })
        .unwrap();

    // Entry 2 is a File
    writer
        .write_file(&FileObservation {
            entry_id: 2,
            parent_id: 1,
            name: "file.txt".to_string(),
            logical_size: 100,
            allocated_size: Some(100),
            file_attributes: 0x20,
            reparse_tag: 0,
            creation_time_utc_ms: 0,
            last_write_time_utc_ms: 0,
            last_access_time_utc_ms: 0,
        })
        .unwrap();

    // Entry 3 tries to have File (entry 2) as its parent
    writer
        .write_file(&FileObservation {
            entry_id: 3,
            parent_id: 2,
            name: "child_of_file.txt".to_string(),
            logical_size: 50,
            allocated_size: Some(50),
            file_attributes: 0x20,
            reparse_tag: 0,
            creation_time_utc_ms: 0,
            last_write_time_utc_ms: 0,
            last_access_time_utc_ms: 0,
        })
        .unwrap();

    let reader = ObservationReader::new(Cursor::new(buf)).unwrap();
    let err = build_graph_from_reader(reader).unwrap_err();
    assert!(matches!(
        err,
        GraphBuildError::ParentNotDirectory {
            entry_id: 3,
            parent_id: 2
        }
    ));
}

#[test]
fn test_aggregate_mismatch_rejected() {
    // Mismatch on directory count
    let mut buf = Vec::new();
    let mut writer = ObservationWriter::new(&mut buf, r"C:TestRoot").unwrap();

    writer
        .write_directory(&DirectoryObservation {
            entry_id: 1,
            parent_id: 0,
            name: "Root".to_string(),
            file_attributes: 0x10,
            reparse_tag: 0,
            creation_time_utc_ms: 0,
            last_write_time_utc_ms: 0,
            last_access_time_utc_ms: 0,
        })
        .unwrap();

    writer
        .write_terminal(&TerminalObservation {
            outcome: RunOutcome::Finished,
            total_directories: 5, // Expected 1
            total_files: 0,
            total_logical_bytes: 0,
            total_allocated_bytes: 0,
            coverage_gap_count: 0,
            duration_ms: 10,
        })
        .unwrap();

    let reader = ObservationReader::new(Cursor::new(buf)).unwrap();
    let err = build_graph_from_reader(reader).unwrap_err();
    assert!(matches!(
        err,
        GraphBuildError::AggregateMismatch {
            field: "total_directories",
            expected: 5,
            actual: 1
        }
    ));
}

#[test]
fn test_missing_terminal_rejected() {
    let mut buf = Vec::new();
    let mut writer = ObservationWriter::new(&mut buf, r"C:TestRoot").unwrap();

    writer
        .write_directory(&DirectoryObservation {
            entry_id: 1,
            parent_id: 0,
            name: "Root".to_string(),
            file_attributes: 0x10,
            reparse_tag: 0,
            creation_time_utc_ms: 0,
            last_write_time_utc_ms: 0,
            last_access_time_utc_ms: 0,
        })
        .unwrap();

    // Stream ends cleanly without terminal
    let reader = ObservationReader::new(Cursor::new(buf)).unwrap();
    let err = build_graph_from_reader(reader).unwrap_err();
    assert!(matches!(err, GraphBuildError::MissingTerminal));
}

#[test]
fn test_record_after_terminal_rejected() {
    let mut buf = Vec::new();
    let mut writer = ObservationWriter::new(&mut buf, r"C:TestRoot").unwrap();

    writer
        .write_directory(&DirectoryObservation {
            entry_id: 1,
            parent_id: 0,
            name: "Root".to_string(),
            file_attributes: 0x10,
            reparse_tag: 0,
            creation_time_utc_ms: 0,
            last_write_time_utc_ms: 0,
            last_access_time_utc_ms: 0,
        })
        .unwrap();

    writer
        .write_terminal(&TerminalObservation {
            outcome: RunOutcome::Finished,
            total_directories: 1,
            total_files: 0,
            total_logical_bytes: 0,
            total_allocated_bytes: 0,
            coverage_gap_count: 0,
            duration_ms: 10,
        })
        .unwrap();

    // Record written after terminal
    writer
        .write_file(&FileObservation {
            entry_id: 2,
            parent_id: 1,
            name: "after_terminal.txt".to_string(),
            logical_size: 100,
            allocated_size: Some(100),
            file_attributes: 0x20,
            reparse_tag: 0,
            creation_time_utc_ms: 0,
            last_write_time_utc_ms: 0,
            last_access_time_utc_ms: 0,
        })
        .unwrap();

    let reader = ObservationReader::new(Cursor::new(buf)).unwrap();
    let err = build_graph_from_reader(reader).unwrap_err();
    assert!(matches!(err, GraphBuildError::RecordAfterTerminal));
}

#[test]
fn test_corrupt_or_truncated_stream_fails_cleanly() {
    // 1. Truncated mid-record
    let mut buf = Vec::new();
    let mut writer = ObservationWriter::new(&mut buf, r"C:TestRoot").unwrap();

    writer
        .write_directory(&DirectoryObservation {
            entry_id: 1,
            parent_id: 0,
            name: "Root".to_string(),
            file_attributes: 0x10,
            reparse_tag: 0,
            creation_time_utc_ms: 0,
            last_write_time_utc_ms: 0,
            last_access_time_utc_ms: 0,
        })
        .unwrap();

    // Truncate the buffer
    let truncated_len = buf.len() - 3;
    let truncated_buf = buf[..truncated_len].to_vec();

    let reader = ObservationReader::new(Cursor::new(truncated_buf)).unwrap();
    let err = build_graph_from_reader(reader).unwrap_err();
    assert!(matches!(err, GraphBuildError::Decode(_)));
}

#[test]
fn test_allocated_zero_preserved_as_known() {
    let mut buf = Vec::new();
    let mut writer = ObservationWriter::new(&mut buf, r"C:TestRoot").unwrap();

    writer
        .write_directory(&DirectoryObservation {
            entry_id: 1,
            parent_id: 0,
            name: "Root".to_string(),
            file_attributes: 0x10,
            reparse_tag: 0,
            creation_time_utc_ms: 0,
            last_write_time_utc_ms: 0,
            last_access_time_utc_ms: 0,
        })
        .unwrap();

    writer
        .write_file(&FileObservation {
            entry_id: 2,
            parent_id: 1,
            name: "sparse_zero_alloc.dat".to_string(),
            logical_size: 1048576,
            allocated_size: Some(0), // Explicit Known 0
            file_attributes: 0x200,
            reparse_tag: 0,
            creation_time_utc_ms: 0,
            last_write_time_utc_ms: 0,
            last_access_time_utc_ms: 0,
        })
        .unwrap();

    writer
        .write_terminal(&TerminalObservation {
            outcome: RunOutcome::Finished,
            total_directories: 1,
            total_files: 1,
            total_logical_bytes: 1048576,
            total_allocated_bytes: 0,
            coverage_gap_count: 0,
            duration_ms: 10,
        })
        .unwrap();

    let reader = ObservationReader::new(Cursor::new(buf)).unwrap();
    let graph = build_graph_from_reader(reader).unwrap();
    let file_entry = graph.entry(2).unwrap();
    assert_eq!(file_entry.logical_size, Some(1048576));
    assert_eq!(
        file_entry.allocated_size,
        Some(0),
        "allocated_size Some(0) must not be converted to None or inferred differently"
    );
    assert!(graph.allocated_bytes_known());
}

#[test]
fn test_allocated_knowledge_empty_files_known_zero() {
    // Empty file set with only directories and special records is known zero
    let mut buf = Vec::new();
    let mut writer = ObservationWriter::new(&mut buf, r"C:EmptyFilesRoot").unwrap();

    writer
        .write_directory(&DirectoryObservation {
            entry_id: 1,
            parent_id: 0,
            name: "Root".to_string(),
            file_attributes: 0x10,
            reparse_tag: 0,
            creation_time_utc_ms: 0,
            last_write_time_utc_ms: 0,
            last_access_time_utc_ms: 0,
        })
        .unwrap();

    writer
        .write_special(&SpecialObservation {
            entry_id: 2,
            parent_id: 1,
            name: "symlink".to_string(),
            file_attributes: 0x400,
            reparse_tag: 0xA000000C,
            creation_time_utc_ms: 0,
            last_write_time_utc_ms: 0,
            last_access_time_utc_ms: 0,
        })
        .unwrap();

    writer
        .write_terminal(&TerminalObservation {
            outcome: RunOutcome::Finished,
            total_directories: 1,
            total_files: 0,
            total_logical_bytes: 0,
            total_allocated_bytes: 0,
            coverage_gap_count: 0,
            duration_ms: 5,
        })
        .unwrap();

    let reader = ObservationReader::new(Cursor::new(buf)).unwrap();
    let graph = build_graph_from_reader(reader).unwrap();
    assert_eq!(graph.terminal().total_allocated_bytes, 0);
    assert!(graph.allocated_bytes_known());
}

#[test]
fn test_allocated_knowledge_all_files_some_known() {
    let mut buf = Vec::new();
    let mut writer = ObservationWriter::new(&mut buf, r"C:AllKnownRoot").unwrap();

    writer
        .write_directory(&DirectoryObservation {
            entry_id: 1,
            parent_id: 0,
            name: "Root".to_string(),
            file_attributes: 0x10,
            reparse_tag: 0,
            creation_time_utc_ms: 0,
            last_write_time_utc_ms: 0,
            last_access_time_utc_ms: 0,
        })
        .unwrap();

    writer
        .write_file(&FileObservation {
            entry_id: 2,
            parent_id: 1,
            name: "file1.txt".to_string(),
            logical_size: 100,
            allocated_size: Some(4096),
            file_attributes: 0x20,
            reparse_tag: 0,
            creation_time_utc_ms: 0,
            last_write_time_utc_ms: 0,
            last_access_time_utc_ms: 0,
        })
        .unwrap();

    writer
        .write_file(&FileObservation {
            entry_id: 3,
            parent_id: 1,
            name: "file2_zero_alloc.txt".to_string(),
            logical_size: 200,
            allocated_size: Some(0),
            file_attributes: 0x20,
            reparse_tag: 0,
            creation_time_utc_ms: 0,
            last_write_time_utc_ms: 0,
            last_access_time_utc_ms: 0,
        })
        .unwrap();

    writer
        .write_special(&SpecialObservation {
            entry_id: 4,
            parent_id: 1,
            name: "special_item".to_string(),
            file_attributes: 0x400,
            reparse_tag: 0xA0000003,
            creation_time_utc_ms: 0,
            last_write_time_utc_ms: 0,
            last_access_time_utc_ms: 0,
        })
        .unwrap();

    writer
        .write_terminal(&TerminalObservation {
            outcome: RunOutcome::Finished,
            total_directories: 1,
            total_files: 2,
            total_logical_bytes: 300,
            total_allocated_bytes: 4096,
            coverage_gap_count: 0,
            duration_ms: 10,
        })
        .unwrap();

    let reader = ObservationReader::new(Cursor::new(buf)).unwrap();
    let graph = build_graph_from_reader(reader).unwrap();
    assert_eq!(graph.terminal().total_allocated_bytes, 4096);
    assert!(graph.allocated_bytes_known());
}

#[test]
fn test_allocated_knowledge_mixed_some_and_none_is_not_known() {
    let mut buf = Vec::new();
    let mut writer = ObservationWriter::new(&mut buf, r"C:MixedRoot").unwrap();

    writer
        .write_directory(&DirectoryObservation {
            entry_id: 1,
            parent_id: 0,
            name: "Root".to_string(),
            file_attributes: 0x10,
            reparse_tag: 0,
            creation_time_utc_ms: 0,
            last_write_time_utc_ms: 0,
            last_access_time_utc_ms: 0,
        })
        .unwrap();

    // File 1: Known allocated size
    writer
        .write_file(&FileObservation {
            entry_id: 2,
            parent_id: 1,
            name: "known1.bin".to_string(),
            logical_size: 500,
            allocated_size: Some(1024),
            file_attributes: 0x20,
            reparse_tag: 0,
            creation_time_utc_ms: 0,
            last_write_time_utc_ms: 0,
            last_access_time_utc_ms: 0,
        })
        .unwrap();

    // File 2: None (unavailable)
    writer
        .write_file(&FileObservation {
            entry_id: 3,
            parent_id: 1,
            name: "unavail.bin".to_string(),
            logical_size: 700,
            allocated_size: None,
            file_attributes: 0x20,
            reparse_tag: 0,
            creation_time_utc_ms: 0,
            last_write_time_utc_ms: 0,
            last_access_time_utc_ms: 0,
        })
        .unwrap();

    // File 3: Known allocated size
    writer
        .write_file(&FileObservation {
            entry_id: 4,
            parent_id: 1,
            name: "known2.bin".to_string(),
            logical_size: 300,
            allocated_size: Some(2048),
            file_attributes: 0x20,
            reparse_tag: 0,
            creation_time_utc_ms: 0,
            last_write_time_utc_ms: 0,
            last_access_time_utc_ms: 0,
        })
        .unwrap();

    // Terminal allocated bytes must match sum of known Some values (1024 + 2048 = 3072)
    writer
        .write_terminal(&TerminalObservation {
            outcome: RunOutcome::Finished,
            total_directories: 1,
            total_files: 3,
            total_logical_bytes: 1500,
            total_allocated_bytes: 3072,
            coverage_gap_count: 0,
            duration_ms: 20,
        })
        .unwrap();

    let reader = ObservationReader::new(Cursor::new(buf)).unwrap();
    let graph = build_graph_from_reader(reader).unwrap();
    assert_eq!(graph.terminal().total_allocated_bytes, 3072);
    assert!(!graph.allocated_bytes_known());
}

#[test]
fn test_allocated_knowledge_all_none_is_not_known() {
    let mut buf = Vec::new();
    let mut writer = ObservationWriter::new(&mut buf, r"C:AllNoneRoot").unwrap();

    writer
        .write_directory(&DirectoryObservation {
            entry_id: 1,
            parent_id: 0,
            name: "Root".to_string(),
            file_attributes: 0x10,
            reparse_tag: 0,
            creation_time_utc_ms: 0,
            last_write_time_utc_ms: 0,
            last_access_time_utc_ms: 0,
        })
        .unwrap();

    writer
        .write_file(&FileObservation {
            entry_id: 2,
            parent_id: 1,
            name: "unavail1.bin".to_string(),
            logical_size: 500,
            allocated_size: None,
            file_attributes: 0x20,
            reparse_tag: 0,
            creation_time_utc_ms: 0,
            last_write_time_utc_ms: 0,
            last_access_time_utc_ms: 0,
        })
        .unwrap();

    writer
        .write_file(&FileObservation {
            entry_id: 3,
            parent_id: 1,
            name: "unavail2.bin".to_string(),
            logical_size: 700,
            allocated_size: None,
            file_attributes: 0x20,
            reparse_tag: 0,
            creation_time_utc_ms: 0,
            last_write_time_utc_ms: 0,
            last_access_time_utc_ms: 0,
        })
        .unwrap();

    writer
        .write_terminal(&TerminalObservation {
            outcome: RunOutcome::Finished,
            total_directories: 1,
            total_files: 2,
            total_logical_bytes: 1200,
            total_allocated_bytes: 0,
            coverage_gap_count: 0,
            duration_ms: 15,
        })
        .unwrap();

    let reader = ObservationReader::new(Cursor::new(buf)).unwrap();
    let graph = build_graph_from_reader(reader).unwrap();
    assert_eq!(graph.terminal().total_allocated_bytes, 0);
    assert!(!graph.allocated_bytes_known());
}

#[test]
fn test_incremental_builder_and_self_parent_rejection() {
    let mut builder = GraphBuilder::new(r"C:CustomTarget");
    let err1 = builder
        .ingest_record(ObservationRecord::Directory(DirectoryObservation {
            entry_id: 1,
            parent_id: 1, // Self-parent on root
            name: "Root".to_string(),
            file_attributes: 0x10,
            reparse_tag: 0,
            creation_time_utc_ms: 0,
            last_write_time_utc_ms: 0,
            last_access_time_utc_ms: 0,
        }))
        .unwrap_err();
    assert!(matches!(err1, GraphBuildError::SelfParent(1)));

    // Valid root
    builder
        .ingest_record(ObservationRecord::Directory(DirectoryObservation {
            entry_id: 1,
            parent_id: 0,
            name: "Root".to_string(),
            file_attributes: 0x10,
            reparse_tag: 0,
            creation_time_utc_ms: 0,
            last_write_time_utc_ms: 0,
            last_access_time_utc_ms: 0,
        }))
        .unwrap();

    // Self-parent on child
    let err2 = builder
        .ingest_record(ObservationRecord::Directory(DirectoryObservation {
            entry_id: 2,
            parent_id: 2, // Self-parent
            name: "Child".to_string(),
            file_attributes: 0x10,
            reparse_tag: 0,
            creation_time_utc_ms: 0,
            last_write_time_utc_ms: 0,
            last_access_time_utc_ms: 0,
        }))
        .unwrap_err();
    assert!(matches!(err2, GraphBuildError::SelfParent(2)));
}

#[test]
fn test_graph_entry_accessors_and_kinds() {
    assert!(EntryKind::Directory.is_directory());
    assert!(!EntryKind::Directory.is_file());
    assert!(!EntryKind::Directory.is_special());

    assert!(!EntryKind::File.is_directory());
    assert!(EntryKind::File.is_file());
    assert!(!EntryKind::File.is_special());

    assert!(!EntryKind::Special.is_directory());
    assert!(!EntryKind::Special.is_file());
    assert!(EntryKind::Special.is_special());
}

#[test]
fn test_graph_children_pagination_and_node_fields() {
    let mut buf = Vec::new();
    let mut writer = ObservationWriter::new(&mut buf, r"C:HierarchyTest").unwrap();

    // 1. Root (id=1, parent=0)
    writer
        .write_directory(&DirectoryObservation {
            entry_id: 1,
            parent_id: 0,
            name: "Root".to_string(),
            file_attributes: 0x10,
            reparse_tag: 0,
            creation_time_utc_ms: 10,
            last_write_time_utc_ms: 20,
            last_access_time_utc_ms: 30,
        })
        .unwrap();

    // 2. DirA (id=2, parent=1)
    writer
        .write_directory(&DirectoryObservation {
            entry_id: 2,
            parent_id: 1,
            name: "DirA".to_string(),
            file_attributes: 0x10,
            reparse_tag: 0,
            creation_time_utc_ms: 11,
            last_write_time_utc_ms: 21,
            last_access_time_utc_ms: 31,
        })
        .unwrap();

    // 3. File in DirA (id=3, parent=2)
    writer
        .write_file(&FileObservation {
            entry_id: 3,
            parent_id: 2,
            name: "file_a.txt".to_string(),
            logical_size: 1000,
            allocated_size: Some(4096),
            file_attributes: 0x20,
            reparse_tag: 0,
            creation_time_utc_ms: 12,
            last_write_time_utc_ms: 22,
            last_access_time_utc_ms: 32,
        })
        .unwrap();

    // 4. File in Root (id=4, parent=1)
    writer
        .write_file(&FileObservation {
            entry_id: 4,
            parent_id: 1,
            name: "file_root.bin".to_string(),
            logical_size: 500,
            allocated_size: Some(512),
            file_attributes: 0x20,
            reparse_tag: 0,
            creation_time_utc_ms: 13,
            last_write_time_utc_ms: 23,
            last_access_time_utc_ms: 33,
        })
        .unwrap();

    writer
        .write_terminal(&TerminalObservation {
            outcome: RunOutcome::Finished,
            total_directories: 2,
            total_files: 2,
            total_logical_bytes: 1500,
            total_allocated_bytes: 4608,
            coverage_gap_count: 0,
            duration_ms: 50,
        })
        .unwrap();

    let reader = ObservationReader::new(Cursor::new(buf)).unwrap();
    let graph = build_graph_from_reader(reader).unwrap();

    // Test virtual parent 0 query (returns root)
    let (total_root, root_nodes) = graph.get_children_page(0, 0, 10).unwrap();
    assert_eq!(total_root, 1);
    assert_eq!(root_nodes.len(), 1);
    assert_eq!(root_nodes[0].id, 1);
    assert_eq!(root_nodes[0].parent_id, 0);
    assert_eq!(root_nodes[0].name, "Root");
    assert_eq!(root_nodes[0].entry_kind, 1);
    assert_eq!(root_nodes[0].child_count, 2);
    assert!(root_nodes[0].has_children);

    // Test querying root's children (parent_id = 1)
    let (total_children, children) = graph.get_children_page(1, 0, 100).unwrap();
    assert_eq!(total_children, 2);
    assert_eq!(children.len(), 2);
    // Directories first: DirA (id=2) then file_root.bin (id=4)
    assert_eq!(children[0].id, 2);
    assert_eq!(children[0].parent_id, 1);
    assert_eq!(children[0].name, "DirA");
    assert_eq!(children[0].entry_kind, 1);
    assert_eq!(children[0].child_count, 1);
    assert!(children[0].has_children);

    assert_eq!(children[1].id, 4);
    assert_eq!(children[1].parent_id, 1);
    assert_eq!(children[1].name, "file_root.bin");
    assert_eq!(children[1].entry_kind, 2);
    assert_eq!(children[1].logical_size, 500);
    assert_eq!(children[1].allocated_size, 512);
    assert!(children[1].allocated_size_known);
    assert_eq!(children[1].child_count, 0);
    assert!(!children[1].has_children);

    // Test querying DirA's children (parent_id = 2)
    let (total_dira, dira_children) = graph.get_children_page(2, 0, 10).unwrap();
    assert_eq!(total_dira, 1);
    assert_eq!(dira_children.len(), 1);
    assert_eq!(dira_children[0].id, 3);
    assert_eq!(dira_children[0].name, "file_a.txt");
    assert_eq!(dira_children[0].logical_size, 1000);
    assert_eq!(dira_children[0].allocated_size, 4096);
    assert!(dira_children[0].allocated_size_known);
    assert_eq!(dira_children[0].child_count, 0);
    assert!(!dira_children[0].has_children);

    // Test invalid parent (non-existent)
    assert!(graph.get_children_page(999, 0, 10).is_err());

    // Test non-directory parent (file id=3)
    assert!(graph.get_children_page(3, 0, 10).is_err());
}

#[test]
fn test_graph_stable_ordering_rules() {
    let mut buf = Vec::new();
    let mut writer = ObservationWriter::new(&mut buf, r"C:SortTest").unwrap();

    // Root (id=1)
    writer
        .write_directory(&DirectoryObservation {
            entry_id: 1,
            parent_id: 0,
            name: "Root".to_string(),
            file_attributes: 0x10,
            reparse_tag: 0,
            creation_time_utc_ms: 10,
            last_write_time_utc_ms: 20,
            last_access_time_utc_ms: 30,
        })
        .unwrap();

    // 1. Dir "zeta"
    writer
        .write_directory(&DirectoryObservation {
            entry_id: 2,
            parent_id: 1,
            name: "zeta".to_string(),
            file_attributes: 0x10,
            reparse_tag: 0,
            creation_time_utc_ms: 11,
            last_write_time_utc_ms: 21,
            last_access_time_utc_ms: 31,
        })
        .unwrap();

    // 2. Dir "Alpha"
    writer
        .write_directory(&DirectoryObservation {
            entry_id: 3,
            parent_id: 1,
            name: "Alpha".to_string(),
            file_attributes: 0x10,
            reparse_tag: 0,
            creation_time_utc_ms: 13,
            last_write_time_utc_ms: 23,
            last_access_time_utc_ms: 33,
        })
        .unwrap();

    // 3. Dir "beta"
    writer
        .write_directory(&DirectoryObservation {
            entry_id: 4,
            parent_id: 1,
            name: "beta".to_string(),
            file_attributes: 0x10,
            reparse_tag: 0,
            creation_time_utc_ms: 15,
            last_write_time_utc_ms: 25,
            last_access_time_utc_ms: 35,
        })
        .unwrap();

    // 4. File "zoo.txt" logical size 5000 (larger than dirs, but dirs must come first!)
    writer
        .write_file(&FileObservation {
            entry_id: 5,
            parent_id: 1,
            name: "zoo.txt".to_string(),
            logical_size: 5000,
            allocated_size: Some(8192),
            file_attributes: 0x20,
            reparse_tag: 0,
            creation_time_utc_ms: 17,
            last_write_time_utc_ms: 27,
            last_access_time_utc_ms: 37,
        })
        .unwrap();

    // 5. File "apple.txt" logical size 100
    writer
        .write_file(&FileObservation {
            entry_id: 6,
            parent_id: 1,
            name: "apple.txt".to_string(),
            logical_size: 100,
            allocated_size: Some(512),
            file_attributes: 0x20,
            reparse_tag: 0,
            creation_time_utc_ms: 18,
            last_write_time_utc_ms: 28,
            last_access_time_utc_ms: 38,
        })
        .unwrap();

    // 6. File "Banana.txt" logical size 100 (same size as apple, test case-insensitive sorting)
    writer
        .write_file(&FileObservation {
            entry_id: 7,
            parent_id: 1,
            name: "Banana.txt".to_string(),
            logical_size: 100,
            allocated_size: Some(512),
            file_attributes: 0x20,
            reparse_tag: 0,
            creation_time_utc_ms: 19,
            last_write_time_utc_ms: 29,
            last_access_time_utc_ms: 39,
        })
        .unwrap();

    // 7. File "apple_small.txt" logical size 50 (smaller size than apple.txt)
    writer
        .write_file(&FileObservation {
            entry_id: 8,
            parent_id: 1,
            name: "apple_small.txt".to_string(),
            logical_size: 50,
            allocated_size: Some(512),
            file_attributes: 0x20,
            reparse_tag: 0,
            creation_time_utc_ms: 20,
            last_write_time_utc_ms: 30,
            last_access_time_utc_ms: 40,
        })
        .unwrap();

    writer
        .write_terminal(&TerminalObservation {
            outcome: RunOutcome::Finished,
            total_directories: 4,
            total_files: 4,
            total_logical_bytes: 5250,
            total_allocated_bytes: 9728,
            coverage_gap_count: 0,
            duration_ms: 50,
        })
        .unwrap();

    let reader = ObservationReader::new(Cursor::new(buf)).unwrap();
    let graph = build_graph_from_reader(reader).unwrap();

    let (total, children) = graph.get_children_page(1, 0, 100).unwrap();
    assert_eq!(total, 7);

    // Expected order:
    // Dirs first (alphabetical case-insensitive):
    // 1. Alpha
    // 2. beta
    // 3. zeta
    // Files second:
    // 4. zoo.txt (logical size 5000)
    // 5. apple.txt (logical size 100, "apple.txt" < "Banana.txt")
    // 6. Banana.txt (logical size 100)
    // 7. apple_small.txt (logical size 50)
    assert_eq!(children[0].name, "Alpha");
    assert_eq!(children[1].name, "beta");
    assert_eq!(children[2].name, "zeta");
    assert_eq!(children[3].name, "zoo.txt");
    assert_eq!(children[4].name, "apple.txt");
    assert_eq!(children[5].name, "Banana.txt");
    assert_eq!(children[6].name, "apple_small.txt");

    // Test offset and limit pagination slicing
    let (total_p1, page1) = graph.get_children_page(1, 0, 2).unwrap();
    assert_eq!(total_p1, 7);
    assert_eq!(page1.len(), 2);
    assert_eq!(page1[0].name, "Alpha");
    assert_eq!(page1[1].name, "beta");

    let (total_p2, page2) = graph.get_children_page(1, 2, 2).unwrap();
    assert_eq!(total_p2, 7);
    assert_eq!(page2.len(), 2);
    assert_eq!(page2[0].name, "zeta");
    assert_eq!(page2[1].name, "zoo.txt");

    let (total_p3, page3) = graph.get_children_page(1, 4, 10).unwrap();
    assert_eq!(total_p3, 7);
    assert_eq!(page3.len(), 3);
    assert_eq!(page3[0].name, "apple.txt");
    assert_eq!(page3[1].name, "Banana.txt");
    assert_eq!(page3[2].name, "apple_small.txt");

    // Offset beyond total
    let (total_p4, page4) = graph.get_children_page(1, 10, 10).unwrap();
    assert_eq!(total_p4, 7);
    assert!(page4.is_empty());
}

#[test]
fn test_graph_builder_progress_nested_path_reconstruction() {
    let mut builder = GraphBuilder::new(r"C:\ProjectRoot");

    // Fallback on empty builder before any records
    assert_eq!(builder.current_directory_path(), r"C:\ProjectRoot");

    // 1. Ingest root directory
    builder
        .ingest_record(ObservationRecord::Directory(DirectoryObservation {
            entry_id: 1,
            parent_id: 0,
            name: r"C:\ProjectRoot".to_string(),
            file_attributes: 0x10,
            reparse_tag: 0,
            creation_time_utc_ms: 100,
            last_write_time_utc_ms: 200,
            last_access_time_utc_ms: 300,
        }))
        .unwrap();
    assert_eq!(builder.current_directory_path(), r"C:\ProjectRoot");

    // 2. Ingest file in root (parent_id = 1)
    builder
        .ingest_record(ObservationRecord::File(FileObservation {
            entry_id: 2,
            parent_id: 1,
            name: "root_file.txt".to_string(),
            logical_size: 100,
            allocated_size: Some(512),
            file_attributes: 0x20,
            reparse_tag: 0,
            creation_time_utc_ms: 101,
            last_write_time_utc_ms: 201,
            last_access_time_utc_ms: 301,
        }))
        .unwrap();
    assert_eq!(builder.current_directory_path(), r"C:\ProjectRoot");

    // 3. Ingest FolderA in root (parent_id = 1)
    builder
        .ingest_record(ObservationRecord::Directory(DirectoryObservation {
            entry_id: 3,
            parent_id: 1,
            name: "FolderA".to_string(),
            file_attributes: 0x10,
            reparse_tag: 0,
            creation_time_utc_ms: 102,
            last_write_time_utc_ms: 202,
            last_access_time_utc_ms: 302,
        }))
        .unwrap();
    assert_eq!(builder.current_directory_path(), r"C:\ProjectRoot");

    // 4. Ingest file inside FolderA (parent_id = 3)
    builder
        .ingest_record(ObservationRecord::File(FileObservation {
            entry_id: 4,
            parent_id: 3,
            name: "nested_file.txt".to_string(),
            logical_size: 200,
            allocated_size: Some(512),
            file_attributes: 0x20,
            reparse_tag: 0,
            creation_time_utc_ms: 103,
            last_write_time_utc_ms: 203,
            last_access_time_utc_ms: 303,
        }))
        .unwrap();
    assert_eq!(builder.current_directory_path(), r"C:\ProjectRoot\FolderA");

    // 5. Ingest FolderB inside FolderA (parent_id = 3)
    builder
        .ingest_record(ObservationRecord::Directory(DirectoryObservation {
            entry_id: 5,
            parent_id: 3,
            name: "FolderB".to_string(),
            file_attributes: 0x10,
            reparse_tag: 0,
            creation_time_utc_ms: 104,
            last_write_time_utc_ms: 204,
            last_access_time_utc_ms: 304,
        }))
        .unwrap();
    assert_eq!(builder.current_directory_path(), r"C:\ProjectRoot\FolderA");

    // 6. Ingest deep file inside FolderB (parent_id = 5)
    builder
        .ingest_record(ObservationRecord::File(FileObservation {
            entry_id: 6,
            parent_id: 5,
            name: "deep_file.txt".to_string(),
            logical_size: 300,
            allocated_size: Some(512),
            file_attributes: 0x20,
            reparse_tag: 0,
            creation_time_utc_ms: 105,
            last_write_time_utc_ms: 205,
            last_access_time_utc_ms: 305,
        }))
        .unwrap();
    assert_eq!(
        builder.current_directory_path(),
        r"C:\ProjectRoot\FolderA\FolderB"
    );
}

#[test]
fn test_graph_builder_progress_root_trailing_slash_nested_path() {
    let mut builder = GraphBuilder::new(r"C:\");

    assert_eq!(builder.current_directory_path(), r"C:\");

    // 1. Ingest root directory C:\
    builder
        .ingest_record(ObservationRecord::Directory(DirectoryObservation {
            entry_id: 1,
            parent_id: 0,
            name: r"C:\".to_string(),
            file_attributes: 0x10,
            reparse_tag: 0,
            creation_time_utc_ms: 100,
            last_write_time_utc_ms: 200,
            last_access_time_utc_ms: 300,
        }))
        .unwrap();
    assert_eq!(builder.current_directory_path(), r"C:\");

    // 2. Ingest Users dir (parent_id = 1)
    builder
        .ingest_record(ObservationRecord::Directory(DirectoryObservation {
            entry_id: 2,
            parent_id: 1,
            name: "Users".to_string(),
            file_attributes: 0x10,
            reparse_tag: 0,
            creation_time_utc_ms: 101,
            last_write_time_utc_ms: 201,
            last_access_time_utc_ms: 301,
        }))
        .unwrap();

    // 3. Ingest file in Users (parent_id = 2)
    builder
        .ingest_record(ObservationRecord::File(FileObservation {
            entry_id: 3,
            parent_id: 2,
            name: "user_file.txt".to_string(),
            logical_size: 50,
            allocated_size: Some(512),
            file_attributes: 0x20,
            reparse_tag: 0,
            creation_time_utc_ms: 102,
            last_write_time_utc_ms: 202,
            last_access_time_utc_ms: 302,
        }))
        .unwrap();
    assert_eq!(builder.current_directory_path(), r"C:\Users");

    // 4. Ingest sub dir inside Users (parent_id = 2)
    builder
        .ingest_record(ObservationRecord::Directory(DirectoryObservation {
            entry_id: 4,
            parent_id: 2,
            name: "testuser".to_string(),
            file_attributes: 0x10,
            reparse_tag: 0,
            creation_time_utc_ms: 103,
            last_write_time_utc_ms: 203,
            last_access_time_utc_ms: 303,
        }))
        .unwrap();

    // 5. Ingest file inside testuser (parent_id = 4)
    builder
        .ingest_record(ObservationRecord::File(FileObservation {
            entry_id: 5,
            parent_id: 4,
            name: "data.json".to_string(),
            logical_size: 150,
            allocated_size: Some(512),
            file_attributes: 0x20,
            reparse_tag: 0,
            creation_time_utc_ms: 104,
            last_write_time_utc_ms: 204,
            last_access_time_utc_ms: 304,
        }))
        .unwrap();
    assert_eq!(builder.current_directory_path(), r"C:\Users\testuser");
}

#[test]
fn test_graph_builder_build_from_reader_with_progress_emits_truthful_current_directory() {
    let mut buf = Vec::new();
    let mut writer = ObservationWriter::new(&mut buf, r"C:\Data\Target").unwrap();

    writer
        .write_directory(&DirectoryObservation {
            entry_id: 1,
            parent_id: 0,
            name: r"C:\Data\Target".to_string(),
            file_attributes: 0x10,
            reparse_tag: 0,
            creation_time_utc_ms: 100,
            last_write_time_utc_ms: 200,
            last_access_time_utc_ms: 300,
        })
        .unwrap();

    writer
        .write_directory(&DirectoryObservation {
            entry_id: 2,
            parent_id: 1,
            name: "Sub1".to_string(),
            file_attributes: 0x10,
            reparse_tag: 0,
            creation_time_utc_ms: 101,
            last_write_time_utc_ms: 201,
            last_access_time_utc_ms: 301,
        })
        .unwrap();

    writer
        .write_file(&FileObservation {
            entry_id: 3,
            parent_id: 2,
            name: "f1.txt".to_string(),
            logical_size: 50,
            allocated_size: Some(512),
            file_attributes: 0x20,
            reparse_tag: 0,
            creation_time_utc_ms: 102,
            last_write_time_utc_ms: 202,
            last_access_time_utc_ms: 302,
        })
        .unwrap();

    writer
        .write_terminal(&TerminalObservation {
            outcome: RunOutcome::Finished,
            total_directories: 2,
            total_files: 1,
            total_logical_bytes: 50,
            total_allocated_bytes: 512,
            coverage_gap_count: 0,
            duration_ms: 10,
        })
        .unwrap();

    let reader = ObservationReader::new(Cursor::new(buf)).unwrap();
    let mut progress_events = Vec::new();
    let _graph = GraphBuilder::build_from_reader_with_progress(
        reader,
        "op-100",
        Some(|p| {
            progress_events.push(p);
        }),
    )
    .unwrap();

    assert!(!progress_events.is_empty());
    for p in &progress_events {
        assert_eq!(p.operation_id, "op-100");
        assert!(!p.current_directory.is_empty());
        assert!(p.current_directory.starts_with(r"C:\Data\Target"));
    }
}

#[test]
fn test_directory_subtree_aggregate_repro() {
    // Repro case:
    // nested_folder contains inner_file.bin of 5000 B (alloc 8192);
    // root also contains known_file.dat of 1024 B (alloc 4096) and empty_file.txt of 0 B (alloc 0).
    // Expected:
    // root: logical 6024 B, allocated 12288 B, allocated_known true
    // nested_folder: logical 5000 B, allocated 8192 B, allocated_known true
    // known_file.dat: logical 1024 B, allocated 4096 B, allocated_known true
    // empty_file.txt: logical 0 B, allocated 0 B, allocated_known true
    let mut buf = Vec::new();
    let mut writer = ObservationWriter::new(&mut buf, r"C:\ReproRoot").unwrap();

    // 1. Root directory (id=1, parent=0)
    writer
        .write_directory(&DirectoryObservation {
            entry_id: 1,
            parent_id: 0,
            name: "Root".to_string(),
            file_attributes: 0x10,
            reparse_tag: 0,
            creation_time_utc_ms: 10,
            last_write_time_utc_ms: 20,
            last_access_time_utc_ms: 30,
        })
        .unwrap();

    // 2. nested_folder (id=2, parent=1)
    writer
        .write_directory(&DirectoryObservation {
            entry_id: 2,
            parent_id: 1,
            name: "nested_folder".to_string(),
            file_attributes: 0x10,
            reparse_tag: 0,
            creation_time_utc_ms: 11,
            last_write_time_utc_ms: 21,
            last_access_time_utc_ms: 31,
        })
        .unwrap();

    // 3. inner_file.bin (id=3, parent=2)
    writer
        .write_file(&FileObservation {
            entry_id: 3,
            parent_id: 2,
            name: "inner_file.bin".to_string(),
            logical_size: 5000,
            allocated_size: Some(8192),
            file_attributes: 0x20,
            reparse_tag: 0,
            creation_time_utc_ms: 12,
            last_write_time_utc_ms: 22,
            last_access_time_utc_ms: 32,
        })
        .unwrap();

    // 4. known_file.dat (id=4, parent=1)
    writer
        .write_file(&FileObservation {
            entry_id: 4,
            parent_id: 1,
            name: "known_file.dat".to_string(),
            logical_size: 1024,
            allocated_size: Some(4096),
            file_attributes: 0x20,
            reparse_tag: 0,
            creation_time_utc_ms: 13,
            last_write_time_utc_ms: 23,
            last_access_time_utc_ms: 33,
        })
        .unwrap();

    // 5. empty_file.txt (id=5, parent=1)
    writer
        .write_file(&FileObservation {
            entry_id: 5,
            parent_id: 1,
            name: "empty_file.txt".to_string(),
            logical_size: 0,
            allocated_size: Some(0),
            file_attributes: 0x20,
            reparse_tag: 0,
            creation_time_utc_ms: 14,
            last_write_time_utc_ms: 24,
            last_access_time_utc_ms: 34,
        })
        .unwrap();

    writer
        .write_terminal(&TerminalObservation {
            outcome: RunOutcome::Finished,
            total_directories: 2,
            total_files: 3,
            total_logical_bytes: 6024,
            total_allocated_bytes: 12288,
            coverage_gap_count: 0,
            duration_ms: 50,
        })
        .unwrap();

    let reader = ObservationReader::new(Cursor::new(buf)).unwrap();
    let graph = build_graph_from_reader(reader).expect("graph build should succeed");

    // Graph entry direct checks
    assert_eq!(graph.root().logical_size, Some(6024));
    assert_eq!(graph.root().allocated_size, Some(12288));
    assert_eq!(graph.entry(2).unwrap().logical_size, Some(5000));
    assert_eq!(graph.entry(2).unwrap().allocated_size, Some(8192));
    assert_eq!(graph.entry(4).unwrap().logical_size, Some(1024));
    assert_eq!(graph.entry(4).unwrap().allocated_size, Some(4096));
    assert_eq!(graph.entry(5).unwrap().logical_size, Some(0));
    assert_eq!(graph.entry(5).unwrap().allocated_size, Some(0));

    // GetChildren page checks for Root (parent_id = 0 -> root node)
    let (total_root, root_nodes) = graph.get_children_page(0, 0, 10).unwrap();
    assert_eq!(total_root, 1);
    assert_eq!(root_nodes[0].id, 1);
    assert_eq!(root_nodes[0].logical_size, 6024);
    assert_eq!(root_nodes[0].allocated_size, 12288);
    assert!(root_nodes[0].allocated_size_known);

    // GetChildren page checks for Root's children (parent_id = 1)
    let (total_children, children) = graph.get_children_page(1, 0, 100).unwrap();
    assert_eq!(total_children, 3);
    assert_eq!(children.len(), 3);

    // Directories first: nested_folder (id=2)
    let nested_node = &children[0];
    assert_eq!(nested_node.id, 2);
    assert_eq!(nested_node.name, "nested_folder");
    assert_eq!(nested_node.entry_kind, 1);
    assert_eq!(nested_node.logical_size, 5000);
    assert_eq!(nested_node.allocated_size, 8192);
    assert!(nested_node.allocated_size_known);
    assert_eq!(nested_node.child_count, 1);
    assert!(nested_node.has_children);

    // Files sorted by logical size descending: known_file.dat (1024) then empty_file.txt (0)
    let known_node = &children[1];
    assert_eq!(known_node.id, 4);
    assert_eq!(known_node.name, "known_file.dat");
    assert_eq!(known_node.entry_kind, 2);
    assert_eq!(known_node.logical_size, 1024);
    assert_eq!(known_node.allocated_size, 4096);
    assert!(known_node.allocated_size_known);
    assert_eq!(known_node.child_count, 0);
    assert!(!known_node.has_children);

    let empty_node = &children[2];
    assert_eq!(empty_node.id, 5);
    assert_eq!(empty_node.name, "empty_file.txt");
    assert_eq!(empty_node.entry_kind, 2);
    assert_eq!(empty_node.logical_size, 0);
    assert_eq!(empty_node.allocated_size, 0);
    assert!(empty_node.allocated_size_known);
    assert_eq!(empty_node.child_count, 0);
    assert!(!empty_node.has_children);
}

#[test]
fn test_directory_aggregates_nested_depth() {
    let mut buf = Vec::new();
    let mut writer = ObservationWriter::new(&mut buf, r"C:\DepthRoot").unwrap();

    // Hierarchy:
    // Root (id=1)
    //   Level1 (id=2, parent=1)
    //     level1_file.bin (id=3, parent=2): logical 2222, alloc 4096
    //     Level2 (id=4, parent=2)
    //       sibling_file.bin (id=5, parent=4): logical 1111, alloc 2048
    //       Level3 (id=6, parent=4)
    //         leaf_file.bin (id=7, parent=6): logical 3333, alloc 4096

    writer
        .write_directory(&DirectoryObservation {
            entry_id: 1,
            parent_id: 0,
            name: "Root".to_string(),
            file_attributes: 0x10,
            reparse_tag: 0,
            creation_time_utc_ms: 0,
            last_write_time_utc_ms: 0,
            last_access_time_utc_ms: 0,
        })
        .unwrap();

    writer
        .write_directory(&DirectoryObservation {
            entry_id: 2,
            parent_id: 1,
            name: "Level1".to_string(),
            file_attributes: 0x10,
            reparse_tag: 0,
            creation_time_utc_ms: 0,
            last_write_time_utc_ms: 0,
            last_access_time_utc_ms: 0,
        })
        .unwrap();

    writer
        .write_file(&FileObservation {
            entry_id: 3,
            parent_id: 2,
            name: "level1_file.bin".to_string(),
            logical_size: 2222,
            allocated_size: Some(4096),
            file_attributes: 0x20,
            reparse_tag: 0,
            creation_time_utc_ms: 0,
            last_write_time_utc_ms: 0,
            last_access_time_utc_ms: 0,
        })
        .unwrap();

    writer
        .write_directory(&DirectoryObservation {
            entry_id: 4,
            parent_id: 2,
            name: "Level2".to_string(),
            file_attributes: 0x10,
            reparse_tag: 0,
            creation_time_utc_ms: 0,
            last_write_time_utc_ms: 0,
            last_access_time_utc_ms: 0,
        })
        .unwrap();

    writer
        .write_file(&FileObservation {
            entry_id: 5,
            parent_id: 4,
            name: "sibling_file.bin".to_string(),
            logical_size: 1111,
            allocated_size: Some(2048),
            file_attributes: 0x20,
            reparse_tag: 0,
            creation_time_utc_ms: 0,
            last_write_time_utc_ms: 0,
            last_access_time_utc_ms: 0,
        })
        .unwrap();

    writer
        .write_directory(&DirectoryObservation {
            entry_id: 6,
            parent_id: 4,
            name: "Level3".to_string(),
            file_attributes: 0x10,
            reparse_tag: 0,
            creation_time_utc_ms: 0,
            last_write_time_utc_ms: 0,
            last_access_time_utc_ms: 0,
        })
        .unwrap();

    writer
        .write_file(&FileObservation {
            entry_id: 7,
            parent_id: 6,
            name: "leaf_file.bin".to_string(),
            logical_size: 3333,
            allocated_size: Some(4096),
            file_attributes: 0x20,
            reparse_tag: 0,
            creation_time_utc_ms: 0,
            last_write_time_utc_ms: 0,
            last_access_time_utc_ms: 0,
        })
        .unwrap();

    writer
        .write_terminal(&TerminalObservation {
            outcome: RunOutcome::Finished,
            total_directories: 4,
            total_files: 3,
            total_logical_bytes: 6666,
            total_allocated_bytes: 10240,
            coverage_gap_count: 0,
            duration_ms: 50,
        })
        .unwrap();

    let reader = ObservationReader::new(Cursor::new(buf)).unwrap();
    let graph = build_graph_from_reader(reader).unwrap();

    assert_eq!(graph.entry(6).unwrap().logical_size, Some(3333));
    assert_eq!(graph.entry(6).unwrap().allocated_size, Some(4096));

    assert_eq!(graph.entry(4).unwrap().logical_size, Some(4444)); // 3333 + 1111
    assert_eq!(graph.entry(4).unwrap().allocated_size, Some(6144)); // 4096 + 2048

    assert_eq!(graph.entry(2).unwrap().logical_size, Some(6666)); // 4444 + 2222
    assert_eq!(graph.entry(2).unwrap().allocated_size, Some(10240)); // 6144 + 4096

    assert_eq!(graph.root().logical_size, Some(6666));
    assert_eq!(graph.root().allocated_size, Some(10240));
}

#[test]
fn test_directory_sorting_by_recursive_size() {
    let mut buf = Vec::new();
    let mut writer = ObservationWriter::new(&mut buf, r"C:\SortDirTest").unwrap();

    // Root (id=1)
    //   SmallDir (id=2) -> small_file (100 B)
    //   BigDir (id=4) -> big_file (10000 B)
    //   MediumDir (id=6) -> medium_file (500 B)

    writer
        .write_directory(&DirectoryObservation {
            entry_id: 1,
            parent_id: 0,
            name: "Root".to_string(),
            file_attributes: 0x10,
            reparse_tag: 0,
            creation_time_utc_ms: 0,
            last_write_time_utc_ms: 0,
            last_access_time_utc_ms: 0,
        })
        .unwrap();

    writer
        .write_directory(&DirectoryObservation {
            entry_id: 2,
            parent_id: 1,
            name: "SmallDir".to_string(),
            file_attributes: 0x10,
            reparse_tag: 0,
            creation_time_utc_ms: 0,
            last_write_time_utc_ms: 0,
            last_access_time_utc_ms: 0,
        })
        .unwrap();

    writer
        .write_file(&FileObservation {
            entry_id: 3,
            parent_id: 2,
            name: "small_file.txt".to_string(),
            logical_size: 100,
            allocated_size: Some(512),
            file_attributes: 0x20,
            reparse_tag: 0,
            creation_time_utc_ms: 0,
            last_write_time_utc_ms: 0,
            last_access_time_utc_ms: 0,
        })
        .unwrap();

    writer
        .write_directory(&DirectoryObservation {
            entry_id: 4,
            parent_id: 1,
            name: "BigDir".to_string(),
            file_attributes: 0x10,
            reparse_tag: 0,
            creation_time_utc_ms: 0,
            last_write_time_utc_ms: 0,
            last_access_time_utc_ms: 0,
        })
        .unwrap();

    writer
        .write_file(&FileObservation {
            entry_id: 5,
            parent_id: 4,
            name: "big_file.txt".to_string(),
            logical_size: 10000,
            allocated_size: Some(16384),
            file_attributes: 0x20,
            reparse_tag: 0,
            creation_time_utc_ms: 0,
            last_write_time_utc_ms: 0,
            last_access_time_utc_ms: 0,
        })
        .unwrap();

    writer
        .write_directory(&DirectoryObservation {
            entry_id: 6,
            parent_id: 1,
            name: "MediumDir".to_string(),
            file_attributes: 0x10,
            reparse_tag: 0,
            creation_time_utc_ms: 0,
            last_write_time_utc_ms: 0,
            last_access_time_utc_ms: 0,
        })
        .unwrap();

    writer
        .write_file(&FileObservation {
            entry_id: 7,
            parent_id: 6,
            name: "medium_file.txt".to_string(),
            logical_size: 500,
            allocated_size: Some(1024),
            file_attributes: 0x20,
            reparse_tag: 0,
            creation_time_utc_ms: 0,
            last_write_time_utc_ms: 0,
            last_access_time_utc_ms: 0,
        })
        .unwrap();

    writer
        .write_terminal(&TerminalObservation {
            outcome: RunOutcome::Finished,
            total_directories: 4,
            total_files: 3,
            total_logical_bytes: 10600,
            total_allocated_bytes: 17920,
            coverage_gap_count: 0,
            duration_ms: 20,
        })
        .unwrap();

    let reader = ObservationReader::new(Cursor::new(buf)).unwrap();
    let graph = build_graph_from_reader(reader).unwrap();

    let (total, children) = graph.get_children_page(1, 0, 10).unwrap();
    assert_eq!(total, 3);
    assert_eq!(children[0].name, "BigDir");
    assert_eq!(children[0].logical_size, 10000);

    assert_eq!(children[1].name, "MediumDir");
    assert_eq!(children[1].logical_size, 500);

    assert_eq!(children[2].name, "SmallDir");
    assert_eq!(children[2].logical_size, 100);
}

#[test]
fn test_empty_directory_aggregates_known_zero() {
    let mut buf = Vec::new();
    let mut writer = ObservationWriter::new(&mut buf, r"C:\EmptyDirTest").unwrap();

    // Root (id=1)
    //   EmptyDir (id=2)
    //   special_link (id=3, parent=1)

    writer
        .write_directory(&DirectoryObservation {
            entry_id: 1,
            parent_id: 0,
            name: "Root".to_string(),
            file_attributes: 0x10,
            reparse_tag: 0,
            creation_time_utc_ms: 0,
            last_write_time_utc_ms: 0,
            last_access_time_utc_ms: 0,
        })
        .unwrap();

    writer
        .write_directory(&DirectoryObservation {
            entry_id: 2,
            parent_id: 1,
            name: "EmptyDir".to_string(),
            file_attributes: 0x10,
            reparse_tag: 0,
            creation_time_utc_ms: 0,
            last_write_time_utc_ms: 0,
            last_access_time_utc_ms: 0,
        })
        .unwrap();

    writer
        .write_special(&SpecialObservation {
            entry_id: 3,
            parent_id: 1,
            name: "special_link".to_string(),
            file_attributes: 0x400,
            reparse_tag: 0xA000000C,
            creation_time_utc_ms: 0,
            last_write_time_utc_ms: 0,
            last_access_time_utc_ms: 0,
        })
        .unwrap();

    writer
        .write_terminal(&TerminalObservation {
            outcome: RunOutcome::Finished,
            total_directories: 2,
            total_files: 0,
            total_logical_bytes: 0,
            total_allocated_bytes: 0,
            coverage_gap_count: 0,
            duration_ms: 10,
        })
        .unwrap();

    let reader = ObservationReader::new(Cursor::new(buf)).unwrap();
    let graph = build_graph_from_reader(reader).unwrap();

    assert_eq!(graph.root().logical_size, Some(0));
    assert_eq!(graph.root().allocated_size, Some(0));
    assert_eq!(graph.entry(2).unwrap().logical_size, Some(0));
    assert_eq!(graph.entry(2).unwrap().allocated_size, Some(0));
    assert!(graph.entry(2).unwrap().allocated_size_known);

    let (total, children) = graph.get_children_page(1, 0, 10).unwrap();
    assert_eq!(total, 2);
    // EmptyDir node
    let empty_dir_node = children.iter().find(|n| n.id == 2).unwrap();
    assert_eq!(empty_dir_node.logical_size, 0);
    assert_eq!(empty_dir_node.allocated_size, 0);
    assert!(empty_dir_node.allocated_size_known);
    assert_eq!(empty_dir_node.child_count, 0);
    assert!(!empty_dir_node.has_children);
}

#[test]
fn test_allocated_knowledge_propagation_and_known_subtotal() {
    let mut buf = Vec::new();
    let mut writer = ObservationWriter::new(&mut buf, r"C:\AllocKnowledgeTest").unwrap();

    // Root (id=1)
    //   DirWithMissing (id=2, parent=1)
    //     file_with_alloc (id=3, parent=2): logical 1000, alloc Some(4096)
    //     file_missing_alloc (id=4, parent=2): logical 2000, alloc None
    //   DirAllKnown (id=5, parent=1)
    //     file_known (id=6, parent=5): logical 3000, alloc Some(8192)

    writer
        .write_directory(&DirectoryObservation {
            entry_id: 1,
            parent_id: 0,
            name: "Root".to_string(),
            file_attributes: 0x10,
            reparse_tag: 0,
            creation_time_utc_ms: 0,
            last_write_time_utc_ms: 0,
            last_access_time_utc_ms: 0,
        })
        .unwrap();

    writer
        .write_directory(&DirectoryObservation {
            entry_id: 2,
            parent_id: 1,
            name: "DirWithMissing".to_string(),
            file_attributes: 0x10,
            reparse_tag: 0,
            creation_time_utc_ms: 0,
            last_write_time_utc_ms: 0,
            last_access_time_utc_ms: 0,
        })
        .unwrap();

    writer
        .write_file(&FileObservation {
            entry_id: 3,
            parent_id: 2,
            name: "file_with_alloc.bin".to_string(),
            logical_size: 1000,
            allocated_size: Some(4096),
            file_attributes: 0x20,
            reparse_tag: 0,
            creation_time_utc_ms: 0,
            last_write_time_utc_ms: 0,
            last_access_time_utc_ms: 0,
        })
        .unwrap();

    writer
        .write_file(&FileObservation {
            entry_id: 4,
            parent_id: 2,
            name: "file_missing_alloc.bin".to_string(),
            logical_size: 2000,
            allocated_size: None,
            file_attributes: 0x20,
            reparse_tag: 0,
            creation_time_utc_ms: 0,
            last_write_time_utc_ms: 0,
            last_access_time_utc_ms: 0,
        })
        .unwrap();

    writer
        .write_directory(&DirectoryObservation {
            entry_id: 5,
            parent_id: 1,
            name: "DirAllKnown".to_string(),
            file_attributes: 0x10,
            reparse_tag: 0,
            creation_time_utc_ms: 0,
            last_write_time_utc_ms: 0,
            last_access_time_utc_ms: 0,
        })
        .unwrap();

    writer
        .write_file(&FileObservation {
            entry_id: 6,
            parent_id: 5,
            name: "file_known.bin".to_string(),
            logical_size: 3000,
            allocated_size: Some(8192),
            file_attributes: 0x20,
            reparse_tag: 0,
            creation_time_utc_ms: 0,
            last_write_time_utc_ms: 0,
            last_access_time_utc_ms: 0,
        })
        .unwrap();

    writer
        .write_terminal(&TerminalObservation {
            outcome: RunOutcome::Finished,
            total_directories: 3,
            total_files: 3,
            total_logical_bytes: 6000,
            total_allocated_bytes: 12288,
            coverage_gap_count: 0,
            duration_ms: 30,
        })
        .unwrap();

    let reader = ObservationReader::new(Cursor::new(buf)).unwrap();
    let graph = build_graph_from_reader(reader).unwrap();

    // DirWithMissing: logical 3000, allocated 4096 (known subtotal), allocated_size_known false
    let dir_missing = graph.entry(2).unwrap();
    assert_eq!(dir_missing.logical_size, Some(3000));
    assert_eq!(dir_missing.allocated_size, Some(4096));
    assert!(!dir_missing.allocated_size_known);

    // DirAllKnown: logical 3000, allocated 8192, allocated_size_known true
    let dir_known = graph.entry(5).unwrap();
    assert_eq!(dir_known.logical_size, Some(3000));
    assert_eq!(dir_known.allocated_size, Some(8192));
    assert!(dir_known.allocated_size_known);

    // Root: logical 6000, allocated 12288, allocated_size_known false (propagated)
    let root = graph.root();
    assert_eq!(root.logical_size, Some(6000));
    assert_eq!(root.allocated_size, Some(12288));
    assert!(!root.allocated_size_known);

    // GetChildren verification
    let (_total, children) = graph.get_children_page(1, 0, 10).unwrap();
    let node_missing = children.iter().find(|n| n.id == 2).unwrap();
    assert_eq!(node_missing.allocated_size, 4096);
    assert!(!node_missing.allocated_size_known);

    let node_known = children.iter().find(|n| n.id == 5).unwrap();
    assert_eq!(node_known.allocated_size, 8192);
    assert!(node_known.allocated_size_known);
}

#[test]
fn test_cancelled_partial_graph_aggregation() {
    let mut buf = Vec::new();
    let mut writer = ObservationWriter::new(&mut buf, r"C:\CancelledRoot").unwrap();

    // Partial graph:
    // Root (id=1)
    //   PartialDir (id=2, parent=1)
    //     partial_file (id=3, parent=2): logical 2500, alloc Some(4096)

    writer
        .write_directory(&DirectoryObservation {
            entry_id: 1,
            parent_id: 0,
            name: "Root".to_string(),
            file_attributes: 0x10,
            reparse_tag: 0,
            creation_time_utc_ms: 0,
            last_write_time_utc_ms: 0,
            last_access_time_utc_ms: 0,
        })
        .unwrap();

    writer
        .write_directory(&DirectoryObservation {
            entry_id: 2,
            parent_id: 1,
            name: "PartialDir".to_string(),
            file_attributes: 0x10,
            reparse_tag: 0,
            creation_time_utc_ms: 0,
            last_write_time_utc_ms: 0,
            last_access_time_utc_ms: 0,
        })
        .unwrap();

    writer
        .write_file(&FileObservation {
            entry_id: 3,
            parent_id: 2,
            name: "partial_file.dat".to_string(),
            logical_size: 2500,
            allocated_size: Some(4096),
            file_attributes: 0x20,
            reparse_tag: 0,
            creation_time_utc_ms: 0,
            last_write_time_utc_ms: 0,
            last_access_time_utc_ms: 0,
        })
        .unwrap();

    // Cancellation terminal
    writer
        .write_terminal(&TerminalObservation {
            outcome: RunOutcome::Cancelled,
            total_directories: 2,
            total_files: 1,
            total_logical_bytes: 2500,
            total_allocated_bytes: 4096,
            coverage_gap_count: 0,
            duration_ms: 15,
        })
        .unwrap();

    let reader = ObservationReader::new(Cursor::new(buf)).unwrap();
    let graph = build_graph_from_reader(reader).expect("cancelled graph build should succeed");

    assert_eq!(graph.terminal().outcome, RunOutcome::Cancelled);
    assert_eq!(graph.entry(2).unwrap().logical_size, Some(2500));
    assert_eq!(graph.entry(2).unwrap().allocated_size, Some(4096));
    assert_eq!(graph.root().logical_size, Some(2500));
    assert_eq!(graph.root().allocated_size, Some(4096));

    let (_total, children) = graph.get_children_page(1, 0, 10).unwrap();
    assert_eq!(children[0].logical_size, 2500);
    assert_eq!(children[0].allocated_size, 4096);
}

#[test]
fn test_deep_iterative_stack_safety() {
    let depth = 5000;
    let mut builder = GraphBuilder::new(r"C:\DeepRoot");

    builder
        .ingest_record(ObservationRecord::Directory(DirectoryObservation {
            entry_id: 1,
            parent_id: 0,
            name: "Root".to_string(),
            file_attributes: 0x10,
            reparse_tag: 0,
            creation_time_utc_ms: 0,
            last_write_time_utc_ms: 0,
            last_access_time_utc_ms: 0,
        }))
        .unwrap();

    for i in 2..=depth {
        builder
            .ingest_record(ObservationRecord::Directory(DirectoryObservation {
                entry_id: i,
                parent_id: i - 1,
                name: format!("dir_{}", i),
                file_attributes: 0x10,
                reparse_tag: 0,
                creation_time_utc_ms: 0,
                last_write_time_utc_ms: 0,
                last_access_time_utc_ms: 0,
            }))
            .unwrap();
    }

    // Leaf file at depth
    let leaf_file_id = depth + 1;
    builder
        .ingest_record(ObservationRecord::File(FileObservation {
            entry_id: leaf_file_id,
            parent_id: depth,
            name: "deep_leaf.bin".to_string(),
            logical_size: 42,
            allocated_size: Some(4096),
            file_attributes: 0x20,
            reparse_tag: 0,
            creation_time_utc_ms: 0,
            last_write_time_utc_ms: 0,
            last_access_time_utc_ms: 0,
        }))
        .unwrap();

    builder
        .ingest_record(ObservationRecord::Terminal(TerminalObservation {
            outcome: RunOutcome::Finished,
            total_directories: depth as u64,
            total_files: 1,
            total_logical_bytes: 42,
            total_allocated_bytes: 4096,
            coverage_gap_count: 0,
            duration_ms: 100,
        }))
        .unwrap();

    let graph = builder.finish().expect("finish must succeed on deep tree");
    assert_eq!(graph.root().logical_size, Some(42));
    assert_eq!(graph.root().allocated_size, Some(4096));
    assert_eq!(graph.entry(depth).unwrap().logical_size, Some(42));
    assert_eq!(graph.entry(depth).unwrap().allocated_size, Some(4096));
}
