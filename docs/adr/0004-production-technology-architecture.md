# Production Technology Architecture

- **Status**: Accepted
- **Date**: 2026-08-28
- **Decider**: Project owner

PigTree requires a concrete, robust production technology architecture to implement its settled domain model, scanning and privilege boundaries, guarded cleanup safety, shared automation contract, and measurable performance targets. We establish a polyglot architecture combining a high-performance, memory-safe Rust core and worker subsystem with a modern C# / latest supported .NET LTS Windows Presentation Foundation (WPF) graphical interface. The system runs as a multi-process topology with a private 1:1 session host per client, bounded byte-mode framed Named Pipes with Protocol Buffers and packed binary streams, zero-secret elevated broker orchestration, an immutable memory-mapped columnar snapshot format (`.pts` / `.ptse`), a virtualized tree-table backed by a bounded sliding-window page cache, a Direct3D 11 hardware-accelerated treemap hosted via `D3DImage` with in-process presentation geometry, truthful bounded UI Automation accessibility, and dual-channel distribution via WiX v5 Per-User MSI and portable ZIP.

## Context

Foundational architecture decisions established the domain contracts, safety boundaries, and performance expectations for PigTree:
- [ADR 0001: Scanning Subsystem and Privilege Architecture](0001-scanning-and-privilege-architecture.md) established the deep observation seam, standard-user Win32 traversal baseline, process-isolated elevated helpers, sandboxed raw MFT parsers, and fail-closed invariant validation.
- [ADR 0002: Guarded Cleanup and Action Safety Architecture](0002-guarded-cleanup-safety.md) established the immutable Action Plan model, live preflight verification before Commit Points, short-lived mutation workers, and auditable Execution Records.
- [ADR 0003: Shared Engine and Automation Contract](0003-shared-engine-and-automation-contract.md) established the transport-neutral public engine seam, dual logical channels (lossless domain observations and coalescible progress events), immutable artifact hierarchy, typed query algebra, and machine-first CLI contracts.
- [Measurable Performance Targets](../performance-targets.md) defined non-negotiable performance floors at a baseline scale of 5,000,000 observed Directory Entries: <= 1.5 GiB peak process-family Private Bytes, cold snapshot reopen p95 <= 3.0 s (NVMe) / <= 6.0 s (SATA), interactive query latency <= 100 ms (p95), sustained 60 FPS viewport rendering, and responsive UI Automation (UIA) tree navigation.

To evaluate concrete technology candidates against these requirements, four technical investigations were conducted:
1. [Rust Snapshot Persistence and Query Architecture](https://github.com/AFlyingP/PigTree/blob/4926000f826894234db456bdafe6b34cc000603f/docs/research/rust-snapshot-persistence-and-query-architecture.md) evaluated storage engines, columnar memory mapping, and graph indexing.
2. [Windows Local IPC Transport, Framing, and Identity Design](https://github.com/AFlyingP/PigTree/blob/054401f83ff22e9ea4aca200a193ae713d88df0e/docs/research/windows-ipc-transport-framing-identity.md) evaluated Windows IPC mechanisms, stream framing protocols, privilege separation boundaries, and identity binding.
3. [Windows Distribution, Packaging, and Update Strategy](https://github.com/AFlyingP/PigTree/blob/292e8a20c518b21f889b46f0fbe42bf761f46b90/docs/research/windows-distribution-packaging-and-update-strategy.md) evaluated .NET deployment models, installer frameworks, shell integration verbs, and update mechanics.
4. [WPF Production Rendering, Virtualization, and Accessibility Architecture](https://github.com/AFlyingP/PigTree/blob/f37c581e252f2de501265116d07230224235b5ca/docs/research/wpf-production-rendering-and-accessibility.md) evaluated UI virtualization techniques, Direct3D treemap integration, UIA provider trees, high contrast themes, and DPI scaling.

These research reports serve as supporting technical evidence. This ADR records the normative architectural decisions, boundaries, and trade-offs.

## Decision

### 1. Multi-Process Executable Topology and Runtime Ownership

PigTree is partitioned into seven distinct executables with strictly defined roles, runtime ownership, and security contexts. Generic or collapsed worker roles are prohibited:

1. **`PigTree.exe`**: Desktop graphical user interface built on C# and the latest supported .NET LTS using Windows Presentation Foundation (WPF). Responsible for visual layout, user interaction, accessibility providers, and clipboard/shell handoffs. Contains zero filesystem scanning, raw parsing, or mutation logic.
2. **`pigtree.exe`**: Command-line interface and automation client built in Rust. Responsible for parsing CLI flags, dispatching commands to the session host, and formatting streaming outputs (JSON, CSV, terminal text) per ADR 0003.
3. **`pigtree-engine.exe`**: Headless core engine and private session host built in Rust, running at standard medium integrity. Implements the transport-neutral domain contract, in-memory graph indexing, columnar snapshot store, typed query algebra, preflight validation, and worker lifecycle orchestration. Dedicated 1:1 per client process (GUI or CLI); persistent multi-tenant background services are prohibited.
4. **`pigtree-scan-worker.exe`**: Disposable standard-user scan worker built in Rust, running at standard medium integrity. Performs non-elevated Win32 directory traversal and emits structured observation records. Exits immediately upon scan completion or cancellation.
5. **`pigtree-elevated-broker.exe`**: Short-lived high-integrity read-only broker built in Rust. Launched on-demand via UAC elevation (`ShellExecuteExW` with `runas`) solely when raw volume scanning or backup privilege is authorized. Acquires `SeBackupPrivilege` and raw volume handles (`\\.\<Volume>`). Contains zero uncontracted raw parsing logic.
6. **`pigtree-raw-parser.exe`**: Sandboxed untrusted parser process built in Rust. Spawned by the elevated broker under a restricted token (stripping administrative SIDs and privileges) or AppContainer. Consumes raw MFT streams across an inherited read-only handle and emits parsed metadata records over an anonymous pipe to the broker.
7. **`pigtree-mutation-worker.exe`**: Short-lived mutation execution helper built in Rust. Spawned on demand per authorized execution group (at medium or elevated integrity as required by the Action Plan). Validates exact plan nonces and live preconditions, executes guarded commit points, generates execution records, and terminates immediately.

### 2. Inter-Process Communication (IPC) Transport, Framing, and Identity

All inter-process communication adheres to strictly defined transport, framing, serialization, and authentication boundaries:

#### Boundary 1: Client (`PigTree.exe` / `pigtree.exe`) to Private Session Host (`pigtree-engine.exe`)
- **Transport**: Duplex byte-mode bounded framed local Named Pipes (`\\.\pipe\pigtree-engine-{session_id}`). Message-mode pipes are prohibited due to cross-runtime buffer constraints and message truncation hazards. Shared writable ring buffers across processes are prohibited. Transport compression is prohibited.
- **Serialization**: Protocol Buffers (protobuf v3) for strongly typed cross-language RPC commands, query requests, challenge dialogues, and coalescible progress notifications.
- **Identity Binding & Handshake**:
  - The client launches `pigtree-engine.exe` passing a session identifier and dedicated pipe endpoint via non-secret command-line parameters.
  - Same-integrity bootstrap credentials are delivered via an inherited read-only handle.
  - Mutual live process identity verification is enforced upon connection (`GetNamedPipeServerProcessId` and `GetNamedPipeClientProcessId`), validating that the connecting endpoints match the expected parent-child process pair.
  - An ephemeral channel key is established over the pipe following identity validation before operational commands are accepted. Zero secrets or channel keys are passed via command-line arguments.

#### Boundary 2: Session Host to Disposable Scan Worker (`pigtree-scan-worker.exe`)
- **Transport & Streams**: Dedicated inherited anonymous data pipe carrying versioned packed Little-Endian binary observation records from worker to engine.
- **Cancellation & Diagnostics**: A dedicated inherited Win32 manual-reset cancellation event handle (`CreateEventW`) checked concurrently during I/O via handle allow-listing, alongside an inherited stderr pipe for structured diagnostic logging.

#### Boundary 3: Session Host to Elevated Broker and Raw Parser
- **Host to Elevated Broker (`pigtree-elevated-broker.exe`)**:
  - Launched via `ShellExecuteExW` using the `runas` verb with non-secret routing parameters (session GUID and pipe endpoint name).
  - The broker connects back to the engine's elevated-restricted Named Pipe.
  - The engine verifies that the connecting process holds high integrity and elevated token privileges via `GetNamedPipeClientProcessId` and `OpenProcessToken` before establishing the channel key.
- **Elevated Broker to Raw Parser (`pigtree-raw-parser.exe`)**:
  - The broker opens the raw volume handle (`\\.\<Volume>`) with read-only access.
  - The broker creates a restricted security token (disabling administrative SIDs, stripping `SeBackupPrivilege` and elevated privileges) or AppContainer isolation.
  - The broker spawns `pigtree-raw-parser.exe` under the restricted token, providing the read-only volume handle as an inherited handle alongside an inherited anonymous output pipe.
  - The parser streams versioned packed Little-Endian metadata records over the anonymous pipe to the broker for invariant validation before forwarding to the engine.

#### Boundary 4: Session Host to Mutation Worker (`pigtree-mutation-worker.exe`)
- **Transport & Protocol**: Dedicated byte-mode bounded framed Named Pipe using Protobuf v3 for mutation control.
- **Authentication**: Authenticates using a single-use plan nonce and cryptographic digest of the exact, immutable Action Plan. The worker executes only authorized steps against verified live preconditions and terminates immediately upon group completion.

### 3. Analysis Snapshot Persistence and Storage Engine Format (`.pts` / `.ptse`)

PigTree implements an immutable, versioned, little-endian, aligned memory-mapped columnar format for base Analysis Snapshots (`.pts`) and ordered Snapshot Enrichments (`.ptse`):

1. **Domain Fidelity**: Explicitly encodes canonical domain facts including Run Outcome, Scope Coverage, Coverage Gaps, Value Knowledge states, Object Identity, entry/object graph topology, timestamps, security metadata, volume Capacity, Free Space, Reconciliation Difference, Unattributed Used Space, and Over-Accounted Allocation.
2. **Columnar Memory-Mapped Storage**: Core attributes (sizes, identities, reference counts, topology offsets) are stored as independent, contiguous primitive columnar arrays, enabling demand-paged OS virtual memory utilization and zero-copy slice casting via `zerocopy` / `bytemuck` without full-file heap deserialization.
3. **Graph Topology Representation**: Directory tree hierarchy is represented via a Compressed Sparse Row (CSR) index for $O(1)$ child lookups and linear memory traversals during subtree aggregations.
4. **Data Integrity & Atomic Settlement**: Each chunk includes checksum validation. Base snapshots and enrichments are written to temporary staging files, verified for internal consistency and checksum integrity, and atomically committed via file replace/rename.
5. **Format Versioning and Migration**: Follows an explicit version policy. Evolving formats migrate via full-file rewrite during explicit save/export operations; the engine does not maintain mutable in-place binary patch layers. The session host enforces fault isolation, failing closed on corrupted or unsupported files without risking memory corruption. Permanent unversioned backward compatibility is not promised beyond explicitly supported format releases.

### 4. WPF Production Rendering, Virtualization, and Accessibility Architecture

The graphical interface is built on C# / latest supported .NET LTS using WPF, structured to maintain responsive interaction on 5,000,000-entry datasets:

1. **Tree-Table Virtualization**:
   - Primary tree-table is implemented via a customized `ListView` paired with a recycling `VirtualizingStackPanel` (`VirtualizationMode="Recycling"`, `ScrollUnit="Pixel"`).
   - Backed by a `VirtualTreeCollection` implementing a flattened virtual projection model.
   - **No Full Client Mirror**: WPF never constructs a 5,000,000-entry managed object array or unmanaged descriptor arena. The collection virtualizes item count based on engine metadata, holding only a bounded sliding-window page cache (200–500 active rows).
   - Synchronous index lookups return lightweight placeholder items during rapid scrolling while fetching row data asynchronously from the engine over IPC.
2. **Hardware-Accelerated Treemap Visualization**:
   - Rendered using Direct3D 11 hosted inside WPF via `System.Windows.Interop.D3DImage` using a legacy DXGI shared handle to a Direct3D 9Ex surface.
   - **In-Process Presentation Geometry**: The Rust engine supplies semantic node weights and hierarchy; the WPF presentation layer calculates squarified $(x, y, w, h)$ bounding geometry locally in C# against active viewport pixel dimensions.
   - Cushion shading and boundaries are rendered in hardware via HLSL pixel shaders.
   - **Conservative Synchronization**: Rendering completes, issues an explicit `ID3D11DeviceContext::Flush()`, and signals `D3DImage.AddDirtyRect` on the UI thread, with robust recovery handling for device loss, `IsFrontBufferAvailableChanged`, WARP software rasterization, and software fallbacks.
3. **Truthful Bounded UI Automation (UIA) and Accessibility**:
   - Custom automation peers expose a truthful, bounded UIA tree covering realized items only.
   - Automation search by property (`FindItemByProperty`) evaluates cached items only, returning null for uncached items rather than blocking the UI thread or attempting to realize the entire tree into managed memory. Full search is delegated through engine-backed filtered projection queries.
   - Item realization (`Realize`) succeeds for cached providers and may become unavailable after cache eviction.
   - Dynamic Windows Contrast Themes tracking integrates with system theme brush keys across tree-table controls and treemap borders.
   - Per-Monitor DPI V2 awareness is enforced across all windowing, font rendering, and Direct3D shared viewports.

### 5. Distribution, Packaging, Shell Integration, and Update Strategy

1. **Runtime Deployment**: WPF client is compiled as a self-contained ReadyToRun (R2R) multi-file deployment targeting the latest supported .NET LTS on x64. The engine, CLI, and helper workers are compiled as optimized native Rust binaries.
2. **Installer Modality**:
   - Primary installer is built with **WiX Toolset v5** emitting a standard MSI package.
   - Default install mode is **Per-User** (`MSIINSTALLPERUSER=1`, installing to `%LocalAppData%\Programs\PigTree`), requiring zero UAC elevation prompts for installation or update.
   - Enterprise administrators can deploy per-machine via `ALLUSERS=1` into `%ProgramFiles%\PigTree`.
   - **MSIX is rejected as the primary installer** due to Virtualized File System (VFS) containerization friction, shell integration limitations, and enterprise deployment friction.
3. **First-Class Portable ZIP**: Standalone archive, zero-install, zero registry dependencies. In v1, updates operate via manual verified archive replacement (unless a versioned-directory atomic switch is deployed).
4. **Shell Integration**: Classic HKCU registry verbs (`Directory\shell\PigTree`, `Drive\shell\PigTree`) for standard users, alongside an optional Windows 11 sparse package (.msix with `allowExternalUri` and package identity) for modern context menus with classic fallback.
5. **Code Signing & Update Policy**: Production binaries and MSI packages are Authenticode signed with SHA-256 digests and RFC 3161 timestamps. Release packages are verified via cryptographic checksums before application. **Silent or forced background updates are strictly prohibited**. Multi-file in-use updates are staged to an isolated directory and applied upon restart.

### 6. Measurable Performance Targets and Release Gates

Performance targets are governed by [Measurable Performance Targets](../performance-targets.md) as binding release criteria across the 5,000,000-entry universal floor:

1. **Scale Floor**: Universal release floor of 5,000,000 observed Directory Entries on supported local storage without instability, memory exhaustion, or interactive stalls.
2. **Memory Footprint**: Total PigTree process family peak Private Bytes <= 1.5 GiB at 5M entries (base idle <= 256 MiB; incremental slope <= 256 bytes/entry).
3. **Snapshot Cold Reopen**: Opening a 5M-entry snapshot into an interactive query state p95 <= 3.0 s on NVMe SSD / <= 6.0 s on SATA SSD.
4. **Interactive Latency**: Single-column sort and primary page retrieval p95 <= 100 ms; multi-predicate queries p95 <= 200 ms.
5. **UI Frame Delivery**: Sustained 60 FPS target (frame delivery p95 <= 16.7 ms, p99 <= 33.3 ms, < 1% frames > 50 ms, zero main thread stalls > 200 ms) for tree-table scrolling and treemap zoom/pan.
6. **Standard Traversal Scan Rate**: Multi-threaded Win32 directory traversal median >= 170,000 entries/s on warm NVMe storage (>= 80,000 entries/s on SATA SSD).
7. **Raw MFT Scanning**: Gated strictly on invariant validation, safety verification, and release gates under ADR 0001 before stable release; not a promised throughput shortcut.
8. **Accessibility Responsiveness**: Screen reader element focus and UIA navigation response time <= 50 ms.

## Consequences

### Positive
- **Optimal Technology Specialization**: Combines Rust's deterministic memory management, zero-cost abstractions, and memory safety for core data structures with C#'s desktop GUI capabilities, rapid layout development, and accessibility infrastructure.
- **Guaranteed Working-Set Stability**: Memory-mapped columnar storage, bounded sliding-window UI paging, and GPU-accelerated Direct3D treemap rendering allow interactive analysis of 5M-entry datasets within the <= 1.5 GiB process-family budget.
- **Least-Privilege Security Boundary**: Seven-executable topology guarantees that untrusted raw parsers run under restricted tokens, elevation is confined to short-lived read-only brokers, and GUI/CLI clients run strictly at standard user integrity.
- **Zero-Friction Distribution**: Per-user MSI and portable ZIP distributions allow standard users to install, run, and update PigTree without administrative rights or invasive background daemons.
- **Truthful Accessibility**: Bounded UIA provider tree, contrast theme integration, and keyboard parity ensure equal usability for assistive technology users without risking Dispatcher lockups.

### Negative and Trade-offs
- **Multi-Process Orchestration**: Managing process lifecycles, IPC framing, mutual PID validation, and error recovery across seven distinct executables introduces architectural orchestration overhead compared to a monolithic binary.
- **Custom UI Virtualization**: WPF built-in controls cannot handle 5M items directly, requiring a custom virtual collection, bounded sliding-window caching, custom automation peers, and manual Direct3D `D3DImage` interop.
- **Self-Contained Deployment Footprint**: Shipping a self-contained ReadyToRun .NET LTS runtime alongside native Rust binaries results in an uncompressed disk distribution footprint of approximately 120–160 MiB.
- **Binary Storage Rigidity**: Memory-mapped columnar stores require disciplined schema versioning, validation checks, and rewrite migration pathways as the format evolves.

## Considered Options

The following alternative architectures were evaluated and rejected:

### 1. Monolithic Single-Process Architecture in C# (.NET)
- *Rejected*: Managing 5M object graphs in the managed .NET heap creates significant Garbage Collection pressure and pause latency during large traversals. It also lacks unmanaged memory safety when parsing potentially corrupt on-disk raw filesystem structures and violates least-privilege separation during elevated operations.

### 2. Monolithic Single-Process Architecture in Rust (e.g. Slint / iced / egui)
- *Rejected*: Pure-Rust GUI frameworks currently lack mature Windows UI Automation (UIA) provider models, comprehensive Windows Contrast Theme integration, native IME composition, rich virtualized tree-grid components, and robust Direct3D shared viewport interop.

### 3. Alternative UI Frameworks: WinUI 3, Tauri v2, Qt 6, Avalonia UI
- *WinUI 3 / Windows App SDK*: Rejected due to high runtime overhead, immature tree-table virtualization recycling, sluggish UIA responsiveness at scale, and external Windows App Runtime deployment dependencies.
- *Tauri v2 / WebView2*: Rejected due to high DOM/Chromium memory consumption when virtualizing dense tables, canvas-to-accessibility impedance mismatches, and slower scrolling compared to native desktop primitives.
- *Qt 6 (C++ / QML)*: Rejected due to commercial licensing complexity on Windows, lack of deep native Windows UIA provider integration compared to WPF, and cross-language maintenance overhead.
- *Avalonia UI*: Rejected because WPF provides deeper platform integration with Windows-specific contracts (Direct3D 9Ex `D3DImage` interop, native Win32 accessibility hooks, and Windows Contrast Themes) without introducing third-party UI framework dependencies.

### 4. Alternative Storage Engines: Relational SQL, DuckDB, Apache Arrow, Key-Value Stores
- *SQLite (Memory-Mapped / WAL)*: Rejected because relational B-tree row storage incurs substantial disk and memory overhead, lacks native Compressed Sparse Row (CSR) graph indexing, and introduces latency cliffs on deep recursive hierarchy queries.
- *DuckDB*: Rejected due to substantial engine binary footprint, lack of zero-copy slice transmutation into custom graph structures, and lack of domain-specific coverage gap and knowledge state primitives.
- *Apache Arrow IPC / Feather*: Rejected because generic Arrow schemas lack support for compressed graph adjacency lists, require custom serialization layers for domain knowledge states, and lack integrated chunk validation.
- *LMDB / RocksDB*: Rejected because key-value lookups require per-record deserialization and pointer indirection, preventing SIMD-vectorized columnar aggregation and fast full-table scans.

### 5. Alternative IPC Mechanisms: Message-Mode Pipes, Shared-Memory Ring Buffers, Compression
- *Message-Mode Named Pipes*: Rejected because Windows message-mode pipes introduce 64 KiB message size limits, cross-runtime framing quirks in .NET, and message truncation risks.
- *Shared-Memory Ring Buffers*: Rejected because writable cross-process shared memory introduces synchronization complexity, race condition hazards across integrity boundaries, and attack surface for negligible throughput gains over byte-mode named pipes.
- *Transport Compression (LZ4/Zstd on IPC)*: Rejected because local in-memory IPC bandwidth on modern systems exceeds multiple gigabytes per second, making compression/decompression CPU overhead a net throughput bottleneck.

### 6. Alternative Packaging: MSIX Primary Installer
- *MSIX / AppX*: Rejected as primary distribution because Virtualized File System (VFS) containerization creates operational friction for disk analyzers, context menu integration is fragile, and enterprise sysadmins resist MSIX deployment over standard MSI.
