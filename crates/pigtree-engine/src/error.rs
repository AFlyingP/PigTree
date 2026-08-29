//! Error types for directory graph ingestion and invariant validation.

use pigtree_protocol::ObservationDecodeError;
use std::fmt;

#[derive(Debug)]
pub enum GraphBuildError {
    InvalidRoot {
        entry_id: u32,
        parent_id: u32,
    },
    DuplicateEntryId(u32),
    MissingParent {
        entry_id: u32,
        parent_id: u32,
    },
    ParentNotDirectory {
        entry_id: u32,
        parent_id: u32,
    },
    SelfParent(u32),
    RecordAfterTerminal,
    DuplicateTerminal,
    MissingTerminal,
    AggregateMismatch {
        field: &'static str,
        expected: u64,
        actual: u64,
    },
    Decode(ObservationDecodeError),
}

impl fmt::Display for GraphBuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GraphBuildError::InvalidRoot {
                entry_id,
                parent_id,
            } => {
                write!(
                    f,
                    "invalid root entry: expected id=1 and parent_id=0, got id={entry_id} and parent_id={parent_id}"
                )
            }
            GraphBuildError::DuplicateEntryId(id) => {
                write!(f, "duplicate entry ID: {id}")
            }
            GraphBuildError::MissingParent {
                entry_id,
                parent_id,
            } => {
                write!(
                    f,
                    "missing or out-of-order parent: entry {entry_id} references non-existent parent {parent_id}"
                )
            }
            GraphBuildError::ParentNotDirectory {
                entry_id,
                parent_id,
            } => {
                write!(
                    f,
                    "parent of entry {entry_id} (id={parent_id}) is not a directory"
                )
            }
            GraphBuildError::SelfParent(id) => {
                write!(f, "entry {id} has self-referencing parent_id")
            }
            GraphBuildError::RecordAfterTerminal => {
                write!(f, "received observation record after terminal record")
            }
            GraphBuildError::DuplicateTerminal => {
                write!(f, "received duplicate terminal record")
            }
            GraphBuildError::MissingTerminal => {
                write!(f, "stream finished without a terminal record")
            }
            GraphBuildError::AggregateMismatch {
                field,
                expected,
                actual,
            } => {
                write!(
                    f,
                    "terminal aggregate mismatch on {field}: expected {expected}, actual {actual}"
                )
            }
            GraphBuildError::Decode(e) => {
                write!(f, "observation stream decode error: {e}")
            }
        }
    }
}

impl std::error::Error for GraphBuildError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            GraphBuildError::Decode(e) => Some(e),
            _ => None,
        }
    }
}

impl From<ObservationDecodeError> for GraphBuildError {
    fn from(e: ObservationDecodeError) -> Self {
        GraphBuildError::Decode(e)
    }
}
