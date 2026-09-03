//! PigTree core engine library.

pub mod builder;
pub mod error;
pub mod graph;
pub mod runner;

pub use builder::{build_graph_from_reader, format_utc_iso, GraphBuilder};
pub use error::GraphBuildError;
pub use graph::{
    CompactEntry, DirectoryGraph, EntryKind, EntryStorage, GraphEntry, GraphQueryError,
    ObjectRecord, StreamBreakdown, NO_OBJECT,
};
pub use pigtree_ipc::CancelHandle;
pub use runner::{
    launch_scan_worker, launch_scan_worker_with_progress, resolve_scan_worker_exe, ScanRunnerError,
};
