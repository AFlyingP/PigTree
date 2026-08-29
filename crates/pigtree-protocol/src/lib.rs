//! Protocol definitions, framing, and protobuf codecs for PigTree.

pub mod crc32c;
pub mod frame;
pub mod json;
pub mod protobuf;
pub mod sha256;

pub use crc32c::compute_crc32c;
pub use frame::{
    decode_frame, encode_frame, read_frame, write_frame, ChannelTag, Frame, FrameFlags,
    FrameHeader, FrameParseError, CRC_SIZE, HEADER_SIZE, MAGIC, MAX_PAYLOAD_SIZE, SCHEMA_VERSION,
};
pub use json::{
    escape_json_string, format_cancelled_envelope, format_diagnostic, format_echo_response,
    format_error_envelope, format_health_response, format_ping_response, format_status_response,
    format_success_envelope, format_version_response,
};
pub use prost::Message;
pub use protobuf::*;
pub use sha256::sha256;
