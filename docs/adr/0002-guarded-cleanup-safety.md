# Guarded Cleanup and Action Safety Architecture

- **Status**: Accepted
- **Date**: 2026-08-28
- **Decider**: Project owner

PigTree requires a disciplined, verified cleanup architecture to remediate disk capacity without risking data loss or system instability. We separate read-only analysis from mutation via an explicit Action Plan executor, restrict v1 mutations to verified Directory Entry deletion and content-verified same-volume NTFS Hard Link replacement, route system and cloud resources to native handoffs, enforce live preflight verification before Commit Points, and record immutable Execution Records.

## Context

Disk space analysis helps users identify storage consumption, but reclaiming capacity presents major hazards on modern Windows systems:

- Filesystem state changes continuously. Historical Snapshots capture past observation intervals, not live truth. Mutating storage from unverified historical scans risks modifying or deleting files that were moved, altered, or replaced.
- File deletion semantics vary widely across filesystems and storage media. Shell recycling behavior depends on volume type, group policy, user configuration, and item size; it cannot be assumed for all fixed or removable media. Permanent deletion is irreversible, yet standard tools frequently blur recoverable recycling with permanent removal.
- Deduplication via Hard Link consolidation carries severe corruption risks if based on partial hashes, stream-subset comparisons, or unverified link counts. Hard Link replacement must verify all Content Streams and account for External Reference Uncertainty across Directory Entries.
- Windows system assets (component stores, driver stores, page/swap/hibernation files, shadow copies) and cloud-managed files (where local files represent remote state) cannot be safely manipulated with generic file deletion APIs.
- Administrative elevation introduces systemic blast radius. Analysis requires scanning breadth, whereas cleanup requires least privilege, strict confinement, and auditable outcomes.

As established in [ADR 0001: Scanning Subsystem and Privilege Architecture](https://github.com/AFlyingP/PigTree/blob/decision/scanning-privilege-architecture/docs/adr/0001-scanning-and-privilege-architecture.md), scanning and analysis components remain strictly read-only. We now define the architecture and safety boundaries for guarded cleanup and storage mutation.

## Decision

### 1. Architectural Seam and Process Isolation

- **Read-Only Scan Seam**: Scan helpers, analyzers, and caching subsystems remain strictly read-only and are never reused for mutation.
- **Dedicated Action Plan Executor**: Neither graphical nor command-line interfaces invoke Windows file mutation APIs directly. All user intents compile into a validated, immutable Action Plan submitted to a single centralized executor that emits an event stream and an immutable Execution Record.
- **Short-Lived Mutation Helpers**: Action Execution occurs through separate, short-lived worker processes partitioned by authorized execution group and required integrity level (medium vs. elevated). Persistent elevated background services are prohibited. The helper authenticates via IPC against the exact plan nonce and operation set, performs only authorized steps, and exits immediately after group completion. Elevation is requested only at execution time after preview inspection; declining elevation leaves system state unchanged. Mutation helpers cannot widen targets, alter Access Rules, change Owner metadata, or bypass security. Mutation helper reports are treated as untrusted until verified.

### 2. Action Plan and Live Preflight

- **Immutable Exact Authorization**: An Action Plan represents an exact, immutable authorization identifying:
  - Target parent Directory Entries and expected names.
  - Strongest available Object Identity for each expected Filesystem Object.
  - Requested operations and live preconditions.
  - Declared Recovery Class and Reclaimable Allocation (exact, range, or Unknown).
  - External Reference Uncertainty and known risks.
  - Action Risk Class and explicit additive reasons.
  - Security context, integrity level, execution and dependency groups, and consent requirements.
- **No Expansion or Wildcards**: Historical Snapshots seed preview only. Live targeted re-observation under the execution token creates an executable Action Plan. Plans prohibit wildcards, implicit descendant expansion, and in-place retargeting.
- **Live Preflight**: Immediately before execution, under the active execution token, each target is safely reopened without following Reparse Points. Preflight verifies Volume identity, strongest Object Identity, expected parent and name, entry kind, path confinement, link count, Reparse Point tags, cloud state, EFS state, security attributes, relevant size and allocation, operation-specific current content evidence, risk classification, and recovery viability. Any divergence stales the step, rejecting execution and requiring a fresh Action Plan rather than an in-place override. Preflight establishes current eligibility but is not an atomic guarantee across subsequent operations.

### 3. Action Risk Classes and Safeguard Catalog

Every proposed operation is assigned an Action Risk Class derived additively from explicit inspectable reasons, including canonical location plus identity/system/package evidence, attributes, Reparse Point/cloud/EFS state, Coverage and identity quality, Hard Links and External Reference Uncertainty, declared recovery, elevation requirement, and operation type:

- **`routine`**: Standard safeguards and single preview confirmation suffice. This classification indicates ordinary operational parameters; it is not a claim that deletion is universally safe.
- **`caution`**: Elevated uncertainty or operational risk (such as External Reference Uncertainty, non-empty structures, or elevated privilege); requires focused reason acknowledgement.
- **`protected`**: Targets managed by Windows or cloud providers (component stores, driver stores, page/swap/hibernation files, shadow copies, package stores, installed applications, or cloud-managed entries); direct mutation is replaced with structured native/provider handoff.
- **`prohibited`**: No sufficiently safe or authorized mutation path exists (for example, active PigTree recovery vaults, execution journals, active records, unknown Reparse Point tags, unmatched duplicate streams, or EFS deduplication in v1). Direct mutation is completely blocked.
- **Safeguard Catalog**: A versioned local catalog of protection rules ships with PigTree releases. Users may add custom protection rules but cannot disable or weaken built-in safeguards.

### 4. Consent and Execution Groups

- **Execution Partitioning**: Action Plans partition into distinct execution groups split at least by Recovery Class, privilege level, Action Risk Class, and operational dependencies.
- **Confirmation Tiers**:
  - *Routine Recoverable Operations*: Single accessible preview confirmation.
  - *Caution Operations*: Focused, per-reason acknowledgement.
  - *Permanent Deletion*: Distinct group and operation requiring a plan-specific generated typed challenge, exact item count confirmation, explicit scope review, and an unambiguous no-recovery disclosure. Permanent deletion is never an automatic fallback when recoverable execution fails.
- **Accessibility Parity**: All risk reasons, recovery expectations, Reclaimable Allocation uncertainties, keeper relationships, challenges, outcomes, vault states, and handoffs provide full semantic parity across keyboard, screen-reading, and non-visual interfaces. Visual styling, colour, diagrams, drag-and-drop, and indentation are never the sole carriers of safety information.

### 5. Deletion Semantics and Recovery

- **Recycle Bin Operations**: Supported via platform IFileOperation where PigTree requests recycle-only behavior through supported flags and preserves any platform permanent-deletion warning. Preflight evaluates volume support, media type, group policy, quota, and size thresholds where defensible without assuming fixed, removable, FAT, or exFAT volumes universally support or lack Recycle Bin. Explicit permanent deletion warnings from the platform are preserved and never auto-consented. A step is recorded as recoverable only when per-item Shell outcome establishes recycled disposition—using the returned destination/recovery item when available, plus per-item success and non-permanent disposition evidence. If this cannot be established, PigTree classifies the outcome as failed or indeterminate and does not report a Recovery Artifact or silently retry permanently. PigTree does not claim a universal durable recovery token. Windows manages retention and restoration; PigTree records the reference and hands off. Immediate Reclaimable Allocation is reported as zero (or conditional upon eventual purge, subject to remaining Hard Links and open handles). Indefinite recovery is not guaranteed.
- **Permanent Deletion**: Supported using standard per-entry file, directory, and Reparse Point Win32 primitives without privilege-bypassing flags. PigTree prohibits POSIX force unlinking, automatic read-only attribute clearing, delayed reboot deletion (MoveFileExW / PendingFileRenameOperations), permission or ownership changes, backup/restore privilege bypass (SE_BACKUP_NAME / SE_RESTORE_NAME for deletion), and EFS bypass. Failures settle as per-operation error outcomes.
- **Directories and Reparse Points**: Non-empty directory deletion freezes an exact observed subtree manifest. Traversal never follows Reparse Points. Any new, moved, or materially changed descendant stales the dependent subtree. Understood and revalidated Reparse Point deletion removes only the Directory Entry itself, never the destination object. Changed or unknown reparse tags are prohibited.
- **Cloud-Managed Storage**: Deletion of cloud-managed entries is protected in v1. PigTree explains local Allocated Size versus remote storage impact and hands off to the provider or Explorer. PigTree does not hydrate, dehydrate, unpin, or delete cloud content directly.

### 6. Same-Volume NTFS Hard Link Replacement

- **Eligibility**: Restricted in v1 to ordinary files on the same NTFS Volume. Requires stable File Reference Numbers, verified link counts, complete relevant Coverage (normally whole-Volume observation to resolve External Reference Uncertainty), parent write access, and matching Owner, Access Rules, and EFS state. All content-bearing Content Streams must be Known and currently fully verified under a recorded Verification Method and Scope. Sampling or historical hashes cannot authorize replacement. Reparse Points, cloud files, package stores, and EFS-encrypted files are prohibited from Hard Link replacement in v1. Differences in timestamps, attributes, or Storage Characteristics must be explicitly presented and acknowledged.
- **Keeper Selection**: The user explicitly chooses the surviving keeper entry, assisted by evidence-based recommendations. Previews clearly distinguish Directory Entry changes from surviving Filesystem Object metadata.
- **Journaled Sequence**:
  1. Record step intent in a durable per-step local journal.
  2. For recoverable mode: create and verify a preservation Hard Link to the victim Filesystem Object in a deterministic restricted per-volume PigTree recovery vault (which remains intentionally location-neutral). For immediate-reclaim mode: create a temporary staging link in a plan-owned same-volume temporary staging area that is protected from generic cleanup while active, with its exact location deferred. If staging or journal creation fails, the operation is ineligible.
  3. Create a temporary Hard Link to the keeper Filesystem Object in the victim parent directory.
  4. Durably record verified identities.
  5. Execute the namespace Commit Point via supported same-volume rename-over.
  6. Reopen and verify that the victim Observed Path resolves to the keeper Filesystem Object.
  7. Retain the preservation Recovery Artifact in the vault or permanently purge the temporary staging link according to the declared Recovery Class.
- **Failure and Recovery Semantics**: Filesystem transactionality is not assumed. Failure or indeterminate status before or at the Commit Point halts the group and restores the in-flight entry where authorized before any irreversible purge. Recovery vault entries are visible in analysis as recovery storage without automatic expiry; restore and purge actions require dedicated new Action Plans. Restore never overwrites modified or occupied paths and may select an alternative eligible destination. Immediate-reclaim mode purges only after post-verification and is irreversible. PigTree never claims immediate allocation release alongside durable recovery.

### 7. Partial Failure, Cancellation, and Execution Records

- **Partial Failure Handling**: Step failures allow independent operations to proceed but immediately stop dependent subtree, duplicate, and recovery groups.
- **Cancellation**: Cancellation halts execution before the next Commit Point. In-flight operations reaching a Commit Point must settle to a known or indeterminate outcome, stopping dependent steps and applying only pre-authorized step recovery. Global multi-operation atomic rollback is not supported.
- **Execution Records**: Every Action Execution emits an immutable Execution Record detailing source plan identity, source Analysis Snapshot and its Observation Interval, consent proofs, security context, live preconditions, attempt timestamps, Commit Points, per-step outcomes, errors, Recovery Artifacts, cancellation events, and post-verification observations. Records persist locally until explicitly deleted and are mandatory while Recovery Artifacts depend on them. Export is available only on explicit user request.
- **Post-Action Accounting**: Source Analysis Snapshots and Action Plans remain immutable. Reclaimable Allocation is never decremented mechanically. Targeted re-observations and whole-Volume capacity reconciliations surface as Snapshot Enrichments or new Analysis Runs. Namespace removal is distinguished from verified allocation release, which open handles or external references may delay.

### 8. Native System Handoffs

- System storage (including WinSxS, driver stores, hibernation/page/swap files, Volume Shadow Copies, Windows Update caches, MSI/AppX caches, installed applications, and future catalog entries) is protected.
- PigTree provides structured explanations, relevant diagnostic facts, and opens supported native tools (Settings, Disk Cleanup, Storage Sense) or documentation.
- PigTree does not execute hazardous maintenance commands (such as DISM /ResetBase) or batch scripts. Native tools own their own consent and execution; PigTree observes outcomes through subsequent rescans.

### 9. Automation and Non-Interactive Execution

- Non-interactive execution requires an exact, pre-generated serialized Action Plan and explicit execution policy parameters.
- One-step "query and force delete" automation workflows are prohibited.
- No interactive UAC prompts are raised during non-interactive runs; stale preconditions, protected resources, or missing consents result in non-zero exit codes and structured error records.
- Protected CLI output emits structured machine-readable facts, recommended tools, settings URIs, documentation links, and privilege requirements without attempting execution.

## Consequences

### Positive
- Prevents catastrophic data loss from stale snapshot assumptions, path ambiguities, and unverified duplicate assumptions.
- Confines destructive privileges to short-lived helpers, preventing broad administrative compromise.
- Provides verifiable auditability through immutable Execution Records and structured recovery tracking.
- Respects Windows OS integrity and cloud synchronization boundaries by routing complex system state to native tools.

### Negative / Costs
- Action Execution requires explicit multi-step workflows (plan creation, live preflight, group consent, execution) rather than immediate one-click deletion.
- Deduplication is restricted to verified same-volume NTFS scenarios, rejecting cross-volume and non-NTFS filesystems in v1.
- Immediate space reclamation is not achieved when using Recycle Bin or recoverable Hard Link vaults until explicit purges occur.

## Considered Options

- **Advisory-Only / Warning-Free Direct Deletion**: Rejected. Omitting safeguards leads to accidental deletion of critical user and system data.
- **Broad Cleaner / Junk Heuristics**: Rejected. Pattern-based temporary directory sweeping damages application caches, incomplete installers, and active user workspaces.
- **Automatic Permanent Fallback**: Rejected. Silently deleting permanently when Recycle Bin allocation or policy fails violates user intent.
- **Opaque Shell-Only Deletion**: Rejected. Relying exclusively on high-level Shell operations without per-item verification prevents accurate accounting and granular error reporting.
- **Force-Deletion Primitives**: Rejected. Clearing read-only attributes automatically, POSIX force unlinking, or scheduling reboot-time deletions bypasses OS protections and creates indeterminate filesystem states.
- **Path-Only or Machine-Learning Safety Scoring**: Rejected. Safety cannot be inferred from path strings or heuristic confidence scores; it requires structural, permission, and storage evidence.
- **Expert Override Modes**: Rejected. Bypassing live preflight or safety checks creates unverifiable mutation paths.
- **Wildcard and Unmanifested Directory Deletion**: Rejected. Deleting unobserved directory descendants risks removing newly created files.
- **Stale Plan Execution**: Rejected. Executing historical plans without live preflight causes race conditions against concurrent filesystem modifications.
- **Global Rollback / Filesystem Transaction Claims**: Rejected. Windows filesystems do not support general multi-file transactionality; claiming atomic plan rollback is inaccurate and unsafe.
- **Direct Cloud Mutation**: Rejected. Modifying or deleting cloud-managed files directly risks unexpected remote synchronization deletions or unwanted hydrations.
- **Reparse Point Target Deletion**: Rejected. Traversal into reparse targets causes accidental deletion of external directory trees.
- **Weak or Sampled Duplicate Deduplication**: Rejected. Deduplicating based on partial content hashes or unnamed-stream-only checks causes permanent data corruption.
- **Automatic Keeper Selection**: Rejected. Hard Link consolidation must allow the user to select the primary surviving path.
- **Delete-Then-Link / TxF Replacement**: Rejected. Deleting victim files before establishing keeper links creates unrecoverable windows of data loss, and Transactional NTFS (TxF) is deprecated.
- **Recycle Bin for Hard Link Recovery**: Rejected. Moving Hard Link victim files to Recycle Bin breaks link relationships and provides unreliable recovery tracking.
- **Automatic Recovery Vault Expiry**: Rejected. Silently purging recovery vaults on background timers risks irreversible data loss without user consent.
- **Persistent Elevated Service**: Rejected. Long-running elevated background services expand the security attack surface compared to on-demand, short-lived mutation workers.
- **GUI-Only Cleanup**: Rejected. Eliminating CLI execution limits administrative workflows, but CLI execution must adhere to identical Action Plan safety.
- **Query-and-Force Automation**: Rejected. Allowing single-step command line query-and-delete parameters bypasses the safety guarantees of exact Action Plans.
