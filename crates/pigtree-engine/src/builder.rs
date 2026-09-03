//! Builder for constructing a DirectoryGraph from an observation stream.

use crate::error::GraphBuildError;
use crate::graph::{
    CompactEntry, DirectoryGraph, EntryKind, EntryStorage, ObjectRecord, StreamBreakdown,
    NO_OBJECT,
};
use pigtree_protocol::protobuf::ScanProgress;
use pigtree_protocol::{
    CoverageGapObservation, DirectoryObservation, ExternalReferenceStatus, FileObservation,
    ObjectIdentity, ObservationReader, ObservationRecord, SpecialObservation, StreamObservation,
    TerminalObservation, ValueKnowledge,
};
use std::collections::HashMap;
use std::io::Read;

pub use pigtree_protocol::json::format_utc_iso;

#[derive(Debug)]
pub struct GraphBuilder {
    root_target: String,
    entries: Vec<CompactEntry>,
    is_dense: bool,
    id_to_idx: HashMap<u32, u32>,
    first_child: Vec<u32>,
    last_child: Vec<u32>,
    next_sibling: Vec<u32>,
    file_object_map: HashMap<ObjectIdentity, Vec<u32>>,
    dir_object_ids: HashMap<u32, ObjectIdentity>,
    file_link_counts: HashMap<u32, ValueKnowledge<u32>>,
    dir_self_alloc: Vec<u64>,
    pending_streams: Vec<(usize, StreamBreakdown)>,
    object_streams: HashMap<u32, Vec<StreamBreakdown>>,
    gaps: Vec<CoverageGapObservation>,
    terminal: Option<TerminalObservation>,
    dir_count: u64,
    file_count: u64,
    special_count: u64,
    logical_bytes: u64,
    allocated_bytes: u64,
    allocated_bytes_known: bool,
    current_dir_id: u32,
}

impl GraphBuilder {
    pub fn new(root_target: impl Into<String>) -> Self {
        const INITIAL_CAPACITY: usize = 128;
        Self {
            root_target: root_target.into(),
            entries: Vec::with_capacity(INITIAL_CAPACITY),
            is_dense: true,
            id_to_idx: HashMap::new(),
            first_child: Vec::with_capacity(INITIAL_CAPACITY),
            last_child: Vec::with_capacity(INITIAL_CAPACITY),
            next_sibling: Vec::with_capacity(INITIAL_CAPACITY),
            file_object_map: HashMap::new(),
            dir_object_ids: HashMap::new(),
            file_link_counts: HashMap::new(),
            dir_self_alloc: Vec::with_capacity(INITIAL_CAPACITY),
            pending_streams: Vec::new(),
            object_streams: HashMap::new(),
            gaps: Vec::new(),
            terminal: None,
            dir_count: 0,
            file_count: 0,
            special_count: 0,
            logical_bytes: 0,
            allocated_bytes: 0,
            allocated_bytes_known: true,
            current_dir_id: 0,
        }
    }

    pub fn ingest_record(&mut self, record: ObservationRecord) -> Result<(), GraphBuildError> {
        if self.terminal.is_some() {
            return Err(GraphBuildError::RecordAfterTerminal);
        }

        match record {
            ObservationRecord::Directory(dir) => {
                self.current_dir_id = if dir.parent_id == 0 {
                    dir.entry_id
                } else {
                    dir.parent_id
                };
                self.ingest_directory(dir)
            }
            ObservationRecord::File(file) => {
                self.current_dir_id = file.parent_id;
                self.ingest_file(file)
            }
            ObservationRecord::Special(special) => {
                self.current_dir_id = special.parent_id;
                self.ingest_special(special)
            }
            ObservationRecord::ContentStream(stream) => self.ingest_stream(stream),
            ObservationRecord::CoverageGap(gap) => {
                self.gaps.push(gap);
                Ok(())
            }
            ObservationRecord::Terminal(term) => self.ingest_terminal(term),
        }
    }

    fn validate_and_insert_entry(
        &mut self,
        id: u32,
        parent_id: u32,
        entry: CompactEntry,
        self_alloc: u64,
    ) -> Result<(), GraphBuildError> {
        if id == parent_id {
            return Err(GraphBuildError::SelfParent(id));
        }

        let curr_idx = self.entries.len();

        if curr_idx == 0 {
            if id != 1 || parent_id != 0 {
                return Err(GraphBuildError::InvalidRoot {
                    entry_id: id,
                    parent_id,
                });
            }
            if entry.kind != EntryKind::Directory {
                return Err(GraphBuildError::InvalidRoot {
                    entry_id: id,
                    parent_id,
                });
            }
        } else {
            if parent_id == 0 || id == 1 {
                return Err(GraphBuildError::InvalidRoot {
                    entry_id: id,
                    parent_id,
                });
            }

            // Check if sequential or switch to sparse mode
            let expected_seq_id = (curr_idx + 1) as u32;
            if self.is_dense && id != expected_seq_id {
                self.is_dense = false;
                self.id_to_idx.reserve(curr_idx + 1);
                for i in 0..curr_idx {
                    self.id_to_idx.insert(self.entries[i].id, i as u32);
                }
            }

            // Check for duplicate ID
            if self.is_dense {
                if id <= curr_idx as u32 {
                    return Err(GraphBuildError::DuplicateEntryId(id));
                }
            } else if self.id_to_idx.contains_key(&id) {
                return Err(GraphBuildError::DuplicateEntryId(id));
            }

            // Lookup parent
            let parent_idx = if self.is_dense {
                if parent_id > curr_idx as u32 {
                    return Err(GraphBuildError::MissingParent {
                        entry_id: id,
                        parent_id,
                    });
                }
                (parent_id - 1) as usize
            } else {
                let &p_idx =
                    self.id_to_idx
                        .get(&parent_id)
                        .ok_or(GraphBuildError::MissingParent {
                            entry_id: id,
                            parent_id,
                        })?;
                p_idx as usize
            };

            if self.entries[parent_idx].kind != EntryKind::Directory {
                return Err(GraphBuildError::ParentNotDirectory {
                    entry_id: id,
                    parent_id,
                });
            }

            // Link into parent's child list using first_child/next_sibling (zero Vec allocations)
            if self.first_child[parent_idx] == u32::MAX {
                self.first_child[parent_idx] = curr_idx as u32;
                self.last_child[parent_idx] = curr_idx as u32;
            } else {
                let prev = self.last_child[parent_idx] as usize;
                self.next_sibling[prev] = curr_idx as u32;
                self.last_child[parent_idx] = curr_idx as u32;
            }
        }

        if !self.is_dense {
            self.id_to_idx.insert(id, curr_idx as u32);
        }

        self.entries.push(entry);
        self.dir_self_alloc.push(self_alloc);
        self.first_child.push(u32::MAX);
        self.last_child.push(u32::MAX);
        self.next_sibling.push(u32::MAX);

        Ok(())
    }

    fn ingest_directory(&mut self, dir: DirectoryObservation) -> Result<(), GraphBuildError> {
        let alloc_size = dir.allocated_size;
        let self_alloc = alloc_size.unwrap_or(0);

        let entry = CompactEntry {
            id: dir.entry_id,
            parent_id: dir.parent_id,
            name: dir.name,
            kind: EntryKind::Directory,
            allocated_size_known: true,
            logical_size: None,
            allocated_size: alloc_size,
            referenced_allocated_bytes: self_alloc,
            unique_allocated_bytes: self_alloc,
            known_subtotal_allocated_bytes: self_alloc,
            object_index: NO_OBJECT,
            file_attributes: dir.file_attributes,
            reparse_tag: dir.reparse_tag,
            child_start: 0,
            child_count: 0,
            creation_time_utc_ms: dir.creation_time_utc_ms,
            last_write_time_utc_ms: dir.last_write_time_utc_ms,
            last_access_time_utc_ms: dir.last_access_time_utc_ms,
        };

        let id = dir.entry_id;
        self.validate_and_insert_entry(id, dir.parent_id, entry, self_alloc)?;

        if let Some(oid) = dir.object_id {
            self.dir_object_ids.insert(id, oid);
        }

        self.dir_count += 1;
        if let Some(alloc) = alloc_size {
            self.allocated_bytes = self.allocated_bytes.saturating_add(alloc);
        }
        Ok(())
    }

    fn ingest_file(&mut self, file: FileObservation) -> Result<(), GraphBuildError> {
        let alloc_size = file.allocated_size;
        let alloc_known = alloc_size.is_some();
        let alloc_bytes = alloc_size.unwrap_or(0);

        let entry = CompactEntry {
            id: file.entry_id,
            parent_id: file.parent_id,
            name: file.name,
            kind: EntryKind::File,
            allocated_size_known: alloc_known,
            logical_size: Some(file.logical_size),
            allocated_size: alloc_size,
            referenced_allocated_bytes: alloc_bytes,
            unique_allocated_bytes: alloc_bytes,
            known_subtotal_allocated_bytes: alloc_bytes,
            object_index: NO_OBJECT,
            file_attributes: file.file_attributes,
            reparse_tag: file.reparse_tag,
            child_start: 0,
            child_count: 0,
            creation_time_utc_ms: file.creation_time_utc_ms,
            last_write_time_utc_ms: file.last_write_time_utc_ms,
            last_access_time_utc_ms: file.last_access_time_utc_ms,
        };

        let id = file.entry_id;
        self.validate_and_insert_entry(id, file.parent_id, entry, 0)?;

        if let Some(oid) = file.object_id {
            self.file_object_map.entry(oid).or_default().push(id);
        }
        if file.total_link_count != ValueKnowledge::NotObserved {
            self.file_link_counts.insert(id, file.total_link_count);
        }

        self.file_count += 1;
        self.logical_bytes = self.logical_bytes.saturating_add(file.logical_size);
        if let Some(alloc) = alloc_size {
            self.allocated_bytes = self.allocated_bytes.saturating_add(alloc);
        } else {
            self.allocated_bytes_known = false;
        }
        Ok(())
    }

    fn ingest_special(&mut self, special: SpecialObservation) -> Result<(), GraphBuildError> {
        let entry = CompactEntry {
            id: special.entry_id,
            parent_id: special.parent_id,
            name: special.name,
            kind: EntryKind::Special,
            allocated_size_known: false,
            logical_size: None,
            allocated_size: None,
            referenced_allocated_bytes: 0,
            unique_allocated_bytes: 0,
            known_subtotal_allocated_bytes: 0,
            object_index: NO_OBJECT,
            file_attributes: special.file_attributes,
            reparse_tag: special.reparse_tag,
            child_start: 0,
            child_count: 0,
            creation_time_utc_ms: special.creation_time_utc_ms,
            last_write_time_utc_ms: special.last_write_time_utc_ms,
            last_access_time_utc_ms: special.last_access_time_utc_ms,
        };

        self.validate_and_insert_entry(special.entry_id, special.parent_id, entry, 0)?;
        self.special_count += 1;
        Ok(())
    }

    /// Attributes a secondary content stream to the object behind its parent
    /// entry. Streams never create entries and never change scope aggregates;
    /// object records are only finalized at settlement, so the parent entry
    /// index is captured now and the mapping to an object happens there. When
    /// the parent object ends up unresolvable (identity unobserved), the
    /// stream has no attributable owner and is dropped.
    fn ingest_stream(&mut self, stream: StreamObservation) -> Result<(), GraphBuildError> {
        let entry_idx = self
            .storage_get_idx(stream.parent_entry_id)
            .ok_or(GraphBuildError::UnknownStreamParent {
                entry_id: stream.parent_entry_id,
            })?;

        self.pending_streams.push((
            entry_idx,
            StreamBreakdown {
                name: stream.name,
                logical_bytes: stream.logical_size,
                allocated_bytes: stream.allocated_size,
            },
        ));
        Ok(())
    }

    fn storage_get_idx(&self, id: u32) -> Option<usize> {
        if self.is_dense {
            if id == 0 || id as usize > self.entries.len() {
                None
            } else {
                Some(id as usize - 1)
            }
        } else {
            self.id_to_idx.get(&id).map(|&idx| idx as usize)
        }
    }

    fn ingest_terminal(&mut self, term: TerminalObservation) -> Result<(), GraphBuildError> {
        if term.total_directories != self.dir_count {
            return Err(GraphBuildError::AggregateMismatch {
                field: "total_directories",
                expected: term.total_directories,
                actual: self.dir_count,
            });
        }

        if term.total_files != self.file_count {
            return Err(GraphBuildError::AggregateMismatch {
                field: "total_files",
                expected: term.total_files,
                actual: self.file_count,
            });
        }

        if term.total_logical_bytes != self.logical_bytes {
            return Err(GraphBuildError::AggregateMismatch {
                field: "total_logical_bytes",
                expected: term.total_logical_bytes,
                actual: self.logical_bytes,
            });
        }

        if term.total_allocated_bytes != self.allocated_bytes {
            return Err(GraphBuildError::AggregateMismatch {
                field: "total_allocated_bytes",
                expected: term.total_allocated_bytes,
                actual: self.allocated_bytes,
            });
        }

        if term.coverage_gap_count != self.gaps.len() as u32 {
            return Err(GraphBuildError::AggregateMismatch {
                field: "coverage_gap_count",
                expected: term.coverage_gap_count as u64,
                actual: self.gaps.len() as u64,
            });
        }

        self.terminal = Some(term);
        Ok(())
    }

    pub fn allocated_bytes_known(&self) -> bool {
        self.allocated_bytes_known
    }

    pub fn current_directory_path(&self) -> String {
        self.reconstruct_path(self.current_dir_id)
    }

    pub fn reconstruct_path(&self, dir_id: u32) -> String {
        if dir_id == 0 || self.entries.is_empty() {
            return self.root_target.clone();
        }

        let mut components = Vec::new();
        let mut curr = dir_id;
        let mut depth = 0;
        while curr != 0 && depth < 10000 {
            let entry = if self.is_dense {
                if curr >= 1 && curr <= self.entries.len() as u32 {
                    &self.entries[(curr - 1) as usize]
                } else {
                    break;
                }
            } else if let Some(&idx) = self.id_to_idx.get(&curr) {
                &self.entries[idx as usize]
            } else {
                break;
            };

            components.push(entry.name.as_str());
            curr = entry.parent_id;
            depth += 1;
        }

        if components.is_empty() {
            return self.root_target.clone();
        }

        components.reverse();
        let mut result = components[0].to_string();
        for comp in &components[1..] {
            if !result.ends_with('\\') && !result.ends_with('/') {
                result.push('\\');
            }
            result.push_str(comp);
        }
        result
    }

    /// Computes recursive directory aggregates and distinct hard-link unique allocation
    /// using normalized ObjectRecord storage, an offline iterative Tarjan LCA algorithm,
    /// and bottom-up signed arithmetic.
    fn compute_aggregates(&mut self) -> (Vec<ObjectRecord>, Vec<u32>, u64) {
        let n = self.entries.len();
        if n == 0 {
            return (Vec::new(), Vec::new(), 0);
        }

        let mut objects: Vec<ObjectRecord> = Vec::new();
        let mut indeterminate_objects: u64 = 0;

        // Phase 1: Normalize canonical file objects, detect link/allocation conflicts
        let file_object_map = std::mem::take(&mut self.file_object_map);
        for (oid, entry_ids) in file_object_map {
            let observed_count = entry_ids.len() as u32;

            // Link resolution with conflict detection (Slice 4)
            let mut resolved_link = ValueKnowledge::NotObserved;
            let mut has_link_conflict = false;
            let mut first_known_link: Option<u32> = None;

            for &id in &entry_ids {
                let link = self
                    .file_link_counts
                    .get(&id)
                    .copied()
                    .unwrap_or(ValueKnowledge::NotObserved);
                match link {
                    ValueKnowledge::Known(k) => match first_known_link {
                        None => {
                            first_known_link = Some(k);
                            resolved_link = ValueKnowledge::Known(k);
                        }
                        Some(prev) => {
                            if prev != k {
                                has_link_conflict = true;
                            }
                        }
                    },
                    ValueKnowledge::Unavailable => {
                        if first_known_link.is_none() {
                            resolved_link = ValueKnowledge::Unavailable;
                        }
                    }
                    ValueKnowledge::NotApplicable => {
                        if first_known_link.is_none()
                            && !matches!(resolved_link, ValueKnowledge::Unavailable)
                        {
                            resolved_link = ValueKnowledge::NotApplicable;
                        }
                    }
                    ValueKnowledge::NotObserved => {}
                }
            }

            let status = if has_link_conflict {
                ExternalReferenceStatus::InconsistentEvidence
            } else {
                ExternalReferenceStatus::derive(resolved_link, observed_count)
            };

            if status == ExternalReferenceStatus::Indeterminate {
                indeterminate_objects += 1;
            }

            // Allocation weight lower-bound and conflict detection (Slice 3)
            let mut min_alloc: Option<u64> = None;
            let mut has_alloc_conflict = false;
            let mut first_alloc: Option<u64> = None;

            for &id in &entry_ids {
                let e_idx = if self.is_dense {
                    (id - 1) as usize
                } else {
                    self.id_to_idx[&id] as usize
                };
                if let Some(alloc) = self.entries[e_idx].allocated_size {
                    min_alloc = Some(min_alloc.map_or(alloc, |m| m.min(alloc)));
                    match first_alloc {
                        None => first_alloc = Some(alloc),
                        Some(prev) => {
                            if prev != alloc {
                                has_alloc_conflict = true;
                            }
                        }
                    }
                }
            }

            if has_alloc_conflict {
                for &id in &entry_ids {
                    let e_idx = if self.is_dense {
                        (id - 1) as usize
                    } else {
                        self.id_to_idx[&id] as usize
                    };
                    self.entries[e_idx].allocated_size_known = false;
                }
                self.allocated_bytes_known = false;
            }

            let weight = min_alloc.unwrap_or(0);
            let obj_idx = objects.len() as u32;
            objects.push(ObjectRecord {
                identity: Some(oid),
                observed_alias_count: observed_count,
                total_link_count: resolved_link,
                external_reference_status: status,
                weight,
            });

            for &id in &entry_ids {
                let e_idx = if self.is_dense {
                    (id - 1) as usize
                } else {
                    self.id_to_idx[&id] as usize
                };
                self.entries[e_idx].object_index = obj_idx;
            }
        }

        // Normalize directory canonical identities
        let dir_object_ids = std::mem::take(&mut self.dir_object_ids);
        for (id, oid) in dir_object_ids {
            let e_idx = if self.is_dense {
                (id - 1) as usize
            } else {
                self.id_to_idx[&id] as usize
            };
            let obj_idx = objects.len() as u32;
            objects.push(ObjectRecord {
                identity: Some(oid),
                observed_alias_count: 1,
                total_link_count: ValueKnowledge::NotApplicable,
                external_reference_status: ExternalReferenceStatus::NotApplicable,
                weight: 0,
            });
            self.entries[e_idx].object_index = obj_idx;
        }

        // Account for identity-less files
        for entry in &mut self.entries {
            if entry.kind == EntryKind::File && entry.object_index == NO_OBJECT {
                let link = self
                    .file_link_counts
                    .get(&entry.id)
                    .copied()
                    .unwrap_or(ValueKnowledge::NotObserved);
                let status = ExternalReferenceStatus::derive(link, 1);
                if status == ExternalReferenceStatus::Indeterminate {
                    indeterminate_objects += 1;
                }
                if link != ValueKnowledge::NotObserved {
                    let obj_idx = objects.len() as u32;
                    objects.push(ObjectRecord {
                        identity: None,
                        observed_alias_count: 1,
                        total_link_count: link,
                        external_reference_status: status,
                        weight: entry.allocated_size.unwrap_or(0),
                    });
                    entry.object_index = obj_idx;
                }
            }
        }

        // Object records are final here: attribute pending streams to their
        // owning objects, preserving observation order. Streams whose parent
        // object was never resolved stay dropped — they have no owner.
        let pending_streams = std::mem::take(&mut self.pending_streams);
        if !pending_streams.is_empty() {
            self.object_streams.reserve(pending_streams.len());
            for (entry_idx, breakdown) in pending_streams {
                let object_index = self.entries[entry_idx].object_index;
                if object_index != NO_OBJECT {
                    self.object_streams
                        .entry(object_index)
                        .or_default()
                        .push(breakdown);
                }
            }
        }

        // Phase 2: Iterative Tarjan's offline LCA & postorder DFS traversal
        let mut tree_delta: Vec<i128> = vec![0; n];
        let mut uf_parent: Vec<u32> = (0..n as u32).collect();
        let mut ancestor: Vec<u32> = (0..n as u32).collect();
        let mut post_order: Vec<usize> = Vec::with_capacity(n);
        let mut last_occurrence: Vec<Option<usize>> = vec![None; objects.len()];
        let mut visited: Vec<bool> = vec![false; n];

        fn find_uf(parent: &mut [u32], i: usize) -> usize {
            let mut root = i;
            while parent[root] as usize != root {
                root = parent[root] as usize;
            }
            let mut curr = i;
            while curr != root {
                let nxt = parent[curr] as usize;
                parent[curr] = root as u32;
                curr = nxt;
            }
            root
        }

        let mut stack: Vec<(usize, u32)> = Vec::with_capacity(128);

        // Root entry is always index 0
        visited[0] = true;
        stack.push((0, self.first_child[0]));

        while let Some((curr_idx, next_child)) = stack.last_mut() {
            let curr = *curr_idx;
            if *next_child != u32::MAX {
                let child = *next_child as usize;
                *next_child = self.next_sibling[child];
                if !visited[child] {
                    visited[child] = true;
                    stack.push((child, self.first_child[child]));
                }
            } else {
                stack.pop();
                let compact = &self.entries[curr];
                if compact.kind == EntryKind::File && compact.object_index != NO_OBJECT {
                    let obj_idx = compact.object_index as usize;
                    let weight = objects[obj_idx].weight;
                    if weight > 0 {
                        if let Some(prev_idx) = last_occurrence[obj_idx] {
                            let rep = find_uf(&mut uf_parent, prev_idx);
                            let lca_idx = ancestor[rep] as usize;
                            tree_delta[lca_idx] = tree_delta[lca_idx]
                                .checked_sub(weight as i128)
                                .expect("delta subtraction underflow");
                        }
                        last_occurrence[obj_idx] = Some(curr);
                    }
                }

                post_order.push(curr);

                if let Some(&(parent_idx, _)) = stack.last() {
                    let root_p = find_uf(&mut uf_parent, parent_idx);
                    let root_c = find_uf(&mut uf_parent, curr);
                    uf_parent[root_c] = root_p as u32;
                    ancestor[root_p] = parent_idx as u32;
                }
            }
        }

        // Traverse any disconnected components in unusual or cancelled streams
        if post_order.len() < n {
            for i in 0..n {
                if !visited[i] {
                    visited[i] = true;
                    stack.push((i, self.first_child[i]));
                    while let Some((curr_idx, next_child)) = stack.last_mut() {
                        let curr = *curr_idx;
                        if *next_child != u32::MAX {
                            let child = *next_child as usize;
                            *next_child = self.next_sibling[child];
                            if !visited[child] {
                                visited[child] = true;
                                stack.push((child, self.first_child[child]));
                            }
                        } else {
                            stack.pop();
                            let compact = &self.entries[curr];
                            if compact.kind == EntryKind::File && compact.object_index != NO_OBJECT
                            {
                                let obj_idx = compact.object_index as usize;
                                let weight = objects[obj_idx].weight;
                                if weight > 0 {
                                    if let Some(prev_idx) = last_occurrence[obj_idx] {
                                        let rep = find_uf(&mut uf_parent, prev_idx);
                                        let lca_idx = ancestor[rep] as usize;
                                        tree_delta[lca_idx] = tree_delta[lca_idx]
                                            .checked_sub(weight as i128)
                                            .expect("delta subtraction underflow");
                                    }
                                    last_occurrence[obj_idx] = Some(curr);
                                }
                            }
                            post_order.push(curr);
                            if let Some(&(parent_idx, _)) = stack.last() {
                                let root_p = find_uf(&mut uf_parent, parent_idx);
                                let root_c = find_uf(&mut uf_parent, curr);
                                uf_parent[root_c] = root_p as u32;
                                ancestor[root_p] = parent_idx as u32;
                            }
                        }
                    }
                }
            }
        }

        // Phase 3: Set unique allocation on file leaves
        for &idx in &post_order {
            let compact = &mut self.entries[idx];
            if compact.kind == EntryKind::File {
                if compact.object_index != NO_OBJECT {
                    compact.unique_allocated_bytes = objects[compact.object_index as usize].weight;
                } else {
                    compact.unique_allocated_bytes = compact.allocated_size.unwrap_or(0);
                }
            } else if compact.kind == EntryKind::Special {
                compact.unique_allocated_bytes = 0;
            }
        }

        // Phase 4: Bottom-up directory size and unique aggregation
        for &idx in &post_order {
            if self.entries[idx].kind != EntryKind::Directory {
                continue;
            }

            let mut sum_logical: u64 = 0;
            let mut sum_referenced: u64 = self.dir_self_alloc[idx];
            let mut sum_known_subtotal: u64 = self.dir_self_alloc[idx];
            let mut child_unique_sum: i128 = 0;
            let mut all_known = true;

            let mut c = self.first_child[idx];
            while c != u32::MAX {
                let child = &self.entries[c as usize];
                sum_logical = sum_logical.saturating_add(child.logical_size.unwrap_or(0));
                sum_referenced = sum_referenced.saturating_add(child.referenced_allocated_bytes);
                sum_known_subtotal =
                    sum_known_subtotal.saturating_add(child.known_subtotal_allocated_bytes);
                child_unique_sum = child_unique_sum
                    .checked_add(child.unique_allocated_bytes as i128)
                    .expect("child unique sum overflow");

                if !child.allocated_size_known {
                    all_known = false;
                }
                if child.kind == EntryKind::File && child.allocated_size.is_none() {
                    all_known = false;
                }
                c = self.next_sibling[c as usize];
            }

            let self_alloc = self.dir_self_alloc[idx];
            let net_unique: i128 = child_unique_sum
                .checked_add(tree_delta[idx])
                .expect("unique calculation overflow")
                .checked_add(self_alloc as i128)
                .expect("unique calculation overflow");

            assert!(
                net_unique >= 0,
                "negative unique allocated bytes ({net_unique}) for directory ID {}: invariant violation",
                self.entries[idx].id
            );

            let dir_entry = &mut self.entries[idx];
            dir_entry.logical_size = Some(sum_logical);
            dir_entry.referenced_allocated_bytes = sum_referenced;
            dir_entry.known_subtotal_allocated_bytes = sum_known_subtotal;
            dir_entry.allocated_size = Some(sum_referenced);
            dir_entry.allocated_size_known = all_known;
            dir_entry.unique_allocated_bytes = net_unique as u64;
        }

        // Phase 5: Deterministic child sorting into centralized all_children slice
        let mut all_children: Vec<u32> = Vec::with_capacity(n);
        let is_dense = self.is_dense;

        for idx in 0..n {
            if self.entries[idx].kind == EntryKind::Directory {
                let mut child_ids = Vec::new();
                let mut c = self.first_child[idx];
                while c != u32::MAX {
                    child_ids.push(self.entries[c as usize].id);
                    c = self.next_sibling[c as usize];
                }

                let lookup_entry =
                    |id: u32, _entries: &[CompactEntry], id_to_idx: &HashMap<u32, u32>| -> usize {
                        if is_dense {
                            (id - 1) as usize
                        } else {
                            id_to_idx[&id] as usize
                        }
                    };

                child_ids.sort_by(|&a_id, &b_id| {
                    let a = &self.entries[lookup_entry(a_id, &self.entries, &self.id_to_idx)];
                    let b = &self.entries[lookup_entry(b_id, &self.entries, &self.id_to_idx)];

                    let a_is_dir = a.kind == EntryKind::Directory;
                    let b_is_dir = b.kind == EntryKind::Directory;
                    if a_is_dir != b_is_dir {
                        return b_is_dir.cmp(&a_is_dir); // Directories first
                    }

                    let a_logical = a.logical_size.unwrap_or(0);
                    let b_logical = b.logical_size.unwrap_or(0);
                    if a_logical != b_logical {
                        return b_logical.cmp(&a_logical); // Logical size descending
                    }

                    let a_lower = a.name.to_lowercase();
                    let b_lower = b.name.to_lowercase();
                    if a_lower != b_lower {
                        return a_lower.cmp(&b_lower); // Name case-insensitive ascending
                    }

                    if a.name != b.name {
                        return a.name.cmp(&b.name);
                    }

                    a_id.cmp(&b_id) // ID ascending
                });

                self.entries[idx].child_start = all_children.len() as u32;
                self.entries[idx].child_count = child_ids.len() as u32;
                all_children.extend_from_slice(&child_ids);
            } else {
                self.entries[idx].child_start = 0;
                self.entries[idx].child_count = 0;
            }
        }

        // Phase 6: Intermediate buffers (tree_delta, uf_parent, ancestor, post_order,
        // last_occurrence, stack, file_link_counts) drop automatically on return.
        (objects, all_children, indeterminate_objects)
    }

    pub fn finish(mut self) -> Result<DirectoryGraph, GraphBuildError> {
        let terminal = self
            .terminal
            .take()
            .ok_or(GraphBuildError::MissingTerminal)?;

        if self.entries.is_empty() {
            if terminal.outcome == pigtree_protocol::RunOutcome::Finished {
                return Err(GraphBuildError::InvalidRoot {
                    entry_id: 0,
                    parent_id: 0,
                });
            }
            return Ok(DirectoryGraph {
                root_target: self.root_target,
                root_id: 0,
                storage: EntryStorage::Dense(Vec::new()),
                all_children: Vec::new(),
                objects: Vec::new(),
                object_streams: self.object_streams,
                gaps: self.gaps,
                terminal,
                allocated_bytes_known: self.allocated_bytes_known,
                indeterminate_external_reference_objects: 0,
            });
        }

        let (objects, all_children, indeterminate_objects) = self.compute_aggregates();

        let storage = if self.is_dense {
            EntryStorage::Dense(self.entries)
        } else {
            EntryStorage::Sparse {
                entries: self.entries,
                id_to_idx: self.id_to_idx,
            }
        };

        Ok(DirectoryGraph {
            root_target: self.root_target,
            root_id: 1,
            storage,
            all_children,
            objects,
            object_streams: self.object_streams,
            gaps: self.gaps,
            terminal,
            allocated_bytes_known: self.allocated_bytes_known,
            indeterminate_external_reference_objects: indeterminate_objects,
        })
    }

    pub fn build_from_reader<R: Read>(
        reader: ObservationReader<R>,
    ) -> Result<DirectoryGraph, GraphBuildError> {
        Self::build_from_reader_with_progress(reader, "", None::<fn(ScanProgress)>)
    }

    pub fn build_from_reader_with_progress<R: Read, F: FnMut(ScanProgress)>(
        mut reader: ObservationReader<R>,
        operation_id: &str,
        mut on_progress: Option<F>,
    ) -> Result<DirectoryGraph, GraphBuildError> {
        let mut builder = Self::new(reader.target_path());
        let mut seq: u64 = 0;
        let mut last_emit = std::time::Instant::now();
        let mut is_first = true;

        while let Some(record) = reader.read_record()? {
            let is_gap = matches!(record, ObservationRecord::CoverageGap(_));
            builder.ingest_record(record)?;

            if let Some(cb) = on_progress.as_mut() {
                let now = std::time::Instant::now();
                if is_first
                    || is_gap
                    || now.duration_since(last_emit) >= std::time::Duration::from_millis(50)
                {
                    seq += 1;
                    let progress = ScanProgress {
                        operation_id: operation_id.to_string(),
                        sequence_number: seq,
                        timestamp_iso: format_utc_iso(std::time::SystemTime::now()),
                        observed_directories: builder.dir_count,
                        observed_files: builder.file_count,
                        observed_logical_bytes: builder.logical_bytes,
                        observed_referenced_allocated_bytes: builder.allocated_bytes,
                        coverage_gaps: builder.gaps.len() as u32,
                        current_phase: "traversing".to_string(),
                        current_directory: builder.current_directory_path(),
                    };
                    cb(progress);
                    last_emit = now;
                    is_first = false;
                }
            }
        }
        builder.finish()
    }
}

pub fn build_graph_from_reader<R: Read>(
    reader: ObservationReader<R>,
) -> Result<DirectoryGraph, GraphBuildError> {
    GraphBuilder::build_from_reader(reader)
}
