# Research Note: WizTree and TreeSize Capability Inventory & Parity Baseline

**Ticket**: [AFlyingP/PigTree#2](https://github.com/AFlyingP/PigTree/issues/2) — *Inventory WizTree and TreeSize capabilities*  
**Date**: March 2025  
**Author**: PigTree Architecture & Research Agent  
**Status**: Complete

---

## 1. Executive Summary

This research note establishes an exhaustive capability inventory and comparative analysis of the two dominant commercial/paid Windows disk space analyzer ecosystems:
1. **WizTree** (Antibody Software, v4.x / current) — High-performance, lightweight, MFT-centric disk space analyzer and treemap visualizer.
2. **TreeSize** (JAM Software, v9.x / TreeSize Free, Personal, Professional, and Consultant) — Enterprise-grade storage management, auditing, deduplication, and reporting suite.

The purpose is to provide an authoritative, primary-source-backed foundation for PigTree's product specification, capability boundary, and information model without requiring guesswork or proprietary feature cloning.

---

## 2. Storage Sources & Filesystem Support

| Source / Target | WizTree Capabilities | TreeSize (Personal / Professional) Capabilities | Primary Sources / Reference |
| :--- | :--- | :--- | :--- |
| **Local NTFS Volumes** | Full support. Scans raw Master File Table (`$MFT`) directly when elevated for near-instantaneous indexing. Fallback to Win32 directory traversal. | Full support. Uses fast MFT scanning when running as Administrator; fallback to multi-threaded Win32 traversal. Evaluates Alternate Data Streams (ADS) and NTFS deduplication in Professional. | [WizTree FAQ](https://diskanalyzer.com/faq), [TreeSize Manual: Notes on NTFS](https://manuals.jam-software.com/treesize/EN/notes_on_ntfs.html) |
| **ReFS Volumes** | Supported via standard Windows filesystem APIs (ReFS does not expose an NTFS-style `$MFT`). | Supported via standard APIs and multi-threaded traversal in Personal/Professional. | [WizTree Changelog](https://diskanalyzer.com/whats-new), [TreeSize Manual](https://manuals.jam-software.com/treesize/) |
| **FAT / FAT32 / exFAT** | Supported via standard Windows directory traversal APIs. | Supported across all editions via standard Windows filesystem APIs. | [WizTree Guides](https://diskanalyzer.com/guide), [TreeSize Overview](https://www.jam-software.com/treesize/) |
| **Network Shares (SMB / UNC / Mapped)** | Supported. Uses standard Windows file system APIs (UNC paths and mapped drive letters `X:\`). | Full support in Professional for Windows Server, Active Directory domain shares, and UNC paths (Free edition restricts domain share scanning). | [WizTree Guides](https://diskanalyzer.com/guide), [TreeSize Overview & Editions](https://www.jam-software.com/treesize/) |
| **Cloud Storage** | Local synchronized folder paths only (OneDrive, Dropbox, etc. mapped to local directories). | Direct scanning of SharePoint Online, Microsoft 365, Amazon S3, Azure Blob Storage, Google Drive, and WebDAV (TreeSize Professional). | [TreeSize Visual Tour](https://www.jam-software.com/treesize/visual-tour), [TreeSize Cloud Storage](https://manuals.jam-software.com/treesize/) |
| **Linux / Unix via SSH** | Not supported. | Supported in TreeSize Professional via SSH remote filesystem scanning. | [TreeSize Online Manual](https://manuals.jam-software.com/treesize/) |
| **Mobile Devices (MTP / PTP)** | Basic Windows Explorer shell folder access where exposed as drive letters. | Mobile devices supported via MTP / WebDAV. | [TreeSize Manual: Scan Targets](https://manuals.jam-software.com/treesize/) |
| **Snapshots & VSS** | No snapshot management. | Volume Shadow Copy (VSS) snapshot scanning and historical snapshot comparison down to file level. | [TreeSize Manual: Disk Usage Comparison](https://manuals.jam-software.com/treesize/EN/disk_usage_comparison.html) |

---

## 3. Scanning Architecture, Performance, & Privilege Models

### 3.1 WizTree Architecture
- **NTFS Direct MFT Parsing**: When launched with elevated administrator privileges, WizTree directly opens raw volume handles (e.g., `\\.\C:`) and parses the NTFS Master File Table (`$MFT`) binary records in memory. This indexes millions of files in seconds, bypassing standard Win32 directory iteration overhead ([WizTree FAQ](https://diskanalyzer.com/faq)).
- **Non-Admin & Non-NTFS Fallback**: For standard non-elevated user accounts, non-NTFS volumes (FAT32, exFAT, ReFS), or individual subfolder scans, WizTree falls back to standard Win32 directory traversal (`FindFirstFileW` / `FindNextFileW` or `GetFileInformationByHandleEx`) ([WizTree FAQ](https://diskanalyzer.com/faq)).
- **Memory & In-Memory Tree Index**: Scans populate a compact in-memory tree hierarchy. Changes can be refreshed rapidly.

### 3.2 TreeSize Architecture
- **Hybrid Multi-Threaded Scanner**: TreeSize employs a multi-threaded scanning pipeline for network shares and non-MFT scans, parallelizing folder traversal across CPU cores ([TreeSize Manual](https://manuals.jam-software.com/treesize/)).
- **NTFS MFT Acceleration**: Similar to WizTree, TreeSize utilizes fast MFT scanning on local NTFS drives when run as Administrator.
- **Continuous Tracking (SpaceObServer Step-Up)**: For scheduled, continuous historical indexing into relational databases (SQL Server, SQLite), JAM Software branches into SpaceObServer, whereas TreeSize standalone operates on point-in-time scans and saved XML indexes ([TreeSize vs SpaceObServer](https://www.jam-software.com/treesize/compare-spaceobserver)).

---

## 4. Surfaced Metadata & Information Model

| Metadata Field | WizTree | TreeSize (Personal/Pro) | Notes & Semantic Differences |
| :--- | :--- | :--- | :--- |
| **Logical File Size** | Yes (`Size`) | Yes (`Size`) | Uncompressed byte length of file payload. |
| **Allocated Size (Size on Disk)** | Yes (`Allocated`) | Yes (`Allocated Space`) | Physical clusters consumed on disk. Reflects compression, sparse files, and cluster slack space. |
| **Item Counts** | Files & Folders counts | Files & Folders counts | Aggregated recursively up the directory tree. |
| **Percentage of Parent / Total** | Yes (`% of Parent`) | Yes (`% of Parent / % of Total`) | Proportional space consumption. |
| **Timestamps** | Last Modified | Last Modified, Creation Date, Last Accessed | WizTree focuses on Last Modified; TreeSize surfaces full NTFS timestamp triad. |
| **File Attributes** | Bitmask (R, H, S, A, C, E, etc.) | Standard & Advanced Attributes | Read-Only, Hidden, System, Archive, Compressed, Encrypted, Reparse Point, Offline. |
| **Owner / Account SID** | No | Yes (Personal/Pro) | Owner user/domain account name, SID, and departmental cost attribution. |
| **Permissions / Access Rights (ACLs)** | No | Yes (Professional) | NTFS Access Control Lists summarized as permissions flags (`+/-R +/-W +/-X`) or detailed security matrices. |
| **Hard Link Count / Deduplication** | Hard links flagged (Allocated shows `0` for secondary links) | Yes, counts hardlinks, skips double-counting, calculates space saved via deduplication | Avoids inflated disk usage calculations for Windows Component Store (`WinSxS`). |
| **Alternate Data Streams (ADS)** | No | Yes (Professional) | Detects and measures hidden NTFS data streams attached to files. |
| **MFT Record Number** | Yes (exported via CLI / internal) | No explicit column in standard UI | Internal NTFS record identifier. |

---

## 5. Analysis Views & Visualizations

### 5.1 WizTree Views
1. **Tree View (Directory Hierarchy)**:
   - Expandable tree list with columns: *Name, % Parent, Size, Allocated, Files, Folders, Modified, Attributes*.
   - Dynamic sorting by any column header.
2. **File View (Flat Table Analysis)**:
   - High-performance flat table of individual files across the entire scan.
   - Configurable result cap (defaults to 1,000 files; configurable up to "ALL").
   - Filterable by file name, extension, date, and size boundaries.
3. **Treemap Visualizer**:
   - Interactive rectangular treemap at the bottom of the interface.
   - Color-coded by file extension/type.
   - Real-time synchronized navigation: clicking a block selects the corresponding file in the tree; hovering shows tooltips with full path, size, and type. Supports zooming into subtrees.
   - Exportable directly to PNG image ([WizTree Guides](https://diskanalyzer.com/guide)).

### 5.2 TreeSize Views
1. **Directory Tree & Details View**:
   - Customizable explorer-like tree and flat detail table with dozens of toggleable columns ([TreeSize Manual: Details](https://manuals.jam-software.com/treesize/EN/details.html)).
2. **Chart Visualizations**:
   - **Treemap Chart**: Hierarchical rectangular heat map with customizable depth, color palettes, and cushioning.
   - **Bar Chart & Pie Chart**: Proportional visual distribution of top folders and file sizes.
3. **Dedicated Extension / File Types View**:
   - Aggregates storage by file extension (e.g., `.mp4`, `.zip`, `.dll`), file category (Video, Audio, System, Documents), and percentage of disk used.
4. **Users / Owners Breakdown**:
   - Groups disk consumption by local or Active Directory user accounts ([TreeSize Visual Tour](https://www.jam-software.com/treesize/visual-tour)).
5. **Age of Files Distribution**:
   - Bins files into age intervals based on Creation, Modification, or Last Access dates (e.g., < 1 month, 1–3 months, > 1 year).
6. **Top 100 / Largest Files View**:
   - Instant listing of the top 100 largest files on the scanned source.
7. **History & Growth Comparison**:
   - Compares current scan state against saved XML baseline scans or Windows Shadow Copies to highlight added, removed, or expanded directories.

---

## 6. Search Behaviors, Query Syntax, & Filters

### 6.1 WizTree Search & Filter Engine
- **Filter Bar Toggle**: `Ctrl + Shift + F` toggles persistent Include and Exclude filters under the main toolbar ([WizTree Guides: Filters](https://diskanalyzer.com/guide)).
- **Query Operators & Syntax**:
  - **Wildcards**: `*` (matches 0 or more chars), `?` (single char).
  - **Boolean AND**: Space separator (e.g., `*.mp4 2024`).
  - **Boolean OR**: Pipe `|` without spaces (e.g., `*.mp3|*.wav|*.flac`).
  - **Exact Phrases**: Double quotes `"..."` (e.g., `"Program Files"`).
  - **Path Matching**: If a query contains a backslash `\`, full path matching is activated automatically; otherwise, matches filename only.
  - **Size Operators**: `>`, `<`, `>=`, `<=`, `=` with units `B`, `KB`, `MB`, `GB` (e.g., `>500MB`). Prefix with `a` for allocated size (`a>1GB`).
  - **Date Operators**: `>today-7`, `<2024/01/01`, `>NOW-24h` with time units (`s`, `m`, `h`, `d`).
  - **Duplicate Detection**: Built-in dropdown in File View matching by *File Name*, *File Name + Size*, or *File Name + Size + Date* (does not compute cryptographic hashes).

### 6.2 TreeSize Search & Filter Engine
- **TreeSize File Search (Dedicated Sub-System)**:
  - Accessible via ribbon or standalone invocation ([TreeSize Manual: File Search](https://manuals.jam-software.com/treesize/EN/file_search.html)).
  - **Duplicate Search**: Matches by *Name*, *Name + Size*, *Name + Date*, or **Cryptographic Checksum** (MD5 / SHA256 content hashing) ([TreeSize Manual: Duplicate Search](https://manuals.jam-software.com/treesize/EN/file_search.html)).
  - **Temporary & Obsolete File Search**: Built-in definitions for system temp files, browser caches, crash dumps, and obsolete file patterns.
  - **Advanced / Custom Search**: Multi-rule Boolean engine combining file size, attribute masks, owner filters, path lengths (e.g., `Path > 255 characters`), and **IFilter full-text search** inside documents (Word, Excel, PDF) ([TreeSize Manual: Advanced Search](https://manuals.jam-software.com/treesize/EN/custom_search.html)).

---

## 7. Cleanup & Deduplication Operations

| Operation | WizTree | TreeSize (Personal/Pro) | Safety & Mechanism |
| :--- | :--- | :--- | :--- |
| **Explorer Context Menu** | Yes | Yes | Full native Windows Shell context menu integration for all files and folders. |
| **Recycle Bin Deletion** | Yes (`Del`) | Yes (`Del`) | Standard shell deletion to Windows Recycle Bin with undo capability. |
| **Permanent Deletion** | Yes (`Shift + Del`) | Yes (`Shift + Del`) | Bypasses Recycle Bin. Prompts user confirmation. |
| **Bulk Move / Archive** | No built-in bulk mover | Yes | Move or copy selected files to alternate destinations, preserving folder hierarchies. |
| **Compression** | No built-in compression | Yes | Apply NTFS compression or archive files into ZIP / 7z containers directly. |
| **Deduplication via Hard Links** | No (manual deletion only) | Yes (Personal/Pro) | Replaces duplicate file instances with NTFS hard links, keeping one physical data instance and reclaiming space instantly without breaking application paths. |
| **Batch Selection Rules** | Limited | Yes | Batch check/uncheck helpers (e.g., "Select all but newest", "Select all in folder", "Select by type"). |
| **Bulk File Rename** | No | Yes | Built-in regular expression and template bulk rename utility. |
| **Custom Script / Tool Execution** | "Open Command Prompt / PowerShell here" | Yes (Custom Actions) | Configurable command-line triggers, scripts, and third-party utility integrations. |

---

## 8. Reporting, Exports, & Automation

### 8.1 Export Formats
- **WizTree**:
  - **CSV / TSV**: `Ctrl + Alt + E` exports current view or full hierarchy (*File Name, Size, Allocated, Modified, Attributes, Files, Folders*) ([WizTree Guides: CSV](https://diskanalyzer.com/guide)).
  - **Indented Clipboard**: `Ctrl + Alt + C` copies formatted plaintext hierarchical tree.
  - **Treemap Image**: PNG export via GUI or CLI.
  - **MFT Dump**: Binary Master File Table dump for forensic and debugging analysis.
- **TreeSize (Personal/Pro)**:
  - **Microsoft Excel (`.xlsx`)**: Multi-tab formatted workbooks with collapsible tree rows, formatting, and charts ([TreeSize Manual: Export](https://manuals.jam-software.com/treesize/EN/details.html)).
  - **PDF & HTML**: Styled reports including embedded charts, breakdowns, and summaries.
  - **XML / SQLite**: Structured index format enabling offline reloads and automated scan comparisons.
  - **CSV & Plaintext**: Formatted or raw delimiter data.
  - **Email Delivery**: Automated direct SMTP sending of generated reports.

### 8.2 Command-Line Interfaces (CLI) & Automation

#### WizTree CLI Parameters
```cmd
wiztree64.exe "<drive_or_folder>" [options]
```
- `/export="<filename.csv>"` — Headless export to CSV file (supports `%d` for YYYYMMDD, `%t` for HHMMSS).
- `/exportfiletypes="<filename.csv>"` — Aggregated extension summary export.
- `/dumpmft="<filename.bin>"` — Dumps raw NTFS MFT to file.
- `/treemapimagefile="<filename.png>"` (with `/treemapimagewidth`, `/treemapimageheight`).
- `/filter="<pattern>"`, `/filterexclude="<pattern>"`, `/filterfullpath=0|1`.
- `/sortby=0|1|2|3` (0: Name, 1: Size desc, 2: Allocated desc, 3: Date desc).
- `/exportfolders=0|1`, `/exportfiles=0|1`, `/exportmftrecno=0|1`, `/exportUTCTime=0|1`.
- `/admin=0|1` — Explicit elevation control.
- `/supportercode=<key>` — Silent enterprise license registration ([WizTree Guides](https://diskanalyzer.com/guide), [WizTree Command Line](https://diskanalyzer.com/guide)).

#### TreeSize CLI Parameters
```cmd
TreeSize.exe /SCAN "<path>" [options]
```
- `/NOGUI` — Headless execution mode.
- `/EXCEL "<file.xlsx>"`, `/CSV "<file.csv>"`, `/PDF "<file.pdf>"`, `/HTML "<file.html>"`, `/XML "<file.xml>"`, `/SQLITE "<file.db>"`.
- `/EMAIL "<recipient>"` — Sends generated report via configured mail server.
- `/COMPARE "<baseline.xml>"` — Generates growth/differential report against prior snapshot.
- `/FILTER "<pattern>"`, `/EXCLUDE "<pattern>"`.
- `/EXPAND "<level>"`, `/DEPTH "<level>"`.
- **Task Scheduler Wizard**: Integrated GUI wizard to schedule recurring background reports via Windows Task Scheduler ([TreeSize Manual: CLI](https://manuals.jam-software.com/treesize/EN/command_line_options.html), [TreeSize Scheduler](https://manuals.jam-software.com/treesize/EN/scheduler_advanced.html)).

---

## 9. Edition & Licensing Breakdown

| Product / Edition | Price Model | Intended Market | Distinct Capabilities & Licensing Rules |
| :--- | :--- | :--- | :--- |
| **WizTree Free** | Free for Personal Use | Home users | Full core scanning and GUI features. Displays "Donate" button and adds donation notice to exported CSVs. Prohibits commercial use. |
| **WizTree Supporter / Commercial** | Tiered by org staff ($19.95+ up to 100 staff) | Small/Medium Businesses, commercial users | Removes donation banner/footer. Valid for 1 year of updates (50% renewal discount). |
| **WizTree Enterprise** | Fixed site/multi-site ($299.95+) | Large organizations (>100 staff) | Unlimited PC deployments, silent CLI installation and licensing (`/supportercode=`), priority support. |
| **TreeSize Free** | Free (Permits commercial use) | Home / lightweight desktop | Basic tree & treemap, basic PDF report. Lacks NTFS ADS/hardlinks, domain share scans, scheduled tasks, and advanced exports. |
| **TreeSize Personal** | Perpetual per-seat (~$24.95) | Freelancers & Power Users | Local & mapped drives, top 100 files, duplicate search, historical XML comparison, full Excel/HTML exports. |
| **TreeSize Professional** | Perpetual per-seat (~$59.95+) | Enterprise IT & Sysadmins | Domain network shares, cloud storage (SharePoint, Azure, S3), SSH scanning, CLI automation, task scheduling, MD5/SHA256 deduplication + hardlink replacement, NTFS ACL permissions analysis. |
| **TreeSize Consultant** | Perpetual portable (~$119.95+) | IT Consultants & MSPs | Portable execution across third-party client systems without per-site installation. |

---

## 10. Defensible Parity Baseline for PigTree

To achieve a defensible market position as an open-source, modern, accessible Windows disk-space analyzer, PigTree must structure its capability boundary intentionally:

### 10.1 Must-Have Parity Core (Phase 1 Baseline)
1. **Dual-Engine Scanning**:
   - Elevated ultra-fast NTFS MFT parser (`$MFT`) for local volumes matching WizTree's scan speed.
   - Non-elevated standard Win32 directory traversal fallback with transparent progress reporting.
2. **Accurate Disk-Space Information Model**:
   - Clear distinction between *Logical File Size* and *Allocated Physical Size (Size on Disk)*.
   - Cluster slack awareness and compression/sparse allocation handling.
   - Correct hard link tracking to prevent inflated duplicate reporting in `WinSxS`.
3. **Synchronized Dual Representation**:
   - High-density Directory Tree view + Flat File View table + Interactive Hierarchical Treemap.
   - Real-time bi-directional cross-selection between tree and visualizer.
4. **Rich In-Memory Filtering & Search**:
   - Sub-millisecond instant filtering over millions of indexed nodes.
   - Support for wildcards, Boolean operators (`AND`/`OR`), size ranges (`>100MB`), date ranges, and path masks.
5. **Guarded Local Cleanup**:
   - Full Windows Shell context menu integration.
   - Safe Recycle Bin deletion (`Del`) with prompt and fallback to Permanent Deletion (`Shift + Del`).
6. **Headless Automation & Clean CLI Exports**:
   - CLI commands for headless scanning and machine-readable exports (JSON, CSV).
   - Zero nagware, zero commercial gating, zero telemetry.

### 10.2 High-Value PigTree Differentiators (Exceeding Competitors)
1. **Accessibility First**: Screen reader support (UI Automation), full keyboard operability, WCAG high-contrast themes, scalable typography (TreeSize and WizTree have known accessibility limitations in dense custom views).
2. **Unified Open Engine & CLI**: Shared core engine powering both desktop GUI and scriptable CLI workflows.
3. **Cryptographic Checksum Deduplication with Hardlink Replacement**: Free and built-in, avoiding TreeSize's commercial paywall.
4. **Privacy & Local Invariance**: Zero telemetry, no cloud dependencies, guarantees that file paths and metadata never leave the host.

### 10.3 Explicit Non-Goals / Phased Enterprise Capabilities
- **Phase 1 Exclusions**: Direct cloud API scanning (SharePoint, Azure Blob, S3), remote SSH traversal, Active Directory domain-wide permission audits, and continuous SQL database background tracking (SpaceObServer scope). Focus first on local Windows fixed, removable, and standard SMB/mapped storage.

---

## 11. Ambiguities, Access Limitations, & Unresolved Gaps

1. **Closed-Source MFT Parser Optimization**:
   - *Fact*: WizTree's raw MFT parser achieved high benchmark speeds through proprietary C++ parsing and in-memory indexing routines.
   - *Limitation/Gap*: Internal record parsing strategies, concurrency handling during `$MFT` reading, and specific memory compacting techniques are not documented beyond high-level descriptions. PigTree will need to benchmark its own MFT parsing engine against documented NTFS structures (`$MFT` record layout, `$DATA` runs, `$INDEX_ROOT` / `$INDEX_ALLOCATION`).
2. **Proprietary Cloud & Remote Protocol Connectors**:
   - *Fact*: TreeSize Professional implements proprietary connectors for SharePoint Graph API, Azure Blob REST API, and Amazon S3.
   - *Limitation/Gap*: Exact API rate-limit throttling and permission management in TreeSize are closed-source. These are intentionally out of scope for PigTree's local-first Phase 1.
3. **IFilter Full-Text Indexing Edge Cases**:
   - *Fact*: TreeSize leverages Windows IFilter APIs for custom in-file text searches.
   - *Limitation/Gap*: Behavior when encountering corrupt office documents or long-tail file locks is proprietary and unstated.

---

## 12. Primary Source Citations

1. **Antibody Software (WizTree)**:
   - [WizTree User Guide & Documentation](https://diskanalyzer.com/guide)
   - [WizTree Command Line Parameters Reference](https://diskanalyzer.com/guide)
   - [WizTree Frequently Asked Questions (FAQ)](https://diskanalyzer.com/faq)
   - [WizTree Version History & Changelog](https://diskanalyzer.com/whats-new)
   - [WizTree Licensing, Pricing & Supporter Codes](https://diskanalyzer.com/donate)
   - [WizTree End User License Agreement (EULA)](https://diskanalyzer.com/eula)
2. **JAM Software (TreeSize)**:
   - [TreeSize Online User Manual](https://manuals.jam-software.com/treesize/)
   - [TreeSize Details View & Column Customization](https://manuals.jam-software.com/treesize/EN/details.html)
   - [TreeSize File Search & Duplicate Finder Documentation](https://manuals.jam-software.com/treesize/EN/file_search.html)
   - [TreeSize Command Line Options & Scheduler Reference](https://manuals.jam-software.com/treesize/EN/command_line_options.html)
   - [TreeSize Advanced Search & Custom Rules](https://manuals.jam-software.com/treesize/EN/custom_search.html)
   - [TreeSize Notes on NTFS (ADS, Hardlinks, Compression)](https://manuals.jam-software.com/treesize/EN/notes_on_ntfs.html)
   - [TreeSize Disk Usage & Snapshot Comparison](https://manuals.jam-software.com/treesize/EN/disk_usage_comparison.html)
   - [TreeSize Editions, Feature Tour & Product Overview](https://www.jam-software.com/treesize/)
   - [TreeSize vs SpaceObServer Technical Comparison](https://www.jam-software.com/treesize/compare-spaceobserver)