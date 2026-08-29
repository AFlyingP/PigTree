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
  - [ADR 0001: Scanning Subsystem and Privilege Architecture](https://github.com/AFlyingP/PigTree/blob/decision/scanning-privilege-architecture/docs/adr/0001-scanning-and-privilege-architecture.md) [[8]](#ref-adr0001)

---

## 1. Purpose & Core Principles

This document establishes the authoritative, measurable product-performance targets, resource budgets, and comparative acceptance criteria for PigTree v1. It serves as the primary acceptance source for architecture and technology selection decisions ([#14](https://github.com/AFlyingP/PigTree/issues/14)) and downstream implementation tickets.

Performance in PigTree is treated as a core product feature, not an afterthought. However, performance claims are invalid if achieved by compromising safety, domain precision, or accessibility. The following principles guide all performance evaluations:

1. **Integrity Over Raw Speed**: Correctness, Coverage, Object Identity, Hard Link accounting, Reparse Point handling, Cloud Files offline placeholder safety, Content Stream fidelity, and capacity reconciliation semantics can never be omitted, truncated, or approximated to claim a faster scan or query time.
2. **Work Equivalence**: Performance comparisons are meaningful only when evaluating identical Analysis Profiles, Scope Coverage, privilege regimes, scanning architectures, hydration policies, and observable work. Operations performing different underlying work are classified and labeled as non-comparable.
3. **Fail-Closed Safety**: Under resource exhaustion or system pressure, PigTree must evict optional accelerators, degrade auxiliary caches, or fail honestly with a qualified partial Analysis Snapshot where possible. Silently dropping observed Directory Entries, altering requested Analysis Profiles, or inventing unobserved facts is prohibited.
4. **Transparent, Reproducible Evidence**: All published performance claims must be backed by public benchmark manifests, verifiable environment specifications, raw iteration counters, and nonparametric statistical confidence intervals. Universal marketing claims such as "Fastest on Windows" or unqualified speedup multipliers are rejected.

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
| - Absolute boundary for  | - Pass/fail p95 & median limits on | - High-stress 10M-entry bounds    |
|   all supported systems  |   Mainstream & Performance tiers   | - 24-hour endurance soak runs     |
| - 5,000,000 Directory    | - Interactive latency <= 100 ms    | - Sub-second live preflight on    |
|   Entries release floor  | - Governs acceptance for #14       |   10k action plans                |
+--------------------------+------------------------------------+-----------------------------------+
```

1. **Universal Production Floor (Mandatory Release Gate)**:
   - PigTree must maintain full functional capability, responsive interaction, and memory stability across all primary workflows for Scan Targets containing at least **5,000,000 observed Directory Entries** on supported local storage.
   - Any failure, crash, out-of-memory condition, or interactive lockup at or below 5M entries is an immediate release blocker.
2. **Normative Reference Budgets (Pass/Fail Acceptance Criteria)**:
   - Numeric budgets defined across designated Reference Hardware Tiers for scanning, memory, query, export, duplicate candidate discovery, content verification, and UI frame rates.
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
   - **Cloud Files Offline Placeholders**: OneDrive, iCloud, and SharePoint trees containing dehydrated placeholders with reparse tags (zero automatic cloud hydration allowed; explicit consented hydration is measured separately and excluded from local verification throughput claims).
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

A primary defect of legacy disk-analyzer benchmarks is comparing tools performing fundamentally unequal work [[1]](#ref-benchmarks)[[2]](#ref-comparison). PigTree formally defines standard Analysis Profiles and execution regimes to establish clear equivalence boundaries.

```
+---------------------------------------------------------------------------------------------------+
|                                 Scan Profiles & Work Breakdown                                    |
+--------------------------+---------------------------------------+--------------------------------+
| Scan Profile / Regime    | Included Observations & Work          | Excluded Work (Timed Apart)   |
+--------------------------+---------------------------------------+--------------------------------+
| Core Accounting Profile  | - Directory Entry names & parents     | - Content Stream data reads    |
| (Standard Traversal or   | - Object Identity & File IDs (where   | - Duplicate candidate grouping |
| Elevated Direct-MFT)     |   supported by filesystem)            | - Full hash content verify     |
|                          | - Object kinds (File, Dir, Special)   | - File export serialization    |
|                          | - Logical & Allocated Sizes           | - UI treemap rendering         |
|                          | - Hard Link & Reparse characteristics |                                |
|                          | - Cloud offline placeholder status    |                                |
|                          | - Coverage, Coverage Gaps, Aggregates |                                |
+--------------------------+---------------------------------------+--------------------------------+
| Full Metadata Profile    | - All Core Accounting observations    | - Content Stream data reads    |
| (Standard Traversal or   | - Timestamp Observations (C/M/A/Ch)   | - Duplicate candidate grouping |
| Elevated Direct-MFT)     | - Declared Owner & DACL Access Rules  | - Duplicate candidate grouping |
|                          |   (where permitted/requested)         | - Full hash content verify     |
|                          | - Storage Characteristics & ADS names | - File export serialization    |
+--------------------------+---------------------------------------+--------------------------------+
```

### 5.1 Scanning Profiles
- **Core Accounting Profile**: The baseline analysis required for complete space accounting. Observes Directory Entry names, hierarchical chains, underlying Object Identity, object kinds, Logical Size, physical Allocated Size, Hard Link reference counting, Reparse Point attributes, Cloud Files offline states, Coverage Gaps, Scope Aggregates, and volume Capacity reconciliation. Object Identity observations respect filesystem capabilities and Value Knowledge ([[7]](#ref-context)); the profile requests all evidence needed for settled identity and accounting, recording unavailable or unobserved fields with Coverage rather than assuming stable File IDs exist identically across every filesystem. Excludes Content Stream byte reading.
- **Full Metadata Profile**: Extends Core Accounting by observing all declared v1 metadata requested by the declared profile: Timestamp Observations (Created, Modified, Accessed, MFT Changed), file Owner (SID/account string) and Access Rules (security descriptors/DACLs) where permitted and requested, explicit Storage Characteristics (compression, sparsity), and Alternate Data Stream identities. Unobserved or inaccessible metadata is recorded with Coverage Gaps rather than forcing universal DACL/Owner reads regardless of profile or permissions. Excludes Content Stream byte reading.

### 5.2 Scanning Regimes
- **Standard User Traversal Regime (Regime B)**: The standard-user scanning regime defined in ADR 0001 [[8]](#ref-adr0001). Traverses directory hierarchies safely under standard user permissions (Medium Integrity). Universal across all filesystems, directory Scan Targets, and standard user accounts [[1]](#ref-benchmarks)[[3]](#ref-scanning).
- **Elevated Direct-MFT Regime (Regime A)**: Separately gated elevated NTFS whole-volume regime defined in ADR 0001 [[8]](#ref-adr0001). Valid exclusively on local NTFS whole-Volume Scan Targets, subject to parser-safety, elevation, and release gates [[1]](#ref-benchmarks)[[3]](#ref-scanning). Performance targets do not prescribe raw handles, privileges, parser mechanics, or APIs.

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
|   |--> Engine accepts command (Scan Plan creation, dispatch, and initial I/O occur after)         |
| [Event: First Useful Interactive Result] (T_first)                                                |
|   |--> Root node & immediate children visible and interactively queryable with provisional Coverage|
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
1. **Start Event (`T_start`)**: The exact timestamp when the engine accepts the analyze command. Scan Plan creation, dispatch, and initial I/O occur subsequently.
2. **First Useful Interactive Result (`T_first`)**: The timestamp when the root directory and its immediate first-level children with provisional Coverage are visible and interactively queryable by the client (not merely materialized in engine memory).
3. **First Operation Status (`T_status`)**: The timestamp when the engine emits its first structured progress/status notification to the client progress sink.
4. **Final Observation (`T_final_obs`)**: The timestamp when the underlying scanning adapter finishes enumerating the final directory entry or reading the final MFT record from storage.
5. **Snapshot Settlement (`T_settled`)**: The timestamp when post-traversal aggregation finishes (Hard Link deduplication settled, volume Capacity reconciliation computed, Scope Coverage finalized), the immutable Analysis Snapshot is sealed, and terminal Run Outcome is emitted.
6. **Cancellation Acceptance (`T_cancel_ack`)**: The timestamp when a cooperative cancellation request is received and acknowledged by the active engine task.
7. **Terminal Settlement on Cancel (`T_cancel_settled`)**: The timestamp when all in-flight I/O stops, resources are reclaimed, and terminal cancellation settlement completes. Where available and coherent, the engine may publish a qualified partial Analysis Snapshot, but a partial snapshot is not guaranteed for every cancelled operation.

---

## 7. Canonical Product Performance Target Tables

The following tables define the canonical, binding performance targets for PigTree v1. Every numeric target is stated exactly once and serves as an immutable gate.

### Statistical Conventions Used:
- **p95 (95th Percentile)**: Governs interactive latency, UI responsiveness, tail latencies, and end-to-end operation timeouts to guarantee smooth user experience under worst-case variance.
- **Median**: Governs sustained throughput, sequential processing rates, and bulk I/O bandwidth to measure central tendency without distortion from single-run outliers.
- **Warm Cache**: Operating system standby cache populated via a single unmeasured pre-warming run.
- **Cold Cache**: Controlled operating system cache state reset and standby list purge (evaluated for relative regression).

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
| **Cloud Dehydrated Exclusion** | Offline reparse placeholders in scope | Zero automatic cloud hydration allowed | Pass/Fail |

*Note: Explicit consented cloud hydration is permitted as a separate user-requested operation, but is measured independently and excluded from local storage verification throughput claims.*

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
| **Mutation Rollback Policy** | Interrupted Action Plan execution | Settle at next Commit Point (no unverified rollback)| Pass/Fail |

---

### 7.9 Concurrent Execution & System Impact Budgets

*Measures system responsiveness and background degradation when scanning concurrently with interactive use or background tasks.*

| Metric Description | Execution Mode & Condition | Target Limit & Impact Constraint | Gate Type |
| :--- | :--- | :--- | :--- |
| **Foreground Balanced Mode Impact**| Active scan with concurrent UI queries | Query/input latency regression <= 25% (budgets pass) | Pass/Fail |
| **Background Low-Impact Regression**| Background scan with active user workflows | Interactive UI latency regression <= 10% | Pass/Fail |
| **Background CPU Utilization** | Background low-impact execution mode | Average CPU <= 25% of logical processor capacity | Pass/Fail |
| **Background I/O Throttling** | Background low-impact execution mode | Bounded low-priority I/O; normal memory caps | Pass/Fail |
| **Effective Throttling Disclosure** | Runtime engine diagnostics | Emit observed throttling status in Operation Events / Diagnostics | Pass/Fail |

---

### 7.10 Endurance & Long-Run Soak Budgets

*Evaluated under automated continuous operation on Tier 1 (Mainstream).*

| Metric Description | Workload Protocol & Duration | Target Limit & Stability Invariant | Gate Type |
| :--- | :--- | :--- | :--- |
| **Standard Soak Test (8 Hours)** | Continuous mixed cycles (scan, reopen, query, candidates, cancel, export) | Retained memory growth <= 5.0% post steady-state; zero handle/thread leaks; interactive and cancellation budgets remain passing | Pass/Fail (Release Gate) |
| **Pre-Release Stress Soak (24 Hours)**| Extended 24-hour continuous mixed cycle stress | Zero unhandled exceptions, zero data corruption, stable memory slope | Stretch / Confidence |

*Note: Soak testing requires that approved absolute interaction, query, and cancellation budgets remain passing throughout extended operation; individual scan and export throughput rates are subject to mixed cycle contention and are evaluated for memory/handle stability rather than peak isolated throughput.*

---

## 8. Caching, Operating Environment & Statistical Protocol

To produce decision-significant benchmarks that resist experimental noise and caching bias, PigTree establishes clear environmental and statistical protocols [[1]](#ref-benchmarks)[[2]](#ref-comparison).

```
+---------------------------------------------------------------------------------------------------+
|                                 Caching & Environmental Protocols                                 |
+-------------------+---------------------------------------+---------------------------------------+
| Cache State       | Protocol Definition                   | Benchmark Application                 |
+-------------------+---------------------------------------+---------------------------------------+
| Warm Cache        | Single unmeasured warmup run          | Absolute product performance targets; |
|                   | immediately prior to measured runs    | interactive UI & query gating budgets |
+-------------------+---------------------------------------+---------------------------------------+
| OS-Cold Cache     | Documented OS cache reset (flush file | Relative regression gating; cold-to-  |
|                   | buffers, purge standby list/working   | warm multiplier tracking; no single   |
|                   | set), reset evidence, 5.0-s dwell     | absolute wall-clock gate              |
+-------------------+---------------------------------------+---------------------------------------+
| Hardware-Cold     | Full system power cycle or controller | Specialized pre-release               |
| Cache             | cache flush procedure                 | characterization                      |
+-------------------+---------------------------------------+---------------------------------------+
```

### 8.1 Environmental Controls
1. **Power Management & Clock Monitoring**: Pin the Windows power scheme to *High Performance* or *Ultimate Performance* and record the active scheme in the benchmark manifest. Power plans do not guarantee elimination of CPU frequency scaling; CPU clock frequencies, package temperatures, and system noise must be monitored via ETW hardware performance counters, and contaminated normative runs exhibiting thermal throttling or unexpected clock shifts must be rejected and invalidated.
2. **Thermal Stability**: Record ambient and initial component temperatures; enforce cooldown windows between high-stress iterations to prevent thermal throttling.
3. **Storage & Filesystem Hygiene**: Record physical drive make/model, firmware revision, volume total capacity, free space percentage, partition filesystem type, cluster allocation size (e.g., 4 KB default), and NTFS volume fragmentation index.
4. **Security & Antivirus Tracks**:
   - *Normative Real-World Track*: Real-time Windows Defender scanning active on default settings, with standard system services enabled.
   - *Controlled Lab Track*: Real-time Defender scanning paused or test partition added to exclusions to isolate raw engine performance from filter driver latency. Both tracks must be published side-by-side with clear labeling.
5. **System Services**: Disclose and record the status of the Windows Search Indexer (`SearchIndexer.exe`) and SuperFetch/SysMain services in the manifest. Disabling Search or SysMain is not required in public comparisons where equivalence with standard Windows default behavior is being measured, but their states must be identical across compared runs. Record Hypervisor-Protected Code Integrity (HVCI) and BitLocker drive encryption status.

### 8.2 Statistical Rigor & Reporting
1. **Sample Sizes**:
   - Macro Benchmarks (End-to-end scans, Reopen, Full Export, Soak): Minimum **$N \ge 10$** measured iterations.
   - Micro Benchmarks (Queries, candidate grouping, preflight steps, frame times): Minimum **$N \ge 20$** measured iterations.
2. **Summary Statistics**:
   - Report raw iteration values, sample Median, empirical 95th percentile (**p95**), Interquartile Range (**IQR**), Minimum, and Maximum.
   - Empirical p95 and its estimation uncertainty are reported despite small macro sample sizes ($N \ge 10$); p95 release gating budgets use the approved $N \ge 10$ minimum sample size.
   - Calculate and publish **95% Nonparametric Percentile Bootstrap Confidence Intervals** (10,000 resamples) for all medians, p95 estimates, and relative ratio comparisons.
   - Aggregate rates and throughput using the **Harmonic Mean** (to prevent rate skew); aggregate cross-workload speedup ratios using the **Geometric Mean** [[1]](#ref-benchmarks).
3. **Outlier & Interference Governance**:
   - Outlier statistical flags trigger automated or manual investigation into system traces and diagnostic logs, not automatic data exclusion.
   - Arbitrary or silent deletion of outlier data points is prohibited.
   - An iteration may be excluded only if an explicit ETW trace or diagnostic log confirms unrelated external system interference (such as Windows Update I/O or background AV signature updates). The excluded run, concrete reason, and replacement run must be explicitly documented in the benchmark output package.

---

## 9. Competitor Comparison Protocol & External Claim Policy

Comparing PigTree against mature commercial products (Antibody Software WizTree and JAM Software TreeSize) requires methodical alignment and licensing compliance [[1]](#ref-benchmarks)[[2]](#ref-comparison).

```
+---------------------------------------------------------------------------------------------------+
|                                 Competitor Comparison Matrix                                      |
+--------------------------+-----------------------+------------------------+-----------------------+
| Dimension                | PigTree (Target)      | WizTree (Pinned Build) | TreeSize Pro (Pinned) |
+--------------------------+-----------------------+------------------------+-----------------------+
| Pinned Reference Version | Current RC / Main     | Current pinned x64     | Current pinned x64    |
|                          |                       | release (e.g. 4.32)    | release (e.g. 9.8.x)  |
| Required License Tier    | Open Source / Core    | Supporter / Commercial | Commercial / Trial    |
| Direct-MFT Scan Mode     | Elevated Direct-MFT   | Elevated Direct-MFT    | Elevated Direct-MFT   |
| Standard Traversal Mode  | Standard User         | Standard User          | Standard User         |
|                          | Traversal             | Traversal (/admin=0)   | Traversal             |
| Subdirectory Scan Scope  | Standard User         | Standard User          | Standard User         |
|                          | Traversal             | Traversal              | Traversal             |
| Offline Cloud Placeholders| Zero Automatic       | Zero Automatic         | Skip Offline Files    |
|                          | Hydration             | Hydration              |                       |
+--------------------------+-----------------------+------------------------+-----------------------+
```

### 9.1 Pinned Versions & Licensing
- Authoritative release gates must execute against officially pinned, properly licensed 64-bit production releases documented in reviewed benchmark manifests.
- The dated 2026-08-28 research snapshot evaluated WizTree 4.32 and JAM Software TreeSize Professional 9.8.x x64 as examples and source snapshot [[2]](#ref-comparison); future gates pin the current comparable production release at the time of manifest review.
- *TreeSize Free* is excluded from automated regression pipelines due to lack of command-line automation [[2]](#ref-comparison) and is used for informational manual/GUI comparisons only.
- Test rigs must maintain valid commercial/supporter licenses where required by vendor terms. Declared binary integrity digests (e.g., cryptographic hash), build dates, and license types must be recorded in test manifests.

### 9.2 Comparative Scan Performance Gates
1. **Fairness Settings & Work Equivalence**: PigTree is evaluated only against competitor configurations configured for equivalent work. Test manifests must record and match:
   - Target path and scan scope.
   - Analysis Profile and actual observed fields.
   - Scope Coverage and Coverage Gaps.
   - Privilege regime (Standard User Traversal vs. Elevated Direct-MFT).
   - Operating system cache state (Warm vs. OS-Cold).
   - Hard-link unique vs. referenced accounting.
   - Alternate Data Stream (ADS) and stream metadata work.
   - Reparse boundary traversal and cycle policies.
   - Cloud hydration policy (zero automatic cloud hydration).
   - Filters, exclusions, and pattern rules.
   - Hidden and system file access permissions.
   - Allocation semantics (Logical Size vs. physical Allocated Size).
   - Microsoft Defender, power plan, and background service states.
   - Output serialization work (e.g. writing to in-memory RAM disk) and execution mode (GUI vs. headless/CLI).
   - *If exact equivalence cannot be configured in a competitor tool, no relative claim or pass/fail comparative gate is permitted; results may be documented for industry context only and labeled as Non-Comparable.*
2. **Standard User Traversal Parity & Speedup Gates**:
   - Evaluated using paired/repeated comparable runs and a nonparametric percentile bootstrap confidence interval for the time ratio.
   - **Parity Gate**: On equivalent Standard User Traversal workloads (Regime B), PigTree passes the parity gate only when the upper 95% bootstrap confidence bound for the PigTree/competitor time ratio is **$\le 1.10$** (within 10% of the fastest comparable pinned tool).
   - **TreeSize Speedup Gate**: On selected standard-traversal workloads, the gate and claim of being at least 10% faster than TreeSize Professional passes only when the upper 95% bootstrap confidence bound on the PigTree/TreeSize time ratio is **$\le 0.90$**.
3. **Elevated Direct-MFT Scanning Gate**:
   - Direct-MFT comparative gates apply **only if and when PigTree's direct-MFT adapter has passed all correctness, parser-safety, and release gates** defined in [ADR 0001](https://github.com/AFlyingP/PigTree/blob/decision/scanning-privilege-architecture/docs/adr/0001-scanning-and-privilege-architecture.md) [[8]](#ref-adr0001).
   - If direct-MFT is enabled, PigTree passes the direct-MFT comparative gate only when the upper 95% bootstrap confidence bound on the PigTree / current comparable pinned WizTree time ratio is **$\le 1.10$** on paired/matched comparable runs.

### 9.3 Public Claim Bounding & Expiration
- PigTree will never make sweeping, unqualified claims such as "The Fastest Disk Space Analyzer on Windows".
- Any public performance statement must explicitly declare and link:
  - Frozen benchmark manifest and exact test execution date.
  - Exact PigTree version, competitor versions, edition tiers, and declared binary integrity digests.
  - Reference hardware specifications, CPU model, RAM, storage media, and Windows build.
  - Target dataset composition, entry count, and directory depth.
  - Active Scanning Profile (Core Accounting vs. Full Metadata) and Privilege Regime (Standard User Traversal vs. Elevated Direct-MFT).
  - Cache regime (Warm vs. Cold), Defender status, and statistical metric ($N$, median, p95, 95% bootstrap CI).
- **Claim Expiration & Retest**: Public comparative performance claims expire whenever either product version, default settings, or material configuration changes, or when the underlying Windows build, hardware platform, or measurement methodology materially changes, and at least with every release candidate (RC)—not only major/minor releases. Expired claims must be re-verified against fresh benchmark manifests before re-publication; unverified or outdated published claims must be marked as *Stale*.

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

### 10.1 Release Governance & Automated Gating Rules
1. **Authoritative Release Pipeline Evaluation**: Absolute and relative performance gating occurs when the authoritative dedicated bare-metal hardware pipeline evaluates candidate builds. Shared PR CI runs non-normative correctness and gross-regression smoke tests; PR CI does not directly generate 5M normative lab results.
2. **Absolute Budget Breach on Authoritative Hardware**: Any candidate build evaluated on dedicated reference hardware that exceeds an absolute numeric budget (such as peak memory > 1.5 GiB at 5M entries, or 5M scan > 75 s on SATA), exhibits a memory scaling breach, violates UI frame stall limits (> 200 ms main thread stall or > 1% frames > 50 ms), or breaches cancellation latency budgets is an **immediate hard blocker**.
3. **Relative Regression Gating on Controlled Hardware**:
   - Evaluated using paired/matched comparable runs and a nonparametric percentile bootstrap CI for the time ratio or percentage change.
   - Regression blocking occurs when the 95% bootstrap confidence interval for matched change demonstrates **$\ge 10\%$ degradation** in execution time.
   - A measured point change of **$5\% \text{ to } 10\%$** triggers an automated performance investigation.
   - A **$\ge 10\%$ increase in peak memory footprint** or any frame stall violation blocks integration.
4. **Baseline Governance**: Baseline benchmark manifests are version-controlled in the repository. Baselines cannot be automatically updated or blessed by CI scripts; any baseline modification requires an explicit peer-reviewed manifest commit detailing the hardware, rationale, and verified run artifacts.

---

## 11. Privacy, Diagnostics & Reproducibility Guarantees

Performance benchmarking must never compromise user data privacy or leak sensitive file system facts [[7]](#ref-context):

1. **Synthetic & Public Datasets by Default**: Automated benchmark pipelines must operate exclusively on deterministic synthetic trees generated by reproducible script generators or public non-sensitive test corpus images.
2. **Complete Reproducibility Package**: Published benchmark releases must include a full reproducibility package containing:
   - Exact command lines, configuration flags, and profile settings.
   - Deterministic dataset generator scripts and declared dataset integrity digest.
   - Raw individual iteration measurements and derived summary statistics (median, p95, IQR, percentile bootstrap CIs).
   - ETW traces, performance counters, and diagnostic event logs.
   - Documented record of any statistical outlier investigations and trace-justified exclusions/replacements.
   - Redacted environment metadata header (hardware, OS build, security states).
   - Automation and analysis scripts where licensing permits (no proprietary competitor binaries are redistributed).
3. **Zero Automatic Trace/Diagnostic Upload**: Benchmark runs, ETW traces, and performance logs must remain local on the test machine. PigTree has no telemetry and prohibits automated background transmission of performance traces or diagnostics.
4. **Redaction Profile & Hardware Fingerprinting Warning**:
   - *Warning*: Redacted traces and hardware performance counter profiles can still potentially fingerprint hardware.
   - Collecting a diagnostic trace from a real-world system requires explicit user-initiated local export and user preview before sharing; automatic upload is prohibited.
   - When a diagnostic trace is exported, it must pass through an explicit local Redaction Profile:
     - File and folder names pseudonymized via one-way cryptographic hashing.
     - User security identifiers (SIDs) and account strings replaced with synthetic principal tokens.
     - Native error strings and paths scrubbed of user directory identifiers.
     - Hardware serial numbers and network identifiers removed.

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

Before any implementation milestone is certified for release, it must be validated against the canonical performance budgets and governance criteria defined in this document:

- [ ] **Universal Scale Floor**: Successfully scans, indexes, and presents a 5,000,000 Directory Entry target without crash or data loss (Section 2, 7.1, 7.2).
- [ ] **Standard Traversal Budgets**: Meets all canonical p95 scan durations, throughput limits, and finalization bounds across Tier 1 SATA, Tier 2 NVMe, and Tier 3 HDD (Section 7.1).
- [ ] **Initial Interactive Availability & Progress**: Surfaces root and immediate first-level children (p95 <= 1.0 s on SSD, <= 2.0 s on HDD), first status <= 250 ms p95, heartbeat gap <= 500 ms, and GUI materialization delay <= 100 ms p95 (Section 7.1).
- [ ] **Memory Scaling Invariants**: Base idle <= 256 MiB; incremental slope <= 256 bytes/entry; peak Private Bytes <= 512 MiB (1M) and <= 1.5 GiB (5M release floor) with graceful degradation under memory pressure (Section 7.2).
- [ ] **Historical Snapshot Reopen**: Loads and query-indexes saved snapshots within Section 7.3 budgets (p95 <= 3.0 s on NVMe, <= 6.0 s on SATA for 5M entries) with non-blocking background view warming.
- [ ] **Query, Filter & Insights Responsiveness**: Primary indexed page <= 100 ms p95; standard filtering, sorting, and domain Insights <= 200 ms p95; complex uncached queries <= 500 ms p95 with deterministic results (Section 7.4).
- [ ] **Export Throughput & Memory**: Emits first record <= 250 ms p95; sustains median >= 100,000 rows/s flat streaming export with <= 128 MiB buffer memory; export cancellation <= 500 ms p95 (Section 7.5).
- [ ] **UI Frame Timing & Accessibility**: Frame duration p95 <= 16.7 ms (>= 60 FPS), p99 <= 33.3 ms (>= 30 FPS), < 1% frames > 50 ms; zero main thread stalls > 200 ms; initial Insights render <= 300 ms p95; accessible workspace first frame <= 500 ms p95; semantic updates <= 200 ms p95; screen reader feeds <= 500 ms (Section 7.6).
- [ ] **Duplicate Candidate Discovery & Content Verification**: Complete 5M candidate grouping <= 5.0 s p95 with <= 512 MiB memory; sequential stream verification achieves >= 70% SSD / >= 60% HDD calibrated bandwidth; zero automatic cloud hydration (explicit consented hydration measured separately); verification cancellation p95 <= 1.0 s (Section 7.7).
- [ ] **Action Plan Preview, Preflight & Nonmutating Validation**: Preview <= 500 ms p95 (1k ops); nonmutating validation <= 2.0 s (warm metadata) / <= 5.0 s (live reads); routine preflight step median <= 100 ms with zero safety check skips; general cancellation ack <= 100 ms and settlement p95 <= 1.0 s (Section 7.8).
- [ ] **Concurrent Execution & Background Impact**: Balanced mode query regression <= 25%; low-impact background scan interactive UI regression <= 10%, average CPU <= 25%, bounded low-priority I/O, and throttling disclosed in Operation Events / Diagnostics (Section 7.9).
- [ ] **Long-Run Soak Stability**: Passes continuous 8-hour mixed soak cycle with retained memory growth <= 5.0% post steady-state, zero handle/thread leaks, and interactive/cancellation budgets passing (Section 7.10).
- [ ] **Competitor Parity & Speedup Gates**: Demonstrates matched-run parity (upper 95% bootstrap bound <= 1.10) against fastest comparable pinned tool and >= 10% faster than TreeSize Pro (upper 95% bootstrap bound <= 0.90) on standard traversal under matched fairness settings (Section 9.2).
- [ ] **Authoritative Governance & Statistical Rigor**: Authoritative dedicated-hardware pipeline evaluation, $N \ge 10$ iterations, percentile bootstrap 95% CIs, trace-justified outlier handling, and complete local reproducibility package with zero background data transmission (Sections 8, 10, 11).

---

## 14. References & Citations

- <a id="ref-benchmarks"></a>**[1] PigTree Team.** (2025). *Benchmark Evidence and Methods for Windows Disk Analyzers*. `docs/research/benchmark-evidence-and-methods.md`.
- <a id="ref-comparison"></a>**[2] PigTree Team.** (2026). *Current Performance Comparison Protocol: Primary-Source Facts*. `docs/research/current-performance-comparison.md` / [GitHub Research Branch](https://github.com/AFlyingP/PigTree/blob/research/current-performance-comparison/docs/research/current-performance-comparison.md).
- <a id="ref-scanning"></a>**[3] PigTree Team.** (2025). *Windows Filesystem Scanning, Storage Allocation, and Elevation Architecture*. `docs/research/windows-scanning-filesystem-elevation-facts.md`.
- <a id="ref-capabilities"></a>**[4] PigTree Team.** (2025). *WizTree and TreeSize Capabilities Comparison*. `docs/research/wiztree-and-treesize-capabilities.md`.
- <a id="ref-workflows"></a>**[5] PigTree Team.** (2025). *Everyday Disk Analysis Workflows and Pain Points*. `docs/research/everyday-disk-analysis-workflows-and-pain-points.md`.
- <a id="ref-ui"></a>**[6] PigTree Team.** (2025). *Windows UI Technologies Evaluation*. `docs/research/windows-ui-technologies.md`.
- <a id="ref-context"></a>**[7] PigTree Team.** (2026). *PigTree Information Architecture and Domain Model*. `CONTEXT.md`.
- <a id="ref-adr0001"></a>**[8] PigTree Team.** (2026). *Scanning Subsystem and Privilege Architecture*. [ADR 0001](https://github.com/AFlyingP/PigTree/blob/decision/scanning-privilege-architecture/docs/adr/0001-scanning-and-privilege-architecture.md).
