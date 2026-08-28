# Everyday Disk-Analysis Workflows, Usability Failures, and Guarded Cleanup

**Author / Assignee:** PigTree Agent  
**Status:** Completed  
**Topic:** Wayfinder Research Ticket [#3](https://github.com/AFlyingP/PigTree/issues/3)  
**Date:** March 2025  

---

## Executive Summary

Disk-space analysis on Windows is driven by a state of acute user friction: a low-disk-space warning, an update failure, a game install refusal, or system degradation. Users launch specialized disk analyzers with a single primary job to be done: **identify large or redundant data quickly, understand what is safe to remove, and reclaim storage without destabilizing the operating system or losing critical files.**

This investigation evaluates primary documentation, official competitor documentation (WizTree / Antibody Software, TreeSize / JAM Software, WinDirStat), Microsoft Learn system architecture documentation, assistive technology guides, and direct user evidence across forums and issue trackers. It separates **first-party design facts** (how NTFS, MFT, shell APIs, and competitor tools actually work) from **anecdotal user pain** (where users get confused, make mistakes, or experience anxiety).

---

## 1. The User Journey: Core Jobs & Decision Points

Everyday users navigate four distinct stages during a disk-space analysis session:

```
┌──────────────────┐    ┌─────────────────────────┐    ┌───────────────────────────┐    ┌──────────────────────┐
│  1. Scan & Scope │───>│ 2. Visual & Tree Nav    │───>│ 3. Investigation & Safety │───>│ 4. Guarded Reclaim   │
│  Select Target   │    │ Locate Large Consumptive│    │ "What is this file?"      │    │ Recycle Bin vs Del   │
│  Manage UAC/Perms│    │ Treemap / Extension View│    │ "Is it safe to delete?"   │    │ Shell vs Native Del  │
└──────────────────┘    └─────────────────────────┘    └───────────────────────────┘    └──────────────────────┘
```

### Key Jobs to Be Done (JTBD)
1. **Emergency Space Reclamation:** Find 10–50+ GB immediately to allow an OS update, large application install, or resume normal workflow.
2. **Drive Hygiene & Audit:** Locate orphaned installations, abandoned cache directories, large download leftovers, and duplicate files.
3. **Capacity Planning:** Understand what categories of files (media, games, system, developer caches) occupy disk space over time.
4. **Safety & Verification:** Distinguish user-managed personal data from OS-managed or application-critical assets before executing destructive actions.

---

## 2. Scan Initiation & Scope: Elevation, Performance, and Coverage

### First-Party Technical Facts
- **MFT Direct Scan vs. Windows API Traversal:**
  - *NTFS MFT Direct Reading:* Competitors like WizTree and TreeSize Professional read `$MFT` directly from the raw NTFS volume ([Antibody Software WizTree Docs](https://diskanalyzer.com/guide), [JAM Software TreeSize Help](https://www.jam-software.com/treesize)). This achieves scans in 1–5 seconds across millions of files by bypassing per-file Win32 directory traversal (`FindFirstFileExW` / `NtQueryDirectoryFile`).
  - *Privilege Requirements:* Direct MFT reading requires elevated administrative rights (`SeBackupPrivilege` / `SeRestorePrivilege` and raw handle access via `\\.\C:`).
  - *Non-NTFS / Remote Fallback:* On exFAT, FAT32, ReFS, network shares (SMB/NFS), and non-elevated runs, analyzers must fall back to recursive Win32 filesystem traversal, where scan times scale linearly with file/directory count and network latency.

### Usability Failures & User Pain Points
1. **The UAC Elevation Barrier & Misleading Scans:**
   - When users run an analyzer without elevation, tools either fail to scan system-protected folders (e.g., `System Volume Information`, `C:\Windows\System32\Configuration`, other user profiles in `C:\Users`) or silently report them as 0 bytes ([JAM Software TreeSize Permissions & NTFS Manual](https://manuals.jam-software.com/treesize/EN/notes_on_ntfs.html)).
   - Users report confusion when total used space reported by a non-elevated scan does not match Windows Explorer’s drive capacity bar by tens of gigabytes.
   - Forcing an immediate UAC prompt on app launch creates trust friction for users who only want to scan a non-system partition or user folder.
2. **The "Performance Cliff" Between Local and Network/External Drives:**
   - Users accustomed to 2-second MFT scans on local SSDs assume a hang or bug when scanning a 4TB USB external drive (often formatted as exFAT) or a NAS share, where enumeration takes several minutes without real-time progress feedback.
3. **Volume Scope Confusion:**
   - Users struggle to understand mounted virtual disks (WSL2 `ext4.vhdx`, Hyper-V VHDX, Docker overlays, Sandbox images) which appear as single massive monolithic files on host storage while hiding the internal consumption structure.

---

## 3. Understanding Results: Information Semantics & Visualizations

### Visualizations vs. Tabular Lists
- **Treemaps (Squarified / Cushion):**
  - *Pros:* High information density; immediately reveals disproportionately large files or directory clusters.
  - *Pain Points:* Users report "visual noise" and cognitive overload. When thousands of small files are rendered as tiny sub-pixel mosaic fragments, the treemap becomes an unreadable texture. Users complain about lack of persistent labels and disorientation when zooming levels without breadcrumbs.
- **Hierarchical Tree-Table:**
  - Standard expandable tree-view with size, percentage bar, file count, and last modified date. Users overwhelmingly rely on tree-tables for precise folder-by-folder decision making.
- **Extension / File Type Breakdown:**
  - Grouping by category (Videos, Archives, Executables, Disk Images, Development) provides immediate mental classification for non-technical users.

### Size Semantics: The Technical Distinctions That Confuse Users

| Semantic Metric | Technical Meaning | User Confusion / Pitfall |
| :--- | :--- | :--- |
| **Logical Size (Size)** | The uncompressed byte count of file contents (`nFileSizeHigh/Low`). | Summing logical sizes leads to massive overestimation in directories with hardlinks or sparse files. |
| **Allocated Size (Size on Disk)** | Physical clusters allocated on the volume (multiples of cluster size, typically 4KB). | Files < 1KB take 4KB on disk (slack); highly sparse or NTFS-compressed files consume less on disk than logical size. |
| **Hardlinks (`WinSxS`)** | Multiple directory entries pointing to the same MFT record / data clusters ([Microsoft Learn Hard Links](https://learn.microsoft.com/windows/win32/fileio/hard-links-and-junctions)). | Naive analyzers count shared files in `C:\Windows\WinSxS` and `System32` multiple times, falsely showing `WinSxS` taking 20–30+ GB when actual exclusive space is 6–8 GB. |
| **Cloud Dehydrated Files** | Reparse points with `FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS` and `FILE_ATTRIBUTE_OFFLINE` (OneDrive Files On-Demand, iCloud, Dropbox) ([Microsoft Cloud Files API](https://learn.microsoft.com/windows/win32/cfapi/build-a-cloud-file-sync-engine)). | Logical size shows 100 GB, allocated size is 0 KB. Naive scanners or hashers open these files and unintentionally trigger full hydration (downloading hundreds of gigabytes over the network). |
| **System Volume Information** | Volume Shadow Copies (VSS restore points), Chkdsk logs, Indexing catalogs ([Microsoft Learn VSS](https://learn.microsoft.com/windows-server/storage/file-server/volume-shadow-copy-service)). | Hidden by default, inaccessible ACLs; users see massive "unaccounted" or "unknown" space that cannot be explored or deleted directly. |

---

## 4. Dangerous Territory & User Cleanup Dilemmas

### The "Can I Delete This?" Problem
Forums (Reddit r/techsupport, SuperUser, Microsoft Community) are inundated with identical recurring questions when users inspect disk analyzer outputs:

```
User Query: "WizTree shows C:\Windows\WinSxS is 22 GB. Can I delete it to free space?"
Reality:    Deleting WinSxS directly bricks Windows Update and component servicing.
Safe Fix:   dism.exe /online /cleanup-image /startcomponentcleanup
```

### High-Risk System Folders Inventory

1. **`C:\Windows\WinSxS` (Windows Side-by-Side Component Store):**
   - *Risk:* Deleting files directly corrupts the Windows component servicing engine and prevents cumulative updates or feature installation ([Microsoft Learn WinSxS Cleanup](https://learn.microsoft.com/windows-hardware/manufacture/desktop/clean-up-the-winsxs-folder)).
   - *Proper Remediation:* Windows Component Cleanup via DISM (`dism /online /cleanup-image /startcomponentcleanup /resetbase`).
2. **`C:\Windows\Installer` (Windows Installer Cache):**
   - *Risk:* Stores cached `.msi` and `.msp` files needed to repair, modify, update, or uninstall installed software. Deleting it breaks future updates and uninstalls with Error 1612 ([Microsoft Learn Restore Missing Installer Cache](https://learn.microsoft.com/troubleshoot/windows-client/installing-updates-features-roles/restore-missing-windows-installer-cache-files)).
   - *Proper Remediation:* Only orphaned installer files (unregistered in registry) may be purged; active records must never be touched.
3. **`C:\Windows\System32\DriverStore\FileRepository`:**
   - *Risk:* Contains staged driver packages. Manual deletion corrupts the driver store and triggers access denials due to `TrustedInstaller` permissions ([Microsoft Learn DriverStore](https://learn.microsoft.com/windows-hardware/drivers/install/driver-store)).
   - *Proper Remediation:* `pnputil.exe /delete-driver <oem#.inf> /uninstall /force`.
4. **`hiberfil.sys`, `pagefile.sys`, `swapfile.sys`:**
   - *Risk:* Locked exclusively by the Windows NT kernel at boot. Users cannot delete them in disk analyzers (results in `ERROR_SHARING_VIOLATION`).
   - *Proper Remediation:* Hibernation size reduction or disabling via `powercfg /hibernate off` or `powercfg /h /type reduced`; Virtual Memory configuration in System Properties.
5. **`AppData\Local` vs. `AppData\Roaming`:**
   - *Risk:* Users aggressively deleting `AppData` wipe application logins, settings, browser profiles, and game saves.
   - *Safe Targets:* True temporary directories (`AppData\Local\Temp`, `AppData\Local\CrashDumps`, browser `GPUCache`, npm/pip package caches).

---

## 5. Reclaiming Space: Safety Rails and Deletion Failure Modes

### Shell Deletion vs. Permanent Deletion Mechanics
- **Recycle Bin (`IFileOperation` with `FOF_ALLOWUNDO` / `FOFX_RECYCLEONDELETE`):**
  - Sends files to `$Recycle.Bin`, allowing recovery.
  - *Failure Mode 1 (Quota Overflow):* If a selected file exceeds the volume’s configured Recycle Bin limit (default ~5–10% of drive size), Windows silently bypasses the Recycle Bin or prompts for immediate permanent deletion ([Microsoft Learn IFileOperation](https://learn.microsoft.com/windows/win32/api/shobjidl_core/nn-shobjidl_core-ifileoperation)).
  - *Failure Mode 2 (Removable & Network Drives):* By default, mapped network drives and removable USB drives have **no Recycle Bin support**. Any delete action on these targets is instant and permanent without shell undo.
- **Cloud File Deletion Traps:**
  - Deleting a cloud-synced dehydrated file in a disk analyzer issues a filesystem delete request. The sync client (OneDrive) treats this as an intentional deletion and moves the file to the cloud Recycle Bin, potentially removing the only copy.
- **Locked Files and Permission Denials:**
  - Files locked by active processes (`ERROR_SHARING_VIOLATION`) or owned by `NT SERVICE\TrustedInstaller` or `SYSTEM` (`ERROR_ACCESS_DENIED`) fail deletion.
  - Naive deletion loops in third-party tools abort midway, leaving partial deletions and inconsistent directory states.
- **Long Paths (> 260 Characters):**
  - Files with deep path hierarchies fail standard Win32 APIs unless the app uses the `\\?\` prefix and Unicode APIs (`*W`).

---

## 6. Accessibility, Assistive Technology & Usability

### Treemap Inaccessibility
- Treemaps are visually dense 2D spatial layouts drawn directly to hardware canvases or GDI/Direct2D surfaces.
- **Screen Reader Reality:** Screen readers (Narrator, NVDA, JAWS) cannot traverse raw graphical treemap canvases. When focused, the treemap is reported as an unlabelled generic control or inaccessible image ([NV Access Guide](https://www.nvaccess.org/)).
- **Mandatory Alternative:** Full accessibility requires a first-class, synchronized **Tree-Table / DataGrid** compliant with Microsoft UI Automation (UIA) patterns (`ITableProvider`, `IGridProvider`, `ISelectionProvider`), ensuring blind and low-vision users have 100% feature parity.

### Keyboard Navigation Standards
- Up/Down arrow navigation for hierarchical rows.
- Left/Right arrows for expanding/collapsing nodes.
- `Enter` to open/drill down; `Backspace` or `Alt+Up` to navigate to parent folder.
- `AppsKey` / `Shift+F10` to open Windows shell context menu for the focused file.
- `F5` to rescan selected node/drive.
- `Ctrl+C` to copy full file paths to clipboard.

### Visual Ergonomics & High Contrast
- **High-DPI Scaling:** Crisp rendering across 100% to 300% DPI scales without blurry text or clipped column headers (Per-Monitor DPI V2 awareness).
- **Colorblind-Safe Palettes:** Avoid relying solely on green-to-red gradients for file extensions or size thresholds. Provide configurable high-contrast themes and text/badge indicators alongside color cues.

---

## 7. Trust, Privacy, and Product Integrity

1. **Zero-Telemetry & Local-Only Invariant:**
   - Disk analyzers are system-wide diagnostic tools. They read confidential file paths, financial documents, private source code, and user activity history.
   - User communities heavily penalize disk tools that embed telemetry, trackers, or online network calls. The standard of trust is complete offline operation with zero off-device metadata transmission.
2. **Elevated Privilege Transparency:**
   - Instead of demanding unconditional Administrator rights on startup, best-in-class tools allow non-elevated launch and transparently offer elevation when a scan touches protected volumes or requires direct MFT acceleration.
3. **Clean Licensing & Non-Destructive Defaults:**
   - Users distrust disk utilities bundled with third-party software, aggressive upselling, or artificial deletion caps.
   - Guarded cleanup must default to safe recovery (Recycle Bin), require explicit multi-step confirmation for permanent deletion, and provide clear warnings when attempting to touch known OS-critical paths.

---

## 8. Synthesis & Implications for PigTree

| Investigation Finding | PigTree Design Implication |
| :--- | :--- |
| **Scan Speed vs. Privileges** | Support two scan pipelines: ultra-fast NTFS direct MFT scan when elevated, and a robust asynchronous Win32/NtQueryDirectoryFile fallback with transparent elevation hints. |
| **Hardlink Overestimation** | Track file inode / MFT record IDs during aggregation to prevent duplicate counting in `C:\Windows\WinSxS` and deduplicated volumes. |
| **Cloud Reparse Safety** | Respect `FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS`; inspect reparse points without reading file streams to avoid unintentional mass OneDrive hydration. |
| **Inaccessible Treemaps** | Provide synchronized dual-view architecture: an interactive visual treemap paired with a fully accessible UI Automation (UIA) tree-table with complete keyboard and screen-reader parity. |
| **Dangerous System Files** | Build a proactive "System Safeguard Knowledge Base" that detects OS-critical paths (`WinSxS`, `Installer`, `DriverStore`, `System Volume Information`, `hiberfil.sys`), explains what they are in plain language, disables reckless deletion, and guides users to native OS cleanup commands (`DISM`, `pnputil`, `powercfg`, `vssadmin`). |
| **Deletion Reliability** | Route deletions through modern Shell COM interfaces (`IFileOperation`), detect non-Recycle-Bin-capable volumes (network/USB), warn on quota overflow, and provide a clear deletion preview drawer. |
| **Privacy & Trust** | Enforce zero-telemetry, 100% local analysis, standalone portable support, and open-source transparency. |

---

## Primary References & Sources

1. **Antibody Software (WizTree):** [WizTree User Guide & MFT Architecture](https://diskanalyzer.com/guide)
2. **JAM Software (TreeSize):**
   - [TreeSize Overview & Product Documentation](https://www.jam-software.com/treesize)
   - [TreeSize Manual: Notes on NTFS & Access Permissions](https://manuals.jam-software.com/treesize/EN/notes_on_ntfs.html)
3. **Microsoft Learn (Storage Management & File APIs):**
   - [Hard Links and Junctions](https://learn.microsoft.com/windows/win32/fileio/hard-links-and-junctions)
   - [Cloud Files API & Dehydrated Reparse Points](https://learn.microsoft.com/windows/win32/cfapi/build-a-cloud-file-sync-engine)
   - [IFileOperation Shell Interface](https://learn.microsoft.com/windows/win32/api/shobjidl_core/nn-shobjidl_core-ifileoperation)
   - [Clean Up the WinSxS Folder (DISM)](https://learn.microsoft.com/windows-hardware/manufacture/desktop/clean-up-the-winsxs-folder)
   - [Restore missing Windows Installer cache files](https://learn.microsoft.com/troubleshoot/windows-client/installing-updates-features-roles/restore-missing-windows-installer-cache-files)
   - [Driver Store Operations (PnPUtil)](https://learn.microsoft.com/windows-hardware/drivers/install/driver-store)
   - [Volume Shadow Copy Service & vssadmin](https://learn.microsoft.com/windows-server/storage/file-server/volume-shadow-copy-service)
   - [Free up drive space in Windows (Storage Sense)](https://support.microsoft.com/windows/free-up-drive-space-in-windows-85529ccb-c365-490d-b548-831022bc9b32)
4. **Assistive Technology & Accessibility:**
   - [NV Access NVDA Screen Reader User Guide](https://www.nvaccess.org/)
   - [Microsoft UI Automation Guidelines for DataGrids & Trees](https://learn.microsoft.com/windows/apps/design/accessibility/accessibility-overview)