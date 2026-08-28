# Shared Engine and Automation Contract

- **Status**: Accepted
- **Date**: 2026-08-28
- **Decider**: Project owner

PigTree requires a single, transport-neutral domain engine that powers both rich interactive graphical workflows and scriptable, non-interactive automation. We define the shared engine contract: a task-level command and operation interface, dual logical event channels (lossless domain observations and coalescible progress), immutable versioned artifacts with explicit lineage, a typed query algebra over declared grains, strict fail-closed compatibility rules, interactive challenge protocols, resource arbitration, privacy-preserving diagnostics, and machine-first CLI semantics.

This ADR establishes the semantic contract across all clients and automations while explicitly deferring concrete engine performance targets (tracked in #13) and production technology architecture such as language, runtime, packaging, IPC topology, serializer, and UI framework (tracked in #14).

## Context

PigTree provides deep disk space analysis and guarded storage remediation on modern Windows filesystems. Its capabilities span two distinct user surfaces:
1. **Interactive Graphical Interface**: Fast visual exploration, interactive treemap navigation, deep sorting/filtering, live progress feedback, on-demand duplicate verification, and guarded cleanup workflows.
2. **Command-Line Interface and Automation**: Scriptable disk audits, scheduled duplicate discovery, CI/CD space validation, headless reporting, and unattended plan execution under strict safety controls.

Previous architecture decisions established foundational guarantees:
- [ADR 0001: Scanning Subsystem and Privilege Architecture](0001-scanning-and-privilege-architecture.md) separated scanning across a deep observation seam, enforcing standard-user traversal by default, process-isolated elevated helpers, and strict validation of raw metadata parsing.
- [ADR 0002: Guarded Cleanup and Action Safety Architecture](0002-guarded-cleanup-safety.md) established the immutable Action Plan model, live preflight verification before Commit Points, immutable Execution Records, and native system handoffs.

Without a unified, transport-neutral engine contract, desktop tools suffer from severe architectural fragmentation:
- **Disparate GUI vs. CLI Implementations**: Analyzers often maintain separate scan engines for GUI and CLI, causing subtle discrepancies in size accounting, link resolution, and error handling.
- **Uncontrolled Mutation and Leaky Primitives**: Exposing low-level filesystem primitives or mutable in-memory scan buffers allows client layers to bypass preflight checks or corrupt historical observations.
- **Ambiguous Progress and Cancellation**: Unstructured console logs and arbitrary percentage estimates mislead automation scripts, while abrupt process cancellation risks leaving unjournaled mutations in indeterminate states.
- **Fragile Serialization and Schema Drift**: Ad-hoc JSON/XML outputs without versioned contracts break automation pipelines when fields evolve or unknown filesystem features appear.
- **Conflation of UI State with Domain Reality**: Embedding viewport geometry, selected UI tabs, or visual color mappings into saved artifacts prevents headless re-analysis and corrupts domain lineage.

A formal, transport-neutral semantic contract is necessary to guarantee that all PigTree frontends and automation agents operate against identical domain truths, safety checks, and data invariants.

## Decision

### 1. Transport-Neutral Public Seam and Shared Domain Capabilities

The shared engine exposes a transport-neutral, task-level interface. The contract defines high-level domain operations rather than low-level filesystem primitives (such as opening directory handles, enumerating raw entries, or issuing individual file deletion system calls).

#### Shared Domain Capabilities
Graphical and command-line interfaces share identical domain capabilities through the engine seam:
- **Analyze**: Execute time-bounded Analysis Runs against Volume or directory Scan Targets under declared Analysis Profiles.
- **Snapshot**: Construct, save, open, and migrate immutable Analysis Snapshots and Snapshot Enrichments. Opening an artifact loads that immutable record; an Artifact View is explicitly assembled from an exact base snapshot plus an ordered sequence of enrichments rather than constructed silently on open.
- **Query**: Execute typed filters, sorts, groupings, and aggregations across declared Query Grains on explicit Artifact Views.
- **Duplicate Discovery and Verification**: Generate non-content Duplicate Candidate Sets and execute separate Duplicate Verification operations that prove every content-bearing stream under a declared Verification Method and Verification Scope.
- **Action Plan Formulation and Validation**: Create, validate, inspect, and export immutable Action Plans with live risk assessment and reclaim accounting.
- **Action Execution**: Execute validated Action Plans under strict Live Preflight, step journaling, and Commit Point governance.
- **Execution Recovery**: Inspect Execution Records and execute an exact, immutable recovery Action Plan for preserved Recovery Artifacts (not a direct restore primitive).
- **Artifact Export and Import**: Export and import self-describing, integrity-manifested artifacts with optional Redaction Profiles. Imported or foreign Action Plans carry evidence only and require a newly created, locally bound Action Plan before execution.
- **Capability Discovery**: Inspect supported contract versions, command schemas, platform adapters, and operational constraints.

Presentation-specific concerns--including treemap layout tiling, zoom coordinates, viewport clipping, sorting column visual order, theme colors, and UI selection state--are strictly client-side concepts and are prohibited from entering the engine seam, artifacts, or query algebra.

### 2. Normative Command Family and Operation Lifecycle

Clients interact with the engine by submitting **Engine Commands**. A command either executes synchronously (for fast, bounded metadata queries and capability checks) or initiates a stateful, time-bounded **Engine Operation** (for long-running traversal, content verification, or plan execution).

Every long-running Engine Operation receives a unique, opaque Operation ID and transitions through a deterministic, monotonically advancing lifecycle:

```
[Accepted] ---> [Running] ---> [Stopping] ---> [Cancelled] ---> [Settled]
                   |
                   +---------> [Failed]   -------------------> [Settled]
                   |
                   +---------> [Finished] -------------------> [Settled]
```

#### Target Effect and Mutation Scope Distinction
The engine contract strictly distinguishes operations that mutate analyzed filesystem state from those that write internal storage or operate read-only:
- **Analyzed Filesystem Mutation**: Only Action Execution (`execution.run`, `execution.recover`) mutates analyzed filesystem targets (such as removing directory entries, executing native deletions, or replacing hard links). All other command families leave analyzed filesystem state untouched.
- **Artifact Storage and Export Writes**: Snapshot persistence (`snapshot.save`, `snapshot.migrate`) and artifact packaging (`artifact.export`, `artifact.import`) write only to PigTree-owned artifact storage or an explicitly selected export destination; they do not alter analyzed filesystem targets.
- **Read-Only Operations**: Analysis traversal, query execution, duplicate candidate discovery, content stream verification, and plan formulation/validation read analyzed filesystem targets or stored artifacts without mutating either.

#### Normative Command Family Table

| Command Family | Primary Commands | Target Effect / Mutation Scope | Long-Running / Operation | Primary Output / Artifact |
|---|---|---|---|---|
| **Analysis** | `analysis.start`, `analysis.cancel` | Analyzed filesystem read-only (No target mutation) | Yes (`AnalysisRun`) | `AnalysisSnapshot` (Complete or Partial) |
| **Snapshot** | `snapshot.open`, `snapshot.save`, `snapshot.migrate`, `snapshot.inspect` | PigTree artifact storage write (`save`/`migrate`) or read (`open`/`inspect`); Analyzed filesystem untouched | Open/Inspect: No; Migrate: Yes | Loaded `AnalysisSnapshot` / `SnapshotEnrichment` / Migrated Snapshot |
| **Query** | `query.execute`, `query.explain` | Read-only query over explicit `ArtifactView`; Analyzed filesystem untouched | No (Paged / Chunked) | Typed Query Result Set + Provenance |
| **Duplicates** | `duplicates.candidates`, `duplicates.verify`, `duplicates.cancel` | Candidates: Read-only over `ArtifactView`; Verify: Read-only stream inspection on analyzed filesystem | Candidates: No; Verify: Yes | `SnapshotEnrichment` (Verification Evidence) |
| **Plan** | `plan.create`, `plan.validate`, `plan.show`, `plan.export` | Read-only formulation and live preflight preview; Analyzed filesystem untouched | Validate: Yes (Live Observation) | Immutable `ActionPlan` |
| **Execution** | `execution.run`, `execution.cancel`, `execution.recover` | **Mutates Analyzed Filesystem State**; Writes recovery artifacts & execution records | Yes (`ActionExecution`) | Immutable `ExecutionRecord` |
| **Artifact** | `artifact.export`, `artifact.import`, `artifact.verify` | Writes export destination or reads/writes PigTree artifact storage; Analyzed filesystem untouched | Yes (for large packages) | Export Package / Verified Artifact |
| **Capability** | `engine.capabilities`, `engine.diagnostics` | Read-only engine introspection; Analyzed filesystem untouched | No | `CapabilityReport` / Diagnostics |

### 3. Dual Logical Channels and Common Event Envelope

Every active Engine Operation exposes two distinct, ordered logical output channels to the client:
1. **Lossless Domain / Data Channel**: Emits ordered, typed domain observations, Coverage Gaps, verification proofs, or execution step results. This channel supports bounded backpressure and chunked transfer. Observations are never dropped, sampled, or coalesced.
2. **Coalescible Progress / Status Channel**: Emits operational phase transitions, resource consumption, observed item counts, and progress status. To prevent UI lockup and IPC saturation under heavy scanning churn, intermediate progress events may be throttled or coalesced. **Critical lifecycle events, typed challenges, safety warnings, and the final Terminal Result are never coalesced or dropped.**

#### Common Operation Event Envelope
Every event emitted across either channel adheres to a standard, versioned envelope containing:
- `operation_id`: Opaque identifier of the initiating Engine Operation.
- `sequence_number`: Monotonically increasing sequence number scoped strictly to the operation.
- `timestamp`: Unambiguous UTC instant.
- `schema_version`: Semantic version of the event schema.
- `phase`: Current lifecycle phase (`discovering`, `enriching`, `aggregating`, `validating`, `preflighting`, `executing`, `finalizing`).
- `channel`: Logical channel (`data` or `progress`).
- `payload`: Typed event payload (observation batch, progress snapshot, challenge request, or diagnostic).
- `correlation_id`: Optional identifier linking the event to a specific parent operation, query, or execution group.
- `provenance`: Adapter or engine subsystem responsible for the event.

Progress payloads report observed work, completed units, elapsed time, resource usage, active stage technical and plain-language labels, fallback/retry counts, and Coverage Gaps. **Denominators, percentages, and estimated times of arrival (ETAs) are included only when mathematically defensible and accompanied by an explicit denominator confidence rating.** When total volume size or entry count is unknown, the engine reports observed totals without synthetic percentage approximations.

#### Operation Replay Retention
The engine maintains a bounded event replay retention window for active and recently settled Engine Operations. Clients and reconnecting subscribers can query or replay events from a specified monotonic sequence number to recover dropped event context without restarting the underlying operation.

### 4. Terminal Run Outcomes, Cancellation, and Partial Results

Every Engine Operation settles into exactly one persistent **Terminal Result** summarizing its execution.

#### Separation of Run Outcome, Usability, and Coverage
The engine strictly separates three orthogonal concepts:
- **Run Outcome**: How the operation process concluded (`finished`, `cancelled`, or `failed`).
- **Artifact Usability and Integrity**: Whether the resulting data structures are self-consistent, uncorrupted, and valid for querying.
- **Coverage**: How completely the Scan Target was observed relative to the declared Analysis Profile (`complete`, `partial`, or `indeterminate`).

An operation that terminates with `cancelled` or `failed` may still publish a fully valid, self-consistent **Partial Analysis Snapshot** when salvageable observations exist. Such snapshots retain their exact Run Outcome, partial Coverage, recorded Coverage Gaps, Observation Interval, and incomplete aggregates. Reopening or querying a partial snapshot never synthesizes missing values or upgrades Coverage.

#### Cancellation Semantics
- **Idempotency**: A cancellation request (`analysis.cancel`, `duplicates.cancel`, `execution.cancel`) is idempotent. Repeated requests for an in-flight or already settled operation return immediate confirmation.
- **Read-Only Operations**: Immediately halt scheduling new traversal or I/O, cancel outstanding asynchronous platform queries where supported, flush in-flight completed observations into a coherent partial snapshot if salvageable, and transition from `stopping` to `cancelled`.
- **Duplicate Verification**: Halts stream verification immediately, discards in-flight unverified comparisons, records verified subset enrichments up to the cancellation boundary, and settles with unverified candidates marked as `unverified_due_to_cancellation`. Partially verified candidates are never upgraded to Verified Duplicate Sets.
- **Action Execution**: Upon accepting a cancellation request, the executor transitions to `stopping` and executes no new operations. For the currently in-flight operation:
  - If execution has not reached its operation-specific **Commit Point**, execution halts immediately without attempting the mutation.
  - If execution is at or beyond the Commit Point, the operation must run to a verified outcome or execute plan-authorized step recovery.
  - The operation never simulates global multi-file transaction rollback; settled mutations remain settled and are durably recorded in the `ExecutionRecord`.

### 5. Immutable Artifact Hierarchy, Lineage, and Compatibility

All long-lived engine data is stored in immutable, kind-versioned, content-manifested artifacts addressed by opaque unique identifiers.

#### Artifact Hierarchy and Lineage Table

| Artifact Kind | Immutability | Dependencies and Lineage | Primary Contents | Mutation Authority |
|---|---|---|---|---|
| **Analysis Snapshot** | Immutable | Scan Target, Analysis Profile, Volume Identity, Observation Interval | Directory Entry tree, Filesystem Object graph, Content Streams, sizes, timestamps, attributes, Coverage, Coverage Gaps | None (Read-only evidence) |
| **Snapshot Enrichment** | Immutable (Append-only) | Base `AnalysisSnapshot` ID + Version, Observation Interval | Verified Duplicate Sets, Verification Methods/Scopes, targeted re-observations, verification evidence | None (Read-only evidence) |
| **Artifact View** | Virtual / Declared | Base `AnalysisSnapshot` + Ordered compatible `SnapshotEnrichment` list | Unified logical presentation of base snapshot overlaid with specific enrichments | None (Read-only query target) |
| **Action Plan** | Immutable | Source `ArtifactView`, Preconditions, Reclaim Analysis, Security Context | Target entry paths, Object Identities, requested mutations, Recovery Classes, Action Risk Classes, Keeper pairings | Authority for Live Preflight and Execution (when locally bound) |
| **Execution Record** | Immutable | Source `ActionPlan` ID + Integrity Digest, Execution Interval | Live preflight observations, Commit Points, per-step outcomes, native platform results, Recovery Artifact references | None (Historical audit and recovery evidence) |
| **Export Package** | Immutable | Source Artifact IDs, Redaction Profile (if applied), Package Digest | Self-contained domain data, integrity manifests, provenance, Coverage metadata, optional rebuildable indexes | Governed by contained artifacts |

#### Versioning, Persistence, and Compatibility Rules
- **Contract Versioning**: The engine contract adheres to Semantic Versioning (`major.minor.patch`). Minor updates are strictly additive (introducing new optional fields, additional query operators, non-breaking diagnostic categories, optional capability flags). Breaking schema changes, removed fields, or altered semantics require a major version bump.
- **Fail-Closed vs. Fail-Safe Unknown Elements**: Engine clients and parsers must fail closed when encountering unknown required fields, unknown command verbs, unknown Action Risk Classes, unknown Recovery Classes, or unsupported artifact schema major versions. Unknown optional fields in minor schema versions must be safely accepted or ignored to preserve forward compatibility. Unknown enum values in required fields must never default silently to a known value.
- **Lineage Integrity**: Every artifact embeds an integrity digest under a declared cryptographic integrity algorithm covering its entire payload and explicitly declares the IDs and digests of all ancestor artifacts.
- **Schema Migration**: Opening an older supported snapshot version never mutates the original on-disk artifact. Migration executes as an explicit, non-destructive operation producing a new versioned artifact with recorded lineage to the original source.
- **Degraded Read-Only Opening**: If an artifact has localized corruption or unsupported non-critical sections, the engine may open independently coherent, verified sections in degraded read-only mode, recording explicit Coverage Gaps. Mutation planning is strictly prohibited against degraded or corrupted artifacts until fresh live observations resolve all gaps.
- **Artifact Packages and Import Trust**: Export Packages bundle one or more immutable artifacts alongside a declared integrity manifest that covers all included artifacts and their lineage records. Imported or foreign Action Plans carry evidence only and possess zero execution authority; executing actions in the local environment requires compiling and validating a newly created, locally bound immutable Action Plan under the local security context.

### 6. Typed Query Algebra, Query Grain, and Value Knowledge Semantics

The engine provides a transport-neutral, typed query algebra for querying, filtering, and aggregating domain facts across immutable artifacts.

#### Query Formulation and Query Grains
Every query submitted to `query.execute` explicitly declares:
- **Source**: Exactly one immutable `ArtifactView` (or an explicit pair of views for comparison/differential queries).
- **Query Grain**: The structural unit of the result set:
  - `directory_entry`: One row per Directory Entry (path-centric view).
  - `filesystem_object`: One row per distinct Filesystem Object (identity-centric view; shared objects appear once).
  - `content_stream`: One row per named/unnamed Content Stream.
  - `duplicate_candidate_set`: One row per candidate or verified duplicate grouping.
  - `volume_reconciliation`: Volume-level used/free/unattributed reconciliation summary.
- **Knowledge Policy**: The explicit handling rule for incomplete Value Knowledge (`Known`, `Not Observed`, `Unavailable`, `Not Applicable`).
- **Predicates**: Composable boolean expressions over attributes, sizes, timestamps, classifications, risk classes, and entry depths.
- **Aggregations**: Computations of totals, counts, distributions, or histograms, explicitly declaring `referenced` vs. `unique` accounting rules and root/kind inclusion.
- **Ordering**: Total, deterministic sort order with stable tie-breakers (e.g., secondary sort by Object Identity).
- **Pagination**: Zero-based offset/limit or opaque continuation tokens bound to normalized query fingerprints (predicates, ordering, grain, projection) and the exact source Artifact View revision/digest to prevent token reuse across mismatched queries or changed data.

#### Three-Valued Logic and Knowledge Policy
Field evaluations follow three-valued logic (`match`, `no_match`, `unknown`). The query's **Knowledge Policy** explicitly governs how `unknown` evaluations behave:
- `exclude_unknown`: Predicates matching true return rows; `unknown` is treated as non-matching.
- `include_unknown`: Predicates matching true or unknown return rows.
- `separate_unknown`: Evaluated rows are partitioned into matching, non-matching, and indeterminate result sets.

**Coercion of unknown values to zero, empty string, or false is strictly prohibited.** Aggregations over partially observed datasets return a **Known Subtotal** accompanied by an explicit completeness rating and list of contributing Coverage Gaps.

### 7. Duplicate Candidate Discovery and Verification Contract

Duplicate remediation requires strict separation between heuristic grouping and bit-for-bit content verification:

```
[Analysis Snapshot] ---> [Duplicate Candidates] ---> [Verification Operation] ---> [Verified Duplicate Set]
(Metadata Evidence)     (Non-Content Grouping)     (Declared Method/Scope)    (Snapshot Enrichment)
```

#### Duplicate Candidate Discovery
- Executed via `duplicates.candidates` over an `ArtifactView`.
- Groups distinct Filesystem Objects based purely on non-content metadata (such as identical stream Logical Sizes, filename patterns, or fast filesystem attributes).
- Multiple Directory Entries pointing to the same Filesystem Object (Hard Links) are recognized as aliases of one object and are never grouped as duplicate candidates against each other.
- Output: An in-memory or persisted Candidate Set. Candidates carry no equality guarantee.

#### Duplicate Verification Operation
- Initiated as a separate, long-running Engine Operation via `duplicates.verify`.
- Targets exact, explicit Object Identities and declared **Verification Scopes** (default: unnamed primary data stream; optional: all named Content Streams).
- Governed by explicit **Resource Policies** and **Interaction Policies** regarding offline/cloud file hydration, byte caps, and network access.
- Emits progressive verification events with byte counts, stream identifiers, and throughput metrics.
- Emits an immutable `SnapshotEnrichment` containing:
  - Recorded **Verification Method** (e.g., cryptographic stream digest verification or full-stream comparison) and algorithm version.
  - Verification Scope covered by the proof.
  - Per-object and per-set status: `verified_equal`, `mismatch`, `inaccessible`, `cloud_placeholder_skipped`, or `cancelled`.
  - Precise observation interval and diagnostic logs.
- **Safety Gate**: Only candidates proven equal across every content-bearing stream in their full requested Verification Scope under the declared Verification Method are classified as a **Verified Duplicate Set**. Candidate evidence alone remains strictly insufficient. Partial comparisons or failed stream reads immediately fail closed and disqualify the candidate set from deduplication eligibility.

### 8. Plan Formulation, Live Preflight, and Execution Boundary

As mandated by [ADR 0002](0002-guarded-cleanup-safety.md), PigTree enforces a strict boundary between preview analysis and storage mutation.

```
[Artifact View] ---> [plan.create] ---> [Action Plan] ---+---> [plan.validate] (Live Preview)
                                                         |
                                                         +---> [execution.run] ---> [Live Preflight] ---> [Commit Point] ---> [Execution Record]
```

#### Plan Formulation (`plan.create`)
- Inputs: Explicit source `ArtifactView`, target Directory Entries, expected Object Identities, requested operations (e.g., recycle, permanent delete, same-volume Hard Link replacement), designated keeper entry pairings, and declared Recovery Classes.
- The engine calculates:
  - Applicable **Action Risk Classes** (`routine`, `caution`, `protected`, `prohibited`) with additive reason codes.
  - Expected **Reclaimable Allocation** (reported as exact value, defensible range, or Unknown; accounting for Hard Links and External Reference Uncertainty).
  - Required execution partitioning (split by privilege level, recovery mode, and dependency order).
  - Required consent tiers and challenge specifications.
- Wildcards, recursive path expansions, and implicit descendant deletions are prohibited. Every affected entry must be explicitly manifested.

#### Preview Validation (`plan.validate`) vs. Action Execution (`execution.run`)
- `plan.validate`: Inspects live filesystem state to preview eligibility, risk classes, and potential stale preconditions without acquiring mutation locks or modifying storage. **Validation provides zero execution authority.**
- `execution.run`: Accepts an exact, immutable Action Plan ID, integrity digest, and explicit Interaction/Consent tokens.
- **Live Preflight**: Immediately before each step in an execution group, the engine performs live re-observation under the active execution context. It verifies volume identity, file IDs, link counts, reparse tags, parent paths, security descriptors, and content evidence.
- If any preflight condition fails, the step is aborted with `precondition_failed`, the dependent group is halted, and zero mutation occurs for the rejected step. Earlier independent steps or execution groups may already have committed, and subsequent independent groups may continue according to the immutable plan.
- Each settled step records its exact outcome, native platform codes, and Recovery Artifact identifiers in an immutable `ExecutionRecord`.

#### Execution Recovery (`execution.recover`)
- Accepts and executes an exact, immutable recovery Action Plan referencing specific Recovery Artifacts, target paths, and preconditions; it is not a direct restore primitive.
- Follows the same Live Preflight, step journaling, and Commit Point safety guarantees as standard execution.

#### Import Trust Rule
- Imported or foreign Action Plans carry evidence only; they possess zero mutation authority.
- Executing actions in the local environment requires compiling and validating a newly created, locally bound immutable Action Plan under the local security context.

### 9. Interaction Policy, Typed Challenges, and Authority Delegation

The shared engine contains no UI rendering code, message boxes, or platform dialog loops. All interactive decisions are modeled as structured, typed **Challenges**.

#### Interaction Policies
Every submitted Engine Command declares its interactive posture:
- `interactive_allowed`: The engine may pause an operation and emit a challenge event, awaiting a client response before proceeding.
- `interactive_forbidden`: The engine must never pause for user interaction. Any condition requiring interactive confirmation (such as cloud hydration, UAC elevation, or permanent deletion warnings) fails closed immediately with an appropriate diagnostic code.
- `interactive_required`: The operation expects interactive guidance and fails immediately if run in a non-interactive environment without challenge handlers.

#### Typed Challenge Protocol
When an operation requires user confirmation:
1. The engine pauses the affected operation or execution step and emits an `OperationEvent` of type `challenge_requested`.
2. The payload contains:
   - Unique, cryptographically random `challenge_nonce`.
   - Challenge Type (`permanent_deletion_confirmation`, `cloud_hydration_consent`, `elevation_consent`, `risk_caution_acknowledgement`).
   - Exact affected scope (item paths, byte count, risk reasons, or provider details).
   - Expiration timeout.
3. The client presents the challenge to the user (via GUI modal dialog, CLI interactive prompt, or automation policy) and submits a `challenge.respond` command binding the `challenge_nonce`, decision (`approved`, `rejected`), and optional response parameters.
4. The engine validates the nonce, caller authorization, and response parameters before resuming or failing the step.

#### Authority Delegation and Elevation
- Connecting to the engine grants zero implicit authority.
- Caller authority semantically derives from the authenticated local caller (process topology and concrete credential mechanisms are deferred to #14).
- When an operation requires administrative privilege (such as elevated traversal or protected cleanup), elevation occurs strictly via the short-lived, least-privilege helper architecture defined in [ADR 0001](0001-scanning-and-privilege-architecture.md) and [ADR 0002](0002-guarded-cleanup-safety.md).
- Elevated helpers authenticate against the exact operation nonce and plan digest, execute solely within their authorized execution group, and terminate immediately upon completion.

### 10. Concurrency Arbitration and Resource Management

The engine maintains no global "current scan" singleton. Multiple independent operations may execute concurrently subject to strict resource arbitration.

#### Resource Policies
Clients assign a **Resource Policy** declaring named resource intent and budgets to each operation:
- `foreground_balanced`: Balanced concurrency and responsive I/O suited for interactive user sessions.
- `background_low_impact`: Constrained concurrency, throttled I/O priorities, minimized working set, and yield-on-contention suited for background maintenance and non-interactive automation.
- `benchmark`: Controlled, declared, and reported resource settings configured for reproducible performance measurement (with specific targets and configurations deferred to #13 and #14).

Resource policies declare named resource intent and enforce budgeted limits on concurrency, memory bounds, I/O rates, network usage, cloud hydration limits, and temporary storage allocation. The engine reports effective, clamped, or rejected policies in its initial operation lifecycle event.

#### Concurrency and Collision Arbitration
- **Read Operations**: Multiple Analysis Runs, queries, and verification operations can execute concurrently across distinct or overlapping Scan Targets, bounded by aggregate memory and concurrency budgets.
- **Mutating Operations**: `execution.run` and `execution.recover` acquire exclusive, typed reservations over their targeted Directory Entries, Filesystem Objects, and volume recovery vaults.
- If a mutating operation encounters a conflicting lock held by another operation, the engine rejects the command with `resource_conflict` or enqueues it according to caller policy. **Silent cancellation, handle stealing, or implicit target retargeting is strictly prohibited.**

### 11. Diagnostics, Privacy, and Redaction Profiles

Diagnostics and error reporting are structured, typed, and privacy-preserving by default.

#### Diagnostic Envelope and Evidence
Every diagnostic emitted by the engine (in event streams, artifacts, or CLI output) contains:
- `code`: Stable, documented diagnostic identifier (e.g., `FS_ACCESS_DENIED`, `NTFS_MFT_CORRUPT_RECORD`, `PREFLIGHT_LINK_COUNT_MISMATCH`).
- `category`: Classification (`filesystem`, `privilege`, `validation`, `preflight`, `resource`, `integrity`).
- `severity`: `info`, `warning`, `error`, `fatal`.
- `scope`: Affected Scan Target, Directory Entry path, or Filesystem Object ID.
- `retryability`: `non_retryable`, `retryable_immediately`, `retryable_with_elevation`, `retryable_with_policy_change`.
- `result_usability`: Impact on current artifact (`artifact_valid_complete`, `artifact_valid_partial`, `artifact_unusable`).
- `message`: Human-readable summary.
- `native_cause`: Structured OS error code (e.g., Win32 `ERROR_ACCESS_DENIED` [5], `NTSTATUS` `STATUS_BUFFER_TOO_SMALL`).
- `diagnostic_evidence`: Underlying error codes, observed attributes, and contextual facts supporting the diagnostic.
- `suggested_policy`: Recommended remediation or policy action (e.g., retry with elevation, adjust Resource Policy, add safeguard exception, or ignore benign gap).

#### Local vs. Redacted Diagnostic Channels
The engine maintains a strict separation between diagnostic channels:
- **Full-Local Diagnostic Channel**: Retains raw native error arguments, unredacted paths, and detailed platform diagnostics for local troubleshooting and local artifact storage.
- **Shareable / Redacted Diagnostic Output**: Strips or pseudonymizes sensitive paths, user identifiers, and native parameters per the active Redaction Profile.

#### Privacy and Redaction Profiles
To allow users to export diagnostics, benchmarks, and space reports without leaking personally identifiable information (PII) or confidential corporate directory structures:
- Authoritative local artifacts store full observed metadata securely on the local machine.
- Export and sharing commands support explicit, versioned **Redaction Profiles**:
  - `obfuscate_paths`: Replaces directory and file names with consistent pseudonyms while preserving directory hierarchy and tree depth.
  - `mask_user_identities`: Masks or strips Owner security principals and SID strings.
  - `scrub_native_messages`: Strips sensitive path arguments from native Win32/NT error strings.
- Redaction preserves declared structural relationships and statistical aggregates, but may pseudonymize Object Identity (e.g., generating consistent pseudonyms within the export rather than revealing exact local filesystem identifiers).
- Exact local Object Identities and observed paths are sensitive; redacted output **carries zero execution authority for mutation**.
- PigTree contains no automated telemetry, cloud tracking, or background reporting services.

### 12. Capability Discovery and Extensibility Boundary

The engine provides runtime capability introspection to ensure client-engine compatibility across diverse Windows versions and deployment environments.

#### Capability Discovery (`engine.capabilities`)
Returns a structured `CapabilityReport` detailing:
- `contract_version`: Supported semantic contract versions.
- `supported_commands`: List of available command verbs and accepted schema versions.
- `filesystem_adapters`: Supported adapters per filesystem (e.g., `win32_directory_traversal`, `ntfs_raw_mft_reader`, `refs_directory_traversal`) and their availability under current privileges.
- `elevation_support`: Availability of privilege elevation helpers and caller authority level.
- `cleanup_capabilities`: Supported platform deletion modes (Recycle Bin, permanent delete, NTFS same-volume Hard Link replacement).
- `query_features`: Supported query operators, index accelerators, and export formats.
- `resource_limits`: Configurable minimum and maximum concurrency, memory, and I/O limits.

#### Extensibility Boundary
- In v1, the shared engine core does not expose a stable third-party in-process plugin ABI.
- Integrations and custom tooling interact exclusively through the transport-neutral command seam, CLI standard streams, versioned artifacts, and namespaced extension properties.

### 13. Automation, CLI Contracts, Machine Streams, and Exit Classes

The command-line interface provides full automation parity with the GUI, designed for scriptability, pipeline integration, and unattended execution.

#### Standard Stream Contracts
- **stdout (Machine Data and Events)**:
  - When streaming long-running operations: Emits a stream of versioned **NDJSON** (Newline Delimited JSON) envelopes, terminating with a final `terminal_result` envelope.
  - When executing synchronous or bounded queries: Emits a single, validated JSON document (or NDJSON record stream if paging/streaming).
  - Explicit tabular export: When requested via `--format csv`, stdout emits RFC 4180 CSV with declared Query Grain, fixed headers, locale-neutral numeric values, and explicit Value Knowledge indicator columns (`_status`, `_provenance`).
- **stderr (Diagnostics and Human Status)**:
  - Formatted progress bars (interactive TTY only), human-readable log messages, and diagnostic summaries.
  - Machine parsers can safely ignore stderr or capture it exclusively for diagnostic logging.

#### Locale-Neutral Data Representation
All machine-readable outputs adhere to strict locale-neutral standards:
- **Numbers**: Integer byte counts (no floating-point truncation; no locale comma/period ambiguities).
- **Timestamps**: Unambiguous UTC timestamp representations with explicit filesystem timestamp kind annotations.
- **Paths**: Exact Windows path spelling preserved in raw strings, paired with a normalized lowercase comparison representation where appropriate.
- **Enums**: Lowercase snake_case identifiers (`finished`, `not_observed`, `routine`, `verified_equal`).

#### Normative CLI Exit Classes

| Exit Code | Exit Class | Semantic Meaning | Terminal Output Guarantee |
|---|---|---|---|
| `0` | `SUCCESS` | Operation finished with terminal success under the selected policy and profile. | Valid artifact / complete result set emitted under selected policy. |
| `1` | `OPERATION_FAILED` | Operation encountered fatal execution or platform error (in Action Execution, earlier mutation steps may have committed). | Fatal diagnostic in Terminal Result; salvageable partial artifact or immutable `ExecutionRecord` detailing settled steps. |
| `2` | `COMMAND_ERROR` | Malformed CLI arguments, invalid schema, or unsupported command. | Error diagnostic emitted to stderr / JSON. |
| `3` | `CANCELLED` | Operation was cleanly cancelled by user or automation signal. | Operation settled with `RunOutcome = cancelled`; may publish a coherent partial artifact if salvageable; `ExecutionRecord` emitted if mutation was in flight. |
| `4` | `COVERAGE_GAPS_PRESENT` | Operation finished, but policy-significant Coverage Gaps or incomplete observations are present relative to the profile. | Valid partial artifact published with explicit Coverage Gaps manifested. |
| `5` | `INTERACTION_REQUIRED` | Operation required user interaction, consent, or privilege elevation that was not provided or occurred under an `interactive_forbidden` policy. | Challenge or elevation specification emitted in terminal envelope. |
| `6` | `PREFLIGHT_FAILED` | Action Execution Live Preflight rejected one or more steps. | Precondition failure diagnostics in `ExecutionRecord`; zero mutations occur for rejected steps (earlier committed steps and independent group outcomes are preserved). |

Strict automation policies (e.g., requiring complete coverage or zero warnings) promote selected warnings or non-fatal Coverage Gaps into Exit Class 4 or another documented non-zero policy exit class for automated pipeline enforcement.

### 14. Invariant Catalog

The shared engine enforces a set of immutable system invariants across all operations, artifacts, and clients:

1. **Snapshot Immutability**: Once an `AnalysisSnapshot`, `SnapshotEnrichment`, `ActionPlan`, or `ExecutionRecord` is finalized and assigned an ID, its content and digest are immutable.
2. **Coverage Honesty**: Missing, inaccessible, or unattempted observations are recorded as `Unavailable` or `Not Observed` with provenance; they are never coerced to zero, omitted silently, or represented as empty containers.
3. **No Phantom Deduplication**: A `VerifiedDuplicateSet` requires proving every content-bearing stream under the declared Verification Method and Verification Scope. Metadata-only candidate groupings or candidate evidence alone never authorize Hard Link replacement.
4. **Preflight Before Commit**: Every mutation in an Action Execution must pass Live Preflight immediately prior to its Commit Point. If preflight fails, that step is aborted with zero mutation, dependent steps in its group are halted, and earlier committed or independent steps are accurately recorded in the `ExecutionRecord`.
5. **No Synthetic Rollback**: PigTree never claims multi-file transaction rollback across Windows filesystems. Settled operations remain settled, and partial executions record exact step outcomes in the `ExecutionRecord`.
6. **Alias Distinction**: Multiple Directory Entries sharing a Filesystem Object are distinct entries referencing one object. Deleting an alias deletes the Directory Entry, freeing storage only when the final reference is removed.
7. **Redaction Authority Prohibition**: Redacted artifacts and query projections carry zero authority for Action Plan formulation or execution. Exact identities may be pseudonymized to protect privacy while preserving structural relationships and aggregates.
8. **Channel Integrity**: Data channel observations are lossless and strictly ordered. Coalescing is restricted entirely to intermediate progress events.
9. **Fail-Closed on Unknown Required Elements**: Unknown required schema fields, command verbs, risk classes, or recovery classes immediately abort processing; they never resolve to default assumptions. Unknown optional fields in minor schema versions are safely ignored.
10. **Clean Engine Boundary**: The shared engine contains zero UI framework code, window handles, or interactive modal dialog loops.

## Consequences

### Positive
- **Guaranteed Parity Across Interfaces**: GUI, CLI, and automated agents execute identical domain logic, validation checks, and safety rules.
- **Robust Automation and Scriptability**: Versioned NDJSON streams, deterministic exit classes, and typed challenges enable reliable pipeline integration.
- **Verifiable Auditability and Safety**: Immutable artifacts, explicit lineage tracking, and live preflight eliminate stale-scan race conditions and data corruption risks.
- **Privacy and Data Control**: Redaction Profiles allow safe sharing of storage audits and diagnostics without exposing sensitive file paths or user identities.
- **Clean Architectural Separation**: Decoupling the semantic domain contract from technology choices allows independent evolution of UI frameworks, storage engines, and IPC mechanisms.

### Negative and Trade-offs
- **Contract Surface Breadth**: Implementing dual logical channels, typed challenge protocols, and comprehensive query algebras requires disciplined architectural scaffolding.
- **Schema Rigidity**: Strict fail-closed versioning requires deliberate migration paths and schema governance for every new engine capability.
- **Multi-Step Automation Overhead**: Headless cleanup requires a two-step plan generation and execution workflow rather than immediate one-liner deletion commands.

## Considered Options

The following architectural alternatives were evaluated and rejected:

- **Direct Win32/Filesystem API Exposure to UI**: Rejected. Allowing the UI layer to directly enumerate directories or invoke file deletion APIs bypasses invariant validation, preflight checks, and audit logging.
- **In-Memory Mutable Snapshot Model**: Rejected. Modifying snapshot objects in place as scans progress prevents reliable historical comparison, breaks multi-client query consistency, and violates lineage guarantees.
- **Single Mixed Output Stream for CLI**: Rejected. Interleaving log messages, progress bars, and domain data on stdout corrupts machine parsers and forces fragile regex scraping.
- **Automatic Heuristic Rollback**: Rejected. Windows filesystems do not support atomic multi-file rollback. Claiming transactional undo creates false confidence and masks partial execution states.
- **Metadata-Only / Candidate Evidence Deduplication**: Rejected. Authorizing Hard Link consolidation based on candidate metadata or unverified stream comparisons causes catastrophic silent data corruption.
- **Implicit "Latest" Artifact View Resolution**: Rejected. Automatically binding queries or plans to the latest unverified enrichment creates race conditions; artifact views must explicitly declare their base snapshot and enrichment chain.
- **Coercing Unknown Values to Zero**: Rejected. Treating inaccessible or unobserved file sizes as zero distorts capacity reconciliation and leads users to delete seemingly "empty" directories that contain protected data.
- **UI State in Saved Artifact Packages**: Rejected. Storing window positions, treemap layout coordinates, or expanded tree nodes inside domain artifacts breaks headless re-analysis and pollutes versioned schemas.
- **Interactive UI Dialogs inside the Engine**: Rejected. Putting message boxes or credential prompts inside the engine blocks non-interactive automation and violates headless operation principles.
- **Uncontracted In-Process Plugins**: Rejected. Providing a stable third-party in-process plugin ABI in v1 introduces ABI fragility and expands the security boundary; integrations interact through the transport-neutral seam.
