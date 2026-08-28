# Research Note: Windows 10/11 Distribution, Packaging, and Update Strategy for WPF and Rust

**Ticket Prerequisite:** [AFlyingP/PigTree#14 - Select the production technology architecture](https://github.com/AFlyingP/PigTree/issues/14)  
**Scope:** Windows 10 and 11 (x64) Open-Source Distribution, .NET Runtime Packaging, Multi-Process Binaries, Elevation Architecture, Shell Integration, Code Signing, and Deterministic Packaging.  
**Date:** March 2025  
**Author:** DeepSeek Harness Research Subagent  

---

## 1. Executive Summary & Decision-Ready Recommendation

PigTree's approved architecture combines a **WPF (.NET 8/9) front-end GUI** with a **Rust core engine/session-host**, **Rust high-speed scanner workers**, and a **Rust CLI**. Distributing this heterogeneous multi-process application on Windows 10 and Windows 11 (x64) as an open-source, high-performance tool requires balancing immediate out-of-the-box user friction, enterprise deployability, security boundaries, and reliable update semantics.

### Key Architectural Decisions (v1 Recommendation)

1. **Self-Contained ReadyToRun (R2R) Multi-File Deployment for WPF:**
   * Distribute .NET 8/9 as a **Self-Contained** application with **ReadyToRun (R2R)** ahead-of-time compilation.
   * **Do NOT use Native AOT** (officially unsupported and broken in WPF due to runtime reflection, BAML loading, and C++/CLI unmanaged dependencies).
   * **Do NOT use single-file bundle self-extract** (`PublishSingleFile` with `IncludeNativeLibrariesForSelfExtract=true`). Instead, use a clean multi-file directory layout containing the WPF executable, private Rust binaries, and native WPF/CLR DLLs loose in the application folder. This avoids cold-startup extraction delays, disk litter in `%TEMP%\net\*`, and antivirus false positives.
2. **Dual-Channel Distribution: First-Class Portable ZIP + WiX v5 Per-User MSI:**
   * **Portable ZIP (First-Class Citizen):** A standalone, zero-installation archive containing the complete self-contained binary folder. Targetable by power users, sysadmins, and incident responders running from USB drives or arbitrary directories without requiring administrator rights or registry writes.
   * **WiX Toolset v5 MSI (Standard Installer):** A clean Windows Installer (`.msi`) configured by default for **Per-User** installation (`Package Scope="perUser"`, installing to `%LocalAppData%\Programs\PigTree`). Requires **zero UAC elevation** for installation or updates. Supports optional **Per-Machine** installation (`Scope="perMachine"`, `Program Files`) for IT/enterprise deployment.
3. **Rejection of MSIX as Primary Distribution Format:**
   * MSIX is rejected for v1 open-source distribution because it enforces strict code signing certificate trust before installation (blocking sideloading for unmanaged open-source users unless developer mode is enabled), and introduces filesystem/registry container virtualization that complicates high-integrity worker spawning and inter-process named pipe communication.
4. **Strict Process-File Separation & Elevation Boundaries:**
   * **Main GUI (`PigTree.exe`):** Runs strictly at **Standard User (Medium Integrity)**. It is never elevated.
   * **Engine / Session Host (`pigtree-engine.exe`):** Spun up on-demand as a private child process or local background service; communicates via local IPC.
   * **Scanner Worker (`pigtree-worker.exe`):** For unprivileged scans (Win32 / Batched Handle traversal), runs as Medium Integrity. For privileged operations (NTFS USN Journal, raw MFT, locked system paths), spawned on-demand via `ShellExecuteExW` with verb `"runas"` (High Integrity UAC prompt), performs the scan, streams data over a DACL-secured local Named Pipe, and immediately terminates.
   * **Command-Line Interface (`pigtree.exe`):** Unified standalone Rust CLI communicating directly with the engine or executing standalone queries.
5. **Universal Shell Integration Architecture:**
   * **Baseline (Windows 10 & 11 Classic):** Explorer context menu verbs ("Scan with PigTree") registered via non-elevated HKCU registry keys (`HKCU\Software\Classes\Directory\shell\PigTree`, `HKCU\Software\Classes\Drive\shell\PigTree`).
   * **Windows 11 Modern Context Menu:** Implemented via an optional **Sparse Package (Package with External Location)** declaring an `IExplorerCommand` COM class, registered dynamically via WinRT / PowerShell `Add-AppxPackage -Register`.
6. **Code Signing & SmartScreen Strategy:**
   * Authenticode signing of all binaries (`.exe`, `.dll`, `.msi`) via **Azure Trusted Signing** (or **SignPath.io** for open-source CI).
   * Transparent publishing of SHA-256 checksums and minisign/GPG signatures on GitHub Releases to mitigate initial SmartScreen reputation warm-up friction.
7. **Deterministic & Reproducible CI Pipeline:**
   * Bit-for-bit reproducible packaging using MSBuild `<Deterministic>true</Deterministic>` and `<ContinuousIntegrationBuild>true</ContinuousIntegrationBuild>`, Cargo `--remap-path-prefix`, WiX v5 deterministic component harvesting, and normalized zip timestamps.

---

## 2. Comparative Matrix: Distribution & Packaging Modalities

| Dimension | Self-Contained + R2R (Multi-File) | Framework-Dependent .NET | Native AOT (WPF) | MSIX Packaging | WiX Toolset v5 (MSI) | Portable ZIP |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| **Prerequisites** | Zero (Runs on clean Win10/11) | Requires .NET Desktop Runtime | Zero (Direct native binary) | Requires trusted cert or Dev Mode | Zero (Windows Installer in-box) | Zero (Unpack & run) |
| **WPF Compatibility** | **100% Fully Supported** | **100% Fully Supported** | **Unsupported / Broken** | Fully Supported | Fully Supported | Fully Supported |
| **Startup Latency** | Fast (R2R native precompilation) | Fast (JIT on startup) | Instant (Native machine code) | Fast | N/A (Installer) | Fast (R2R) |
| **Disk / Download Footprint**| ~65–85 MB uncompressed | ~15–25 MB (Requires ~60MB runtime) | ~25–35 MB (Theoretical) | ~70–90 MB | ~35–45 MB (Compressed MSI) | ~35–45 MB (Compressed ZIP)|
| **Elevation & IPC Flexibility**| Complete (Unrestricted Win32) | Complete (Unrestricted Win32) | Complete (Unrestricted Win32) | Restricted (VFS / AppContainer)| Complete (Standard Win32) | Complete (Unrestricted Win32)|
| **UAC Elevation Required** | None (Per-User) | None (Per-User) | None (Per-User) | None (Per-User) | None (for Per-User MSI) | **None** |
| **Silent Enterprise Install**| Via script | Via script | Via script | Intune / AppInstaller | **Native MSI (`/qn` flags)** | N/A (Scripted unzip) |
| **Auto-Update Mechanism** | Velopack / In-App Updater | Velopack / In-App Updater | Velopack / In-App Updater | AppInstaller / Store | Windows Installer MajorUpgrade | In-App Notification / Download|
| **Reproducible Build Feasible**| **Yes (Full MSBuild/Cargo CI)**| **Yes** | **Yes** | Moderate (Appx manifest/zip) | **Yes (WiX deterministic)** | **Yes (Normalized zip)** |

---

## 3. Deep-Dive Technology Analysis & Primary Sources

### 3.1 .NET Deployment Strategy: Framework-Dependent vs. Self-Contained vs. Native AOT

#### Primary Sources
* [Microsoft Learn: .NET Application Publishing Overview](https://learn.microsoft.com/en-us/dotnet/core/deploying/)
* [Microsoft Learn: ReadyToRun Compilation](https://learn.microsoft.com/en-us/dotnet/core/deploying/ready-to-run)
* [Microsoft Learn: Native AOT Deployment](https://learn.microsoft.com/en-us/dotnet/core/deploying/native-aot/)
* [dotnet/wpf Issue #11205: Native AOT Support for WPF](https://github.com/dotnet/wpf/issues/11205)
* [dotnet/wpf Issue #5909: Trimming and WPF Compatibility](https://github.com/dotnet/wpf/issues/5909)
* [Microsoft Learn: Single-File Deployment and Executable Bundling](https://learn.microsoft.com/en-us/dotnet/core/deploying/single-file/overview)

#### Technical Findings

1. **Framework-Dependent vs. Self-Contained:**
   * *Framework-Dependent:* Produces small binaries (~5–15 MB) but strictly depends on the target machine having the exact matching **Microsoft.WindowsDesktop.App** runtime installed. If missing, the user is greeted by a cryptic runtime download dialog or application crash (`0x80008081` / missing `hostfxr.dll`). In IT/enterprise environments and field diagnostics, assuming the latest .NET 8/9 Desktop Runtime is installed creates unforced customer support burden.
   * *Self-Contained:* Bundles the CoreCLR runtime engine (`coreclr.dll`, `clrjit.dll`), the BCL assemblies, and the native WPF rendering subsystem (`wpfgfx_cor3.dll`, `PresentationNative_cor3.dll`, `D3DCompiler_47_cor3.dll`). It guarantees that PigTree executes identically across every Windows 10 (Build 19041+) and Windows 11 installation with zero prerequisite installation steps.

2. **ReadyToRun (R2R) Ahead-of-Time Precompilation:**
   * ReadyToRun formats assemblies as crossgen2-compiled ReadyToRun images containing both native machine code (x64) and IL bytecode fallback.
   * *Impact:* Reduces WPF cold-startup JIT overhead by 30%–50%. Because disk analyzers are frequently launched ad-hoc to inspect a sudden low-disk-space condition, fast cold startup is a direct quality metric.
   * *Tradeoff:* Increases assembly size by approximately 20%–30%, which is well within acceptable boundaries for desktop distribution.

3. **Native AOT Infeasibility for WPF:**
   * While .NET 8 and .NET 9 introduced production Native AOT for console apps, ASP.NET Core web APIs, and minimal libraries, **WPF is fundamentally unsupported for Native AOT**.
   * *Root Cause:* WPF's internal architecture is built on dynamic reflection, runtime dependency property registration (`DependencyProperty.Register`), dynamic type descriptor reflection for data binding (`System.ComponentModel.TypeDescriptor`), unmanaged C++/CLI interop layers (`DirectWriteForwarder.dll`), and the runtime BAML (Binary Application Markup Language) interpreter/stream loader.
   * Compiling WPF with `<PublishAot>true</PublishAot>` results in compilation errors (`NETSDK1168: WPF is not supported or recommended with trimming enabled`) or catastrophic runtime `NullReferenceException` and `MissingMethodException` failures when XAML templates attempt runtime instantiation.
   * *Conclusion:* Native AOT must not be planned or promised for WPF in the v1/v2 architecture.

4. **Single-File Bundling vs. Multi-File Clean Directory:**
   * .NET provides `<PublishSingleFile>true</PublishSingleFile>`. For managed code, .NET 6+ loads assemblies directly from memory mapped bundles.
   * *The WPF Native Binary Hazard:* WPF relies on native unmanaged C++ DLLs (`wpfgfx_cor3.dll`, `PresentationNative_cor3.dll`). When `IncludeNativeLibrariesForSelfExtract=true` is enabled, the CLR host unpacks these DLLs into a user temporary directory (`%TEMP%\net\<app>\<bundle_id>\`) on first run.
   * *Consequences of Self-Extract:*
     * Adds 200–500ms disk extraction delay on cold startup.
     * Triggers aggressive heuristic scanning in Windows Defender / commercial antivirus minifilters monitoring temp directory executable drops.
     * Leaves orphaned directory litter when unclean shutdowns occur.
   * *Heterogeneous Multi-Process Architecture:* PigTree is not a single executable—it consists of the WPF UI, the Rust engine (`pigtree-engine.exe`), scanner worker (`pigtree-worker.exe`), and CLI (`pigtree.exe`).
   * *Recommended Layout:* Multi-file self-contained directory layout without single-file bundling. All executables and shared runtime DLLs reside in a single flat or structured installation folder.

---

### 3.2 Installer Technologies: MSIX vs. WiX Toolset v5 (MSI) vs. Portable ZIP

#### Primary Sources
* [WiX Toolset v5 Official Documentation](https://wixtoolset.org/docs/intro/)
* [Microsoft Learn: MSIX Overview and Architecture](https://learn.microsoft.com/en-us/windows/msix/overview)
* [Microsoft Learn: AppInstaller and Web Install for MSIX](https://learn.microsoft.com/en-us/windows/msix/app-installer/installing-windows10-apps-web)
* [Microsoft Learn: Package with External Location (Sparse Package)](https://learn.microsoft.com/en-us/windows/apps/desktop/modernize/grant-identity-to-non-packaged-apps)
* [Microsoft Learn: Windows Installer Standard Properties (ALLUSERS, MSIINSTALLPERUSER)](https://learn.microsoft.com/en-us/windows/win32/msi/msiinstallperuser)

#### Deep Comparison

#### 1. MSIX (Windows App Package)
* **Strengths:** 100% clean declarative uninstall, atomic updates via `.appinstaller`, tamper-evident block-level payload verification, first-class Windows 11 context menu identity.
* **Fatal Flaws for Open-Source Utility:**
  1. *Mandatory Trusted Certificate Signing:* An MSIX package **cannot be installed** by double-clicking unless the code-signing certificate chains to a trusted root certificate in the user's Trusted Root Certification Authorities store. If an open-source project distributes a self-signed or unverified MSIX, the user receives an impassable red banner: `"The publisher of this package cannot be verified"` (`0x800B0109`). Sideloading requires running PowerShell scripts to install root certificates or enabling Developer Mode.
  2. *Virtualized Container Boundaries:* MSIX isolates the file system (`VFS`) and registry. While unpackaged processes can be spawned, elevated out-of-process helper execution (`runas` verbs) and bidirectional named-pipe rendezvous can hit AppContainer / Virtualized Environment sandbox ACL edge cases.
  3. *Incompatibility with Portable Workflows:* MSIX cannot be "unzipped" and run directly from a USB stick without installation.

#### 2. WiX Toolset v5 (Modern MSI Windows Installer)
* **Strengths:**
  1. *Universal Win32 Compatibility:* Fully compatible with all Windows versions (Windows 10/11 x64, Windows Server 2016–2025).
  2. *Per-User Dual-Purpose Architecture:* WiX v5 supports `<Package Scope="perUserOrMachine">` with standard MSI properties `ALLUSERS=2` and `MSIINSTALLPERUSER=1`.
     * By default, installs to `%LocalAppData%\Programs\PigTree` with **zero UAC prompt** required.
     * When executed with administrative credentials or via corporate deployment (e.g. `msiexec /i PigTree.msi ALLUSERS=1 /qn`), installs to `C:\Program Files\PigTree` for all users.
  3. *Transactional Upgrades & Rollback:* Utilizes Windows Installer's built-in `MajorUpgrade` mechanism. If an update fails mid-installation, Windows Installer atomically rolls back the filesystem and registry to the exact prior working state.
  4. *Restart Manager Integration:* Automatically integrates with Windows Restart Manager (`RmStartSession`), notifying running instances of PigTree or its engine daemon to gracefully close and restart during updates, avoiding forced machine reboots.
  5. *Open-Source & Fully Scriptable:* WiX v5 compiles via standard .NET CLI tooling (`dotnet build`) without requiring Visual Studio IDE GUI dependencies.

#### 3. Portable ZIP Distribution
* **First-Class Requirement:** WizTree, TreeSize Free, Process Hacker/System Informer, and Process Monitor have demonstrated that Windows systems engineering utilities must provide a standalone portable ZIP.
* **Properties:**
  * Contains the exact directory image: `PigTree.exe`, `pigtree-engine.exe`, `pigtree-worker.exe`, `pigtree.exe`, runtime DLLs, and a marker configuration file (`portable.lock` or local config directory).
  * Requires zero installation, leaves zero registry artifacts, and can be extracted to any directory (`C:\Tools\PigTree`, USB drive, or network share).
  * Operates with full functionality (including on-demand UAC elevated scanning).

---

### 3.3 Code Signing, SmartScreen Reputation, and Open-Source Realities

#### Primary Sources
* [Microsoft Learn: Microsoft Defender SmartScreen Overview](https://learn.microsoft.com/en-us/windows/security/operating-system-security/virus-and-threat-protection/microsoft-defender-smartscreen/)
* [Microsoft Learn: Trusted Signing Overview (formerly Azure Code Signing)](https://learn.microsoft.com/en-us/azure/trusted-signing/overview)
* [SignPath Foundation: Free Code Signing for Open Source Projects](https://signpath.org/about-foundation/)
* [Microsoft Learn: SignTool.exe Documentation](https://learn.microsoft.com/en-us/windows/win32/seccrypto/signtool)

#### SmartScreen Mechanics & Reputation

1. **How SmartScreen Evaluates Binaries:**
   * SmartScreen computes a SHA-256 hash of the downloaded executable/installer and inspects its Authenticode digital signature.
   * If a binary is **Unsigned**, or signed with a brand-new certificate that has not accumulated telemetry reputation across the global Windows install base, SmartScreen displays the warning:
     > *"Windows protected your PC — Microsoft Defender SmartScreen prevented an unrecognized app from starting."*
   * As users click *"More info -> Run anyway"*, Microsoft Defender telemetry aggregates download volume and stability metrics, gradually granting the binary and certificate trusted reputation.

2. **Code Signing Options for Open-Source PigTree:**
   * *Option A: SignPath Foundation (Recommended for Open-Source):*
     * SignPath provides free code signing certificates (via trusted public CAs) for verified open-source GitHub projects.
     * Signing is executed entirely inside GitHub Actions CI runner pipelines using automated, tamper-proof signing policies.
   * *Option B: Azure Trusted Signing (Microsoft Trusted Signing):*
     * Microsoft's managed cloud signing service. Costs ~$9.99/month for basic identity validation.
     * Integrates directly into CI via `signtool.exe` using the Azure Trusted Signing dlib plugin (`Azure.CodeSigning.Dlib`).
     * Automatically recognized by Microsoft Defender SmartScreen and builds reputation rapidly.
   * *Option C: Community Fallback (Unsigned / Self-Signed + Checksums):*
     * In the event that official certificate validation is pending, all releases must publish verifiable SHA-256 checksums (`SHA256SUMS.txt`) and detached cryptographic signatures (Minisign or GPG) alongside clear documentation on bypassing SmartScreen warnings.

---

### 3.4 Process-File Separation & Elevation Architecture

#### Primary Sources
* [Microsoft Learn: User Interface Privilege Isolation (UIPI) Overview](https://learn.microsoft.com/en-us/windows/win32/winmsg/about-messages-and-message-queues#user-interface-privilege-isolation)
* [Microsoft Learn: Designing Applications for UAC and Least Privilege](https://learn.microsoft.com/en-us/windows/win32/sbscs/application-manifests)
* [Microsoft Learn: IPC over Named Pipes and Security Attributes](https://learn.microsoft.com/en-us/windows/win32/ipc/named-pipe-security-and-access-rights)
* [PigTree Research Note: Windows Scanning, Filesystem, and Elevation Facts](docs/research/windows-scanning-filesystem-elevation-facts.md)

#### Security Principle: Unprivileged GUI + On-Demand Elevated Worker

```
 +-------------------------------------------------------+
 |  PigTree.exe (WPF GUI)                                |
 |  - Medium Integrity (Standard User, asInvoker)        |
 |  - UI Automation / XAML / Treemap Canvas              |
 +---------------------------+---------------------------+
                             |
       +---------------------+---------------------+
       | Local Named Pipe    | Local Named Pipe    |
       | (Medium Integrity)  | (High Integrity)    |
       v                     v                     v
+-------------------+ +-------------------+ +---------------------+
| pigtree-engine.exe| | pigtree-worker.exe| | pigtree-worker.exe  |
| - Medium Integrity| | - Medium Integrity| | - Elevated Admin    |
| - Query Cache     | | - Win32 Directory | | - High Integrity    |
| - Session State   | |   Scan (SMB/FAT)  | | - Raw MFT / USN     |
+-------------------+ +-------------------+ +---------------------+
```

1. **Why the WPF GUI Must Never Run Elevated:**
   * *User Interface Privilege Isolation (UIPI):* When an application runs at High Integrity (elevated admin), Windows UIPI blocks unprivileged window messages from reaching it. This breaks standard Windows desktop ergonomics:
     * Drag-and-drop of files or folders from standard File Explorer windows into the PigTree window is silently blocked.
     * Accessibility tools (screen readers, screen magnifiers, automation frameworks) running at Medium Integrity cannot interact with the elevated window.
   * *Shatter Attack & Security Surface:* Complex UI stacks (WPF, DirectX, XAML, font loaders, image decoders) contain vast attack surfaces. Running them in a High Integrity process token allows any potential rendering exploit to achieve immediate Administrator access on the machine.

2. **Exact Process Separation:**
   * `PigTree.exe` (WPF GUI): Manifest contains `<requestedExecutionLevel level="asInvoker" uiAccess="false" />`. Always launches as a standard user.
   * `pigtree-engine.exe` (Rust Engine & Session Host): Handles session caching, snapshot storage, and query execution. Runs at Medium Integrity.
   * `pigtree-worker.exe` (Rust Scanner Helper):
     * *Standard Scan:* Spawned by the GUI or Engine as a standard child process to perform unprivileged Win32 / Batched Handle scans.
     * *Elevated Scan:* When scanning a local NTFS volume requiring USN Journal (`FSCTL_ENUM_USN_DATA`) or raw MFT parsing, the GUI spawns `pigtree-worker.exe` using Win32 `ShellExecuteExW` with `lpVerb = L"runas"` and parameter arguments specifying a unique IPC channel identifier.
     * *UAC Scope:* Triggers a single UAC consent dialog. The worker performs the high-speed scan, streams the parsed record buffer or snapshot file over the secured local IPC channel, and immediately terminates.
   * `pigtree.exe` (Rust CLI): Standalone CLI entry point for headless execution, scripting, and automation pipelines.

3. **IPC Security Attributes:**
   * All Named Pipes connecting GUI, Engine, and Elevated Worker (`\\.\pipe\pigtree-<session_guid>`) configure a strict Security Descriptor (`SECURITY_ATTRIBUTES` with DACL):
     * Grants `GENERIC_READ | GENERIC_WRITE` exclusively to the current user's SID (`TOKEN_USER`) and the Local Administrators group SID (`S-1-5-32-544`).
     * Prevents cross-session or unprivileged local user tampering.

---

### 3.5 Shell Integration Architecture (Windows 10 & Windows 11)

#### Primary Sources
* [Microsoft Learn: Creating Context Menu Handlers (Win32 Shell)](https://learn.microsoft.com/en-us/windows/win32/shell/context-menu-handlers)
* [Microsoft Learn: Windows 11 Modern Context Menu & IExplorerCommand](https://learn.microsoft.com/en-us/windows/apps/desktop/modernize/grant-identity-to-non-packaged-apps)
* [Microsoft Learn: Registering Shell Verbs in Registry](https://learn.microsoft.com/en-us/windows/win32/shell/fa-verbs)

#### Implementation Strategy

1. **Classic Shell Verbs (Windows 10 and Windows 11 "Show More Options"):**
   * Registered during installation (or dynamically via GUI Settings toggle).
   * For Per-User installs, registered under Current User registry hive (no UAC required):
     * `HKCU\Software\Classes\Directory\shell\PigTree` -> Default: `"Scan with PigTree"`, Icon: `"%LocalAppData%\Programs\PigTree\PigTree.exe,0"`
     * `HKCU\Software\Classes\Directory\shell\PigTree\command` -> Default: `""%LocalAppData%\Programs\PigTree\PigTree.exe" "%1""`
     * `HKCU\Software\Classes\Directory\Background\shell\PigTree\command` -> Default: `""%LocalAppData%\Programs\PigTree\PigTree.exe" "%V""`
     * `HKCU\Software\Classes\Drive\shell\PigTree\command` -> Default: `""%LocalAppData%\Programs\PigTree\PigTree.exe" "%1""`

2. **Windows 11 Modern Context Menu (Top-Level Command):**
   * Windows 11 requires a registered package identity and an out-of-process COM server implementing `IExplorerCommand`.
   * *Architecture:* PigTree provides an `AppxManifest.xml` sparse package manifest and an unmanaged COM DLL or Rust wrapper implementing `IExplorerCommand` (`GetTitle`, `GetIcon`, `GetState`, `Invoke`).
   * *Registration:* During MSI install or when the user checks "Enable Windows 11 Modern Context Menu" in Settings, PigTree invokes WinRT `PackageManager.RegisterPackageByUriAsync` or PowerShell `Add-AppxPackage -Register AppxManifest.xml` pointing to the installed application folder.

---

### 3.6 Atomic Updates, In-Use File Locking, and Rollback

#### Primary Sources
* [Microsoft Learn: Restart Manager Overview](https://learn.microsoft.com/en-us/windows/win32/rstmgr/about-restart-manager)
* [Microsoft Learn: MoveFileExW and Pending File Operations](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-movefileexw)
* [Velopack Architecture Documentation](https://velopack.io/docs/reference/overview)

#### Technical Analysis: Overcoming Windows In-Use File Locking

1. **The Windows File-Locking Constraint:**
   * On Windows, executing binary images (`.exe`, `.dll`) are mapped into memory with `FILE_SHARE_READ`. Attempting to delete or overwrite an active executable returns `ERROR_ACCESS_DENIED` (5) or `ERROR_SHARING_VIOLATION` (32).

2. **Update Strategies Evaluated:**

| Strategy | Mechanism | Pros | Cons / Verdict |
| :--- | :--- | :--- | :--- |
| **WiX / MSI MajorUpgrade** | Windows Installer transactional engine + Restart Manager | 100% transactional rollback on failure, standard corporate GPO compatibility | Requires running MSI installer; perfect for installer channel. |
| **Atomic Folder Swap (Velopack / Versioned Directories)** | Installs new version to `app-1.2.0/`, updates `current` shortcut, purges old folder on next restart | Fast, non-blocking background download, zero UAC prompt in `%LocalAppData%` | Requires folder-based launcher or shortcut redirection. |
| **Rename-in-Place Update** | Active `PigTree.exe` is renamed to `PigTree.exe.old` via `MoveFileExW`, new binary written to `PigTree.exe`, old file purged on reboot | Minimalist, single-directory layout | Requires external helper process to finish cleanup. |

3. **Recommended Dual-Track Update Architecture:**
   * *For MSI Installed Channel:* Updates are delivered as delta/full `.msi` packages. The in-app update checker detects the release, downloads the MSI to a temporary directory, and executes `msiexec /i PigTree-update.msi /qn` (or interactive UI). Windows Installer handles atomic replacement and rollback.
   * *For Portable ZIP Channel:* In-app notification alerts the user to the new version with release notes. Power users can click "Download Update" which retrieves the verified ZIP, extracts it via a temporary helper script (`pigtree-updater.exe`) that swaps the binaries after the main process exits, and relaunches the app.

---

### 3.7 Deterministic and Reproducible Packaging Pipeline

#### Primary Sources
* [Microsoft Learn: Deterministic Builds in Roslyn / MSBuild](https://learn.microsoft.com/en-us/dotnet/csharp/language-reference/compiler-options/code-generation#deterministic)
* [Rust Compiler Documentation: Source Path Remapping (`--remap-path-prefix`)](https://doc.rust-lang.org/rustc/command-line-arguments.html#--remap-path-prefix)
* [Reproducible Builds: SOURCE_DATE_EPOCH Specification](https://reproducible-builds.org/docs/source-date-epoch/)
* [WiX Toolset: Deterministic Component ID and Cabinet Generation](https://wixtoolset.org/docs/tools/wixext/wixutil/)

#### Requirements for Byte-for-Byte Reproducibility

1. **.NET (C# / WPF) Reproducible Settings:**
   * In `PigTree.Wpf.csproj`:
     ```xml
     <PropertyGroup>
       <Deterministic>true</Deterministic>
       <ContinuousIntegrationBuild>true</ContinuousIntegrationBuild>
       <PathMap>$(MSBuildProjectDirectory)=/_/</PathMap>
       <EmbedUntrackedSources>true</EmbedUntrackedSources>
     </PropertyGroup>
     ```
   * *Effect:* Eliminates developer machine absolute file paths from debug symbols and assembly headers, ensuring identical compilation output regardless of build machine directory.

2. **Rust Engine & Worker Reproducible Settings:**
   * In `.cargo/config.toml` or build script:
     ```toml
     [build]
     rustflags = [
       "--remap-path-prefix", "/home/runner/work/PigTree=/src",
       "--remap-path-prefix", "C:\\Users\\runneradmin\\AppData\\Local\\Temp=/tmp"
     ]
     ```
   * Pinned dependencies via `Cargo.lock`.
   * Pinned toolchain via `rust-toolchain.toml`.

3. **Packaging Artifact Reproducibility:**
   * **ZIP Generation:** Set fixed timestamps for all archive headers (using the Git commit timestamp via `SOURCE_DATE_EPOCH`) to prevent ZIP CRC mismatches caused by file modification timestamps.
   * **WiX MSI Compilation:** Use deterministic GUID generation (`Guid="*"` with standard WiX v5 hashing algorithms) and fixed cabinet timestamps.

---

## 4. Concrete Physical Layout on Disk

When installed to `%LocalAppData%\Programs\PigTree` or unpacked from a portable ZIP, the directory structure is organized as follows:

```
PigTree/
├── PigTree.exe                    # WPF GUI Entry Point (Medium Integrity)
├── PigTree.dll                    # WPF Managed Application Logic
├── PigTree.runtimeconfig.json      # .NET CoreCLR Runtime Configuration
├── pigtree-engine.exe             # Rust Private Engine & Session Host
├── pigtree-worker.exe             # Rust Scanner Helper (Medium or High Integrity)
├── pigtree.exe                    # Standalone Rust CLI Tool
│
├── coreclr.dll                    # .NET Runtime Engine
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
├── AppxManifest.xml               # Sparse Package Manifest (Win11 Context Menu)
├── PigTreeShellCommand.dll        # Unmanaged IExplorerCommand COM Stub (Optional)
├── assets/                        # Icons, Branding, and Localization resources
│   ├── pigtree.ico
│   └── logo.png
└── LICENSE.txt
```

---

## 5. Rejected Alternatives & Technical Boundaries

1. **Rejected: Framework-Dependent Deployment**
   * *Reason:* Requires end users to independently locate, download, and install the .NET Desktop Runtime. Breaks the zero-friction experience expected of a standalone system utility.
2. **Rejected: Native AOT Compilation for WPF**
   * *Reason:* Officially unsupported by Microsoft and fundamentally incompatible with WPF's reliance on dynamic reflection, BAML loading, and C++/CLI dependencies in .NET 8 and .NET 9.
3. **Rejected: Single-File Bundling with Self-Extraction**
   * *Reason:* Extracting native WPF rendering binaries (`wpfgfx_cor3.dll`, etc.) to `%TEMP%\net\*` introduces cold-startup latency, leaves unmanaged disk litter, and triggers antivirus false positives.
4. **Rejected: MSIX-Only Distribution**
   * *Reason:* Blocks sideloading for users without pre-installed root CA certificates, cannot provide a portable zero-install USB experience, and imposes containerization friction on elevated helper execution.
5. **Rejected: Elevated (High Integrity) WPF GUI**
   * *Reason:* Violates Windows User Interface Privilege Isolation (UIPI), blocks File Explorer drag-and-drop, breaks accessibility tools, and exposes a massive UI framework surface to elevated privilege vulnerabilities.

---

## 6. Release Verification Gates

Before publishing an official release of PigTree, the automated CI/CD pipeline must enforce the following validation gates:

1. **Cross-Compilation & Unit Test Gate:**
   * Rust test suite passes with 100% coverage on core scanning/engine logic (`cargo test --workspace`).
   * .NET test suite passes for WPF view models, converters, and IPC client adapters (`dotnet test`).
2. **Deterministic Build Verification Gate:**
   * Build artifacts generated on two separate clean CI runners produce bit-for-bit identical SHA-256 checksums.
3. **Authenticode Code Signing Gate:**
   * All `.exe`, `.dll`, and `.msi` binaries are signed using Azure Trusted Signing / SignPath with a valid timestamp counter-signature.
   * Signature verification (`signtool verify /pa /v PigTree.exe`) succeeds without warnings.
4. **Headless Smoke Test Gate:**
   * Automated PowerShell script validates:
     1. Silent per-user MSI installation to temporary user profile.
     2. `PigTree.exe` and `pigtree-engine.exe` launch and complete standard IPC handshake.
     3. `pigtree-worker.exe` executes a test directory scan and returns valid stream data.
     4. Silent MSI uninstallation leaves zero orphaned files.
5. **Antivirus & SmartScreen Telemetry Gate:**
   * Submission to VirusTotal API to ensure 0/70 detection flags before public release announcement.

---

## 7. Primary Source Citations

1. **Microsoft Learn (.NET Application Deployment & Publishing):**  
   https://learn.microsoft.com/en-us/dotnet/core/deploying/
2. **Microsoft Learn (ReadyToRun Compilation Architecture):**  
   https://learn.microsoft.com/en-us/dotnet/core/deploying/ready-to-run
3. **Microsoft Learn (Native AOT Deployment & Limitations):**  
   https://learn.microsoft.com/en-us/dotnet/core/deploying/native-aot/
4. **Microsoft Learn (.NET Single-File Deployment Overview):**  
   https://learn.microsoft.com/en-us/dotnet/core/deploying/single-file/overview
5. **dotnet/wpf GitHub Repository (Native AOT & Trimming Support Status):**  
   https://github.com/dotnet/wpf/issues/11205  
   https://github.com/dotnet/wpf/issues/5909
6. **WiX Toolset v5 Documentation (Architecture, Scopes, and MajorUpgrade):**  
   https://wixtoolset.org/docs/intro/
7. **Microsoft Learn (Package with External Location / Sparse Packages):**  
   https://learn.microsoft.com/en-us/windows/apps/desktop/modernize/grant-identity-to-non-packaged-apps
8. **Microsoft Learn (Windows 11 Explorer Command & Modern Context Menus):**  
   https://learn.microsoft.com/en-us/windows/apps/desktop/modernize/grant-identity-to-non-packaged-apps
9. **Microsoft Learn (Trusted Signing / Azure Code Signing):**  
   https://learn.microsoft.com/en-us/azure/trusted-signing/overview
10. **SignPath Foundation (Free Code Signing for Open Source):**  
    https://signpath.org/about-foundation/
11. **Microsoft Learn (User Interface Privilege Isolation - UIPI):**  
    https://learn.microsoft.com/en-us/windows/win32/winmsg/about-messages-and-message-queues#user-interface-privilege-isolation
12. **Microsoft Learn (Windows Restart Manager Architecture):**  
    https://learn.microsoft.com/en-us/windows/win32/rstmgr/about-restart-manager
13. **Microsoft Learn (Deterministic Builds with MSBuild):**  
    https://learn.microsoft.com/en-us/dotnet/csharp/language-reference/compiler-options/code-generation#deterministic
14. **Reproducible Builds Organization (SOURCE_DATE_EPOCH Specification):**  
    https://reproducible-builds.org/docs/source-date-epoch/
15. **Velopack Documentation (Windows In-Use Updating & Architecture):**  
    https://velopack.io/docs/reference/overview
