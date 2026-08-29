//! Binary observation record framing and codecs for worker -> engine communication.

use std::fmt;
use std::io::{self, Read, Write};
use std::str::Utf8Error;

pub const WORKER_MAGIC: [u8; 4] = [0x50, 0x54, 0x57, 0x4F]; // ASCII "PTWO"
pub const WORKER_STREAM_VERSION: u16 = 0x0001;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum RecordTag {
    Directory = 0x01,
    File = 0x02,
    SpecialObject = 0x03,
    CoverageGap = 0x04,
    Terminal = 0x05,
}

impl RecordTag {
    pub fn from_u8(val: u8) -> Result<Self, ObservationDecodeError> {
        match val {
            0x01 => Ok(RecordTag::Directory),
            0x02 => Ok(RecordTag::File),
            0x03 => Ok(RecordTag::SpecialObject),
            0x04 => Ok(RecordTag::CoverageGap),
            0x05 => Ok(RecordTag::Terminal),
            other => Err(ObservationDecodeError::InvalidRecordTag(other)),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum RunOutcome {
    Finished = 0,
    Cancelled = 1,
    Failed = 2,
}

impl RunOutcome {
    pub fn from_u8(val: u8) -> Result<Self, ObservationDecodeError> {
        match val {
            0 => Ok(RunOutcome::Finished),
            1 => Ok(RunOutcome::Cancelled),
            2 => Ok(RunOutcome::Failed),
            other => Err(ObservationDecodeError::InvalidOutcome(other)),
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            RunOutcome::Finished => "finished",
            RunOutcome::Cancelled => "cancelled",
            RunOutcome::Failed => "failed",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectoryObservation {
    pub entry_id: u32,
    pub parent_id: u32,
    pub name: String,
    pub file_attributes: u32,
    pub reparse_tag: u32,
    pub creation_time_utc_ms: u64,
    pub last_write_time_utc_ms: u64,
    pub last_access_time_utc_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileObservation {
    pub entry_id: u32,
    pub parent_id: u32,
    pub name: String,
    pub logical_size: u64,
    pub allocated_size: Option<u64>,
    pub file_attributes: u32,
    pub reparse_tag: u32,
    pub creation_time_utc_ms: u64,
    pub last_write_time_utc_ms: u64,
    pub last_access_time_utc_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpecialObservation {
    pub entry_id: u32,
    pub parent_id: u32,
    pub name: String,
    pub file_attributes: u32,
    pub reparse_tag: u32,
    pub creation_time_utc_ms: u64,
    pub last_write_time_utc_ms: u64,
    pub last_access_time_utc_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoverageGapObservation {
    pub path: String,
    pub error_code: u32,
    pub error_message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalObservation {
    pub outcome: RunOutcome,
    pub total_directories: u64,
    pub total_files: u64,
    pub total_logical_bytes: u64,
    pub total_allocated_bytes: u64,
    pub coverage_gap_count: u32,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObservationRecord {
    Directory(DirectoryObservation),
    File(FileObservation),
    Special(SpecialObservation),
    CoverageGap(CoverageGapObservation),
    Terminal(TerminalObservation),
}

#[derive(Debug)]
pub enum ObservationDecodeError {
    PrematureEof,
    InvalidMagic([u8; 4]),
    UnsupportedVersion(u16),
    InvalidRecordTag(u8),
    InvalidUtf8(Utf8Error),
    InvalidOutcome(u8),
    Io(io::Error),
}

impl fmt::Display for ObservationDecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ObservationDecodeError::PrematureEof => {
                write!(f, "premature EOF in observation stream")
            }
            ObservationDecodeError::InvalidMagic(m) => {
                write!(
                    f,
                    "invalid observation stream magic: [{:#04x}, {:#04x}, {:#04x}, {:#04x}]",
                    m[0], m[1], m[2], m[3]
                )
            }
            ObservationDecodeError::UnsupportedVersion(v) => {
                write!(f, "unsupported observation stream version: {v}")
            }
            ObservationDecodeError::InvalidRecordTag(t) => {
                write!(f, "invalid observation record tag: {t:#04x}")
            }
            ObservationDecodeError::InvalidUtf8(e) => {
                write!(f, "invalid UTF-8 in observation string: {e}")
            }
            ObservationDecodeError::InvalidOutcome(o) => {
                write!(f, "invalid run outcome code: {o}")
            }
            ObservationDecodeError::Io(e) => write!(f, "I/O error in observation stream: {e}"),
        }
    }
}

impl std::error::Error for ObservationDecodeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ObservationDecodeError::InvalidUtf8(e) => Some(e),
            ObservationDecodeError::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<io::Error> for ObservationDecodeError {
    fn from(e: io::Error) -> Self {
        if e.kind() == io::ErrorKind::UnexpectedEof {
            ObservationDecodeError::PrematureEof
        } else {
            ObservationDecodeError::Io(e)
        }
    }
}

impl From<Utf8Error> for ObservationDecodeError {
    fn from(e: Utf8Error) -> Self {
        ObservationDecodeError::InvalidUtf8(e)
    }
}

fn write_u16_str<W: Write>(writer: &mut W, s: &str) -> io::Result<()> {
    let bytes = s.as_bytes();
    let len = u16::try_from(bytes.len()).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "string length {} exceeds maximum u16 length ({})",
                bytes.len(),
                u16::MAX
            ),
        )
    })?;
    writer.write_all(&len.to_le_bytes())?;
    writer.write_all(bytes)?;
    Ok(())
}

fn read_u16_str<R: Read>(reader: &mut R) -> Result<String, ObservationDecodeError> {
    let mut len_buf = [0u8; 2];
    reader.read_exact(&mut len_buf)?;
    let len = u16::from_le_bytes(len_buf) as usize;
    let mut buf = vec![0u8; len];
    reader.read_exact(&mut buf)?;
    String::from_utf8(buf).map_err(|e| ObservationDecodeError::InvalidUtf8(e.utf8_error()))
}

/// Writer for encoding packed Little-Endian observation records to a stream.
#[derive(Debug)]
pub struct ObservationWriter<W: Write> {
    writer: W,
}

impl<W: Write> ObservationWriter<W> {
    pub fn new(mut writer: W, target_path: &str) -> Result<Self, io::Error> {
        writer.write_all(&WORKER_MAGIC)?;
        writer.write_all(&WORKER_STREAM_VERSION.to_le_bytes())?;
        write_u16_str(&mut writer, target_path)?;
        writer.flush()?;
        Ok(Self { writer })
    }

    pub fn write_directory(&mut self, dir: &DirectoryObservation) -> Result<(), io::Error> {
        self.writer.write_all(&[RecordTag::Directory as u8])?;
        self.writer.write_all(&dir.entry_id.to_le_bytes())?;
        self.writer.write_all(&dir.parent_id.to_le_bytes())?;
        self.writer.write_all(&dir.file_attributes.to_le_bytes())?;
        self.writer.write_all(&dir.reparse_tag.to_le_bytes())?;
        self.writer
            .write_all(&dir.creation_time_utc_ms.to_le_bytes())?;
        self.writer
            .write_all(&dir.last_write_time_utc_ms.to_le_bytes())?;
        self.writer
            .write_all(&dir.last_access_time_utc_ms.to_le_bytes())?;
        write_u16_str(&mut self.writer, &dir.name)?;
        Ok(())
    }

    pub fn write_file(&mut self, file: &FileObservation) -> Result<(), io::Error> {
        self.writer.write_all(&[RecordTag::File as u8])?;
        self.writer.write_all(&file.entry_id.to_le_bytes())?;
        self.writer.write_all(&file.parent_id.to_le_bytes())?;
        self.writer.write_all(&file.logical_size.to_le_bytes())?;

        match file.allocated_size {
            Some(alloc) => {
                self.writer.write_all(&[1u8])?; // Known
                self.writer.write_all(&alloc.to_le_bytes())?;
            }
            None => {
                self.writer.write_all(&[0u8])?; // Unavailable
                self.writer.write_all(&0u64.to_le_bytes())?;
            }
        }

        self.writer.write_all(&file.file_attributes.to_le_bytes())?;
        self.writer.write_all(&file.reparse_tag.to_le_bytes())?;
        self.writer
            .write_all(&file.creation_time_utc_ms.to_le_bytes())?;
        self.writer
            .write_all(&file.last_write_time_utc_ms.to_le_bytes())?;
        self.writer
            .write_all(&file.last_access_time_utc_ms.to_le_bytes())?;
        write_u16_str(&mut self.writer, &file.name)?;
        Ok(())
    }

    pub fn write_special(&mut self, special: &SpecialObservation) -> Result<(), io::Error> {
        self.writer.write_all(&[RecordTag::SpecialObject as u8])?;
        self.writer.write_all(&special.entry_id.to_le_bytes())?;
        self.writer.write_all(&special.parent_id.to_le_bytes())?;
        self.writer
            .write_all(&special.file_attributes.to_le_bytes())?;
        self.writer.write_all(&special.reparse_tag.to_le_bytes())?;
        self.writer
            .write_all(&special.creation_time_utc_ms.to_le_bytes())?;
        self.writer
            .write_all(&special.last_write_time_utc_ms.to_le_bytes())?;
        self.writer
            .write_all(&special.last_access_time_utc_ms.to_le_bytes())?;
        write_u16_str(&mut self.writer, &special.name)?;
        Ok(())
    }

    pub fn write_coverage_gap(&mut self, gap: &CoverageGapObservation) -> Result<(), io::Error> {
        self.writer.write_all(&[RecordTag::CoverageGap as u8])?;
        self.writer.write_all(&gap.error_code.to_le_bytes())?;
        write_u16_str(&mut self.writer, &gap.path)?;
        write_u16_str(&mut self.writer, &gap.error_message)?;
        Ok(())
    }

    pub fn write_terminal(&mut self, term: &TerminalObservation) -> Result<(), io::Error> {
        self.writer.write_all(&[RecordTag::Terminal as u8])?;
        self.writer.write_all(&[term.outcome as u8])?;
        self.writer
            .write_all(&term.total_directories.to_le_bytes())?;
        self.writer.write_all(&term.total_files.to_le_bytes())?;
        self.writer
            .write_all(&term.total_logical_bytes.to_le_bytes())?;
        self.writer
            .write_all(&term.total_allocated_bytes.to_le_bytes())?;
        self.writer
            .write_all(&term.coverage_gap_count.to_le_bytes())?;
        self.writer.write_all(&term.duration_ms.to_le_bytes())?;
        self.writer.flush()?;
        Ok(())
    }

    pub fn flush(&mut self) -> Result<(), io::Error> {
        self.writer.flush()
    }

    pub fn into_inner(self) -> W {
        self.writer
    }

    pub fn get_ref(&self) -> &W {
        &self.writer
    }

    pub fn get_mut(&mut self) -> &mut W {
        &mut self.writer
    }
}

/// Reader for decoding packed Little-Endian observation records from a stream.
#[derive(Debug)]
pub struct ObservationReader<R: Read> {
    reader: R,
    target_path: String,
}

impl<R: Read> ObservationReader<R> {
    pub fn new(mut reader: R) -> Result<Self, ObservationDecodeError> {
        let mut magic = [0u8; 4];
        reader.read_exact(&mut magic)?;
        if magic != WORKER_MAGIC {
            return Err(ObservationDecodeError::InvalidMagic(magic));
        }

        let mut ver_buf = [0u8; 2];
        reader.read_exact(&mut ver_buf)?;
        let version = u16::from_le_bytes(ver_buf);
        if version != WORKER_STREAM_VERSION {
            return Err(ObservationDecodeError::UnsupportedVersion(version));
        }

        let target_path = read_u16_str(&mut reader)?;

        Ok(Self {
            reader,
            target_path,
        })
    }

    pub fn target_path(&self) -> &str {
        &self.target_path
    }

    pub fn into_inner(self) -> R {
        self.reader
    }

    pub fn get_ref(&self) -> &R {
        &self.reader
    }

    pub fn get_mut(&mut self) -> &mut R {
        &mut self.reader
    }

    pub fn read_record(&mut self) -> Result<Option<ObservationRecord>, ObservationDecodeError> {
        let mut tag_buf = [0u8; 1];
        match self.reader.read(&mut tag_buf) {
            Ok(0) => return Ok(None), // Clean EOF between records
            Ok(1) => {}
            Ok(_) => unreachable!(),
            Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
            Err(e) => return Err(ObservationDecodeError::Io(e)),
        }

        let tag = RecordTag::from_u8(tag_buf[0])?;

        match tag {
            RecordTag::Directory => {
                let mut fixed_buf = [0u8; 4 + 4 + 4 + 4 + 8 + 8 + 8];
                self.reader.read_exact(&mut fixed_buf)?;

                let entry_id = u32::from_le_bytes(fixed_buf[0..4].try_into().unwrap());
                let parent_id = u32::from_le_bytes(fixed_buf[4..8].try_into().unwrap());
                let file_attributes = u32::from_le_bytes(fixed_buf[8..12].try_into().unwrap());
                let reparse_tag = u32::from_le_bytes(fixed_buf[12..16].try_into().unwrap());
                let creation_time_utc_ms =
                    u64::from_le_bytes(fixed_buf[16..24].try_into().unwrap());
                let last_write_time_utc_ms =
                    u64::from_le_bytes(fixed_buf[24..32].try_into().unwrap());
                let last_access_time_utc_ms =
                    u64::from_le_bytes(fixed_buf[32..40].try_into().unwrap());
                let name = read_u16_str(&mut self.reader)?;

                Ok(Some(ObservationRecord::Directory(DirectoryObservation {
                    entry_id,
                    parent_id,
                    name,
                    file_attributes,
                    reparse_tag,
                    creation_time_utc_ms,
                    last_write_time_utc_ms,
                    last_access_time_utc_ms,
                })))
            }
            RecordTag::File => {
                let mut fixed_buf = [0u8; 4 + 4 + 8 + 1 + 8 + 4 + 4 + 8 + 8 + 8];
                self.reader.read_exact(&mut fixed_buf)?;

                let entry_id = u32::from_le_bytes(fixed_buf[0..4].try_into().unwrap());
                let parent_id = u32::from_le_bytes(fixed_buf[4..8].try_into().unwrap());
                let logical_size = u64::from_le_bytes(fixed_buf[8..16].try_into().unwrap());
                let alloc_known = fixed_buf[16] != 0;
                let raw_alloc = u64::from_le_bytes(fixed_buf[17..25].try_into().unwrap());
                let allocated_size = if alloc_known { Some(raw_alloc) } else { None };
                let file_attributes = u32::from_le_bytes(fixed_buf[25..29].try_into().unwrap());
                let reparse_tag = u32::from_le_bytes(fixed_buf[29..33].try_into().unwrap());
                let creation_time_utc_ms =
                    u64::from_le_bytes(fixed_buf[33..41].try_into().unwrap());
                let last_write_time_utc_ms =
                    u64::from_le_bytes(fixed_buf[41..49].try_into().unwrap());
                let last_access_time_utc_ms =
                    u64::from_le_bytes(fixed_buf[49..57].try_into().unwrap());
                let name = read_u16_str(&mut self.reader)?;

                Ok(Some(ObservationRecord::File(FileObservation {
                    entry_id,
                    parent_id,
                    name,
                    logical_size,
                    allocated_size,
                    file_attributes,
                    reparse_tag,
                    creation_time_utc_ms,
                    last_write_time_utc_ms,
                    last_access_time_utc_ms,
                })))
            }
            RecordTag::SpecialObject => {
                let mut fixed_buf = [0u8; 4 + 4 + 4 + 4 + 8 + 8 + 8];
                self.reader.read_exact(&mut fixed_buf)?;

                let entry_id = u32::from_le_bytes(fixed_buf[0..4].try_into().unwrap());
                let parent_id = u32::from_le_bytes(fixed_buf[4..8].try_into().unwrap());
                let file_attributes = u32::from_le_bytes(fixed_buf[8..12].try_into().unwrap());
                let reparse_tag = u32::from_le_bytes(fixed_buf[12..16].try_into().unwrap());
                let creation_time_utc_ms =
                    u64::from_le_bytes(fixed_buf[16..24].try_into().unwrap());
                let last_write_time_utc_ms =
                    u64::from_le_bytes(fixed_buf[24..32].try_into().unwrap());
                let last_access_time_utc_ms =
                    u64::from_le_bytes(fixed_buf[32..40].try_into().unwrap());
                let name = read_u16_str(&mut self.reader)?;

                Ok(Some(ObservationRecord::Special(SpecialObservation {
                    entry_id,
                    parent_id,
                    name,
                    file_attributes,
                    reparse_tag,
                    creation_time_utc_ms,
                    last_write_time_utc_ms,
                    last_access_time_utc_ms,
                })))
            }
            RecordTag::CoverageGap => {
                let mut err_buf = [0u8; 4];
                self.reader.read_exact(&mut err_buf)?;
                let error_code = u32::from_le_bytes(err_buf);
                let path = read_u16_str(&mut self.reader)?;
                let error_message = read_u16_str(&mut self.reader)?;

                Ok(Some(ObservationRecord::CoverageGap(
                    CoverageGapObservation {
                        path,
                        error_code,
                        error_message,
                    },
                )))
            }
            RecordTag::Terminal => {
                let mut fixed_buf = [0u8; 1 + 8 + 8 + 8 + 8 + 4 + 8];
                self.reader.read_exact(&mut fixed_buf)?;

                let outcome = RunOutcome::from_u8(fixed_buf[0])?;
                let total_directories = u64::from_le_bytes(fixed_buf[1..9].try_into().unwrap());
                let total_files = u64::from_le_bytes(fixed_buf[9..17].try_into().unwrap());
                let total_logical_bytes = u64::from_le_bytes(fixed_buf[17..25].try_into().unwrap());
                let total_allocated_bytes =
                    u64::from_le_bytes(fixed_buf[25..33].try_into().unwrap());
                let coverage_gap_count = u32::from_le_bytes(fixed_buf[33..37].try_into().unwrap());
                let duration_ms = u64::from_le_bytes(fixed_buf[37..45].try_into().unwrap());

                Ok(Some(ObservationRecord::Terminal(TerminalObservation {
                    outcome,
                    total_directories,
                    total_files,
                    total_logical_bytes,
                    total_allocated_bytes,
                    coverage_gap_count,
                    duration_ms,
                })))
            }
        }
    }
}
