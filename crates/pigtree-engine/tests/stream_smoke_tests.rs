use pigtree_engine::{build_graph_from_reader, DirectoryGraph, EntryKind};
use pigtree_protocol::{
    DirectoryObservation, ExternalReferenceStatus, FileObservation, ObjectIdentity,
    ObservationReader, ObservationWriter, RunOutcome, TerminalObservation, ValueKnowledge,
};
use std::io::{Cursor, Write};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpectedSmokeAggregates {
    pub total_directories: u64,
    pub total_files: u64,
    pub total_entries: usize,
    pub total_logical_bytes: u64,
    pub total_referenced_allocated_bytes: u64,
    pub total_unique_allocated_bytes: u64,
    pub confirmed_none_files: u64,
    pub confirmed_external_files: u64,
    pub not_applicable_entries: u64,
    pub indeterminate_objects: u64,
}

pub struct DeterministicStreamGenerator;

impl DeterministicStreamGenerator {
    /// Generates a fully known, consistent 10,000-entry observation stream into `writer`
    /// and returns the exact mathematically expected aggregates.
    pub fn generate_10k_stream<W: Write>(
        writer: &mut ObservationWriter<W>,
    ) -> ExpectedSmokeAggregates {
        let volume_guid = [0xAA; 16];
        const D: u32 = 100;
        const DIR_SELF_ALLOC: u64 = 4096;

        // 1. Root directory (id=1, parent=0)
        writer
            .write_directory(&DirectoryObservation {
                entry_id: 1,
                parent_id: 0,
                name: "SmokeRoot".to_string(),
                file_attributes: 0x10,
                reparse_tag: 0,
                creation_time_utc_ms: 100,
                last_write_time_utc_ms: 200,
                last_access_time_utc_ms: 300,
                object_id: None,
                allocated_size: Some(DIR_SELF_ALLOC),
                total_link_count: ValueKnowledge::NotApplicable,
            })
            .unwrap();

        // 2. Subdirectories 2..=100 in an ordered 5-ary tree
        for d in 2..=D {
            let parent_id = (d - 2) / 5 + 1;
            writer
                .write_directory(&DirectoryObservation {
                    entry_id: d,
                    parent_id,
                    name: format!("dir_{d:03}"),
                    file_attributes: 0x10,
                    reparse_tag: 0,
                    creation_time_utc_ms: 100 + u64::from(d),
                    last_write_time_utc_ms: 200 + u64::from(d),
                    last_access_time_utc_ms: 300 + u64::from(d),
                    object_id: None,
                    allocated_size: Some(DIR_SELF_ALLOC),
                    total_link_count: ValueKnowledge::NotApplicable,
                })
                .unwrap();
        }

        let mut next_entry_id: u32 = D + 1; // 101

        // 3. Independent single-link files: 4,900 files
        // Each has logical 1000, alloc 1024, Known(1) link count -> ConfirmedNone
        const NUM_INDEPENDENT: u32 = 4900;
        for i in 0..NUM_INDEPENDENT {
            let parent_id = (i % D) + 1;
            let oid = ObjectIdentity::new(volume_guid, u128::from(next_entry_id));
            writer
                .write_file(&FileObservation {
                    entry_id: next_entry_id,
                    parent_id,
                    name: format!("ind_{i:04}.dat"),
                    logical_size: 1000,
                    allocated_size: Some(1024),
                    file_attributes: 0x20,
                    reparse_tag: 0,
                    creation_time_utc_ms: 1000 + u64::from(next_entry_id),
                    last_write_time_utc_ms: 2000 + u64::from(next_entry_id),
                    last_access_time_utc_ms: 3000 + u64::from(next_entry_id),
                    object_id: Some(oid),
                    total_link_count: ValueKnowledge::Known(1),
                })
                .unwrap();
            next_entry_id += 1;
        }

        // 4. Hard-link groups of 3 aliases: 1,650 objects * 3 = 4,950 files
        // Each has logical 2000, alloc 2048, Known(3) link count -> ConfirmedNone
        const NUM_GROUPS_3: u32 = 1650;
        for g in 0..NUM_GROUPS_3 {
            let oid = ObjectIdentity::new(volume_guid, 1_000_000 + u128::from(g));
            for alias_idx in 0..3 {
                let parent_id = ((g * 3 + alias_idx) % D) + 1;
                writer
                    .write_file(&FileObservation {
                        entry_id: next_entry_id,
                        parent_id,
                        name: format!("hl3_g{g:04}_a{alias_idx}.dat"),
                        logical_size: 2000,
                        allocated_size: Some(2048),
                        file_attributes: 0x20,
                        reparse_tag: 0,
                        creation_time_utc_ms: 1000 + u64::from(next_entry_id),
                        last_write_time_utc_ms: 2000 + u64::from(next_entry_id),
                        last_access_time_utc_ms: 3000 + u64::from(next_entry_id),
                        object_id: Some(oid),
                        total_link_count: ValueKnowledge::Known(3),
                    })
                    .unwrap();
                next_entry_id += 1;
            }
        }

        // 5. ConfirmedExternal hard-link groups of 2 aliases: 25 objects * 2 = 50 files
        // Total link count Known(5) > observed 2 -> ConfirmedExternal
        const NUM_GROUPS_EXT: u32 = 25;
        for g in 0..NUM_GROUPS_EXT {
            let oid = ObjectIdentity::new(volume_guid, 2_000_000 + u128::from(g));
            for alias_idx in 0..2 {
                let parent_id = ((g * 2 + alias_idx) % D) + 1;
                writer
                    .write_file(&FileObservation {
                        entry_id: next_entry_id,
                        parent_id,
                        name: format!("hlext_g{g:02}_a{alias_idx}.dat"),
                        logical_size: 4000,
                        allocated_size: Some(4096),
                        file_attributes: 0x20,
                        reparse_tag: 0,
                        creation_time_utc_ms: 1000 + u64::from(next_entry_id),
                        last_write_time_utc_ms: 2000 + u64::from(next_entry_id),
                        last_access_time_utc_ms: 3000 + u64::from(next_entry_id),
                        object_id: Some(oid),
                        total_link_count: ValueKnowledge::Known(5),
                    })
                    .unwrap();
                next_entry_id += 1;
            }
        }

        assert_eq!(next_entry_id, 10001);

        let total_files = u64::from(NUM_INDEPENDENT + NUM_GROUPS_3 * 3 + NUM_GROUPS_EXT * 2);
        let total_dirs = u64::from(D);
        let total_logical = u64::from(NUM_INDEPENDENT) * 1000
            + u64::from(NUM_GROUPS_3 * 3) * 2000
            + u64::from(NUM_GROUPS_EXT * 2) * 4000;
        let total_ref_alloc = total_dirs * DIR_SELF_ALLOC
            + u64::from(NUM_INDEPENDENT) * 1024
            + u64::from(NUM_GROUPS_3 * 3) * 2048
            + u64::from(NUM_GROUPS_EXT * 2) * 4096;
        let total_uniq_alloc = total_dirs * DIR_SELF_ALLOC
            + u64::from(NUM_INDEPENDENT) * 1024
            + u64::from(NUM_GROUPS_3) * 2048
            + u64::from(NUM_GROUPS_EXT) * 4096;

        // 6. Terminal
        writer
            .write_terminal(&TerminalObservation {
                outcome: RunOutcome::Finished,
                total_directories: total_dirs,
                total_files,
                total_logical_bytes: total_logical,
                total_allocated_bytes: total_ref_alloc,
                coverage_gap_count: 0,
                duration_ms: 42,
            })
            .unwrap();

        ExpectedSmokeAggregates {
            total_directories: total_dirs,
            total_files,
            total_entries: (total_dirs + total_files) as usize,
            total_logical_bytes: total_logical,
            total_referenced_allocated_bytes: total_ref_alloc,
            total_unique_allocated_bytes: total_uniq_alloc,
            confirmed_none_files: u64::from(NUM_INDEPENDENT + NUM_GROUPS_3 * 3),
            confirmed_external_files: u64::from(NUM_GROUPS_EXT * 2),
            not_applicable_entries: total_dirs,
            indeterminate_objects: 0,
        }
    }
}

#[test]
fn test_10k_public_seam_correctness_smoke() {
    let mut buf = Vec::with_capacity(2 * 1024 * 1024);
    let mut writer = ObservationWriter::new(&mut buf, r"C:\SmokeTest").unwrap();

    let expected = DeterministicStreamGenerator::generate_10k_stream(&mut writer);

    // Verify raw buffer was written
    assert!(!buf.is_empty());

    // Build graph through public reader seam
    let reader = ObservationReader::new(Cursor::new(buf)).unwrap();
    let graph: DirectoryGraph = build_graph_from_reader(reader).unwrap();

    // 1. Total entry counts
    assert_eq!(graph.total_entries(), expected.total_entries);
    assert_eq!(
        graph.terminal().total_directories,
        expected.total_directories
    );
    assert_eq!(graph.terminal().total_files, expected.total_files);

    // 2. Exact expected root aggregates
    assert_eq!(graph.logical_bytes(), expected.total_logical_bytes);
    assert_eq!(
        graph.referenced_allocated_bytes(),
        expected.total_referenced_allocated_bytes
    );
    assert_eq!(
        graph.unique_allocated_bytes(),
        expected.total_unique_allocated_bytes
    );
    assert_eq!(
        graph.known_subtotal_allocated_bytes(),
        expected.total_referenced_allocated_bytes
    );
    assert!(graph.allocated_bytes_known());
    assert_eq!(
        graph.indeterminate_external_reference_objects(),
        expected.indeterminate_objects
    );

    // 3. Exact external reference status counts across all entries
    let mut count_confirmed_none: u64 = 0;
    let mut count_confirmed_ext: u64 = 0;
    let mut count_not_applicable: u64 = 0;
    let mut count_indeterminate: u64 = 0;
    let mut count_inconsistent: u64 = 0;

    for id in 1..=(expected.total_entries as u32) {
        let entry = graph
            .entry(id)
            .unwrap_or_else(|| panic!("entry {id} missing"));
        match entry.external_reference_status {
            ExternalReferenceStatus::ConfirmedNone => count_confirmed_none += 1,
            ExternalReferenceStatus::ConfirmedExternal => count_confirmed_ext += 1,
            ExternalReferenceStatus::NotApplicable => count_not_applicable += 1,
            ExternalReferenceStatus::Indeterminate => count_indeterminate += 1,
            ExternalReferenceStatus::InconsistentEvidence => count_inconsistent += 1,
        }
        if entry.kind == EntryKind::Directory {
            assert_eq!(
                entry.external_reference_status,
                ExternalReferenceStatus::NotApplicable
            );
            assert_eq!(entry.total_link_count, ValueKnowledge::NotApplicable);
        }
    }

    assert_eq!(count_confirmed_none, expected.confirmed_none_files);
    assert_eq!(count_confirmed_ext, expected.confirmed_external_files);
    assert_eq!(count_not_applicable, expected.not_applicable_entries);
    assert_eq!(count_indeterminate, 0);
    assert_eq!(count_inconsistent, 0);

    // 4. Bounded child pagination on root (dir 1) and subdirectories
    let (root_child_count, root_children) = graph.get_children_page(1, 0, 10).unwrap();
    assert!(root_child_count > 0);
    assert_eq!(root_children.len(), 10);
    // Directories must come first
    for child in &root_children {
        if child.entry_kind == 1 {
            assert_eq!(
                child.external_reference_status,
                pigtree_protocol::protobuf::ExternalReferenceStatusProto::ExternalReferenceStatusNotApplicable as i32
            );
        }
    }

    // Direct page beyond limit
    let (total_d2, d2_page) = graph.get_children_page(2, 0, 50).unwrap();
    assert!(total_d2 > 0);
    assert_eq!(d2_page.len(), 50.min(total_d2));
}
