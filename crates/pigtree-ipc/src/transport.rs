//! Framed message transport for exchanging protobuf messages over duplex streams.

use crate::error::IpcError;
use crate::pipe::PipeStream;
use crate::win32::HANDLE;
use pigtree_protocol::crc32c::compute_crc32c;
use pigtree_protocol::frame::{
    read_frame, write_frame, ChannelTag, Frame, FrameFlags, FrameHeader, FrameParseError, CRC_SIZE,
    HEADER_SIZE, MAGIC, MAX_PAYLOAD_SIZE, SCHEMA_VERSION,
};
use pigtree_protocol::Message;
use std::io::{Read, Write};

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

    pub fn recv_frame_interruptible(
        &mut self,
        cancel_event: Option<HANDLE>,
        timeout_ms: Option<u32>,
    ) -> Result<Option<Frame>, IpcError> {
        let mut header_buf = [0u8; HEADER_SIZE];
        let mut first_byte = [0u8; 1];

        let n = self
            .stream
            .read_overlapped(&mut first_byte, cancel_event, timeout_ms)?;
        if n == 0 {
            return Ok(None);
        }
        header_buf[0] = first_byte[0];

        self.stream
            .read_exact_interruptible(&mut header_buf[1..], cancel_event, timeout_ms)?;

        let magic: [u8; 2] = [header_buf[0], header_buf[1]];
        if magic != MAGIC {
            return Err(IpcError::Protocol(FrameParseError::InvalidMagic(magic)));
        }

        let version = u16::from_le_bytes([header_buf[2], header_buf[3]]);
        if version != SCHEMA_VERSION {
            return Err(IpcError::Protocol(FrameParseError::UnsupportedVersion(
                version,
            )));
        }

        let channel_tag = ChannelTag::from_u8(header_buf[4]).map_err(IpcError::Protocol)?;
        let flags = FrameFlags(header_buf[5]);
        let reserved = u16::from_le_bytes([header_buf[6], header_buf[7]]);
        if reserved != 0 {
            return Err(IpcError::Protocol(FrameParseError::InvalidReserved(
                reserved,
            )));
        }

        let sequence_number = u64::from_le_bytes([
            header_buf[8],
            header_buf[9],
            header_buf[10],
            header_buf[11],
            header_buf[12],
            header_buf[13],
            header_buf[14],
            header_buf[15],
        ]);

        let payload_len = u32::from_le_bytes([
            header_buf[16],
            header_buf[17],
            header_buf[18],
            header_buf[19],
        ]) as usize;
        if payload_len > MAX_PAYLOAD_SIZE {
            return Err(IpcError::Protocol(FrameParseError::PayloadTooLarge(
                payload_len,
            )));
        }

        let mut payload = vec![0u8; payload_len];
        if payload_len > 0 {
            self.stream
                .read_exact_interruptible(&mut payload, cancel_event, timeout_ms)?;
        }

        let mut crc_buf = [0u8; CRC_SIZE];
        self.stream
            .read_exact_interruptible(&mut crc_buf, cancel_event, timeout_ms)?;
        let expected_crc = u32::from_le_bytes(crc_buf);

        let mut data_for_crc = Vec::with_capacity(HEADER_SIZE + payload_len);
        data_for_crc.extend_from_slice(&header_buf);
        data_for_crc.extend_from_slice(&payload);
        let calculated_crc = compute_crc32c(&data_for_crc);

        if expected_crc != calculated_crc {
            return Err(IpcError::Protocol(FrameParseError::ChecksumMismatch {
                expected: expected_crc,
                calculated: calculated_crc,
            }));
        }

        Ok(Some(Frame {
            header: FrameHeader {
                channel_tag,
                flags,
                sequence_number,
            },
            payload,
        }))
    }

    pub fn recv_message_interruptible<M: Message + Default>(
        &mut self,
        cancel_event: Option<HANDLE>,
        timeout_ms: Option<u32>,
    ) -> Result<Option<(FrameHeader, M)>, IpcError> {
        match self.recv_frame_interruptible(cancel_event, timeout_ms)? {
            Some(frame) => {
                let msg = M::decode(&frame.payload[..])?;
                Ok(Some((frame.header, msg)))
            }
            None => Ok(None),
        }
    }
}
