# Windows Local IPC Transport, Framing, and Identity Design

- **Status**: Complete Research
- **Date**: 2026-08-28
- **Originating Ticket**: [#14 - Select the production technology architecture](https://github.com/AFlyingP/PigTree/issues/14)
- **Scope**: Windows 10 & 11 (x64), WPF (.NET 8/9), Rust Engine/Workers, Windows Local IPC, Security Architecture
- **Primary Source References**: See [Section 10: Primary Source Citations & References](#10-primary-source-citations--references)

---

## 1. Executive Summary & Recommended Topology

This document provides the authoritative research, technical evaluation, security model, and concrete protocol specification for local Inter-Process Communication (IPC), identity authentication, privilege boundaries, stream framing, and flow control across all PigTree subsystems.

PigTree's approved technology baseline establishes a multi-process architecture consisting of:
1. **Interactive Graphical Client**: Built with WPF on modern supported .NET (.NET 8/9), executing in the user's interactive desktop session at **Medium Mandatory Integrity Level (Medium IL)**.
2. **Private Session Host**: A dedicated, short-lived, transport-neutral domain engine compiled in native Rust, running at **Medium IL** (or matching user token) and bound 1:1 to the active client lifecycle.
3. **Disposable Standard Scan Workers**: Short-lived, isolated native Rust worker processes executing Win32 directory traversal and metadata queries at **Medium IL**.
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
|                   + Inherited Cancellation Event     /                   |                   \                     |
|                                                     v                    |                    v                   |
|                   +----------------------------------+                   |  +----------------------------------+  |
|                   |   Disposable Standard Worker     |                   |  |    Dedicated Mutation Worker     |  |
|                   |  (Win32 Traversal & Queries)     |                   |  | (Action Plan Step Journaling)   |  |
|                   |     [Medium Integrity Level]     |                   |  | [Medium IL or High IL on UAC]    |  |
|                   +----------------------------------+                   |  +----------------------------------+  |
|                                                                          |                                        |
|                                            [ShellExecuteExW runas UAC]   | Session-Host Coordinated Elevation     |
|                                            [Ephemeral Broker Nonce]      | (Full Interactive GUI & CLI Parity)    |
|                                                                          v                                        |
|                                                     +---------------------------------------+                     |
|                                                     |     Elevated Read-Only Broker         |                     |
|                                                     | (Volume Handle Opener & Orchestrator) |                     |
|                                                     |         [High Integrity Level]        |                     |
|                                                     +---------------------------------------+                     |
|                                                                          |                                        |
|                                       [Boundary 3: Inherited Pipe]       | Duplicated Read-Only Volume Handle     |
|                                       [CreateProcessAsUserW Sandbox]     | Restricted Token (No Privileges)       |
|                                       [Job Object Containment]           | Low Mandatory Integrity Level          |
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

| IPC Boundary | Source Process & IL | Target Process & IL | Recommended Transport | Wire Serialization Policy | Security & Authentication Mechanisms | Modeled Target Throughput |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| **Boundary 1** (Client to Engine) | WPF Client (Medium IL) | Rust Session Host (Medium IL) | Windows Asynchronous Named Pipe (`FILE_FLAG_OVERLAPPED`, Byte Mode) | **Protocol Buffers v3** (Prost / Google.Protobuf) with Length-Prefixed Framing | `FILE_FLAG_FIRST_PIPE_INSTANCE`, `PIPE_REJECT_REMOTE_CLIENTS`, User SID DACL, SACL `S:(ML;;NW;;;ME)`, Non-CLI Nonce via Bootstrap Pipe, Client PID & Creation Time Verification, `TokenImpersonationLevel.Identification` | Control: <1k ops/s<br>Events: 60Hz UI + Paged Views |
| **Boundary 2** (Engine to Scan Worker) | Rust Session Host (Medium IL) | Standard Scan Worker (Medium IL) | Uni-directional Inherited Anonymous Pipe + Inherited Manual-Reset Event | **Versioned Packed Binary Records** (Explicit Little-Endian Layout) | Whitelisted Handle Inheritance (`bInheritHandles = TRUE`, `PROC_THREAD_ATTRIBUTE_HANDLE_LIST`), Non-CLI Nonce Delivery, Child Job Object Lifecycle | Modeled Floor >= 170k obs/s<br>(Batch: 512 items / ~64 KiB) |
| **Boundary 3** (Elevated Broker to Parser) | Elevated Broker (High IL) | Raw-Parser Child (Restricted / Low IL) | Uni-directional Inherited Anonymous Pipe + Read-Only Volume Handle | **Versioned Packed Binary Records** (Explicit Little-Endian Layout) | `CreateRestrictedToken` + `SetTokenInformation(TokenIntegrityLevel)`, `CreateProcessAsUserW`, Job Object Memory Limit, Watchdog Heartbeat | Modeled Proj >= 500k–1M obs/s<br>(Batch: 2048 items / ~256 KiB) |
| **Boundary 4** (Engine to Mutation Worker) | Rust Session Host (Medium IL) | Dedicated Mutation Worker (Medium or High IL) | Asynchronous Named Pipe or Dedicated Inherited Pipe | **Protocol Buffers v3** (Strict Step Journaling) | Ephemeral Step-by-Step Nonce Handshake, Strict Single-Step Request-Response, Dedicated Isolated Binary (`pigtree-mutator.exe`), Zero Bulk Queuing | Synchronous Step Handshake (<100 steps/s, High Safety) |

---

## 2. Analysis of the Four Process & IPC Boundaries

### 2.1 Boundary 1: WPF (.NET) Client to Private Rust Session Host

#### Topology and Process Lifecycle
The WPF GUI client manages the lifetime of its dedicated Rust session host. Upon client startup:
1. The WPF client generates a cryptographically secure 256-bit ephemeral launch nonce (32 random bytes from `System.Security.Cryptography.RandomNumberGenerator`).
2. The WPF client creates an anonymous, read-only bootstrap pipe for initial secret delivery.
3. The WPF client derives a unique Named Pipe server endpoint: `\\\\.\\pipe\\pigtree-session-{PID}-{SessionUUID}`.
4. The client launches the Rust Session Host binary (`pigtree-engine.exe`) using `CreateProcessW` with `EXTENDED_STARTUPINFO_PRESENT`, explicitly allow-listing the bootstrap pipe handle via `PROC_THREAD_ATTRIBUTE_HANDLE_LIST` and passing the target Named Pipe path.
5. **No secrets are passed via command-line arguments** (neutralizing command-line inspection via Task Manager, WMI, or ETW). The launch nonce is written by the client into the bootstrap pipe handle and read once by the host during startup.
6. The Rust Session Host creates the Named Pipe instance using `CreateNamedPipeW` with `FILE_FLAG_FIRST_PIPE_INSTANCE` and `PIPE_REJECT_REMOTE_CLIENTS`, applying an explicit SDDL Security Descriptor.
7. The WPF client connects using `.NET NamedPipeClientStream` configured strictly with `TokenImpersonationLevel.Identification` (preventing any unauthorized token impersonation by the server).
8. The client transmits a handshake frame containing the 256-bit launch nonce.
9. The Session Host validates:
   - Client process ID via Win32 `GetNamedPipeClientProcessId(hPipe, &clientPid)`.
   - Client session ID via Win32 `GetNamedPipeClientSessionId(hPipe, &clientSessionId)`.
   - Process creation timestamp via `GetProcessTimes` against the expected parent process handle (binding the pipe connection to the exact process instance and defeating rapid PID-reuse attacks).
   - The cryptographic match of the 256-bit launch nonce.
10. If either process terminates or disconnects, the counterpart shuts down cleanly.

#### Dual Logical Channels over Named Pipe
ADR 0003 mandates two distinct logical event channels:
1. **Lossless Domain / Data Channel**: Emits ordered observations, Coverage Gaps, query results, and verification proofs. Must never drop or coalesce messages; requires bounded upstream backpressure.
2. **Coalescible Progress / Status Channel**: Emits phase transitions, scan counters, resource consumption, and progress status. Under high scanning churn (e.g., 200k items/s), progress messages are coalesced and throttled at 60 Hz (16.6 ms intervals) to eliminate UI rendering overhead and IPC saturation.

**Multiplexing Design**:
A single full-duplex asynchronous Named Pipe operates in **Byte Mode** (`PIPE_TYPE_BYTE | PIPE_READMODE_BYTE`). Each logical message is wrapped in a 20-byte binary frame header containing a `ChannelTag` (0x01 = Command/Response, 0x02 = Lossless Domain Event, 0x03 = Coalescible Progress Pulse, 0x04 = Cancellation/Heartbeat). This provides total ordering, eliminates connection setup overhead of multiple pipes, and allows the client-side .NET reader to route progress events to a `System.Threading.Channels.Channel<T>.CreateBounded(new BoundedChannelOptions(1) { FullMode = BoundedChannelFullMode.DropOldest })` while routing domain data to an unbounded or backpressured queue.

---

### 2.2 Boundary 2: Rust Session Host to Disposable Standard Scan Workers

#### Architecture and Lifecycle
Standard user-mode directory enumeration executes inside a disposable native Rust worker process (`pigtree-worker.exe`):
- The worker is spawned by the Session Host using Win32 `CreateProcessW` with `EXTENDED_STARTUPINFO_PRESENT` and `bInheritHandles = TRUE`.
- To prevent accidental handle inheritance (a common vulnerability where child processes inherit open sockets, database locks, or volume handles), the Session Host explicitly populates `PROC_THREAD_ATTRIBUTE_HANDLE_LIST` containing **only** the worker's stdout anonymous pipe handle and the cancellation event handle.
- The Session Host assigns the worker process to an anonymous Windows **Job Object** configured with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`. When the Session Host drops the Job handle or terminates, the Windows kernel terminates all descendant worker processes.

#### Dedicated Inherited Manual-Reset Cancellation Event
Host-to-worker cancellation must not depend on bidirectional pipe multiplexing or complex reader loops in the scanning worker.
- **Mechanism**: The Session Host creates an anonymous manual-reset Win32 Event (`CreateEventW(NULL, TRUE, FALSE, NULL)`), sets `SetHandleInformation(hCancelEvent, HANDLE_FLAG_INHERIT, HANDLE_FLAG_INHERIT)`, and passes it in the `PROC_THREAD_ATTRIBUTE_HANDLE_LIST`.
- **Data Flow**: The data pipe is strictly uni-directional (Worker -> Host). The worker reads the cancellation event handle during traversal (using non-blocking `WaitForSingleObject(hCancelEvent, 0) == WAIT_OBJECT_0` or asynchronous event polling).
- **Graceful Window & Escalation**: When the host signals the event (`SetEvent(hCancelEvent)`), the worker immediately pauses directory traversal, flushes its active batch with an `END_OF_STREAM` flag, and exits cleanly. If the worker fails to exit within 500 ms, the Session Host escalates to hard termination via `TerminateJobObject` / `TerminateProcess`.

---

### 2.3 Boundary 3: High-Integrity Read-Only Broker to Restricted Raw-Parser Child

#### Elevated Orchestration Flow (GUI & CLI Parity)
Privileged whole-volume analysis requires an explicit, audited elevation flow coordinated by the domain engine:
1. The client (WPF UI or CLI) submits an `analysis.start` command requesting whole-volume analysis with elevated policy.
2. The **Medium-Integrity Rust Session Host** evaluates the scan plan, identifies the privilege requirement, and returns a typed `ElevationChallenge { challenge_id, scan_target, proposed_adapter }` over Boundary 1.
3. The client presents the challenge to the user (GUI dialog or interactive CLI prompt). Upon user approval, the client submits `challenge.accept { challenge_id }`.
4. The **Medium-Integrity Session Host—not the WPF client—coordinates elevation**:
   - The Session Host creates a dedicated Named Pipe listener `\\\\.\\pipe\\pigtree-broker-{SessionUUID}` with a freshly generated 256-bit broker launch nonce and first-instance protection.
   - The Session Host invokes `ShellExecuteExW` with `lpVerb = L"runas"` targeting `pigtree-broker.exe`, passing the broker pipe path.
   - Windows triggers the User Account Control (UAC) consent dialog.
   - Upon elevation, `pigtree-broker.exe` (High IL) connects to the Session Host's broker pipe, submits the 256-bit broker nonce, and verifies the active session and plan ID.
   - This architecture guarantees identical privilege orchestration for both GUI and CLI automation workflows.

#### Restricted Raw-Parser Child Launch & Sandboxing
ADR 0001 establishes that raw on-disk NTFS metadata parsing involves parsing complex, undocumented binary structures from DASD storage. Because parser bugs or malformed on-disk filesystems could lead to memory corruption, the raw parser must be completely isolated from privileged credentials:

```
[ High-Integrity Broker Process ]
  1. Opens volume handle: \\\\.\\C: with GENERIC_READ | FILE_READ_ATTRIBUTES | FILE_READ_DATA
  2. Opens Broker Process Primary Token via OpenProcessToken()
  3. Calls CreateRestrictedToken(hToken, DISABLE_MAX_PRIVILEGE | LUA_TOKEN, ...)
  4. Calls SetTokenInformation(hRestrictedToken, TokenIntegrityLevel, LowIntegritySID)
  5. Duplicates Volume Handle & Pipe Handle with bInheritHandle = TRUE
  6. Initializes STARTUPINFOEXW with PROC_THREAD_ATTRIBUTE_HANDLE_LIST
  7. Calls CreateProcessAsUserW(hRestrictedToken, ..., bInheritHandles = TRUE, &siEx, ...)
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

#### Step-by-Step Win32 API Implementation:
1. **Primary Token Restriction**:
   - `CreateProcessW` cannot assign a new primary token to a child process. The Broker must use `CreateProcessAsUserW`.
   - The Broker calls `OpenProcessToken(GetCurrentProcess(), TOKEN_DUPLICATE | TOKEN_QUERY | TOKEN_ADJUST_DEFAULT | TOKEN_ASSIGN_PRIMARY, &hToken)`.
   - The Broker calls `CreateRestrictedToken(hToken, DISABLE_MAX_PRIVILEGE | LUA_TOKEN, 0, NULL, 0, NULL, 0, NULL, &hRestrictedToken)`.
   - The Broker calls `SetTokenInformation(hRestrictedToken, TokenIntegrityLevel, &mandatoryLabel, sizeof(TOKEN_MANDATORY_LABEL))` configuring **Low Mandatory Integrity** (`S-1-16-4096`).
   - *Privilege Verification*: Elevated administrator processes have `SeAssignPrimaryTokenPrivilege` and `SeIncreaseQuotaPrivilege` in their token privileges. The broker enables these via `AdjustTokenPrivileges` prior to invoking `CreateProcessAsUserW`.
2. **Handle Whitelisting & Confinement**:
   - The Broker creates an anonymous pipe for observation streaming.
   - The Broker calls `DuplicateHandle` on the raw volume handle and pipe write handle, setting `bInheritHandle = TRUE`.
   - The Broker calls `InitializeProcThreadAttributeList` and `UpdateProcThreadAttribute` with `PROC_THREAD_ATTRIBUTE_HANDLE_LIST`, passing the explicit array of inheritable handles.
3. **Process Creation**:
   - The Broker calls `CreateProcessAsUserW` passing `hRestrictedToken`, `bInheritHandles = TRUE`, and `EXTENDED_STARTUPINFO_PRESENT`.
4. **Job Object Containment**:
   - The Broker assigns the child to an anonymous Job Object with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`, `JOB_OBJECT_LIMIT_PROCESS_MEMORY` (512 MiB hard commit limit), and `JOB_OBJECT_LIMIT_ACTIVE_PROCESS` (1).
5. **Watchdog and Fail-Closed Fallback**:
   - The Broker monitors the child via an asynchronous watchdog timer. If the child crashes, exceeds memory limits, or fails to emit a heartbeat within 2.0 seconds:
     - The Broker terminates the Job Object immediately.
     - The Broker logs the failure event and invariant violation.
     - The Broker executes automatic, transparent fallback to the **Elevated Win32 Documented Traversal Adapter** as required by ADR 0001 Section 9.

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

To select the optimal transport for each boundary, candidate Windows IPC technologies were evaluated against PigTree's requirements:

```
+-----------------------------------------------------------------------------------------------------------------------------+
|                                              IPC Technology Evaluation Matrix                                               |
+--------------------------+---------------------+---------------------+-----------------------+------------------------------+
| Transport Mechanism      | Modeled Bandwidth   | Security & ACLs     | Async / Overlapped    | Complexity & Safety Tradeoff |
+--------------------------+---------------------+---------------------+-----------------------+------------------------------+
| Windows Named Pipes      | Synthetic Pipe Cap: | Native Win32 DACL   | Full Native Support   | Optimal balance for cross-   |
| (Overlapped Byte Mode)   | > 1,500 MB/s        | & Integrity Labels  | (Tokio & .NET async)  | language and engine IPC      |
+--------------------------+---------------------+---------------------+-----------------------+------------------------------+
| Inherited Anonymous      | Synthetic Pipe Cap: | Direct Kernel Handle| Synchronous Win32     | Optimal for parent-child     |
| Pipes (Win32 Handles)    | > 1,500 MB/s        | Isolation (No Name) | (Worker thread loop)  | disposable scan workers      |
+--------------------------+---------------------+---------------------+-----------------------+------------------------------+
| ALPC / Windows RPC       | Synthetic RPC Cap:  | Windows Security    | Complex callback model| Rejected: Undocumented ALPC; |
| (MS-RPC / ntdll)         | ~ 400 - 800 MB/s    | Descriptors         | (MIDL IDL runtime)    | fragile cross-language C#/Rust|
+--------------------------+---------------------+---------------------+-----------------------+------------------------------+
| Shared Memory Sections   | Memory Bus Cap:     | Section Object DACL | Requires out-of-band  | Rejected: High concurrency bug|
| (CreateFileMappingW+shm) | > 10,000 MB/s       | (Complex page ACLs) | signaling events      | risk, TOCTOU pointer hazards |
+--------------------------+---------------------+---------------------+-----------------------+------------------------------+
| Loopback TCP Sockets     | Synthetic Socket:   | Localhost binding   | Native async sockets  | Rejected: Firewall warnings, |
| (127.0.0.1)              | ~ 600 - 1,200 MB/s  | (Vulnerable to port | (Tokio / .NET Socket) | multi-user port collision,   |
|                          |                     | scanning / snooping)|                       | lack of Windows ACL control  |
+--------------------------+---------------------+---------------------+-----------------------+------------------------------+
```

### 3.1 Throughput & Bandwidth Modeled Projections (>= 170k observations/s)

**Modeled Data Volume**:
- An individual `ObservedDirectoryEntry` observation contains:
  - `file_id`: 64-bit integer (8 bytes)
  - `parent_file_id`: 64-bit integer (8 bytes)
  - `logical_size`: 64-bit integer (8 bytes)
  - `allocated_size`: 64-bit integer (8 bytes)
  - `attributes`: 32-bit integer (4 bytes)
  - `timestamps` (created, modified, accessed): 3 x 64-bit integers (24 bytes)
  - `reparse_tag` + flags: 64-bit integer (8 bytes)
  - `file_name`: Length-prefixed UTF-8 string (modeled average: 32–48 bytes)
  - Status flags & alignment: 8 bytes
  - **Total modeled record size per observation**: ~110 to 128 bytes.

**Required Modeled Transport Bandwidth**:
$$\text{Required Bandwidth} = 170,000 \text{ items/s} \times 128 \text{ bytes} \approx 21.76 \text{ MB/s}$$

**Analysis & Decision**:
- Windows Named Pipes and Anonymous Pipes operating with 64 KiB buffers provide synthetic transfer capacity exceeding **1,500 MB/s**, which is more than **60x the required bandwidth of 21.76 MB/s**.
- Grouping observations into 512-item chunks (~64 KiB) requires only **~332 pipe write/read system calls per second**.
- Compression (such as LZ4) is **explicitly dropped** from local IPC. The 21.76 MB/s payload is trivial for local pipes; adding compression would waste CPU cycles and introduce unnecessary decoding complexity.
- Shared Memory Section objects (`CreateFileMappingW`) are **rejected** because they introduce severe cross-process synchronization hazards, race conditions, and TOCTOU pointer vulnerabilities across untrusted parser boundaries without providing measurable end-to-end performance gains.

---

## 4. Launch Orchestration, Identity Authentication, and Security Controls

### 4.1 Nonce Delivery and Process Identity Binding

To protect against local tampering, eavesdropping, and rapid PID-reuse attacks, all IPC connections enforce multi-factor identity binding:

```
[ Client / Parent Process ]                               [ Server / Child Process ]
             |                                                         |
             | 1. Generate 256-bit CSPRNG Launch Secret                |
             | 2. Create Inherited Read-Only Bootstrap Pipe            |
             | 3. CreateProcessW(PROC_THREAD_ATTRIBUTE_HANDLE_LIST)   |
             | ------------------------------------------------------> |
             |                                                         |
             | 4. Transmits 256-bit Secret over Bootstrap Pipe         |
             | ======================================================> |
             |                                 5. Reads Launch Secret  |
             |                                 6. Closes Bootstrap Pipe|
             |                                 7. Creates Named Pipe   |
             |                                    (FIRST_PIPE_INSTANCE |
             |                                     REJECT_REMOTE)      |
             |                                                         |
             | 8. Connects with TokenImpersonationLevel.Identification |
             | ======================================================> |
             |                                                         |
             | 9. Sends Handshake Frame [Magic, ClientPID, Nonce]      |
             | ------------------------------------------------------> |
             |                                                         |
             |                                10. Server Validates:    |
             |                                    - Nonce Matches      |
             |                                    - GetNamedPipeClient |
             |                                      ProcessId == PID   |
             |                                    - GetProcessTimes    |
             |                                      Creation Time Match|
             |                                    - ClientSessionId    |
             |                                                         |
             | 11. Handshake Accepted [Status: OK]                     |
             | <------------------------------------------------------ |
             |                                                         |
             | === SECURE AUTHENTICATED SESSION ESTABLISHED ===        |
```

### 4.2 Security Descriptors, SDDL, and Mandatory Integrity Levels

When creating the Named Pipe server (`CreateNamedPipeW`), the Session Host applies an explicit Security Descriptor constructed via `ConvertStringSecurityDescriptorToSecurityDescriptorW`:

```text
D:(A;;GRGW;;;<CURRENT_USER_SID>)S:(ML;;NW;;;ME)
```

#### SDDL Specification:
1. **DACL (`D:`)**:
   - `(A;;GRGW;;;<CURRENT_USER_SID>)`: Allows Read/Write (`GENERIC_READ | GENERIC_WRITE`) **only** to the specific logged-in user's Security Identifier (SID), retrieved dynamically via `GetTokenInformation(TokenUser)`.
   - Prevents other standard users or service accounts on multi-user Windows machines from accessing the pipe.
   - *Optional Administrator / SYSTEM Policy*: If elevated background diagnostics are enabled, `(A;;GRGW;;;SY)(A;;GRGW;;;BA)` may be appended.
2. **SACL / Mandatory Integrity Label (`S:(ML;;NW;;;ME)`)**:
   - `ML`: System Mandatory Label ACE.
   - `NW` (`SDDL_NO_WRITE_UP`): Enforces that processes with a lower integrity level (e.g., Low IL sandbox or browser process) cannot obtain write access to the pipe.
   - `ME` (`SDDL_ML_MEDIUM`): Establishes Medium Integrity Level.
3. **Win32 Open Mode Flags**:
   - `FILE_FLAG_FIRST_PIPE_INSTANCE` (`0x00080000`): Guarantees that the session host is creating the very first instance of this pipe name. If a rogue process previously created a pipe with the same name, `CreateNamedPipeW` fails immediately with `ERROR_ACCESS_DENIED`, preventing pipe squatting.
   - `PIPE_REJECT_REMOTE_CLIENTS` (`0x00000008`): Strictly instructs the Windows Named Pipe File System (NPFS) driver to reject all remote SMB connections, neutralizing network-based attacks.

### 4.3 Handle Inheritance Rules & Confinement

Every invocation of `CreateProcessW` and `CreateProcessAsUserW` for worker and helper processes must adhere to strict handle confinement:
- Set `bInheritHandles = TRUE` in `CreateProcessW` / `CreateProcessAsUserW` (mandatory when using handle lists).
- Pass `EXTENDED_STARTUPINFO_PRESENT` in `dwCreationFlags`.
- Initialize `STARTUPINFOEXW` with `InitializeProcThreadAttributeList` and `UpdateProcThreadAttribute` using `PROC_THREAD_ATTRIBUTE_HANDLE_LIST`.
- Pass an explicit array containing **only** the required inheritable handles (e.g., anonymous pipe stdout handle, cancellation event handle, or duplicated read-only volume handle).
- Ensure `SetHandleInformation(handle, HANDLE_FLAG_INHERIT, HANDLE_FLAG_INHERIT)` has been called on every whitelisted handle.
- Windows strictly blocks all other open handles in the parent process from crossing into the child.

---

## 5. Framing, Backpressure, Streaming, and Cancellation Protocol

### 5.1 Binary Frame Header Format

All communication over Named Pipes and Anonymous Pipes is partitioned into length-prefixed, self-delimiting binary frames.

```
 0                   1                   2                   3
 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|          Magic: 0x5054 ('PT')         |     Schema Version    |  (4 bytes: u16 + u16)
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|  Channel Tag  |  Frame Flags  |           Reserved            |  (4 bytes: u8 + u8 + u16)
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                       Sequence Number                         |
|                          (64-bit)                             |  (8 bytes: u64)
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                     Payload Length (u32)                      |  (4 bytes: u32)
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                     Payload Data (Varlen)                     |
|                               ...                             |  (N bytes)
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                    CRC-32C Checksum (u32)                     |  (4 bytes: u32)
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
```

#### Exact Header Field Layout & Arithmetic:
- **Magic (2 bytes, offset 0..1)**: Fixed `0x5054` (ASCII `"PT"` for PigTree).
- **Schema Version (2 bytes, offset 2..3)**: Semantic wire protocol version (`0x0001` for v1).
- **Channel Tag (1 byte, offset 4)**:
  - `0x01`: Command / Control Request & Response
  - `0x02`: Lossless Domain Observation Stream (Batched Directory Entries)
  - `0x03`: Coalescible Progress Pulse (Scan counters, phase transitions)
  - `0x04`: Cooperative Cancellation / Heartbeat
- **Frame Flags (1 byte, offset 5)**:
  - Bit 0 (`0x01`): `END_OF_STREAM` (Signals terminal frame of an operation)
  - Bit 1 (`0x02`): `CHALLENGE_REQUIRED` (Indicates a pending interactive confirmation)
- **Reserved (2 bytes, offset 6..7)**: Must be zeroed (`0x0000`).
- **Sequence Number (8 bytes, offset 8..15)**: Monotonically increasing unsigned 64-bit integer scoped to the Operation ID.
- **Payload Length (4 bytes, offset 16..19)**: Unsigned 32-bit integer representing payload length $N$. **Hard Cap**: Maximum allowed frame size is **4 MiB (4,194,304 bytes)**. Any frame exceeding this cap triggers immediate connection termination.
- **Total Fixed Header Size**: **20 bytes**.
- **Payload Data**: $N$ bytes (offset 20 .. $20 + N - 1$).
- **CRC-32C Checksum (4 bytes, offset $20+N$ .. $23+N$)**: **CRC-32C (Castagnoli polynomial `0x1EDC6F41`, standardized in RFC 3720 / SSE4.2 hardware instruction `CRC32`)** computed over the entire 20-byte header plus the $N$-byte payload.
- **Total Wire Frame Size**: **$24 + N$ bytes**.

### 5.2 Backpressure and Flow Control

- **Kernel Pipe Buffering**: Windows Named and Anonymous pipes maintain internal kernel buffers (default 64 KiB). When the consumer lags behind, `WriteFile` blocks the worker thread, applying zero-overhead backpressure directly to filesystem enumeration.
- **Session Host In-Memory Channel**: The Rust Session Host buffers incoming batches in a bounded Tokio mpsc channel (`tokio::sync::mpsc::channel(64)`). If domain snapshot aggregation slows down, the channel fills, the pipe reader stops reading, and the worker pauses filesystem traversal.

---

## 6. Serialization and Schema Evolution Policy

### 6.1 Boundary 1 (Client <-> Session Host): Protocol Buffers v3

For the cross-language boundary between WPF (.NET) and the Rust Session Host, **Protocol Buffers v3** is selected:
- **Ecosystem Integration**: Official code generation in both Rust (`prost` + `prost-build`) and C# (.NET `Google.Protobuf`).
- **Schema Evolution Invariants**:
  - All fields are explicitly numbered.
  - New fields are optional and assigned new field tags.
  - Deprecated fields are marked `reserved` and never reused.
  - Forward and backward compatibility allows client and engine to evolve across minor versions without synchronization lockstep.
- **Safety Caps**: Google.Protobuf and Prost decoders are configured with strict parsing limits:
  - Maximum Recursion Depth: `64`
  - Maximum Message Size: `4 MiB`

### 6.2 Boundaries 2 & 3 (Rust Host <-> Scan Workers / Parser Child): Packed Binary Records

To guarantee extreme throughput, zero parsing ambiguity, and strict deterministic layout across privilege boundaries, the observation stream uses a **Versioned Packed Binary Record Format**:

```
+-------------------------------------------------------------------------------+
|                       Batch Header (16 bytes)                                 |
|  record_count: u32 | batch_flags: u32 | scan_epoch: u64                       |
+-------------------------------------------------------------------------------+
|                       Contiguous Array of Observation Records                 |
|  +-------------------------------------------------------------------------+  |
|  | Fixed-Width Observation Header (72 bytes)                               |  |
|  | - file_id: u64 (8 bytes)                                                |  |
|  | - parent_file_id: u64 (8 bytes)                                         |  |
|  | - logical_size: u64 (8 bytes)                                           |  |
|  | - allocated_size: u64 (8 bytes)                                         |  |
|  | - file_attributes: u32 (4 bytes)                                        |  |
|  | - observation_status: u32 (4 bytes)                                     |  |
|  | - creation_time: u64 (8 bytes, FILETIME)                                |  |
|  | - last_write_time: u64 (8 bytes, FILETIME)                              |  |
|  | - last_access_time: u64 (8 bytes, FILETIME)                             |  |
|  | - reparse_tag: u32 (4 bytes)                                            |  |
|  | - name_length_bytes: u16 (2 bytes)                                      |  |
|  | - reserved: u16 (2 bytes)                                               |  |
|  +-------------------------------------------------------------------------+  |
|  | Variable-Length UTF-8 File Name (name_length_bytes)                     |  |
|  +-------------------------------------------------------------------------+  |
+-------------------------------------------------------------------------------+
```

- **Explicit Little-Endian Encoding**: All integer fields are explicitly decoded as little-endian (`u64::from_le_bytes`, `u32::from_le_bytes`), eliminating compiler-dependent padding or endianness issues.
- **Zero-Allocation Decoding**: The session host deserializes records into contiguous arena buffers without intermediate heap allocations per file record.

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
| **Named Pipe Squatting** | Malicious local process creates `\\\\.\\pipe\\pigtree-session-...` before PigTree starts. | Client connects to rogue server; sensitive file paths and scan queries intercepted. | 1. `FILE_FLAG_FIRST_PIPE_INSTANCE` causes server creation to fail if pipe exists.<br>2. 256-bit unguessable CSPRNG nonce delivered over private bootstrap pipe.<br>3. Mutual PID & Process Creation Time verification via `GetNamedPipeClientProcessId` and `GetProcessTimes`. |
| **Cross-User Snooping** | Another standard user on multi-user Windows machine connects to PigTree Named Pipe. | Data leakage of scanned filesystem paths and metadata. | Strict SDDL DACL granting `GENERIC_READ | GENERIC_WRITE` **only** to active user SID (`(A;;GRGW;;;<UserSID>)`). Remote connections blocked via `PIPE_REJECT_REMOTE_CLIENTS`. |
| **Low-Integrity Sandbox Write Injection** | Compromised web browser or sandboxed app attempts to inject commands into session host. | Code execution or unauthorized filesystem analysis. | SACL Mandatory Integrity Label `S:(ML;;NW;;;ME)` (No-Write-Up) rejects all write attempts from Low IL processes. |
| **Raw Parser Memory Corruption** | Malformed or adversarial NTFS volume metadata triggers buffer overflow in raw MFT parser. | Potential arbitrary code execution with elevated privileges. | 1. Parser runs in separate child process spawned via `CreateProcessAsUserW` under **Restricted Token** (`DISABLE_MAX_PRIVILEGE`, `LUA_TOKEN`, Low IL).<br>2. Zero write privileges, read-only duplicated volume handle.<br>3. Job Object memory limits + Watchdog timer kill parser on crash without affecting broker. |
| **Oversized Message / OOM DoS** | Rogue or buggy worker emits multi-gigabyte frame header to exhaust host memory. | Host process Out-Of-Memory (OOM) crash and denial of service. | Hard 4 MiB frame length cap enforced at frame reader level before payload buffer allocation. Connection dropped immediately on violation. |
| **Worker Process Hang / Deadlock** | Worker thread hangs in uninterruptible kernel I/O wait (e.g. failing drive sector). | UI freezes, scan never finishes or cancels. | Asynchronous watchdog heartbeat + inherited manual-reset cancellation event with 500 ms deadline, followed by Job Object termination. |
| **Handle Leakage Privilege Escalation** | Child worker process inherits sensitive parent handles (tokens, write file handles). | Sandboxed child accesses unauthorized parent resources. | `PROC_THREAD_ATTRIBUTE_HANDLE_LIST` with `bInheritHandles = TRUE` explicitly whitelists only the required pipe and cancellation handles during process creation. |

---

## 8. Rejected Alternatives & Technical Trade-offs

| Candidate Alternative | Reason for Rejection |
| :--- | :--- |
| **gRPC over HTTP/2 (Loopback TCP)** | Rejected due to high memory/binary footprint, multi-user port collision conflicts, Windows Defender / Third-party firewall popup warnings on localhost binding, and lack of native Windows Security Identifier (SID) DACL enforcement. |
| **Named Pipe Message Mode (`PIPE_TYPE_MESSAGE`)** | Rejected because message-mode named pipes have complex asynchronous cancellation semantics in Tokio on Windows, whereas length-delimited Byte Mode (`PIPE_TYPE_BYTE`) provides superior reliability, portable framing, and predictable buffer control. |
| **Raw ALPC (`NtAlpc*` System Calls)** | Rejected because ALPC is an undocumented, private Windows NT kernel interface subject to breaking internal changes across Windows updates, offering no documented Rust or .NET SDK bindings. |
| **Shared Memory Section Objects (`CreateFileMappingW` + Ring Buffers)** | Rejected because modeled 170k observations/s requires only ~22 MB/s bandwidth (easily handled by pipes at >1,500 MB/s synthetic capacity). Shared memory introduces severe synchronization complexity, concurrency race bugs, and TOCTOU pointer validation vulnerabilities across untrusted parser boundaries. |
| **In-Process C++/CLI or Native DLL Interop (P/Invoke / C-ABI)** | Rejected because running the entire engine inside the WPF process violates process isolation, risks bringing down the UI on worker crashes, and prevents clean standard-user vs. elevated privilege separation. |
| **Payload Compression (LZ4 / Zstd) on Local IPC** | Rejected because local inter-process bandwidth is abundant; compressing metadata batches wastes CPU cycles and complicates framing without providing latency or memory benefits. |

---

## 9. Release Verification Gates & Test Strategy

To guarantee the reliability, security, and performance of the IPC subsystem, the following automated release gates must pass before production deployment:

1. **Empirical IPC Throughput Benchmark Gate**:
   - Automated benchmark simulating continuous emission of `5,000,000` directory entry observations over Named Pipes and Anonymous Pipes on the reference test harness.
   - **Pass Criterion**: End-to-end throughput must achieve **>= 170,000 observations/second** on the reference test system, validating modeled requirements.
2. **Security DACL & Multi-User Isolation Gate**:
   - Automated integration test running under secondary test credentials attempting to connect to an active PigTree Named Pipe.
   - **Pass Criterion**: Connection must be rejected by the Windows kernel with `ERROR_ACCESS_DENIED`.
3. **Mandatory Integrity Level Gate**:
   - Automated test spawning a Low Integrity test client (`ConvertStringSidToSidW` with Low IL) attempting to write to the Session Host pipe.
   - **Pass Criterion**: Write access denied by kernel NPFS driver (`ERROR_ACCESS_DENIED`).
4. **Fuzzing & Malformed Frame Resilience Gate**:
   - Structure-aware fuzzing (via `cargo fuzz` / libFuzzer) feeding mutated wire frames, truncated payloads, corrupted CRC-32C checksums, and oversized length headers (> 4 MiB) into the frame decoder.
   - **Pass Criterion**: 100% fail-closed rejection without crashes, memory leaks, or unhandled panics.
5. **Parser Containment & Sandbox Escape Gate**:
   - Test harness simulating a hard crash (`abort()`, null pointer dereference, and infinite loop) inside the Restricted Raw-Parser Child.
   - **Pass Criterion**: Elevated Broker detects child termination, terminates Job Object within < 100 ms, logs failure, and executes seamless fallback to elevated Win32 directory traversal.
6. **Cancellation Latency Soak Gate**:
   - Stress test signaling the inherited cancellation event during maximum-throughput pipe streaming.
   - **Pass Criterion**: Worker process cleanly terminates, Job Objects close, and the engine transitions to `Settled(Cancelled)` within **< 500 ms** in 100% of iterations.

---

## 10. Primary Source Citations & References

1. **Microsoft Windows Win32 API Documentation**:
   - *CreateNamedPipeW function*: [Microsoft Learn - CreateNamedPipeW](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-createnamedpipew) (Details `FILE_FLAG_FIRST_PIPE_INSTANCE`, `PIPE_REJECT_REMOTE_CLIENTS`, `PIPE_ACCESS_DUPLEX`, `PIPE_TYPE_BYTE`).
   - *GetNamedPipeClientProcessId function*: [Microsoft Learn - GetNamedPipeClientProcessId](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-getnamedpipeclientprocessid).
   - *GetNamedPipeClientSessionId function*: [Microsoft Learn - GetNamedPipeClientSessionId](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-getnamedpipeclientsessionid).
   - *UpdateProcThreadAttribute function & PROC_THREAD_ATTRIBUTE_HANDLE_LIST*: [Microsoft Learn - UpdateProcThreadAttribute](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-updateprocthreadattribute).
   - *CreateProcessAsUserW function*: [Microsoft Learn - CreateProcessAsUserW](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-createprocessasuserw).
   - *CreateRestrictedToken function*: [Microsoft Learn - CreateRestrictedToken](https://learn.microsoft.com/en-us/windows/win32/api/securitybaseapi/nf-securitybaseapi-createrestrictedtoken) (Details `DISABLE_MAX_PRIVILEGE`, `LUA_TOKEN`, restricted SIDs).
   - *Security Descriptor Definition Language (SDDL) for Mandatory Labels*: [Microsoft Learn - Mandatory Integrity Labels in SDDL](https://learn.microsoft.com/en-us/windows/win32/secauthz/mandatory-integrity-control).
   - *Job Objects & Limits*: [Microsoft Learn - Job Objects](https://learn.microsoft.com/en-us/windows/win32/procthread/job-objects) (`JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`, `SetInformationJobObject`).
   - *GetProcessTimes & Process Security*: [Microsoft Learn - GetProcessTimes](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-getprocesstimes).

2. **Microsoft .NET Documentation**:
   - *System.IO.Pipes.NamedPipeClientStream*: [Microsoft Learn - NamedPipeClientStream Class](https://learn.microsoft.com/en-us/dotnet/api/system.io.pipes.namedpipeclientstream).
   - *System.IO.Pipes.NamedPipeServerStream*: [Microsoft Learn - NamedPipeServerStream Class](https://learn.microsoft.com/en-us/dotnet/api/system.io.pipes.namedpipeserverstream).
   - *System.Threading.Channels*: [Microsoft Learn - System.Threading.Channels](https://learn.microsoft.com/en-us/dotnet/api/system.threading.channels).

3. **Rust Tokio & Windows Ecosystem Documentation**:
   - *tokio::net::windows::named_pipe*: [Tokio Documentation - Named Pipe ServerOptions & ClientOptions](https://docs.rs/tokio/latest/tokio/net/windows/named_pipe/struct.ServerOptions.html).
   - *tokio_util::codec::LengthDelimitedCodec*: [Tokio Util Documentation - Codecs](https://docs.rs/tokio-util/latest/tokio_util/codec/struct.LengthDelimitedCodec.html).
   - *windows-sys / windows crate*: [Microsoft Windows crate for Rust](https://crates.io/crates/windows-sys).

4. **Protocol Buffers & Checksum Specifications**:
   - *Protocol Buffers v3 Language Guide*: [Protobuf Language Guide (proto3)](https://protobuf.dev/programming-guides/proto3/).
   - *Prost Protocol Buffers for Rust*: [Prost Documentation](https://docs.rs/prost/latest/prost/).
   - *RFC 3720 / CRC-32C Specification*: [IETF RFC 3720 - CRC-32C Castagnoli Polynomial](https://datatracker.ietf.org/doc/html/rfc3720).

5. **PigTree Repository Architecture Contracts**:
   - *ADR 0001: Scanning Subsystem and Privilege Architecture*: `docs/adr/0001-scanning-and-privilege-architecture.md`
   - *ADR 0002: Guarded Cleanup and Action Safety Architecture*: `docs/adr/0002-guarded-cleanup-safety.md`
   - *ADR 0003: Shared Engine and Automation Contract*: `docs/adr/0003-shared-engine-and-automation-contract.md`
   - *Product Performance Targets & Acceptance Budgets*: `docs/performance-targets.md`
