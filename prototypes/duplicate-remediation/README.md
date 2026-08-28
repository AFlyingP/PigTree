# PigTree: Safe Duplicate Review & Remediation Prototype

> **PROTOTYPE WARNING**: This is an interactive throwaway UI prototype. All data is mock data held strictly in memory. The workflow begins from read-only disk analysis and stops at an immutable **Action Plan preview**. It **never** performs destructive disk mutations, network calls, or filesystem changes.

---

## Core Evaluation Question

> **Which interaction model lets everyday users understand duplicate groups, current verification, which copy remains, metadata consequences, recovery versus immediate reclaim, failures, and cloud hydration before committing an Action Plan?**

---

## Quick Start & Local Execution

To run the standalone HTTP server:

```bash
python -m http.server 8015 --bind 127.0.0.1 --directory prototypes/duplicate-remediation
```

Open your browser to any of the three variant routes:

- **Variant A (Guided Review)**: [http://127.0.0.1:8015/?variant=guided](http://127.0.0.1:8015/?variant=guided)
- **Variant B (Evidence Matrix)**: [http://127.0.0.1:8015/?variant=matrix](http://127.0.0.1:8015/?variant=matrix)
- **Variant C (Plan Workspace)**: [http://127.0.0.1:8015/?variant=plan](http://127.0.0.1:8015/?variant=plan)

To run the automated validation test suite:

```bash
node prototypes/duplicate-remediation/validate-prototype.js
```

---

## The Three Interaction Hypotheses

| Variant | Paradigm | Best Suited For | Key Interaction Features |
| :--- | :--- | :--- | :--- |
| **A. `guided`** | **Guided Step Rail** | Everyday users (Default hypothesis) | Sequential step rail (`Candidate` &rarr; `Verify` &rarr; `Choose Keeper` &rarr; `Choose Actions` &rarr; `Review Plan`). Plain-language group narrative, rich copy cards, and sticky consequence bar. |
| **B. `matrix`** | **Evidence Comparison Matrix** | Power users & Systems administrators | Dense comparative matrix table where columns are Filesystem Objects and rows are evidence categories (streams, ACLs, timestamps, hashes, hard links, recovery eligibility). Inline verification drawer. |
| **C. `plan`** | **Plan-Builder Workspace** | High-throughput batch operations | Three-pane workspace: Filterable duplicate group queue on the left, synchronized deep inspector in the center, and live Action Plan ledger with grouped preconditions on the right. |

*Note: All user selections (chosen keepers, actions, excluded items, hydration consents, and verification progress) are preserved intact when switching between variants.*

---

## Mock Scenarios & Evaluation Guide

The prototype implements five challenging real-world Windows filesystem scenarios to stress-test safety concepts:

### 1. `Vacation originals` (Content Stream & ACL Mismatch)
- **Filesystem Reality**: Three 4.8 GB camera RAW files. Two have identical unnamed streams and ACLs; the third (`Downloads\IMG_9204_RAW (1).CR3`) contains a named `Zone.Identifier:$DATA` stream (Internet Mark-of-the-Web) and restrictive ACLs.
- **Verification**: Candidate discovery is not proof. Clicking **Start Verification** steps through all 5 stages and discovers the stream/ACL mismatch, settling into `Mismatch Detected` and locking hard link actions.
- **Remediation**: Click **Exclude from group** on the Downloads copy. Re-running verification verifies the remaining two copies (`Verified`), unlocking Hard Link consolidation.
- **Recovery Accounting**:
  - *Recoverable Hard Link*: Immediate reclaim = **0 B**, Retained in PigTree recovery storage = **4.8 GB**.
  - *Immediate-reclaim Hard Link*: Immediate reclaim = **4.8 GB**, Retained = **0 B** (Irreversible).
  - *Recycle Bin*: Immediate reclaim = **0 B**, Conditional future reclaim = **4.8 GB**.

### 2. `Build artifacts` (Hard Link Aliases & Multi-Object Reclaim)
- **Filesystem Reality**: Four 1.2 GB library objects. The build output file has an existing Hard Link alias in `dist\bin`. Both point to the **same underlying Filesystem Object** (`linkCount: 2`).
- **Safety Rule**: The linked object appears **once** with two Directory Entries, never as two duplicate candidates.
- **Accounting**: When 3 non-keeper objects are hard-linked, reclaim correctly calculates **3 &times; 1.2 GB = 3.6 GB** (counting distinct victim objects, not path aliases).

### 3. `OneDrive project archive` (Online-Only Cloud Placeholder)
- **Filesystem Reality**: Two 7.4 GB archives. One is local on SSD; the other is an online-only OneDrive placeholder (`Allocated Size: 0 bytes`).
- **Hydration Consent**: Clicking verification opens an explicit **Cloud Hydration Consent Dialog** outlining 7.4 GB download and disk impact. Declining keeps it unverified.
- **Protection**: Direct deletion or hard-linking of cloud entries is protected and routed to **Cloud Provider Handoff** (Explorer / OneDrive). Local reclaim for deleting the online-only placeholder is accurately reported as **0 B** (never a fake 7.4 GB).

### 4. `Installer cache lookalikes` (System Protected Resources)
- **Filesystem Reality**: Files under `Program Files\WindowsApps` (TrustedInstaller) and `Windows\SystemTemp`.
- **Safety Rule**: Direct deletion is blocked (`Action Risk Class: protected`). Actions are routed to **Native System Handoffs** (Windows Settings > Installed Apps, Storage Sense, Disk Cleanup). **Never labeled "safe to delete"**.

### 5. `Changed since scan` (Live Preflight Invalidation)
- **Filesystem Reality**: A 900 MB report pair where live re-observation detects that the Downloads copy was modified (File ID changed, timestamp +2m, size expanded to 912 MB).
- **Safety Rule**: Status settles to **Stale / Prohibited**. In-place overrides are prohibited by ADR 0002 safety rules; a fresh Analysis Run and Action Plan are required.

---

## Action Plan Preview (Execution Safety Boundary)

Clicking **Preview Immutable Action Plan** opens the execution manifest modal:
- Shows exact Directory Entry targets and expected File IDs.
- Lists Keeper redirection relationships.
- Groups operations by Recovery Class, privilege level, and risk class.
- Details live preflight re-observation requirements and Commit Point semantics (rename-over and staging link purge).
- Explains partial failure and cancellation semantics.
- **No Execute Button**: Flow ends strictly at preview and in-memory export.

---

## Keyboard Navigation & Accessibility

- **Variant Switching**: Press <kbd>&larr;</kbd> (Left Arrow) or <kbd>&rarr;</kbd> (Right Arrow) to toggle between Guided, Matrix, and Plan workspaces (automatically suppressed when typing in inputs or interacting with dialogs).
- **Dialogs**: Press <kbd>Escape</kbd> to close any active modal with focus restored to the trigger button.
- **Screen Readers**: Full ARIA markup (`aria-live="polite"` status announcer, accessible headings, semantic tables, and modal focus traps).
- **Contrast & Theming**: Supports Light Mode, Dark Mode (`prefers-color-scheme: dark`), Forced Colors mode, and scalable rem typography at 200% zoom.

---

## Non-Goals

1. **No Disk Mutation**: Prototype has no backend execution engine and performs zero destructive actions.
2. **No Persistent Database**: All state is held in memory for immediate interactive experimentation.
3. **No Heavy Frameworks**: Pure dependency-free ES Modules, standard HTML5, and vanilla CSS.
