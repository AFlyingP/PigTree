//! Directory graph and entry data structures.

use pigtree_protocol::{CoverageGapObservation, TerminalObservation};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryKind {
    Directory,
    File,
    Special,
}

impl EntryKind {
    pub fn is_directory(&self) -> bool {
        matches!(self, EntryKind::Directory)
    }

    pub fn is_file(&self) -> bool {
        matches!(self, EntryKind::File)
    }

    pub fn is_special(&self) -> bool {
        matches!(self, EntryKind::Special)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphEntry {
    pub id: u32,
    pub parent_id: u32,
    pub name: String,
    pub kind: EntryKind,
    pub logical_size: Option<u64>,
    pub allocated_size: Option<u64>,
    pub file_attributes: u32,
    pub reparse_tag: u32,
    pub creation_time_utc_ms: u64,
    pub last_write_time_utc_ms: u64,
    pub last_access_time_utc_ms: u64,
    pub children: Vec<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectoryGraph {
    pub(crate) root_target: String,
    pub(crate) root_id: u32,
    pub(crate) entries: HashMap<u32, GraphEntry>,
    pub(crate) gaps: Vec<CoverageGapObservation>,
    pub(crate) terminal: TerminalObservation,
    pub(crate) allocated_bytes_known: bool,
}

impl DirectoryGraph {
    pub fn root_target(&self) -> &str {
        &self.root_target
    }

    pub fn root_id(&self) -> u32 {
        self.root_id
    }

    pub fn root(&self) -> &GraphEntry {
        &self.entries[&self.root_id]
    }

    pub fn try_root(&self) -> Option<&GraphEntry> {
        self.entries.get(&self.root_id)
    }

    pub fn entry(&self, id: u32) -> Option<&GraphEntry> {
        self.entries.get(&id)
    }

    pub fn entries(&self) -> &HashMap<u32, GraphEntry> {
        &self.entries
    }

    pub fn gaps(&self) -> &[CoverageGapObservation] {
        &self.gaps
    }

    pub fn terminal(&self) -> &TerminalObservation {
        &self.terminal
    }

    pub fn total_entries(&self) -> usize {
        self.entries.len()
    }

    pub fn allocated_bytes_known(&self) -> bool {
        self.allocated_bytes_known
    }
}
