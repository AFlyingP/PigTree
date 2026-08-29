//! Protobuf v3 message definitions and prost-generated structures.

include!(concat!(env!("OUT_DIR"), "/pigtree.session.v1.rs"));

pub use prost::DecodeError as ProtoError;
pub use prost::Message;

/// Encode a prost message to a byte vector.
pub fn encode_message<M: Message>(msg: &M) -> Vec<u8> {
    let mut buf = Vec::with_capacity(msg.encoded_len());
    msg.encode(&mut buf)
        .expect("encoding to Vec should not fail");
    buf
}

/// Decode a prost message from a byte slice.
pub fn decode_message<M: Message + Default>(buf: &[u8]) -> Result<M, prost::DecodeError> {
    M::decode(buf)
}
