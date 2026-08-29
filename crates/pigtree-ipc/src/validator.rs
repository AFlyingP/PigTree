//! Authoritative local scan target validator and classification types.

use crate::win32::*;
use std::fmt;
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};

/// Convert a string slice to null-terminated UTF-16 for Win32 API calls.
pub fn to_wide_null(s: &str) -> Vec<u16> {
    std::ffi::OsStr::new(s)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

/// Convert a null-terminated UTF-16 slice to a Rust String.
pub fn wide_slice_to_string(slice: &[u16]) -> String {
    let len = slice.iter().position(|&c| c == 0).unwrap_or(slice.len());
    String::from_utf16_lossy(&slice[..len])
}

/// Win32 drive kind classification for scan targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DriveKind {
    Unknown,
    NoRootDir,
    Removable,
    Fixed,
    Remote,
    CdRom,
    RamDisk,
    Other(u32),
}

impl DriveKind {
    /// Classify a raw Win32 drive type constant returned by GetDriveTypeW.
    pub fn from_raw(raw: u32) -> Self {
        match raw {
            DRIVE_UNKNOWN => Self::Unknown,
            DRIVE_NO_ROOT_DIR => Self::NoRootDir,
            DRIVE_REMOVABLE => Self::Removable,
            DRIVE_FIXED => Self::Fixed,
            DRIVE_REMOTE => Self::Remote,
            DRIVE_CDROM => Self::CdRom,
            DRIVE_RAMDISK => Self::RamDisk,
            other => Self::Other(other),
        }
    }

    /// Whether this drive type is supported for PigTree local scans.
    /// Only fixed storage (DRIVE_FIXED) and removable storage (DRIVE_REMOVABLE) are accepted.
    pub fn is_supported(&self) -> bool {
        matches!(self, Self::Fixed | Self::Removable)
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Unknown => "DRIVE_UNKNOWN",
            Self::NoRootDir => "DRIVE_NO_ROOT_DIR",
            Self::Removable => "DRIVE_REMOVABLE",
            Self::Fixed => "DRIVE_FIXED",
            Self::Remote => "DRIVE_REMOTE",
            Self::CdRom => "DRIVE_CDROM",
            Self::RamDisk => "DRIVE_RAMDISK",
            Self::Other(_) => "DRIVE_OTHER",
        }
    }
}

/// Filesystem kind classification for scan targets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileSystemKind {
    Ntfs,
    ReFs,
    Fat32,
    ExFat,
    Unsupported(String),
}

impl FileSystemKind {
    /// Parse and classify a filesystem name string (case-insensitively).
    /// Accepts NTFS, ReFS, FAT32, and exFAT.
    pub fn parse(name: &str) -> Self {
        let trimmed = name.trim();
        if trimmed.eq_ignore_ascii_case("NTFS") {
            Self::Ntfs
        } else if trimmed.eq_ignore_ascii_case("ReFS") {
            Self::ReFs
        } else if trimmed.eq_ignore_ascii_case("FAT32") {
            Self::Fat32
        } else if trimmed.eq_ignore_ascii_case("exFAT") {
            Self::ExFat
        } else {
            Self::Unsupported(trimmed.to_string())
        }
    }

    /// Whether this filesystem is supported for PigTree local scans.
    pub fn is_supported(&self) -> bool {
        !matches!(self, Self::Unsupported(_))
    }

    pub fn as_str(&self) -> &str {
        match self {
            Self::Ntfs => "NTFS",
            Self::ReFs => "ReFS",
            Self::Fat32 => "FAT32",
            Self::ExFat => "exFAT",
            Self::Unsupported(s) => s.as_str(),
        }
    }
}

/// Lexically determines whether a path string represents a UNC or network path.
///
/// This check is purely lexical and performs zero Win32 or filesystem calls, avoiding
/// any unintended network probes or hangs.
pub fn is_lexical_unc(raw: &str) -> bool {
    let s = raw.trim();
    if s.is_empty() {
        return false;
    }

    let bytes = s.as_bytes();
    let is_slash = |b: u8| b == b'\\' || b == b'/';

    // 1. Check for extended UNC prefix: \\?\UNC\ or \\.\UNC\ or \??\UNC\ or //?/UNC/ etc.
    if bytes.len() >= 8 {
        let first_two_slash = (is_slash(bytes[0]) && is_slash(bytes[1]))
            || (bytes[0] == b'\\' && bytes[1] == b'?' && bytes[2] == b'?');
        if (first_two_slash && (bytes[2] == b'?' || bytes[2] == b'.') && is_slash(bytes[3]))
            || (bytes.starts_with(br"\??\\"))
        {
            let unc_part = &bytes[4..7];
            let after_unc = bytes[7];
            if unc_part.eq_ignore_ascii_case(b"UNC") && is_slash(after_unc) {
                return true;
            }
        }
    }

    // 2. Check for standard UNC or device paths starting with \\ or // or \/ or /\
    if bytes.len() >= 2 && is_slash(bytes[0]) && is_slash(bytes[1]) {
        // If it starts with \\?\ or \\.\, check if it is a local DOS drive (e.g. \\?\C:\ or \\.\C:\)
        if bytes.len() >= 4 && (bytes[2] == b'?' || bytes[2] == b'.') && is_slash(bytes[3]) {
            // Check if followed by drive letter like C:
            if bytes.len() >= 6 && bytes[4].is_ascii_alphabetic() && bytes[5] == b':' {
                // Extended local drive path, e.g. \\?\C:\foo or \\.\C:\foo
                return false;
            }
            // Any other \\?\ or \\.\ that is not a local drive letter is treated as UNC / unsupported device
            return true;
        }

        // Starts with \\ or // followed by a regular char (e.g. \\server\share or //server/share)
        return true;
    }

    false
}

/// Typed validation errors for scan targets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetValidationError {
    /// Target path is empty or whitespace-only.
    EmptyPath,
    /// Target path was identified as a UNC or network path.
    UncPathNotSupported { path: String },
    /// Target directory does not exist.
    NotFound { path: String },
    /// Target path exists but is not a directory.
    NotADirectory { path: String },
    /// Safe canonicalization of target path failed.
    CanonicalizationFailed { path: String, reason: String },
    /// Failed to resolve volume mount point / volume root via GetVolumePathNameW.
    VolumeResolutionFailed { path: String, win32_code: u32 },
    /// Drive type is not supported (e.g. REMOTE, CDROM, RAMDISK, UNKNOWN, NO_ROOT_DIR).
    UnsupportedDriveType {
        path: String,
        volume_root: String,
        drive_kind: DriveKind,
    },
    /// Failed to query volume information via GetVolumeInformationW.
    VolumeInformationFailed {
        path: String,
        volume_root: String,
        win32_code: u32,
    },
    /// Volume filesystem is not supported (only NTFS, ReFS, FAT32, exFAT are accepted).
    UnsupportedFileSystem {
        path: String,
        volume_root: String,
        filesystem_name: String,
    },
}

impl fmt::Display for TargetValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyPath => write!(f, "Target directory path cannot be empty"),
            Self::UncPathNotSupported { path } => {
                write!(f, "UNC and network paths are not supported: {path}")
            }
            Self::NotFound { path } => write!(f, "Target directory does not exist: {path}"),
            Self::NotADirectory { path } => write!(f, "Target path is not a directory: {path}"),
            Self::CanonicalizationFailed { path, reason } => {
                write!(f, "Failed to resolve target path '{path}': {reason}")
            }
            Self::VolumeResolutionFailed { path, win32_code } => {
                write!(
                    f,
                    "Failed to determine volume root for '{path}' (Win32 error {win32_code})"
                )
            }
            Self::UnsupportedDriveType {
                volume_root,
                drive_kind,
                ..
            } => {
                write!(
                    f,
                    "Target volume '{volume_root}' has unsupported drive type '{}' (only fixed and removable local drives are supported)",
                    drive_kind.as_str()
                )
            }
            Self::VolumeInformationFailed {
                volume_root,
                win32_code,
                ..
            } => {
                write!(
                    f,
                    "Failed to query volume information for '{volume_root}' (Win32 error {win32_code})"
                )
            }
            Self::UnsupportedFileSystem {
                volume_root,
                filesystem_name,
                ..
            } => {
                write!(
                    f,
                    "Filesystem '{filesystem_name}' on volume '{volume_root}' is not supported (supported filesystems: NTFS, ReFS, FAT32, exFAT)"
                )
            }
        }
    }
}

impl std::error::Error for TargetValidationError {}

/// Validated and classified scan target information.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedScanTarget {
    /// Original path string provided by caller/user.
    pub display_path: String,
    /// Canonicalized filesystem path.
    pub canonical_path: PathBuf,
    /// Extended path for Win32 file operations (e.g. \\?\C:\...).
    pub extended_path: String,
    /// Volume root path derived via GetVolumePathNameW (e.g. C:\).
    pub volume_root: String,
    /// Verified drive kind (Fixed or Removable).
    pub drive_kind: DriveKind,
    /// Verified filesystem kind (NTFS, ReFS, FAT32, or exFAT).
    pub filesystem_kind: FileSystemKind,
    /// Raw filesystem name string from GetVolumeInformationW.
    pub filesystem_name: String,
    /// Volume serial number if available.
    pub volume_serial_number: Option<u32>,
}

/// Authoritatively validates a local scan target path against all PigTree requirements.
///
/// 1. Rejects empty or whitespace paths.
/// 2. Lexically rejects UNC and extended UNC paths before making any OS call.
/// 3. Validates path exists and is a directory.
/// 4. Canonicalizes path safely and derives extended path.
/// 5. Derives volume root using `GetVolumePathNameW`.
/// 6. Validates drive type using `GetDriveTypeW` (accepts only DRIVE_FIXED and DRIVE_REMOVABLE).
/// 7. Validates filesystem using `GetVolumeInformationW` (accepts case-insensitive NTFS, ReFS, FAT32, exFAT).
pub fn validate_scan_target(
    target: impl AsRef<Path>,
) -> Result<ValidatedScanTarget, TargetValidationError> {
    let target_ref = target.as_ref();
    let display_path = target_ref.to_string_lossy().to_string();
    let trimmed = display_path.trim();

    if trimmed.is_empty() {
        return Err(TargetValidationError::EmptyPath);
    }

    // Lexical UNC check must run before any filesystem/OS call to avoid network access
    if is_lexical_unc(trimmed) {
        return Err(TargetValidationError::UncPathNotSupported { path: display_path });
    }

    let path = Path::new(trimmed);
    if !path.exists() {
        return Err(TargetValidationError::NotFound { path: display_path });
    }

    if !path.is_dir() {
        return Err(TargetValidationError::NotADirectory { path: display_path });
    }

    // Canonicalize path safely
    let canonical_path = match path.canonicalize() {
        Ok(p) => p,
        Err(e) => {
            return Err(TargetValidationError::CanonicalizationFailed {
                path: display_path,
                reason: e.to_string(),
            });
        }
    };

    let canonical_str = canonical_path.to_string_lossy().to_string();

    // Check if canonicalization yielded a UNC path (e.g. underlying network share)
    if is_lexical_unc(&canonical_str) {
        return Err(TargetValidationError::UncPathNotSupported { path: display_path });
    }

    // Determine extended path
    let extended_path = if canonical_str.starts_with(r"\\?\") || canonical_str.starts_with(r"\\.\")
    {
        canonical_str.clone()
    } else {
        format!(r"\\?\{}", canonical_str)
    };

    // Derive volume root with GetVolumePathNameW
    let extended_w = to_wide_null(&extended_path);
    let mut volume_root_buf = [0u16; 512];
    let ok = unsafe {
        GetVolumePathNameW(
            extended_w.as_ptr(),
            volume_root_buf.as_mut_ptr(),
            volume_root_buf.len() as DWORD,
        )
    };

    if ok == 0 {
        let err = unsafe { GetLastError() };
        return Err(TargetValidationError::VolumeResolutionFailed {
            path: display_path,
            win32_code: err,
        });
    }

    let volume_root = wide_slice_to_string(&volume_root_buf);
    let volume_root_w = to_wide_null(&volume_root);

    // Call GetDriveTypeW
    let raw_drive_type = unsafe { GetDriveTypeW(volume_root_w.as_ptr()) };
    let drive_kind = DriveKind::from_raw(raw_drive_type);

    if !drive_kind.is_supported() {
        return Err(TargetValidationError::UnsupportedDriveType {
            path: display_path,
            volume_root,
            drive_kind,
        });
    }

    // Call GetVolumeInformationW
    let mut vol_name = [0u16; 260];
    let mut serial_number: DWORD = 0;
    let mut max_comp_len: DWORD = 0;
    let mut fs_flags: DWORD = 0;
    let mut fs_name_buf = [0u16; 260];

    let ok_vol = unsafe {
        GetVolumeInformationW(
            volume_root_w.as_ptr(),
            vol_name.as_mut_ptr(),
            vol_name.len() as DWORD,
            &mut serial_number,
            &mut max_comp_len,
            &mut fs_flags,
            fs_name_buf.as_mut_ptr(),
            fs_name_buf.len() as DWORD,
        )
    };

    if ok_vol == 0 {
        let err = unsafe { GetLastError() };
        return Err(TargetValidationError::VolumeInformationFailed {
            path: display_path,
            volume_root,
            win32_code: err,
        });
    }

    let fs_name = wide_slice_to_string(&fs_name_buf);
    let fs_kind = FileSystemKind::parse(&fs_name);

    if !fs_kind.is_supported() {
        return Err(TargetValidationError::UnsupportedFileSystem {
            path: display_path,
            volume_root,
            filesystem_name: fs_name,
        });
    }

    Ok(ValidatedScanTarget {
        display_path,
        canonical_path,
        extended_path,
        volume_root,
        drive_kind,
        filesystem_kind: fs_kind,
        filesystem_name: fs_name,
        volume_serial_number: Some(serial_number),
    })
}
