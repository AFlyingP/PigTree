//! Builder for constructing a DirectoryGraph from an observation stream.

use crate::error::GraphBuildError;
use crate::graph::{DirectoryGraph, EntryKind, GraphEntry};
use pigtree_protocol::protobuf::ScanProgress;
use pigtree_protocol::{
    CoverageGapObservation, DirectoryObservation, FileObservation, ObservationReader,
    ObservationRecord, SpecialObservation, TerminalObservation,
};
use std::collections::HashMap;
use std::io::Read;

pub use pigtree_protocol::json::format_utc_iso;

#[derive(Debug)]
pub struct GraphBuilder {
    root_target: String,
    entries: HashMap<u32, GraphEntry>,
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
        Self {
            root_target: root_target.into(),
            entries: HashMap::new(),
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
        entry: GraphEntry,
    ) -> Result<(), GraphBuildError> {
        if id == parent_id {
            return Err(GraphBuildError::SelfParent(id));
        }

        if self.entries.is_empty() {
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

            if self.entries.contains_key(&id) {
                return Err(GraphBuildError::DuplicateEntryId(id));
            }

            let parent =
                self.entries
                    .get_mut(&parent_id)
                    .ok_or(GraphBuildError::MissingParent {
                        entry_id: id,
                        parent_id,
                    })?;

            if parent.kind != EntryKind::Directory {
                return Err(GraphBuildError::ParentNotDirectory {
                    entry_id: id,
                    parent_id,
                });
            }

            parent.children.push(id);
        }

        self.entries.insert(id, entry);
        Ok(())
    }

    fn ingest_directory(&mut self, dir: DirectoryObservation) -> Result<(), GraphBuildError> {
        let entry = GraphEntry {
            id: dir.entry_id,
            parent_id: dir.parent_id,
            name: dir.name,
            kind: EntryKind::Directory,
            logical_size: None,
            allocated_size: None,
            allocated_size_known: true,
            file_attributes: dir.file_attributes,
            reparse_tag: dir.reparse_tag,
            creation_time_utc_ms: dir.creation_time_utc_ms,
            last_write_time_utc_ms: dir.last_write_time_utc_ms,
            last_access_time_utc_ms: dir.last_access_time_utc_ms,
            children: Vec::new(),
        };

        self.validate_and_insert_entry(dir.entry_id, dir.parent_id, entry)?;
        self.dir_count += 1;
        Ok(())
    }

    fn ingest_file(&mut self, file: FileObservation) -> Result<(), GraphBuildError> {
        let entry = GraphEntry {
            id: file.entry_id,
            parent_id: file.parent_id,
            name: file.name,
            kind: EntryKind::File,
            logical_size: Some(file.logical_size),
            allocated_size: file.allocated_size,
            allocated_size_known: file.allocated_size.is_some(),
            file_attributes: file.file_attributes,
            reparse_tag: file.reparse_tag,
            creation_time_utc_ms: file.creation_time_utc_ms,
            last_write_time_utc_ms: file.last_write_time_utc_ms,
            last_access_time_utc_ms: file.last_access_time_utc_ms,
            children: Vec::new(),
        };

        self.validate_and_insert_entry(file.entry_id, file.parent_id, entry)?;
        self.file_count += 1;
        self.logical_bytes = self.logical_bytes.saturating_add(file.logical_size);
        if let Some(alloc) = file.allocated_size {
            self.allocated_bytes = self.allocated_bytes.saturating_add(alloc);
        } else {
            self.allocated_bytes_known = false;
        }
        Ok(())
    }

    fn ingest_special(&mut self, special: SpecialObservation) -> Result<(), GraphBuildError> {
        let entry = GraphEntry {
            id: special.entry_id,
            parent_id: special.parent_id,
            name: special.name,
            kind: EntryKind::Special,
            logical_size: None,
            allocated_size: None,
            allocated_size_known: false,
            file_attributes: special.file_attributes,
            reparse_tag: special.reparse_tag,
            creation_time_utc_ms: special.creation_time_utc_ms,
            last_write_time_utc_ms: special.last_write_time_utc_ms,
            last_access_time_utc_ms: special.last_access_time_utc_ms,
            children: Vec::new(),
        };

        self.validate_and_insert_entry(special.entry_id, special.parent_id, entry)?;
        self.special_count += 1;
        Ok(())
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
            if let Some(entry) = self.entries.get(&curr) {
                components.push(entry.name.as_str());
                curr = entry.parent_id;
                depth += 1;
            } else {
                break;
            }
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

    fn compute_directory_aggregates(&mut self) {
        if self.entries.is_empty() {
            return;
        }

        // Collect topological traversal order using iterative DFS starting from root (id=1).
        let mut visited = std::collections::HashSet::with_capacity(self.entries.len());
        let mut stack = Vec::with_capacity(self.entries.len());
        let mut order = Vec::with_capacity(self.entries.len());

        if self.entries.contains_key(&1) {
            stack.push(1);
            visited.insert(1);
        }

        while let Some(id) = stack.pop() {
            order.push(id);
            if let Some(entry) = self.entries.get(&id) {
                for &child_id in &entry.children {
                    if visited.insert(child_id) {
                        stack.push(child_id);
                    }
                }
            }
        }

        // Handle any disconnected entries if present in partial/unusual graphs
        if visited.len() < self.entries.len() {
            for &id in self.entries.keys() {
                if visited.insert(id) {
                    stack.push(id);
                    while let Some(curr_id) = stack.pop() {
                        order.push(curr_id);
                        if let Some(entry) = self.entries.get(&curr_id) {
                            for &child_id in &entry.children {
                                if visited.insert(child_id) {
                                    stack.push(child_id);
                                }
                            }
                        }
                    }
                }
            }
        }

        // Reverse the traversal order so all children precede their parent (bottom-up post-order).
        order.reverse();

        for id in order {
            let entry = match self.entries.get(&id) {
                Some(e) => e,
                None => continue,
            };

            if entry.kind != EntryKind::Directory {
                continue;
            }

            let mut sum_logical: u64 = 0;
            let mut sum_alloc: u64 = 0;
            let mut all_known = true;

            for &child_id in &entry.children {
                if let Some(child) = self.entries.get(&child_id) {
                    sum_logical = sum_logical.saturating_add(child.logical_size.unwrap_or(0));
                    sum_alloc = sum_alloc.saturating_add(child.allocated_size.unwrap_or(0));
                    match child.kind {
                        EntryKind::File => {
                            if child.allocated_size.is_none() {
                                all_known = false;
                            }
                        }
                        EntryKind::Directory => {
                            if !child.allocated_size_known {
                                all_known = false;
                            }
                        }
                        EntryKind::Special => {}
                    }
                }
            }

            if let Some(dir_entry) = self.entries.get_mut(&id) {
                dir_entry.logical_size = Some(sum_logical);
                dir_entry.allocated_size = Some(sum_alloc);
                dir_entry.allocated_size_known = all_known;
            }
        }
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
                entries: self.entries,
                gaps: self.gaps,
                terminal,
                allocated_bytes_known: self.allocated_bytes_known,
            });
        }

        self.compute_directory_aggregates();

        Ok(DirectoryGraph {
            root_target: self.root_target,
            root_id: 1,
            entries: self.entries,
            gaps: self.gaps,
            terminal,
            allocated_bytes_known: self.allocated_bytes_known,
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
                        observed_allocated_bytes: builder.allocated_bytes,
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
