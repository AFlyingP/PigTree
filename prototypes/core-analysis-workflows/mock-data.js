// PigTree Prototype Mock Data & Domain Definitions
// Conforms to CONTEXT.md domain specifications

export const PRESETS = [
  { id: 'standard', name: 'Default Standard Scan', desc: 'Standard filesystem metadata, stream sizes, and known reparse points.', profile: 'Standard' },
  { id: 'fast', name: 'Fast Overview Scan', desc: 'Top-level directory and large file allocation bounds.', profile: 'Fast' },
  { id: 'developer', name: 'Developer Workspace', desc: 'Inspect build outputs, caches, node_modules, and git object stores.', profile: 'Developer' },
  { id: 'media_large', name: 'Media & Large Objects (>100MB)', desc: 'Prioritize files and archives over 100 MB.', profile: 'LargeObjects' }
];

export const MOCK_VOLUMES = [
  {
    id: 'vol_c',
    label: 'Local Disk (C:)',
    mountPath: 'C:\\',
    filesystem: 'NTFS',
    capacityBytes: 512 * 1024 * 1024 * 1024, // 512 GB
    freeBytes: 120 * 1024 * 1024 * 1024,     // 120 GB
    usedBytes: 392 * 1024 * 1024 * 1024,     // 392 GB
    accountedUniqueBytes: 368 * 1024 * 1024 * 1024, // 368 GB
    unattributedUsedBytes: 24 * 1024 * 1024 * 1024, // 24 GB
    overAccountedBytes: 0,
    coverage: 'partial',
    runOutcome: 'finished',
    observationInterval: {
      startedAt: '2025-04-10T14:20:00Z',
      completedAt: '2025-04-10T14:20:42Z',
      durationSeconds: 42
    },
    coverageGaps: [
      {
        path: 'C:\\System Volume Information',
        reason: 'Access denied under current security context (STATUS_ACCESS_DENIED)',
        defensibleBound: 'Unknown allocation; volume shadow copies & system restore points typically reside here.',
        noncommittalPrompt: 'Additional security privileges or backup-intent read may reveal more metadata.'
      }
    ]
  },
  {
    id: 'vol_d',
    label: 'Data Volume (D:)',
    mountPath: 'D:\\',
    filesystem: 'ReFS',
    capacityBytes: 1024 * 1024 * 1024 * 1024, // 1 TB
    freeBytes: 450 * 1024 * 1024 * 1024,      // 450 GB
    usedBytes: 574 * 1024 * 1024 * 1024,      // 574 GB
    accountedUniqueBytes: 570 * 1024 * 1024 * 1024,
    unattributedUsedBytes: 4 * 1024 * 1024 * 1024,
    overAccountedBytes: 0,
    coverage: 'complete',
    runOutcome: 'finished',
    observationInterval: {
      startedAt: '2025-04-09T10:00:00Z',
      completedAt: '2025-04-09T10:01:15Z',
      durationSeconds: 75
    },
    coverageGaps: []
  }
];

export const HISTORICAL_SNAPSHOTS = [
  {
    id: 'snap_c_prev_month',
    name: 'C:\ Snapshot (14 days ago - March 27, 2025)',
    targetType: 'volume',
    targetPath: 'C:\\',
    volumeId: 'vol_c',
    recordedAt: '2025-03-27T09:15:00Z',
    runOutcome: 'finished',
    coverage: 'partial',
    profileName: 'Standard',
    note: 'Historical snapshot: facts observed during the observation interval, not live system state.'
  },
  {
    id: 'snap_downloads_archive',
    name: 'Downloads Folder Snapshot (February 10, 2025)',
    targetType: 'directory',
    targetPath: 'C:\\Users\\Alex\\Downloads',
    volumeId: 'vol_c',
    recordedAt: '2025-02-10T16:00:00Z',
    runOutcome: 'finished',
    coverage: 'complete',
    profileName: 'Standard',
    note: 'Historical directory snapshot; external reference uncertainties apply.'
  }
];

// In-memory file system tree and object records
// Distinguishes Directory Entry from Filesystem Object
export const MOCK_OBJECTS = {
  'obj_root_c': {
    id: 'obj_root_c',
    kind: 'directory',
    logicalBytes: 375 * 1024 * 1024 * 1024,
    allocatedBytes: 368 * 1024 * 1024 * 1024,
    owner: 'NT SERVICE\\TrustedInstaller',
    accessRules: 'FullControl (SYSTEM, Administrators)',
    storageCharacteristics: ['standard'],
    linksCount: 1,
    streams: [{ name: '::$DATA', logicalBytes: 0, allocatedBytes: 0, status: 'observed' }]
  },
  'obj_shell32': {
    id: 'obj_shell32',
    kind: 'file',
    logicalBytes: 14.2 * 1024 * 1024,
    allocatedBytes: 14.2 * 1024 * 1024,
    owner: 'NT SERVICE\\TrustedInstaller',
    accessRules: 'ReadAndExecute (Users), FullControl (TrustedInstaller)',
    storageCharacteristics: ['hardlinked'],
    linksCount: 2, // Hard link in System32 AND WinSxS
    streams: [{ name: '::$DATA', logicalBytes: 14.2 * 1024 * 1024, allocatedBytes: 14.2 * 1024 * 1024, status: 'observed' }]
  },
  'obj_ntoskrnl': {
    id: 'obj_ntoskrnl',
    kind: 'file',
    logicalBytes: 11.8 * 1024 * 1024,
    allocatedBytes: 11.8 * 1024 * 1024,
    owner: 'NT SERVICE\\TrustedInstaller',
    accessRules: 'ReadAndExecute (Users)',
    storageCharacteristics: ['standard'],
    linksCount: 1,
    streams: [{ name: '::$DATA', logicalBytes: 11.8 * 1024 * 1024, allocatedBytes: 11.8 * 1024 * 1024, status: 'observed' }]
  },
  'obj_onedrive_zip': {
    id: 'obj_onedrive_zip',
    kind: 'file',
    logicalBytes: 4.8 * 1024 * 1024 * 1024, // 4.8 GB logical
    allocatedBytes: 0,                     // 0 bytes allocated on physical disk!
    owner: 'DESKTOP-PIG\\Alex',
    accessRules: 'FullControl (Alex)',
    storageCharacteristics: ['online-only', 'reparse-point', 'sparse'],
    reparseTag: 'IO_REPARSE_TAG_CLOUD_FILES (OneDrive Cloud File)',
    linksCount: 1,
    streams: [{ name: '::$DATA', logicalBytes: 4.8 * 1024 * 1024 * 1024, allocatedBytes: 0, status: 'observed' }]
  },
  'obj_ubuntu_iso': {
    id: 'obj_ubuntu_iso',
    kind: 'file',
    logicalBytes: 5.8 * 1024 * 1024 * 1024,
    allocatedBytes: 5.8 * 1024 * 1024 * 1024,
    owner: 'DESKTOP-PIG\\Alex',
    accessRules: 'FullControl (Alex)',
    storageCharacteristics: ['standard'],
    linksCount: 1,
    streams: [{ name: '::$DATA', logicalBytes: 5.8 * 1024 * 1024 * 1024, allocatedBytes: 5.8 * 1024 * 1024 * 1024, status: 'observed' }]
  },
  'obj_win11_iso': {
    id: 'obj_win11_iso',
    kind: 'file',
    logicalBytes: 6.2 * 1024 * 1024 * 1024,
    allocatedBytes: 6.2 * 1024 * 1024 * 1024,
    owner: 'DESKTOP-PIG\\Alex',
    accessRules: 'FullControl (Alex)',
    storageCharacteristics: ['standard'],
    linksCount: 1,
    streams: [{ name: '::$DATA', logicalBytes: 6.2 * 1024 * 1024 * 1024, allocatedBytes: 6.2 * 1024 * 1024 * 1024, status: 'observed' }]
  },
  'obj_starfall_pak1': {
    id: 'obj_starfall_pak1',
    kind: 'file',
    logicalBytes: 38.2 * 1024 * 1024 * 1024,
    allocatedBytes: 38.2 * 1024 * 1024 * 1024,
    owner: 'DESKTOP-PIG\\Alex',
    accessRules: 'FullControl (Alex)',
    storageCharacteristics: ['standard'],
    linksCount: 1,
    streams: [{ name: '::$DATA', logicalBytes: 38.2 * 1024 * 1024 * 1024, allocatedBytes: 38.2 * 1024 * 1024 * 1024, status: 'observed' }]
  },
  'obj_starfall_pak2': {
    id: 'obj_starfall_pak2',
    kind: 'file',
    logicalBytes: 18.4 * 1024 * 1024 * 1024,
    allocatedBytes: 18.4 * 1024 * 1024 * 1024,
    owner: 'DESKTOP-PIG\\Alex',
    accessRules: 'FullControl (Alex)',
    storageCharacteristics: ['standard'],
    linksCount: 1,
    streams: [{ name: '::$DATA', logicalBytes: 18.4 * 1024 * 1024 * 1024, allocatedBytes: 18.4 * 1024 * 1024 * 1024, status: 'observed' }]
  },
  'obj_pagefile': {
    id: 'obj_pagefile',
    kind: 'special',
    specialRole: 'Windows Paging File',
    logicalBytes: 16.0 * 1024 * 1024 * 1024,
    allocatedBytes: 16.0 * 1024 * 1024 * 1024,
    owner: 'NT AUTHORITY\\SYSTEM',
    accessRules: 'Exclusive System Access',
    storageCharacteristics: ['system-critical', 'locked'],
    linksCount: 1,
    streams: [{ name: '::$DATA', logicalBytes: 16.0 * 1024 * 1024 * 1024, allocatedBytes: 16.0 * 1024 * 1024 * 1024, status: 'observed' }]
  },
  'obj_hiberfil': {
    id: 'obj_hiberfil',
    kind: 'special',
    specialRole: 'Windows Hibernation File',
    logicalBytes: 12.8 * 1024 * 1024 * 1024,
    allocatedBytes: 12.8 * 1024 * 1024 * 1024,
    owner: 'NT AUTHORITY\\SYSTEM',
    accessRules: 'Exclusive System Access',
    storageCharacteristics: ['system-critical', 'locked'],
    linksCount: 1,
    streams: [{ name: '::$DATA', logicalBytes: 12.8 * 1024 * 1024 * 1024, allocatedBytes: 12.8 * 1024 * 1024 * 1024, status: 'observed' }]
  }
};

// Tree nodes representing Directory Entries in C:\
export const MOCK_TREE_ROOT = {
  id: 'node_root',
  name: 'C:\\',
  path: 'C:\\',
  kind: 'directory',
  objectId: 'obj_root_c',
  entryCount: 48210,
  uniqueObjectCount: 44102,
  referencedAllocatedBytes: 382.4 * 1024 * 1024 * 1024, // Notice referenced is larger than unique due to hardlinks!
  uniqueAllocatedBytes: 368.0 * 1024 * 1024 * 1024,
  referencedLogicalBytes: 389.1 * 1024 * 1024 * 1024,
  uniqueLogicalBytes: 375.0 * 1024 * 1024 * 1024,
  observationStatus: 'observed',
  coverage: 'partial', // partial because of System Volume Information
  coverageGapsCount: 1,
  modifiedTime: '2025-04-10T14:00:00Z',
  category: 'Root Directory',
  children: [
    {
      id: 'node_games',
      name: 'Games',
      path: 'C:\\Games',
      kind: 'directory',
      entryCount: 12850,
      uniqueObjectCount: 12850,
      referencedAllocatedBytes: 68.6 * 1024 * 1024 * 1024,
      uniqueAllocatedBytes: 68.6 * 1024 * 1024 * 1024,
      referencedLogicalBytes: 68.4 * 1024 * 1024 * 1024,
      uniqueLogicalBytes: 68.4 * 1024 * 1024 * 1024,
      observationStatus: 'observed',
      coverage: 'complete',
      modifiedTime: '2025-04-08T19:30:00Z',
      category: 'Games & Apps',
      cleanupSafe: 'native_uninstall',
      children: [
        {
          id: 'node_starfall',
          name: 'Starfall',
          path: 'C:\\Games\\Starfall',
          kind: 'directory',
          entryCount: 8400,
          uniqueObjectCount: 8400,
          referencedAllocatedBytes: 64.6 * 1024 * 1024 * 1024,
          uniqueAllocatedBytes: 64.6 * 1024 * 1024 * 1024,
          referencedLogicalBytes: 64.5 * 1024 * 1024 * 1024,
          uniqueLogicalBytes: 64.5 * 1024 * 1024 * 1024,
          observationStatus: 'observed',
          coverage: 'complete',
          modifiedTime: '2025-04-08T19:30:00Z',
          category: 'Game Directory',
          cleanupSafe: 'native_uninstall',
          children: [
            {
              id: 'node_starfall_pak1',
              name: 'assets.pak',
              path: 'C:\\Games\\Starfall\\assets.pak',
              kind: 'file',
              objectId: 'obj_starfall_pak1',
              entryCount: 1,
              uniqueObjectCount: 1,
              referencedAllocatedBytes: 38.2 * 1024 * 1024 * 1024,
              uniqueAllocatedBytes: 38.2 * 1024 * 1024 * 1024,
              referencedLogicalBytes: 38.2 * 1024 * 1024 * 1024,
              uniqueLogicalBytes: 38.2 * 1024 * 1024 * 1024,
              observationStatus: 'observed',
              coverage: 'complete',
              fileExt: '.pak',
              modifiedTime: '2025-04-05T12:00:00Z',
              category: 'Game Data Package',
              cleanupSafe: 'danger_game_asset'
            },
            {
              id: 'node_starfall_pak2',
              name: 'textures.pak',
              path: 'C:\\Games\\Starfall\\textures.pak',
              kind: 'file',
              objectId: 'obj_starfall_pak2',
              entryCount: 1,
              uniqueObjectCount: 1,
              referencedAllocatedBytes: 18.4 * 1024 * 1024 * 1024,
              uniqueAllocatedBytes: 18.4 * 1024 * 1024 * 1024,
              referencedLogicalBytes: 18.4 * 1024 * 1024 * 1024,
              uniqueLogicalBytes: 18.4 * 1024 * 1024 * 1024,
              observationStatus: 'observed',
              coverage: 'complete',
              fileExt: '.pak',
              modifiedTime: '2025-04-05T12:00:00Z',
              category: 'Game Texture Package',
              cleanupSafe: 'danger_game_asset'
            },
            {
              id: 'node_starfall_exe',
              name: 'starfall.exe',
              path: 'C:\\Games\\Starfall\\starfall.exe',
              kind: 'file',
              entryCount: 1,
              uniqueObjectCount: 1,
              referencedAllocatedBytes: 45 * 1024 * 1024,
              uniqueAllocatedBytes: 45 * 1024 * 1024,
              referencedLogicalBytes: 45 * 1024 * 1024,
              uniqueLogicalBytes: 45 * 1024 * 1024,
              observationStatus: 'observed',
              coverage: 'complete',
              fileExt: '.exe',
              modifiedTime: '2025-04-05T12:00:00Z',
              category: 'Application Executable',
              cleanupSafe: 'danger_game_asset'
            }
          ]
        },
        {
          id: 'node_retro',
          name: 'RetroEngine',
          path: 'C:\\Games\\RetroEngine',
          kind: 'directory',
          entryCount: 4450,
          uniqueObjectCount: 4450,
          referencedAllocatedBytes: 4.0 * 1024 * 1024 * 1024,
          uniqueAllocatedBytes: 4.0 * 1024 * 1024 * 1024,
          referencedLogicalBytes: 3.9 * 1024 * 1024 * 1024,
          uniqueLogicalBytes: 3.9 * 1024 * 1024 * 1024,
          observationStatus: 'observed',
          coverage: 'complete',
          modifiedTime: '2025-02-14T10:15:00Z',
          category: 'Game Directory',
          cleanupSafe: 'native_uninstall'
        }
      ]
    },
    {
      id: 'node_users',
      name: 'Users',
      path: 'C:\\Users',
      kind: 'directory',
      entryCount: 16540,
      uniqueObjectCount: 16540,
      referencedAllocatedBytes: 107.9 * 1024 * 1024 * 1024,
      uniqueAllocatedBytes: 107.9 * 1024 * 1024 * 1024,
      referencedLogicalBytes: 112.5 * 1024 * 1024 * 1024, // Higher logical due to OneDrive placeholder!
      uniqueLogicalBytes: 112.5 * 1024 * 1024 * 1024,
      observationStatus: 'observed',
      coverage: 'complete',
      modifiedTime: '2025-04-10T14:15:00Z',
      category: 'User Profiles',
      children: [
        {
          id: 'node_user_alex',
          name: 'Alex',
          path: 'C:\\Users\\Alex',
          kind: 'directory',
          entryCount: 15900,
          uniqueObjectCount: 15900,
          referencedAllocatedBytes: 106.8 * 1024 * 1024 * 1024,
          uniqueAllocatedBytes: 106.8 * 1024 * 1024 * 1024,
          referencedLogicalBytes: 111.4 * 1024 * 1024 * 1024,
          uniqueLogicalBytes: 111.4 * 1024 * 1024 * 1024,
          observationStatus: 'observed',
          coverage: 'complete',
          modifiedTime: '2025-04-10T14:15:00Z',
          category: 'User Profile Directory',
          children: [
            {
              id: 'node_alex_downloads',
              name: 'Downloads',
              path: 'C:\\Users\\Alex\\Downloads',
              kind: 'directory',
              entryCount: 124,
              uniqueObjectCount: 124,
              referencedAllocatedBytes: 28.9 * 1024 * 1024 * 1024,
              uniqueAllocatedBytes: 28.9 * 1024 * 1024 * 1024,
              referencedLogicalBytes: 28.6 * 1024 * 1024 * 1024,
              uniqueLogicalBytes: 28.6 * 1024 * 1024 * 1024,
              observationStatus: 'observed',
              coverage: 'complete',
              modifiedTime: '2025-04-10T11:00:00Z',
              category: 'User Downloads',
              cleanupSafe: 'user_reviewable',
              children: [
                {
                  id: 'node_alex_win11_iso',
                  name: 'Windows11_Setup_23H2.iso',
                  path: 'C:\\Users\\Alex\\Downloads\\Windows11_Setup_23H2.iso',
                  kind: 'file',
                  objectId: 'obj_win11_iso',
                  entryCount: 1,
                  uniqueObjectCount: 1,
                  referencedAllocatedBytes: 6.2 * 1024 * 1024 * 1024,
                  uniqueAllocatedBytes: 6.2 * 1024 * 1024 * 1024,
                  referencedLogicalBytes: 6.2 * 1024 * 1024 * 1024,
                  uniqueLogicalBytes: 6.2 * 1024 * 1024 * 1024,
                  observationStatus: 'observed',
                  coverage: 'complete',
                  fileExt: '.iso',
                  modifiedTime: '2024-12-10T08:00:00Z',
                  category: 'Disk Image',
                  cleanupSafe: 'user_reviewable'
                },
                {
                  id: 'node_alex_ubuntu_iso',
                  name: 'ubuntu-24.04-desktop-amd64.iso',
                  path: 'C:\\Users\\Alex\\Downloads\\ubuntu-24.04-desktop-amd64.iso',
                  kind: 'file',
                  objectId: 'obj_ubuntu_iso',
                  entryCount: 1,
                  uniqueObjectCount: 1,
                  referencedAllocatedBytes: 5.8 * 1024 * 1024 * 1024,
                  uniqueAllocatedBytes: 5.8 * 1024 * 1024 * 1024,
                  referencedLogicalBytes: 5.8 * 1024 * 1024 * 1024,
                  uniqueLogicalBytes: 5.8 * 1024 * 1024 * 1024,
                  observationStatus: 'observed',
                  coverage: 'complete',
                  fileExt: '.iso',
                  modifiedTime: '2025-02-25T14:30:00Z',
                  category: 'Disk Image',
                  cleanupSafe: 'user_reviewable'
                },
                {
                  id: 'node_alex_dataset_zip',
                  name: 'heavy_dataset_raw.zip',
                  path: 'C:\\Users\\Alex\\Downloads\\heavy_dataset_raw.zip',
                  kind: 'file',
                  entryCount: 1,
                  uniqueObjectCount: 1,
                  referencedAllocatedBytes: 8.4 * 1024 * 1024 * 1024,
                  uniqueAllocatedBytes: 8.4 * 1024 * 1024 * 1024,
                  referencedLogicalBytes: 8.4 * 1024 * 1024 * 1024,
                  uniqueLogicalBytes: 8.4 * 1024 * 1024 * 1024,
                  observationStatus: 'observed',
                  coverage: 'complete',
                  fileExt: '.zip',
                  modifiedTime: '2024-09-18T16:00:00Z',
                  category: 'Archive',
                  cleanupSafe: 'user_reviewable'
                },
                {
                  id: 'node_alex_unused_archive',
                  name: 'UnusedProjectArchive.tar.gz',
                  path: 'C:\\Users\\Alex\\Downloads\\UnusedProjectArchive.tar.gz',
                  kind: 'file',
                  entryCount: 1,
                  uniqueObjectCount: 1,
                  referencedAllocatedBytes: 4.1 * 1024 * 1024 * 1024,
                  uniqueAllocatedBytes: 4.1 * 1024 * 1024 * 1024,
                  referencedLogicalBytes: 4.1 * 1024 * 1024 * 1024,
                  uniqueLogicalBytes: 4.1 * 1024 * 1024 * 1024,
                  observationStatus: 'observed',
                  coverage: 'complete',
                  fileExt: '.gz',
                  modifiedTime: '2024-03-01T10:00:00Z',
                  category: 'Archive',
                  cleanupSafe: 'user_reviewable'
                },
                {
                  id: 'node_alex_node_msi',
                  name: 'Node_v20_Installer.msi',
                  path: 'C:\\Users\\Alex\\Downloads\\Node_v20_Installer.msi',
                  kind: 'file',
                  entryCount: 1,
                  uniqueObjectCount: 1,
                  referencedAllocatedBytes: 32 * 1024 * 1024,
                  uniqueAllocatedBytes: 32 * 1024 * 1024,
                  referencedLogicalBytes: 32 * 1024 * 1024,
                  uniqueLogicalBytes: 32 * 1024 * 1024,
                  observationStatus: 'observed',
                  coverage: 'complete',
                  fileExt: '.msi',
                  modifiedTime: '2025-04-05T09:00:00Z',
                  category: 'Installer Package',
                  cleanupSafe: 'user_reviewable'
                }
              ]
            },
            {
              id: 'node_alex_onedrive',
              name: 'OneDrive',
              path: 'C:\\Users\\Alex\\OneDrive',
              kind: 'directory',
              entryCount: 450,
              uniqueObjectCount: 450,
              referencedAllocatedBytes: 0.1 * 1024 * 1024 * 1024,
              uniqueAllocatedBytes: 0.1 * 1024 * 1024 * 1024,
              referencedLogicalBytes: 4.9 * 1024 * 1024 * 1024, // Demonstrates cloud placeholder!
              uniqueLogicalBytes: 4.9 * 1024 * 1024 * 1024,
              observationStatus: 'observed',
              coverage: 'complete',
              modifiedTime: '2025-04-09T18:00:00Z',
              category: 'Cloud Storage Sync',
              children: [
                {
                  id: 'node_alex_onedrive_zip',
                  name: 'Archive_2024_Backup.zip',
                  path: 'C:\\Users\\Alex\\OneDrive\\Archive_2024_Backup.zip',
                  kind: 'file',
                  objectId: 'obj_onedrive_zip',
                  entryCount: 1,
                  uniqueObjectCount: 1,
                  referencedAllocatedBytes: 0, // 0 bytes allocated on disk!
                  uniqueAllocatedBytes: 0,
                  referencedLogicalBytes: 4.8 * 1024 * 1024 * 1024,
                  uniqueLogicalBytes: 4.8 * 1024 * 1024 * 1024,
                  observationStatus: 'observed',
                  coverage: 'complete',
                  storageCharacteristics: ['online-only', 'reparse-point'],
                  fileExt: '.zip',
                  modifiedTime: '2025-01-05T12:00:00Z',
                  category: 'Cloud File Placeholder (Online Only)',
                  cleanupSafe: 'cloud_online_only'
                },
                {
                  id: 'node_alex_onedrive_doc',
                  name: 'ProjectProposals.docx',
                  path: 'C:\\Users\\Alex\\OneDrive\\ProjectProposals.docx',
                  kind: 'file',
                  entryCount: 1,
                  uniqueObjectCount: 1,
                  referencedAllocatedBytes: 12.5 * 1024 * 1024,
                  uniqueAllocatedBytes: 12.5 * 1024 * 1024,
                  referencedLogicalBytes: 12.4 * 1024 * 1024,
                  uniqueLogicalBytes: 12.4 * 1024 * 1024,
                  observationStatus: 'observed',
                  coverage: 'complete',
                  fileExt: '.docx',
                  modifiedTime: '2025-04-02T11:20:00Z',
                  category: 'Document',
                  cleanupSafe: 'user_reviewable'
                }
              ]
            },
            {
              id: 'node_alex_appdata',
              name: 'AppData',
              path: 'C:\\Users\\Alex\\AppData',
              kind: 'directory',
              entryCount: 9200,
              uniqueObjectCount: 9200,
              referencedAllocatedBytes: 24.8 * 1024 * 1024 * 1024,
              uniqueAllocatedBytes: 24.8 * 1024 * 1024 * 1024,
              referencedLogicalBytes: 24.5 * 1024 * 1024 * 1024,
              uniqueLogicalBytes: 24.5 * 1024 * 1024 * 1024,
              observationStatus: 'observed',
              coverage: 'complete',
              modifiedTime: '2025-04-10T14:10:00Z',
              category: 'Application Cache & Settings',
              cleanupSafe: 'cache_review',
              children: [
                {
                  id: 'node_alex_temp',
                  name: 'Local\\Temp',
                  path: 'C:\\Users\\Alex\\AppData\\Local\\Temp',
                  kind: 'directory',
                  entryCount: 3100,
                  uniqueObjectCount: 3100,
                  referencedAllocatedBytes: 8.2 * 1024 * 1024 * 1024,
                  uniqueAllocatedBytes: 8.2 * 1024 * 1024 * 1024,
                  referencedLogicalBytes: 8.2 * 1024 * 1024 * 1024,
                  uniqueLogicalBytes: 8.2 * 1024 * 1024 * 1024,
                  observationStatus: 'observed',
                  coverage: 'complete',
                  modifiedTime: '2025-04-10T14:10:00Z',
                  category: 'Temporary Files',
                  cleanupSafe: 'system_temp'
                }
              ]
            },
            {
              id: 'node_alex_projects',
              name: 'Projects',
              path: 'C:\\Users\\Alex\\Projects',
              kind: 'directory',
              entryCount: 6100,
              uniqueObjectCount: 6100,
              referencedAllocatedBytes: 36.5 * 1024 * 1024 * 1024,
              uniqueAllocatedBytes: 36.5 * 1024 * 1024 * 1024,
              referencedLogicalBytes: 36.2 * 1024 * 1024 * 1024,
              uniqueLogicalBytes: 36.2 * 1024 * 1024 * 1024,
              observationStatus: 'observed',
              coverage: 'complete',
              modifiedTime: '2025-04-10T13:45:00Z',
              category: 'Developer Code & Datasets',
              children: [
                {
                  id: 'node_alex_largeml',
                  name: 'LargeMLModel',
                  path: 'C:\\Users\\Alex\\Projects\\LargeMLModel',
                  kind: 'directory',
                  entryCount: 450,
                  uniqueObjectCount: 450,
                  referencedAllocatedBytes: 28.6 * 1024 * 1024 * 1024,
                  uniqueAllocatedBytes: 28.6 * 1024 * 1024 * 1024,
                  referencedLogicalBytes: 28.5 * 1024 * 1024 * 1024,
                  uniqueLogicalBytes: 28.5 * 1024 * 1024 * 1024,
                  observationStatus: 'observed',
                  coverage: 'complete',
                  modifiedTime: '2025-04-06T15:00:00Z',
                  category: 'Machine Learning Models & Weights',
                  cleanupSafe: 'user_reviewable'
                }
              ]
            }
          ]
        }
      ]
    },
    {
      id: 'node_windows',
      name: 'Windows',
      path: 'C:\\Windows',
      kind: 'directory',
      entryCount: 11420,
      uniqueObjectCount: 8850, // Fewer unique objects than entries due to WinSxS hardlinks!
      referencedAllocatedBytes: 48.2 * 1024 * 1024 * 1024, // Sum of all links
      uniqueAllocatedBytes: 42.1 * 1024 * 1024 * 1024,    // Distinct object allocation
      referencedLogicalBytes: 49.5 * 1024 * 1024 * 1024,
      uniqueLogicalBytes: 43.0 * 1024 * 1024 * 1024,
      observationStatus: 'observed',
      coverage: 'complete',
      modifiedTime: '2025-04-10T04:15:00Z',
      category: 'Windows Operating System',
      cleanupSafe: 'protected_system',
      children: [
        {
          id: 'node_winsxs',
          name: 'WinSxS',
          path: 'C:\\Windows\\WinSxS',
          kind: 'directory',
          entryCount: 4800,
          uniqueObjectCount: 2230,
          referencedAllocatedBytes: 18.5 * 1024 * 1024 * 1024,
          uniqueAllocatedBytes: 12.4 * 1024 * 1024 * 1024, // Hardlinked with System32
          referencedLogicalBytes: 19.0 * 1024 * 1024 * 1024,
          uniqueLogicalBytes: 12.8 * 1024 * 1024 * 1024,
          observationStatus: 'observed',
          coverage: 'complete',
          modifiedTime: '2025-04-09T22:00:00Z',
          category: 'Windows Component Store (Hardlinked)',
          cleanupSafe: 'protected_dism_only',
          children: [
            {
              id: 'node_winsxs_shell32',
              name: 'amd64_microsoft-windows-shell32_...\\shell32.dll',
              path: 'C:\\Windows\\WinSxS\\amd64_microsoft-windows-shell32_6.3.9600.17415\\shell32.dll',
              kind: 'file',
              objectId: 'obj_shell32', // Shared object with System32\shell32.dll!
              entryCount: 1,
              uniqueObjectCount: 1,
              referencedAllocatedBytes: 14.2 * 1024 * 1024,
              uniqueAllocatedBytes: 14.2 * 1024 * 1024,
              referencedLogicalBytes: 14.2 * 1024 * 1024,
              uniqueLogicalBytes: 14.2 * 1024 * 1024,
              observationStatus: 'observed',
              coverage: 'complete',
              fileExt: '.dll',
              modifiedTime: '2025-03-20T12:00:00Z',
              category: 'Hardlinked System DLL (WinSxS Component Store)',
              cleanupSafe: 'protected_system'
            }
          ]
        },
        {
          id: 'node_system32',
          name: 'System32',
          path: 'C:\\Windows\\System32',
          kind: 'directory',
          entryCount: 5200,
          uniqueObjectCount: 5200,
          referencedAllocatedBytes: 21.0 * 1024 * 1024 * 1024,
          uniqueAllocatedBytes: 21.0 * 1024 * 1024 * 1024,
          referencedLogicalBytes: 21.2 * 1024 * 1024 * 1024,
          uniqueLogicalBytes: 21.2 * 1024 * 1024 * 1024,
          observationStatus: 'observed',
          coverage: 'complete',
          modifiedTime: '2025-04-10T04:15:00Z',
          category: 'Windows Core System Files',
          cleanupSafe: 'protected_system',
          children: [
            {
              id: 'node_sys32_shell32',
              name: 'shell32.dll',
              path: 'C:\\Windows\\System32\\shell32.dll',
              kind: 'file',
              objectId: 'obj_shell32', // Shared object with WinSxS!
              entryCount: 1,
              uniqueObjectCount: 1,
              referencedAllocatedBytes: 14.2 * 1024 * 1024,
              uniqueAllocatedBytes: 14.2 * 1024 * 1024,
              referencedLogicalBytes: 14.2 * 1024 * 1024,
              uniqueLogicalBytes: 14.2 * 1024 * 1024,
              observationStatus: 'observed',
              coverage: 'complete',
              fileExt: '.dll',
              modifiedTime: '2025-03-20T12:00:00Z',
              category: 'Hardlinked System DLL (System32)',
              cleanupSafe: 'protected_system'
            },
            {
              id: 'node_sys32_ntoskrnl',
              name: 'ntoskrnl.exe',
              path: 'C:\\Windows\\System32\\ntoskrnl.exe',
              kind: 'file',
              objectId: 'obj_ntoskrnl',
              entryCount: 1,
              uniqueObjectCount: 1,
              referencedAllocatedBytes: 11.8 * 1024 * 1024,
              uniqueAllocatedBytes: 11.8 * 1024 * 1024,
              referencedLogicalBytes: 11.8 * 1024 * 1024,
              uniqueLogicalBytes: 11.8 * 1024 * 1024,
              observationStatus: 'observed',
              coverage: 'complete',
              fileExt: '.exe',
              modifiedTime: '2025-03-20T12:00:00Z',
              category: 'Windows NT OS Kernel Executable',
              cleanupSafe: 'protected_system'
            }
          ]
        },
        {
          id: 'node_softwaredist',
          name: 'SoftwareDistribution',
          path: 'C:\\Windows\\SoftwareDistribution',
          kind: 'directory',
          entryCount: 1420,
          uniqueObjectCount: 1420,
          referencedAllocatedBytes: 3.4 * 1024 * 1024 * 1024,
          uniqueAllocatedBytes: 3.4 * 1024 * 1024 * 1024,
          referencedLogicalBytes: 3.4 * 1024 * 1024 * 1024,
          uniqueLogicalBytes: 3.4 * 1024 * 1024 * 1024,
          observationStatus: 'observed',
          coverage: 'complete',
          modifiedTime: '2025-04-09T14:00:00Z',
          category: 'Windows Update Cache',
          cleanupSafe: 'native_disk_cleanup'
        }
      ]
    },
    {
      id: 'node_programfiles',
      name: 'Program Files',
      path: 'C:\\Program Files',
      kind: 'directory',
      entryCount: 4200,
      uniqueObjectCount: 4200,
      referencedAllocatedBytes: 45.8 * 1024 * 1024 * 1024,
      uniqueAllocatedBytes: 45.8 * 1024 * 1024 * 1024,
      referencedLogicalBytes: 45.2 * 1024 * 1024 * 1024,
      uniqueLogicalBytes: 45.2 * 1024 * 1024 * 1024,
      observationStatus: 'observed',
      coverage: 'complete',
      modifiedTime: '2025-04-09T16:00:00Z',
      category: 'Installed 64-bit Applications',
      cleanupSafe: 'native_uninstall',
      children: [
        {
          id: 'node_adobe',
          name: 'Adobe',
          path: 'C:\\Program Files\\Adobe',
          kind: 'directory',
          entryCount: 1800,
          uniqueObjectCount: 1800,
          referencedAllocatedBytes: 18.6 * 1024 * 1024 * 1024,
          uniqueAllocatedBytes: 18.6 * 1024 * 1024 * 1024,
          referencedLogicalBytes: 18.5 * 1024 * 1024 * 1024,
          uniqueLogicalBytes: 18.5 * 1024 * 1024 * 1024,
          observationStatus: 'observed',
          coverage: 'complete',
          modifiedTime: '2025-04-01T12:00:00Z',
          category: 'Creative Suite Suite Software',
          cleanupSafe: 'native_uninstall'
        },
        {
          id: 'node_docker',
          name: 'Docker',
          path: 'C:\\Program Files\\Docker',
          kind: 'directory',
          entryCount: 1200,
          uniqueObjectCount: 1200,
          referencedAllocatedBytes: 14.4 * 1024 * 1024 * 1024,
          uniqueAllocatedBytes: 14.4 * 1024 * 1024 * 1024,
          referencedLogicalBytes: 14.2 * 1024 * 1024 * 1024,
          uniqueLogicalBytes: 14.2 * 1024 * 1024 * 1024,
          observationStatus: 'observed',
          coverage: 'complete',
          modifiedTime: '2025-04-02T10:00:00Z',
          category: 'Container Virtualization Tools',
          cleanupSafe: 'native_uninstall'
        }
      ]
    },
    {
      id: 'node_pagefile',
      name: 'pagefile.sys',
      path: 'C:\\pagefile.sys',
      kind: 'special',
      objectId: 'obj_pagefile',
      entryCount: 1,
      uniqueObjectCount: 1,
      referencedAllocatedBytes: 16.0 * 1024 * 1024 * 1024,
      uniqueAllocatedBytes: 16.0 * 1024 * 1024 * 1024,
      referencedLogicalBytes: 16.0 * 1024 * 1024 * 1024,
      uniqueLogicalBytes: 16.0 * 1024 * 1024 * 1024,
      observationStatus: 'observed',
      coverage: 'complete',
      fileExt: '.sys',
      modifiedTime: '2025-04-10T14:20:00Z',
      category: 'Windows Virtual Memory Paging File',
      cleanupSafe: 'system_critical_lock'
    },
    {
      id: 'node_hiberfil',
      name: 'hiberfil.sys',
      path: 'C:\\hiberfil.sys',
      kind: 'special',
      objectId: 'obj_hiberfil',
      entryCount: 1,
      uniqueObjectCount: 1,
      referencedAllocatedBytes: 12.8 * 1024 * 1024 * 1024,
      uniqueAllocatedBytes: 12.8 * 1024 * 1024 * 1024,
      referencedLogicalBytes: 12.8 * 1024 * 1024 * 1024,
      uniqueLogicalBytes: 12.8 * 1024 * 1024 * 1024,
      observationStatus: 'observed',
      coverage: 'complete',
      fileExt: '.sys',
      modifiedTime: '2025-04-10T14:20:00Z',
      category: 'Windows Fast Startup / Hibernation File',
      cleanupSafe: 'powercfg_disable_hibernation'
    },
    {
      id: 'node_sysvolinfo',
      name: 'System Volume Information',
      path: 'C:\\System Volume Information',
      kind: 'directory',
      entryCount: 0,
      uniqueObjectCount: 0,
      referencedAllocatedBytes: 0,
      uniqueAllocatedBytes: 0,
      referencedLogicalBytes: 0,
      uniqueLogicalBytes: 0,
      observationStatus: 'inaccessible',
      coverage: 'indeterminate',
      isCoverageGap: true,
      coverageGapReason: 'STATUS_ACCESS_DENIED under current security context. Omits volume shadow copies, restore points, and indexing database.',
      modifiedTime: '2025-04-10T14:00:00Z',
      category: 'System Storage (Inaccessible)',
      cleanupSafe: 'system_inaccessible'
    }
  ]
};

// Reconciliation item for volume scope (NEVER an ordinary directory)
export const RECONCILIATION_ITEM = {
  id: 'reconciliation_unattributed',
  name: '[Unattributed Used Space]',
  path: 'C:\\ (Volume Reconciliation: Unattributed Used Space)',
  kind: 'reconciliation',
  isReconciliation: true,
  allocatedBytes: 24.0 * 1024 * 1024 * 1024, // 24 GB
  logicalBytes: 24.0 * 1024 * 1024 * 1024,
  description: 'Difference between Volume Used Space (392 GB) and Accounted Unique Allocation (368 GB). Indicates filesystem metadata/MFT reserves, inaccessible system regions (e.g. System Volume Information shadow copies), unsupported features, or live storage changes during the observation interval. This is a reconciliation measure, not an ordinary directory.',
  category: 'Reconciliation Difference'
};

// Hardlink aliases lookup table
export const HARDLINK_ALIASES = {
  'obj_shell32': [
    'C:\\Windows\\System32\\shell32.dll',
    'C:\\Windows\\WinSxS\\amd64_microsoft-windows-shell32_6.3.9600.17415\\shell32.dll'
  ]
};

if (typeof window !== 'undefined') {
  window.PigTreeMockData = {
    PRESETS,
    MOCK_VOLUMES,
    HISTORICAL_SNAPSHOTS,
    MOCK_OBJECTS,
    MOCK_TREE_ROOT,
    RECONCILIATION_ITEM,
    HARDLINK_ALIASES
  };
}
