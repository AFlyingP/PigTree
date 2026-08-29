//! Tests for authoritative local scan target validation.

use pigtree_ipc::validator::*;
use pigtree_ipc::win32::*;
use std::fs::{self, File};

#[test]
fn test_lexical_unc_detection() {
    // UNC network paths (standard and forward slash)
    assert!(is_lexical_unc(r"\\server\share"));
    assert!(is_lexical_unc("//server/share"));
    assert!(is_lexical_unc(r"\\localhost\c$"));
    assert!(is_lexical_unc(r"\\127.0.0.1\c$"));
    assert!(is_lexical_unc(r"\\192.168.1.1\share\dir"));

    // Extended UNC paths
    assert!(is_lexical_unc(r"\\?\UNC\server\share"));
    assert!(is_lexical_unc(r"\\?\unc\server\share"));
    assert!(is_lexical_unc(r"\\.\UNC\server\share"));
    assert!(is_lexical_unc("//?/UNC/server/share"));
    assert!(is_lexical_unc(r"\??\UNC\server\share"));

    // Non-UNC paths (local paths, drive letters, extended DOS paths)
    assert!(!is_lexical_unc(r"C:\"));
    assert!(!is_lexical_unc("C:/"));
    assert!(!is_lexical_unc(r"C:\Users\test"));
    assert!(!is_lexical_unc("C:/Users/test"));
    assert!(!is_lexical_unc(r"\\?\C:\Users\test"));
    assert!(!is_lexical_unc(r"\\.\C:\Users\test"));
    assert!(!is_lexical_unc(r"\\?\D:\"));
    assert!(!is_lexical_unc("."));
    assert!(!is_lexical_unc(".."));
    assert!(!is_lexical_unc(r"relative\path"));
    assert!(!is_lexical_unc(""));
}

#[test]
fn test_drive_kind_classification() {
    // Accepted drive types: FIXED and REMOVABLE
    assert!(DriveKind::from_raw(DRIVE_FIXED).is_supported());
    assert_eq!(DriveKind::from_raw(DRIVE_FIXED), DriveKind::Fixed);
    assert_eq!(DriveKind::from_raw(DRIVE_FIXED).as_str(), "DRIVE_FIXED");

    assert!(DriveKind::from_raw(DRIVE_REMOVABLE).is_supported());
    assert_eq!(DriveKind::from_raw(DRIVE_REMOVABLE), DriveKind::Removable);
    assert_eq!(
        DriveKind::from_raw(DRIVE_REMOVABLE).as_str(),
        "DRIVE_REMOVABLE"
    );

    // Rejected drive types
    assert!(!DriveKind::from_raw(DRIVE_REMOTE).is_supported());
    assert_eq!(DriveKind::from_raw(DRIVE_REMOTE), DriveKind::Remote);
    assert_eq!(DriveKind::from_raw(DRIVE_REMOTE).as_str(), "DRIVE_REMOTE");

    assert!(!DriveKind::from_raw(DRIVE_CDROM).is_supported());
    assert_eq!(DriveKind::from_raw(DRIVE_CDROM), DriveKind::CdRom);
    assert_eq!(DriveKind::from_raw(DRIVE_CDROM).as_str(), "DRIVE_CDROM");

    assert!(!DriveKind::from_raw(DRIVE_RAMDISK).is_supported());
    assert_eq!(DriveKind::from_raw(DRIVE_RAMDISK), DriveKind::RamDisk);
    assert_eq!(DriveKind::from_raw(DRIVE_RAMDISK).as_str(), "DRIVE_RAMDISK");

    assert!(!DriveKind::from_raw(DRIVE_UNKNOWN).is_supported());
    assert_eq!(DriveKind::from_raw(DRIVE_UNKNOWN), DriveKind::Unknown);

    assert!(!DriveKind::from_raw(DRIVE_NO_ROOT_DIR).is_supported());
    assert_eq!(DriveKind::from_raw(DRIVE_NO_ROOT_DIR), DriveKind::NoRootDir);

    assert!(!DriveKind::from_raw(999).is_supported());
    assert_eq!(DriveKind::from_raw(999), DriveKind::Other(999));
}

#[test]
fn test_filesystem_kind_classification() {
    // Accepted filesystems (case-insensitive)
    for name in &["NTFS", "ntfs", "Ntfs", "  NTFS  "] {
        let kind = FileSystemKind::parse(name);
        assert!(kind.is_supported(), "Expected {name} to be supported");
        assert_eq!(kind, FileSystemKind::Ntfs);
        assert_eq!(kind.as_str(), "NTFS");
    }

    for name in &["ReFS", "refs", "REFS"] {
        let kind = FileSystemKind::parse(name);
        assert!(kind.is_supported(), "Expected {name} to be supported");
        assert_eq!(kind, FileSystemKind::ReFs);
        assert_eq!(kind.as_str(), "ReFS");
    }

    for name in &["FAT32", "fat32", "Fat32"] {
        let kind = FileSystemKind::parse(name);
        assert!(kind.is_supported(), "Expected {name} to be supported");
        assert_eq!(kind, FileSystemKind::Fat32);
        assert_eq!(kind.as_str(), "FAT32");
    }

    for name in &["exFAT", "exfat", "EXFAT"] {
        let kind = FileSystemKind::parse(name);
        assert!(kind.is_supported(), "Expected {name} to be supported");
        assert_eq!(kind, FileSystemKind::ExFat);
        assert_eq!(kind.as_str(), "exFAT");
    }

    // Rejected filesystems
    for name in &[
        "FAT",
        "FAT12",
        "FAT16",
        "CDFS",
        "UDF",
        "NFS",
        "SMB",
        "ext4",
        "CSVFS",
        "UnknownFS",
    ] {
        let kind = FileSystemKind::parse(name);
        assert!(!kind.is_supported(), "Expected {name} to be rejected");
        assert_eq!(kind, FileSystemKind::Unsupported(name.to_string()));
    }
}

#[test]
fn test_validate_empty_and_unc_paths() {
    // Empty path
    assert_eq!(
        validate_scan_target(""),
        Err(TargetValidationError::EmptyPath)
    );
    assert_eq!(
        validate_scan_target("   "),
        Err(TargetValidationError::EmptyPath)
    );

    // Standard UNC
    match validate_scan_target(r"\\nonexistent_server\share") {
        Err(TargetValidationError::UncPathNotSupported { path }) => {
            assert!(path.contains("nonexistent_server"));
        }
        other => panic!("Expected UncPathNotSupported, got {other:?}"),
    }

    // Extended UNC
    match validate_scan_target(r"\\?\UNC\dummy\share") {
        Err(TargetValidationError::UncPathNotSupported { path }) => {
            assert!(path.contains("dummy"));
        }
        other => panic!("Expected UncPathNotSupported, got {other:?}"),
    }
}

#[test]
fn test_validate_nonexistent_and_file_targets() {
    // Nonexistent directory
    let bad_path = r"C:\pigtree_test_nonexistent_dir_999999";
    match validate_scan_target(bad_path) {
        Err(TargetValidationError::NotFound { path }) => {
            assert_eq!(path, bad_path);
        }
        other => panic!("Expected NotFound, got {other:?}"),
    }

    // Existing file (not a directory)
    let temp_file_dir =
        std::env::temp_dir().join(format!("pigtree_file_test_{}", std::process::id()));
    fs::create_dir_all(&temp_file_dir).unwrap();
    let file_path = temp_file_dir.join("test_file.txt");
    File::create(&file_path).unwrap();

    match validate_scan_target(&file_path) {
        Err(TargetValidationError::NotADirectory { path }) => {
            assert!(path.contains("test_file.txt"));
        }
        other => panic!("Expected NotADirectory, got {other:?}"),
    }

    let _ = fs::remove_dir_all(&temp_file_dir);
}

#[test]
fn test_validate_live_temp_directory() {
    let temp_dir = std::env::temp_dir().join(format!("pigtree_val_test_{}", std::process::id()));
    fs::create_dir_all(&temp_dir).unwrap();

    let display_str = temp_dir.to_string_lossy().to_string();
    match validate_scan_target(&temp_dir) {
        Ok(validated) => {
            assert_eq!(validated.display_path, display_str);
            assert!(validated.canonical_path.exists());
            assert!(validated.extended_path.starts_with(r"\\?\"));
            assert!(validated.volume_root.ends_with(r"\"));
            assert!(validated.drive_kind.is_supported());
            assert!(validated.filesystem_kind.is_supported());
            assert!(!validated.filesystem_name.is_empty());
        }
        Err(TargetValidationError::UnsupportedFileSystem {
            filesystem_name,
            volume_root,
            ..
        }) => {
            eprintln!("Skipping live validation: volume {volume_root} filesystem {filesystem_name} is unsupported");
        }
        Err(err) => {
            panic!("Unexpected validation failure on live temp directory: {err:?}");
        }
    }

    let _ = fs::remove_dir_all(&temp_dir);
}
