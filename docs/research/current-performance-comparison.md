# Current Performance Comparison Protocol: Primary-Source Facts

**Document Date:** 2026-08-28  
**Source Snapshot Date:** 2026-08-28  
**Scope:** Primary-source evidence and technical facts for establishing an honest, reproducible performance comparison protocol for PigTree against Antibody Software WizTree and JAM Software TreeSize.  
**Related Documents:** `docs/research/benchmark-evidence-and-methods.md` (Statistical foundation and measurement methodology).

---

## 1. Primary Source Landscape & Current Named Versions

To ensure fair and unassailable comparative evaluations, all benchmark regimes must reference specific, currently supported production releases and editions.

### 1.1 Antibody Software WizTree
* **Current Production Version:** WizTree 4.32 (Released 2026-08-05) [[1]](https://diskanalyzer.com/whats-new).
* **Architecture & Platforms:** Native x86 and x64 Windows builds; officially verified on Windows Arm via Prism Emulation [[1]](https://diskanalyzer.com/whats-new).
* **Editions:**
  * *Free Edition:* Free for personal, non-commercial use only. Includes animated donation prompt and notices in CSV exports [[2]](https://diskanalyzer.com/faq)[[3]](https://diskanalyzer.com/).
  * *Supporter / Commercial License:* Tiered licensing ($25–$500 by organization seat count) or Enterprise ($750 site license) removing donation prompts and enabling commercial/automated usage [[2]](https://diskanalyzer.com/faq)[[3]](https://diskanalyzer.com/).
* **Official Marketing Claims & Wording:**
  * Claims to be *"The Fastest Disk Space Analyzer"* on Windows [[3]](https://diskanalyzer.com/).
  * States that it achieves performance by *"directly reading the Master File Table (MFT) from NTFS formatted drives"* and *"completely bypasses standard Windows operating system file system routines"* [[3]](https://diskanalyzer.com/)[[4]](https://diskanalyzer.com/guide).

### 1.2 JAM Software TreeSize
* **Current Production Versions:**
  * TreeSize Free: Version 4.8.x (native 64-bit support) [[5]](https://www.jam-software.com/treesize_free)[[6]](https://www.jam-software.com/treesize_free/changes.shtml).
  * TreeSize Professional: Version 9.8.x (native 64-bit architecture) [[7]](https://www.jam-software.com/treesize)[[8]](https://www.jam-software.com/treesize/changes.shtml).
* **Editions & Segmentation:**
  * *TreeSize Free:* Targeted at home and non-commercial users. Supports basic drive and folder scanning with treemap visualization. Does not include command-line automation, advanced multithreading configuration, or permission scanning [[5]](https://www.jam-software.com/treesize_free)[[9]](https://www.jam-software.com/treesize/editions-and-pricing).
  * *TreeSize Personal:* Targeted at single-user power users/freelancers; adds duplicate search and history tracking, but lacks command-line automation and cloud storage connectors [[9]](https://www.jam-software.com/treesize/editions-and-pricing).
  * *TreeSize Professional:* Targeted at enterprise systems administrators; includes full command-line automation, configurable multithreading (up to 32 threads), custom export formats, NTFS permission analysis, and cloud storage targets (SharePoint, Amazon S3, Azure Blob) [[7]](https://www.jam-software.com/treesize)[[9]](https://www.jam-software.com/treesize/editions-and-pricing).
* **Official Marketing Claims & Wording:**
  * Claims *"Fast scanning by working directly with the Master File Table (MFT) on NTFS drives"* [[5]](https://www.jam-software.com/treesize_free).
  * Highlights multi-threaded parallel scanning across directory subtrees for non-MFT and network/cloud targets [[7]](https://www.jam-software.com/treesize)[[10]](https://manuals.jam-software.com/treesize/EN/options_general.html).

---

## 2. Feature, Privilege, and Execution Regime Differences

Fair comparison requires mapping each tool's scanning engines to identical execution regimes. A comparison that pits an elevated direct-MFT scan against a standard Win32 directory traversal is methodologically invalid.

| Dimension | Antibody Software WizTree 4.32 | JAM Software TreeSize Pro 9.8.x | JAM Software TreeSize Free 4.8.x |
| :--- | :--- | :--- | :--- |
| **Elevated Direct MFT Mode** | Supported (Default on NTFS; requires Administrator / `SeBackupPrivilege`) [[2]](https://diskanalyzer.com/faq)[[3]](https://diskanalyzer.com/) | Supported on local NTFS volumes (Requires Administrator) [[5]](https://www.jam-software.com/treesize_free)[[8]](https://www.jam-software.com/treesize/changes.shtml) | Supported on local NTFS volumes (Requires Administrator) [[5]](https://www.jam-software.com/treesize_free) |
| **Standard User Mode (Win32 / NT API)** | Fallback mode when non-elevated or passed `/admin=0` [[2]](https://diskanalyzer.com/faq)[[4]](https://diskanalyzer.com/guide) | Fallback traversal engine; used for non-NTFS, subdirectories, or non-admin [[9]](https://www.jam-software.com/treesize/editions-and-pricing)[[10]](https://manuals.jam-software.com/treesize/EN/options_general.html) | Standard Win32 traversal fallback [[5]](https://www.jam-software.com/treesize_free) |
| **Subdirectory Scanning Scope** | Non-elevated Win32 traversal on target subtree [[4]](https://diskanalyzer.com/guide) | Traversal of target subtree [[9]](https://www.jam-software.com/treesize/editions-and-pricing) | Traversal of target subtree [[5]](https://www.jam-software.com/treesize_free) |
| **Multithreading Configuration** | Internal streaming pipeline; no user thread pool configuration [[4]](https://diskanalyzer.com/guide) | Configurable scan thread count and thread CPU priority in Options [[10]](https://manuals.jam-software.com/treesize/EN/options_general.html) | Fixed internal threading [[5]](https://www.jam-software.com/treesize_free)[[9]](https://www.jam-software.com/treesize/editions-and-pricing) |
| **Command-Line Interface (CLI)** | Full CLI support across 32-bit and 64-bit binaries [[4]](https://diskanalyzer.com/guide) | Full CLI support (Professional Edition only) [[11]](https://manuals.jam-software.com/treesize/EN/command_line_options.html) | **No CLI Support** (GUI interactive only) [[9]](https://www.jam-software.com/treesize/editions-and-pricing) |
| **Raw MFT Image Dump** | Dedicated `/dumpmft` CLI switch [[4]](https://diskanalyzer.com/guide) | No standalone MFT dump CLI switch | No CLI support |
| **Non-NTFS / Network Targets** | Win32 traversal for FAT32, exFAT, ReFS, and SMB shares [[3]](https://diskanalyzer.com/)[[4]](https://diskanalyzer.com/guide) | Native scanning for FAT, exFAT, ReFS, SMB, SharePoint, S3, Azure [[7]](https://www.jam-software.com/treesize)[[9]](https://www.jam-software.com/treesize/editions-and-pricing) | Basic local and network traversal [[5]](https://www.jam-software.com/treesize_free)[[9]](https://www.jam-software.com/treesize/editions-and-pricing) |

---

## 3. Command-Line Automation Reference

Automated benchmark harnesses must invoke competitor tools using their documented command-line parameters.

### 3.1 WizTree CLI Interface
Official syntax and switches documented in the WizTree User Guide [[4]](https://diskanalyzer.com/guide):
```cmd
wiztree64.exe "<drive_or_path>" /export="<outfile.csv>" [/filter="<spec>"] [/filterexclude="<spec>"] [/sortby=0|1|2|3|4|5] [/sortdir=0|1] [/admin=0|1]
```
* `<drive_or_path>`: Drive letter (`C:`), path (`C:\Folder`), or relative path (e.g. `.` or `..` supported in v4.32+) [[1]](https://diskanalyzer.com/whats-new).
* `/export="<outfile.csv>"`: Exports scanned file data to CSV format and terminates.
* `/admin=0|1`: Controls whether WizTree attempts administrative elevation (`1` default for MFT) or runs in standard user mode (`0`) [[2]](https://diskanalyzer.com/faq)[[4]](https://diskanalyzer.com/guide).
* `/sortby=0|1|2|3|4|5`: Controls sort column (0 = Size, 1 = Allocated, 2 = Modified, 3 = Files, 4 = Folders, 5 = Percent) [[4]](https://diskanalyzer.com/guide).
* `/dumpmft="<outfile>"`: Dumps raw NTFS MFT records directly to a file (supports `%d` date and `%t` time specifiers) [[4]](https://diskanalyzer.com/guide).

### 3.2 TreeSize Professional CLI Interface
Official syntax and switches documented in the TreeSize Professional Manual [[11]](https://manuals.jam-software.com/treesize/EN/command_line_options.html):
```cmd
TreeSize.exe [/OPTION] [SCANPATH]
```
* `/SCAN <path>`: Target drive, folder, or UNC path to scan.
* `/OPTIONS <options.xml>`: Loads pre-configured scan options (thread count, hardlink tracking, filters) exported from the GUI [[10]](https://manuals.jam-software.com/treesize/EN/options_general.html)[[11]](https://manuals.jam-software.com/treesize/EN/command_line_options.html).
* `/NOGUI`: Runs silently without displaying the user interface window.
* `/CSV <outfile.csv>` / `/EXCEL <outfile.xlsx>` / `/TEXT <outfile.txt>`: Specifies export target format and file path [[11]](https://manuals.jam-software.com/treesize/EN/command_line_options.html).
* `/RESTRICTED` & `/READONLY`: Starts in restricted execution mode [[11]](https://manuals.jam-software.com/treesize/EN/command_line_options.html).
* *Note on TreeSize Free:* Because TreeSize Free does not support command-line arguments [[9]](https://www.jam-software.com/treesize/editions-and-pricing), automated benchmark runners must evaluate TreeSize via TreeSize Professional.

---

## 4. Licensing and Trial Constraints for Automated Benchmarking

Benchmarking automation in continuous integration (CI) or automated test rigs must comply with vendor licensing terms.

1. **Antibody Software WizTree:**
   * Free edition is strictly licensed for *personal, non-commercial home use* [[2]](https://diskanalyzer.com/faq)[[3]](https://diskanalyzer.com/).
   * Commercial and enterprise evaluation, including automated test pipelines on company infrastructure, requires purchasing a Supporter Code or Enterprise License [[2]](https://diskanalyzer.com/faq)[[3]](https://diskanalyzer.com/).
   * Supporter codes remain valid for all versions released within one year of purchase [[2]](https://diskanalyzer.com/faq).
2. **JAM Software TreeSize:**
   * TreeSize Free is strictly licensed for *non-commercial, private use* [[9]](https://www.jam-software.com/treesize/editions-and-pricing).
   * TreeSize Professional provides a 30-day evaluation trial for commercial evaluation [[7]](https://www.jam-software.com/treesize). Permanent automated execution in institutional benchmark harnesses requires commercial licensing [[9]](https://www.jam-software.com/treesize/editions-and-pricing).

---

## 5. Scan Settings and Accounting Configuration Parameters

Variations in scan accounting settings introduce dramatic performance deltas and must be explicitly recorded for every benchmark run:

1. **NTFS Hard Link Tracking:**
   * *TreeSize:* Configured via *Options > General scan options > Scan Accuracy > Track NTFS hard links* [[10]](https://manuals.jam-software.com/treesize/EN/options_general.html). Tracking hard links requires tracking inode/FileID references in memory to prevent double-counting physical disk allocation.
   * *WizTree:* Automatically exposes *Size* (sum of file logical sizes) and *Allocated Space* (cluster allocation) [[4]](https://diskanalyzer.com/guide).
2. **Alternate Data Streams (ADS):**
   * *TreeSize:* Configured via *Options > General scan options > Scan Accuracy > Track Alternate Data Streams* [[10]](https://manuals.jam-software.com/treesize/EN/options_general.html). Scanning ADS adds significant overhead via extra filesystem stream enumeration syscalls.
3. **Cloud & Dehydrated Offline Files:**
   * *TreeSize:* Option *Skip offline files* in search and scan settings prevents unintended hydration of cloud placeholders (OneDrive / SharePoint) [[10]](https://manuals.jam-software.com/treesize/EN/options_general.html).
4. **Export Serialization Isolation:**
   * CLI benchmarks exporting to CSV on the same physical disk being scanned introduce storage write contention. All CLI benchmark output paths must be directed to a dedicated high-speed RAM disk or isolated temporary drive.

---

## 6. Microsoft Performance Instrumentation, Caching, and Power Standards

Measurement harness design must rely on official Windows performance and system APIs:

1. **Memory Working Set & System Cache Control:**
   * `SetSystemFileCacheSize`: Win32 API function (`memoryapi.h`) to configure the working set limits of the system file cache. Requires `SE_INCREASE_QUOTA_NAME` privilege [[12]](https://learn.microsoft.com/en-us/windows/win32/api/memoryapi/nf-memoryapi-setsystemfilecachesize).
   * Standby list purging via low-level NT API `NtSetSystemInformation(SystemMemoryListInformation, ...)` or Sysinternals `RAMMap.exe -empty standby` [[12]](https://learn.microsoft.com/en-us/windows/win32/api/memoryapi/nf-memoryapi-setsystemfilecachesize).
2. **Windows Power Scheme Control:**
   * `powercfg.exe`: Built-in Windows utility to manage active power schemes. Benchmarks must enforce the *High Performance* or *Ultimate Performance* power scheme via `powercfg /setactive <Scheme_GUID>` to eliminate CPU dynamic frequency scaling noise [[13]](https://learn.microsoft.com/en-us/windows-hardware/design/device-experiences/powercfg-command-line-options).
3. **ETW Tracing & Kernel Provider Attribution:**
   * **Windows Performance Recorder (`wpr.exe`):** Pre-installed CLI tool for ETW kernel event recording [[14]](https://learn.microsoft.com/en-us/windows-hardware/test/wpt/windows-performance-recorder)[[15]](https://learn.microsoft.com/en-us/windows-hardware/test/wpt/wpr-command-line-options).
   * **Kernel Storage Providers:** System keywords `DiskIO`, `DiskIOInit`, `FileIO`, and `FileIOInit` in WPR profiles capture precise kernel-level I/O operations and disk service times [[16]](https://learn.microsoft.com/en-us/windows-hardware/test/wpt/keyword-in-systemprovider).
   * **Windows Performance Analyzer (`wpa.exe`):** Official ADK tool for visualizing and analyzing recorded `.etl` trace files [[17]](https://learn.microsoft.com/en-us/windows-hardware/test/wpt/windows-performance-analyzer).

---

## 7. Decision inputs, not targets

This section explicitly separates external verified facts from PigTree's architectural decisions and performance targets:

### 7.1 Verified External Facts (Decision Inputs)
1. **Privilege Boundary:** Elevated MFT direct reading is restricted to local NTFS partitions; non-elevated user access and non-NTFS volumes (FAT32, exFAT, ReFS, network shares) strictly require directory traversal APIs.
2. **Competitor CLI Availability:** TreeSize Free lacks any CLI interface, requiring TreeSize Professional for automated testing. WizTree provides full CLI automation across both Free and licensed builds.
3. **Licensing Constraints:** Automated commercial testing of WizTree and TreeSize requires supporter/commercial licensing.
4. **Feature Options Overhead:** Enabling hardlink deduplication and ADS stream enumeration measurably increases scan duration and memory footprint.
5. **Operating System Cache Behavior:** Cold-cache testing requires explicit administrative flushing of the Windows Standby List and System File Cache.

### 7.2 PigTree Target Choices (Design & Performance Goals)
1. **Regime Parity:** PigTree must implement and expose separate, dedicated scan engines for elevated direct-MFT scanning and non-elevated multi-threaded Win32/NT directory traversal.
2. **Transparent Accounting:** PigTree will clearly distinguish and present both *Logical Apparent Size* (uncompressed sum across all directory hard links) and *Physical Allocated Size* (cluster allocation deduplicated by FileID).
3. **No Unintended Hydration:** PigTree will enforce `FILE_FLAG_OPEN_REPARSE_POINT` across all traversal paths to guarantee zero hydration of dehydrated cloud files (OneDrive / iCloud).
4. **First-Class CLI & Automation:** PigTree will provide a native, scriptable CLI with structured JSON and CSV outputs, independent of commercial license tiers.
5. **Reproducible Benchmark Suite:** PigTree's official benchmark reports will publish complete environment disclosures, raw ETW I/O counters, and non-parametric statistical metrics (Medians, IQR, 95% Bootstrap CIs).

---

## 8. Primary Source Citations & References

* <a id="ref-1"></a>**[1] Antibody Software.** (2026). *WizTree What's New & Changelog (Version 4.32)*. Available at: [https://diskanalyzer.com/whats-new](https://diskanalyzer.com/whats-new)
* <a id="ref-2"></a>**[2] Antibody Software.** (2026). *WizTree Frequently Asked Questions (FAQ)*. Available at: [https://diskanalyzer.com/faq](https://diskanalyzer.com/faq)
* <a id="ref-3"></a>**[3] Antibody Software.** (2026). *WizTree - The Fastest Disk Space Analyzer (Official Product Page)*. Available at: [https://diskanalyzer.com/](https://diskanalyzer.com/)
* <a id="ref-4"></a>**[4] Antibody Software.** (2026). *WizTree User Guide and Command Line Reference*. Available at: [https://diskanalyzer.com/guide](https://diskanalyzer.com/guide)
* <a id="ref-5"></a>**[5] JAM Software.** (2026). *TreeSize Free Product Overview*. Available at: [https://www.jam-software.com/treesize_free](https://www.jam-software.com/treesize_free)
* <a id="ref-6"></a>**[6] JAM Software.** (2026). *TreeSize Free Changelog & Version History*. Available at: [https://www.jam-software.com/treesize_free/changes.shtml](https://www.jam-software.com/treesize_free/changes.shtml)
* <a id="ref-7"></a>**[7] JAM Software.** (2026). *TreeSize Professional Product Overview*. Available at: [https://www.jam-software.com/treesize](https://www.jam-software.com/treesize)
* <a id="ref-8"></a>**[8] JAM Software.** (2026). *TreeSize Professional Changelog & Version History*. Available at: [https://www.jam-software.com/treesize/changes.shtml](https://www.jam-software.com/treesize/changes.shtml)
* <a id="ref-9"></a>**[9] JAM Software.** (2026). *TreeSize Editions & Feature Comparison Matrix*. Available at: [https://www.jam-software.com/treesize/editions-and-pricing](https://www.jam-software.com/treesize/editions-and-pricing)
* <a id="ref-10"></a>**[10] JAM Software.** (2026). *TreeSize Manual: General Scan Options & Scan Accuracy*. Available at: [https://manuals.jam-software.com/treesize/EN/options_general.html](https://manuals.jam-software.com/treesize/EN/options_general.html)
* <a id="ref-11"></a>**[11] JAM Software.** (2026). *TreeSize Manual: Command Line Options & Automated Reporting*. Available at: [https://manuals.jam-software.com/treesize/EN/command_line_options.html](https://manuals.jam-software.com/treesize/EN/command_line_options.html)
* <a id="ref-12"></a>**[12] Microsoft Learn.** (2023). *SetSystemFileCacheSize function (memoryapi.h)*. Available at: [https://learn.microsoft.com/en-us/windows/win32/api/memoryapi/nf-memoryapi-setsystemfilecachesize](https://learn.microsoft.com/en-us/windows/win32/api/memoryapi/nf-memoryapi-setsystemfilecachesize)
* <a id="ref-13"></a>**[13] Microsoft Learn.** (2023). *Powercfg command-line options*. Available at: [https://learn.microsoft.com/en-us/windows-hardware/design/device-experiences/powercfg-command-line-options](https://learn.microsoft.com/en-us/windows-hardware/design/device-experiences/powercfg-command-line-options)
* <a id="ref-14"></a>**[14] Microsoft Learn.** (2023). *Windows Performance Recorder (WPR)*. Available at: [https://learn.microsoft.com/en-us/windows-hardware/test/wpt/windows-performance-recorder](https://learn.microsoft.com/en-us/windows-hardware/test/wpt/windows-performance-recorder)
* <a id="ref-15"></a>**[15] Microsoft Learn.** (2023). *WPR Command-Line Options*. Available at: [https://learn.microsoft.com/en-us/windows-hardware/test/wpt/wpr-command-line-options](https://learn.microsoft.com/en-us/windows-hardware/test/wpt/wpr-command-line-options)
* <a id="ref-16"></a>**[16] Microsoft Learn.** (2023). *Keyword (in SystemProvider) - WPT Technical Reference*. Available at: [https://learn.microsoft.com/en-us/windows-hardware/test/wpt/keyword-in-systemprovider](https://learn.microsoft.com/en-us/windows-hardware/test/wpt/keyword-in-systemprovider)
* <a id="ref-17"></a>**[17] Microsoft Learn.** (2023). *Windows Performance Analyzer (WPA)*. Available at: [https://learn.microsoft.com/en-us/windows-hardware/test/wpt/windows-performance-analyzer](https://learn.microsoft.com/en-us/windows-hardware/test/wpt/windows-performance-analyzer)
