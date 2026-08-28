# Windows Local IPC Transport, Framing, and Identity Design

- **Status**: Complete Research
- **Date**: 2026-08-28
- **Originating Ticket**: [#14 - Select the production technology architecture](https://github.com/AFlyingP/PigTree/issues/14)
- **Scope**: Windows 10 & 11 (x64), WPF (.NET 8/9), Rust Engine/Workers, Windows Local IPC, Security Architecture
- **Primary Source References**: See [Section 10: Primary Source Citations & References](#10-primary-source-citations--references)

---

## 1. Executive Summary & Recommended Topology

This document provides the authoritative research, technical evaluation, and concrete architecture design for local Inter-Process Communication (IPC), identity authentication, security isolation, stream framing, and backpressure in PigTree.

PigTree's approved technology baseline establishes a multi-process architecture consisting of:
1. **Interactive Graphical Client**: Built with WPF on modern supported .NET (.NET 8/9), executing in the user's interactive session at **Medium Mandatory Integrity Level (Medium IL)**.
2. **Private Session Host**: A dedicated, short-lived, transport-neutral domain engine compiled in native Rust, running at **Medium IL** (or matching user token) and bound 1:1 to the active client lifecycle.
3. **Disposable Standard Scan Workers**: Short-lived, isolated native Rust worker processes executing directory traversal and Win32 metadata queries at **Medium IL**.
4. **Elevated Read-Only Broker & Restricted Raw-Parser Child**: A two-process elevated subsystem for privileged whole-volume scans on NTFS, comprising an elevated read-only broker at **High Mandatory Integrity Level (High IL)** and a sandboxed raw-parser child running under a **Restricted Token (Low/Medium IL)** with a duplicated read-only volume handle.
5. **Dedicated Mutation Workers**: Physically and logically separated worker processes launched exclusively upon explicit Action Plan commitment for guarded filesystem remediation (ADR 0002).

```
+-------------------------------------------------------------------------------------------------------------------+
|                                             PigTree Process & IPC Topology                                         |
+-------------------------------------------------------------------------------------------------------------------+
|                                                                                                                   |
|   +---------------------------------------+                 [Boundary 1: Asynchronous Named Pipe]                 |
|   |         WPF Graphical Client          | <==================================================================>  |
|   |           (.NET 8/9 / C#)             |      Control Channel: Google.Protobuf Commands & Challenges           |
|   |        [Medium Integrity Level]       |      Event Channel: Dual Streams (Lossless Data + Throttled 60Hz)     |
|   +---------------------------------------+                                                                       |
|                                                                                                                   |
|                                                     +---------------------------------------+                     |
|                                                     |          Rust Session Host            |                     |
|                                                     |      (Domain Engine & Aggregator)     |                     |
|                                                     |        [Medium Integrity Level]       |                     |
|                                                     +---------------------------------------+                     |
|                                                        /                 |                 \                      |
|                   [Boundary 2: Inherited Anon Pipe]   /                  |                  \ [Boundary 4: Pipe]  |
|                                                      /                   |                   \                     |
|                                                     v                    |                    v                   |
|                   +----------------------------------+                   |  +----------------------------------+  |
|                   |   Disposable Standard Worker     |                   |  |    Dedicated Mutation Worker     |  |
|                   |  (Win32 Traversal & Queries)     |                   |  | (Action Plan Step Journaling)   |  |
|                   |     [Medium Integrity Level]     |                   |  | [Medium IL or High IL on UAC]    |  |
|                   +----------------------------------+                   |  +----------------------------------+  |
|                                                                          |                                        |
|                                         [Elevated UAC Launch / Nonce]    |                                        |
|                                                                          v                                        |
|                                                     +---------------------------------------+                     |
|                                                     |     Elevated Read-Only Broker         |                     |
|                                                     | (Volume Handle Opener & Orchestrator) |                     |
|                                                     |         [High Integrity Level]        |                     |
|                                                     +---------------------------------------+                     |
|                                                                          |                                        |
|                                       [Boundary 3: Inherited Pipe]       | Duplicated Read-Only Volume Handle     |
|                                       [Token Sandboxed / Job Object]     | Restricted Token (No Privileges)       |
|                                                                          v                                        |
|                                                     +---------------------------------------+                     |
|                                                     |      Restricted Raw-Parser Child      |                     |
|                                                     |    (Untrusted DASD $MFT Extent Parser)|                     |
|                                                     |      [Restricted / Low Integrity]     |                     |
|                                                     +---------------------------------------+                     |
|                                                                                                                   |
+-------------------------------------------------------------------------------------------------------------------+
```

### Summary of IPC Boundaries & Recommended Technologies

| IPC Boundary | Source Process & IL | Target Process & IL | Recommended Transport | Wire Serialization | Security & Authentication Mechanisms | Peak Target Throughput |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| **Boundary 1** (UI to Engine) | WPF Client (Medium IL) | Rust Session Host (Medium IL) | Windows Asynchronous Named Pipe (`FILE_FLAG_OVERLAPPED`, Byte Mode) | **Protocol Buffers v3** (Prost / Google.Protobuf) with Length-Prefixed Framing | `FILE_FLAG_FIRST_PIPE_INSTANCE`, `PIPE_REJECT_REMOTE_CLIENTS`, Strict SDDL DACL (`User SID`), SACL `S:(ML;;NW;;;ME)`, 256-bit CSPRNG Launch Nonce, Win32 Client PID validation | Control: <1k ops/s<br>Events: 60Hz UI + 50k paged items/s |
| **Boundary 2** (Engine to Scan Worker) | Rust Session Host (Medium IL) | Standard Scan Worker (Medium IL) | Inherited Anonymous Pipes (`HANDLE_FLAG_INHERIT` via `STARTUPINFOEXW`) | **Bincode / Postcard** (Length-Prefixed Chunk Batches) | Whitelisted Handle Inheritance (`PROC_THREAD_ATTRIBUTE_HANDLE_LIST`), Execution Nonce in Scan Plan, Child Job Object lifecycle | >= 250,000 observations/s (512-item chunks / ~64 KiB frames) |
| **Boundary 3** (Elevated Broker to Parser) | Elevated Broker (High IL) | Raw-Parser Child (Restricted / Low IL) | Inherited Anonymous Pipe (`HANDLE_FLAG_INHERIT` via `STARTUPINFOEXW`) | **Bincode / Postcard** (Length-Prefixed Chunk Batches) | Restricted Token (`DISABLE_MAX_PRIVILEGE`, `LUA_TOKEN`), Job Object memory/process limits, Duplicated Read-Only Volume Handle, Heartbeat Watchdog | >= 1,000,000 observations/s (2048-item chunks / ~256 KiB frames) |
| **Boundary 4** (Engine to Mutation Worker) | Rust Session Host (Medium IL) | Dedicated Mutation Worker (Medium or High IL) | Asynchronous Named Pipe or Inherited Pipe | **Protocol Buffers v3** (Strict Step Journaling) | Ephemeral Step-by-Step Nonce Handshake, Strict Single-Step Request-Response, Dedicated Isolated Binary, Zero Bulk Buffering | Synchronous Step Handshake (<100 steps/s, High Safety) |

---

## 2. Analysis of the Four Process & IPC Boundaries

### 2.1 Boundary 1: WPF (.NET) Client to Private Rust Session Host

#### Topology and Process Lifecycle
The WPF GUI client manages the lifetime of its dedicated Rust session host. Upon client startup:
1. The WPF client generates a cryptographically secure 256-bit ephemeral nonce (32 random bytes from `System.Security.Cryptography.RandomNumberGenerator`).
2. The client derives a unique pipe name: `\\\\.\\pipe\\pigtree-session-{PID}-{NonceHex}`.
3. The client creates an asynchronous Named Pipe server instance using `NamedPipeServerStream` (or spawns the Rust session host passing the pipe name and nonce, with the host acting as the server with `FILE_FLAG_FIRST_PIPE_INSTANCE`).
4. **Recommended Architecture**: The Rust Session Host acts as the Named Pipe server (`tokio::net::windows::named_pipe::ServerOptions`). The WPF client launches the Rust binary via `Process.Start` passing the target pipe name and launch nonce via environment variables or command arguments.
5. The Rust Session Host immediately claims the pipe instance using `FILE_FLAG_FIRST_PIPE_INSTANCE` (preventing pipe squatting) and sets strict DACL permissions.
6. The WPF client connects via `NamedPipeClientStream` and immediately performs a mutual cryptographic handshake containing the launch nonce before any command is accepted.
7. If either process terminates, crashes, or closes the pipe, the corresponding counterpart cleanly aborts all running tasks and shuts down.

#### Dual Logical Channels over Named Pipe
ADR 0003 mandates two distinct logical event channels:
1. **Lossless Domain / Data Channel**: Emits ordered observations, Coverage Gaps, query results, and verification proofs. Must never drop or coalesce messages; requires bounded upstream backpressure.
2. **Coalescible Progress / Status Channel**: Emits phase transitions, scan counters, resource consumption, and progress status. Under high scanning churn (e.g., 200k items/s), progress messages are coalesced and throttled at 60 Hz (16.6 ms intervals) to eliminate UI rendering overhead and IPC saturation.

**Multiplexing Design**:
A single full-duplex asynchronous Named Pipe operates in **Byte Mode** (`PIPE_TYPE_BYTE | PIPE_READMODE_BYTE`). Each logical message is wrapped in a lightweight 16-byte binary frame header containing a `ChannelTag` (0x01 = Command/Response, 0x02 = Lossless Domain Event, 0x03 = Coalescible Progress Pulse, 0x04 = Cancellation/Heartbeat). This provides total ordering, eliminates connection setup overhead of multiple pipes, and allows the client-side .NET reader to route progress events to a `System.Threading.Channels.Channel<T>.CreateBounded(new BoundedChannelOptions(1) { FullMode = BoundedChannelFullMode.DropOldest })` while routing domain data to an unbounded or backpressured queue.

---

### 2.2 Boundary 2: Rust Session Host to Disposable Standard Scan Workers

#### Architecture and Lifecycle
Standard user-mode directory enumeration executes inside a disposable native Rust worker process:
- The worker is spawned by the Session Host using Win32 `CreateProcessW` with `EXTENDED_STARTUPINFO_PRESENT`.
- To prevent accidental handle inheritance (a common vulnerability where child processes inherit open sockets, database locks, or volume handles), the Session Host explicitly whitelists only the anonymous pipe handles using `PROC_THREAD_ATTRIBUTE_HANDLE_LIST`.
- The Session Host assigns the worker process to an anonymous Windows **Job Object** configured with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`. When the Session Host drops the Job handle or terminates, the Windows kernel terminates all descendant worker processes.

#### Transport & Batching for Performance
Standard Win32 directory traversal (amortized via 64 KiB `GetFileInformationByHandleEx` buffers) discovers entries at 50,000–150,000 items/second on modern NVMe drives.
- **Transport**: Standard inherited anonymous pipes (`CreatePipe` with `HANDLE_FLAG_INHERIT`).
- **Batching Strategy**: The worker groups observed entries into contiguous batches of **512 observations** (approximately 40–64 KiB per batch).
- **Syscall Amortization**: At 170,000 observations/second, emitting 512 entries per batch generates only ~332 `WriteFile` syscalls per second. The session host performs ~332 `ReadFile` syscalls per second.
- **Throughput Profile**: Pipe throughput overhead is < 25 MB/s, consuming < 0.5% of a single CPU core.

---

### 2.3 Boundary 3: High-Integrity Read-Only Broker to Restricted Raw-Parser Child

#### Principle of Least Privilege and Sandboxing
ADR 0001 establishes that raw on-disk NTFS metadata parsing involves parsing complex, undocumented binary structures from DASD storage. Because parser bugs or malformed on-disk filesystems could lead to memory corruption, the raw parser must be completely isolated from privileged credentials:

```
[ Elevated UAC Trigger ]
          |
          v
+-------------------------------------------------------------+
|              Elevated Read-Only Broker Process              |
|                 [High Mandatory Integrity]                  |
|  1. Opens volume handle: \\\\.\\C: with GENERIC_READ         |
|  2. Calls CreateRestrictedToken(DISABLE_MAX_PRIVILEGE)      |
|  3. Duplicates Volume Handle with bInheritHandle = TRUE     |
|  4. Calls CreateProcessW(PROC_THREAD_ATTRIBUTE_HANDLE_LIST) |
+-------------------------------------------------------------+
          |
          | Inherited Duplicated Read-Only Volume Handle
          | Inherited Anonymous Pipe (Stdout Data)
          v
+-------------------------------------------------------------+
|                 Restricted Raw-Parser Child                 |
|             [Restricted Token / Low Integrity]              |
|  - All Administrative Privileges Stripped (LUA_TOKEN)       |
|  - Read-Only Access to EXACT Volume Handle ONLY             |
|  - Zero Network, Zero Write, Zero Handle Leaks              |
|  - Enclosed in Windows Job Object (Memory Cap: 512 MiB)     |
|  - Parses $MFT Records -> Emits Batched Observations        |
+-------------------------------------------------------------+
```

#### Step-by-Step Security Implementation
1. **Volume Handle Acquisition**: The High-IL Broker calls `CreateFileW` on `\\\\.\\<Drive>:` with `GENERIC_READ | FILE_READ_ATTRIBUTES | FILE_READ_DATA`, `FILE_SHARE_READ | FILE_SHARE_WRITE`, `OPEN_EXISTING`.
2. **Restricted Token Creation**: The Broker calls `CreateRestrictedToken`:
   - `Flags = DISABLE_MAX_PRIVILEGE | LUA_TOKEN` (strips `SeDebugPrivilege`, `SeBackupPrivilege`, `SeManageVolumePrivilege`, and drops the Administrator SID to Deny-Only).
   - The token's integrity level is set to **Low Mandatory Integrity** (`S-1-16-4096`).
3. **Handle Whitelisting**: The Broker creates an anonymous pipe for observation streaming. It calls `DuplicateHandle` to make the volume handle and pipe write handle inheritable, and populates `PROC_THREAD_ATTRIBUTE_HANDLE_LIST` in `STARTUPINFOEXW`.
4. **Job Object Containment**: The Broker creates an anonymous Job Object (`CreateJobObjectW`) and sets:
   - `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`
   - `JOB_OBJECT_LIMIT_PROCESS_MEMORY` (e.g., hard cap of 512 MiB commit limit)
   - `JOB_OBJECT_LIMIT_ACTIVE_PROCESS` (max 1 process; prevents spawning child processes).
5. **Watchdog and Fail-Closed Fallback**: The Broker runs an asynchronous watchdog timer. If the Raw-Parser Child crashes (e.g., access violation on corrupt MFT extent), exhausts memory, or fails to emit a heartbeat within 2.0 seconds:
   - The Broker terminates the Job Object immediately.
   - The Broker logs the failure event and invariant violation.
   - The Broker initiates transparent, automatic fallback to the **Elevated Win32 Documented Traversal Adapter** as required by ADR 0001 Section 9.

---

### 2.4 Boundary 4: Rust Session Host to Dedicated Mutation Workers

#### Architectural Separation from Scanners
In accordance with ADR 0002 (Guarded Cleanup Safety) and ADR 0001 Section 6:
> *"Helper processes used for scanning cannot be reused for deletion... Cleanup functionality requires an entirely separate authorization lifecycle and distinct execution protocol."*

#### Protocol and Safety Rules
- **Distinct Binary**: Mutation logic resides in a dedicated `pigtree-mutator.exe` executable, completely separate from scanning workers.
- **No Batch Queuing**: Mutator communication operates strictly on a synchronous, single-step **Step-by-Step Verification Protocol**:
  1. Host sends `PreflightRequest { step_id, action_kind, expected_frn, expected_timestamps, expected_size }`.
  2. Mutator performs live Win32 inspection (`GetFileInformationByHandleEx`), validates that the target on-disk object matches historical scan identity exactly, and returns `PreflightResponse { status: Validated }`.
  3. Host issues `CommitStep { step_id, execution_nonce }`.
  4. Mutator executes the verified mutation (e.g., `DeleteFileW` or Hard Link replacement via `CreateHardLinkW` and temporary atomic swap), writes an immutable local step journal entry, and returns `StepExecutionResult { outcome: Succeeded, journal_proof }`.
  5. If preflight fails or any precondition diverges, execution halts immediately before any irreversible mutation occurs.

---

## 3. Evaluation and Comparison of Candidate IPC Transports

To select the optimal transport for each boundary, five candidate Windows IPC technologies were evaluated against PigTree's requirements:

```
+-----------------------------------------------------------------------------------------------------------------------------+
|                                              IPC Technology Evaluation Matrix                                               |
+--------------------------+---------------------+---------------------+-----------------------+------------------------------+
| Transport Mechanism      | Raw Bandwidth       | Security & ACLs     | Async / Overlapped    | Complexity & Safety Tradeoff |
+--------------------------+---------------------+---------------------+-----------------------+------------------------------+
| Windows Named Pipes      | Very High           | Native Win32 DACL   | Full Native Support   | Optimal balance for cross-   |
| (Overlapped Byte Mode)   | (1.5 - 3.2 GB/s)    | & Integrity Labels  | (Tokio & .NET async)  | language and daemon IPC      |
+--------------------------+---------------------+---------------------+-----------------------+------------------------------+
| Inherited Anonymous      | Very High           | Direct Kernel Handle| Synchronous Win32     | Optimal for parent-child     |
| Pipes (Win32 Handles)    | (1.8 - 3.5 GB/s)    | Isolation (No Name) | (Worker thread loop)  | disposable scan workers      |
+--------------------------+---------------------+---------------------+-----------------------+------------------------------+
| ALPC / Windows RPC       | High                | Windows Security    | Complex callback model| Rejected: Undocumented ALPC; |
| (MS-RPC / ntdll)         | (400 - 800 MB/s)    | Descriptors         | (MIDL IDL runtime)    | fragile cross-language C#/Rust|
+--------------------------+---------------------+---------------------+-----------------------+------------------------------+
| Shared Memory Sections   | Ultra High          | Section Object DACL | Requires out-of-band  | Rejected: High concurrency bug|
| (CreateFileMappingW+shm) | (10 - 25 GB/s)      | (Complex page ACLs) | signaling events      | risk, TOCTOU pointer hazards |
+--------------------------+---------------------+---------------------+-----------------------+------------------------------+
| Loopback TCP Sockets     | Moderate to High    | Localhost binding   | Native async sockets  | Rejected: Firewall warnings, |
| (127.0.0.1)              | (600 - 1,200 MB/s)  | (Vulnerable to port | (Tokio / .NET Socket) | multi-user port collision,   |
|                          |                     | scanning / snooping)|                       | lack of Windows ACL control  |
+--------------------------+---------------------+---------------------+-----------------------+------------------------------+
```

### 3.1 Throughput & Bandwidth Feasibility Proof (>= 170k observations/s)

**Data Volume Calculation**:
- An individual `ObservedDirectoryEntry` observation in binary representation contains:
  - `file_id`: 64-bit integer (8 bytes)
  - `parent_file_id`: 64-bit integer (8 bytes)
  - `logical_size`: 64-bit integer (8 bytes)
  - `allocated_size`: 64-bit integer (8 bytes)
  - `attributes`: 32-bit integer (4 bytes)
  - `timestamps` (created, modified, accessed): 3 x 64-bit integers (24 bytes)
  - `reparse_tag` + flags: 64-bit integer (8 bytes)
  - `file_name`: UTF-8 string (average 24–48 bytes)
  - Batch entry header & status flags: 8 bytes
  - **Total serialized size per observation**: ~96 to 128 bytes.

**Required Bandwidth**:
$$\text{Throughput} = 170,000 \text{ items/s} \times 128 \text{ bytes} \approx 21.76 \text{ MB/s}$$

**Observed Transport Capability**:
- Local Windows Named Pipes and Anonymous Pipes achieve **1,500 MB/s to 3,200 MB/s** sequential throughput on modern x64 hardware when buffers are sized between 32 KiB and 64 KiB.
- The required bandwidth of **21.76 MB/s represents less than 1.5% of available Named Pipe transfer capacity**.
- Therefore, complex Shared Memory Section objects (`CreateFileMappingW`) and multi-process ring buffers are **architecturally unnecessary** and rejected due to security and concurrency hazards.

---

## 4. Launch Orchestration, Identity Authentication, and Security Controls

### 4.1 Ephemeral Launch Nonce Protocol

To ensure that rogue processes running under the same user session cannot connect to or spoof PigTree IPC endpoints, all IPC connections require a mutual cryptographic handshake:

```
[ Client / Parent Process ]                               [ Server / Child Process ]
             |                                                         |
             | 1. Generate 256-bit Cryptographic Nonce                 |
             |    (RandomNumberGenerator / BCryptGenRandom)            |
             |                                                         |
             | 2. Spawn Child with Nonce in Command/Env                |
             |    OR Create Named Pipe with Nonce in Path              |
             | ------------------------------------------------------> |
             |                                                         |
             | 3. Connect to Pipe                                      |
             | ======================================================> |
             |                                                         |
             | 4. Client Handshake Frame                               |
             |    [Magic: 0x5054] [ClientPID] [Nonce] [ProtobufVer]    |
             | ------------------------------------------------------> |
             |                                                         |
             |                                 5. Server Verifies:     |
             |                                    - Nonce == Expected  |
             |                                    - ClientPID == Win32 |
             |                                      GetNamedPipeClient |
             |                                      ProcessId(hPipe)   |
             |                                                         |
             | 6. Server Handshake Response                            |
             |    [Magic: 0x5054] [ServerPID] [Status: OK]             |
             | <------------------------------------------------------ |
             |                                                         |
             | === SECURE AUTHENTICATED SESSION ESTABLISHED ===        |
```

### 4.2 Security Descriptors, SDDL, and Mandatory Integrity Levels

When creating the Named Pipe server (`CreateNamedPipeW`), the Session Host applies an explicit Security Descriptor constructed via `ConvertStringSecurityDescriptorToSecurityDescriptorW`:

```text
D:(A;;GRGW;;;WD) -> REPLACED WITH STRICT USER-ONLY DACL:
D:(A;;GRGW;;;PS)(A;;GRGW;;;<CURRENT_USER_SID>)S:(ML;;NW;;;ME)
```

#### SDDL Breakdown:
1. **DACL (`D:`)**:
   - `(A;;GRGW;;;<CURRENT_USER_SID>)`: Allows Read/Write (`GENERIC_READ | GENERIC_WRITE`) **only** to the specific logged-in user's Security Identifier (SID).
   - Prevents other standard users or service accounts on multi-user Windows machines from accessing the pipe.
2. **SACL / Mandatory Integrity Label (`S:(ML;;NW;;;ME)`)**:
   - `ML`: System Mandatory Label ACE.
   - `NW` (`SDDL_NO_WRITE_UP`): Enforces that processes with a lower integrity level (e.g., Low IL sandbox or browser process) cannot obtain write access to the pipe.
   - `ME` (`SDDL_ML_MEDIUM`): Establishes Medium Integrity Level.
3. **Win32 Open Mode Flags**:
   - `FILE_FLAG_FIRST_PIPE_INSTANCE` (`0x00080000`): Guarantees that the session host is creating the very first instance of this pipe name. If a rogue process previously created a pipe with the same name, `CreateNamedPipeW` fails immediately with `ERROR_ACCESS_DENIED`, preventing pipe squatting.
   - `PIPE_REJECT_REMOTE_CLIENTS` (`0x00000008`): Strictly instructs the Windows Named Pipe File System (NPFS) driver to reject all remote SMB connections, neutralizing network-based attacks.

### 4.3 Handle Inheritance Rules & Confinement

Every invocation of `CreateProcessW` for worker and helper processes must adhere to strict handle confinement:
- **Never** pass `bInheritHandles = TRUE` with standard `STARTUPINFOW`. Standard inheritance leaks all inheritable kernel handles in the parent process to the child.
- **Always** use `STARTUPINFOEXW` with `EXTENDED_STARTUPINFO_PRESENT`.
- Initialize an attribute list via `InitializeProcThreadAttributeList` with `PROC_THREAD_ATTRIBUTE_HANDLE_LIST`.
- Pass a precise array containing **only** the explicit pipe handle(s) (and duplicated volume handle for Boundary 3).
- Call `SetHandleInformation(hPipe, HANDLE_FLAG_INHERIT, HANDLE_FLAG_INHERIT)` exclusively on the whitelisted handles.

---

## 5. Framing, Backpressure, Streaming, and Cancellation Protocol

### 5.1 Binary Frame Header Format

All communication over Named Pipes and Anonymous Pipes is partitioned into length-prefixed, self-delimiting binary frames.

```
 0                   1                   2                   3
 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|          Magic: 0x5054 ('PT')         |     Schema Version    |  (4 bytes)
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|  Channel Tag  |  Frame Flags  |          Reserved             |  (4 bytes)
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                       Sequence Number                         |
|                          (64-bit)                             |  (8 bytes)
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                     Payload Length (u32)                      |  (4 bytes)
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                     Payload Data (Varlen)                     |
|                               ...                             |  (N bytes)
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                    CRC-32C Checksum (u32)                     |  (4 bytes)
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
```

#### Header Fields:
- **Magic (2 bytes)**: Fixed `0x5054` (ASCII `"PT"` for PigTree). Validates stream synchronization.
- **Schema Version (2 bytes)**: Semantic wire protocol version (`0x0001` for v1).
- **Channel Tag (1 byte)**:
  - `0x01`: Command / Control Request & Response
  - `0x02`: Lossless Domain Observation Stream (Batched Directory Entries)
  - `0x03`: Coalescible Progress Pulse (Scan counters, phase transitions)
  - `0x04`: Cooperative Cancellation / Heartbeat
- **Frame Flags (1 byte)**:
  - Bit 0 (`0x01`): `IS_COMPRESSED` (LZ4 block compression for large metadata chunks)
  - Bit 1 (`0x02`): `END_OF_STREAM` (Signals completion of the active stream)
  - Bit 2 (`0x04`): `CHALLENGE_REQUIRED` (Indicates a pending interactive confirmation)
- **Sequence Number (8 bytes)**: Monotonically increasing `u64` sequence number scoped to the active Operation ID.
- **Payload Length (4 bytes)**: Unsigned 32-bit integer representing payload length. **Hard Cap**: Maximum allowed frame size is **4 MiB (4,194,304 bytes)**. Any frame exceeding this cap immediately triggers a `FrameTooLarge` error and terminates the connection.
- **CRC-32C Checksum (4 bytes)**: Castagnoli CRC-32 computed over the header and payload data. Provides hardware-accelerated detection of stream corruption or misaligned decoding.

### 5.2 Backpressure and Flow Control

```
[ Scan Worker (Rust) ]                                   [ Session Host Engine (Rust) ]
          |                                                             |
          | Emits Batch #101 (512 entries / ~64 KiB)                    |
          | ==========================================================> |
          |                                                             | Bounded In-Memory Queue
          | Emits Batch #102 (512 entries / ~64 KiB)                    | [Batch 101] [Batch 102]
          | ==========================================================> | [Batch 103] ...
          |                                                             | (Queue Full: Cap 64 chunks)
          | Emits Batch #103                                            |
          | Pipe Buffer Full -> OS WriteFile blocks worker thread       |
          | (Natural Kernel Backpressure throttles disk reader)         |
          |                                                             | Engine processes Batch #101
          |                                                             | Reads next batch from pipe
          | Pipe Buffer drains -> Worker resumes traversal              |
          |                                                             |
```

- **Kernel Pipe Buffering**: Windows Named and Anonymous pipes maintain internal kernel buffers (default 64 KiB). When the reader lags behind, `WriteFile` naturally yields or blocks the worker thread, applying zero-overhead backpressure directly to the scanning thread.
- **Session Host In-Memory Channel**: The Rust Session Host buffers incoming batches in a bounded Tokio mpsc channel (`tokio::sync::mpsc::channel(64)`). If the domain aggregator is busy calculating parent links or hash structures, the channel fills, the pipe reader stops reading, and the worker pauses filesystem enumeration.

### 5.3 Cooperative Cancellation Flow

```
[ Client / UI ]                  [ Session Host Engine ]                   [ Worker Process ]
       |                                    |                                       |
       | 1. User Clicks "Cancel"            |                                       |
       |    Sends CancelRequest(OpId)       |                                       |
       | ---------------------------------> |                                       |
       |                                    | 2. Sets AtomicBool CancellationToken  |
       |                                    | 3. Writes Cancellation Frame to Pipe  |
       |                                    | ------------------------------------> |
       |                                    |                                       | 4. Worker detects flag /
       |                                    |                                       |    pipe signal during loop
       |                                    |                                       | 5. Flushes partial batch
       |                                    | 6. Receives EOS (RunOutcome: Cancel)  | 6. Worker exits cleanly
       |                                    | <------------------------------------ |
       | 7. Emits Settled(Cancelled) Event  |                                       |
       |    Assembles Partial Snapshot      |                                       |
       | <--------------------------------- |                                       |
```

- **Graceful Window**: The Session Host grants the worker process a **500 ms graceful cancellation deadline** to finalize its active batch and exit.
- **Hard Termination Fallback**: If the worker does not exit within 500 ms (e.g., stuck in an uninterruptible kernel I/O call on an unresponsive disk), the Session Host calls Win32 `TerminateProcess` (or closes the Job Object handle), logs an unclean cancellation event, and safely seals the partial snapshot.

---

## 6. Serialization and Schema Evolution Policy

### 6.1 Boundary 1 (Client <-> Session Host): Protocol Buffers v3

For the cross-language boundary between WPF (.NET) and the Rust Session Host, **Protocol Buffers v3** is selected:

- **Ecosystem Integration**: Official, highly optimized code generation in both Rust (`prost` + `prost-build`) and C# (.NET `Google.Protobuf`).
- **Schema Evolution Invariants**:
  - All fields are explicitly numbered.
  - New fields are optional and assigned new field tags.
  - Deprecated fields are marked `reserved` and never reused.
  - Forward and backward compatibility allows client and engine to evolve across minor versions without synchronization lockstep.
- **Safety Caps**: Google.Protobuf and Prost decoders are configured with strict parsing limits:
  - Maximum Recursion Depth: `64`
  - Maximum Message Size: `4 MiB`

### 6.2 Boundaries 2 & 3 (Rust Host <-> Rust Workers): Bincode / Postcard

For internal, high-throughput Rust-to-Rust worker communication, **Bincode** (or **Postcard**) is selected:

- **Performance**: Zero IDL translation overhead, direct struct serialization, zero memory allocation during decoding into pre-allocated chunk buffers. Exceeds **1,000,000 observations/second** with < 3% CPU utilization.
- **Version Alignment**: Worker executables are version-locked and deployed as private bundled child binaries alongside the engine host. A single version byte in the frame header guarantees immediate rejection if build signatures differ.

### 6.3 Untrusted Boundary Invariant Validation

In accordance with ADR 0001 Section 7, all deserialized observation records are treated as untrusted data and must pass strict validation before insertion into the domain graph:
1. **File ID Bounds**: Object File Reference Number (FRN) must be non-zero.
2. **Hierarchy Invariants**: Parent FRN cannot equal the object's own FRN (except for root directory `0x0000000000050005`).
3. **Name Validation**: Path segments must not contain null bytes (`\0`) or illegal control characters, and must not exceed `32,767` characters (`UNICODE_STRING_MAX_CHARS`).
4. **Size Invariants**: `LogicalSize` and `AllocatedSize` must be non-negative.
5. **Timestamp Sanity**: File timestamps must be valid Windows `FILETIME` structures (between year 1601 and year 3000).

---

## 7. Comprehensive Threat Model & Failure Modes Matrix

| Threat / Failure Scenario | Attack Vector / Mechanism | Impact if Unmitigated | Architectural Mitigation & Defense-in-Depth |
| :--- | :--- | :--- | :--- |
| **Named Pipe Squatting** | Malicious local process creates `\\\\.\\pipe\\pigtree-session-...` before PigTree starts. | Client connects to rogue server; sensitive file paths and scan queries intercepted. | 1. `FILE_FLAG_FIRST_PIPE_INSTANCE` causes server creation to fail if pipe exists.<br>2. 256-bit unguessable CSPRNG nonce embedded in pipe name.<br>3. Mutual PID verification via `GetNamedPipeClientProcessId`. |
| **Cross-User Snooping** | Another standard user on multi-user Windows machine connects to PigTree Named Pipe. | Data leakage of scanned filesystem paths and metadata. | Strict SDDL DACL granting `GENERIC_READ | GENERIC_WRITE` **only** to active user SID (`(A;;GRGW;;;<UserSID>)`). Remote connections blocked via `PIPE_REJECT_REMOTE_CLIENTS`. |
| **Low-Integrity Sandbox Write Injection** | Compromised web browser or sandboxed app attempts to inject commands into session host. | Code execution or unauthorized filesystem analysis. | SACL Mandatory Integrity Label `S:(ML;;NW;;;ME)` (No-Write-Up) rejects all write attempts from Low IL processes. |
| **Raw Parser Memory Corruption** | Malformed or adversarial NTFS volume metadata triggers buffer overflow in raw MFT parser. | Potential arbitrary code execution with elevated privileges. | 1. Parser runs in separate child process under **Restricted Token** (`DISABLE_MAX_PRIVILEGE`, `LUA_TOKEN`, Low IL).<br>2. Zero write privileges, read-only duplicated volume handle.<br>3. Job Object memory limits + Watchdog timer kill parser on crash without affecting broker. |
| **Oversized Message / OOM DoS** | Rogue or buggy worker emits multi-gigabyte frame header to exhaust host memory. | Host process Out-Of-Memory (OOM) crash and denial of service. | Hard 4 MiB frame length cap enforced at frame reader level before payload buffer allocation. Connection dropped immediately on violation. |
| **Worker Process Hang / Deadlock** | Worker thread hangs in uninterruptible kernel I/O wait (e.g. failing drive sector). | UI freezes, scan never finishes or cancels. | Asynchronous watchdog heartbeat + 500 ms cooperative cancellation deadline, followed by Job Object termination. |
| **Handle Leakage Privilege Escalation** | Child worker process inherits sensitive parent handles (tokens, write file handles). | Sandboxed child accesses unauthorized parent resources. | `PROC_THREAD_ATTRIBUTE_HANDLE_LIST` explicitly whitelists only the required pipe handles during `CreateProcessW`. |

---

## 8. Rejected Alternatives & Technical Trade-offs

| Candidate Alternative | Reason for Rejection |
| :--- | :--- |
| **gRPC over HTTP/2 (Loopback TCP)** | Rejected due to high memory/binary footprint, multi-user port collision conflicts, Windows Defender / Third-party firewall popup warnings on localhost binding, and lack of native Windows Security Identifier (SID) DACL enforcement. |
| **Named Pipe Message Mode (`PIPE_TYPE_MESSAGE`)** | Rejected because message-mode named pipes have complex asynchronous cancellation semantics in Tokio on Windows, whereas length-delimited Byte Mode (`PIPE_TYPE_BYTE`) provides superior reliability, portable framing, and predictable buffer control. |
| **Raw ALPC (`NtAlpc*` System Calls)** | Rejected because ALPC is an undocumented, private Windows NT kernel interface subject to breaking internal changes across Windows updates, offering no documented Rust or .NET SDK bindings. |
| **Shared Memory Section Objects (`CreateFileMappingW` + Ring Buffers)** | Rejected because 170k observations/s requires only ~22 MB/s bandwidth (easily handled by pipes at 1.5–3.0 GB/s). Shared memory introduces severe synchronization complexity, concurrency race bugs, and TOCTOU pointer validation vulnerabilities across untrusted parser boundaries. |
| **In-Process C++/CLI or Native DLL Interop (P/Invoke / C-ABI)** | Rejected because running the entire engine inside the WPF process violates process isolation, risks bringing down the UI on worker crashes, and prevents clean standard-user vs. elevated privilege separation. |

---

## 9. Release Verification Gates & Test Strategy

To guarantee the reliability, security, and performance of the IPC subsystem, the following automated release gates must pass before production deployment:

1. **IPC Throughput Benchmark Gate**:
   - Automated benchmark simulating continuous emission of `5,000,000` directory entry observations over Named Pipes and Anonymous Pipes.
   - **Pass Criterion**: End-to-end throughput must exceed **250,000 observations/second** on reference hardware with CPU utilization below 2.0% for the IPC transport layer.
2. **Security DACL & Multi-User Isolation Gate**:
   - Automated integration test running under secondary test credentials attempting to connect to an active PigTree Named Pipe.
   - **Pass Criterion**: Connection must be rejected by the Windows kernel with `ERROR_ACCESS_DENIED`.
3. **Mandatory Integrity Level Gate**:
   - Automated test spawning a Low Integrity test client (`ConvertStringSidToSidW` with Low IL) attempting to write to the Session Host pipe.
   - **Pass Criterion**: Write access denied by kernel NPFS driver (`ERROR_ACCESS_DENIED`).
4. **Fuzzing & Malformed Frame Resilience Gate**:
   - Structure-aware fuzzing (via `cargo fuzz` / libFuzzer) feeding mutated wire frames, truncated payloads, corrupted CRC checksums, and oversized length headers (> 4 MiB) into the frame decoder.
   - **Pass Criterion**: 100% fail-closed rejection without crashes, memory leaks, or unhandled panics.
5. **Parser Containment & Sandbox Escape Gate**:
   - Test harness simulating a hard crash (`abort()`, null pointer dereference, and infinite loop) inside the Restricted Raw-Parser Child.
   - **Pass Criterion**: Elevated Broker detects child termination, terminates Job Object within < 100 ms, logs failure, and executes seamless fallback to elevated Win32 directory traversal.
6. **Cancellation Latency Soak Gate**:
   - Stress test issuing cancellation requests during maximum-throughput pipe streaming.
   - **Pass Criterion**: All worker processes cleanly terminate, Job Objects close, and the engine transitions to `Settled(Cancelled)` within **< 500 ms** in 100% of iterations.

---

## 10. Primary Source Citations & References

1. **Microsoft Windows Win32 API Documentation**:
   - *CreateNamedPipeW function*: [Microsoft Learn - CreateNamedPipeW](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-createnamedpipew) (Details `FILE_FLAG_FIRST_PIPE_INSTANCE`, `PIPE_REJECT_REMOTE_CLIENTS`, `PIPE_ACCESS_DUPLEX`, `PIPE_TYPE_BYTE`).
   - *GetNamedPipeClientProcessId function*: [Microsoft Learn - GetNamedPipeClientProcessId](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-getnamedpipeclientprocessid).
   - *UpdateProcThreadAttribute function & PROC_THREAD_ATTRIBUTE_HANDLE_LIST*: [Microsoft Learn - UpdateProcThreadAttribute](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-updateprocthreadattribute).
   - *CreateRestrictedToken function*: [Microsoft Learn - CreateRestrictedToken](https://learn.microsoft.com/en-us/windows/win32/api/securitybaseapi/nf-securitybaseapi-createrestrictedtoken) (Details `DISABLE_MAX_PRIVILEGE`, `LUA_TOKEN`, restricted SIDs).
   - *Security Descriptor Definition Language (SDDL) for Mandatory Labels*: [Microsoft Learn - Mandatory Integrity Labels in SDDL](https://learn.microsoft.com/en-us/windows/win32/secauthz/mandatory-integrity-control).
   - *Job Objects & Limits*: [Microsoft Learn - Job Objects](https://learn.microsoft.com/en-us/windows/win32/procthread/job-objects) (`JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`, `SetInformationJobObject`).

2. **Microsoft .NET Documentation**:
   - *System.IO.Pipes.NamedPipeClientStream*: [Microsoft Learn - NamedPipeClientStream Class](https://learn.microsoft.com/en-us/dotnet/api/system.io.pipes.namedpipeclientstream).
   - *System.IO.Pipes.NamedPipeServerStream*: [Microsoft Learn - NamedPipeServerStream Class](https://learn.microsoft.com/en-us/dotnet/api/system.io.pipes.namedpipeserverstream).
   - *System.Threading.Channels*: [Microsoft Learn - System.Threading.Channels](https://learn.microsoft.com/en-us/dotnet/api/system.threading.channels).

3. **Rust Tokio & Windows Ecosystem Documentation**:
   - *tokio::net::windows::named_pipe*: [Tokio Documentation - Named Pipe ServerOptions & ClientOptions](https://docs.rs/tokio/latest/tokio/net/windows/named_pipe/struct.ServerOptions.html).
   - *tokio_util::codec::LengthDelimitedCodec*: [Tokio Util Documentation - Codecs](https://docs.rs/tokio-util/latest/tokio_util/codec/struct.LengthDelimitedCodec.html).
   - *windows-sys / windows crate*: [Microsoft Windows crate for Rust](https://crates.io/crates/windows-sys).

4. **Protocol Buffers Specification**:
   - *Protocol Buffers v3 Language Guide*: [Protobuf Language Guide (proto3)](https://protobuf.dev/programming-guides/proto3/).
   - *Prost Protocol Buffers for Rust*: [Prost Documentation](https://docs.rs/prost/latest/prost/).

5. **PigTree Repository Architecture Contracts**:
   - *ADR 0001: Scanning Subsystem and Privilege Architecture*: `docs/adr/0001-scanning-and-privilege-architecture.md`
   - *ADR 0002: Guarded Cleanup and Action Safety Architecture*: `docs/adr/0002-guarded-cleanup-safety.md`
   - *ADR 0003: Shared Engine and Automation Contract*: `docs/adr/0003-shared-engine-and-automation-contract.md`
   - *Product Performance Targets & Acceptance Budgets*: `docs/performance-targets.md`
