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
