// Mock Data for PigTree Duplicate Review and Remediation Prototype
// Contains 5 realistic scenarios exercising complex Windows/NTFS edge cases.

export const INITIAL_GROUPS = [
  {
    id: "group-vacation",
    name: "Vacation originals",
    category: "Photos & Media",
    logicalSizePerCopy: 4800000000, // 4.8 GB (4.47 GiB)
    formattedSize: "4.8 GB",
    volume: "C:",
    filesystem: "NTFS",
    status: "candidate", // candidate | verifying | verified | mismatch | blocked
    verificationStepIndex: 0,
    verificationMethod: "Full byte-by-byte comparison & all-stream cryptographic hash (SHA-256)",
    verificationScope: "All content-bearing Content Streams",
    lastVerified: null,
    story: "Three 4.8 GB camera RAW files discovered across Pictures, Desktop, and Downloads. Candidate discovery identified identical file sizes and initial metadata. Full verification is required before any cleanup actions unlock.",
    mismatchReason: "Stream & Access Rules mismatch detected: 'IMG_9204_RAW (1).CR3' in Downloads contains an extra 'Zone.Identifier:$DATA' stream (ZoneId=3 Internet Mark-of-the-Web) and divergent ACLs (read-only for user). The other two files have matching stream sets and ACLs.",
    objects: [
      {
        id: "obj-vac-1",
        fileId: "0x000100000002A111",
        logicalSize: 4800000000,
        allocatedSize: 4800000000,
        storageCharacteristic: "Resident / Standard Allocation",
        isCloud: false,
        isProtected: false,
        isStale: false,
        owner: "DESKTOP-ALEX\\Alex",
        accessRules: "BUILTIN\\Administrators:(F), DESKTOP-ALEX\\Alex:(F)",
        attributes: "Archive (FILE_ATTRIBUTE_ARCHIVE)",
        mtime: "2024-07-14 15:30:22",
        linkCount: 1,
        coverage: "Complete",
        directoryEntries: [
          {
            path: "C:\\Users\\Alex\\Pictures\\2024\\Vacation\\IMG_9204_RAW.CR3",
            parent: "C:\\Users\\Alex\\Pictures\\2024\\Vacation",
            name: "IMG_9204_RAW.CR3",
            isPrimary: true
          }
        ],
        streams: [
          { name: "::$DATA (Unnamed stream)", logicalSize: 4800000000, allocatedSize: 4800000000, hash: "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855" }
        ],
        streamCount: 1,
        reasons: ["Original media folder", "Full write permissions", "Complete whole-volume link coverage"],
        recommendedKeeper: true,
        excluded: false
      },
      {
        id: "obj-vac-2",
        fileId: "0x000100000002A112",
        logicalSize: 4800000000,
        allocatedSize: 4800000000,
        storageCharacteristic: "Resident / Standard Allocation",
        isCloud: false,
        isProtected: false,
        isStale: false,
        owner: "DESKTOP-ALEX\\Alex",
        accessRules: "BUILTIN\\Administrators:(F), DESKTOP-ALEX\\Alex:(F)",
        attributes: "Archive (FILE_ATTRIBUTE_ARCHIVE)",
        mtime: "2024-07-14 15:30:22",
        linkCount: 1,
        coverage: "Complete",
        directoryEntries: [
          {
            path: "C:\\Users\\Alex\\Desktop\\Vacation_Export\\IMG_9204_RAW.CR3",
            parent: "C:\\Users\\Alex\\Desktop\\Vacation_Export",
            name: "IMG_9204_RAW.CR3",
            isPrimary: false
          }
        ],
        streams: [
          { name: "::$DATA (Unnamed stream)", logicalSize: 4800000000, allocatedSize: 4800000000, hash: "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855" }
        ],
        streamCount: 1,
        reasons: ["Export staging directory", "Identical stream set and security to primary"],
        recommendedKeeper: false,
        excluded: false
      },
      {
        id: "obj-vac-3",
        fileId: "0x000100000002A113",
        logicalSize: 4800000248, // 4.8 GB + 248 bytes
        allocatedSize: 4800004096,
        storageCharacteristic: "Resident / Standard Allocation (Named stream present)",
        isCloud: false,
        isProtected: false,
        isStale: false,
        owner: "DESKTOP-ALEX\\Alex",
        accessRules: "BUILTIN\\Administrators:(F), DESKTOP-ALEX\\Alex:(R), Everyone:(R)", // Divergent ACL
        attributes: "Archive (FILE_ATTRIBUTE_ARCHIVE)",
        mtime: "2024-07-14 15:30:22",
        linkCount: 1,
        coverage: "Complete",
        directoryEntries: [
          {
            path: "C:\\Users\\Alex\\Downloads\\IMG_9204_RAW (1).CR3",
            parent: "C:\\Users\\Alex\\Downloads",
            name: "IMG_9204_RAW (1).CR3",
            isPrimary: false
          }
        ],
        streams: [
          { name: "::$DATA (Unnamed stream)", logicalSize: 4800000000, allocatedSize: 4800000000, hash: "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855" },
          { name: "Zone.Identifier:$DATA (Named Stream)", logicalSize: 248, allocatedSize: 4096, hash: "sha256:7f83b1657ff1fc53b92dc18148a1d65dfc2d4b1fa3d677284addd200126d9069" }
        ],
        streamCount: 2,
        reasons: ["Browser download copy", "Has Zone.Identifier stream (Zone 3)", "Restricted ACLs"],
        recommendedKeeper: false,
        excluded: false,
        mismatchDetails: {
          divergentStreams: ["Zone.Identifier:$DATA (248 bytes)"],
          divergentAcl: "Alex has Read-only (expected Full Control); Everyone has Read",
          actionBlockReason: "Hard link replacement prohibited: Content stream count and security descriptors do not match candidate keeper."
        }
      }
    ]
  },
  {
    id: "group-build",
    name: "Build artifacts",
    category: "Development",
    logicalSizePerCopy: 1200000000, // 1.2 GB
    formattedSize: "1.2 GB",
    volume: "C:",
    filesystem: "NTFS",
    status: "verified", // pre-verified
    verificationStepIndex: 4,
    verificationMethod: "Full byte-by-byte comparison & all-stream cryptographic hash (SHA-256)",
    verificationScope: "All content-bearing Content Streams",
    lastVerified: "Today 10:43",
    story: "Four distinct 1.2 GB library objects across developer build, test, and archive locations. Note: 'build/output' has an existing Hard Link alias in 'dist/bin', which represents the SAME physical object (linkCount: 2) and is counted once. Whole-volume link coverage is complete.",
    mismatchReason: null,
    objects: [
      {
        id: "obj-build-1",
        fileId: "0x000200000003B201",
        logicalSize: 1200000000,
        allocatedSize: 1200000000,
        storageCharacteristic: "Resident / Hard Linked (2 directory entries)",
        isCloud: false,
        isProtected: false,
        isStale: false,
        owner: "DESKTOP-ALEX\\Alex",
        accessRules: "BUILTIN\\Administrators:(F), DESKTOP-ALEX\\Alex:(F)",
        attributes: "Archive (FILE_ATTRIBUTE_ARCHIVE)",
        mtime: "2025-01-10 09:12:00",
        linkCount: 2, // 2 links to SAME object
        coverage: "Complete (whole-volume verified)",
        directoryEntries: [
          {
            path: "C:\\Dev\\Engine\\build\\output\\x64\\release\\engine_core.lib",
            parent: "C:\\Dev\\Engine\\build\\output\\x64\\release",
            name: "engine_core.lib",
            isPrimary: true
          },
          {
            path: "C:\\Dev\\Engine\\dist\\bin\\engine_core.lib",
            parent: "C:\\Dev\\Engine\\dist\\bin",
            name: "engine_core.lib",
            isPrimary: false,
            isAlias: true
          }
        ],
        streams: [
          { name: "::$DATA (Unnamed stream)", logicalSize: 1200000000, allocatedSize: 1200000000, hash: "sha256:a1b2c3d4e5f60718293a4b5c6d7e8f90123456789abcdef0123456789abcdef0" }
        ],
        streamCount: 1,
        reasons: ["Build system output directory", "Already shared via Hard Link with dist/bin", "Primary active reference"],
        recommendedKeeper: true,
        excluded: false
      },
      {
        id: "obj-build-2",
        fileId: "0x000200000003B202",
        logicalSize: 1200000000,
        allocatedSize: 1200000000,
        storageCharacteristic: "Resident / Standard Allocation",
        isCloud: false,
        isProtected: false,
        isStale: false,
        owner: "DESKTOP-ALEX\\Alex",
        accessRules: "BUILTIN\\Administrators:(F), DESKTOP-ALEX\\Alex:(F)",
        attributes: "Archive (FILE_ATTRIBUTE_ARCHIVE)",
        mtime: "2025-01-10 09:12:00",
        linkCount: 1,
        coverage: "Complete (whole-volume verified)",
        directoryEntries: [
          {
            path: "C:\\Dev\\Engine\\backup_libs\\engine_core.lib",
            parent: "C:\\Dev\\Engine\\backup_libs",
            name: "engine_core.lib",
            isPrimary: false
          }
        ],
        streams: [
          { name: "::$DATA (Unnamed stream)", logicalSize: 1200000000, allocatedSize: 1200000000, hash: "sha256:a1b2c3d4e5f60718293a4b5c6d7e8f90123456789abcdef0123456789abcdef0" }
        ],
        streamCount: 1,
        reasons: ["Manual backup copy", "Eligible for hard link consolidation or recycling"],
        recommendedKeeper: false,
        excluded: false
      },
      {
        id: "obj-build-3",
        fileId: "0x000200000003B203",
        logicalSize: 1200000000,
        allocatedSize: 1200000000,
        storageCharacteristic: "Resident / Standard Allocation",
        isCloud: false,
        isProtected: false,
        isStale: false,
        owner: "DESKTOP-ALEX\\Alex",
        accessRules: "BUILTIN\\Administrators:(F), DESKTOP-ALEX\\Alex:(F)",
        attributes: "Archive (FILE_ATTRIBUTE_ARCHIVE)",
        mtime: "2025-01-10 09:12:00",
        linkCount: 1,
        coverage: "Complete (whole-volume verified)",
        directoryEntries: [
          {
            path: "C:\\Dev\\Tools\\shared\\engine_core.lib",
            parent: "C:\\Dev\\Tools\\shared",
            name: "engine_core.lib",
            isPrimary: false
          }
        ],
        streams: [
          { name: "::$DATA (Unnamed stream)", logicalSize: 1200000000, allocatedSize: 1200000000, hash: "sha256:a1b2c3d4e5f60718293a4b5c6d7e8f90123456789abcdef0123456789abcdef0" }
        ],
        streamCount: 1,
        reasons: ["Tools dependency copy", "Eligible for hard link consolidation"],
        recommendedKeeper: false,
        excluded: false
      },
      {
        id: "obj-build-4",
        fileId: "0x000200000003B204",
        logicalSize: 1200000000,
        allocatedSize: 1200000000,
        storageCharacteristic: "Resident / Standard Allocation",
        isCloud: false,
        isProtected: false,
        isStale: false,
        owner: "DESKTOP-ALEX\\Alex",
        accessRules: "BUILTIN\\Administrators:(F), DESKTOP-ALEX\\Alex:(F)",
        attributes: "Archive (FILE_ATTRIBUTE_ARCHIVE)",
        mtime: "2025-01-10 09:12:00",
        linkCount: 1,
        coverage: "Complete (whole-volume verified)",
        directoryEntries: [
          {
            path: "C:\\Dev\\Archive\\v2.4\\engine_core.lib",
            parent: "C:\\Dev\\Archive\\v2.4",
            name: "engine_core.lib",
            isPrimary: false
          }
        ],
        streams: [
          { name: "::$DATA (Unnamed stream)", logicalSize: 1200000000, allocatedSize: 1200000000, hash: "sha256:a1b2c3d4e5f60718293a4b5c6d7e8f90123456789abcdef0123456789abcdef0" }
        ],
        streamCount: 1,
        reasons: ["Old release snapshot", "Eligible for hard link consolidation"],
        recommendedKeeper: false,
        excluded: false
      }
    ]
  },
  {
    id: "group-onedrive",
    name: "OneDrive project archive",
    category: "Cloud & Synchronization",
    logicalSizePerCopy: 7400000000, // 7.4 GB
    formattedSize: "7.4 GB",
    volume: "C:",
    filesystem: "NTFS",
    status: "candidate", // verification blocked on cloud hydration consent
    verificationStepIndex: 0,
    verificationMethod: "Full byte-by-byte comparison & all-stream cryptographic hash (SHA-256)",
    verificationScope: "All content-bearing Content Streams",
    lastVerified: null,
    story: "Two candidate copies of a 7.4 GB project archive. One is locally resident (Allocated Size 7.4 GB); the other is an online-only OneDrive placeholder (Allocated Size 0 bytes). Verification requires explicit hydration consent. Direct file mutations on cloud items are protected and routed to provider handoff.",
    mismatchReason: null,
    cloudHydrationConsentGiven: false,
    hydrationRequirements: {
      estimatedDownloadBytes: 7400000000,
      estimatedAllocationBytes: 7400000000,
      formattedDownload: "7.4 GB",
      provider: "Microsoft OneDrive (Personal)",
      warning: "Hydrating this file will download 7.4 GB over your active network connection and allocate 7.4 GB on drive C:. Declining consent leaves the item unverified. Direct mutation in PigTree is protected; cloud operations must be handled via OneDrive or Windows Explorer."
    },
    objects: [
      {
        id: "obj-cloud-1",
        fileId: "0x000300000004C301",
        logicalSize: 7400000000,
        allocatedSize: 7400000000,
        storageCharacteristic: "Resident / Standard Allocation",
        isCloud: false,
        isProtected: false,
        isStale: false,
        owner: "DESKTOP-ALEX\\Alex",
        accessRules: "BUILTIN\\Administrators:(F), DESKTOP-ALEX\\Alex:(F)",
        attributes: "Archive (FILE_ATTRIBUTE_ARCHIVE)",
        mtime: "2023-11-20 14:00:10",
        linkCount: 1,
        coverage: "Complete",
        directoryEntries: [
          {
            path: "C:\\Users\\Alex\\Projects\\Archived\\2023_RenderProject.zip",
            parent: "C:\\Users\\Alex\\Projects\\Archived",
            name: "2023_RenderProject.zip",
            isPrimary: true
          }
        ],
        streams: [
          { name: "::$DATA (Unnamed stream)", logicalSize: 7400000000, allocatedSize: 7400000000, hash: "sha256:9f8e7d6c5b4a39281701f0e1d2c3b4a596874839201a0b1c2d3e4f5061728394" }
        ],
        streamCount: 1,
        reasons: ["Local physical copy on SSD", "Standard NTFS allocation"],
        recommendedKeeper: true,
        excluded: false
      },
      {
        id: "obj-cloud-2",
        fileId: "0x000300000004C302",
        logicalSize: 7400000000,
        allocatedSize: 0, // 0 local bytes!
        storageCharacteristic: "Online-Only / Reparse Point / Cloud-Managed (IO_REPARSE_TAG_CLOUD)",
        isCloud: true,
        cloudStatus: "Online-only placeholder (Pinned: No)",
        isProtected: true, // protected Action Risk Class
        protectionReason: "Cloud-managed storage provider (OneDrive). Direct PigTree mutation or hard linking is prohibited in v1. Deleting placeholder releases 0 bytes of local disk space.",
        isStale: false,
        owner: "DESKTOP-ALEX\\Alex",
        accessRules: "BUILTIN\\Administrators:(F), DESKTOP-ALEX\\Alex:(F)",
        attributes: "Reparse Point, Sparse, Recall on Data Access (FILE_ATTRIBUTE_REPARSE_POINT | FILE_ATTRIBUTE_OFFLINE)",
        mtime: "2023-11-20 14:00:10",
        linkCount: 1,
        coverage: "Metadata Only (Content unhydrated)",
        directoryEntries: [
          {
            path: "C:\\Users\\Alex\\OneDrive\\Projects\\2023_RenderProject.zip",
            parent: "C:\\Users\\Alex\\OneDrive\\Projects",
            name: "2023_RenderProject.zip",
            isPrimary: false
          }
        ],
        streams: [
          { name: "::$DATA (Cloud Placeholder)", logicalSize: 7400000000, allocatedSize: 0, hash: "unverified (hydration required)" }
        ],
        streamCount: 1,
        reasons: ["OneDrive sync folder", "Reparse Point tag IO_REPARSE_TAG_CLOUD", "Allocated size: 0 bytes"],
        recommendedKeeper: false,
        excluded: false,
        handoffInfo: {
          toolName: "Microsoft OneDrive Sync Client / Windows Explorer",
          targetPath: "C:\\Users\\Alex\\OneDrive\\Projects\\2023_RenderProject.zip",
          instructions: "Manage cloud files through OneDrive context menu ('Free up space' or 'Always keep on this device') or OneDrive Settings.",
          rescanExpectation: "After managing cloud files in OneDrive or File Explorer, run a fresh PigTree scan to refresh cloud placeholder and hydration status."
        }
      }
    ]
  },
  {
    id: "group-installer",
    name: "Installer cache lookalikes",
    category: "System & Applications",
    logicalSizePerCopy: 650000000, // 650 MB
    formattedSize: "650 MB",
    volume: "C:",
    filesystem: "NTFS",
    status: "verified", // verified metadata/content
    verificationStepIndex: 4,
    verificationMethod: "Full byte-by-byte comparison & all-stream cryptographic hash (SHA-256)",
    verificationScope: "All content-bearing Content Streams",
    lastVerified: "Today 10:44",
    story: "Two identical 650 MB installer SDK files located in protected Windows package store and system cache locations. Action Risk Class is 'protected'. Direct deletion or hard-linking is blocked; actions are routed to Windows Native System Handoffs. Never label safe to delete.",
    mismatchReason: null,
    objects: [
      {
        id: "obj-inst-1",
        fileId: "0x000400000005D401",
        logicalSize: 650000000,
        allocatedSize: 650000000,
        storageCharacteristic: "Protected System Package (WindowsApps)",
        isCloud: false,
        isProtected: true,
        protectionReason: "Windows AppX/MSIX Package Store (TrustedInstaller-owned). Direct deletion breaks package signing and Windows Store integrity.",
        isStale: false,
        owner: "NT SERVICE\\TrustedInstaller",
        accessRules: "NT SERVICE\\TrustedInstaller:(F), BUILTIN\\Administrators:(RX), ALL APPLICATION PACKAGES:(RX)",
        attributes: "Read-only, System (FILE_ATTRIBUTE_READONLY | FILE_ATTRIBUTE_SYSTEM)",
        mtime: "2024-05-18 11:22:45",
        linkCount: 1,
        coverage: "Complete",
        directoryEntries: [
          {
            path: "C:\\Program Files\\WindowsApps\\Microsoft.DesktopAppInstaller_1.22.11261.0_x64__8wekyb3d8bbwe\\AppInstallerSDK.dll",
            parent: "C:\\Program Files\\WindowsApps\\Microsoft.DesktopAppInstaller_1.22.11261.0_x64__8wekyb3d8bbwe",
            name: "AppInstallerSDK.dll",
            isPrimary: true
          }
        ],
        streams: [
          { name: "::$DATA (Unnamed stream)", logicalSize: 650000000, allocatedSize: 650000000, hash: "sha256:4b227777d4dd1fc61c6f884f48641d02b4d121d3fd328cb08b5531fcacdabf8a" }
        ],
        streamCount: 1,
        reasons: ["Windows Package Store", "TrustedInstaller ownership", "System protected"],
        recommendedKeeper: true,
        excluded: false,
        handoffInfo: {
          toolName: "Windows Settings > Installed Apps",
          instructions: "To remove or modify modern Windows packages, uninstall or repair them through Windows Settings (Apps > Installed apps) or PowerShell AppX package management cmdlets.",
          rescanExpectation: "After modifying system packages through Windows Settings, perform a fresh PigTree scan to refresh duplicate analysis."
        }
      },
      {
        id: "obj-inst-2",
        fileId: "0x000400000005D402",
        logicalSize: 650000000,
        allocatedSize: 650000000,
        storageCharacteristic: "System Temp Staging Cache",
        isCloud: false,
        isProtected: true,
        protectionReason: "Windows System Temp Cache. Managed by Windows Storage Sense and Disk Cleanup. Direct PigTree mutation is protected.",
        isStale: false,
        owner: "NT AUTHORITY\\SYSTEM",
        accessRules: "NT AUTHORITY\\SYSTEM:(F), BUILTIN\\Administrators:(F)",
        attributes: "Archive, System (FILE_ATTRIBUTE_SYSTEM)",
        mtime: "2024-05-18 11:22:45",
        linkCount: 1,
        coverage: "Complete",
        directoryEntries: [
          {
            path: "C:\\Windows\\SystemTemp\\AppPackages\\AppInstallerSDK.dll",
            parent: "C:\\Windows\\SystemTemp\\AppPackages",
            name: "AppInstallerSDK.dll",
            isPrimary: false
          }
        ],
        streams: [
          { name: "::$DATA (Unnamed stream)", logicalSize: 650000000, allocatedSize: 650000000, hash: "sha256:4b227777d4dd1fc61c6f884f48641d02b4d121d3fd328cb08b5531fcacdabf8a" }
        ],
        streamCount: 1,
        reasons: ["System temp staging directory", "Protected system location"],
        recommendedKeeper: false,
        excluded: false,
        handoffInfo: {
          toolName: "Windows Storage Sense / Disk Cleanup",
          instructions: "Manage system temporary files and staging caches using Windows Storage Sense (Settings > System > Storage) or Disk Cleanup.",
          rescanExpectation: "After running Storage Sense or Disk Cleanup, perform a fresh PigTree scan to verify released disk space."
        }
      }
    ]
  },
  {
    id: "group-stale",
    name: "Changed since scan",
    category: "Documents & Reports",
    logicalSizePerCopy: 900000000, // 900 MB
    formattedSize: "900 MB",
    volume: "C:",
    filesystem: "NTFS",
    status: "stale_error", // Stale live preflight error
    verificationStepIndex: 4,
    verificationMethod: "Full byte-by-byte comparison & all-stream cryptographic hash (SHA-256)",
    verificationScope: "All content-bearing Content Streams",
    lastVerified: "Today 10:40 (Invalidated)",
    story: "A 900 MB report pair that was verified during earlier analysis. Targeted live re-observation detected that the Downloads copy was modified (File ID changed, timestamp updated, size expanded to 912 MB). Proposed actions are strictly prohibited and cannot be overridden in place. A fresh scan is required.",
    mismatchReason: "Live Preflight Invalidation: Target 'Downloads\\Annual_2025_Final.pdf' changed since Analysis Snapshot. File ID changed from 0x000500000006E502 to 0x000500000006E999, modified time updated +2m, size changed to 912 MB.",
    objects: [
      {
        id: "obj-stale-1",
        fileId: "0x000500000006E501",
        logicalSize: 900000000,
        allocatedSize: 900000000,
        storageCharacteristic: "Resident / Standard Allocation",
        isCloud: false,
        isProtected: false,
        isStale: false,
        owner: "DESKTOP-ALEX\\Alex",
        accessRules: "BUILTIN\\Administrators:(F), DESKTOP-ALEX\\Alex:(F)",
        attributes: "Archive (FILE_ATTRIBUTE_ARCHIVE)",
        mtime: "2025-02-14 16:20:00",
        linkCount: 1,
        coverage: "Complete",
        directoryEntries: [
          {
            path: "C:\\Users\\Alex\\Documents\\Reports\\Annual_2025_Final.pdf",
            parent: "C:\\Users\\Alex\\Documents\\Reports",
            name: "Annual_2025_Final.pdf",
            isPrimary: true
          }
        ],
        streams: [
          { name: "::$DATA (Unnamed stream)", logicalSize: 900000000, allocatedSize: 900000000, hash: "sha256:5566778899aabbccddeeff00112233445566778899aabbccddeeff0011223344" }
        ],
        streamCount: 1,
        reasons: ["Documents repository", "Unmodified"],
        recommendedKeeper: true,
        excluded: false
      },
      {
        id: "obj-stale-2",
        fileId: "0x000500000006E999", // Divergent File ID!
        logicalSize: 912000000, // Divergent Size!
        allocatedSize: 912000000,
        storageCharacteristic: "Resident / Standard Allocation (Modified live)",
        isCloud: false,
        isProtected: false,
        isStale: true, // Stale / Changed
        staleReason: "File ID, modified timestamp, and size diverge from Analysis Snapshot (912 MB vs 900 MB snapshot). In-place mutation prohibited by ADR 0002.",
        owner: "DESKTOP-ALEX\\Alex",
        accessRules: "BUILTIN\\Administrators:(F), DESKTOP-ALEX\\Alex:(F)",
        attributes: "Archive (FILE_ATTRIBUTE_ARCHIVE)",
        mtime: "2025-02-14 16:22:15", // Changed mtime
        linkCount: 1,
        coverage: "Diverged since scan",
        directoryEntries: [
          {
            path: "C:\\Users\\Alex\\Downloads\\Annual_2025_Final.pdf",
            parent: "C:\\Users\\Alex\\Downloads",
            name: "Annual_2025_Final.pdf",
            isPrimary: false
          }
        ],
        streams: [
          { name: "::$DATA (Unnamed stream)", logicalSize: 912000000, allocatedSize: 912000000, hash: "sha256:changed999888777666555444333222111000fff" }
        ],
        streamCount: 1,
        reasons: ["Modified after snapshot", "Divergent content & identity"],
        recommendedKeeper: false,
        excluded: false
      }
    ]
  }
];

export const VERIFICATION_STAGES = [
  { id: "preflight", name: "Preflight identities & links", detail: "Safely open target Directory Entries, check Volume IDs, File Reference Numbers, link counts, and repelling Reparse Points without following links." },
  { id: "streams", name: "Enumerate content-bearing streams", detail: "Inspect unnamed data stream and all named Alternate Data Streams (Zone.Identifier, security descriptors, metadata streams)." },
  { id: "hash", name: "Read, hash & compare full bytes", detail: "Stream full byte payload through SHA-256 cryptographic digest and perform byte-by-byte cross-comparison. (No partial sampling)." },
  { id: "recheck", name: "Recheck identity & live preconditions", detail: "Re-evaluate live File IDs, link counts, and parent directories to ensure targets have not been replaced in flight." },
  { id: "settled", name: "Verification Settlement", detail: "Outcome recorded with Verification Method, Scope, and timestamp." }
];

export const ACTION_TYPES = {
  RETAIN: {
    id: "retain",
    label: "Retain copy",
    shortDescription: "Keep this directory entry and physical object intact without modification.",
    recoveryClass: "none",
    riskClass: "routine",
    requiresConfirmation: false
  },
  RECYCLE: {
    id: "recycle",
    label: "Move to Recycle Bin",
    shortDescription: "Send entry to Windows Recycle Bin via platform IFileOperation. Conditional future reclaim.",
    recoveryClass: "conditional",
    riskClass: "routine",
    requiresConfirmation: true
  },
  PERMANENT_DELETE: {
    id: "permanent_delete",
    label: "Permanently delete",
    shortDescription: "Direct Win32 entry deletion without recycling. Immediate reclaim. Irreversible.",
    recoveryClass: "permanent",
    riskClass: "caution",
    requiresConfirmation: true,
    requiresTypedChallenge: true
  },
  HARDLINK_RECOVERABLE: {
    id: "hardlink_recoverable",
    label: "Hard Link (Recoverable)",
    shortDescription: "Preserve victim object in PigTree recovery vault, then point entry to keeper. 0 immediate reclaim.",
    recoveryClass: "retained",
    riskClass: "routine",
    requiresConfirmation: true
  },
  HARDLINK_IMMEDIATE: {
    id: "hardlink_immediate",
    label: "Hard Link (Immediate reclaim)",
    shortDescription: "Point entry to keeper and immediately purge staging link after verification. Immediate reclaim. Irreversible.",
    recoveryClass: "permanent",
    riskClass: "caution",
    requiresConfirmation: true
  },
  NATIVE_HANDOFF: {
    id: "native_handoff",
    label: "Native System Handoff",
    shortDescription: "Open Windows Settings, Storage Sense, or Disk Cleanup to manage protected system resources.",
    recoveryClass: "handoff",
    riskClass: "protected",
    requiresConfirmation: false
  },
  PROVIDER_HANDOFF: {
    id: "provider_handoff",
    label: "Cloud Provider Handoff",
    shortDescription: "Handoff to OneDrive / Windows Explorer context menu to manage cloud placeholders safely.",
    recoveryClass: "handoff",
    riskClass: "protected",
    requiresConfirmation: false
  }
};
