# Production Technology Architecture

- **Status**: Accepted
- **Date**: 2026-08-28
- **Decider**: Project owner

PigTree requires a concrete, robust production technology architecture to implement its settled domain model, scanning and privilege boundaries, guarded cleanup safety, shared automation contract, and measurable performance targets. We establish a polyglot architecture combining a high-performance, memory-safe Rust core and worker subsystem with a modern C# / latest supported .NET LTS Windows Presentation Foundation (WPF) graphical interface. The system runs as a multi-process topology with a private 1:1 session host per client, byte-mode framed Named Pipes with Protocol Buffers and packed binary streams, zero-secret elevated broker orchestration, a custom memory-mapped columnar snapshot format (`.pts` / `.ptse`), a virtualized tree-table backed by a nonblocking sliding-window cache, a Direct3D 11 hardware-accelerated treemap hosted via `D3DImage`, bounded truthful UI Automation accessibility, and dual-channel distribution via WiX v5 Per-User MSI and portable ZIP.

## Context

Previous foundational architecture decisions defined the domain requirements and operational contracts for PigTree:
- [ADR 0001: Scanning Subsystem and Privilege Architecture](https://github.com/AFlyingP/PigTree/blob/decision/scanning-privilege-architecture/docs/adr/0001-scanning-and-privilege-architecture.md) established the deep observation seam, standard-user Win32 traversal baseline, process-isolated elevated helpers, sandboxed raw MFT parsers, and fail-closed invariant validation.
- [ADR 0002: Guarded Cleanup and Action Safety Architecture](https://github.com/AFlyingP/PigTree/blob/decision/guarded-cleanup-safety/docs/adr/0002-guarded-cleanup-safety.md) established the immutable Action Plan model, live preflight verification before Commit Points, short-lived mutation workers, and auditable Execution Records.
- [ADR 0003: Shared Engine and Automation Contract](https://github.com/AFlyingP/PigTree/blob/decision/shared-engine-automation-contract/docs/adr/0003-shared-engine-and-automation-contract.md) established the transport-neutral public engine seam, dual logical channels (lossless domain observations and coalescible progress events), immutable artifact hierarchy, typed query algebra, and machine-first CLI contracts.
- [Measurable Performance Targets](https://github.com/AFlyingP/PigTree/blob/decision/measurable-performance-targets/docs/performance-targets.md) defined non-negotiable performance floors at a baseline scale of 5,000,000 Directory Entries: <= 750 MiB peak memory working set, <= 250 ms cold snapshot reopen time, <= 100 ms interactive query/sort latency, sustained 60 FPS viewport rendering, and responsive UI Automation (UIA) tree navigation.

To select the concrete production technology stack that satisfies these contracts without introducing performance cliffs or security liabilities, four rigorous technical investigations were conducted:
1. [Rust Snapshot Persistence and Query Architecture](https://github.com/AFlyingP/PigTree/blob/4926000f826894234db456bdafe6b34cc000603f/docs/research/rust-snapshot-persistence-and-query-architecture.md) evaluated embedded database engines, columnar formats, and binary chunk stores for storing and querying 5M+ filesystem graphs.
2. [Windows Local IPC Transport, Framing, and Identity Design](https://github.com/AFlyingP/PigTree/blob/054401f83ff22e9ea4aca200a193ae713d88df0e/docs/research/windows-ipc-transport-framing-identity.md) evaluated Windows IPC mechanisms, stream framing protocols, privilege separation boundaries, and mutual authentication handshake topologies.
3. [Windows Distribution, Packaging, and Update Strategy](https://github.com/AFlyingP/PigTree/blob/292e8a20c518b21f889b46f0fbe42bf761f46b90/docs/research/windows-distribution-packaging-and-update-strategy.md) evaluated .NET deployment models, installer frameworks, shell integration verbs, code signing, and in-use update mechanics.
4. [WPF Production Rendering, Virtualization, and Accessibility Architecture](https://github.com/AFlyingP/PigTree/blob/f37c581e252f2de501265116d07230224235b5ca/docs/research/wpf-production-rendering-and-accessibility.md) evaluated UI frameworks, tree-table virtualization techniques, Direct3D/Direct2D treemap integration, UIA provider trees, high contrast themes, and DPI scaling.

This decision synthesizes these findings into the normative production technology architecture for PigTree.

## Decision

### 1. Multi-Process Executable Topology and Runtime Ownership

PigTree is partitioned into seven distinct executables with strictly defined roles, runtime ownership, and security contexts. Generic or collapsed worker roles are prohibited.

```
+---------------------------------------------------------------------------------------------------+
|                                        APPLICATION ENTRYPOINTS                                    |
|  +--------------------------------------------+    +-------------------------------------------+  |
|  |           PigTree.exe (WPF / .NET LTS)     |    |              pigtree.exe (Rust)           |  |
|  |     Interactive Desktop GUI Client         |    |        Command-Line Interface / Scripting |  |
|  +--------------------------------------------+    +-------------------------------------------+  |
+---------------------------------------------------------------------------------------------------+
                                                  |
                  [Boundary 1: Byte-Mode Framed Named Pipe + Protobuf / Binary Stream]
                                                  v
+---------------------------------------------------------------------------------------------------+
|  pigtree-engine.exe (Rust, Medium Integrity, Private 1:1 Session Host per Client)                |
|  - In-Memory Graph Index & CSR Topology           - Custom Memory-Mapped .pts/.ptse Chunk Store   |
|  - Typed Query Engine & Three-Valued Logic        - Action Plan Validator & Orchestrator          |
|  - Dual-Channel Event & Progress Dispatcher       - Privilege Elevation Coordinator               |
+---------------------------------------------------------------------------------------------------+
          |                                      |                                      |
   [Boundary 2]                           [Boundary 3]                           [Boundary 4]
   (Anon Pipe + Event)             (Non-Secret Pipe + Channel Key)        (Single-Use Plan Nonce)
          |                                      |                                      |
          v                                      v                                      v
+------------------------+      +----------------------------------+   +----------------------------+
| pigtree-scan-worker.exe|      | pigtree-elevated-broker.exe      |   | pigtree-mutation-worker.exe|
| (Rust, Medium Integ.)  |      | (Rust, High Integrity, Read-Only)|   | (Rust, Med/High Integ.)    |
| Standard Win32 Scanner |      | Volume Handles & Backup Privilege|   | Live Preflight & Commits   |
+------------------------+      +----------------------------------+   +----------------------------+
                                                 |
                                     (Restricted Token / Pipe)
                                                 v
                                +----------------------------------+
                                | pigtree-raw-parser.exe           |
                                | (Rust, Untrusted / Restricted)   |
                                | Isolated Raw NTFS MFT Parser     |
                                +----------------------------------+
```

#### Executable Roles and Language Boundaries:
1. **`PigTree.exe`**: Desktop graphical user interface built on C# and the latest supported .NET LTS using Windows Presentation Foundation (WPF). Responsible exclusively for visual rendering, layout, user interaction, accessibility providers, and clipboard/shell handoffs. Contains zero filesystem scanning, raw parsing, or mutation logic.
2. **`pigtree.exe`**: First-class command-line interface and automation client built in Rust. Responsible for parsing CLI flags, dispatching commands to the session host, and formatting streaming outputs (JSON, CSV, human-readable terminal text) per ADR 0003.
3. **`pigtree-engine.exe`**: Headless core engine and private session host built in Rust. Runs at standard medium integrity. Implements the transport-neutral domain contract, in-memory graph index, columnar snapshot store, typed query algebra, preflight validation, and worker lifecycle orchestration. Dedicated 1:1 per client process (GUI or CLI); persistent multi-tenant background services are prohibited.
4. **`pigtree-scan-worker.exe`**: Disposable standard-user scan worker built in Rust. Runs at standard medium integrity. Performs non-elevated Win32 directory traversal and emits structured observation chunks. Exits immediately upon scan completion or cancellation.
5. **`pigtree-elevated-broker.exe`**: Short-lived high-integrity read-only broker built in Rust. Launched on-demand via UAC elevation (`ShellExecuteExW` with `runas`) solely when raw volume scanning or backup privilege is authorized. Acquires `SeBackupPrivilege` and raw volume handles (`\\.\<Volume>`). Contains zero uncontracted raw parsing logic.
6. **`pigtree-raw-parser.exe`**: Sandboxed untrusted parser process built in Rust. Spawned by the elevated broker under a heavily restricted token (stripping administrative SIDs and privileges) or AppContainer. Consumes raw MFT streams across an inherited read-only handle and emits parsed metadata records over an anonymous pipe to the broker.
7. **`pigtree-mutation-worker.exe`**: Short-lived mutation execution helper built in Rust. Spawned on demand per authorized execution group (at medium or elevated integrity as required by the Action Plan). Validates exact plan nonces and live preconditions, performs guarded commit points, generates execution records, and terminates immediately.

### 2. Inter-Process Communication (IPC) Transport, Framing, and Identity

All inter-process communication adheres to strictly defined transport, framing, serialization, and authentication protocols:

#### Boundary 1: Client (`PigTree.exe` / `pigtree.exe`) to Private Session Host (`pigtree-engine.exe`)
- **Transport**: Duplex byte-mode Named Pipe (`\\.\pipe\pigtree-engine-{session_id}`). Message-mode pipes are prohibited due to cross-runtime buffer constraints and message truncation hazards. Shared writable ring buffers across processes are prohibited to prevent synchronization vulnerabilities.
- **Framing**: Strict 4-byte little-endian length prefix prepending every payload frame (`[Length: u32 LE][Payload: u8...]`), enforcing a maximum frame size of 16 MiB to prevent memory exhaustion attacks.
- **Serialization**:
  - *Control, Queries, and Events*: Protocol Buffers (protobuf v3) for strongly typed, cross-language RPC commands, query requests, challenge dialogues, and coalescible progress notifications.
  - *Bulk Observation Streams*: Versioned, tightly packed Little-Endian binary structs for high-throughput streaming (e.g. streaming search results or live observation chunks), avoiding protobuf serialization overhead.
- **Identity Binding and Handshake**:
  - The client launches `pigtree-engine.exe` passing a cryptographically random session identifier and a dedicated pipe name via non-secret command-line parameters.
  - Upon pipe connection, mutual identity validation is enforced: the client calls `GetNamedPipeServerProcessId` and validates that the engine PID matches the spawned child; the engine calls `GetNamedPipeClientProcessId` and verifies the client PID matches its designated parent.
  - An OS-cold cryptographic challenge-response handshake over the pipe establishes an ephemeral session channel key before any operational commands are accepted. Zero secrets or authentication keys are passed via command-line arguments.

#### Boundary 2: Session Host to Disposable Scan Worker (`pigtree-scan-worker.exe`)
- **Transport**: Standard inherited anonymous data pipes for binary observation chunk transfer from worker to engine.
- **Cancellation**: A dedicated inherited Win32 manual-reset event handle (`CreateEventW`). The worker polls this event concurrently with I/O via `WaitForSingleObject` / overlapped I/O, guaranteeing immediate cooperative cancellation within <= 50 ms without tearing transport streams.
- **Diagnostics**: Inherited standard error (stderr) pipe dedicated to structured NDJSON diagnostic and error logging.

#### Boundary 3: Session Host to Elevated Broker and Raw Parser
- **Host to Elevated Broker (`pigtree-elevated-broker.exe`)**:
  - The host launches the broker via `ShellExecuteExW` using the `runas` verb. Command-line parameters contain only a non-secret routing token (session GUID and pipe endpoint name).
  - The broker establishes a connection back to the host's dedicated elevated Named Pipe (`\\.\pipe\pigtree-broker-{session_id}`).
  - The host verifies that the connecting client process holds high integrity and elevated token privileges via `GetNamedPipeClientProcessId` and `OpenProcessToken`.
- **Elevated Broker to Raw Parser (`pigtree-raw-parser.exe`)**:
  - The broker opens the raw volume handle (`\\.\<Volume>`) with read-only access.
  - The broker creates a restricted security token via `CreateRestrictedToken` (disabling administrative SIDs, stripping `SeBackupPrivilege` and all elevated privileges) or creates an AppContainer profile.
  - The broker spawns `pigtree-raw-parser.exe` under the restricted token, passing the read-only volume handle as an inherited handle alongside an inherited anonymous output pipe.
  - The parser streams uncompressed, parsed record structs over the anonymous pipe back to the broker, which performs invariant checks before forwarding observations to the engine.

#### Boundary 4: Session Host to Mutation Worker (`pigtree-mutation-worker.exe`)
- **Transport**: Dedicated byte-mode Named Pipe with 4-byte LE length framing.
- **Authentication and Safety**: Spawned per authorized action group. Authenticates against the host using a single-use cryptographically random plan nonce and a SHA-256 digest of the exact, immutable Action Plan. The worker executes only authorized steps against verified live preconditions and terminates immediately upon group completion.

### 3. Analysis Snapshot Persistence and Storage Engine Format (`.pts` / `.ptse`)

PigTree implements a custom, little-endian, 64-byte aligned columnar chunk store format for Analysis Snapshots (`.pts`) and Snapshot Enrichments (`.ptse`). Embedded SQL databases (SQLite, DuckDB), general-purpose key-value stores (LMDB, RocksDB), and unspecialized serialization formats (JSON, Arrow IPC) are rejected for primary snapshot persistence.

```
+---------------------------------------------------------------------------------------------------+
|                                  .PTS BINARY FILE STRUCTURE                                       |
+---------------------------------------------------------------------------------------------------+
| 0x0000 | Superblock (128 Bytes, #[repr(C)], 64-byte aligned)                                      |
|        | - Magic: b"PTSS" (0x53535450 LE)                                                         |
|        | - Format Version: Major (u16), Minor (u16), Flags (u32)                                  |
|        | - Snapshot UUID (128-bit) | Target Type (u8) | Outcome (u8) | Scope Coverage (u8)         |
|        | - Observation Interval (Start u64, End u64) | Capacity / Free / Reconciliation Differences|
|        | - Chunk Registry Offset (u64) | Chunk Registry Count (u32) | Header CRC-32 (u32)         |
+---------------------------------------------------------------------------------------------------+
| 0x0080 | Chunk Registry / Table of Contents (Array of 64-Byte Descriptors, 64-byte aligned)       |
|        | - Chunk Type: b"FSOB", b"DENT", b"STRT", b"TOPO", b"SZIX", b"CGAP", b"TIME", b"SECD"    |
|        | - Flags (u32) | Data Offset (u64) | Uncompressed Len (u64) | Compressed Len (u64)        |
|        | - Record Count (u64) | Checksum CRC-32 (u32) | BLAKE3 Prefix (16 bytes)                  |
+---------------------------------------------------------------------------------------------------+
| 0x0... | Columnar Structure-of-Arrays (SoA) Data Chunks (All 64-byte aligned)                     |
|        | 1. b"FSOB" (Filesystem Objects: logical/allocated sizes, 128-bit identities, ref counts) |
|        | 2. b"DENT" (Directory Entries: parent IDs, object IDs, name offsets, classifications)   |
|        | 3. b"STRT" (Deduplicated UTF-8 String Dictionary & Offset Index)                          |
|        | 4. b"TOPO" (Compressed Sparse Row / CSR Hierarchy Index for O(1) Child Traversal)        |
|        | 5. b"SZIX" (Compact Secondary Size Index for Accelerated Range & Top-N Queries)          |
|        | 6. b"CGAP" (Structured Coverage Gaps Table: paths, OS errors, lower bounds)              |
|        | 7. Profile-Gated Optional Chunks: b"TIME" (Timestamps), b"SECD" (SIDs/DACLs), b"CSTR"    |
+---------------------------------------------------------------------------------------------------+
```

#### Key Storage Specifications:
1. **Superblock and Chunk Descriptors**: Fixed-size, naturally aligned `#[repr(C)]` structs. `#[repr(C, packed)]` is strictly prohibited to prevent unaligned reference undefined behavior.
2. **Columnar Structure-of-Arrays (SoA)**: Data columns (e.g. `logical_sizes`, `allocated_sizes`, `object_identities`, `hard_link_ref_counts`) are stored as independent, contiguous primitive arrays within chunk payloads, enabling vectorization and demand-paged OS cache utilization.
3. **Safe Zero-Copy Memory Transmutation**: Chunk payloads are memory-mapped (`CreateFileMappingW` / `MapViewOfFile`) and safely transmuted to typed Rust slices (`&[T]`) via `zerocopy` / `bytemuck`. 64-byte file offset alignment guarantees that natural alignment invariants for `u8`, `u16`, `u32`, `u64`, and `u128` are unconditionally satisfied.
4. **Graph Topology Representation**: Directory tree hierarchy is represented via a **Compressed Sparse Row (CSR)** structure in the `TOPO` chunk, consisting of a `child_row_offsets` array (`&[u32]`) and a `child_entry_ids` array (`&[u32]`), enabling $O(1)$ child lookups and linear memory traversals for subtree aggregations.
5. **String Dictionary**: Monolithic deduplicated UTF-8 byte chunk (`STRT`) with a parallel 32-bit slice offset array, avoiding per-string heap allocations.
6. **Data Integrity Verification**: Every chunk descriptor contains an explicit ISO-HDLC CRC-32 checksum and a 16-byte BLAKE3 hash prefix for fast corruption detection before zero-copy slice mapping.

### 4. WPF Production Rendering, Virtualization, and Accessibility

The graphical user interface is implemented in C# on .NET LTS using WPF, architected specifically to overcome WPF's historical performance bottlenecks at 5M-entry scale.

```
+---------------------------------------------------------------------------------------------------+
|                                      WPF GUI ARCHITECTURE                                         |
+---------------------------------------------------------------------------------------------------+
|  +---------------------------------------------------------------------------------------------+  |
|  | TreeListView: Customized ListView + VirtualizingStackPanel (Recycling, Pixel Scrolling)    |  |
|  | - Backed by VirtualTreeCollection (IList) implementing Flattened Virtual Projection         |  |
|  | - Nonblocking Sliding-Window Page Cache (500-1000 items, < 1 MB managed heap footprint)     |  |
|  | - Bounded Truthful UI Automation: Custom TreeListViewAutomationPeer & ItemPeers             |  |
|  +---------------------------------------------------------------------------------------------+  |
|                                                  |                                                |
|                                       (Cross-Component Selection)                                 |
|                                                  v                                                |
|  +---------------------------------------------------------------------------------------------+  |
|  | TreemapView: System.Windows.Interop.D3DImage Shared Surface Host                            |  |
|  | - In-Process C# Squarified Layout Partitioner (Calculates (x,y,w,h) against Viewport Bounds)|  |
|  | - Direct3D 11 Render Pipeline (Cushion Shading & Gradient Borders via HLSL Pixel Shaders)   |  |
|  | - DXGI Legacy Shared Handle (D3D11_RESOURCE_MISC_SHARED) bound to Direct3D 9Ex Surface      |  |
|  | - Conservative Render Synchronization: ID3D11DeviceContext::Flush() -> UI Dispatcher       |  |
|  | - High Contrast Integration (SystemColors.HighlightBrushKey) & Per-Monitor DPI V2 Scaling  |  |
|  | - Device Loss & Surface Invalidation Recovery (WARP & Direct2D Fallbacks)                   |  |
|  +---------------------------------------------------------------------------------------------+  |
+---------------------------------------------------------------------------------------------------+
```

#### Key Presentation Specifications:
1. **Tree-Table Virtualization**:
   - Built-in recursive WPF `TreeView` is prohibited at production scale due to unbounded visual tree allocation and recursive traversal overhead.
   - The primary tree-table is implemented via a customized `ListView` paired with a recycling `VirtualizingStackPanel` (`VirtualizationMode="Recycling"`, `ScrollUnit="Pixel"`).
   - The data source is a custom `VirtualTreeCollection` implementing `IList`, `INotifyCollectionChanged`, and `IItemContainerMapping`.
   - **No Full Client Mirror**: WPF never constructs a 5,000,000-entry managed object array or unmanaged descriptor arena. The collection virtualizes `Count` based on engine metadata, holding only a bounded sliding-window cache of active rows (500–1000 items, consuming < 1 MiB managed memory).
   - Synchronous `IList` index lookups return lightweight placeholder items during rapid scrolling while fetching row data asynchronously from the engine over IPC.
2. **Hardware-Accelerated Treemap Visualization**:
   - The interactive treemap is rendered using Direct3D 11 hosted seamlessly inside WPF via `System.Windows.Interop.D3DImage`. Direct3D 9Ex interop is achieved via the documented DXGI legacy shared handle (`D3D11_RESOURCE_MISC_SHARED` via `IDXGIResource::GetSharedHandle`).
   - **Seam Placement**: Semantic node weights and hierarchy are supplied by the Rust engine; the WPF presentation layer computes the geometric $(x, y, w, h)$ squarified treemap partitions locally in C# against active viewport dimensions. Engine code remains completely decoupled from display pixels and DPI scaling.
   - Cushion shading and hierarchy boundary highlights are computed in hardware via HLSL pixel shaders, eliminating WPF Airspace clipping limitations and window airspace boundaries.
   - **Conservative Synchronization**: Direct3D 11 rendering completes, issues an explicit `ID3D11DeviceContext::Flush()`, and updates `D3DImage.AddDirtyRect` on the UI thread. Complete recovery handling manages `IsFrontBufferAvailableChanged` events, device loss, WARP software rasterization, and GDI+/Direct2D software fallbacks.
3. **UI Automation (UIA) and Accessibility**:
   - Custom automation peers (`TreeListViewAutomationPeer`, `TreeListViewItemAutomationPeer`) expose a truthful, bounded UIA tree model implementing `ISelectionItemProvider`, `IExpandCollapseProvider`, `ITableItemProvider`, and `ItemStatus`.
   - Screen reader search queries (`FindItemByProperty`) resolve against a lightweight, lock-free projection index without triggering massive visual container realization on the UI Dispatcher.
   - Native Windows Contrast Themes (Contrast High, Contrast Light, Contrast Dark) are tracked dynamically via `SystemParameters.HighContrast` and system theme brush keys, applying accessible color ramps and high-contrast borders across both tree-table controls and treemap nodes.
   - Per-Monitor DPI V2 awareness is enforced across all windowing, font rendering, and Direct3D shared surface viewports.

### 5. Packaging, Distribution, Shell Integration, and Update Strategy

PigTree provides a transparent, dual-channel Windows distribution model that respects user autonomy, requires no administrative rights for baseline use, and prevents background update interference.

```
+---------------------------------------------------------------------------------------------------+
|                                 DISTRIBUTION & PACKAGING MATRIX                                   |
+---------------------------------------------------------------------------------------------------+
|  1. WiX Toolset v5 MSI (Recommended Default Installer)                                            |
|     - Default Per-User Scope (MSIINSTALLPERUSER=1 -> %LocalAppData%\Programs\PigTree)            |
|     - Zero UAC elevation prompts required during installation or update                          |
|     - Optional Per-Machine Enterprise Scope (ALLUSERS=1 -> Program Files\PigTree)                 |
|     - Standard Windows Add/Remove Programs integration & transactional rollback                   |
+---------------------------------------------------------------------------------------------------+
|  2. First-Class Portable ZIP Archive                                                              |
|     - Fully self-contained, zero-installer, zero-registry archive                                |
|     - Targetable for portable flash drives, sysadmins, and incident responders                   |
|     - Manual verified replacement with update notifications in v1                                |
+---------------------------------------------------------------------------------------------------+
|  3. .NET Deployment Architecture                                                                  |
|     - Self-contained ReadyToRun (R2R) multi-file deployment targeting latest supported .NET LTS  |
|     - Embedded private .NET runtime; zero external .NET Desktop Runtime prerequisite installation|
|     - Native x64 binaries for Rust engine, CLI, and helper workers                               |
+---------------------------------------------------------------------------------------------------+
|  4. Shell Integration Architecture                                                                |
|     - Classic HKCU Registry Verbs (Directory\shell\PigTree, Drive\shell\PigTree) for standard user|
|     - Optional Windows 11 Sparse Package (.msix with allowExternalUri) for modern Win11 context  |
+---------------------------------------------------------------------------------------------------+
```

#### Key Distribution Specifications:
1. **Runtime Deployment**: WPF client is compiled as a self-contained ReadyToRun (R2R) multi-file bundle on the latest supported .NET LTS for x64. The engine, CLI, and worker helpers are compiled as optimized native Rust binaries. Native AOT for the GUI is rejected in v1 due to lack of official WPF AOT support, but native Rust compilation delivers instant startup for engine and CLI.
2. **Installer Modality**:
   - Primary installer is built with **WiX Toolset v5** emitting a standard MSI package.
   - Default install mode is **Per-User** (`MSIINSTALLPERUSER=1`), installing to `%LocalAppData%\Programs\PigTree` with zero UAC elevation prompts.
   - Enterprise system administrators can deploy per-machine via `ALLUSERS=1` into `%ProgramFiles%\PigTree`.
   - **MSIX is rejected as the primary installer** due to Virtualized File System (VFS) containerization friction, context menu registration instability, and enterprise deployment barriers.
3. **Portable ZIP Distribution**: First-class, zero-install portable ZIP containing all binaries and self-contained runtime files. In v1, portable updates operate via opt-in notification and manual archive replacement (or an atomic versioned-directory switch when available).
4. **Shell Integration**: Standard per-user shell context menus (`Directory\shell\PigTree` and `Drive\shell\PigTree`) registered via HKCU registry keys. An optional Windows 11 sparse package (`.msix` manifest with `allowExternalUri` and package identity) enables top-level modern Windows 11 context menus without containerizing application binaries.
5. **Code Signing and Updates**:
   - All production executables and MSI installers are signed with Authenticode certificates using SHA-256 digests and RFC 3161 timestamps.
   - Release manifests are verified via Ed25519 signatures and SHA-256 checksums before initiating updates.
   - **Silent or forced background updates are strictly prohibited**. Updates are user-initiated or opt-in notified.
   - Multi-file in-use locking during updates is resolved by staging new binaries in an isolated directory and executing a fast atomic directory pointer cutover or short-lived helper handoff upon restart.

### 6. Measurable Benchmark and Release Gates

All modeled throughput, memory, and latency projections remain benchmark-gated criteria. No component may ship in stable production builds without satisfying the empirical performance suite defined in `docs/performance-targets.md`:

1. **Working Set Memory Gate**: Peak working set memory <= 750 MiB during a 5,000,000-entry whole-volume analysis and interactive visualization session.
2. **Snapshot Cold Reopen Gate**: Opening a saved 5,000,000-entry `.pts` file into an interactive GUI or CLI query state in <= 250 ms via demand-paged memory mapping.
3. **Query and Sort Latency Gate**: Executing arbitrary column sorting, depth filtering, or metadata queries across 5M entries in <= 100 ms (p95).
4. **UI Render and Virtualization Gate**: Sustained 60 FPS viewport scrolling and treemap zoom/pan interactions, with visual container realization latency <= 16 ms per frame.
5. **Scan Throughput Gates**:
   - Standard-user multi-threaded Win32 directory traversal >= 170,000 entries/s on warm NVMe storage.
   - Elevated raw NTFS MFT parsing >= 300,000 entries/s (release-gated under ADR 0001).
6. **IPC Transport Gate**: Bulk observation streaming over byte-mode Named Pipes >= 250,000 records/s with roundtrip RPC request-response latency <= 1 ms.
7. **Accessibility Latency Gate**: Screen reader (Narrator/NVDA) element focus and UIA navigation response time <= 50 ms.

## Consequences

### Positive
- **Optimal Technology Separation**: Combines Rust's memory safety, low-level Win32 systems access, zero-cost abstractions, and deterministic memory management with C#'s mature desktop GUI ecosystem, rapid UI development, and rich accessibility infrastructure.
- **Uncompromised Desktop Performance**: Memory-mapped columnar storage, hardware-accelerated Direct3D treemap rendering, and virtualized tree-table paging deliver sub-second response times and 60 FPS interactions on 5M-entry datasets within a <= 750 MiB working set.
- **Rigorous Security and Least Privilege**: Multi-process architecture guarantees that untrusted raw parsers run in sandboxes, elevated privileges are isolated to short-lived read-only brokers, and GUI/CLI clients run strictly at standard user integrity with zero ambient elevation.
- **Zero-Friction Distribution**: Per-user MSI and portable ZIP distributions allow standard users to install, run, and update PigTree without administrative rights, corporate policy blocks, or invasive background daemons.
- **Full WCAG AA and UIA Accessibility**: Bounded virtualized UI Automation tree, contrast theme integration, and keyboard parity ensure equal usability for assistive technology users.

### Negative and Trade-offs
- **Multi-Process Complexity**: Managing process lifecycles, IPC framing, mutual PID validation, and error recovery across seven distinct executables introduces substantial orchestration logic compared to a monolithic application.
- **Custom Presentation Virtualization**: WPF's built-in controls cannot handle 5M items out-of-the-box, necessitating custom `VirtualTreeCollection` paging, custom `AutomationPeer` implementations, and manual Direct3D/WPF `D3DImage` interop.
- **Self-Contained Deployment Footprint**: Shipping a self-contained ReadyToRun .NET LTS runtime alongside native Rust binaries results in an uncompressed disk distribution footprint of approximately 120–160 MiB (compressed installer/ZIP of ~45–65 MiB).
- **Format Evolution Rigidity**: Memory-mapped binary chunk stores require strict versioning, chunk header management, and schema migration protocols to ensure long-term artifact compatibility.

## Considered Options

The following alternative technologies and architectural designs were evaluated and rejected:

### 1. Monolithic Single-Process Architecture in C# (.NET)
- *Rejected*: Running disk scanning, raw MFT parsing, graph indexing, and GUI rendering inside a single managed .NET process causes severe Garbage Collection (GC) pauses (frequent Gen2 GC pauses exceeding 500–1200 ms when managing 5M objects), lacks memory-safety guarantees when parsing corrupted on-disk filesystem structures, and violates least-privilege separation during elevated scans.

### 2. Monolithic Single-Process Architecture in Rust (e.g. Slint / iced / egui)
- *Rejected*: While Rust provides exceptional systems performance, modern pure-Rust GUI frameworks lack mature Windows UI Automation (UIA) tree providers, complete Windows High Contrast / Contrast Theme integration, native IME composition, rich multi-column tree-grid virtualization, and proven Direct3D viewport sharing.

### 3. Alternative UI Frameworks: WinUI 3 / Windows App SDK, Tauri v2, Qt 6, Avalonia UI
- *WinUI 3 / Windows App SDK*: Rejected due to high memory overhead, immature tree-table virtualization recycling, sluggish UIA performance at scale, and external Windows App Runtime deployment dependencies.
- *Tauri v2 / WebView2*: Rejected due to excessive DOM/Chromium memory consumption when indexing 5M entries, canvas-to-accessibility impedance mismatches, and sluggish tree-grid scrolling compared to native desktop primitives.
- *Qt 6 (C++ / QML)*: Rejected due to significant commercial licensing complexity (GPLv3 / LGPLv3 dynamic linking constraints on Windows), lack of deep Windows UIA automation provider fidelity compared to WPF, and higher maintenance friction across polyglot C++/Rust boundaries.
- *Avalonia UI*: Rejected because WPF provides deeper, more battle-tested integration with Windows-specific platform contracts (Direct3D 9Ex `D3DImage` interop, native Win32 accessibility hooks, and Windows High Contrast themes) while maintaining zero third-party UI framework dependencies.

### 4. Alternative Storage Engines: SQLite, DuckDB, Apache Arrow, Embedded Key-Value
- *SQLite (Memory-Mapped / WAL)*: Rejected because relational B-tree row storage incurs 3–4x higher disk and memory overhead (~600–900 MiB for 5M rows), lacks native CSR graph hierarchy support, and introduces query latency cliffs (> 450 ms) on deep recursive subtree aggregations.
- *DuckDB*: Rejected due to a heavy engine binary footprint (+30 MiB), inability to memory-map custom graph structures with zero-copy slice casting, and lack of domain-specific coverage gap and knowledge state primitives.
- *Apache Arrow IPC / Feather*: Rejected because generic Arrow schemas lack support for compressed graph adjacency lists (CSR topology), require custom serialization layers for domain knowledge states, and do not provide integrated CRC/BLAKE3 chunk integrity verification.
- *LMDB / redb / RocksDB*: Rejected because key-value lookups require per-record deserialization and pointer indirection, preventing SIMD-vectorized columnar aggregation and sub-second full-table scans.

### 5. Alternative IPC Mechanisms: Message-Mode Pipes, Shared-Memory Ring Buffers, Transport Compression
- *Message-Mode Named Pipes*: Rejected because Windows message-mode pipes suffer from 64 KiB message size limits, cross-runtime framing quirks in .NET, and truncation errors on large buffer reads.
- *Shared-Memory Ring Buffers*: Rejected because writable cross-process shared memory introduces severe synchronization complexity, race condition hazards across integrity boundaries, and elevated attack surface for negligible throughput gains over byte-mode named pipes on modern NVMe systems.
- *Transport Compression (LZ4/Zstd on IPC)*: Rejected because local in-memory IPC bandwidth on modern CPUs exceeds 2–4 GB/s, making compression/decompression CPU overhead a net throughput bottleneck for local IPC.

### 6. Alternative Packaging: MSIX Primary Installer
- *MSIX / AppX*: Rejected as primary distribution because MSIX Virtualized File System (VFS) and registry containerization create operational friction for disk analyzers, context menu integration is unstable on older Windows 10 versions, and enterprise sysadmins resist MSIX deployment over standard MSI.
