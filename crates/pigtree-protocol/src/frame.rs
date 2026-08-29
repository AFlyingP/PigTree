//! Binary frame header format, serialization, and deserialization.

use crate::crc32c::compute_crc32c;
use std::fmt;
use std::io::{self, Read, Write};

pub const MAGIC: [u8; 2] = [0x50, 0x54]; // ASCII "PT"
pub const SCHEMA_VERSION: u16 = 0x0001;
pub const HEADER_SIZE: usize = 20;
pub const CRC_SIZE: usize = 4;
pub const MAX_PAYLOAD_SIZE: usize = 4 * 1024 * 1024; // 4 MiB

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ChannelTag {
    Command = 0x01,
    LosslessDomain = 0x02,
    ProgressPulse = 0x03,
    CancellationHeartbeat = 0x04,
}

impl ChannelTag {
    pub fn from_u8(val: u8) -> Result<Self, FrameParseError> {
        match val {
            0x01 => Ok(ChannelTag::Command),
            0x02 => Ok(ChannelTag::LosslessDomain),
            0x03 => Ok(ChannelTag::ProgressPulse),
            0x04 => Ok(ChannelTag::CancellationHeartbeat),
            other => Err(FrameParseError::InvalidChannelTag(other)),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FrameFlags(pub u8);

impl FrameFlags {
    pub const END_OF_STREAM: FrameFlags = FrameFlags(0x01);
    pub const CHALLENGE_REQUIRED: FrameFlags = FrameFlags(0x02);

    pub fn empty() -> Self {
        FrameFlags(0)
    }

    pub fn contains(&self, other: FrameFlags) -> bool {
        (self.0 & other.0) == other.0
    }

    pub fn bits(&self) -> u8 {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrameHeader {
    pub channel_tag: ChannelTag,
    pub flags: FrameFlags,
    pub sequence_number: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    pub header: FrameHeader,
    pub payload: Vec<u8>,
}

#[derive(Debug)]
pub enum FrameParseError {
    PrematureEof,
    InvalidMagic([u8; 2]),
    UnsupportedVersion(u16),
    InvalidChannelTag(u8),
    InvalidReserved(u16),
    PayloadTooLarge(usize),
    ChecksumMismatch { expected: u32, calculated: u32 },
    Io(io::Error),
}

impl fmt::Display for FrameParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FrameParseError::PrematureEof => write!(f, "premature EOF while reading frame"),
            FrameParseError::InvalidMagic(m) => {
                write!(f, "invalid frame magic: [{:#04x}, {:#04x}]", m[0], m[1])
            }
            FrameParseError::UnsupportedVersion(v) => write!(f, "unsupported wire version: {v}"),
            FrameParseError::InvalidChannelTag(t) => write!(f, "invalid channel tag: {t}"),
            FrameParseError::InvalidReserved(r) => write!(f, "non-zero reserved field: {r}"),
            FrameParseError::PayloadTooLarge(size) => {
                write!(f, "payload size {size} exceeds maximum {MAX_PAYLOAD_SIZE}")
            }
            FrameParseError::ChecksumMismatch {
                expected,
                calculated,
            } => {
                write!(
                    f,
                    "checksum mismatch: expected {expected:#010x}, calculated {calculated:#010x}"
                )
            }
            FrameParseError::Io(err) => write!(f, "I/O error during frame parsing: {err}"),
        }
    }
}

impl std::error::Error for FrameParseError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            FrameParseError::Io(err) => Some(err),
            _ => None,
        }
    }
}

impl From<io::Error> for FrameParseError {
    fn from(err: io::Error) -> Self {
        if err.kind() == io::ErrorKind::UnexpectedEof {
            FrameParseError::PrematureEof
        } else {
            FrameParseError::Io(err)
        }
    }
}

/// Encodes a frame header and payload into the 20-byte LE header + payload + 4-byte LE CRC32C format.
pub fn encode_frame(header: &FrameHeader, payload: &[u8]) -> Result<Vec<u8>, FrameParseError> {
    if payload.len() > MAX_PAYLOAD_SIZE {
        return Err(FrameParseError::PayloadTooLarge(payload.len()));
    }

    let total_size = HEADER_SIZE + payload.len() + CRC_SIZE;
    let mut buffer = Vec::with_capacity(total_size);

    // 0..2: Magic
    buffer.extend_from_slice(&MAGIC);
    // 2..4: Schema version
    buffer.extend_from_slice(&SCHEMA_VERSION.to_le_bytes());
    // 4: Channel tag
    buffer.push(header.channel_tag as u8);
    // 5: Frame flags
    buffer.push(header.flags.bits());
    // 6..8: Reserved (must be 0)
    buffer.extend_from_slice(&0u16.to_le_bytes());
    // 8..16: Sequence number
    buffer.extend_from_slice(&header.sequence_number.to_le_bytes());
    // 16..20: Payload length
    buffer.extend_from_slice(&(payload.len() as u32).to_le_bytes());

    // 20..20+N: Payload
    buffer.extend_from_slice(payload);

    // Compute CRC32C over header + payload
    let crc = compute_crc32c(&buffer);
    buffer.extend_from_slice(&crc.to_le_bytes());

    Ok(buffer)
}

/// Decodes a frame from an in-memory buffer.
pub fn decode_frame(buffer: &[u8]) -> Result<(FrameHeader, Vec<u8>), FrameParseError> {
    if buffer.len() < HEADER_SIZE {
        return Err(FrameParseError::PrematureEof);
    }

    let magic: [u8; 2] = [buffer[0], buffer[1]];
    if magic != MAGIC {
        return Err(FrameParseError::InvalidMagic(magic));
    }

    let version = u16::from_le_bytes([buffer[2], buffer[3]]);
    if version != SCHEMA_VERSION {
        return Err(FrameParseError::UnsupportedVersion(version));
    }

    let channel_tag = ChannelTag::from_u8(buffer[4])?;
    let flags = FrameFlags(buffer[5]);
    let reserved = u16::from_le_bytes([buffer[6], buffer[7]]);
    if reserved != 0 {
        return Err(FrameParseError::InvalidReserved(reserved));
    }

    let sequence_number = u64::from_le_bytes([
        buffer[8], buffer[9], buffer[10], buffer[11], buffer[12], buffer[13], buffer[14],
        buffer[15],
    ]);

    let payload_len = u32::from_le_bytes([buffer[16], buffer[17], buffer[18], buffer[19]]) as usize;
    if payload_len > MAX_PAYLOAD_SIZE {
        return Err(FrameParseError::PayloadTooLarge(payload_len));
    }

    let required_len = HEADER_SIZE + payload_len + CRC_SIZE;
    if buffer.len() < required_len {
        return Err(FrameParseError::PrematureEof);
    }

    let expected_crc = u32::from_le_bytes([
        buffer[HEADER_SIZE + payload_len],
        buffer[HEADER_SIZE + payload_len + 1],
        buffer[HEADER_SIZE + payload_len + 2],
        buffer[HEADER_SIZE + payload_len + 3],
    ]);

    let calculated_crc = compute_crc32c(&buffer[..HEADER_SIZE + payload_len]);
    if expected_crc != calculated_crc {
        return Err(FrameParseError::ChecksumMismatch {
            expected: expected_crc,
            calculated: calculated_crc,
        });
    }

    let payload = buffer[HEADER_SIZE..HEADER_SIZE + payload_len].to_vec();
    let header = FrameHeader {
        channel_tag,
        flags,
        sequence_number,
    };

    Ok((header, payload))
}

/// Reads and decodes a single complete frame from a stream.
///
/// Returns `Ok(None)` on clean EOF before any frame header bytes have been read.
/// Returns `Err(FrameParseError::PrematureEof)` if EOF is encountered after reading
/// partial header, payload, or CRC bytes.
pub fn read_frame<R: Read>(reader: &mut R) -> Result<Option<Frame>, FrameParseError> {
    let mut header_buf = [0u8; HEADER_SIZE];
    let mut first_byte = [0u8; 1];
    match reader.read(&mut first_byte) {
        Ok(0) => return Ok(None),
        Ok(1) => {
            header_buf[0] = first_byte[0];
        }
        Ok(_) => unreachable!(),
        Err(err) => return Err(FrameParseError::Io(err)),
    }

    match reader.read_exact(&mut header_buf[1..]) {
        Ok(()) => {}
        Err(err) if err.kind() == io::ErrorKind::UnexpectedEof => {
            return Err(FrameParseError::PrematureEof);
        }
        Err(err) => return Err(FrameParseError::Io(err)),
    }

    let magic: [u8; 2] = [header_buf[0], header_buf[1]];
    if magic != MAGIC {
        return Err(FrameParseError::InvalidMagic(magic));
    }

    let version = u16::from_le_bytes([header_buf[2], header_buf[3]]);
    if version != SCHEMA_VERSION {
        return Err(FrameParseError::UnsupportedVersion(version));
    }

    let channel_tag = ChannelTag::from_u8(header_buf[4])?;
    let flags = FrameFlags(header_buf[5]);
    let reserved = u16::from_le_bytes([header_buf[6], header_buf[7]]);
    if reserved != 0 {
        return Err(FrameParseError::InvalidReserved(reserved));
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
        return Err(FrameParseError::PayloadTooLarge(payload_len));
    }

    let mut payload = vec![0u8; payload_len];
    if payload_len > 0 {
        match reader.read_exact(&mut payload) {
            Ok(()) => {}
            Err(err) if err.kind() == io::ErrorKind::UnexpectedEof => {
                return Err(FrameParseError::PrematureEof);
            }
            Err(err) => return Err(FrameParseError::Io(err)),
        }
    }

    let mut crc_buf = [0u8; CRC_SIZE];
    match reader.read_exact(&mut crc_buf) {
        Ok(()) => {}
        Err(err) if err.kind() == io::ErrorKind::UnexpectedEof => {
            return Err(FrameParseError::PrematureEof);
        }
        Err(err) => return Err(FrameParseError::Io(err)),
    }
    let expected_crc = u32::from_le_bytes(crc_buf);

    let mut data_for_crc = Vec::with_capacity(HEADER_SIZE + payload_len);
    data_for_crc.extend_from_slice(&header_buf);
    data_for_crc.extend_from_slice(&payload);
    let calculated_crc = compute_crc32c(&data_for_crc);

    if expected_crc != calculated_crc {
        return Err(FrameParseError::ChecksumMismatch {
            expected: expected_crc,
            calculated: calculated_crc,
        });
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

/// Writes a frame to a stream.
pub fn write_frame<W: Write>(
    writer: &mut W,
    header: &FrameHeader,
    payload: &[u8],
) -> Result<(), FrameParseError> {
    let encoded = encode_frame(header, payload)?;
    writer.write_all(&encoded)?;
    writer.flush()?;
    Ok(())
}
