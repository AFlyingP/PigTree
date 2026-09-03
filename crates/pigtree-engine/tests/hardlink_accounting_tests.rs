use pigtree_engine::{build_graph_from_reader, DirectoryGraph};
use pigtree_protocol::{
    DirectoryObservation, ExternalReferenceStatus, FileObservation, ObjectIdentity,
    ObservationReader, ObservationWriter, RunOutcome, SpecialObservation, TerminalObservation,
    ValueKnowledge,
};
use std::io::Cursor;

#[test]
fn test_two_hard_links_same_directory() {
    let mut buf = Vec::new();
    let mut writer = ObservationWriter::new(&mut buf, r"C:\Test").unwrap();

    let volume_guid = [1u8; 16];
    let shared_id = ObjectIdentity::new(volume_guid, 1001);

    // 1. Root directory (id=1, parent=0)
    writer
        .write_directory(&DirectoryObservation {
            entry_id: 1,
            parent_id: 0,
            name: "Test".to_string(),
            file_attributes: 0x10,
            reparse_tag: 0,
            creation_time_utc_ms: 100,
            last_write_time_utc_ms: 200,
            last_access_time_utc_ms: 300,
            object_id: None,
            allocated_size: None,
            total_link_count: ValueKnowledge::NotObserved,
        })
        .unwrap();

    // 2. SubDir (id=2, parent=1)
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
            object_id: None,
            allocated_size: None,
            total_link_count: ValueKnowledge::NotObserved,
        })
        .unwrap();

    // 3. File A in SubDir (id=3, parent=2, 10 bytes)
    writer
        .write_file(&FileObservation {
            entry_id: 3,
            parent_id: 2,
            name: "file_a.dat".to_string(),
            logical_size: 10,
            allocated_size: Some(10),
            file_attributes: 0x20,
            reparse_tag: 0,
            creation_time_utc_ms: 102,
            last_write_time_utc_ms: 202,
            last_access_time_utc_ms: 302,
            object_id: Some(shared_id),
            total_link_count: ValueKnowledge::Known(2),
        })
        .unwrap();

    // 4. File B in SubDir (id=4, parent=2, 10 bytes, same object_id)
    writer
        .write_file(&FileObservation {
            entry_id: 4,
            parent_id: 2,
            name: "file_b.dat".to_string(),
            logical_size: 10,
            allocated_size: Some(10),
            file_attributes: 0x20,
            reparse_tag: 0,
            creation_time_utc_ms: 103,
            last_write_time_utc_ms: 203,
            last_access_time_utc_ms: 303,
            object_id: Some(shared_id),
            total_link_count: ValueKnowledge::Known(2),
        })
        .unwrap();

    // 5. Terminal
    writer
        .write_terminal(&TerminalObservation {
            outcome: RunOutcome::Finished,
            total_directories: 2,
            total_files: 2,
            total_logical_bytes: 20,
            total_allocated_bytes: 20,
            coverage_gap_count: 0,
            duration_ms: 10,
        })
        .unwrap();

    let cursor = Cursor::new(buf);
    let reader = ObservationReader::new(cursor).unwrap();
    let graph = build_graph_from_reader(reader).unwrap();

    let root = graph.root();
    assert_eq!(root.referenced_allocated_bytes, 20);
    assert_eq!(root.unique_allocated_bytes, 10);
    assert_eq!(root.logical_size, Some(20));
    assert!(root.allocated_size_known);

    let subdir = graph.entry(2).unwrap();
    assert_eq!(subdir.referenced_allocated_bytes, 20);
    assert_eq!(subdir.unique_allocated_bytes, 10);
    assert_eq!(subdir.logical_size, Some(20));
    assert!(subdir.allocated_size_known);

    let file_a = graph.entry(3).unwrap();
    assert_eq!(file_a.referenced_allocated_bytes, 10);
    assert_eq!(file_a.unique_allocated_bytes, 10);
    assert_eq!(file_a.observed_alias_count, 2);
    assert_eq!(file_a.total_link_count, ValueKnowledge::Known(2));
    assert_eq!(
        file_a.external_reference_status,
        ExternalReferenceStatus::ConfirmedNone
    );

    let file_b = graph.entry(4).unwrap();
    assert_eq!(file_b.referenced_allocated_bytes, 10);
    assert_eq!(file_b.unique_allocated_bytes, 10);
    assert_eq!(file_b.observed_alias_count, 2);
    assert_eq!(file_b.total_link_count, ValueKnowledge::Known(2));
    assert_eq!(
        file_b.external_reference_status,
        ExternalReferenceStatus::ConfirmedNone
    );
}

// ---------------------------------------------------------------------------
// Shared helpers for the issue #20 test matrix.
// ---------------------------------------------------------------------------

const DIR_ATTRS: u32 = 0x10; // FILE_ATTRIBUTE_DIRECTORY
const FILE_ATTRS: u32 = 0x20; // FILE_ATTRIBUTE_ARCHIVE

type TestWriter<'a> = ObservationWriter<&'a mut Vec<u8>>;

fn volume(seed: u8) -> [u8; 16] {
    [seed; 16]
}

fn emit_root(w: &mut TestWriter<'_>) {
    emit_dir(w, 1, 0, "Test", None, None, ValueKnowledge::NotObserved);
}

#[allow(clippy::too_many_arguments)]
fn emit_dir(
    w: &mut TestWriter<'_>,
    entry_id: u32,
    parent_id: u32,
    name: &str,
    object_id: Option<ObjectIdentity>,
    allocated_size: Option<u64>,
    total_link_count: ValueKnowledge<u32>,
) {
    w.write_directory(&DirectoryObservation {
        entry_id,
        parent_id,
        name: name.to_string(),
        file_attributes: DIR_ATTRS,
        reparse_tag: 0,
        creation_time_utc_ms: 100 + u64::from(entry_id),
        last_write_time_utc_ms: 200 + u64::from(entry_id),
        last_access_time_utc_ms: 300 + u64::from(entry_id),
        object_id,
        allocated_size,
        total_link_count,
    })
    .unwrap();
}

#[allow(clippy::too_many_arguments)]
fn emit_file(
    w: &mut TestWriter<'_>,
    entry_id: u32,
    parent_id: u32,
    name: &str,
    logical_size: u64,
    allocated_size: Option<u64>,
    object_id: Option<ObjectIdentity>,
    total_link_count: ValueKnowledge<u32>,
) {
    w.write_file(&FileObservation {
        entry_id,
        parent_id,
        name: name.to_string(),
        logical_size,
        allocated_size,
        file_attributes: FILE_ATTRS,
        reparse_tag: 0,
        creation_time_utc_ms: 100 + u64::from(entry_id),
        last_write_time_utc_ms: 200 + u64::from(entry_id),
        last_access_time_utc_ms: 300 + u64::from(entry_id),
        object_id,
        total_link_count,
    })
    .unwrap();
}

#[allow(dead_code)]
fn emit_special(w: &mut TestWriter<'_>, entry_id: u32, parent_id: u32, name: &str) {
    w.write_special(&SpecialObservation {
        entry_id,
        parent_id,
        name: name.to_string(),
        file_attributes: 0x400,  // FILE_ATTRIBUTE_REPARSE_POINT
        reparse_tag: 0xA000000C, // IO_REPARSE_TAG_SYMLINK
        creation_time_utc_ms: 100 + u64::from(entry_id),
        last_write_time_utc_ms: 200 + u64::from(entry_id),
        last_access_time_utc_ms: 300 + u64::from(entry_id),
        object_id: None,
    })
    .unwrap();
}

fn emit_terminal(
    w: &mut TestWriter<'_>,
    directories: u64,
    files: u64,
    logical: u64,
    allocated: u64,
) {
    w.write_terminal(&TerminalObservation {
        outcome: RunOutcome::Finished,
        total_directories: directories,
        total_files: files,
        total_logical_bytes: logical,
        total_allocated_bytes: allocated,
        coverage_gap_count: 0,
        duration_ms: 10,
    })
    .unwrap();
}

fn build_graph(buf: Vec<u8>) -> DirectoryGraph {
    let reader = ObservationReader::new(Cursor::new(buf)).unwrap();
    build_graph_from_reader(reader).unwrap()
}

// ---------------------------------------------------------------------------
// Matrix 1: Independent single-link files — S_ref == S_uniq at every level.
// ---------------------------------------------------------------------------

#[test]
fn test_independent_files_ref_eq_uniq_at_every_level() {
    let mut buf = Vec::new();
    let mut w = ObservationWriter::new(&mut buf, r"C:\Test").unwrap();

    emit_root(&mut w);
    // A(2) with two independent files.
    emit_dir(&mut w, 2, 1, "A", None, None, ValueKnowledge::NotObserved);
    emit_file(
        &mut w,
        3,
        2,
        "a1.txt",
        100,
        Some(100),
        Some(ObjectIdentity::new(volume(1), 10)),
        ValueKnowledge::NotObserved,
    );
    emit_file(
        &mut w,
        4,
        2,
        "a2.txt",
        200,
        Some(200),
        Some(ObjectIdentity::new(volume(1), 11)),
        ValueKnowledge::NotObserved,
    );
    // B(5) with one independent file.
    emit_dir(&mut w, 5, 1, "B", None, None, ValueKnowledge::NotObserved);
    emit_file(
        &mut w,
        6,
        5,
        "b1.txt",
        300,
        Some(300),
        Some(ObjectIdentity::new(volume(1), 12)),
        ValueKnowledge::NotObserved,
    );
    // File directly under root.
    emit_file(
        &mut w,
        7,
        1,
        "r1.txt",
        50,
        Some(50),
        Some(ObjectIdentity::new(volume(1), 13)),
        ValueKnowledge::NotObserved,
    );
    emit_terminal(&mut w, 3, 4, 650, 650);

    let graph = build_graph(buf);

    let a = graph.entry(2).unwrap();
    assert_eq!(a.referenced_allocated_bytes, 300);
    assert_eq!(a.unique_allocated_bytes, 300);
    assert_eq!(a.logical_size, Some(300));
    assert!(a.allocated_size_known);

    let b = graph.entry(5).unwrap();
    assert_eq!(b.referenced_allocated_bytes, 300);
    assert_eq!(b.unique_allocated_bytes, 300);

    let root = graph.root();
    assert_eq!(root.referenced_allocated_bytes, 650);
    assert_eq!(root.unique_allocated_bytes, 650);
    assert_eq!(root.logical_size, Some(650));
    assert!(root.allocated_size_known);

    // Every file: referenced == unique, single alias, Indeterminate (default profile).
    for id in [3u32, 4, 6, 7] {
        let file = graph.entry(id).unwrap();
        assert_eq!(file.referenced_allocated_bytes, file.unique_allocated_bytes);
        assert_eq!(file.observed_alias_count, 1);
        assert_eq!(file.total_link_count, ValueKnowledge::NotObserved);
        assert_eq!(
            file.external_reference_status,
            ExternalReferenceStatus::Indeterminate
        );
    }
}

// ---------------------------------------------------------------------------
// Matrix 2: k intra-directory hard links to one object.
// ---------------------------------------------------------------------------

#[test]
fn test_k_intra_directory_hard_links() {
    const K: u64 = 3;
    let mut buf = Vec::new();
    let mut w = ObservationWriter::new(&mut buf, r"C:\Test").unwrap();

    let shared = ObjectIdentity::new(volume(2), 2001);

    emit_root(&mut w);
    emit_dir(&mut w, 2, 1, "D", None, None, ValueKnowledge::NotObserved);
    for i in 0..K {
        emit_file(
            &mut w,
            3 + i as u32,
            2,
            &format!("f{i}.dat"),
            100,
            Some(100),
            Some(shared),
            ValueKnowledge::Known(K as u32),
        );
    }
    emit_terminal(&mut w, 2, K, 100 * K, 100 * K);

    let graph = build_graph(buf);

    let d = graph.entry(2).unwrap();
    assert_eq!(d.referenced_allocated_bytes, 100 * K);
    assert_eq!(d.unique_allocated_bytes, 100);
    assert_eq!(d.logical_size, Some(100 * K));
    assert!(d.allocated_size_known);

    let root = graph.root();
    assert_eq!(root.referenced_allocated_bytes, 100 * K);
    assert_eq!(root.unique_allocated_bytes, 100);

    for id in 3..(3 + K as u32) {
        let alias = graph.entry(id).unwrap();
        assert_eq!(alias.referenced_allocated_bytes, 100);
        assert_eq!(alias.unique_allocated_bytes, 100);
        assert_eq!(alias.observed_alias_count, K as u32);
        assert_eq!(alias.total_link_count, ValueKnowledge::Known(K as u32));
        assert_eq!(
            alias.external_reference_status,
            ExternalReferenceStatus::ConfirmedNone
        );
    }
}

// ---------------------------------------------------------------------------
// Matrix 3: Hard links across sibling directories.
// ---------------------------------------------------------------------------

#[test]
fn test_hard_links_across_sibling_directories() {
    let mut buf = Vec::new();
    let mut w = ObservationWriter::new(&mut buf, r"C:\Test").unwrap();

    let shared = ObjectIdentity::new(volume(3), 3001);

    emit_root(&mut w);
    emit_dir(&mut w, 2, 1, "D1", None, None, ValueKnowledge::NotObserved);
    emit_file(
        &mut w,
        3,
        2,
        "f1.dat",
        100,
        Some(100),
        Some(shared),
        ValueKnowledge::NotObserved,
    );
    emit_dir(&mut w, 4, 1, "D2", None, None, ValueKnowledge::NotObserved);
    emit_file(
        &mut w,
        5,
        4,
        "f2.dat",
        100,
        Some(100),
        Some(shared),
        ValueKnowledge::NotObserved,
    );
    emit_terminal(&mut w, 3, 2, 200, 200);

    let graph = build_graph(buf);

    // Each sibling directory reports the file locally.
    let d1 = graph.entry(2).unwrap();
    assert_eq!(d1.referenced_allocated_bytes, 100);
    assert_eq!(d1.unique_allocated_bytes, 100);

    let d2 = graph.entry(4).unwrap();
    assert_eq!(d2.referenced_allocated_bytes, 100);
    assert_eq!(d2.unique_allocated_bytes, 100);

    // Sibling scopes overlap and are not additive: parent counts the object once.
    let root = graph.root();
    assert_eq!(root.referenced_allocated_bytes, 200);
    assert_eq!(root.unique_allocated_bytes, 100);
    assert!(d1.unique_allocated_bytes + d2.unique_allocated_bytes >= root.unique_allocated_bytes);

    // Variant A: default un-enriched scan — NotObserved total, Indeterminate.
    for id in [3u32, 5] {
        let alias = graph.entry(id).unwrap();
        assert_eq!(alias.observed_alias_count, 2);
        assert_eq!(alias.total_link_count, ValueKnowledge::NotObserved);
        assert_eq!(
            alias.external_reference_status,
            ExternalReferenceStatus::Indeterminate
        );
    }
}

// ---------------------------------------------------------------------------
// Matrix 4: Cross-branch deep-tree LCA correctness.
//
// root
//   A
//     B                     <- intermediate LCA of the two X occurrences
//       C
//         fX1 (X, 100)  fY1 (Y, 50)
//       D
//         fX2 (X, 100)      <- X duplicated across C and D (LCA = B)
//   E
//     fY2 (Y, 50)           <- Y duplicated across A-branch and E (LCA = root)
//     fZ1 (Z, 30)  fZ2 (Z, 30)  <- Z duplicated within E (LCA = E)
// ---------------------------------------------------------------------------

#[test]
fn test_cross_branch_deep_tree_lca_correctness() {
    let mut buf = Vec::new();
    let mut w = ObservationWriter::new(&mut buf, r"C:\Test").unwrap();

    let x = ObjectIdentity::new(volume(4), 4001);
    let y = ObjectIdentity::new(volume(4), 4002);
    let z = ObjectIdentity::new(volume(4), 4003);

    emit_root(&mut w);
    emit_dir(&mut w, 2, 1, "A", None, None, ValueKnowledge::NotObserved);
    emit_dir(&mut w, 3, 2, "B", None, None, ValueKnowledge::NotObserved);
    emit_dir(&mut w, 4, 3, "C", None, None, ValueKnowledge::NotObserved);
    emit_file(
        &mut w,
        5,
        4,
        "fX1",
        100,
        Some(100),
        Some(x),
        ValueKnowledge::NotObserved,
    );
    emit_file(
        &mut w,
        6,
        4,
        "fY1",
        50,
        Some(50),
        Some(y),
        ValueKnowledge::NotObserved,
    );
    emit_dir(&mut w, 7, 3, "D", None, None, ValueKnowledge::NotObserved);
    emit_file(
        &mut w,
        8,
        7,
        "fX2",
        100,
        Some(100),
        Some(x),
        ValueKnowledge::NotObserved,
    );
    emit_dir(&mut w, 9, 1, "E", None, None, ValueKnowledge::NotObserved);
    emit_file(
        &mut w,
        10,
        9,
        "fY2",
        50,
        Some(50),
        Some(y),
        ValueKnowledge::NotObserved,
    );
    emit_file(
        &mut w,
        11,
        9,
        "fZ1",
        30,
        Some(30),
        Some(z),
        ValueKnowledge::NotObserved,
    );
    emit_file(
        &mut w,
        12,
        9,
        "fZ2",
        30,
        Some(30),
        Some(z),
        ValueKnowledge::NotObserved,
    );
    emit_terminal(&mut w, 6, 6, 360, 360);

    let graph = build_graph(buf);

    // X duplicated across C and D: intermediate LCA B counts X exactly once.
    let c = graph.entry(4).unwrap();
    assert_eq!(c.referenced_allocated_bytes, 150);
    assert_eq!(c.unique_allocated_bytes, 150); // X + Y
    let d = graph.entry(7).unwrap();
    assert_eq!(d.referenced_allocated_bytes, 100);
    assert_eq!(d.unique_allocated_bytes, 100);
    let b = graph.entry(3).unwrap();
    assert_eq!(b.referenced_allocated_bytes, 250);
    assert_eq!(b.unique_allocated_bytes, 150); // X once + Y once

    // A sees X once and Y once.
    let a = graph.entry(2).unwrap();
    assert_eq!(a.referenced_allocated_bytes, 250);
    assert_eq!(a.unique_allocated_bytes, 150);

    // E deduplicates Z locally (LCA = E): Y once + Z once.
    let e = graph.entry(9).unwrap();
    assert_eq!(e.referenced_allocated_bytes, 110);
    assert_eq!(e.unique_allocated_bytes, 80);

    // Root: Y's LCA is the root; everything distinct counted once.
    let root = graph.root();
    assert_eq!(root.referenced_allocated_bytes, 360);
    assert_eq!(root.unique_allocated_bytes, 180); // X(100) + Y(50) + Z(30)

    // Alias evidence.
    assert_eq!(graph.entry(5).unwrap().observed_alias_count, 2); // X
    assert_eq!(graph.entry(8).unwrap().observed_alias_count, 2); // X
    assert_eq!(graph.entry(6).unwrap().observed_alias_count, 2); // Y
    assert_eq!(graph.entry(10).unwrap().observed_alias_count, 2); // Y
    assert_eq!(graph.entry(11).unwrap().observed_alias_count, 2); // Z
    assert_eq!(graph.entry(12).unwrap().observed_alias_count, 2); // Z
}

// ---------------------------------------------------------------------------
// Slice 1: Directories with canonical ObjectIdentity and observed self-allocation.
// ---------------------------------------------------------------------------

#[test]
fn test_directory_canonical_identity_and_self_allocation() {
    let mut buf = Vec::new();
    let mut w = ObservationWriter::new(&mut buf, r"C:\Test").unwrap();

    let root_oid = ObjectIdentity::new(volume(10), 1);
    let sub_oid = ObjectIdentity::new(volume(10), 2);
    let file_oid = ObjectIdentity::new(volume(10), 3);

    // Root directory with canonical identity and self-allocation (4096 bytes)
    emit_dir(
        &mut w,
        1,
        0,
        "Test",
        Some(root_oid),
        Some(4096),
        ValueKnowledge::NotObserved,
    );

    // Sub directory with canonical identity and self-allocation (4096 bytes)
    emit_dir(
        &mut w,
        2,
        1,
        "Sub",
        Some(sub_oid),
        Some(4096),
        ValueKnowledge::NotObserved,
    );

    // File in Sub (1000 logical, 1024 allocated)
    emit_file(
        &mut w,
        3,
        2,
        "file.dat",
        1000,
        Some(1024),
        Some(file_oid),
        ValueKnowledge::NotObserved,
    );

    // Total allocated = 4096 (root) + 4096 (sub) + 1024 (file) = 9216
    emit_terminal(&mut w, 2, 1, 1000, 9216);

    let graph = build_graph(buf);

    let sub = graph.entry(2).unwrap();
    assert_eq!(sub.referenced_allocated_bytes, 5120);
    assert_eq!(sub.unique_allocated_bytes, 5120);
    assert_eq!(sub.total_link_count, ValueKnowledge::NotApplicable);
    assert_eq!(
        sub.external_reference_status,
        ExternalReferenceStatus::NotApplicable
    );

    let root = graph.root();
    assert_eq!(root.referenced_allocated_bytes, 9216);
    assert_eq!(root.unique_allocated_bytes, 9216);
    assert_eq!(root.total_link_count, ValueKnowledge::NotApplicable);
    assert_eq!(
        root.external_reference_status,
        ExternalReferenceStatus::NotApplicable
    );

    // Only the file with NotObserved link count is Indeterminate; directories never inflate target uncertainty
    assert_eq!(graph.indeterminate_external_reference_objects(), 1);
}

// ---------------------------------------------------------------------------
// Slice 2: Mixed alias allocation test: Known(100) + None for one object.
// ---------------------------------------------------------------------------

#[test]
fn test_mixed_alias_allocation_known_and_none() {
    let mut buf = Vec::new();
    let mut w = ObservationWriter::new(&mut buf, r"C:\Test").unwrap();

    let shared = ObjectIdentity::new(volume(20), 200);

    emit_root(&mut w);
    // SubDir 1 with known alias
    emit_dir(
        &mut w,
        2,
        1,
        "Sub1",
        None,
        None,
        ValueKnowledge::NotObserved,
    );
    emit_file(
        &mut w,
        3,
        2,
        "known.dat",
        100,
        Some(100),
        Some(shared),
        ValueKnowledge::Known(2),
    );

    // SubDir 2 with missing (None) alias allocation
    emit_dir(
        &mut w,
        4,
        1,
        "Sub2",
        None,
        None,
        ValueKnowledge::NotObserved,
    );
    emit_file(
        &mut w,
        5,
        4,
        "missing.dat",
        100,
        None,
        Some(shared),
        ValueKnowledge::Known(2),
    );

    emit_terminal(&mut w, 3, 2, 200, 100);

    let graph = build_graph(buf);

    let known_file = graph.entry(3).unwrap();
    assert_eq!(known_file.referenced_allocated_bytes, 100);
    assert_eq!(known_file.unique_allocated_bytes, 100);
    assert!(known_file.allocated_size_known);

    let missing_file = graph.entry(5).unwrap();
    assert_eq!(missing_file.referenced_allocated_bytes, 0);
    // Crucial invariant: missing alias leaf's unique scope receives object's known lower bound
    assert_eq!(missing_file.unique_allocated_bytes, 100);
    assert!(!missing_file.allocated_size_known);

    let sub1 = graph.entry(2).unwrap();
    assert_eq!(sub1.referenced_allocated_bytes, 100);
    assert_eq!(sub1.unique_allocated_bytes, 100);
    assert!(sub1.allocated_size_known);

    let sub2 = graph.entry(4).unwrap();
    assert_eq!(sub2.referenced_allocated_bytes, 0);
    assert_eq!(sub2.unique_allocated_bytes, 100);
    assert!(!sub2.allocated_size_known);
    assert_eq!(sub2.known_subtotal_allocated_bytes, 0);

    let root = graph.root();
    assert_eq!(root.referenced_allocated_bytes, 100);
    assert_eq!(root.unique_allocated_bytes, 100);
    assert_eq!(root.known_subtotal_allocated_bytes, 100);
    assert!(!root.allocated_size_known);
    assert!(!graph.allocated_bytes_known());
}

// ---------------------------------------------------------------------------
// Slice 3: Conflicting known allocation test: aliases 100/200.
// ---------------------------------------------------------------------------

#[test]
fn test_conflicting_known_allocation_order_100_then_200() {
    let mut buf = Vec::new();
    let mut w = ObservationWriter::new(&mut buf, r"C:\Test").unwrap();
    let shared = ObjectIdentity::new(volume(30), 300);

    emit_root(&mut w);
    emit_dir(
        &mut w,
        2,
        1,
        "Sub1",
        None,
        None,
        ValueKnowledge::NotObserved,
    );
    emit_file(
        &mut w,
        3,
        2,
        "a.dat",
        100,
        Some(100),
        Some(shared),
        ValueKnowledge::Known(2),
    );

    emit_dir(
        &mut w,
        4,
        1,
        "Sub2",
        None,
        None,
        ValueKnowledge::NotObserved,
    );
    emit_file(
        &mut w,
        5,
        4,
        "b.dat",
        100,
        Some(200),
        Some(shared),
        ValueKnowledge::Known(2),
    );

    emit_terminal(&mut w, 3, 2, 200, 300);

    let graph = build_graph(buf);

    let f1 = graph.entry(3).unwrap();
    assert_eq!(f1.referenced_allocated_bytes, 100);
    assert_eq!(f1.unique_allocated_bytes, 100);

    let f2 = graph.entry(5).unwrap();
    assert_eq!(f2.referenced_allocated_bytes, 200);
    assert_eq!(f2.unique_allocated_bytes, 100);

    let sub1 = graph.entry(2).unwrap();
    assert_eq!(sub1.referenced_allocated_bytes, 100);
    assert_eq!(sub1.unique_allocated_bytes, 100);

    let sub2 = graph.entry(4).unwrap();
    assert_eq!(sub2.referenced_allocated_bytes, 200);
    assert_eq!(sub2.unique_allocated_bytes, 100);

    let root = graph.root();
    assert_eq!(root.referenced_allocated_bytes, 300);
    assert_eq!(root.known_subtotal_allocated_bytes, 300);
    assert_eq!(root.unique_allocated_bytes, 100);
    assert!(
        !root.allocated_size_known,
        "conflicting alias allocation must mark enclosing completeness false"
    );
    assert!(!graph.allocated_bytes_known());
}

#[test]
fn test_conflicting_known_allocation_order_200_then_100() {
    let mut buf = Vec::new();
    let mut w = ObservationWriter::new(&mut buf, r"C:\Test").unwrap();
    let shared = ObjectIdentity::new(volume(30), 300);

    emit_root(&mut w);
    emit_dir(
        &mut w,
        2,
        1,
        "Sub1",
        None,
        None,
        ValueKnowledge::NotObserved,
    );
    emit_file(
        &mut w,
        3,
        2,
        "a.dat",
        100,
        Some(200),
        Some(shared),
        ValueKnowledge::Known(2),
    );

    emit_dir(
        &mut w,
        4,
        1,
        "Sub2",
        None,
        None,
        ValueKnowledge::NotObserved,
    );
    emit_file(
        &mut w,
        5,
        4,
        "b.dat",
        100,
        Some(100),
        Some(shared),
        ValueKnowledge::Known(2),
    );

    emit_terminal(&mut w, 3, 2, 200, 300);

    let graph = build_graph(buf);

    let f1 = graph.entry(3).unwrap();
    assert_eq!(f1.referenced_allocated_bytes, 200);
    assert_eq!(f1.unique_allocated_bytes, 100);

    let f2 = graph.entry(5).unwrap();
    assert_eq!(f2.referenced_allocated_bytes, 100);
    assert_eq!(f2.unique_allocated_bytes, 100);

    let root = graph.root();
    assert_eq!(root.referenced_allocated_bytes, 300);
    assert_eq!(root.known_subtotal_allocated_bytes, 300);
    assert_eq!(root.unique_allocated_bytes, 100);
    assert!(
        !root.allocated_size_known,
        "conflicting alias allocation must mark enclosing completeness false"
    );
    assert!(!graph.allocated_bytes_known());
}

// ---------------------------------------------------------------------------
// Slice 4: Conflicting total link count tests in both orders.
// ---------------------------------------------------------------------------

#[test]
fn test_conflicting_total_link_count_order_2_then_3() {
    let mut buf = Vec::new();
    let mut w = ObservationWriter::new(&mut buf, r"C:\Test").unwrap();
    let shared = ObjectIdentity::new(volume(40), 400);

    emit_root(&mut w);
    emit_dir(
        &mut w,
        2,
        1,
        "Sub1",
        None,
        None,
        ValueKnowledge::NotObserved,
    );
    emit_file(
        &mut w,
        3,
        2,
        "a.dat",
        100,
        Some(100),
        Some(shared),
        ValueKnowledge::Known(2),
    );

    emit_dir(
        &mut w,
        4,
        1,
        "Sub2",
        None,
        None,
        ValueKnowledge::NotObserved,
    );
    emit_file(
        &mut w,
        5,
        4,
        "b.dat",
        100,
        Some(100),
        Some(shared),
        ValueKnowledge::Known(3),
    );

    emit_terminal(&mut w, 3, 2, 200, 200);

    let graph = build_graph(buf);

    for id in [3u32, 5] {
        let entry = graph.entry(id).unwrap();
        assert_eq!(entry.observed_alias_count, 2);
        assert_eq!(
            entry.external_reference_status,
            ExternalReferenceStatus::InconsistentEvidence,
            "conflicting link counts must yield InconsistentEvidence, never ConfirmedNone or ConfirmedExternal"
        );
    }
    assert_eq!(graph.indeterminate_external_reference_objects(), 0);
}

#[test]
fn test_conflicting_total_link_count_order_3_then_2() {
    let mut buf = Vec::new();
    let mut w = ObservationWriter::new(&mut buf, r"C:\Test").unwrap();
    let shared = ObjectIdentity::new(volume(40), 400);

    emit_root(&mut w);
    emit_dir(
        &mut w,
        2,
        1,
        "Sub1",
        None,
        None,
        ValueKnowledge::NotObserved,
    );
    emit_file(
        &mut w,
        3,
        2,
        "a.dat",
        100,
        Some(100),
        Some(shared),
        ValueKnowledge::Known(3),
    );

    emit_dir(
        &mut w,
        4,
        1,
        "Sub2",
        None,
        None,
        ValueKnowledge::NotObserved,
    );
    emit_file(
        &mut w,
        5,
        4,
        "b.dat",
        100,
        Some(100),
        Some(shared),
        ValueKnowledge::Known(2),
    );

    emit_terminal(&mut w, 3, 2, 200, 200);

    let graph = build_graph(buf);

    for id in [3u32, 5] {
        let entry = graph.entry(id).unwrap();
        assert_eq!(entry.observed_alias_count, 2);
        assert_eq!(
            entry.external_reference_status,
            ExternalReferenceStatus::InconsistentEvidence,
            "conflicting link counts must yield InconsistentEvidence, never ConfirmedNone or ConfirmedExternal"
        );
    }
    assert_eq!(graph.indeterminate_external_reference_objects(), 0);
}

#[test]
fn test_indeterminate_link_count_counted_once_per_distinct_file_object() {
    let mut buf = Vec::new();
    let mut w = ObservationWriter::new(&mut buf, r"C:\Test").unwrap();
    let shared = ObjectIdentity::new(volume(40), 401);

    // Root directory with NotObserved link count
    emit_dir(
        &mut w,
        1,
        0,
        "Test",
        None,
        None,
        ValueKnowledge::NotObserved,
    );

    // SubDir with NotObserved link count
    emit_dir(&mut w, 2, 1, "Sub", None, None, ValueKnowledge::NotObserved);

    // 3 aliases to the same shared object with NotObserved link count
    emit_file(
        &mut w,
        3,
        2,
        "a.dat",
        100,
        Some(100),
        Some(shared),
        ValueKnowledge::NotObserved,
    );
    emit_file(
        &mut w,
        4,
        2,
        "b.dat",
        100,
        Some(100),
        Some(shared),
        ValueKnowledge::NotObserved,
    );
    emit_file(
        &mut w,
        5,
        1,
        "c.dat",
        100,
        Some(100),
        Some(shared),
        ValueKnowledge::NotObserved,
    );

    emit_terminal(&mut w, 2, 3, 300, 300);

    let graph = build_graph(buf);

    for id in [3u32, 4, 5] {
        let entry = graph.entry(id).unwrap();
        assert_eq!(entry.observed_alias_count, 3);
        assert_eq!(
            entry.external_reference_status,
            ExternalReferenceStatus::Indeterminate
        );
    }

    // Must be exactly 1: the distinct file object is counted once, directories never counted
    assert_eq!(graph.indeterminate_external_reference_objects(), 1);
}

// ---------------------------------------------------------------------------
// Slice 5: Nested ancestor/descendant, sibling overlap, k-alias, sparse ID, deep chain.
// ---------------------------------------------------------------------------

#[test]
fn test_nested_ancestor_descendant_hard_links() {
    let mut buf = Vec::new();
    let mut w = ObservationWriter::new(&mut buf, r"C:\Test").unwrap();
    let shared = ObjectIdentity::new(volume(50), 500);

    emit_root(&mut w);
    // Dir A (id=2, parent=1)
    emit_dir(
        &mut w,
        2,
        1,
        "DirA",
        None,
        None,
        ValueKnowledge::NotObserved,
    );
    // File A1 directly in DirA (id=3, parent=2)
    emit_file(
        &mut w,
        3,
        2,
        "a1.dat",
        500,
        Some(500),
        Some(shared),
        ValueKnowledge::Known(2),
    );

    // Nested Dir B (id=4, parent=2) -> Dir C (id=5, parent=4)
    emit_dir(
        &mut w,
        4,
        2,
        "DirB",
        None,
        None,
        ValueKnowledge::NotObserved,
    );
    emit_dir(
        &mut w,
        5,
        4,
        "DirC",
        None,
        None,
        ValueKnowledge::NotObserved,
    );
    // File A2 in DirC (id=6, parent=5)
    emit_file(
        &mut w,
        6,
        5,
        "a2.dat",
        500,
        Some(500),
        Some(shared),
        ValueKnowledge::Known(2),
    );

    emit_terminal(&mut w, 4, 2, 1000, 1000);

    let graph = build_graph(buf);

    let dirc = graph.entry(5).unwrap();
    assert_eq!(dirc.referenced_allocated_bytes, 500);
    assert_eq!(dirc.unique_allocated_bytes, 500);

    let dirb = graph.entry(4).unwrap();
    assert_eq!(dirb.referenced_allocated_bytes, 500);
    assert_eq!(dirb.unique_allocated_bytes, 500);

    let dira = graph.entry(2).unwrap();
    assert_eq!(dira.referenced_allocated_bytes, 1000);
    // Ancestor DirA contains both aliases: deduplicates to 500
    assert_eq!(dira.unique_allocated_bytes, 500);

    let root = graph.root();
    assert_eq!(root.referenced_allocated_bytes, 1000);
    assert_eq!(root.unique_allocated_bytes, 500);
}

#[test]
fn test_sparse_entry_ids_hard_links() {
    let mut buf = Vec::new();
    let mut w = ObservationWriter::new(&mut buf, r"C:\Test").unwrap();
    let shared = ObjectIdentity::new(volume(50), 501);

    emit_root(&mut w);
    emit_dir(
        &mut w,
        100,
        1,
        "Sub100",
        None,
        None,
        ValueKnowledge::NotObserved,
    );
    emit_file(
        &mut w,
        500,
        100,
        "f500.dat",
        250,
        Some(250),
        Some(shared),
        ValueKnowledge::Known(2),
    );

    emit_dir(
        &mut w,
        2000,
        1,
        "Sub2000",
        None,
        None,
        ValueKnowledge::NotObserved,
    );
    emit_file(
        &mut w,
        9999,
        2000,
        "f9999.dat",
        250,
        Some(250),
        Some(shared),
        ValueKnowledge::Known(2),
    );

    emit_terminal(&mut w, 3, 2, 500, 500);

    let graph = build_graph(buf);

    let s1 = graph.entry(100).unwrap();
    assert_eq!(s1.referenced_allocated_bytes, 250);
    assert_eq!(s1.unique_allocated_bytes, 250);

    let s2 = graph.entry(2000).unwrap();
    assert_eq!(s2.referenced_allocated_bytes, 250);
    assert_eq!(s2.unique_allocated_bytes, 250);

    let root = graph.root();
    assert_eq!(root.referenced_allocated_bytes, 500);
    assert_eq!(root.unique_allocated_bytes, 250);
}

#[test]
fn test_k_alias_complex_sibling_overlap() {
    let mut buf = Vec::new();
    let mut w = ObservationWriter::new(&mut buf, r"C:\Test").unwrap();
    let x = ObjectIdentity::new(volume(50), 502);
    let y = ObjectIdentity::new(volume(50), 503);

    emit_root(&mut w);

    // Branch 1: SubA (2) -> f1 (3, X)
    emit_dir(
        &mut w,
        2,
        1,
        "SubA",
        None,
        None,
        ValueKnowledge::NotObserved,
    );
    emit_file(
        &mut w,
        3,
        2,
        "f1.dat",
        100,
        Some(100),
        Some(x),
        ValueKnowledge::Known(4),
    );

    // Branch 2: SubB (4) -> SubB1 (5) -> f2 (6, X), f_y (7, Y)
    //                    -> SubB2 (8) -> f3 (9, X)
    emit_dir(
        &mut w,
        4,
        1,
        "SubB",
        None,
        None,
        ValueKnowledge::NotObserved,
    );
    emit_dir(
        &mut w,
        5,
        4,
        "SubB1",
        None,
        None,
        ValueKnowledge::NotObserved,
    );
    emit_file(
        &mut w,
        6,
        5,
        "f2.dat",
        100,
        Some(100),
        Some(x),
        ValueKnowledge::Known(4),
    );
    emit_file(
        &mut w,
        7,
        5,
        "f_y.dat",
        200,
        Some(200),
        Some(y),
        ValueKnowledge::Known(1),
    );

    emit_dir(
        &mut w,
        8,
        4,
        "SubB2",
        None,
        None,
        ValueKnowledge::NotObserved,
    );
    emit_file(
        &mut w,
        9,
        8,
        "f3.dat",
        100,
        Some(100),
        Some(x),
        ValueKnowledge::Known(4),
    );

    // Branch 3: SubC (10) -> f4 (11, X)
    emit_dir(
        &mut w,
        10,
        1,
        "SubC",
        None,
        None,
        ValueKnowledge::NotObserved,
    );
    emit_file(
        &mut w,
        11,
        10,
        "f4.dat",
        100,
        Some(100),
        Some(x),
        ValueKnowledge::Known(4),
    );

    // Total files = 5 (4 of X, 1 of Y), total logical = 4*100 + 200 = 600, total allocated = 600
    emit_terminal(&mut w, 6, 5, 600, 600);

    let graph = build_graph(buf);

    let sub_b1 = graph.entry(5).unwrap();
    assert_eq!(sub_b1.referenced_allocated_bytes, 300);
    assert_eq!(sub_b1.unique_allocated_bytes, 300);

    let sub_b2 = graph.entry(8).unwrap();
    assert_eq!(sub_b2.referenced_allocated_bytes, 100);
    assert_eq!(sub_b2.unique_allocated_bytes, 100);

    let sub_b = graph.entry(4).unwrap();
    assert_eq!(sub_b.referenced_allocated_bytes, 400);
    // SubB contains two occurrences of X (f2 and f3) + Y: unique must be 100 (X) + 200 (Y) = 300
    assert_eq!(sub_b.unique_allocated_bytes, 300);

    let sub_a = graph.entry(2).unwrap();
    assert_eq!(sub_a.referenced_allocated_bytes, 100);
    assert_eq!(sub_a.unique_allocated_bytes, 100);

    let sub_c = graph.entry(10).unwrap();
    assert_eq!(sub_c.referenced_allocated_bytes, 100);
    assert_eq!(sub_c.unique_allocated_bytes, 100);

    let root = graph.root();
    assert_eq!(root.referenced_allocated_bytes, 600);
    // Root contains 4 occurrences of X + 1 occurrence of Y: unique = 100 + 200 = 300
    assert_eq!(root.unique_allocated_bytes, 300);
}

#[test]
fn test_deep_iterative_chain_hard_link_stack_safety() {
    let mut buf = Vec::new();
    let mut w = ObservationWriter::new(&mut buf, r"C:\Test").unwrap();
    let shared = ObjectIdentity::new(volume(50), 504);

    emit_root(&mut w);

    // Deep chain of 3,000 directories: 1 -> 2 -> 3 -> ... -> 3000
    const DEPTH: u32 = 3000;
    for d in 2..=DEPTH {
        emit_dir(
            &mut w,
            d,
            d - 1,
            &format!("d{d}"),
            None,
            None,
            ValueKnowledge::NotObserved,
        );
    }

    // Top alias directly under root (id=DEPTH+1, parent=1)
    emit_file(
        &mut w,
        DEPTH + 1,
        1,
        "top.dat",
        100,
        Some(100),
        Some(shared),
        ValueKnowledge::Known(2),
    );

    // Bottom alias deep at DEPTH (id=DEPTH+2, parent=DEPTH)
    emit_file(
        &mut w,
        DEPTH + 2,
        DEPTH,
        "bottom.dat",
        100,
        Some(100),
        Some(shared),
        ValueKnowledge::Known(2),
    );

    emit_terminal(&mut w, u64::from(DEPTH), 2, 200, 200);

    let graph = build_graph(buf);

    let bottom_dir = graph.entry(DEPTH).unwrap();
    assert_eq!(bottom_dir.referenced_allocated_bytes, 100);
    assert_eq!(bottom_dir.unique_allocated_bytes, 100);

    let root = graph.root();
    assert_eq!(root.referenced_allocated_bytes, 200);
    assert_eq!(root.unique_allocated_bytes, 100);
}

// ---------------------------------------------------------------------------
// Slice 8: Child sorting finalization & bounded pagination behavior tests.
// ---------------------------------------------------------------------------

#[test]
fn test_finalized_child_sorting_and_bounded_pagination() {
    let mut buf = Vec::new();
    let mut w = ObservationWriter::new(&mut buf, r"C:\\Test").unwrap();

    emit_root(&mut w);

    // Parent directory D (id=2, parent=1)
    emit_dir(&mut w, 2, 1, "D", None, None, ValueKnowledge::NotObserved);

    // Add 500 children under D with mixed directories and files:
    // Subdirs: d_000 .. d_099 (100 dirs)
    // Files: f_000 .. f_399 (400 files) with various sizes
    for i in 0..100 {
        emit_dir(
            &mut w,
            10 + i,
            2,
            &format!("d_{i:03}"),
            None,
            None,
            ValueKnowledge::NotObserved,
        );
    }
    for i in 0..400 {
        let size = ((400 - i) * 10) as u64; // Descending sizes: larger size first
        emit_file(
            &mut w,
            110 + i,
            2,
            &format!("f_{i:03}"),
            size,
            Some(size),
            None,
            ValueKnowledge::NotObserved,
        );
    }

    emit_terminal(&mut w, 102, 400, 802000, 802000);

    let graph = build_graph(buf);

    // Page 0 with limit 50: should contain first 50 directories
    let (total, page0) = graph.get_children_page(2, 0, 50).unwrap();
    assert_eq!(total, 500);
    assert_eq!(page0.len(), 50);
    for (idx, node) in page0.iter().enumerate() {
        assert_eq!(
            node.entry_kind, 1,
            "first 100 children must all be directories"
        );
        assert_eq!(node.name, format!("d_{idx:03}"));
    }

    // Page 1 with limit 50 (offset 50): next 50 directories (d_050 .. d_099)
    let (total1, page1) = graph.get_children_page(2, 50, 50).unwrap();
    assert_eq!(total1, 500);
    assert_eq!(page1.len(), 50);
    for (idx, node) in page1.iter().enumerate() {
        assert_eq!(node.entry_kind, 1);
        assert_eq!(node.name, format!("d_{:03}", 50 + idx));
    }

    // Page 2 with limit 50 (offset 100): first 50 files (sorted by logical size descending)
    let (total2, page2) = graph.get_children_page(2, 100, 50).unwrap();
    assert_eq!(total2, 500);
    assert_eq!(page2.len(), 50);
    for (idx, node) in page2.iter().enumerate() {
        assert_eq!(node.entry_kind, 2, "children after 100 must be files");
        assert_eq!(node.name, format!("f_{idx:03}"));
        assert_eq!(node.logical_bytes, ((400 - idx) * 10) as u64);
    }

    // Page with offset beyond total: returns 0 nodes, total preserved
    let (total_end, page_end) = graph.get_children_page(2, 500, 50).unwrap();
    assert_eq!(total_end, 500);
    assert!(page_end.is_empty());
}

// ---------------------------------------------------------------------------
// Slice 9: Compiler size_of regression tests for compact types.
// ---------------------------------------------------------------------------

#[test]
fn test_compact_types_size_of_regression() {
    use pigtree_engine::{CompactEntry, ObjectRecord};

    let compact_size = std::mem::size_of::<CompactEntry>();
    let object_record_size = std::mem::size_of::<ObjectRecord>();

    println!(
        "Measured layouts: CompactEntry = {} bytes, ObjectRecord = {} bytes",
        compact_size, object_record_size
    );
    println!(
        "Option<ObjectIdentity> = {} bytes",
        std::mem::size_of::<Option<ObjectIdentity>>()
    );
    println!(
        "ObjectIdentity = {} bytes",
        std::mem::size_of::<ObjectIdentity>()
    );
    println!(
        "ValueKnowledge<u32> = {} bytes",
        std::mem::size_of::<ValueKnowledge<u32>>()
    );
    println!(
        "ExternalReferenceStatus = {} bytes",
        std::mem::size_of::<ExternalReferenceStatus>()
    );

    // AC-12 memory target: <= 1.5 GiB Private Bytes for 5M entries.
    // 5M * 160 bytes = 800 MB core entries, leaving ample headroom for names and runtime.
    assert!(
        compact_size <= 160,
        "CompactEntry size {compact_size} bytes exceeds maximum ceiling of 160 bytes"
    );

    // ObjectRecord layout limit
    assert!(
        object_record_size <= 80,
        "ObjectRecord size {object_record_size} bytes exceeds ceiling of 80 bytes"
    );
}

// ---------------------------------------------------------------------------
// Matrix 10: Single in-target alias whose object has links outside the target.
// Issue #20 variant D — no alias badge territory, but the object must still be
// reported ConfirmedExternal and its full allocation counted once as unique.
// ---------------------------------------------------------------------------

#[test]
fn test_single_alias_with_known_external_link_confirmed_external() {
    let mut buf = Vec::new();
    let mut writer = ObservationWriter::new(&mut buf, r"C:\Test").unwrap();

    emit_root(&mut writer);

    // One entry in the target, but the object has 2 links on the volume.
    let external_id = ObjectIdentity::new(volume(7), 77);
    emit_file(
        &mut writer,
        2,
        1,
        "shared.dat",
        1000,
        Some(1024),
        Some(external_id),
        ValueKnowledge::Known(2),
    );

    emit_terminal(&mut writer, 1, 1, 1000, 1024);

    let graph = build_graph(buf);

    let file = graph.entry(2).unwrap();
    assert_eq!(file.observed_alias_count, 1);
    assert_eq!(
        file.external_reference_status,
        ExternalReferenceStatus::ConfirmedExternal,
        "N_tot=Known(2) > N_obs=1 means one link lives outside the target"
    );
    assert_eq!(file.referenced_allocated_bytes, 1024);
    assert_eq!(file.unique_allocated_bytes, 1024);

    let root = graph.root();
    assert_eq!(root.referenced_allocated_bytes, 1024);
    assert_eq!(root.unique_allocated_bytes, 1024);
    assert_eq!(
        graph.indeterminate_external_reference_objects(),
        0,
        "confirmed evidence must not raise the indeterminate summary count"
    );
}

// ---------------------------------------------------------------------------
// Matrix 11: Cloud placeholders (AC-8). Logical size is always the full
// addressable content; physical allocation is whatever the directory query
// observed — 0 for a dehydrated blob, unobserved when no evidence exists.
// Nothing here may hydrate or synthesize a size.
// ---------------------------------------------------------------------------

#[test]
fn test_cloud_placeholder_logical_preserved_allocation_observed_or_unknown() {
    const IO_REPARSE_TAG_ONEDRIVE: u32 = 0x8000_0011;
    const IO_REPARSE_TAG_FILE_PLACEHOLDER: u32 = 0x8000_0015;

    let mut buf = Vec::new();
    let mut writer = ObservationWriter::new(&mut buf, r"C:\Cloud").unwrap();

    emit_root(&mut writer);

    // Dehydrated OneDrive placeholder: directory query reports 0 allocated.
    writer
        .write_file(&FileObservation {
            entry_id: 2,
            parent_id: 1,
            name: "dehydrated.dat".to_string(),
            logical_size: 4096,
            allocated_size: Some(0),
            file_attributes: 0x20 | 0x400, // ARCHIVE | REPARSE_POINT
            reparse_tag: IO_REPARSE_TAG_ONEDRIVE,
            creation_time_utc_ms: 110,
            last_write_time_utc_ms: 210,
            last_access_time_utc_ms: 310,
            object_id: None,
            total_link_count: ValueKnowledge::NotObserved,
        })
        .unwrap();

    // Placeholder whose allocation was not established by the query.
    writer
        .write_file(&FileObservation {
            entry_id: 3,
            parent_id: 1,
            name: "unestablished.dat".to_string(),
            logical_size: 2048,
            allocated_size: None,
            file_attributes: 0x20 | 0x400,
            reparse_tag: IO_REPARSE_TAG_FILE_PLACEHOLDER,
            creation_time_utc_ms: 111,
            last_write_time_utc_ms: 211,
            last_access_time_utc_ms: 311,
            object_id: None,
            total_link_count: ValueKnowledge::NotObserved,
        })
        .unwrap();

    emit_terminal(&mut writer, 1, 2, 6144, 0);

    let graph = build_graph(buf);

    let dehydrated = graph.entry(2).unwrap();
    assert_eq!(dehydrated.logical_size, Some(4096));
    assert_eq!(dehydrated.referenced_allocated_bytes, 0);
    assert_eq!(dehydrated.unique_allocated_bytes, 0);
    assert!(dehydrated.allocated_size_known);
    assert_eq!(dehydrated.reparse_tag, IO_REPARSE_TAG_ONEDRIVE);

    let unestablished = graph.entry(3).unwrap();
    assert_eq!(unestablished.logical_size, Some(2048));
    assert!(
        !unestablished.allocated_size_known,
        "missing allocation evidence must stay unobserved, never forced to 0"
    );
    assert_eq!(unestablished.reparse_tag, IO_REPARSE_TAG_FILE_PLACEHOLDER);

    // Both entries stay ordinary files: 2 entries, no stream or placeholder inflation.
    let root = graph.root();
    assert_eq!(root.logical_size, Some(6144));
    assert!(!root.allocated_size_known);
    assert_eq!(root.known_subtotal_allocated_bytes, 0);
    assert_eq!(root.referenced_allocated_bytes, 0);
    assert_eq!(root.unique_allocated_bytes, 0);
    assert_eq!(graph.total_entries(), 3);
}
