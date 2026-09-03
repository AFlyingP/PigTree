//! Protocol definitions, framing, and protobuf codecs for PigTree.

pub mod crc32c;
pub mod frame;
pub mod json;
pub mod observation;
pub mod protobuf;
pub mod sha256;

pub use crc32c::compute_crc32c;
pub use frame::{
    decode_frame, encode_frame, read_frame, write_frame, ChannelTag, Frame, FrameFlags,
    FrameHeader, FrameParseError, CRC_SIZE, HEADER_SIZE, MAGIC, MAX_PAYLOAD_SIZE, SCHEMA_VERSION,
};
pub use json::{
    entry_kind_to_str, escape_json_string, external_reference_status_to_str,
    format_cancelled_envelope, format_diagnostic, format_directory_entry_json,
    format_directory_entry_ndjson, format_directory_entry_ndjson_event, format_echo_response,
    format_error_envelope, format_health_response, format_ping_response, format_status_response,
    format_success_envelope, format_version_response, link_count_knowledge_to_str,
};
pub use observation::{
    CoverageGapObservation, DirectoryObservation, ExternalReferenceStatus, FileObservation,
    ObjectIdentity, ObservationDecodeError, ObservationReader, ObservationRecord,
    ObservationWriter, RecordTag, RunOutcome, SpecialObservation, TerminalObservation,
    TotalLinkCount, ValueKnowledge, WORKER_MAGIC, WORKER_STREAM_VERSION, WORKER_STREAM_VERSION_V1,
    WORKER_STREAM_VERSION_V2,
};
pub use prost::Message;
pub use protobuf::*;
pub use sha256::sha256;
