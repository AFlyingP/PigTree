//! Directory graph and entry data structures.

use pigtree_protocol::protobuf::ExternalReferenceStatusProto;
use pigtree_protocol::{
    CoverageGapObservation, ExternalReferenceStatus, ObjectIdentity, TerminalObservation,
    ValueKnowledge,
};
use std::collections::HashMap;

pub const NO_OBJECT: u32 = u32::MAX;

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

/// Normalized identity, link evidence, and canonical weight for a distinct filesystem object.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectRecord {
    pub identity: Option<ObjectIdentity>,
    pub observed_alias_count: u32,
    pub total_link_count: ValueKnowledge<u32>,
    pub external_reference_status: ExternalReferenceStatus,
    pub weight: u64,
}

/// Compact memory-efficient representation of an entry in the directory graph.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompactEntry {
    pub id: u32,
    pub parent_id: u32,
    pub name: String,
    pub kind: EntryKind,
    pub allocated_size_known: bool,
    pub logical_size: Option<u64>,
    pub allocated_size: Option<u64>,
    pub referenced_allocated_bytes: u64,
    pub unique_allocated_bytes: u64,
    pub known_subtotal_allocated_bytes: u64,
    pub object_index: u32,
    pub file_attributes: u32,
    pub reparse_tag: u32,
    pub child_start: u32,
    pub child_count: u32,
    pub creation_time_utc_ms: u64,
    pub last_write_time_utc_ms: u64,
    pub last_access_time_utc_ms: u64,
}

/// Storage backend for graph entries: dense vector for sequential IDs with sparse fallback.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EntryStorage {
    Dense(Vec<CompactEntry>),
    Sparse {
        entries: Vec<CompactEntry>,
        id_to_idx: HashMap<u32, u32>,
    },
}

impl EntryStorage {
    pub fn len(&self) -> usize {
        match self {
            EntryStorage::Dense(v) => v.len(),
            EntryStorage::Sparse { entries, .. } => entries.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn get(&self, id: u32) -> Option<&CompactEntry> {
        if id == 0 {
            return None;
        }
        match self {
            EntryStorage::Dense(v) => {
                let idx = (id.checked_sub(1)?) as usize;
                v.get(idx)
            }
            EntryStorage::Sparse { entries, id_to_idx } => {
                let &idx = id_to_idx.get(&id)?;
                entries.get(idx as usize)
            }
        }
    }

    pub fn get_by_index(&self, idx: usize) -> Option<&CompactEntry> {
        match self {
            EntryStorage::Dense(v) => v.get(idx),
            EntryStorage::Sparse { entries, .. } => entries.get(idx),
        }
    }
}

/// A projected public entry in the directory graph.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphEntry {
    pub id: u32,
    pub parent_id: u32,
    pub name: String,
    pub kind: EntryKind,
    pub logical_size: Option<u64>,
    pub allocated_size: Option<u64>,
    pub allocated_size_known: bool,
    pub referenced_allocated_bytes: u64,
    pub unique_allocated_bytes: u64,
    pub known_subtotal_allocated_bytes: u64,
    pub object_id: Option<ObjectIdentity>,
    pub observed_alias_count: u32,
    pub total_link_count: ValueKnowledge<u32>,
    pub external_reference_status: ExternalReferenceStatus,
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
    pub(crate) storage: EntryStorage,
    pub(crate) all_children: Vec<u32>,
    pub(crate) objects: Vec<ObjectRecord>,
    pub(crate) gaps: Vec<CoverageGapObservation>,
    pub(crate) terminal: TerminalObservation,
    pub(crate) allocated_bytes_known: bool,
    pub(crate) indeterminate_external_reference_objects: u64,
}

impl DirectoryGraph {
    pub fn root_target(&self) -> &str {
        &self.root_target
    }

    pub fn root_id(&self) -> u32 {
        self.root_id
    }

    pub fn root(&self) -> GraphEntry {
        self.entry(self.root_id).expect("root entry must exist")
    }

    pub fn try_root(&self) -> Option<GraphEntry> {
        self.entry(self.root_id)
    }

    pub fn entry(&self, id: u32) -> Option<GraphEntry> {
        let compact = self.storage.get(id)?;
        Some(self.project_graph_entry(compact))
    }

    pub fn gaps(&self) -> &[CoverageGapObservation] {
        &self.gaps
    }

    pub fn terminal(&self) -> &TerminalObservation {
        &self.terminal
    }

    pub fn total_entries(&self) -> usize {
        self.storage.len()
    }

    pub fn allocated_bytes_known(&self) -> bool {
        self.allocated_bytes_known
    }

    pub fn referenced_allocated_bytes(&self) -> u64 {
        self.storage
            .get(self.root_id)
            .map(|r| r.referenced_allocated_bytes)
            .unwrap_or(0)
    }

    pub fn unique_allocated_bytes(&self) -> u64 {
        self.storage
            .get(self.root_id)
            .map(|r| r.unique_allocated_bytes)
            .unwrap_or(0)
    }

    pub fn known_subtotal_allocated_bytes(&self) -> u64 {
        self.storage
            .get(self.root_id)
            .map(|r| r.known_subtotal_allocated_bytes)
            .unwrap_or(0)
    }

    pub fn logical_bytes(&self) -> u64 {
        self.storage
            .get(self.root_id)
            .and_then(|r| r.logical_size)
            .unwrap_or(0)
    }

    /// Number of distinct Filesystem Objects in the snapshot whose hard-link
    /// external reference status is `Indeterminate`. Identity-less entries each
    /// count as one unresolved object. Surfaced at Scan Target summary level
    /// per issue #20 AC-7, never as per-row badges on un-enriched scans.
    pub fn indeterminate_external_reference_objects(&self) -> u64 {
        self.indeterminate_external_reference_objects
    }

    pub fn storage(&self) -> &EntryStorage {
        &self.storage
    }

    pub fn objects(&self) -> &[ObjectRecord] {
        &self.objects
    }

    pub(crate) fn children_of(&self, compact: &CompactEntry) -> Vec<u32> {
        if compact.child_count == 0 {
            Vec::new()
        } else {
            let start = compact.child_start as usize;
            let count = compact.child_count as usize;
            self.all_children[start..start + count].to_vec()
        }
    }

    pub(crate) fn project_graph_entry(&self, compact: &CompactEntry) -> GraphEntry {
        let children = self.children_of(compact);
        let (object_id, observed_alias_count, total_link_count, external_reference_status) =
            if compact.object_index != NO_OBJECT {
                let obj = &self.objects[compact.object_index as usize];
                (
                    obj.identity,
                    obj.observed_alias_count,
                    obj.total_link_count,
                    obj.external_reference_status,
                )
            } else {
                match compact.kind {
                    EntryKind::Directory | EntryKind::Special => (
                        None,
                        1,
                        ValueKnowledge::NotApplicable,
                        ExternalReferenceStatus::NotApplicable,
                    ),
                    EntryKind::File => (
                        None,
                        1,
                        ValueKnowledge::NotObserved,
                        ExternalReferenceStatus::derive(ValueKnowledge::NotObserved, 1),
                    ),
                }
            };

        GraphEntry {
            id: compact.id,
            parent_id: compact.parent_id,
            name: compact.name.clone(),
            kind: compact.kind,
            logical_size: compact.logical_size,
            allocated_size: compact.allocated_size,
            allocated_size_known: compact.allocated_size_known,
            referenced_allocated_bytes: compact.referenced_allocated_bytes,
            unique_allocated_bytes: compact.unique_allocated_bytes,
            known_subtotal_allocated_bytes: compact.known_subtotal_allocated_bytes,
            object_id,
            observed_alias_count,
            total_link_count,
            external_reference_status,
            file_attributes: compact.file_attributes,
            reparse_tag: compact.reparse_tag,
            creation_time_utc_ms: compact.creation_time_utc_ms,
            last_write_time_utc_ms: compact.last_write_time_utc_ms,
            last_access_time_utc_ms: compact.last_access_time_utc_ms,
            children,
        }
    }

    /// Queries a paginated list of immediate child entries for the given `parent_id`.
    /// Children are already sorted in deterministic order during finalization.
    pub fn get_children_page(
        &self,
        parent_id: u32,
        offset: usize,
        limit: usize,
    ) -> Result<(usize, Vec<pigtree_protocol::protobuf::DirectoryEntryNode>), GraphQueryError> {
        let child_ids: &[u32] = if parent_id == 0 {
            if self.root_id != 0 && self.storage.get(self.root_id).is_some() {
                std::slice::from_ref(&self.root_id)
            } else {
                &[]
            }
        } else {
            let parent = self
                .storage
                .get(parent_id)
                .ok_or(GraphQueryError::ParentNotFound(parent_id))?;
            if parent.kind != EntryKind::Directory {
                return Err(GraphQueryError::ParentNotDirectory(parent_id));
            }
            let start = parent.child_start as usize;
            let count = parent.child_count as usize;
            &self.all_children[start..start + count]
        };

        let total_children = child_ids.len();
        let nodes = if offset >= total_children {
            Vec::new()
        } else {
            let end = (offset + limit).min(total_children);
            child_ids[offset..end]
                .iter()
                .filter_map(|&id| {
                    self.storage.get(id).map(|compact| {
                        let (observed_alias_count, total_link_count, external_status) =
                            if compact.object_index != NO_OBJECT {
                                let obj = &self.objects[compact.object_index as usize];
                                (
                                    obj.observed_alias_count,
                                    Some(obj.total_link_count.into()),
                                    ExternalReferenceStatusProto::from(
                                        obj.external_reference_status,
                                    ) as i32,
                                )
                            } else {
                                match compact.kind {
                                    EntryKind::Directory | EntryKind::Special => (
                                        1,
                                        Some(ValueKnowledge::<u32>::NotApplicable.into()),
                                        ExternalReferenceStatusProto::from(
                                            ExternalReferenceStatus::NotApplicable,
                                        ) as i32,
                                    ),
                                    EntryKind::File => (
                                        1,
                                        Some(ValueKnowledge::<u32>::NotObserved.into()),
                                        ExternalReferenceStatusProto::from(
                                            ExternalReferenceStatus::Indeterminate,
                                        ) as i32,
                                    ),
                                }
                            };

                        pigtree_protocol::protobuf::DirectoryEntryNode {
                            id: compact.id,
                            parent_id: compact.parent_id,
                            name: compact.name.clone(),
                            entry_kind: match compact.kind {
                                EntryKind::Directory => 1,
                                EntryKind::File => 2,
                                EntryKind::Special => 3,
                            },
                            logical_bytes: compact.logical_size.unwrap_or(0),
                            referenced_allocated_bytes: compact.referenced_allocated_bytes,
                            allocated_size_known: compact.allocated_size_known,
                            child_count: compact.child_count,
                            has_children: compact.child_count > 0,
                            unique_allocated_bytes: compact.unique_allocated_bytes,
                            observed_alias_count,
                            total_link_count,
                            external_reference_status: external_status,
                            known_subtotal_allocated_bytes: compact.known_subtotal_allocated_bytes,
                        }
                    })
                })
                .collect()
        };

        Ok((total_children, nodes))
    }
}
