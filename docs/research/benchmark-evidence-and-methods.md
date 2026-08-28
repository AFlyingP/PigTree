# Benchmark Evidence and Methods for Windows Disk Analyzers

**Author:** PigTree Core Team
**Date:** March 2025
**Topic:** Reproducible, High-Fidelity Benchmarking Methodology for PigTree, WizTree, and TreeSize
**Status:** Research Note (Decision-Complete Methodology)
**Related Issue:** [#6](https://github.com/AFlyingP/PigTree/issues/6)
**Primary Standards & Citations:** ACM SIGPLAN Empirical Evaluation Guidelines [[1]](#ref-sigplan); Hoefler & Belli (SC15) [[2]](#ref-hoefler); Georges et al. (OOPSLA'07) [[3]](#ref-georges); Microsoft Windows Systems Architecture & Win32/NT API [[4]](#ref-ms-findfirst)[[5]](#ref-ms-reparse)[[6]](#ref-ms-filecache)[[7]](#ref-ms-etw)[[8]](#ref-ms-memory); JAM Software TreeSize Documentation [[9]](#ref-treesize-modes)[[10]](#ref-treesize-cli); Antibody Software WizTree Documentation [[11]](#ref-wiztree-mft)[[12]](#ref-wiztree-cli).

---

## 1. Executive Summary & Benchmark Philosophy

Disk-space analyzers operate at the intersection of raw storage throughput, Windows kernel I/O, NTFS/ReFS metadata structures, complex directory graphs, in-memory tree indexing, and interactive UI visualization. Historical performance marketing in this software category frequently relies on misleading universal claims (such as "Scans any drive in 1 second!" or "10x faster than competitor X"). Such claims collapse fundamental architectural distinctions, including:
1. **Privilege & Access Regimes:** Direct Master File Table (MFT) raw volume parsing (Administrator required) versus recursive Win32 directory traversal (Standard User / non-elevated).
2. **Caching Regimes:** Cold storage retrieval (disk I/O bound) versus warm OS standby list / file system cache traversal (CPU/memory-bandwidth bound).
3. **Filesystem & Storage Topology:** Local NVMe SSDs versus legacy spinning hard disks (HDDs) versus high-latency SMB/UNC network shares.
4. **Information Density & Accuracy:** Surface-level directory size rollups versus complete accounting of hard links, NTFS compression, sparse files, reparse points, Alternate Data Streams (ADS), and Cloud Files placeholders.

This research note defines an **evidence-based, statistically sound, and fully reproducible benchmarking methodology** for comparing PigTree, Antibody Software WizTree, and JAM Software TreeSize. It establishes exact measurement definitions, workload archetypes, system control protocols, resource counters, and statistical reporting standards, ensuring that PigTree's engineering decisions and published benchmarks remain methodologically unassailable.

---

## 2. Scanning & Privilege Regimes

Benchmarking disk analyzers requires decomposing scanning operations into distinct architectural regimes. Comparing an analyzer running in an elevated direct-MFT mode against another running in a non-elevated recursive Win32 mode produces invalid, non-commensurable data.

```
+---------------------------------------------------------------------------------------------------+
|                                   Windows Disk Analysis Regimes                                   |
+---------------------------------------------------+-----------------------------------------------+
| Regime A: Elevated Direct Metadata Access        | Regime B: Standard User / Directory Traversal |
| - Administrator / SeBackupPrivilege               | - Standard User (Medium Integrity)            |
| - Raw Volume Handle (\\.\C:)                     | - Win32 API / NT Syscalls                     |
| - Sequential $MFT / USN Journal Read              | - FindFirstFileExW / NtQueryDirectoryFile     |
| - Bypasses directory-by-directory traversal       | - Recursive directory tree traversal          |
| - Bypasses per-folder DACLs                       | - Subject to ACL security checks & locks      |
| - NTFS volume-wide only                           | - Any filesystem (NTFS, ReFS, exFAT, SMB)     |
+---------------------------------------------------+-----------------------------------------------+
```

### 2.1 Regime A: Elevated Direct Metadata Access (MFT / Raw Volume)
* **Mechanisms:** The application requests administrative elevation (requireAdministrator or runtime UAC prompt), opens a direct handle to the raw volume (e.g. `CreateFileW(L"\\\\.\\C:", GENERIC_READ, ...)`), queries volume geometry via `FSCTL_GET_NTFS_VOLUME_DATA`, retrieves retrieval extents of `$MFT` via `FSCTL_GET_RETRIEVAL_POINTERS`, and streams Master File Table records sequentially in large block buffers (64 KB to 4 MB) [[4]](#ref-ms-findfirst)[[11]](#ref-wiztree-mft).
* **Characteristics:** Throughput is bounded almost purely by sequential disk read speed (cold) or memory bandwidth (warm) and multi-core record parsing efficiency. Directory hierarchy reconstruction occurs in user-mode memory after reading record parent IDs.
* **Scope Constraints:** Restricted to local NTFS volumes. Cannot scan individual subfolders without parsing the entire volume MFT, and cannot be used over SMB network shares, FAT32/exFAT partitions, or ReFS volumes (which do not expose a classic `$MFT` table).

### 2.2 Regime B: Standard User / Win32 & NT Directory Traversal
* **Mechanisms:** The application executes under standard user rights (Medium Integrity level) without UAC prompts. It traverses the filesystem hierarchy recursively using:
  * Standard Win32: `FindFirstFileExW` configured with `FindExInfoBasic` (skipping legacy 8.3 short names) and `FIND_FIRST_EX_LARGE_FETCH` (requesting 64 KB internal directory buffers) [[4]](#ref-ms-findfirst).
  * Low-level NT syscalls: `NtQueryDirectoryFile` / `NtQueryDirectoryFileEx` using the `FileIdBothDirectoryInformation` (class 37) information class, or `GetFileInformationByHandleEx` with `FileIdBothDirectoryRestartInfo` [[4]](#ref-ms-findfirst).
* **Characteristics:** Execution time is heavily dominated by random I/O seek latency (on HDDs), syscall/context switch frequency, kernel-to-user buffer copying, and Windows Security Reference Monitor (SRM) access checks across directory nodes.
* **Scope Constraints:** Universal across all filesystems (NTFS, ReFS, exFAT, FAT32, UDF), individual subdirectories, and network shares (SMB/NFS).

### 2.3 Benchmark Rule: Strict Regime Isolation
All comparative evaluations must strictly segregate and label measurements by regime:
* **Elevated MFT Comparison:** WizTree (Admin mode) vs. TreeSize (MFT mode) vs. PigTree (Elevated MFT engine).
* **Standard User Traversal Comparison:** WizTree (`/admin=0`) vs. TreeSize (Win32 mode, single-thread & multi-thread) vs. PigTree (Standard User engine).
* **Subdirectory Scope Comparison:** Scanning targeted directory trees (e.g., `C:\Users\Username` or `C:\Program Files`) where volume-wide MFT scanning is either prohibited or inefficient.

---

## 3. Representative Datasets & Structural Edge Cases

A robust benchmark suite must test across distinct synthetic and real-world file trees designed to stress specific performance and algorithmic boundaries.

```
+---------------------------------------------------------------------------------------------------+
|                                 Benchmark Test Suite Workloads                                    |
+-------------------+--------------------+------------------------+---------------------------------+
| Workload Suite    | Total Files/Dirs   | Primary Characteristics| Target Stress Point             |
+-------------------+--------------------+------------------------+---------------------------------+
| 1. Small-File Dev | 1,000,000+ files   | < 4 KB size, node_mods,| Syscall overhead, tree depth,   |
|    Tree (Deep)    | 200,000 dirs       | git objects, cargo src | memory overhead per node        |
+-------------------+--------------------+------------------------+---------------------------------+
| 2. Big-Data /     | 50,000 files       | 500 MB to 50 GB files, | 64-bit size aggregation,        |
|    Media Volume   | 2,000 dirs         | multi-TB total size    | sparse/compact accounting       |
+-------------------+--------------------+------------------------+---------------------------------+
| 3. Windows System | 500,000+ files     | WinSxS hardlinks, WOF, | Hard link deduplication,        |
|    Drive (Real)   | 80,000 dirs        | junctions, cloud stubs | reparse points, locked files    |
+-------------------+--------------------+------------------------+---------------------------------+
| 4. Path & Name    | 50,000 files       | Paths > 260 chars,     | MAX_PATH boundaries, \\\\?\\       |
|    Edge Cases     | 10,000 dirs        | Unicode, ADS, sparse   | prefix, stream enumeration      |
+-------------------+--------------------+------------------------+---------------------------------+
| 5. Multi-Million  | 10,000,000+ files  | Massive enterprise     | Scalability of memory model,    |
|    Scale Test     | 1,000,000 dirs     | storage volume         | indexing time, UI virtualization|
+-------------------+--------------------+------------------------+---------------------------------+
```

### 3.1 Structural Edge Cases & Correctness Criteria
Benchmark tools must verify not only elapsed time but also **semantic correctness and accounting accuracy**:

1. **Hard Links (`nNumberOfLinks > 1`):**
   * *Mechanism:* In NTFS, multiple directory entries (hard links) point to the same MFT record number / `FileId` [[4]](#ref-ms-findfirst)[[5]](#ref-ms-reparse). Common in `C:\Windows\WinSxS`.
   * *Correctness Requirement:* The benchmark harness must verify whether analyzers double-count physical disk allocation. Analyzers must report both *Logical Apparent Size* (sum of all link references) and *Physical Allocated Size* (counting unique `FileId` blocks exactly once).
2. **Reparse Points & Directory Junctions:**
   * *Tags:* `IO_REPARSE_TAG_MOUNT_POINT` (junctions), `IO_REPARSE_TAG_SYMLINK` (symbolic links), `IO_REPARSE_TAG_APPEXECLINK` (UWP execution aliases) [[5]](#ref-ms-reparse).
   * *Correctness Requirement:* The analyzer must not enter recursive infinite loops across circular junctions. It must allow configurable traversal policies and clearly differentiate junction link size from target directory subtree size.
3. **Cloud Files / Dehydrated Placeholders (OneDrive / Cloud Sync):**
   * *Tags:* `IO_REPARSE_TAG_FILE_PLACEHOLDER`, `IO_REPARSE_TAG_CLOUD` (`FILE_ATTRIBUTE_REPARSE_POINT` combined with `FILE_ATTRIBUTE_OFFLINE`) [[5]](#ref-ms-reparse).
   * *Correctness Requirement:* Scanning must **never** trigger file hydration (downloading gigabytes of data from the cloud). Traversal must specify `FILE_FLAG_OPEN_REPARSE_POINT` [[5]](#ref-ms-reparse). Physical on-disk allocation (`AllocationSize` = 0 or block header size) must be distinguished from remote logical size (`EndOfFile`).
4. **Compression, Sparse Files, and CompactOS (WOF):**
   * *Attributes & Tags:* `FILE_ATTRIBUTE_SPARSE_FILE`, `FILE_ATTRIBUTE_COMPRESSED`, `IO_REPARSE_TAG_WOF` (Windows Overlay Filter utilizing XPRESS4K/LZX compression) [[5]](#ref-ms-reparse).
   * *Correctness Requirement:* Analyzers must distinguish *Size* (`EndOfFile` / uncompressed byte count) from *Allocated Size on Disk* (`GetCompressedFileSizeW` / cluster allocations).
5. **Alternate Data Streams (ADS):**
   * *Mechanism:* Additional named streams attached to NTFS file records (e.g., `file.txt:Zone.Identifier` or antivirus metadata).
   * *Measurement Criterion:* Evaluated via `FindFirstStreamW` / `FindNextStreamW` or MFT stream attributes. Benchmark tests must note whether ADS discovery is enabled and measure the corresponding performance delta.
6. **Path Lengths Exceeding `MAX_PATH`:**
   * *Mechanism:* Hierarchies deeper than 260 characters (`\\?\\ ` extended-length prefix and `longPathAware` process manifest) [[4]](#ref-ms-findfirst).
   * *Correctness Requirement:* The scanner must complete without truncation, buffer overflow, or skipped subtrees.

---

## 4. Hardware, Caching, and Environmental Controls

Benchmarking storage software is notoriously vulnerable to uncontrolled hardware and operating system noise. The benchmark protocol mandates strict environmental isolation.

### 4.1 Storage Hardware Tiers
Measurements must be replicated across three standard storage tiers:
1. **Tier 1 (High-Performance NVMe):** PCIe Gen4/Gen5 NVMe M.2 SSD (e.g., Samsung 990 Pro, WD Black SN850X). Characterized by >1,000,000 random read IOPS and multi-gigabyte/sec sequential throughput.
2. **Tier 2 (SATA SSD):** Standard SATA III SSD (e.g., Crucial MX500). Characterized by ~90,000 IOPS and ~550 MB/s bus limit.
3. **Tier 3 (Mechanical HDD):** 7200 RPM SATA Hard Disk Drive. Characterized by 75–150 IOPS and severe 10–15 ms mechanical seek latencies. Crucial for measuring the real-world impact of random directory traversal versus sequential MFT reads.
4. **Tier 4 (Network SMB):** 1 Gbps and 10 Gbps SMB 3.1.1 shares with measured round-trip latencies (0.5 ms to 20 ms).

### 4.2 Operating System Caching Regimes: Cold vs. Warm

```
+---------------------------------------------------------------------------------------------------+
|                                 Operating System Caching States                                   |
+------------------------------------+--------------------------------------------------------------+
| Cache State                        | Description & Invalidation Mechanism                         |
+------------------------------------+--------------------------------------------------------------+
| 1. OS-Cold State                   | All file metadata, directory structures, and file pages      |
|    (Flushed Memory Cache)          | purged from the Windows Standby List and System Working Set. |
|                                    | Programmatically achieved via SetSystemFileCacheSize &       |
|                                    | NtSetSystemInformation(SystemMemoryListInformation).         |
+------------------------------------+--------------------------------------------------------------+
| 2. Hardware-Cold State             | True cold hardware state: Drive controller DRAM cache,       |
|    (Power-Cycled Device)           | SLC write/read buffers, and host-memory buffers flushed.     |
|                                    | Requires system reboot / device power cycle.                 |
+------------------------------------+--------------------------------------------------------------+
| 3. OS-Warm State                   | Metadata and directory records reside in Windows physical    |
|    (In-Memory Cache)               | RAM (Standby List / File System Cache). Traversal avoids     |
|                                    | physical storage I/O and tests CPU/memory parsing limits.    |
+------------------------------------+--------------------------------------------------------------+
```

#### Programmatic Cache Control Protocol (OS-Cold State)
To achieve reproducible cold-cache runs without requiring full system reboots between every repetition, the benchmark runner must execute an automated cache-purging procedure requiring administrative rights [[6]](#ref-ms-filecache):
1. **Flush Dirty Buffers:** Issue `FlushFileBuffers` across all active volume handles to commit dirty pages to persistent storage.
2. **Trim System Working Set:** Call `SetSystemFileCacheSize((SIZE_T)-1, (SIZE_T)-1, 0)` from `memoryapi.h` (requires `SeIncreaseQuotaPrivilege`) [[6]](#ref-ms-filecache).
3. **Purge Standby Memory Lists:** Invoke the low-level NT API `NtSetSystemInformation` with the `SystemMemoryListInformation` information class and command code `MemoryEmptyWorkingSets` / `MemoryPurgeStandbyList` (or via Sysinternals `RAMMap.exe -empty standby`) [[6]](#ref-ms-filecache).
4. **Settle Window:** Enforce an idle dwell time of 5.0 seconds to allow the Windows Memory Manager and storage controller to reach steady-state.

### 4.3 Minifilter & Antivirus Interference Controls
Windows storage I/O passes through the Filter Manager stack (`fltmgr.sys`). Security drivers (such as Microsoft Defender `WdFilter.sys`, BitLocker `fvevol.sys`, or third-party EDR agents) intercept file system opens and directory enumerations, introducing substantial measurement variance:
* **Benchmark Protocol:**
  * For baseline engine profiling: Dedicated test partitions must be excluded from real-time antivirus inspection, or executed in a controlled lab image with real-time protection temporarily paused.
  * For real-world environment profiling: A dedicated test series must explicitly measure and report the performance impact of Microsoft Defender active scanning during full-disk traversal.
  * Windows Search Indexer (`SearchIndexer.exe`) and Superfetch/SysMain services must be set to a fixed, documented state across runs.

---

## 5. Exact Competitor Versions, Configurations, and Automation

To ensure absolute reproducibility, comparisons must specify exact binary versions, build numbers, architecture (x64), and configuration flags.

### 5.1 Competitor Inventory & Execution Matrix

```
+---------------------------------------------------------------------------------------------------+
| Competitor Software & Baseline Configuration Matrix                                               |
+----------------------+---------------+----------------------------------+-------------------------+
| Application          | Version Tested| Configuration / CLI Command Line | Scanning Mode           |
+----------------------+---------------+----------------------------------+-------------------------+
| Antibody WizTree     | 4.22+ (x64)   | WizTree64.exe "C:" /export="out" | Direct MFT Sequential   |
|                      |               | /admin=1 /sortby=0               | (Elevated)              |
+----------------------+---------------+----------------------------------+-------------------------+
| Antibody WizTree     | 4.22+ (x64)   | WizTree64.exe "C:\Path" /admin=0 | Win32 Traversal         |
|                      |               | /export="out" /sortby=0          | (Non-Elevated)          |
+----------------------+---------------+----------------------------------+-------------------------+
| JAM TreeSize Free    | 4.7+ (x64)    | TreeSize.exe /SCAN "C:"          | Direct MFT / Win32      |
|                      |               | /NOGUI /TEXT "out.csv"           | (2 Traversal Threads)   |
+----------------------+---------------+----------------------------------+-------------------------+
| JAM TreeSize Pro     | 9.1+ (x64)    | TreeSize.exe /SCAN "C:" /NOGUI   | Direct MFT / Win32      |
|                      |               | /OPTIONS "threads32.xml"         | (Configurable: 1–32 Thr)|
+----------------------+---------------+----------------------------------+-------------------------+
| PigTree              | Pre-Release   | pigtree-cli scan "C:" --mft      | Direct MFT Engine       |
|                      | (x64)         | pigtree-cli scan "C:" --win32    | Multi-threaded Win32    |
+----------------------+---------------+----------------------------------+-------------------------+
```

### 5.2 Settings & Feature Parity Alignment
When comparing analyzers, internal calculation options must be aligned [[9]](#ref-treesize-modes)[[10]](#ref-treesize-cli)[[11]](#ref-wiztree-mft)[[12]](#ref-wiztree-cli):
1. **Hard Link Accounting:** Explicitly configure whether hard links are tracked and deduplicated or treated as independent files.
2. **ADS Scanning:** Disable Alternate Data Stream scanning across all tools, or enable it across all tools supporting it.
3. **Export & Output Overhead:** When measuring raw scan throughput via CLI, ensure export generation (CSV serialization / disk write) is either decoupled from the scan timing measurement or isolated to an in-memory RAM disk to prevent disk write contention.

---

## 6. Comprehensive Metrics, Lifecycle Phases, and Measurement Instrumentation

A disk analyzer's lifecycle spans scanning, aggregation, memory indexing, searching, and UI rendering. Benchmarks must capture fine-grained metrics across all phases.

```
+---------------------------------------------------------------------------------------------------+
|                                  Disk Analyzer Lifecycle Phases                                   |
|                                                                                                   |
|  [Initiate Scan]                                                                                  |
|         │                                                                                         |
|         ▼                                                                                         |
|  ┌──────────────────────────────┐  ──> TTFR (Time to First Progressive Tree Render)               |
|  │ Phase 1: Storage Traversal   │                                                                 |
|  │ (MFT Bulk Read / Win32 Walk) │  ──> Disk I/O Read Bytes, Ops/sec, Storage Bandwidth             |
|  └──────────────┬───────────────┘                                                                 |
|                 ▼                                                                                 |
|  ┌──────────────────────────────┐  ──> T_scan (Scan Completion: all entries discovered)           |
|  │ Phase 2: Graph Aggregation   │                                                                 |
|  │ (Rollups, Links, Dedup)      │  ──> T_agg (Aggregation Done: tree sizes mathematically locked) |
|  └──────────────┬───────────────┘                                                                 |
|                 ▼                                                                                 |
|  ┌──────────────────────────────┐  ──> Peak Working Set, Private Commit (RAM Footprint)           |
|  │ Phase 3: Steady-State Index  │                                                                 |
|  │ (In-Memory Tree & Filtering) │  ──> In-Memory Search / Filter Latency (Regex over 1M items)    |
|  └──────────────┬───────────────┘                                                                 |
|                 ▼                                                                                 |
|  ┌──────────────────────────────┐  ──> UI Treemap Layout Time, Frame Time (ms), Drop Rate         |
|  │ Phase 4: UI Visualization    │                                                                 |
|  └──────────────────────────────┘                                                                 |
+---------------------------------------------------------------------------------------------------+
```

### 6.1 Lifecycle Phases & Timing Definitions

1. **Time-to-First-Result (TTFR) / Progressive Latency:**
   * *Definition:* Elapsed time from scan initiation until the top-level directory root and its immediate children are rendered in the visual UI and responsive to user expansion.
   * *Significance:* Captures perceived user responsiveness during long scans.
2. **Scan Traversal Completion Time (T_scan):**
   * *Definition:* Time from scan trigger until the last directory/file record is read from disk/OS APIs and placed into the analysis pipeline.
3. **Full Aggregation Completion Time (T_agg):**
   * *Definition:* Time when all recursive folder sizes, allocated sizes, file counts, and hardlink reconciliation calculations are finalized and locked in the in-memory tree.
4. **In-Memory Query & Filter Latency (T_query):**
   * *Definition:* Time required to execute a complete text/regex query (e.g., `*.dll` or `node_modules`) across the entire in-memory dataset ($N \ge 1,000,000$ files) and update the sorted view.
5. **UI Treemap & Grid Layout Latency (T_render):**
   * *Definition:* Time required to compute the treemap partition layout (e.g., Squarified Treemap algorithm) across the top $K$ nodes and draw the visual frame. Measured via ETW `DxgKrnl` present events or UI thread message queue frame timing [[7]](#ref-ms-etw).

### 6.2 Resource Consumption Counters

Data collection must use official Microsoft performance APIs and Event Tracing for Windows (ETW) [[7]](#ref-ms-etw)[[8]](#ref-ms-memory):

* **Memory Metrics (via `GetProcessMemoryInfo` / `PROCESS_MEMORY_COUNTERS_EX`):**
  * **Private Bytes (`PrivateUsage`):** Total committed virtual memory dedicated exclusively to the process. The primary indicator of true memory footprint and allocator bloat [[8]](#ref-ms-memory).
  * **Working Set (`WorkingSetSize` / `PrivateWorkingSet`):** Physical RAM pages actively resident [[8]](#ref-ms-memory).
  * **Peak Working Set (`PeakWorkingSetSize`):** Maximum physical RAM consumed during peak scanning/aggregation [[8]](#ref-ms-memory).
* **CPU & Thread Metrics (via `GetProcessTimes`):**
  * **Kernel CPU Time (t_kernel):** Time spent inside the Windows NT kernel (I/O management, SRM access checks, syscall dispatch). High kernel time indicates excessive syscall overhead.
  * **User CPU Time (t_user):** Time spent executing application code (MFT parsing, string handling, tree building).
  * **Total CPU Core-Seconds (t_total = t_kernel + t_user):** Total energy and processing cost across all cores.
* **Storage I/O Counters (via `GetProcessIoCounters` & ETW `DiskIO`/`FileIO`):**
  * **`ReadTransferCount`:** Total bytes read from disk/cache [[7]](#ref-ms-etw).
  * **`ReadOperationCount`:** Total number of read I/O operations issued.
  * **Physical Disk Service Time & Queue Depth:** Measured via ETW kernel provider `Microsoft-Windows-Kernel-Disk` [[7]](#ref-ms-etw).

### 6.3 Measurement Instrumentation & Precision
* **Wall-Clock High-Precision Timing:** All duration measurements must utilize `QueryPerformanceCounter` (QPC) with frequency queried via `QueryPerformanceFrequency`. QPC provides sub-microsecond, monotonic hardware clock timing unaffected by system clock adjustments [[4]](#ref-ms-findfirst).
* **ETW Tracing:** For in-depth kernel attribution, automate trace capture via `wpr.exe` (Windows Performance Recorder) with `FileIO`, `DiskIO`, and `VirtualAlloc` providers, and analyze via the `Microsoft.Performance.Toolkit` TraceProcessing library [[7]](#ref-ms-etw).

---

## 7. Statistical Rigor, Sample Sizes, and Reporting Standards

In compliance with established systems performance research standards (ACM SIGPLAN Guidelines [[1]](#ref-sigplan), Hoefler & Belli SC15 [[2]](#ref-hoefler), Georges et al. [[3]](#ref-georges)), benchmarking disk analyzers must adhere to rigorous statistical treatment.

```
+---------------------------------------------------------------------------------------------------+
|                                 Statistical Reporting Framework                                   |
+--------------------------+-----------------------+------------------------------------------------+
| Metric Type              | Correct Metric        | Incorrect / Prohibited Summary Metric          |
+--------------------------+-----------------------+------------------------------------------------+
| Durations / Elapsed Time | Median, Arithmetic    | Single-run measurements, best-case minimum     |
| (T_scan, T_agg, T_render)| Mean with Std Dev, IQR| without reporting variance                     |
+--------------------------+-----------------------+------------------------------------------------+
| Rates / Throughputs      | Harmonic Mean         | Arithmetic Mean of rates (causes mathematical  |
| (MB/s, Files/sec)        |                       | skew towards high outliers)                    |
+--------------------------+-----------------------+------------------------------------------------+
| Normalized Speedups /    | Geometric Mean        | Arithmetic Mean of ratios (distorts reciprocal |
| Relative Performance     |                       | baselines)                                     |
+--------------------------+-----------------------+------------------------------------------------+
| Confidence Intervals     | 95% Bootstrap / CI    | Stating superiority without overlapping CI     |
| & Significance           | (Empirical Percentile)| verification                                   |
+--------------------------+-----------------------+------------------------------------------------+
```

### 7.1 Repetition & Sampling Protocol
1. **Sample Size:** Every benchmark scenario must execute for a minimum of $N \ge 10$ independent repetitions (or $N \ge 20$ for micro-benchmarks such as query/filter latency).
2. **Warm Run Protocol:** For warm cache tests, execute 1 unmeasured warmup run to prime operating system caches, followed by $N$ recorded runs.
3. **Cold Run Protocol:** For cold cache tests, execute the automated cache flush sequence (Section 4.2) prior to each of the $N$ recorded runs.
4. **Outlier Policy:** Do not arbitrarily discard outliers. If an outlier occurs, inspect system ETW traces for background OS interruptions (e.g., Windows Update, Defender background scan) and document findings.

### 7.2 Non-Parametric & Distributional Metrics
System execution times on modern multitasking operating systems rarely follow a pure Gaussian normal distribution due to thread preemption, cache line conflicts, and SSD garbage collection [[2]](#ref-hoefler).
* **Mandatory Summary Metrics:**
  * **Median ($Q_2$ / 50th percentile):** Robust measure of central tendency.
  * **Interquartile Range (IQR = $Q_3 - Q_1$):** Measures dispersion without normality assumptions.
  * **Min & Max (Range):** Full bounds of observed performance.
  * **95% Confidence Interval ($CI_{95}$):** Computed via non-parametric percentile bootstrap (resampling $B = 10,000$ iterations).
* **Visualizations:** All comparative benchmark reports should present **Box-and-Whisker Plots** or **Cumulative Distribution Function (CDF)** curves rather than simple bare bar charts.

### 7.3 Environmental Disclosure Checklist
Every published benchmark dataset must include a complete metadata header disclosing:
* **OS:** Windows version, build number, Update Build Revision (UBR), and architecture (e.g., Windows 11 Pro 23H2 Build 22631.3296 x64).
* **CPU:** Processor make, model, physical/logical core count, base and turbo clock frequencies (e.g., AMD Ryzen 9 7950X, 16C/32T).
* **Memory:** Total physical capacity, DDR generation, transfer speed, and channel configuration (e.g., 64 GB DDR5-6000 CL30, Dual Channel).
* **Storage:** Drive model, firmware revision, interface, total capacity, partition filesystem, cluster size, and current volume fullness percentage (e.g., Samsung 990 Pro 2TB NVMe, FW 3B2QJXD7, NTFS 4KB clusters, 68% full).
* **Security & Power:** Power profile (High Performance), Microsoft Defender real-time scanning status, BitLocker encryption status (Hardware vs. Software XTS-AES-128), and Core Isolation / Memory Integrity (HVCI) state.

---

## 8. Summary of Methodological Guidelines for PigTree

| Dimension | Methodological Requirement for PigTree Benchmarking |
| :--- | :--- |
| **Privilege Regimes** | Strictly segregate comparisons between Direct MFT (Admin) and Directory Traversal (User). |
| **Datasets** | Test across multi-million small-file dev trees, media volumes, and real Windows system drives. |
| **Edge Cases** | Verify accounting accuracy for hard links (deduplicated allocation), junctions, cloud stubs, and WOF. |
| **Cache States** | Differentiate and programmatically control OS-Cold (`SetSystemFileCacheSize` + `RAMMap`) and OS-Warm states. |
| **Competitors** | Test against exact, pinned versions of WizTree (4.x) and TreeSize (Free/Pro 9.x) with matched settings. |
| **Metrics** | Capture TTFR, T_scan, T_agg, in-memory filter latency, UI frame time, Private Bytes, and Kernel/User CPU. |
| **Timing Tools** | Use `QueryPerformanceCounter` (sub-microsecond precision) and ETW kernel traces (`FileIO`/`DiskIO`). |
| **Statistics** | Execute $N \ge 10$ runs; report Medians, IQR, 95% Bootstrap CIs, and Harmonic Means for throughput rates. |
| **Claims** | Prohibit universal marketing claims; present multi-dimensional performance matrices with full environment disclosures. |

---

## 9. Primary References

* <a id="ref-sigplan"></a>**[1] ACM SIGPLAN.** (2017). *Empirical Evaluation Guidelines and Checklist*. ACM Special Interest Group on Programming Languages. Available at: [https://www.sigplan.org/Resources/EmpiricalEvaluation/](https://www.sigplan.org/Resources/EmpiricalEvaluation/)
* <a id="ref-hoefler"></a>**[2] Hoefler, T., & Belli, R.** (2015). *Scientific Benchmarking of Parallel Computing Systems: Twelve Ways to Tell the Masses When Reporting Performance Results*. In *Proceedings of the International Conference for High Performance Computing, Networking, Storage and Analysis (SC '15)*. ACM/IEEE. [https://doi.org/10.1145/2807591.2807644](https://doi.org/10.1145/2807591.2807644)
* <a id="ref-georges"></a>**[3] Georges, A., Buytaert, D., & Eeckhout, L.** (2007). *Statistically Rigorous Java Performance Evaluation*. In *Proceedings of the 22nd Annual ACM SIGPLAN Conference on Object-Oriented Programming Systems, Languages, and Applications (OOPSLA '07)*, pp. 57–76. [https://doi.org/10.1145/1297027.1297033](https://doi.org/10.1145/1297027.1297033)
* <a id="ref-ms-findfirst"></a>**[4] Microsoft Learn.** (2023). *FindFirstFileExW function (fileapi.h) and File Management Architecture*. Microsoft Corporation. Available at: [https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-findfirstfileexw](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-findfirstfileexw)
* <a id="ref-ms-reparse"></a>**[5] Microsoft Learn.** (2023). *Reparse Points, Reparse Tags, and File Operations*. Microsoft Corporation. Available at: [https://learn.microsoft.com/en-us/windows/win32/fileio/reparse-points](https://learn.microsoft.com/en-us/windows/win32/fileio/reparse-points)
* <a id="ref-ms-filecache"></a>**[6] Microsoft Learn.** (2023). *SetSystemFileCacheSize function (memoryapi.h) & Sysinternals RAMMap*. Microsoft Corporation. Available at: [https://learn.microsoft.com/en-us/windows/win32/api/memoryapi/nf-memoryapi-setsystemfilecachesize](https://learn.microsoft.com/en-us/windows/win32/api/memoryapi/nf-memoryapi-setsystemfilecachesize)
* <a id="ref-ms-etw"></a>**[7] Microsoft Learn.** (2023). *Event Tracing for Windows (ETW) and Windows Performance Toolkit (WPT)*. Microsoft Corporation. Available at: [https://learn.microsoft.com/en-us/windows-hardware/test/wpt/](https://learn.microsoft.com/en-us/windows-hardware/test/wpt/)
* <a id="ref-ms-memory"></a>**[8] Microsoft Learn.** (2023). *PROCESS_MEMORY_COUNTERS_EX structure (psapi.h) and Memory Management Architecture*. Microsoft Corporation. Available at: [https://learn.microsoft.com/en-us/windows/win32/api/psapi/ns-psapi-process_memory_counters_ex](https://learn.microsoft.com/en-us/windows/win32/api/psapi/ns-psapi-process_memory_counters_ex)
* <a id="ref-treesize-modes"></a>**[9] JAM Software.** (2024). *TreeSize Professional & Free Manual: Scan Modes and Multithreading*. JAM Software GmbH. Available at: [https://manuals.jam-software.com/treesize/](https://manuals.jam-software.com/treesize/)
* <a id="ref-treesize-cli"></a>**[10] JAM Software.** (2024). *TreeSize Command Line Options and Automated Reporting*. JAM Software GmbH. Available at: [https://manuals.jam-software.com/treesize/command_line_options.html](https://manuals.jam-software.com/treesize/command_line_options.html)
* <a id="ref-wiztree-mft"></a>**[11] Antibody Software.** (2024). *WizTree Direct MFT Reading Technology and Architecture*. Antibody Software. Available at: [https://diskanalyzer.com/](https://diskanalyzer.com/)
* <a id="ref-wiztree-cli"></a>**[12] Antibody Software.** (2024). *WizTree Command Line Parameters Reference*. Antibody Software. Available at: [https://diskanalyzer.com/guide](https://diskanalyzer.com/guide)
