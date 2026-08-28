# PigTree: Core Analysis Workflows Prototype

> **PROTOTYPE / THROWAWAY CODE**
> This prototype explores user workflow variants for PigTree. It contains no production code, filesystem hooks, analytics, or persistent state.

---

## 🎯 Evaluated Question

> *“Which prototype of scan-target and reusable-preset selection, scan progress, results overview, synchronized tree/table, flat-file, treemap, file-type, age, and largest-item views, search and filtering, saved-analysis reopening, detail inspection, and guarded-cleanup handoff best helps everyday users answer what is using space and what they can safely do next while keeping all expert information discoverable and accessible?”* (AFlyingP/PigTree#9)

---

## 🚀 One-Command Launch

Run the following command from the repository root to start Python's built-in HTTP server:

```bash
python -m http.server 8000 --directory prototypes/core-analysis-workflows
```

Then open your browser to:
- **Default / Explorer Variant:** [http://localhost:8000/?variant=explorer](http://localhost:8000/?variant=explorer)
- **Insights Variant:** [http://localhost:8000/?variant=insights](http://localhost:8000/?variant=insights)
- **Workbench Variant:** [http://localhost:8000/?variant=workbench](http://localhost:8000/?variant=workbench)

*(The prototype also works by directly opening `index.html` in modern browsers).*

---

## ⌨️ Keyboard Controls

- **`←` (Left Arrow)** / **`→` (Right Arrow)**: Cycle previous / next workflow variant (URL parameters update automatically).
- **`1`**: Jump directly to **Variant 1: Explorer** (`?variant=explorer`).
- **`2`**: Jump directly to **Variant 2: Insights** (`?variant=insights`).
- **`3`**: Jump directly to **Variant 3: Workbench** (`?variant=workbench`).
- **`Escape`**: Dismiss open modals (Guarded Cleanup Action Plan preview or Coverage Gap dialog).
- **`Tab` / `Shift+Tab`**: Focus controls, tree nodes, table headers, and treemap cells in logical order.
- **`Enter` / `Space`**: Select rows, trigger sorting, open modals, or toggle tree expansion.

---

## 📐 Workflow Variants

### 1. `explorer` (Navigation-First)
- **Concept:** Calm, structured three-pane layout for users accustomed to traditional file managers and disk utilities.
- **Components:**
  - **Left Pane:** Expandable folder tree hierarchy with live allocation badges.
  - **Center Pane:** Multi-view analysis workspace supporting **Folder Table**, **Flat Files**, **Treemap**, **File Types**, **Age Breakdown**, and **Largest Items**.
  - **Bottom Subpanel:** Proportional synchronized Treemap preview.
  - **Right Pane:** Contextual Detail Inspector distinguishing entry facts from underlying filesystem object facts.

### 2. `insights` (Question-First, Everyday User)
- **Concept:** Plain-language, progressive disclosure answering the primary questions ordinary users ask:
  1. *“What is taking up your disk space?”* (Categorized breakdown into Users, Games, OS).
  2. *“What changed since the last snapshot?”* (Historical comparison highlights).
  3. *“What can I safely review for cleanup?”* (High-confidence user-reviewable areas like Downloads and Temp caches).
  4. *“Why is there unattributed or inaccessible space?”* (Clear explanations for 24 GB NTFS metadata & System Volume Information).
- **Progressive Drill-Down:** Interactive synchronized table and treemap situated directly below summary cards.

### 3. `workbench` (Dense Expert Workspace)
- **Concept:** High-density, command- and token-driven workspace for system administrators and power users.
- **Components:**
  - **Top Command Bar:** Fast filter tokens (`size > 1GB`, `type:archives`, `type:binaries`, profile presets).
  - **Dense Matrix Table:** Full metadata columns exposing **Unique Allocated Size**, **Referenced Allocated Size**, **Logical Size**, **Hard Link Reference Counts**, **Object IDs**, **Storage Characteristics**, and **Owner/Permissions**.
  - **Secondary Side Panels:** Proportional visual treemap coupled with a deep Object & Stream inspector.

---

## 🔬 Domain Concepts & Test Cases Demonstrated

The prototype implements the domain model documented in `CONTEXT.md`:

1. **Hard Link Accounting & Aliasing:**
   - `C:\Windows\System32\shell32.dll` and `C:\Windows\WinSxS\amd64_microsoft-windows-shell32_...\shell32.dll` share the same underlying object identity (`obj_shell32`).
   - Shows **Referenced Allocated Size** (28.4 MB) vs **Unique Allocated Size** (14.2 MB) and **Entry Count** (2) vs **Unique Object Count** (1).
   - Guarded cleanup preview accurately computes **0 B Reclaimable Allocation** (single reference decrement only).

2. **Cloud Placeholders / Storage Characteristics:**
   - `C:\Users\Alex\OneDrive\Archive_2024_Backup.zip` has **4.8 GB Logical Size** but **0 B Physical Disk Allocation** (`online-only`, `reparse-point`).

3. **Coverage Gaps & Inaccessible Scopes:**
   - `C:\System Volume Information` is flagged with Observation Status `inaccessible` (`STATUS_ACCESS_DENIED`).
   - Snapshot Coverage is marked `PARTIAL` with a noncommittal guidance prompt: *“Additional security privileges or backup-intent read may reveal more metadata.”*
   - Volume accounted allocation is treated as a **Known Subtotal**.

4. **Capacity Reconciliation & Unattributed Used Space:**
   - Volume Used Space: **392 GB**, Accounted Unique Allocation: **368 GB**.
   - The **24 GB difference** is surfaced as a dedicated **Reconciliation Difference** row/block, never disguised as a fake folder.

5. **Guarded Cleanup Action Plan Handoff:**
   - Reclaimable allocation estimates distinguish exact unshared files, hard link decrements, online-only placeholders, and directory bounds under external reference uncertainty.
   - Built-in protection warnings prevent destructive deletion of OS system files, suggesting native tools (`DISM`, `Cleanmgr`, or Windows Settings).

6. **Historical Snapshots:**
   - Loading saved historical snapshots displays a clear warning banner: *“Facts observed during that interval, not live system state.”*

---

## 🛠 Prototype Limitations

- **Simulated In-Memory Data:** Changes, scan runs, and cleanup handoffs operate entirely on in-memory mock models.
- **No Native File Operations:** No files are deleted, modified, or accessed from the host OS.
- **No Production Toolchain:** Standard static HTML/CSS/JS without external npm or CDN dependencies.
