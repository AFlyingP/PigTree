//! Framed message transport for exchanging protobuf messages over duplex streams.

use crate::error::IpcError;
use crate::pipe::PipeStream;
use pigtree_protocol::frame::{
    read_frame, write_frame, ChannelTag, Frame, FrameFlags, FrameHeader,
};
use pigtree_protocol::Message;
use std::io::{Read, Write};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameReadiness {
    Empty,
    Partial,
    Complete,
}

pub struct FramedSession<S> {
    stream: S,
    sequence_number: u64,
}

impl<S: Read + Write> FramedSession<S> {
    pub fn new(stream: S) -> Self {
        Self {
            stream,
            sequence_number: 0,
        }
    }

    pub fn into_inner(self) -> S {
        self.stream
    }

    pub fn stream_mut(&mut self) -> &mut S {
        &mut self.stream
    }

    pub fn next_seq(&mut self) -> u64 {
        self.sequence_number += 1;
        self.sequence_number
    }

    pub fn send_message<M: Message>(
        &mut self,
        channel: ChannelTag,
        flags: FrameFlags,
        msg: &M,
    ) -> Result<u64, IpcError> {
        let seq = self.next_seq();
        let header = FrameHeader {
            channel_tag: channel,
            flags,
            sequence_number: seq,
        };
        let payload = msg.encode_to_vec();
        write_frame(&mut self.stream, &header, &payload)?;
        Ok(seq)
    }

    pub fn recv_message<M: Message + Default>(
        &mut self,
    ) -> Result<Option<(FrameHeader, M)>, IpcError> {
        match read_frame(&mut self.stream)? {
            Some(frame) => {
                let msg = M::decode(&frame.payload[..])?;
                Ok(Some((frame.header, msg)))
            }
            None => Ok(None),
        }
    }

    pub fn send_frame(&mut self, header: &FrameHeader, payload: &[u8]) -> Result<(), IpcError> {
        write_frame(&mut self.stream, header, payload)?;
        Ok(())
    }

    pub fn recv_frame(&mut self) -> Result<Option<Frame>, IpcError> {
        let frame = read_frame(&mut self.stream)?;
        Ok(frame)
    }
}

impl FramedSession<PipeStream> {
    pub fn has_incoming_data(&self) -> Result<bool, IpcError> {
        self.stream.has_incoming_data()
    }

    pub fn peek_frame_readiness(&self) -> Result<FrameReadiness, IpcError> {
        self.stream.peek_frame_readiness()
    }
}
