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

/// An entry in the directory graph.
///
/// # Size Invariants
/// - For `EntryKind::File`: `logical_size` represents the file's self logical size (`Some(u64)`),
///   and `allocated_size` represents the file's self physical storage (`Some(u64)` or `None` if unavailable).
///   `allocated_size_known` is `true` if `allocated_size.is_some()`.
/// - For `EntryKind::Directory`: during stream ingestion prior to settlement, `logical_size` and
///   `allocated_size` are `None`. Upon graph settlement (`GraphBuilder::finish`),
///   recursive scope aggregates are computed in O(N+E) bottom-up order: `logical_size` becomes
///   `Some(sum of child logical aggregates)`, `allocated_size` becomes `Some(known subtotal of descendant allocation)`,
///   and `allocated_size_known` is `true` if and only if all observed descendants have known allocated size.
///   Empty directories receive `Some(0)` logical size, `Some(0)` allocated size, and `allocated_size_known: true`.
/// - For `EntryKind::Special`: `logical_size` and `allocated_size` remain `None`, and `allocated_size_known` is `false`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphEntry {
    pub id: u32,
    pub parent_id: u32,
    pub name: String,
    pub kind: EntryKind,
    pub logical_size: Option<u64>,
    pub allocated_size: Option<u64>,
    pub allocated_size_known: bool,
    pub file_attributes: u32,
    pub reparse_tag: u32,
    pub creation_time_utc_ms: u64,
    pub last_write_time_utc_ms: u64,
    pub last_access_time_utc_ms: u64,
    pub children: Vec<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GraphQueryError {
    ParentNotFound(u32),
    ParentNotDirectory(u32),
}

impl std::fmt::Display for GraphQueryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GraphQueryError::ParentNotFound(id) => write!(f, "Parent entry ID {} not found", id),
            GraphQueryError::ParentNotDirectory(id) => {
                write!(f, "Entry ID {} is not a directory", id)
            }
        }
    }
}

impl std::error::Error for GraphQueryError {}

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

    /// Queries a paginated list of immediate child entries for the given `parent_id`.
    ///
    /// Deterministic ordering rules:
    /// 1. Directories first.
    /// 2. Logical size descending.
    /// 3. Name case-insensitive ascending.
    /// 4. ID ascending.
    pub fn get_children_page(
        &self,
        parent_id: u32,
        offset: usize,
        limit: usize,
    ) -> Result<(usize, Vec<pigtree_protocol::protobuf::DirectoryEntryNode>), GraphQueryError> {
        let mut child_ids = if parent_id == 0 {
            if self.root_id != 0 && self.entries.contains_key(&self.root_id) {
                vec![self.root_id]
            } else {
                Vec::new()
            }
        } else {
            let parent = self
                .entries
                .get(&parent_id)
                .ok_or(GraphQueryError::ParentNotFound(parent_id))?;
            if parent.kind != EntryKind::Directory {
                return Err(GraphQueryError::ParentNotDirectory(parent_id));
            }
            parent.children.clone()
        };

        // Deterministic ordering:
        child_ids.sort_by(|&a_id, &b_id| {
            let a = self.entries.get(&a_id);
            let b = self.entries.get(&b_id);

            let a_is_dir = a.is_some_and(|e| e.kind == EntryKind::Directory);
            let b_is_dir = b.is_some_and(|e| e.kind == EntryKind::Directory);
            if a_is_dir != b_is_dir {
                return b_is_dir.cmp(&a_is_dir); // Directories first
            }

            let a_logical = a.and_then(|e| e.logical_size).unwrap_or(0);
            let b_logical = b.and_then(|e| e.logical_size).unwrap_or(0);
            if a_logical != b_logical {
                return b_logical.cmp(&a_logical); // Logical size descending
            }

            let a_name = a.map_or("", |e| &e.name);
            let b_name = b.map_or("", |e| &e.name);
            let a_lower = a_name.to_lowercase();
            let b_lower = b_name.to_lowercase();
            if a_lower != b_lower {
                return a_lower.cmp(&b_lower); // Name case-insensitive ascending
            }

            if a_name != b_name {
                return a_name.cmp(b_name);
            }

            a_id.cmp(&b_id) // ID ascending
        });

        let total_children = child_ids.len();
        let nodes = if offset >= total_children {
            Vec::new()
        } else {
            let end = (offset + limit).min(total_children);
            child_ids[offset..end]
                .iter()
                .filter_map(|&id| {
                    self.entries.get(&id).map(|entry| {
                        pigtree_protocol::protobuf::DirectoryEntryNode {
                            id: entry.id,
                            parent_id: entry.parent_id,
                            name: entry.name.clone(),
                            entry_kind: match entry.kind {
                                EntryKind::Directory => 1,
                                EntryKind::File => 2,
                                EntryKind::Special => 3,
                            },
                            logical_size: entry.logical_size.unwrap_or(0),
                            allocated_size: entry.allocated_size.unwrap_or(0),
                            allocated_size_known: entry.allocated_size_known,
                            child_count: entry.children.len() as u32,
                            has_children: !entry.children.is_empty(),
                        }
                    })
                })
                .collect()
        };

        Ok((total_children, nodes))
    }
}
