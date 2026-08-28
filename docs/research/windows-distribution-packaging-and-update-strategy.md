# Research Note: Windows 10/11 Distribution, Packaging, and Update Strategy for WPF and Rust

**Ticket Prerequisite:** [AFlyingP/PigTree#14 - Select the production technology architecture](https://github.com/AFlyingP/PigTree/issues/14)  
**Scope:** Windows 10 and 11 (x64) Open-Source Distribution, .NET Runtime Packaging, Multi-Process Executable Topologies, Elevation & Security Boundaries, Shell Integration, Code Signing, and Deterministic Packaging.  
**Date:** March 2025 (Revised)  
**Author:** DeepSeek Harness Research Subagent  

---

## 1. Executive Summary: Facts vs. Architectural Recommendations

To establish a production-ready Windows distribution and update model for PigTree, this research delineates **immutable Windows platform facts** from **recommended architectural choices** for the v1 release.

PigTree combines a **WPF front-end GUI on the latest supported .NET Long Term Support (LTS) release at implementation/release time**, a **private medium-integrity Rust core engine / session-host**, **disposable read-only scanning workers**, a **short-lived elevated read-only broker with isolated raw parser**, a **dedicated short-lived mutation worker**, and a **standalone Rust CLI**.

```
 +-----------------------------+     +-----------------------------+
 |   PigTree.exe (WPF GUI)     |     |      pigtree.exe (CLI)      |
 |   Medium Integrity (asInvoker)|   |   Medium Integrity (asInvoker)|
 +--------------+--------------+     +--------------+--------------+
                |                                   |
                +-----------------+-----------------+
                                  |
                                  v
                +-----------------------------------+
                |      pigtree-engine.exe           |
                |  Medium-Integrity Session Host    |
                |  - Query Engine & Graph Store     |
                |  - Typed Challenge Protocol       |
                |  - Worker Process Supervisor      |
                +-----------------+-----------------+
                                  |
       +--------------------------+--------------------------+
       | (Medium Integrity)       | (Elevated UAC / runas)   | (Guarded Action Plan)
       v                          v                          v
+----------------------+   +-----------------------+   +------------------------+
|pigtree-scan-worker.exe|  |pigtree-elevated-broker|   |pigtree-mutation-worker |
| - Win32 Directory    |   | - Read-Only Vol Handle|   | - Live Preflight       |
|   Traversal          |   | - Watchdog Supervisor |   | - Commit Point Gates   |
| - Standard User Token|   +-----------+-----------+   | - Immutable Exec Record|
+----------------------+               | (Restricted   +------------------------+
                                       |  Sandbox Pipe)
                                       v
                           +-----------------------+
                           | pigtree-raw-parser.exe|
                           | - Raw MFT Clust Parser|
                           | - Restricted Token    |
                           | - Fail-Closed Invariants
                           +-----------------------+
```

### 1.1 Objective Platform Facts (Windows & .NET Runtime Realities)
1. **WPF Native AOT Incompatibility:** WPF is fundamentally incompatible with Native AOT in supported .NET LTS releases. The framework relies inherently on unannotated runtime reflection, dynamic dependency property registration, runtime BAML parsing, and unmanaged C++/CLI shims (`wpfgfx_cor3.dll`, `DirectWriteForwarder`), triggering pervasive trimming warnings (`IL2026`, `IL3050`) and runtime crashes.
2. **Single-File Self-Extract Overhead:** WPF contains unmanaged C++ assemblies that cannot be loaded purely in-memory from bundle headers. Using `.NET Single-File` with native extraction forces runtime unpacking to `%TEMP%\net\*`, adding cold-start latency, triggering AV heuristics, and leaving disk litter.
3. **User Interface Privilege Isolation (UIPI):** An elevated (High Integrity) GUI cannot receive unprivileged Windows messages from File Explorer (breaking drag-and-drop) or standard accessibility tools (screen readers).
4. **MSIX Signature Gate:** MSIX packages refuse installation on Windows unless signed by a certificate chaining to the local machine's Trusted Root Certification Authorities store, blocking unmanaged open-source distribution without pre-installed root certs or Developer Mode sideloading.
5. **Windows In-Use File Locking & Multi-File Atomicity:** Executing binaries (`.exe`, `.dll`) hold memory-mapped image handles with `FILE_SHARE_READ`, causing direct overwrite or deletion to fail with `ERROR_ACCESS_DENIED` (5) or `ERROR_SHARING_VIOLATION` (32). Swapping a directory of multiple loose binaries in-place cannot be executed atomically at the individual file level.
6. **Command-Line Exposure:** Process command-line arguments are visible across the logon session via WMI (`Win32_Process`), ETW tracing, and process inspection tools. Passing secrets via command-line arguments is insecure regardless of Named Pipe DACLs.

### 1.2 Architectural Recommendations for PigTree v1
1. **Self-Contained ReadyToRun (R2R) Multi-File Layout:** Ship the latest supported .NET LTS at implementation/release time as self-contained with R2R ahead-of-time precompilation, distributing assemblies and native binaries loose in the application directory without single-file bundling or Native AOT.
2. **Dual-Channel Distribution (Portable-First + Per-User MSI):**
   * *Portable ZIP:* First-class, zero-install, zero-registry archive targetable by power users, sysadmins, and incident responders. For v1, updates are download-only / manual replacement with release notification and checksum/signature verification (or an atomic versioned-directory pointer switch).
   * *WiX v5 MSI:* Default Per-User installer (`Scope="perUser"`, `%LocalAppData%\Programs\PigTree`) requiring **zero UAC prompts** for installation or update, with transactional `MajorUpgrade` rollback and optional Per-Machine enterprise deployment (`Scope="perMachine"`).
3. **Typed Challenge Elevation Architecture (Strict ADR 0001/0002/0003 & IPC Conformance):**
   * The WPF GUI and CLI speak **only** to the medium-integrity Engine via typed IPC.
   * When elevated scanning is required, the Engine issues a typed `ScanChallenge`. Upon user consent, the Engine coordinates launching `pigtree-elevated-broker.exe` via `ShellExecuteExW` (`lpVerb = L"runas"`, `fMask = SEE_MASK_NOCLOSEPROCESS`), retaining the process handle `hProcess`.
   * **Zero Command-Line Secrets:** Pipe endpoint path, broker role, session identifier, target volume identifier, and immutable plan digest on the command line are strictly non-secret routing/context metadata. Authoritative bootstrap, token validation, PID/creation-time verification, and proof protocols are owned exclusively by the IPC architecture.
   * The elevated broker is strictly read-only, connecting back *only* to the Engine's authenticated private IPC endpoint.
   * The elevated broker spawns `pigtree-raw-parser.exe` in an isolated restricted child process with watchdog monitoring; any parsing fault triggers immediate fail-closed fallback to elevated documented Win32 traversal.
   * File remediation/cleanup is handled exclusively by a dedicated `pigtree-mutation-worker.exe` with Live Preflight and Commit Point verification; **no scanner/mutator reuse is permitted**.
4. **Hybrid Shell Integration:** Classic HKCU registry verbs for universal Windows 10/11 context menus, with an optional Sparse Package (Package with External Location via `Add-AppxPackage -Register <Manifest> -ExternalLocation <AppDir>`) for top-level Windows 11 context menus.
5. **Code Signing:** Authenticode signing via Azure Trusted Signing or SignPath.io for CI, backed by public SHA-256 checksums and minisign/GPG signatures.

---

## 2. Comparative Evaluation Matrix: Packaging & Distribution Modalities

| Dimension | Self-Contained + R2R (.NET LTS Multi-File) | Framework-Dependent .NET | Native AOT (WPF) | MSIX Packaging | WiX Toolset v5 (MSI) | Portable ZIP |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| **Prerequisites** | Zero (Runs on clean Win10/11) | Requires .NET Desktop Runtime | Zero (Direct native binary) | Requires trusted cert or Dev Mode | Zero (Windows Installer in-box) | Zero (Unpack & run) |
| **WPF Compatibility** | **100% Fully Supported** | **100% Fully Supported** | **Unsupported / Broken** | Fully Supported | Fully Supported | Fully Supported |
| **Startup Latency** | Fast (R2R native precompilation) | Moderate (JIT overhead on launch) | Instant (Native machine code) | Fast | N/A (Installer) | Fast (R2R) |
| **Disk / Download Footprint**| ~65–85 MB uncompressed | ~15–25 MB (Requires ~60MB runtime) | ~25–35 MB (Theoretical) | ~70–90 MB | ~35–45 MB (Compressed MSI) | ~35–45 MB (Compressed ZIP)|
| **Elevation & IPC Topology** | Complete (Unrestricted Win32) | Complete (Unrestricted Win32) | Complete (Unrestricted Win32) | Restricted (AppContainer/VFS) | Complete (Standard Win32) | Complete (Unrestricted Win32)|
| **UAC Elevation Required** | None (Runs as Standard User) | None (Runs as Standard User) | None (Runs as Standard User) | None (Per-User AppX) | **None** (for Per-User MSI) | **None** |
| **Silent Enterprise Install**| Scripted copy | Scripted copy | Scripted copy | Intune / AppInstaller | **Native MSI (`/qn` flags)** | Scripted unpack |
| **Update Mechanism** | In-App / WiX MajorUpgrade | In-App / WiX MajorUpgrade | In-App / WiX MajorUpgrade | AppInstaller / Store | **Windows Installer MajorUpgrade** | Download-Only / Manual or Versioned Switch |
| **Reproducible Build Feasible**| **Yes (Full MSBuild/Cargo CI)**| **Yes** | **Yes** | Moderate (Appx manifest/zip) | **Yes (WiX deterministic GUIDs)** | **Yes (Normalized timestamps)** |

---

## 3. Deep-Dive Technology Analysis & Primary Sources

### 3.1 .NET Deployment Strategy: Framework-Dependent vs. Self-Contained vs. Native AOT

#### Primary Sources
* [Microsoft Learn: .NET Application Publishing Overview](https://learn.microsoft.com/en-us/dotnet/core/deploying/)
* [Microsoft Learn: ReadyToRun Compilation](https://learn.microsoft.com/en-us/dotnet/core/deploying/ready-to-run)
* [Microsoft Learn: Native AOT Deployment](https://learn.microsoft.com/en-us/dotnet/core/deploying/native-aot/)
* [Microsoft Learn: Warnings for Trimming and AOT (IL2026, IL3050)](https://learn.microsoft.com/en-us/dotnet/core/deploying/trimming/trim-warnings/il2026)
* [dotnet/wpf Issue #11205: Native AOT Support for WPF](https://github.com/dotnet/wpf/issues/11205)
* [dotnet/wpf Issue #5909: Trimming and WPF Compatibility](https://github.com/dotnet/wpf/issues/5909)
* [Microsoft Learn: Single-File Deployment and Executable Bundling](https://learn.microsoft.com/en-us/dotnet/core/deploying/single-file/overview)

#### Technical Findings

1. **Targeting Supported .NET LTS Releases:**
   * PigTree specifies targeting the **latest supported .NET Long Term Support (LTS) release at implementation/release time** (e.g., .NET 8 / .NET 10 LTS).
   * *Framework-Dependent Deployment:* Relies on the host system possessing the matching **Microsoft.WindowsDesktop.App** runtime. Missing runtimes generate error dialogs (`0x80008081` / missing `hostfxr.dll`), imposing unforced friction on ad-hoc system administration.
   * *Self-Contained Deployment:* Bundles the CoreCLR runtime engine (`coreclr.dll`, `clrjit.dll`), BCL assemblies, and native WPF rendering subsystems (`wpfgfx_cor3.dll`, `PresentationNative_cor3.dll`, `D3DCompiler_47_cor3.dll`). Guarantees zero-prerequisite execution across all supported Windows 10 (Build 19041+) and Windows 11 environments.

2. **ReadyToRun (R2R) Compilation:**
   * Crossgen2 compiles IL assemblies ahead-of-time into ReadyToRun PE images containing dual native x64 machine code and IL fallback.
   * *Performance Impact:* Decreases cold-startup JIT compilation time by 30%–50%, ensuring near-instant GUI initialization when launched during emergency low-disk-space scenarios.
   * *Binary Footprint:* Increases assembly size by ~20%–30%, easily accommodated within modern distribution limits.

3. **Native AOT Infeasibility for WPF:**
   * Across modern .NET LTS versions, **WPF is fundamentally unsupported for Native AOT and full IL trimming**.
   * *Root Cause Analysis:*
     * **Dynamic Reflection & Binding:** WPF data binding and template instantiations rely on dynamic reflection (`System.ComponentModel.TypeDescriptor`, `PropertyDescriptor`) without static Roslyn trim annotations, generating hundreds of `IL2026` (`RequiresUnreferencedCode`) and `IL3050` (`RequiresDynamicCode`) compiler diagnostics.
     * **BAML Stream Loading:** The BAML runtime parser dynamically maps compiled token streams to runtime types and properties.
     * **Dependency Property System:** Dynamic `DependencyProperty.Register` and `OverrideMetadata` mechanisms do not expose static type dependencies to IL linkers.
     * **Unmanaged C++/CLI Code:** Core subsystems (e.g., `DirectWriteForwarder.dll`) bridge native DirectX/DirectWrite to managed code via C++/CLI, which is unsupported under the Native AOT toolchain.
   * *Conclusion:* Native AOT cannot be used for WPF. ReadyToRun self-contained deployment is the official, supported path.

4. **Single-File Bundling vs. Multi-File Clean Directory Layout:**
   * While managed assemblies can execute in-memory from a single-file bundle header, native WPF binaries (`wpfgfx_cor3.dll`, `PresentationNative_cor3.dll`) cannot. Setting `IncludeNativeLibrariesForSelfExtract=true` forces extraction to `%TEMP%\net\<app>\<id>\` on first run.
   * *Consequences:* Cold-start delays (200–500ms), AV minifilter scan friction, and orphaned temp files.
   * *Recommendation:* Multi-file self-contained directory layout without single-file bundling, co-locating all UI and engine executables with shared runtime DLLs in one clean directory.

---

### 3.2 Multi-Process Executable Topology & Elevation Architecture

#### Primary Sources
* [Microsoft Learn: User Interface Privilege Isolation (UIPI) Overview](https://learn.microsoft.com/en-us/windows/win32/winmsg/about-messages-and-message-queues#user-interface-privilege-isolation)
* [Microsoft Learn: Designing Applications for UAC and Least Privilege](https://learn.microsoft.com/en-us/windows/win32/sbscs/application-manifests)
* [Microsoft Learn: ShellExecuteExW & SEE_MASK_NOCLOSEPROCESS](https://learn.microsoft.com/en-us/windows/win32/api/shellapi/nf-shellapi-shellexecuteexw)
* [Microsoft Learn: Named Pipe Security and Access Rights](https://learn.microsoft.com/en-us/windows/win32/ipc/named-pipe-security-and-access-rights)
* [PigTree ADR 0001: Scanning Subsystem and Privilege Architecture](docs/adr/0001-scanning-and-privilege-architecture.md)
* [PigTree ADR 0002: Guarded Cleanup and Action Safety Architecture](docs/adr/0002-guarded-cleanup-safety.md)
* [PigTree ADR 0003: Shared Engine and Automation Contract](docs/adr/0003-shared-engine-and-automation-contract.md)
* [PigTree Research Note: Windows Local IPC Transport, Framing, and Identity Design](docs/research/windows-ipc-transport-framing-identity.md)

#### Strict Security & Elevation Protocol

```
+-------------------------------------------------------------------------------+
| FRONT-END CLIENTS (Medium Integrity, asInvoker)                               |
| - PigTree.exe (WPF GUI): XAML, TreeTable, Treemap, Accessibility Automation    |
| - pigtree.exe (Rust CLI): Headless audits, JSON stream, automation pipelines   |
+---------------------------------------+---------------------------------------+
                                        | Typed Local IPC (Framed JSON-RPC / Protobuf)
                                        v
+-------------------------------------------------------------------------------+
| SHARED ENGINE & SESSION HOST (pigtree-engine.exe)                             |
| - Medium Integrity (Standard User, asInvoker)                                 |
| - Central Scan Planner, Snapshot Store, Graph Aggregator, Query Algebra       |
| - Security Authority: Issues Typed Challenges, Supervises Worker Lifecycles  |
+-------------------+-------------------+-------------------+-------------------+
                    |                   |                   |
 (1) Standard Scan  | (2) Elevation     | (3) Guarded       |
     Worker Spawn   |     Challenge     |     Action Plan   |
                    v     & Broker      v     Execution     v
+-----------------------+ +-----------------------+ +-------------------------+
|pigtree-scan-worker.exe| |pigtree-elevated-broker| |pigtree-mutation-worker  |
|- Medium Integrity     | |- High Integrity       | |- Medium/Elevated per-task|
|- Disposable Worker    | |- Read-Only Vol Handle | |- Live Preflight Checks  |
|- Win32 Traversal      | |- Watchdog Supervisor  | |- Commit Point Gates     |
|- Target Directory     | |- Non-Secret Routing CLI|- Immutable Exec Records |
+-----------------------+ +-----------+-----------+ +-------------------------+
                                      | (4) Isolated Restricted Child Pipe
                                      v
                          +-----------------------+
                          | pigtree-raw-parser.exe|
                          | - Disposable Child    |
                          | - Restricted Token    |
                          | - MFT Extent Parsing  |
                          | - Fail-Closed Invariants
                          +-----------------------+
```

#### Detailed Process Roles & Security Lifecycle

1. **Client Layer (`PigTree.exe` & `pigtree.exe`):**
   * Both frontend applications run strictly at **Standard User (Medium Integrity)**.
   * `PigTree.exe` specifies `<requestedExecutionLevel level="asInvoker" uiAccess="false" />`. It never runs elevated, preventing UIPI drag-and-drop blocking, screen-reader isolation, and massive UI-surface elevation vulnerabilities.
   * The GUI communicates **exclusively** with `pigtree-engine.exe`. The GUI **never directly spawns or commands an elevated process**.

2. **Shared Engine Daemon (`pigtree-engine.exe`):**
   * Medium-integrity process acting as the central coordination point, state store, and security challenge issuer per ADR 0003.
   * Supervises all worker lifecycles, authenticates IPC handshakes, validates Scan Plans, and applies fail-closed policies.

3. **Standard Scan Worker (`pigtree-scan-worker.exe`):**
   * Disposable medium-integrity process spawned by the Engine for standard unprivileged scans (Win32 directory enumeration and batched handle queries).

4. **Privileged Scan Flow & Elevated Broker (`pigtree-elevated-broker.exe`):**
   * When an NTFS whole-volume scan requires elevation (or protected paths produce Coverage Gaps), the Engine returns a typed `ScanChallenge` to the client.
   * Upon explicit user confirmation (or `--elevated` CLI flag), the unelevated Engine (or an unelevated OS launcher shim) invokes Win32 `ShellExecuteExW` with `lpVerb = L"runas"` and `fMask = SEE_MASK_NOCLOSEPROCESS` targeting `pigtree-elevated-broker.exe`, retaining the returned `hProcess` handle.
   * **Zero Command-Line Secrets:** Command-line parameters pass only non-secret routing and contextual identifiers: the target Named Pipe endpoint path, broker role, session GUID, target volume GUID, and immutable Scan Plan digest. No secrets or tokens are placed in command-line arguments, as command lines are visible system-wide via WMI (`Win32_Process`) and ETW.
   * **Authoritative Bootstrap & IPC Authentication:** Owned exclusively by the IPC architecture (see `docs/research/windows-ipc-transport-framing-identity.md`).
     * The Engine creates the Named Pipe instance with `FILE_FLAG_FIRST_PIPE_INSTANCE`, `PIPE_REJECT_REMOTE_CLIENTS`, and an SDDL Security Descriptor.
     * When the broker connects, the Engine verifies the client process ID (`GetNamedPipeClientProcessId`), session ID (`GetNamedPipeClientSessionId`), and process creation timestamp (`GetProcessTimes`) matching the exact `hProcess` handle retained from `ShellExecuteExW` (binding the connection to the launched instance and defeating PID-reuse attacks).
     * The Engine validates that the client token represents the expected user/admin elevation at High Integrity Level (`OpenProcessToken` verification).
     * The elevated broker reciprocally validates the server process identity before trusting it.
     * If the IPC architecture defines an out-of-band proof exchange, it occurs within the authenticated pipe stream.
   * **Strict Read-Only Enforcement:** The broker acquires `SE_MANAGE_VOLUME_NAME` / `SE_BACKUP_NAME`, opens a strictly **read-only volume handle** (`GENERIC_READ`, `FILE_SHARE_READ | FILE_SHARE_WRITE`), and acts as a supervisor. It cannot write, delete, alter ACLs, or dismount volumes.

5. **Restricted Raw Parser Child & Watchdog (`pigtree-raw-parser.exe`):**
   * The elevated broker duplicates the read-only volume handle to an isolated, disposable child process (`pigtree-raw-parser.exe`) executing under a restricted token where supported.
   * The raw parser unpacks MFT records, enforces acyclic directory hierarchies, and verifies structural invariants per ADR 0001.
   * **Watchdog & Fallback Boundary:** The elevated broker acts as a watchdog. Any crash, hang, out-of-bounds read, or invariant violation in `pigtree-raw-parser.exe` causes the broker to immediately terminate the child, abort the raw path, and seamlessly fallback to elevated documented Win32 traversal over the read-only volume handle, logging the fallback event.

6. **Dedicated Mutation Worker (`pigtree-mutation-worker.exe`):**
   * **Strict Separation of Concerns:** In accordance with ADR 0001 and ADR 0002, **scanners and mutators are never combined**. Scanning workers cannot perform deletions, and mutation workers cannot perform disk discovery.
   * Invoked solely during Guarded Action Plan execution.
   * Executes **Live Preflight Verification** (re-verifying File ID, timestamps, attributes, and content hashes immediately before mutation) and enforces explicit **Commit Points** before executing Directory Entry deletions or hard link consolidations, emitting an immutable Execution Record.

---

### 3.3 Installer & Distribution Modalities: MSIX vs. WiX v5 (MSI) vs. Portable ZIP

#### Primary Sources
* [WiX Toolset v5 Official Documentation](https://wixtoolset.org/docs/intro/)
* [Microsoft Learn: MSIX Packaging Fundamentals](https://learn.microsoft.com/en-us/windows/msix/overview)
* [Microsoft Learn: Standard Installer Properties (ALLUSERS, MSIINSTALLPERUSER)](https://learn.microsoft.com/en-us/windows/win32/msi/msiinstallperuser)
* [Microsoft Learn: Package with External Location (Sparse Package)](https://learn.microsoft.com/en-us/windows/apps/desktop/modernize/grant-identity-to-non-packaged-apps)

#### Technical Comparison

1. **MSIX Limitations for Open-Source Systems Tools:**
   * *Mandatory Certificate Trust:* MSIX strictly mandates that the signing certificate chain to a trusted root in the OS store. Self-signed or open-source community certificates fail installation with `0x800B0109` ("Publisher cannot be verified") unless users run administrative scripts to install root certificates.
   * *Containerized Isolation:* Filesystem/registry virtualization complicates multi-process named pipe rendezvous and on-demand elevated worker execution.
   * *Portable Incompatibility:* Cannot be unzipped and run standalone from a USB toolkit.

2. **WiX Toolset v5 (MSI) Architectural Advantages:**
   * *Zero-Elevation Per-User Install:* Standard MSI properties `ALLUSERS=2` and `MSIINSTALLPERUSER=1` (or `<Package Scope="perUser">`) install to `%LocalAppData%\Programs\PigTree` with **zero UAC prompts**.
   * *Enterprise Per-Machine Support:* IT administrators can deploy globally via `msiexec /i PigTree.msi ALLUSERS=1 /qn` to `C:\Program Files\PigTree`.
   * *Transactional Upgrades & Rollback:* Utilizes Windows Installer's built-in `MajorUpgrade` engine, providing bit-level transactional rollback if an update fails mid-installation.
   * *Restart Manager Integration:* Integrates with Windows Restart Manager (`RmStartSession`) to gracefully coordinate shutting down active engine/UI processes during an update without forcing reboots.

3. **Portable ZIP (First-Class Citizen):**
   * Contains the complete self-contained folder layout with a marker configuration file (`portable.lock`).
   * Leaves zero registry traces, requires no installer execution, and supports fully functional standard and elevated scans on any USB or network path.

---

### 3.4 Shell Integration Architecture: Classic Registry vs. Windows 11 Sparse Package

#### Primary Sources
* [Microsoft Learn: Creating Context Menu Handlers (Win32 Shell)](https://learn.microsoft.com/en-us/windows/win32/shell/context-menu-handlers)
* [Microsoft Learn: Grant Identity to Non-Packaged Desktop Apps (Sparse Packages)](https://learn.microsoft.com/en-us/windows/apps/desktop/modernize/grant-identity-to-non-packaged-apps)
* [Microsoft Learn: AppxManifest with External Location](https://learn.microsoft.com/en-us/uwp/schemas/appxpackage/uapmanifestschema/element-uap10-allowexternalcontent)

#### Shell Registration Mechanics

1. **Classic Win32 Shell Verbs (Windows 10 & Windows 11 "Show More Options"):**
   * Registered under Current User hive for per-user installations (no elevation required):
     * `HKCU\Software\Classes\Directory\shell\PigTree` -> Default: `"Scan with PigTree"`, Icon: `"%LocalAppData%\Programs\PigTree\PigTree.exe,0"`
     * `HKCU\Software\Classes\Directory\shell\PigTree\command` -> Default: `""%LocalAppData%\Programs\PigTree\PigTree.exe" "%1""`
     * `HKCU\Software\Classes\Directory\Background\shell\PigTree\command` -> Default: `""%LocalAppData%\Programs\PigTree\PigTree.exe" "%V""`
     * `HKCU\Software\Classes\Drive\shell\PigTree\command` -> Default: `""%LocalAppData%\Programs\PigTree\PigTree.exe" "%1""`

2. **Windows 11 Modern Context Menu (Top-Level Integration):**
   * Windows 11 top-level context menus require **Package Identity** and an `IExplorerCommand` COM server.
   * *Sparse Package Layout:* PigTree includes an `AppxManifest.xml` containing `<uap10:AllowExternalContent>true</uap10:AllowExternalContent>` and declaring `desktop4:FileExplorerContextMenus`.
   * *Registration Command:* Registered dynamically without MSIX containerization via PowerShell / WinRT `PackageManager`:
     ```powershell
     Add-AppxPackage -Register "AppxManifest.xml" -ExternalLocation "C:\Users\<User>\AppData\Local\Programs\PigTree"
     ```

---

### 3.5 Code Signing, SmartScreen Reputation, and Open-Source Options

#### Primary Sources
* [Microsoft Learn: Microsoft Defender SmartScreen Overview](https://learn.microsoft.com/en-us/windows/security/operating-system-security/virus-and-threat-protection/microsoft-defender-smartscreen/)
* [Microsoft Learn: Trusted Signing Overview (formerly Azure Code Signing)](https://learn.microsoft.com/en-us/azure/trusted-signing/overview)
* [SignPath Foundation: Free Code Signing for Open Source Projects](https://signpath.org/about-foundation/)

#### Technical Realities of SmartScreen

1. **Reputation Dynamics:**
   * SmartScreen computes SHA-256 binary digests and inspects Authenticode certificate chains.
   * New certificates start with neutral reputation and accumulate trust as download telemetry demonstrates safety across the Windows install base.
2. **Recommended Signing Infrastructure:**
   * *Primary Option:* **SignPath Foundation** (free Authenticode signing for qualifying GitHub open-source projects via automated CI workflows).
   * *Secondary Option:* **Azure Trusted Signing** (Microsoft cloud-backed signing service, ~$9.99/month, natively integrated into `signtool.exe` via `Azure.CodeSigning.Dlib`).
   * *Verification & Trust Transparency:* Publish detached Minisign/GPG signatures and `SHA256SUMS.txt` alongside all GitHub Releases to ensure verifiable integrity for security-conscious users.

---

### 3.6 In-Use File Locking, Atomic Updates, and Rollback

#### Primary Sources
* [Microsoft Learn: Restart Manager Overview](https://learn.microsoft.com/en-us/windows/win32/rstmgr/about-restart-manager)
* [Microsoft Learn: MoveFileExW and Pending File Operations](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-movefileexw)

#### Evaluated Update Strategies & Multi-File Atomicity Realities

1. **The Multi-File In-Use Locking Barrier:**
   * Because PigTree consists of multiple executables (`PigTree.exe`, `pigtree-engine.exe`, `pigtree-scan-worker.exe`, `pigtree-elevated-broker.exe`, `pigtree-raw-parser.exe`, `pigtree-mutation-worker.exe`, `pigtree.exe`) and shared runtime DLLs, **an in-place update cannot atomically overwrite individual active files on disk**.
   * Attempting loose in-place file replacement while any worker or host process holds an open image handle fails with `ERROR_ACCESS_DENIED` (5) or `ERROR_SHARING_VIOLATION` (32), leaving the application in a corrupt, partially updated state if interrupted.

2. **Channel-Specific Update Architecture:**

| Distribution Channel | Recommended Update Mechanism | Atomicity & Rollback Guarantees | Assessment |
| :--- | :--- | :--- | :--- |
| **WiX / MSI Installed Channel** | Windows Installer `MajorUpgrade` + Restart Manager | **Full Transactional Rollback:** Handled atomically by MSI transaction boundaries; rolling back all files and registry keys if any step fails. | **Recommended for MSI Installs:** Native enterprise standard, silent `/qn` capable, zero UAC for per-user. |
| **Portable ZIP Channel (v1 Baseline)** | In-App Release Notification + Verified Download (Manual or Versioned Directory Swap) | **Download-Only / Manual Replace or Versioned Pointer:** In v1, the app alerts the user to updates with release notes and verified checksum links. For automated swapping, a versioned directory layout (`apps/v1.0.0/`, `apps/v1.0.1/`) with an atomic launcher / current-pointer switch is required. | **Recommended for Portable:** Prevents partial in-place file corruption; preserves zero-install portable boundaries. |

---

### 3.7 Deterministic and Reproducible Packaging Pipeline

#### Primary Sources
* [Microsoft Learn: Deterministic Builds in Roslyn / MSBuild](https://learn.microsoft.com/en-us/dotnet/csharp/language-reference/compiler-options/code-generation#deterministic)
* [Rust Compiler Documentation: Source Path Remapping (`--remap-path-prefix`)](https://doc.rust-lang.org/rustc/command-line-arguments.html#--remap-path-prefix)
* [Reproducible Builds: SOURCE_DATE_EPOCH Specification](https://reproducible-builds.org/docs/source-date-epoch/)
* [WiX Toolset: Deterministic Component ID and Cabinet Generation](https://wixtoolset.org/docs/tools/wixext/wixutil/)

#### Concrete Reproducibility Settings

1. **.NET (C# / WPF) Project Configuration (`PigTree.Wpf.csproj`):**
   ```xml
   <PropertyGroup>
     <Deterministic>true</Deterministic>
     <ContinuousIntegrationBuild>true</ContinuousIntegrationBuild>
     <PathMap>$(MSBuildProjectDirectory)=/_/</PathMap>
     <EmbedUntrackedSources>true</EmbedUntrackedSources>
   </PropertyGroup>
   ```
2. **Rust Configuration (`.cargo/config.toml`):**
   ```toml
   [build]
   rustflags = [
     "--remap-path-prefix", "/home/runner/work/PigTree=/src",
     "--remap-path-prefix", "C:\\Users\\runneradmin\\AppData\\Local\\Temp=/tmp"
   ]
   ```
3. **Artifact Normalization:**
   * Pinned dependencies via `Cargo.lock` and standard `.NET` package lock files.
   * Set fixed archive timestamps via `SOURCE_DATE_EPOCH` for all portable `.zip` packages.
   * Use deterministic GUID generation in WiX v5 (`Guid="*"` with standard hashing algorithms).

---

## 4. Concrete Physical Layout on Disk

When installed to `%LocalAppData%\Programs\PigTree` or extracted from a portable ZIP, the directory structure is organized with explicit process-file separation:

```
PigTree/
├── PigTree.exe                    # WPF GUI Entry Point (Medium Integrity, asInvoker)
├── PigTree.dll                    # WPF Managed Application Assemblies
├── PigTree.runtimeconfig.json      # .NET CoreCLR Runtime Configuration
│
├── pigtree-engine.exe             # Rust Engine & Session Host (Medium Integrity)
├── pigtree-scan-worker.exe        # Rust Unprivileged Scan Worker (Medium Integrity)
├── pigtree-elevated-broker.exe    # Rust Short-Lived Elevated Read-Only Broker (High Integrity)
├── pigtree-raw-parser.exe         # Rust Restricted MFT Parser Child (Restricted Token)
├── pigtree-mutation-worker.exe    # Rust Guarded Mutation Worker (Live Preflight & Commit Points)
├── pigtree.exe                    # Standalone Rust CLI Entry Point (Medium Integrity)
│
├── coreclr.dll                    # .NET Runtime Engine (Latest Supported LTS)
├── clrjit.dll                     # .NET JIT Compiler
├── System.Private.CoreLib.dll     # Core Base Class Library
├── PresentationFramework.dll      # WPF Presentation Framework (R2R compiled)
├── PresentationCore.dll           # WPF Presentation Core (R2R compiled)
├── WindowsBase.dll                # WPF Windows Base (R2R compiled)
│
├── wpfgfx_cor3.dll                # WPF Native DirectX Rendering Subsystem
├── PresentationNative_cor3.dll    # WPF Native Interop Shim
├── D3DCompiler_47_cor3.dll        # Direct3D Shader Compiler
├── vcruntime140_cor3.dll          # VC++ Runtime Shim
│
├── AppxManifest.xml               # Sparse Package Manifest (Win11 Context Menu Identity)
├── PigTreeShellCommand.dll        # Unmanaged IExplorerCommand COM Stub (Win11 Context Menu)
├── assets/                        # Icons, Branding, and UI Resources
│   ├── pigtree.ico
│   └── logo.png
└── LICENSE.txt
```

---

## 5. Rejected Alternatives & Technical Boundaries

1. **Rejected: Framework-Dependent .NET Deployment**
   * *Reason:* Forces users to manually resolve missing .NET Desktop Runtime dependencies, breaking zero-friction system diagnostic utility expectations.
2. **Rejected: Native AOT Compilation for WPF**
   * *Reason:* Unsupported in modern .NET LTS versions; pervasive dynamic reflection, runtime BAML parsing, and C++/CLI dependencies cause trimming failures (`IL2026`, `IL3050`) and runtime crashes.
3. **Rejected: Single-File Bundling with Self-Extraction**
   * *Reason:* Unpacking native WPF rendering binaries (`wpfgfx_cor3.dll`) to `%TEMP%\net\*` introduces cold-start latency, disk clutter, and AV false positives.
4. **Rejected: MSIX-Only Distribution**
   * *Reason:* Mandates trusted certificate store installation, blocks unprivileged zero-install portable workflows, and complicates high-integrity helper spawning.
5. **Rejected: Elevated (High Integrity) WPF GUI**
   * *Reason:* Violates Windows UIPI (blocks Explorer drag-and-drop), breaks accessibility tools, and exposes a massive UI rendering surface to privilege escalation attacks.
6. **Rejected: Unified / Reused Scanner-Mutator Worker**
   * *Reason:* Violates ADR 0001 and ADR 0002. Scanning is strictly read-only; mutation requires a distinct authorization lifecycle, Live Preflight verification, and Commit Point logging.
7. **Rejected: Command-Line Secret Passing**
   * *Reason:* Violates local IPC security architecture (`docs/research/windows-ipc-transport-framing-identity.md`). Command lines are visible via WMI/ETW; authentication and instance identity are verified via OS process handles (`SEE_MASK_NOCLOSEPROCESS`), PID/creation-time checks, and in-band IPC handshake.

---

## 6. Release Verification Gates

Before publishing an official release, the CI/CD pipeline enforces the following validation gates:

1. **Cross-Compilation & Test Gate:**
   * Full workspace test execution: `cargo test --workspace` and `dotnet test`.
2. **Deterministic Build Verification Gate:**
   * Multi-runner compilation producing bit-for-bit identical SHA-256 artifact checksums.
3. **Authenticode Code Signing Gate:**
   * All `.exe`, `.dll`, and `.msi` binaries signed with valid timestamp counter-signatures (`signtool verify /pa /v PigTree.exe`).
4. **Headless Smoke Test Gate:**
   * Automated PowerShell script validates silent MSI per-user install, verifies IPC handshake between GUI and Engine, confirms worker execution and watchdog fallback, tests mutation preflight gating, and verifies clean uninstallation.
5. **Antivirus & SmartScreen Gate:**
   * VirusTotal API scan ensuring 0/70 detection flags before release publication.

---

## 7. Primary Source Citations

1. **Microsoft Learn (.NET Application Deployment & Publishing):**  
   https://learn.microsoft.com/en-us/dotnet/core/deploying/
2. **Microsoft Learn (ReadyToRun Compilation Architecture):**  
   https://learn.microsoft.com/en-us/dotnet/core/deploying/ready-to-run
3. **Microsoft Learn (Native AOT Deployment & Limitations):**  
   https://learn.microsoft.com/en-us/dotnet/core/deploying/native-aot/
4. **Microsoft Learn (Warnings for Trimming and AOT - IL2026, IL3050):**  
   https://learn.microsoft.com/en-us/dotnet/core/deploying/trimming/trim-warnings/il2026
5. **Microsoft Learn (.NET Single-File Deployment Overview):**  
   https://learn.microsoft.com/en-us/dotnet/core/deploying/single-file/overview
6. **dotnet/wpf GitHub Repository (Native AOT & Trimming Support Status):**  
   https://github.com/dotnet/wpf/issues/11205  
   https://github.com/dotnet/wpf/issues/5909
7. **WiX Toolset v5 Documentation (Architecture, Scopes, and MajorUpgrade):**  
   https://wixtoolset.org/docs/intro/
8. **Microsoft Learn (Package with External Location / Sparse Packages):**  
   https://learn.microsoft.com/en-us/windows/apps/desktop/modernize/grant-identity-to-non-packaged-apps
9. **Microsoft Learn (Windows 11 Explorer Command & Modern Context Menus):**  
   https://learn.microsoft.com/en-us/windows/apps/desktop/modernize/grant-identity-to-non-packaged-apps
10. **Microsoft Learn (Trusted Signing / Azure Code Signing):**  
    https://learn.microsoft.com/en-us/azure/trusted-signing/overview
11. **SignPath Foundation (Free Code Signing for Open Source):**  
    https://signpath.org/about-foundation/
12. **Microsoft Learn (User Interface Privilege Isolation - UIPI):**  
    https://learn.microsoft.com/en-us/windows/win32/winmsg/about-messages-and-message-queues#user-interface-privilege-isolation
13. **Microsoft Learn (Windows Restart Manager Architecture):**  
    https://learn.microsoft.com/en-us/windows/win32/rstmgr/about-restart-manager
14. **Microsoft Learn (Deterministic Builds with MSBuild):**  
    https://learn.microsoft.com/en-us/dotnet/csharp/language-reference/compiler-options/code-generation#deterministic
15. **Reproducible Builds Organization (SOURCE_DATE_EPOCH Specification):**  
    https://reproducible-builds.org/docs/source-date-epoch/
16. **PigTree ADR 0001 (Scanning Subsystem and Privilege Architecture):**  
    [docs/adr/0001-scanning-and-privilege-architecture.md](docs/adr/0001-scanning-and-privilege-architecture.md)
17. **PigTree ADR 0002 (Guarded Cleanup and Action Safety Architecture):**  
    [docs/adr/0002-guarded-cleanup-safety.md](docs/adr/0002-guarded-cleanup-safety.md)
18. **PigTree ADR 0003 (Shared Engine and Automation Contract):**  
    [docs/adr/0003-shared-engine-and-automation-contract.md](docs/adr/0003-shared-engine-and-automation-contract.md)
19. **PigTree Research Note (Windows Local IPC Transport, Framing, and Identity Design):**  
    [docs/research/windows-ipc-transport-framing-identity.md](docs/research/windows-ipc-transport-framing-identity.md)
