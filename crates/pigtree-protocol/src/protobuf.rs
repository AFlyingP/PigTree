//! Protobuf v3 message definitions and prost-generated structures.

include!(concat!(env!("OUT_DIR"), "/pigtree.session.v1.rs"));

pub use prost::DecodeError as ProtoError;
pub use prost::Message;

use crate::observation::{ExternalReferenceStatus, ValueKnowledge};

impl From<ValueKnowledge<u32>> for LinkCountKnowledgeProto {
    fn from(v: ValueKnowledge<u32>) -> Self {
        match v {
            ValueKnowledge::NotObserved => LinkCountKnowledgeProto {
                status: LinkCountKnowledgeStatus::NotObserved as i32,
                count: 0,
            },
            ValueKnowledge::Known(c) => LinkCountKnowledgeProto {
                status: LinkCountKnowledgeStatus::Known as i32,
                count: c,
            },
            ValueKnowledge::Unavailable => LinkCountKnowledgeProto {
                status: LinkCountKnowledgeStatus::Unavailable as i32,
                count: 0,
            },
            ValueKnowledge::NotApplicable => LinkCountKnowledgeProto {
                status: LinkCountKnowledgeStatus::NotApplicable as i32,
                count: 0,
            },
        }
    }
}

impl From<ExternalReferenceStatus> for ExternalReferenceStatusProto {
    fn from(s: ExternalReferenceStatus) -> Self {
        match s {
            ExternalReferenceStatus::ConfirmedNone => {
                ExternalReferenceStatusProto::ExternalReferenceStatusConfirmedNone
            }
            ExternalReferenceStatus::ConfirmedExternal => {
                ExternalReferenceStatusProto::ExternalReferenceStatusConfirmedExternal
            }
            ExternalReferenceStatus::Indeterminate => {
                ExternalReferenceStatusProto::ExternalReferenceStatusIndeterminate
            }
            ExternalReferenceStatus::InconsistentEvidence => {
                ExternalReferenceStatusProto::ExternalReferenceStatusInconsistentEvidence
            }
            ExternalReferenceStatus::NotApplicable => {
                ExternalReferenceStatusProto::ExternalReferenceStatusNotApplicable
            }
        }
    }
}

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
