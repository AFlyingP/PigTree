# Scanning Subsystem and Privilege Architecture

- **Status**: Accepted
- **Date**: 2026-08-28
- **Decider**: Project owner

PigTree requires an architecture that delivers fast, accurate disk space analysis across Windows filesystems while upholding safety, least privilege, and domain correctness. We separate scanning from snapshot aggregation and presentation via a deep observation seam, enforce standard-user traversal by default, isolate short-lived read-only elevated operations and raw parsers in constrained worker processes, and require rigorous validation and fallback for uncontracted raw metadata parsing.

## Context

Windows filesystem scanning presents conflicting trade-offs between privilege requirements, traversal performance, API stability, and observation semantics:

- Standard user-mode Win32 traversal handles all filesystems and directory targets safely without elevation, but is constrained by Win32 metadata query overhead and per-directory access permissions.
- Direct MFT parsing on NTFS volumes can accelerate metadata reading for whole volumes when elevated, but relies on an undocumented, unsupported on-disk format, introduces parser vulnerability risks, fails to capture runtime state like cloud hydration or live reparse targets, and cannot scan directory subsets without full-volume traversal.
- Alternative filesystems (ReFS, FAT32, exFAT) have distinct metadata architectures, lack supported raw parser models, or require different discovery strategies (such as USN journals on ReFS, which lack file sizes and require enrichment).
- Users frequently run disk analyzers under standard accounts, where elevation prompts must never be automatic, unexpected, or required for baseline operation.
- Analyzer tools that request full-process elevation or run persistent elevated background services create unnecessary security surface, while tools that mutate snapshots or guess progress mislead users on data integrity.

## Decision

### 1. Deep Observation Seam

The scanning subsystem is separated from the rest of PigTree across a single, deep observation seam. Callers initiate an Analysis Run by providing:
- Exactly one **Scan Target** (a whole Volume or a single local directory),
- An **Analysis Profile**,
- A security and elevation policy,
- A resource budget,
- A progress sink, and
- A cooperative cancellation token.

The scanning subsystem emits a typed observation stream, structured Coverage Gaps, provenance metadata, adapter-attempt and fallback lifecycle events, and a terminal Run Outcome.

All higher-level concerns—including Analysis Snapshot construction, deriving Scope Coverage from requested observations and Coverage Gaps relative to the Analysis Profile, graph aggregation, whole-volume capacity reconciliation, persistence, UI rendering, and CLI output formatting—live strictly above this seam. Filesystem adapters never construct private tree models, maintain UI state, or write to persistence.

### 2. Central Scan Planner

Scanning execution is governed by a central Scan Planner. Before any I/O occurs, the planner evaluates filesystem capability probes, target scope and filesystem type, the requested Analysis Profile, current process token privilege, release-enabled adapter flags, and benchmark policy to produce an immutable, ordered Scan Plan.

The Scan Plan explicitly defines the adapter execution sequence and fallback triggers. Individual filesystem adapters are strictly passive executors: they never prompt the user, secretly fall back to alternative strategies, broaden the Scan Target boundary, or alter the requested Analysis Profile.

### 3. Version 1 Adapter Matrix

The v1 release defines explicit adapter boundaries based on target kind and filesystem:

- **Directory Scan Targets (all filesystems)**: Documented Win32 directory enumeration and metadata queries restricted strictly to the selected directory boundary. Whole-volume scanning followed by path filtering is explicitly prohibited for directory targets.
- **Standard-User NTFS Whole Volume**: Documented Win32 directory traversal.
- **FAT32 and exFAT (Fixed and Removable Targets)**: Documented Win32 traversal.
- **ReFS (Standard and Elevated)**: Documented Win32 traversal. Elevated USN-assisted journal discovery is an optional future capability only if rigorous benchmarks prove end-to-end throughput gains, as USN records lack size information and require secondary query enrichment.
- **Approved Elevated NTFS Whole Volume**: Elevated documented Win32 traversal is the selected adapter unless and until the stable-release verification gates in Section 11 are satisfied and the raw MFT parser adapter is explicitly enabled; only when gated and enabled does the raw-MFT-first chain apply, attempting the raw MFT parser adapter first with automatic, transparent fallback to elevated documented traversal upon any validation failure, unexpected layout, or parsing anomaly.
- **Excluded from v1 Scope**: Direct user-mode reliance on `NtQueryDirectoryFile` as the primary contract, raw parsers for FAT32/exFAT/ReFS, whole-volume scan-and-filter for directory targets, continuous/live journal change monitoring, and SMB or remote network storage sources.

### 4. Default Analysis Profile

The default Analysis Profile captures the core facts needed for accurate space analysis without incurring prohibitive secondary I/O:
- Directory Entry hierarchy, entry names, and parent relationships;
- Filesystem Object classification (File, Directory, Special Object);
- Strongest available Object Identity evidence (e.g., volume-scoped File IDs on NTFS/ReFS);
- Logical Size and Allocated Size where supported by the filesystem;
- Core filesystem attributes and filesystem-defined Timestamp Observations;
- Hard Link references, Reparse Point tags, and cloud-storage placeholder characteristics;
- Explicit Value Knowledge states (Known, Not Observed, Unavailable, Not Applicable) and observation provenance.

Coverage (and Scope Coverage) is an output and snapshot semantic derived strictly above the scanner from requested observations and reported Coverage Gaps relative to the active Analysis Profile; it is not an observation class that the profile itself captures. All observed values and Value Knowledge states preserve provenance as required.

Security principal (Owner), Access Rules (ACLs), alternate Content Stream enumeration, file content reading/hashing, and duplicate verification are strictly excluded from the default profile and require explicit Snapshot Enrichments or specialized analysis profiles.

### 5. Standard-First Execution and Explicit Elevation Flow

All scans begin under standard-user privileges without elevation prompts, ensuring immediate utility.

Elevation is offered only as a distinct, subsequent Analysis Run when the planner or standard run identifies:
1. Protected paths that generated Coverage Gaps or inaccessible required metadata, or
2. A measured, material whole-volume acceleration opportunity on an approved NTFS volume.

Every elevation offer must explicitly present the target, the proposed adapter class, and the specific impact (e.g., gap resolution or speed gain). If the user declines, cancels, or encounters an elevation failure, the existing standard Analysis Snapshot remains unchanged and fully accessible.

CLI execution requires an explicit privileged-scan option/policy or allowed interactive consent to attempt privileged execution, with no surprise UAC prompts, while leaving exact CLI syntax to the shared engine and automation contract; the outcome is recorded cleanly in machine-readable output.

### 6. Worker Lifecycle and Privilege Separation

Ordinary scans execute within a disposable, medium-integrity worker process.

Privileged scans execute via a short-lived, least-privilege, read-only elevated broker process. The broker is bound via authenticated IPC and session binding to a single user session, execution nonce, Scan Target, Analysis Profile, and Scan Plan. The broker exits immediately upon run completion, failure, or cancellation.

The broker's capabilities are strictly constrained:
- It operates exclusively in read-only mode;
- It cannot write, delete, move, or modify files;
- It cannot hydrate cloud or offline files;
- It cannot modify journal state, lock volumes, or dismount filesystems;
- It cannot take file ownership or modify Access Control Lists (ACLs);
- It cannot perform cleanup or remediation actions. Any future cleanup functionality will require an entirely separate authorization lifecycle and distinct execution protocol.

### 7. Inter-Process Communication (IPC) Trust Boundary

The IPC channel between the main application and helper workers is treated as a strict security boundary:
- The communication channel is authenticated and bound to the specific execution plan and nonce;
- All messages are strictly versioned, length-bounded, and deserialized under tight size and memory caps;
- The main process validates all payload structures, enum values, size invariants, Object Identity representations, relationship graphs, entry orderings, and target confinement boundaries;
- Helper output is treated as privileged but untrusted data;
- The interface avoids shared mutable object graphs, shared memory write segments, or unsolicited handle passing and commands.

### 8. Raw Parser Isolation and Containment

The elevated broker opens a read-only handle to the targeted volume device. The raw on-disk parsing logic executes in a separate, disposable child process isolated from the broker's lifecycle and management operations.

Where supported by the platform, the parsing child runs under a restricted token with a duplicated read-only volume handle. If token restriction is constrained, process isolation and strict IPC validation remain mandatory. Raw parser hangs must be detected by a bounded heartbeat/watchdog/timeout, and the broker must terminate the child before fallback. Any crash, out-of-bounds read, or parsing rejection similarly results in worker termination without crashing the broker or compromising the fallback to documented traversal.

### 9. Raw MFT Invariant Validation and Fail-Closed Acceptance

Because Microsoft does not document or support raw MFT layouts as a public contract, the raw MFT adapter operates on a strict fail-closed basis.

Before and during processing, the adapter validates:
- Known NTFS layout assumptions and boot sector geometry;
- Record signatures, fixup arrays, and sequence numbers;
- Parent-child directory relationships and acyclic hierarchy invariants;
- Size accounting rules requiring non-negative values and format/stream-specific consistency, without assuming Allocated Size is greater than or equal to Logical Size because sparse, compressed, or resident storage may differ;
- Live consistency markers, employing bounded rereads for records updated during traversal.

If an unsupported layout version, corrupt record structure, inconsistency, or indeterminate Observation Interval is encountered, the raw attempt is immediately rejected. Partially parsed raw records are discarded and never enter the snapshot. The privileged run automatically transitions to the elevated documented traversal adapter, logging and surfacing the fallback event.

### 10. Authority of Raw Facts and Enrichment Boundaries

Validated raw MFT records serve as authoritative sources only for:
- Discovery and volume hierarchy;
- Object Identity and Directory Entry name/parent pairings;
- Object kind, standard attributes, and filesystem timestamps;
- Allocated Size and Logical Size evidence for supported stream types;
- Hard Link counts and basic Reparse Point tags with proven semantics.

All extended or context-dependent facts—including profile-specific security descriptors, cloud placeholder states, complex alternate streams, Accessibility, EFS encryption status, and ambiguous Reparse Points—must be resolved through supported Win32 queries with provenance recorded. PigTree will never bypass EFS encryption, alter security permissions to force reads, or infer Allocated Size from Logical Size.

### 11. Stable-Release Verification Gates

The raw MFT adapter is disabled by default in stable production releases until a comprehensive verification suite passes:
1. Differential scanning tests comparing raw output against documented Win32 traversal across standard and synthetic corpora;
2. Malformed and corrupted MFT fuzzing to verify parser isolation and crash resilience;
3. Windows and NTFS version compatibility matrices (covering cluster sizes, record sizes, and feature flags);
4. Interruption and cancellation stress tests under active filesystem churn;
5. Demonstrable end-to-end benchmark performance advantages over optimized Win32 traversal on representative hardware.

When enabled in production, every raw run continues to execute mandatory invariant checks, whole-volume capacity reconciliation, and a bounded deterministic sample comparison against documented APIs. Disagreements trigger an immediate fallback. Until all gates pass, elevated NTFS scans default to elevated documented Win32 traversal.

### 12. Cross-Filesystem Consistency and Information Model Alignment

The scanning engine adheres to the unified PigTree domain model across NTFS, ReFS, FAT32, and exFAT:
- Unsupported filesystem features (e.g., Hard Links on FAT32, compression on exFAT) are recorded as `Not Applicable` with provenance, rather than zero or synthetic values;
- Missing or inaccessible metadata is explicitly marked as `Unavailable` with failure reasons;
- Hard Link accounting requires volume-scoped Object Identity and whole-scope traversal; directory Scan Targets preserve `External Reference Uncertainty` to indicate that links may exist outside the scanned boundary;
- Alternate Content Streams are observed only when declared in the active Analysis Profile.

### 13. Reparse Points, Cloud Storage, and Live Churn Safety

To prevent infinite recursion, storage corruption, and unexpected network activity:
- Base scans inspect Reparse Point tags but never traverse directory junctions, symbolic link targets, or volume mount points into external boundaries;
- Online-only cloud storage placeholders (e.g., OneDrive, Cloud Files) are identified via metadata without triggering content hydration or file recall;
- Implicit content reads are forbidden during standard scanning;
- Dynamic filesystem modifications during scanning are handled with bounded retries; if an object disappears or changes incompatibly, its status is marked as `disappeared` or `errored`, reflecting the impact on Scope Coverage without claiming atomic point-in-time consistency.

### 14. Progress Tracking, Cancellation, and Transparent Fallback

Progress is communicated through adapter-neutral lifecycle phases: `discovering`, `enriching`, `aggregating`, `validating`, and `finalizing`.
- Progress metrics report observed items, elapsed time, and denominator confidence; percentage estimates and ETAs are surfaced only when the total denominator is mathematically defensible;
- User-facing status messages use plain, honest language while preserving exact provenance and diagnostic logs for technical analysis;
- Cooperative cancellation stops new scheduling promptly, requests cancellation of supported outstanding asynchronous I/O on a best-effort basis, bounds remaining chunk processing and timeouts, finalizes coherent observations into an immutable partial snapshot, and terminates helper processes without leaking half-decoded records;
- Adapter fallbacks are recorded as explicit lifecycle events containing attempt timestamps, failure reasons, and fallback intervals.

### 15. Resource Management and Structured Diagnostics

Scanning workers adhere to strict resource bounds:
- Worker thread pools apply bounded concurrency and backpressure queues;
- Snapshot aggregation and reduction are deterministic for the same accepted observation set, independent of parallel worker completion order (without implying repeat scans of a changing filesystem yield identical observations);
- Resource limits enforce memory and handle caps; if limits are reached, the scan slows gracefully or fails honestly rather than dropping observations silently.
- Local structured diagnostic logs record adapter types, timing benchmarks, entry counts, validation checks, and Coverage Gaps. File paths in diagnostic logs are redacted or hashed by default, and diagnostic data is never transmitted automatically.

## Consequences

### Positive
- **Strong security posture**: Zero full-process elevation; read-only, short-lived, targeted helper broker; strict process isolation for raw on-disk parsers.
- **Uncompromised standard-user utility**: Standard-user scanning works out-of-the-box across all filesystems without unexpected UAC prompts.
- **High data integrity**: Fail-closed parsing, invariant validation, and transparent fallback ensure incomplete or corrupt filesystem structures never produce incorrect snapshots.
- **Domain consistency**: Unified information model cleanly reflects differences in filesystem capabilities via explicit Value Knowledge without distorting reality.

### Negative and Trade-offs
- **Architectural complexity**: Multi-process worker management, IPC message serialization, and plan negotiation require more infrastructure than in-process scanning.
- **Validation overhead**: Strict invariant checking, differential sampling, and reconciliation checks add compute overhead to raw scanning.
- **Gated optimization**: Raw MFT acceleration remains disabled in stable builds until rigorous testing and benchmark criteria are satisfied, meaning initial elevated NTFS scans rely on documented traversal.

## Considered Options

The following alternative architectures were evaluated and rejected:
- **Whole-application elevation**: Running the entire UI and analyzer as administrator violates least privilege and exposes the complete application attack surface.
- **Persistent background service**: Installing an always-running elevated service introduces permanent security overhead and maintenance complexity for an on-demand desktop tool.
- **Raw MFT as the default or exclusive scanner**: Raw parsing is brittle, unsupported on non-NTFS volumes, unworkable for directory targets without full-volume scanning, and risks snapshot corruption without fallback.
- **Direct NT-native API (`NtQueryDirectoryFile`) as supported v1 contract**: Undocumented internal APIs carry stability risks across Windows updates without providing sufficient performance advantages over optimized Win32 directory enumeration.
- **Whole-volume scan and path filter for directory targets**: Scanning an entire disk to analyze a small folder wastes I/O and requires unnecessary volume-level permissions.
- **Automatic or repeated UAC prompts**: Prompting during startup or midway through traversal degrades user trust.
- **In-place snapshot mutation**: Modifying snapshots during or after scanning violates immutability and corrupts historical comparisons.
- **Volume locking or dismounting**: Taking volumes offline or locking files disrupts running applications and system stability.
- **Modifying ACLs or taking ownership**: Altering filesystem permissions to read protected files risks damaging system security and user configuration.
- **Reusing scanning helpers for deletion/cleanup**: Combining analysis and file deletion in a single helper breaks the read-only security boundary.
- **Dual scanning on every run**: Concurrently running both raw and documented traversals doubles I/O load and negates performance gains.
- **Heuristic progress guessing**: Displaying fake progress bars or arbitrary ETAs misinforms the user.
- **Silent fallback**: Falling back without user visibility or diagnostic records conceals operational failures.
- **Implicit reparse following or cloud hydration**: Following junctions risks infinite loops, and hydrating cloud files consumes network bandwidth and local disk space.
- **Continuous journal indexing in v1**: Background journal watching introduces complex state synchronization beyond the scope of point-in-time analysis.
