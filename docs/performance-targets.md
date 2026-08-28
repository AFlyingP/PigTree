# PigTree Product Performance Targets & Acceptance Budgets

- **Status**: Approved Decision
- **Date**: 2026-08-28
- **Decider**: Project Owner
- **Decision Issue**: [#13](https://github.com/AFlyingP/PigTree/issues/13)
- **Primary Source Citations & References**:
  - `docs/research/benchmark-evidence-and-methods.md` (Benchmark Evidence and Methods) [[1]](#ref-benchmarks)
  - `docs/research/current-performance-comparison.md` / [Current Performance Comparison Research](https://github.com/AFlyingP/PigTree/blob/research/current-performance-comparison/docs/research/current-performance-comparison.md) [[2]](#ref-comparison)
  - `docs/research/windows-scanning-filesystem-elevation-facts.md` (Windows Filesystem Scanning & Elevation) [[3]](#ref-scanning)
  - `docs/research/wiztree-and-treesize-capabilities.md` (WizTree and TreeSize Capabilities) [[4]](#ref-capabilities)
  - `docs/research/everyday-disk-analysis-workflows-and-pain-points.md` (Everyday Workflows & Pain Points) [[5]](#ref-workflows)
  - `docs/research/windows-ui-technologies.md` (Windows UI Technologies Evaluation) [[6]](#ref-ui)
  - `CONTEXT.md` (PigTree Domain Model & Information Architecture) [[7]](#ref-context)

---

## 1. Purpose & Non-Negotiable Principles

This document establishes the authoritative, measurable product-performance targets, resource budgets, and comparative acceptance criteria for PigTree v1. It serves as the primary acceptance source for upcoming architecture and technology selection decisions ([#14](https://github.com/AFlyingP/PigTree/issues/14)) and downstream implementation tickets.

Performance in PigTree is treated as a core product feature, not an afterthought. However, performance claims are invalid if achieved by compromising safety, domain precision, or accessibility. The following principles are absolute:

1. **Integrity Over Raw Speed**: Correctness, Coverage, Object Identity, Hard Link accounting, Reparse Point handling, Cloud Files offline placeholder safety, Content Stream fidelity, and capacity reconciliation semantics can never be omitted, truncated, or approximated to claim a faster scan or query time.
2. **Strict Work Equivalence**: Performance comparisons are meaningful only when evaluating identical Analysis Profiles, Scope Coverage, privilege regimes, scanning architectures, hydration policies, and observable work. Operations performing different underlying work are strictly classified and labeled as non-comparable.
3. **Fail-Closed Safety**: Under resource exhaustion or system pressure, PigTree must evict optional accelerators, degrade auxiliary caches, or fail honestly with a qualified partial Analysis Snapshot. Silently dropping observed Directory Entries, altering requested Analysis Profiles, or inventing unobserved facts is strictly prohibited.
4. **Transparent, Reproducible Evidence**: All published performance claims must be backed by public benchmark manifests, verifiable environment specifications, raw iteration counters, and non-parametric statistical confidence intervals. Universal marketing claims such as "Fastest on Windows" or unqualified speedup multipliers are rejected.

---

## 2. Target Classification & Governance Model

Performance budgets in PigTree are organized into three distinct tiers to distinguish mandatory release blockers from aspirational engineering frontiers:

```
+---------------------------------------------------------------------------------------------------+
|                                 Performance Target Classification                                 |
+--------------------------+------------------------------------+-----------------------------------+
| Universal Floor          | Normative Reference Budgets        | Stretch & Stress Goals            |
| (Release Gate)           | (Pass/Fail Engineering Targets)    | (Post-v1 & Hardware Ceilings)     |
+--------------------------+------------------------------------+-----------------------------------+
| - Absolute boundary for  | - Rigorous p95 & median limits on  | - High-stress 10M-entry bounds    |
|   all supported systems  |   Mainstream & Performance tiers   | - 24-hour endurance soak runs     |
| - 5,000,000 Directory    | - Interactive latency <= 100 ms    | - Sub-second live preflight on    |
|   Entries release floor  | - Governs acceptance for #14       |   10k action plans                |
+--------------------------+------------------------------------+-----------------------------------+
```

1. **Universal Production Floor (Mandatory Release Gate)**:
   - PigTree must maintain full functional capability, responsive interaction, and memory stability across all primary workflows for Scan Targets containing at least **5,000,000 observed Directory Entries** on supported local storage.
   - Any failure, crash, out-of-memory condition, or interactive lockup at or below 5M entries is an immediate release blocker.
2. **Normative Reference Budgets (Pass/Fail Acceptance Criteria)**:
   - Precise numeric budgets defined across designated Reference Hardware Tiers for scanning, memory, query, export, duplicate candidate discovery, content verification, and UI frame rates.
   - Architectural proposals and technology candidates evaluated in [#14](https://github.com/AFlyingP/PigTree/issues/14) must demonstrate compliance with these budgets to be accepted.
3. **Stretch & Stress Goals (Informational Engineering Targets)**:
   - High-scale stress testing at **10,000,000 observed Directory Entries** and extended 24-hour soak tests.
   - Stretch goals evaluate architectural headroom and guide future optimization; failure to meet stretch goals does not block v1 release if all universal floors and normative budgets pass.

---

## 3. Reference Hardware & Execution Tiers

To ensure reproducible verification across continuous integration, local development, and formal benchmarking labs, PigTree defines three standard hardware tiers. Exact CPU models, memory timings, SSD controller models, firmware revisions, and Windows build numbers are captured in reviewed benchmark manifests [[1]](#ref-benchmarks)[[2]](#ref-comparison).

```
+---------------------------------------------------------------------------------------------------+
|                                  Reference Hardware & Storage Tiers                               |
+------------------+-----------------------+--------------------+-------------------+---------------+
| Tier Name        | Processor             | Physical RAM       | Storage Subsystem | Target Role   |
+------------------+-----------------------+--------------------+-------------------+---------------+
| Tier 1:          | x64 Architecture,     | 16 GiB DDR4/DDR5   | SATA III SSD      | Primary Base  |
| Mainstream       | 4 Physical Cores min  |                    | (AHCI, 500 MB/s)  | Acceptance    |
+------------------+-----------------------+--------------------+-------------------+---------------+
| Tier 2:          | x64 Architecture,     | 32 GiB DDR4/DDR5   | Modern NVMe SSD   | High-End &    |
| Performance      | 8 Physical Cores min  |                    | (PCIe 4.0/5.0)    | Stretch Base  |
+------------------+-----------------------+--------------------+-------------------+---------------+
| Tier 3:          | x64 Architecture,     | 16 GiB DDR4/DDR5   | 7200 RPM SATA HDD | Mechanical    |
| Legacy HDD       | 4 Physical Cores min  |                    | (Spindle Media)   | Storage Base  |
+------------------+-----------------------+--------------------+-------------------+---------------+
```

*Note: These tiers define standard testbed environments. They do not prescribe internal runtime threading models, CPU core affinity, or execution concurrency strategies, which are architectural implementation concerns deferred to [#14](https://github.com/AFlyingP/PigTree/issues/14).*

---

## 4. Normative Datasets & Scale Boundary

Performance must be evaluated across structured test workloads designed to stress distinct filesystem mechanics and scale boundaries [[1]](#ref-benchmarks):

1. **Normative v1 Datasets (Mandatory Target Coverage)**:
   - **Local NTFS Volumes**: System drives, general data drives, deep development trees (`node_modules`, `.git`, Cargo target directories).
   - **ReFS, FAT32, exFAT Volumes**: Non-NTFS local partitions and high-capacity external drives.
   - **Removable Media**: High-latency USB flash drives and external storage.
   - **Cloud Files Offline Placeholders**: OneDrive, iCloud, and SharePoint trees containing dehydrated placeholders with reparse tags (`FILE_FLAG_OPEN_REPARSE_POINT` mandatory; zero hydration allowed).
   - **Protected & System Trees**: `C:\Windows\WinSxS`, `System Volume Information`, and locked administrator trees requiring graceful handling of Coverage Gaps.
   - **Structural Extremes**: Heavy Hard Link graphs (WinSxS multi-link aliasing), NTFS sparse and compressed files, deeply nested paths (> 260 characters / 32,767 Unicode characters), and files with Alternate Data Streams (ADS).
2. **Network Storage Boundary**:
   - Remote network shares (SMB/UNC paths) are officially **informational and outside the normative v1 release boundary**. SMB traversal characteristics may be reported for diagnostic baselines but do not carry pass/fail gating budgets for v1 release.
3. **Standard Workload Scales**:
   - **1M Tree**: Standard 1,000,000 observed Directory Entry workload (typical modern user or development drive).
   - **5M Tree (Normative Floor)**: 5,000,000 observed Directory Entry workload (large system or multi-user workstation).
   - **10M Tree (Stress/Stretch)**: 10,000,000 observed Directory Entry workload (enterprise workstation or large data repository).

---

## 5. Exact Work Definitions & Equivalence Profiles

A primary defect of legacy disk-analyzer benchmarks is comparing tools performing fundamentally unequal work [[1]](#ref-benchmarks)[[2]](#ref-comparison). PigTree formally defines standard Analysis Profiles and execution regimes to establish rigorous equivalence boundaries.

```
+---------------------------------------------------------------------------------------------------+
|                                 Scan Profiles & Work Breakdown                                    |
+--------------------------+---------------------------------------+--------------------------------+
| Scan Profile / Regime    | Included Observations & Work          | Excluded Work (Timed Apart)   |
+--------------------------+---------------------------------------+--------------------------------+
| Core Accounting Profile  | - Directory Entry names & parents     | - Content Stream data reads    |
| (Standard Traversal or   | - Object Identity & stable File IDs   | - Duplicate candidate grouping |
| Elevated Direct-MFT)     | - Object kinds (File, Dir, Special)   | - Full hash content verify     |
|                          | - Logical & Allocated Sizes           | - File export serialization    |
|                          | - Hard Link & Reparse characteristics | - UI treemap rendering         |
|                          | - Cloud offline placeholder status    |                                |
|                          | - Coverage, Coverage Gaps, Aggregates |                                |
+--------------------------+---------------------------------------+--------------------------------+
| Full Metadata Profile    | - All Core Accounting observations    | - Content Stream data reads    |
| (Standard Traversal or   | - Timestamp Observations (C/M/A/Ch)   | - Duplicate candidate grouping |
| Elevated Direct-MFT)     | - Windows Owner & Access Rules (DACL) | - Full hash content verify     |
|                          | - Storage Characteristics & ADS names | - File export serialization    |
+--------------------------+---------------------------------------+--------------------------------+
```

### 5.1 Scanning Profiles
- **Core Accounting Profile**: The baseline analysis required for complete space accounting. Observes Directory Entry names, hierarchical chains, underlying Object Identity (NTFS File ID / ReFS File ID), object kinds, Logical Size, physical Allocated Size, Hard Link reference counting, Reparse Point attributes, Cloud Files offline states, Coverage Gaps, Scope Aggregates, and volume Capacity reconciliation. **Strictly excludes Content Stream byte reading**.
- **Full Metadata Profile**: Extends Core Accounting by observing all declared v1 metadata: Timestamp Observations (Created, Modified, Accessed, MFT Changed), file Owner (SID/account string), Access Rules (security descriptors/DACLs), explicit Storage Characteristics (compression, sparsity), and Alternate Data Stream identities. **Strictly excludes Content Stream byte reading**.

### 5.2 Scanning Regimes
- **Standard User Traversal Regime (Regime B)**: Medium-integrity execution using standard Win32 and NT directory enumeration APIs (`FindFirstFileExW` / `NtQueryDirectoryFile`). Universal across all filesystems, directory Scan Targets, and standard user accounts [[1]](#ref-benchmarks)[[3]](#ref-scanning).
- **Elevated Direct-MFT Regime (Regime A)**: Elevated execution with `SeBackupPrivilege` opening raw volume handles (`\\.\C:`) to sequentially parse Master File Table records. Valid exclusively on local NTFS whole-Volume Scan Targets [[1]](#ref-benchmarks)[[3]](#ref-scanning).

*Rule: Standard User Traversal and Elevated Direct-MFT are distinct architectural regimes. Benchmarking must never compare an elevated direct-MFT scan against a standard directory traversal as like-for-like.*

### 5.3 Auxiliary & Secondary Operations
Duplicate Candidate discovery, Full Content Verification, structured File Export, and UI workspace layout rendering are distinct pipeline phases. They must be isolated, benchmarked, and reported independently of the core scan.

---

## 6. Metric Lifecycle Events & Measurement Delimiters

To eliminate ambiguity across automated benchmark runners, all performance timings must measure between explicit, observable domain lifecycle events emitted across the engine seam [[1]](#ref-benchmarks)[[7]](#ref-context):

```
+---------------------------------------------------------------------------------------------------+
|                                  Analysis Run Lifecycle Events                                    |
+---------------------------------------------------------------------------------------------------+
| [Start: Command Accepted]                                                                         |
|   |--> Dispatch Scan Plan, initiate adapter, first I/O request                                    |
| [Event: First Useful Interactive Result] (T_first)                                                |
|   |--> Root node & immediate children materialized with provisional Coverage                     |
| [Event: First Operation Status] (T_status)                                                        |
|   |--> Progress sink emits first non-zero status / target acknowledgment                          |
| [Interval: Traversal Phase] (T_traversal = T_final_obs - T_start)                                  |
|   |--> Active filesystem reading; continuous heartbeat updates (gap <= 500 ms)                   |
| [Event: Final Observation Received] (T_final_obs)                                                 |
|   |--> Last directory entry / MFT record read from source                                         |
| [Interval: Finalization Phase] (T_finalization = T_settled - T_final_obs)                         |
|   |--> Hard link consolidation, volume capacity reconciliation, snapshot freeze                   |
| [Event: Snapshot Settlement] (T_settled)                                                          |
|   |--> Immutable Analysis Snapshot created; terminal Run Outcome recorded; ready for query        |
| [Interval: End-to-End Analysis] (T_e2e = T_settled - T_start)                                     |
+---------------------------------------------------------------------------------------------------+
```

### Detailed Event Definitions:
1. **Start Event (`T_start`)**: The exact timestamp when the engine accepts the analyze command, completes Scan Plan creation, and initiates its first I/O operation.
2. **First Useful Interactive Result (`T_first`)**: The timestamp when the root directory and its immediate first-level children are materialized in memory and queryable by client views with provisional Coverage.
3. **First Operation Status (`T_status`)**: The timestamp when the engine emits its first structured progress/status notification to the client progress sink.
4. **Final Observation (`T_final_obs`)**: The timestamp when the underlying scanning adapter finishes enumerating the final directory entry or reading the final MFT record from storage.
5. **Snapshot Settlement (`T_settled`)**: The timestamp when post-traversal aggregation finishes (Hard Link deduplication settled, volume Capacity reconciliation computed, Scope Coverage finalized), the immutable Analysis Snapshot is sealed, and terminal Run Outcome is emitted.
6. **Cancellation Acceptance (`T_cancel_ack`)**: The timestamp when a cooperative cancellation request is received and acknowledged by the active engine task.
7. **Terminal Settlement on Cancel (`T_cancel_settled`)**: The timestamp when all in-flight I/O stops, resources are reclaimed, and a valid qualified partial Analysis Snapshot is sealed.

---

## 7. Canonical Product Performance Target Tables

The following tables define the canonical, binding performance targets for PigTree v1. Every numeric target is stated exactly once and serves as an immutable gate.

### Statistical Conventions Used:
- **p95 (95th Percentile)**: Governs interactive latency, UI responsiveness, tail latencies, and end-to-end operation timeouts to guarantee smooth user experience under worst-case variance.
- **Median**: Governs sustained throughput, sequential processing rates, and bulk I/O bandwidth to measure central tendency without distortion from single-run outliers.
- **Warm Cache**: Operating system standby cache populated via a single unmeasured pre-warming run.
- **Cold Cache**: Controlled operating system file cache and standby list purge (evaluated for relative regression).

---

### 7.1 Standard Traversal Scanning & Aggregation Budgets

*All metrics evaluate Warm-Cache Standard User Traversal (Regime B) unless explicitly noted.*

| Metric Description | Target Scale / Workload | Reference Tier | Metric Statistic & Target Limit | Gate Type |
| :--- | :--- | :--- | :--- | :--- |
| **First Useful Interactive Result** (`T_first`) | Any Supported Target | Tier 1 (SATA) / Tier 2 (NVMe) | p95 <= 1.0 s | Pass/Fail |
| **First Useful Interactive Result** (`T_first`) | Any Supported Target | Tier 3 (HDD) | p95 <= 2.0 s | Pass/Fail |
| **First Operation Status** (`T_status`) | Any Supported Target | All Tiers | p95 <= 250 ms | Pass/Fail |
| **Active Progress Heartbeat Interval** | All Analysis Runs | All Tiers | Max Gap <= 500 ms | Pass/Fail |
| **GUI Event Materialization Delay** | Emitted Engine Events | All Tiers | p95 <= 100 ms | Pass/Fail |
| **End-to-End Settlement** (`T_e2e`, Core Accounting) | 1,000,000 Small-File Tree | Tier 1 (SATA SSD) | p95 <= 12.0 s | Pass/Fail |
| **End-to-End Settlement** (`T_e2e`, Core Accounting) | 1,000,000 Small-File Tree | Tier 2 (NVMe SSD) | p95 <= 6.0 s | Pass/Fail |
| **End-to-End Settlement** (`T_e2e`, Core Accounting) | 1,000,000 Small-File Tree | Tier 3 (HDD) | p95 <= 90.0 s | Pass/Fail |
| **End-to-End Settlement** (`T_e2e`, Core Accounting) | 5,000,000 Directory Entries | Tier 1 (SATA SSD) | p95 <= 75.0 s | Pass/Fail (Release Floor) |
| **End-to-End Settlement** (`T_e2e`, Core Accounting) | 5,000,000 Directory Entries | Tier 2 (NVMe SSD) | p95 <= 35.0 s | Pass/Fail (Release Floor) |
| **End-to-End Settlement** (`T_e2e`, Core Accounting) | 10,000,000 Directory Entries | Tier 2 (NVMe SSD) | p95 <= 90.0 s | Stretch Goal |
| **Steady-State Traversal Throughput** | 1M to 5M Entries Interval | Tier 1 (SATA SSD) | Median >= 80,000 entries/s | Pass/Fail |
| **Steady-State Traversal Throughput** | 1M to 5M Entries Interval | Tier 2 (NVMe SSD) | Median >= 170,000 entries/s | Pass/Fail |
| **Full Metadata Relative Throughput** | Equivalent Target Workload | All Tiers | Median >= 70% of Core Accounting | Pass/Fail |
| **Post-Traversal Finalization Phase** (`T_finalization`) | 1,000,000 Small-File Tree | All Tiers | p95 <= min(10% of `T_traversal`, 2.0 s) | Pass/Fail |
| **Post-Traversal Finalization Phase** (`T_finalization`) | 5,000,000 Directory Entries | All Tiers | p95 <= min(10% of `T_traversal`, 8.0 s) | Pass/Fail |
| **Cold-Cache Scan Gating** | 1M and 5M Standard Datasets | All Tiers | Reported; Relative Regression Gated | Pass/Fail (Relative) |

---

### 7.2 Memory Footprint & Process Family Scaling Budgets

*Evaluated across the entire PigTree process family (host, workers, UI shell, renderers) under standard user execution on Tier 1 (Mainstream 16 GiB).*

| Metric Description | Target Scale / Scope | Target Limit & Model | Gate Type |
| :--- | :--- | :--- | :--- |
| **Base Product Idle Overhead** | Process Family at Rest (Zero Scan) | Peak Private Bytes <= 256 MiB | Pass/Fail |
| **Incremental Memory Slope** | Per Observed Directory Entry | Incremental Private Bytes <= 256 bytes/entry | Pass/Fail |
| **Total Peak Memory Footprint** | 1,000,000 Observed Entries | Peak Private Bytes <= 512 MiB | Pass/Fail |
| **Total Peak Memory Footprint** | 5,000,000 Observed Entries (Release Floor) | Peak Private Bytes <= 1.5 GiB | Pass/Fail (Release Floor) |
| **Total Peak Memory Footprint** | 10,000,000 Observed Entries (32 GiB RAM) | Peak Private Bytes <= 3.0 GiB | Stretch Goal |
| **Memory Pressure Degradation** | High Memory Load Condition | Evict caches / degrade accelerators; zero data loss | Pass/Fail |
| **Honest Coherence Failure** | Memory Exhaustion (OOM) | Fail honestly with qualified partial artifact; no crash | Pass/Fail |

---

### 7.3 Artifact Reopen & Snapshot Loading Budgets

*Measures time from open command acceptance until the historical Analysis Snapshot is verified, loaded into an immutable Artifact View, indexed, and available for interactive query and Insights display.*

| Metric Description | Target Scale / Artifact Size | Reference Tier | Metric Statistic & Target Limit | Gate Type |
| :--- | :--- | :--- | :--- | :--- |
| **Historical Snapshot Reopen** | 1,000,000 Entries Snapshot | Tier 1 (SATA) / Tier 2 (NVMe) | p95 <= 1.0 s | Pass/Fail |
| **Historical Snapshot Reopen** | 5,000,000 Entries Snapshot | Tier 1 (SATA SSD) | p95 <= 6.0 s | Pass/Fail (Release Floor) |
| **Historical Snapshot Reopen** | 5,000,000 Entries Snapshot | Tier 2 (NVMe SSD) | p95 <= 3.0 s | Pass/Fail (Release Floor) |
| **Historical Snapshot Reopen** | 10,000,000 Entries Snapshot | Tier 2 (NVMe SSD) | p95 <= 8.0 s | Stretch Goal |
| **Background View Warming** | Non-blocking secondary index warming | All Tiers | Background async; zero query skew | Pass/Fail |

---

### 7.4 Query, Filter, Sort & Insights Budgets

*Evaluated on an active, settled Artifact View containing 5,000,000 observed Directory Entries.*

| Metric Description | Query Type & Operation Scope | Metric Statistic & Target Limit | Gate Type |
| :--- | :--- | :--- | :--- |
| **Indexed Primary Page Access** | First page retrieval on sorted view (Top 100) | p95 <= 100 ms | Pass/Fail |
| **Standard Filtering & Sorting** | Common Name, Path prefix, Size, Type, Date filters | p95 <= 200 ms | Pass/Fail |
| **Domain Insights Overview** | Largest items, top extensions, age distribution | p95 <= 200 ms | Pass/Fail |
| **Complex Uncached Queries** | Regex search, multi-clause predicates, unique object aggregates | p95 <= 500 ms | Pass/Fail |
| **Long Query Acknowledgment** | Any query executing > 100 ms | Acknowledge <= 100 ms; cancellable | Pass/Fail |
| **Query Result Determinism** | Repeated identical query executions | 100% exact, deterministic output | Pass/Fail |

---

### 7.5 Export Throughput & Memory Budgets

*Evaluated during data export to a high-speed RAM-disk or non-target physical storage volume to prevent destination I/O write contention [[1]](#ref-benchmarks)[[2]](#ref-comparison).*

| Metric Description | Target Format & Scope | Metric Statistic & Target Limit | Gate Type |
| :--- | :--- | :--- | :--- |
| **First Export Record Emitted** | JSON, NDJSON, CSV formats | p95 <= 250 ms | Pass/Fail |
| **Flat Row Streaming Throughput** | CSV, NDJSON, Flat JSON (5M entries) | Median >= 100,000 rows/s | Pass/Fail |
| **Additional Export Memory** | In-flight serialization buffering | Peak Incremental Memory <= 128 MiB | Pass/Fail |
| **Hierarchical JSON Reporting** | Nested JSON tree structures | Report throughput in bytes/s (not rows) | Pass/Fail |
| **Export Cancellation** | User-aborted export operation | p95 <= 500 ms | Pass/Fail |

---

### 7.6 Graphical UI Responsiveness, Frame Timing & Accessibility Budgets

*Evaluated against an active 5,000,000 entry Analysis Snapshot in the graphical workspace.*

| Metric Description | Interaction & Viewport Workflow | Metric Statistic & Target Limit | Gate Type |
| :--- | :--- | :--- | :--- |
| **Input-to-Visible Feedback** | Keystroke, click, selection, button toggle | p95 <= 100 ms | Pass/Fail |
| **Main UI Thread Responsiveness** | Any background engine or indexing operation | No main thread block > 200 ms | Pass/Fail |
| **Interactive Frame Time (p95)** | Scripted table scroll, tree expand, treemap zoom | Frame Duration p95 <= 16.7 ms (>= 60 FPS) | Pass/Fail |
| **Interactive Frame Time (p99)** | Scripted table scroll, tree expand, treemap zoom | Frame Duration p99 <= 33.3 ms (>= 30 FPS) | Pass/Fail |
| **Severe Frame Stalls** | Scripted continuous navigation & resizing | < 1.0% of frames > 50.0 ms | Pass/Fail |
| **Initial Insights Viewport Render** | Initial workspace view opening | p95 <= 300 ms | Pass/Fail |
| **Accessible Workspace First Frame** | Initial assistive DOM / layout ready | p95 <= 500 ms | Pass/Fail |
| **Progressive Treemap Refinement** | Visual refinement after data availability | Complete refinement <= 1.0 s | Pass/Fail |
| **Semantic State Updates** | Focus, selection, expanded, sort/filter, dialogs | p95 <= 200 ms | Pass/Fail |
| **Assistive Screen Reader Feeds** | Coalesced status & progress announcements | Emitted <= 500 ms (errors unsuppressed) | Pass/Fail |
| **Long UI Task Acknowledgment** | Heavy background recalculations (> 100 ms) | Acknowledge <= 100 ms; phase <= 250 ms | Pass/Fail |

---

### 7.7 Duplicate Candidate Discovery & Streaming Verification Budgets

*Evaluated on an open 5,000,000 entry Analysis Snapshot.*

| Metric Description | Operation & Scope | Target Limits & Statistics | Gate Type |
| :--- | :--- | :--- | :--- |
| **Initial Candidate Groups Emitted** | First Duplicate Candidate Sets displayed | p95 <= 500 ms | Pass/Fail |
| **Complete Candidate Grouping** | Full candidate discovery over 5M entries | p95 <= 5.0 s | Pass/Fail (Release Floor) |
| **Complete Candidate Grouping** | Full candidate discovery over 10M entries | p95 <= 12.0 s | Stretch Goal |
| **Candidate Discovery Memory** | Temporary incremental working memory | Peak Incremental Memory <= 512 MiB | Pass/Fail |
| **Content Verification Startup** | Begin content hashing & progress display | p95 <= 500 ms | Pass/Fail |
| **Content Read Throughput (SSD)** | Local sequential stream verification (SATA/NVMe)| Median >= 70% calibrated drive read bandwidth | Pass/Fail |
| **Content Read Throughput (HDD)** | Local sequential stream verification (Spindle) | Median >= 60% calibrated drive read bandwidth | Pass/Fail |
| **Content Progress Heartbeat Gap** | Byte-level verification progress updates | Max Gap <= 500 ms | Pass/Fail |
| **Mismatch Proof Presentation** | Surfacing verified content inequality | p95 <= 250 ms after proof | Pass/Fail |
| **Verification Cancellation** | User-aborted content stream verification | p95 <= 1.0 s; hard ceiling <= 2.0 s | Pass/Fail |
| **Cloud Dehydrated Exclusion** | Offline reparse placeholders in scope | Zero automatic hydration allowed | Pass/Fail |

---

### 7.8 Action Plan Preview, Live Preflight & Mutation Safety Budgets

*Evaluated during guarded storage remediation workflows [[7]](#ref-context).*

| Metric Description | Operation & Plan Complexity | Metric Statistic & Target Limit | Gate Type |
| :--- | :--- | :--- | :--- |
| **Action Plan Preview Generation** | Plan compilation for 1,000 operations | p95 <= 500 ms | Pass/Fail |
| **Action Plan Preview Generation** | Plan compilation for 10,000 operations | p95 <= 2.0 s | Stretch Goal |
| **Nonmutating Plan Validation** | Warm metadata validation (1,000 ops) | p95 <= 2.0 s | Pass/Fail |
| **Nonmutating Plan Validation** | Validation requiring live reads (1,000 ops) | p95 <= 5.0 s | Pass/Fail |
| **Nonmutating Plan Validation** | Validation requiring live reads (10,000 ops) | p95 <= 15.0 s | Stretch Goal |
| **Live Preflight Initial Decision** | Preflight check for first executable step | p95 <= 500 ms | Pass/Fail |
| **Routine Preflight Step Latency** | Sequential step verification | Median <= 100 ms per step | Pass/Fail |
| **Preflight Safety Invariant** | Safety verification checks under load | Zero deadline skips of safety checks | Pass/Fail |
| **General Task Cancellation** | Cancellation acknowledgment | p95 <= 100 ms | Pass/Fail |
| **Terminal Cancellation Settlement**| Cease I/O and reach safe clean state | p95 <= 1.0 s; hard ceiling <= 2.0 s | Pass/Fail |
| **Mutation Rollback Policy** | Interrupted Action Plan execution | Settle at next Commit Point (no fake rollback)| Pass/Fail |

---

### 7.9 Concurrent Execution & System Impact Budgets

*Measures system responsiveness and background degradation when scanning concurrently with interactive use or background tasks.*

| Metric Description | Execution Mode & Condition | Target Limit & Impact Constraint | Gate Type |
| :--- | :--- | :--- | :--- |
| **Foreground Balanced Mode Impact**| Active scan with concurrent UI queries | Query/input latency regression <= 25% (budgets pass) | Pass/Fail |
| **Background Low-Impact Regression**| Background scan with active user workflows | Interactive UI latency regression <= 10% | Pass/Fail |
| **Background CPU Utilization** | Background low-impact execution mode | Average CPU <= 25% of logical processor capacity | Pass/Fail |
| **Background I/O Throttling** | Background low-impact execution mode | Bounded low-priority I/O; normal memory caps | Pass/Fail |
| **Effective Throttling Disclosure** | Runtime engine diagnostics | Emit observed throttling status in telemetry | Pass/Fail |

---

### 7.10 Endurance & Long-Run Soak Budgets

*Evaluated under automated continuous operation on Tier 1 (Mainstream).*

| Metric Description | Workload Protocol & Duration | Target Limit & Stability Invariant | Gate Type |
| :--- | :--- | :--- | :--- |
| **Standard Soak Test (8 Hours)** | Continuous mixed cycles (scan, reopen, query, candidates, cancel, export) | Retained memory growth <= 5.0% post steady-state; zero handle/thread leaks; all budgets pass | Pass/Fail (Release Gate) |
| **Pre-Release Stress Soak (24 Hours)**| Extended 24-hour continuous mixed cycle stress | Zero unhandled exceptions, zero data corruption, stable memory slope | Stretch / Confidence |

---

## 8. Caching, Operating Environment & Statistical Protocol

To produce decision-significant benchmarks that resist experimental noise and caching bias, PigTree establishes strict environmental and statistical protocols [[1]](#ref-benchmarks)[[2]](#ref-comparison).

```
+---------------------------------------------------------------------------------------------------+
|                                 Caching & Environmental Protocols                                 |
+-------------------+---------------------------------------+---------------------------------------+
| Cache State       | Protocol Definition                   | Benchmark Application                 |
+-------------------+---------------------------------------+---------------------------------------+
| Warm Cache        | Single unmeasured warmup run immediately | Absolute product performance targets; |
|                   | prior to measured iterations          | interactive UI & query gating budgets |
+-------------------+---------------------------------------+---------------------------------------+
| OS-Cold Cache     | System File Cache & Standby List      | Relative regression gating; cold-to-  |
|                   | purged via SetSystemFileCacheSize &   | warm multiplier tracking; no single   |
|                   | NtSetSystemInformation (30s dwell)   | absolute wall-clock gate              |
+-------------------+---------------------------------------+---------------------------------------+
| Hardware-Cold     | Full system power cycle / unmount     | Special pre-release characterization; |
| Cache             | with device controller flush          | hardware controller cache dissipation |
+-------------------+---------------------------------------+---------------------------------------+
```

### 8.1 Environmental Controls
1. **Power Management**: Enforce Windows *High Performance* or *Ultimate Performance* power scheme via `powercfg /setactive` to eliminate dynamic CPU clock frequency scaling noise [[1]](#ref-benchmarks)[[2]](#ref-comparison).
2. **Thermal Stability**: Monitor CPU package temperature and clock frequency via ETW hardware performance counters; automatically reject and invalidate iterations exhibiting thermal throttling.
3. **Storage & Filesystem Hygiene**: Record physical drive model, firmware version, volume total capacity, free space percentage, filesystem type, cluster allocation size (e.g., 4 KB default), and NTFS volume fragmentation index.
4. **Security & Antivirus Tracks**:
   - *Normative Track*: Real-time Windows Defender scanning active on default settings.
   - *Isolated Baseline Track*: Real-time Defender scanning disabled and target tree added to exclusions to isolate raw engine performance from third-party filter driver latency. Both tracks must be published side-by-side.
5. **System Services**: Disable Windows Search Indexer (`WSearch`) and SuperFetch/SysMain during formal test runs to eliminate background disk contention. Record status of Hypervisor-Protected Code Integrity (HVCI) and BitLocker drive encryption.

### 8.2 Statistical Rigor & Reporting
1. **Sample Sizes**:
   - Macro Benchmarks (End-to-end scans, Reopen, Full Export, Soak): Minimum **$N \ge 10$** measured iterations.
   - Micro Benchmarks (Queries, candidate grouping, preflight steps, frame times): Minimum **$N \ge 20$** measured iterations.
2. **Summary Statistics**:
   - Report raw iteration values, sample Median, empirical 95th percentile (**p95**), Interquartile Range (**IQR**), Minimum, and Maximum.
   - Calculate and publish **95% Nonparametric BCa Bootstrap Confidence Intervals** (10,000 resamples) for all medians and p95 estimates.
   - Aggregate rates and throughput using the **Harmonic Mean** (to prevent rate skew); aggregate cross-workload speedup ratios using the **Geometric Mean** [[1]](#ref-benchmarks).
3. **Outlier & Interference Governance**:
   - Arbitrary or silent deletion of outlier data points is strictly prohibited.
   - An iteration may be excluded only if an explicit ETW trace confirms unrelated system interference (such as Windows Update I/O or background AV signature download). The excluded run, concrete ETW reason, and replacement run must be explicitly documented in the benchmark output package.

---

## 9. Competitor Comparison Protocol & External Claim Policy

Comparing PigTree against mature commercial products (Antibody Software WizTree and JAM Software TreeSize) requires absolute methodological and licensing integrity [[1]](#ref-benchmarks)[[2]](#ref-comparison).

```
+---------------------------------------------------------------------------------------------------+
|                                 Competitor Comparison Matrix                                      |
+--------------------------+-----------------------+------------------------+-----------------------+
| Dimension                | PigTree (Target)      | WizTree (Pinned Build) | TreeSize Pro (Pinned) |
+--------------------------+-----------------------+------------------------+-----------------------+
| Pinned Reference Version | Current RC / Main     | WizTree 4.32 x64 [[2]] | TreeSize Pro 9.8.x [[2]]|
| Required License Tier    | Open Source / Core    | Supporter / Commercial | Commercial / Trial    |
| Direct-MFT Scan Mode     | Elevated Adapter      | Default Admin Mode     | MFT Mode (Admin)      |
| Standard Traversal Mode  | Win32 / NT Directory  | /admin=0 Traversal     | Win32 Traversal Mode  |
| Subdirectory Scan Scope  | Native Traversal      | Win32 Subtree Scan     | Win32 Subtree Scan    |
| Offline Cloud Placeholders| Zero Hydration Flags | Zero Hydration         | Skip Offline Files    |
+--------------------------+-----------------------+------------------------+-----------------------+
```

### 9.1 Pinned Versions & Licensing
- Benchmark suites must execute against officially pinned, properly licensed 64-bit production releases: **WizTree 4.32 x64** and **JAM Software TreeSize Professional 9.8.x x64** (as documented in current comparison research [[2]](#ref-comparison)).
- *TreeSize Free* is excluded from automated regression pipelines due to lack of command-line automation [[2]](#ref-comparison) and is used for informational GUI comparison only.
- Test rigs must maintain valid commercial/supporter licenses where required by vendor terms. Exact binary SHA-256 hashes, build dates, and license types must be recorded in test manifests.

### 9.2 Comparative Scan Performance Gates
1. **Regime & Profile Equivalence**: PigTree is evaluated only against competitor configurations configured for equivalent work (identical hard link tracking, ADS observation, reparse handling, and output serialization to RAM-disk).
2. **Standard User Traversal Parity**:
   - On equivalent Standard User Traversal workloads (Regime B), PigTree's median end-to-end scan time must be **$\le 1.10\times$ (within 10%) of the fastest comparable tool**.
   - On selected standard-traversal workloads, PigTree must **outperform TreeSize Professional median scan time by at least 10% ($\ge 10\%$ faster)** with decision-significant non-overlapping 95% bootstrap confidence intervals.
3. **Elevated Direct-MFT Scanning Gate**:
   - Direct-MFT comparative gates apply **only if and when PigTree's direct-MFT adapter has passed all correctness, parser-safety, and release gates** defined in [ADR 0001](https://github.com/AFlyingP/PigTree/blob/decision/scanning-privilege-architecture/docs/adr/0001-scanning-and-privilege-architecture.md).
   - If direct-MFT is enabled, PigTree median scan time must be within **$\le 1.10\times$ of WizTree 4.32**.
4. **Incommensurable Work Policy**: If exact feature or accounting equivalence cannot be configured in a competitor tool, results may be published for transparent industry context but must be explicitly labeled as *Non-Comparable* and excluded from pass/fail gating.

### 9.3 Public Claim Bounding & Expiration
- PigTree will never make sweeping, unqualified claims such as "The Fastest Disk Space Analyzer on Windows".
- Any public performance statement must explicitly declare:
  - Exact PigTree version, competitor versions, and edition tiers.
  - Reference hardware specifications, CPU model, RAM, storage media, and Windows build.
  - Target dataset composition, entry count, and directory depth.
  - Active Scanning Profile (Core Accounting vs. Full Metadata) and Privilege Regime (Standard Traversal vs. Direct-MFT).
  - Cache regime (Warm vs. Cold), Defender status, and statistical metric ($N$, median, p95, 95% CI).
- **Claim Expiration**: All comparative performance claims automatically expire upon a new major/minor release of PigTree, an update to the compared competitor product, or a major Windows OS update. Expired claims must be re-verified against fresh benchmark manifests before re-publication.

---

## 10. Regression Tracking, CI/CD Cadence & Baseline Management

Continuous performance governance ensures that code changes do not silently erode performance budgets.

```
+---------------------------------------------------------------------------------------------------+
|                                     CI/CD Execution Cadence                                       |
+-------------------+-----------------------+-----------------------------------+-------------------+
| Pipeline Tier     | Environment           | Workloads & Scope                 | Frequency         |
+-------------------+-----------------------+-----------------------------------+-------------------+
| Smoke CI          | Shared Cloud Runner   | 10k synthetic tree; unit counters | Every PR / Push   |
| (Correctness)     | (Non-Normative)       | Gross regression detector (>50%)  |                   |
+-------------------+-----------------------+-----------------------------------+-------------------+
| Dedicated Lab     | Bare-Metal Mainstream | 1M standard tree; Warm & Cold     | Nightly Automated |
| (Normative Gates) | Reference Testbed     | Full memory & query benchmarks    | Build Run         |
+-------------------+-----------------------+-----------------------------------+-------------------+
| Stress & Soak     | Bare-Metal Mainstream | 5M release floor; 10M stretch;    | Weekly Scheduled  |
| Pipeline          | and NVMe Testbeds     | 8-hour mixed soak workload        | Run               |
+-------------------+-----------------------+-----------------------------------+-------------------+
| Full RC Release   | All Reference Tiers   | Complete filesystem matrix,       | Every Release     |
| Validation Suite  | (SATA, NVMe, HDD)     | pinned competitor test suite      | Candidate (RC)    |
+-------------------+-----------------------+-----------------------------------+-------------------+
```

### 10.1 Automated Gating Rules
1. **Absolute Budget Breach**: Any commit or PR that causes an absolute numeric budget breach (e.g., peak memory > 1.5 GiB at 5M entries, or 5M scan > 75 s on SATA) is an **immediate hard blocker**.
2. **Relative Regression on Controlled Hardware**:
   - A measured increase of **$\ge 10\%$ in median or p95** execution time with non-overlapping 95% bootstrap confidence intervals blocks integration.
   - An increase of **$5\% \text{ to } 10\%$** triggers an automated performance investigation.
   - A **$\ge 10\%$ increase in peak memory footprint** or any frame stall violation (> 200 ms main thread stall or > 1% frames > 50 ms) blocks integration.
3. **Baseline Governance**: Baseline benchmark manifests are version-controlled in the repository. Baselines cannot be automatically updated or "blessed" by CI scripts; any baseline modification requires an explicit peer-reviewed manifest commit detailing the hardware, rationale, and verified run artifacts.

---

## 11. Privacy, Diagnostics & Reproducibility Guarantees

Performance benchmarking must never compromise user data privacy or leak sensitive file system facts [[7]](#ref-context):

1. **Synthetic & Public Datasets by Default**: Automated benchmark pipelines must operate exclusively on deterministic synthetic trees generated by reproducible script generators or public non-sensitive test corpus images.
2. **Zero Automatic Telemetry Upload**: Benchmark runs, ETW traces, and performance logs must remain strictly local on the test machine. PigTree prohibits automated background transmission of performance traces.
3. **Redaction Profile for Diagnostic Sharing**: If a performance defect or trace must be collected from a real-world user system, it must pass through an explicit local preview and Redaction Profile:
   - File and folder names pseudonymized via one-way cryptographic hashing.
   - User security identifiers (SIDs) and account strings replaced with synthetic principal tokens.
   - Native error strings and paths scrubbed of user directory identifiers.
   - Hardware serial numbers and network identifiers removed to prevent hardware fingerprinting.

---

## 12. Architecture & Technology Selection Deferrals

To maintain clean separation between product performance requirements and implementation decisions, this document defines **acceptance criteria only**. It explicitly defers all engineering choices to [#14](https://github.com/AFlyingP/PigTree/issues/14):

- **Implementation Languages**: Deferred (C++, Rust, C#/.NET, Go, or hybrid architectures).
- **In-Memory Data Structures & Indexing**: Deferred (Arena allocators, compressed trie, columnar tables, B-trees, cache-conscious graphs).
- **Persistence & Serialization Engine**: Deferred (Custom binary snapshot, FlatBuffers, SQLite, LMDB, memory-mapped storage).
- **Process Topology & IPC Transport**: Deferred (Single-process multi-threading vs. multi-process host/worker isolation, shared memory ring buffers, anonymous pipes).
- **UI Framework & Presentation Architecture**: Deferred (WPF, WinUI 3, native Win32/Direct2D, Web-native shell, GPU-accelerated canvas).
- **Worker Thread Pool & Scheduling**: Deferred (I/O completion ports, work-stealing thread pool, async I/O coroutines).
- **Packaging & Distribution Model**: Deferred (MSIX, standalone portable executable, setup installer).

---

## 13. Acceptance & Verification Checklist

Before any implementation milestone is certified for release, it must be validated against this checklist:

- [ ] **Universal Scale Floor**: Successfully scans, indexes, and presents a 5,000,000 Directory Entry target without crash or data loss.
- [ ] **Standard Traversal Budgets**: Meets all p95 scan durations (<= 12 s at 1M SATA, <= 6 s at 1M NVMe, <= 75 s at 5M SATA, <= 35 s at 5M NVMe).
- [ ] **Initial Interactive Availability**: Surfaces root and immediate children (p95 <= 1.0 s) with first status within <= 250 ms.
- [ ] **Memory Scaling Invariant**: Base idle <= 256 MiB; incremental slope <= 256 bytes/entry; peak Private Bytes <= 512 MiB (1M) and <= 1.5 GiB (5M).
- [ ] **Snapshot Reopen**: Loads and query-indexes saved 5M snapshot within p95 <= 3.0 s (NVMe) / <= 6.0 s (SATA).
- [ ] **Query & Filter Responsiveness**: Primary indexed page <= 100 ms p95; standard filters and Insights <= 200 ms p95.
- [ ] **Export Throughput**: Emits first record <= 250 ms p95; sustains median >= 100,000 rows/s flat streaming export with <= 128 MiB buffer memory.
- [ ] **UI Frame & Responsiveness**: Frame duration p95 <= 16.7 ms, p99 <= 33.3 ms, < 1% frames > 50 ms; zero main thread stalls > 200 ms.
- [ ] **Candidate Discovery & Stream Verification**: Complete 5M duplicate candidates <= 5.0 s p95; stream verification achieves >= 70% drive read bandwidth; zero cloud hydration.
- [ ] **Action Plan & Preflight Safety**: Plan preview <= 500 ms p95 (1k ops); routine preflight step median <= 100 ms; zero safety deadline bypasses.
- [ ] **Cancellation Latency**: General task cancellation acknowledged <= 100 ms; terminal settlement p95 <= 1.0 s (hard ceiling 2.0 s).
- [ ] **Competitor Scan Parity**: Demonstrates median scan time <= 1.10x fastest comparable tool and >= 10% faster than TreeSize Pro on standard traversal.
- [ ] **8-Hour Soak Test**: Passes continuous 8-hour mixed cycle with <= 5.0% retained memory growth post steady-state.
- [ ] **Statistical & Privacy Compliance**: All reported benchmark results backed by $N \ge 10$ iterations, raw counters, bootstrap 95% CIs, and zero telemetry leakage.

---

## 14. References & Citations

- <a id="ref-benchmarks"></a>**[1] PigTree Team.** (2025). *Benchmark Evidence and Methods for Windows Disk Analyzers*. `docs/research/benchmark-evidence-and-methods.md`.
- <a id="ref-comparison"></a>**[2] PigTree Team.** (2026). *Current Performance Comparison Protocol: Primary-Source Facts*. `docs/research/current-performance-comparison.md` / [GitHub Research Branch](https://github.com/AFlyingP/PigTree/blob/research/current-performance-comparison/docs/research/current-performance-comparison.md).
- <a id="ref-scanning"></a>**[3] PigTree Team.** (2025). *Windows Filesystem Scanning, Storage Allocation, and Elevation Architecture*. `docs/research/windows-scanning-filesystem-elevation-facts.md`.
- <a id="ref-capabilities"></a>**[4] PigTree Team.** (2025). *WizTree and TreeSize Capabilities Comparison*. `docs/research/wiztree-and-treesize-capabilities.md`.
- <a id="ref-workflows"></a>**[5] PigTree Team.** (2025). *Everyday Disk Analysis Workflows and Pain Points*. `docs/research/everyday-disk-analysis-workflows-and-pain-points.md`.
- <a id="ref-ui"></a>**[6] PigTree Team.** (2025). *Windows UI Technologies Evaluation*. `docs/research/windows-ui-technologies.md`.
- <a id="ref-context"></a>**[7] PigTree Team.** (2026). *PigTree Information Architecture and Domain Model*. `CONTEXT.md`.
