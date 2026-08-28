# PigTree

A Windows disk space analyzer for inspecting filesystem capacity, storage allocation, and directory structures.

## Language

**Volume**:
A Windows storage namespace with its own identity and capacity accounting; it need not have a drive letter.

**Scan Target**:
Exactly one selected Volume or one local directory that defines an Analysis Run's traversal and accounting boundary. A directory Scan Target reports only the allocation and coverage reachable within that directory; volume-level Capacity reconciliation applies only when the Scan Target is a whole Volume.

**Analysis Profile**:
The declared traversal rules and observation classes an Analysis Run attempts for its Scan Target. Coverage is evaluated relative to the Analysis Profile; facts intentionally outside it are Not Observed and do not create Coverage Gaps.

**Analysis Run**:
The time-bounded act of observing one Scan Target.

**Analysis Snapshot**:
An immutable representation of what an Analysis Run has observed. Any useful snapshot, including one from a cancelled or failed Analysis Run, may be saved and reopened while retaining its Run Outcome, Coverage, Coverage Gaps, Observation Interval, and incomplete aggregates; reopening never upgrades them.

**File**:
A Filesystem Object whose primary role is owning Content Streams.

**Directory**:
A Filesystem Object whose primary role is containing Directory Entries; it may also have self storage and filesystem-specific Content Streams.

**Special Object**:
A Filesystem Object whose filesystem-defined role is neither File nor Directory.

**Directory Entry**:
One name within a Directory that refers to a Filesystem Object; its name and parent relationship are entry facts, and Observed Path is derived from entry chains. Entry-derived classifications do not become universal Filesystem Object facts.

**Filesystem Object**:
A stored entity within one Volume with one underlying identity when the filesystem supplies one; multiple Directory Entries may refer to the same object. Object identity, Content Streams, sizes/allocation, Storage Characteristics, Owner, Access Rules, and object-level Timestamp Observations belong to the object unless the filesystem explicitly defines a fact on an entry instead. Its Logical Size aggregates the Logical Sizes of its owned Content Streams; its Allocated Size aggregates stream allocation and any other storage defensibly attributable to the object. Content Stream sizes are a breakdown of object sizes, not additional bytes. Storage not defensibly attributable to an object remains Unattributed Used Space at whole-Volume scope.

**Hard Link**:
A Directory Entry that refers to a Filesystem Object also referred to by another Directory Entry; it does not create an independent copy of the object's allocation.

**Reparse Point**:
A filesystem-defined storage characteristic or behavior that may apply to a File, Directory, or Special Object, rather than an object kind itself; PigTree represents it as observed but does not treat its target contents as children of the entry unless that target is selected separately as a Scan Target.

**Logical Size**:
The number of addressable content bytes of a Filesystem Object or Content Stream, independent of physical allocation.

**Allocated Size**:
Physical storage allocated to one Filesystem Object or Content Stream, excluding allocation owned by unrelated objects.

**Referenced Allocated Size**:
The sum of Allocated Size reached through Directory Entries in a scope, so the same Filesystem Object can contribute more than once through multiple links.

**Unique Allocated Size**:
The Allocated Size of the distinct Filesystem Objects reachable in a scope, counting each underlying object once within that scope; sibling scopes can overlap, so their Unique Allocated Sizes are not necessarily additive.

**External Reference Uncertainty**:
The condition where a Filesystem Object reached by a directory Scan Target may have Directory Entries outside that target. Its full allocation contributes once to Unique Allocated Size within the target, but Reclaimable Allocation cannot assume removal of the final filesystem reference without sufficient wider evidence.

**Run Outcome**:
How an Analysis Run ended: finished, cancelled, or failed. It does not state how completely the Scan Target was observed.

**Coverage**:
How completely an Analysis Snapshot represents its Scan Target: complete, partial, or indeterminate relative to an Analysis Profile. Complete means every requested observation within the Scan Target was either Known or Not Applicable; requested Unavailable observations or unresolved regions make Coverage partial or indeterminate as warranted. Scope Coverage composes from requested observations and Coverage Gaps within that scope; a scope is not Complete when any requested observation in it is Unavailable or an unresolved Coverage Gap intersects it. Child scopes retain independently reportable Coverage. Not Observed facts outside the profile do not reduce Coverage. A finished Analysis Run can have partial or indeterminate Coverage.

**Observation Interval**:
The period from the first to the last observation represented by an Analysis Snapshot. Unless the source itself provides point-in-time consistency, a snapshot describes facts observed during this interval rather than one atomic instant.

**Observation Status**:
The state of knowledge for an attempted observation, such as observed, metadata-only, inaccessible, disappeared, or errored; it qualifies values rather than replacing unknown values with zero.

**Coverage Gap**:
A scoped part of a Scan Target that PigTree could not fully observe, recorded with its reason and any defensible known bounds.

**Capacity**:
The total storage capacity reported for a whole Volume.

**Free Space**:
Capacity reported as currently available on a whole Volume.

**Used Space**:
Capacity minus Free Space for a whole Volume.

**Accounted Unique Allocation**:
The Unique Allocated Size PigTree can attribute to observed Filesystem Objects within a whole-Volume Analysis Snapshot.

**Reconciliation Difference**:
Used Space minus Accounted Unique Allocation for a whole-Volume Analysis Snapshot. A positive difference is Unattributed Used Space; a negative difference is Over-Accounted Allocation. Both discrepancies remain visible and indicate incomplete, temporally inconsistent, or semantically non-comparable observations; neither is an ordinary Directory.

**Unattributed Used Space**:
The positive part of Reconciliation Difference for a whole-Volume Analysis Snapshot: Used Space that PigTree cannot defensibly attribute to observed Filesystem Objects. It indicates incomplete, temporally inconsistent, or semantically non-comparable observations (such as reserved, inaccessible, unsupported, or changing storage) and must not be presented as an ordinary Directory.

**Over-Accounted Allocation**:
The magnitude of a negative Reconciliation Difference for a whole-Volume Analysis Snapshot, where Accounted Unique Allocation exceeds Used Space. It indicates incomplete, temporally inconsistent, or semantically non-comparable observations and must not be presented as an ordinary Directory.

**Content Stream**:
A named or unnamed content-bearing part owned by a Filesystem Object, with its own Logical Size, Allocated Size, Observation Status, and Storage Characteristics; streams are not Directory Entries.

**Storage Characteristic**:
A property that explains how content is stored or retrieved, such as sparse, filesystem-compressed, resident, or online-only; it does not redefine Logical Size or Allocated Size.

**Timestamp Observation**:
A recorded filesystem-defined timestamp kind and value with provenance and Value Knowledge. PigTree does not assume that timestamp kinds such as created, modified, accessed, or metadata-changed have identical semantics across filesystems; any age aggregate identifies the timestamp kind it uses.

**Referenced Logical Size**:
The sum of Logical Size reached through Directory Entries in a scope, so the same Filesystem Object can contribute more than once through multiple links.

**Unique Logical Size**:
The Logical Size of distinct Filesystem Objects reachable in a scope, counting each underlying object once within that scope; sibling scopes can overlap and are not necessarily additive.

**Value Knowledge**:
Whether a surfaced value is Known, Not Observed, Unavailable, or Not Applicable. Known values carry relevant provenance and distinguish observations from derivations or estimates. Not Observed means the Analysis Profile or a Snapshot Enrichment has not requested the fact. Unavailable means the fact was requested but could not be established and carries a reason. Not Applicable means the concept does not apply. Unknown may be user-facing shorthand but is not a canonical state.

**Snapshot Enrichment**:
Immutable metadata observed after the base Analysis Snapshot, with its own Observation Interval and provenance; it supplements but never rewrites what the original Analysis Run observed and may show that a live object changed or disappeared.

**Owner**:
The security principal recorded as owning a Filesystem Object, when known.

**Access Rules**:
Recorded permission metadata governing a Filesystem Object, when known.

**Accessibility**:
What PigTree was actually able to observe about a Filesystem Object or Content Stream under the security context used for an attempt, expressed through Observation Status; it is not inferred solely from Owner or Access Rules, and Owner or Access Rules are not inferred from an access failure.

**Self Logical Size**:
The Logical Size of one Filesystem Object without descendant objects.

**Self Allocated Size**:
The Allocated Size of one Filesystem Object without descendant objects.

**Scope Aggregate**:
A measure across a scope's object and the Filesystem Objects reachable through its Directory Entries; directory scope aggregates are distinct from the directory object's Self Logical Size and Self Allocated Size.

**Known Subtotal**:
The sum of Known contributing values when an aggregate is incomplete because some contributions are Unavailable or Not Observed. It is a lower bound only where all omitted contributions are non-negative; any stronger lower or upper bound must be explicitly supported by evidence.

**Duplicate Candidate Set**:
A set of distinct Filesystem Objects grouped by non-content evidence that makes equal content plausible; multiple Directory Entries for the same Filesystem Object are aliases and never separate duplicate candidates.

**Verified Duplicate Set**:
A set of distinct Filesystem Objects whose selected Content Streams were observed to be equal using a recorded Verification Method. It proves only the explicitly recorded Verification Scope and does not imply equal names, timestamps, Storage Characteristics, other Content Streams, Owner, Access Rules, or other metadata.

**Verification Method**:
The recorded method and evidence used to establish equality between selected Content Streams.

**Verification Scope**:
The exact Content Stream or set of Content Streams whose equality a verification claim covers; the default duplicate-content scope is the unnamed data stream when it exists.

**Object Identity**:
The strongest available evidence that observations refer to the same Filesystem Object, qualified by its Volume, evidence type, and observation scope. Filesystem-provided stable identifiers can establish identity within an Analysis Snapshot; path-derived identity is provisional, and identity across snapshots is never assumed without sufficient evidence.

**Observed Path**:
A snapshot-relative route derived from a chain of Directory Entries. It is an observation, not Filesystem Object identity; one object can have multiple Observed Paths, and paths can differ between snapshots.

**Historical Snapshot**:
An Analysis Snapshot considered solely as an immutable observation from its recorded Observation Interval. Reopening it does not assert that its objects, paths, or values still exist or remain current.

**Reclaimable Allocation**:
The allocation expected to become free if a specific validated Action Plan succeeds, accounting for known hard links, shared Filesystem Objects, filesystem semantics, and incomplete knowledge. It is reported as an exact value only when proven, otherwise as a defensible range or Unknown.

**Action Plan**:
An immutable proposed set of operations over specified Directory Entries and Filesystem Objects, including preconditions, expected relationship changes, expected Reclaimable Allocation, recovery expectations, and known uncertainties. Deleting an entry removes that Directory Entry; it frees object allocation only when no remaining filesystem references preserve that allocation. Hard-link replacement changes which Filesystem Object an entry refers to rather than deleting every alias of either object.

**Action Risk Class**:
The operation classification routine, caution, protected, or prohibited, derived from explicit reasons about potential data loss, system disruption, uncertainty, and recoverability. Routine means ordinary safeguards suffice; caution requires focused acknowledgement; protected replaces direct mutation with a native/provider handoff; prohibited means no safe or sufficiently authorized mutation path exists.
_Avoid_: Safe, numeric score, ML confidence

**Recovery Class**:
The declared recovery expectation of an operation, including whether it relies on a Recovery Artifact or is permanent. It does not guarantee that a platform-managed artifact will remain available indefinitely and does not imply immediate Reclaimable Allocation.
_Avoid_: Undo mode, backup level

**Recovery Artifact**:
Preserved state or a platform-managed recovery reference created or retained by an Action Execution to support an authorized restore. It may continue to occupy allocation and has an explicitly stated retention owner and limits.
_Avoid_: Backup copy, undo file

**Action Execution**:
One time-bounded attempt to apply an immutable Action Plan to live storage under a stated security context and consent policy.
_Avoid_: Mutation run, cleanup session

**Execution Record**:
An immutable record of the evidence and outcomes of an Action Execution, including precondition observations, Commit Points, per-operation outcomes, errors, and Recovery Artifacts. It does not alter the source Action Plan or Analysis Snapshot.
_Avoid_: Audit log, run history

**Commit Point**:
The operation-specific state transition after which cancellation cannot simply leave that operation unattempted; it must reach a recorded outcome or use recovery already authorized by the Action Plan. It does not imply whole-plan atomicity.
_Avoid_: Point of no return, transaction boundary

**Entry Count**:
The number of Directory Entries reachable in a stated scope and filter; multiple entries that refer to the same Filesystem Object count separately. Every count declares whether it includes the scope root and which object kinds it includes.

**Unique Object Count**:
The number of distinct Filesystem Objects reachable in a stated scope and filter, counting each Object Identity once within that scope. Every count declares whether it includes the scope root and which object kinds it includes.

**Entry Classification**:
A classification of a Directory Entry using an explicit rule, such as filename extension. It is not a universal property of the referenced Filesystem Object. Any aggregate using it states the classification rule and whether it counts Directory Entries or distinct Filesystem Objects.

**Content Classification**:
A classification based on observed Content Stream evidence, with provenance and Value Knowledge. It is distinct from Entry Classification.
