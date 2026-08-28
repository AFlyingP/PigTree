# Research Note: WPF Production Rendering, Tree-Table Virtualization, Hardware-Accelerated Treemap, and Accessibility Architecture

**Ticket:** Prerequisite for [AFlyingP/PigTree#14](https://github.com/AFlyingP/PigTree/issues/14) (Select the production technology architecture)  
**Date:** March 2025  
**Scope:** Authoritative engineering investigation and decision-ready architectural design for the PigTree Windows Presentation Foundation (WPF on supported .NET LTS) user interface. Covers dense virtualized hierarchical tree-table rendering, hardware-accelerated synchronized treemap visualization, UI Automation (UIA) custom peer semantics, high contrast and theme integration, text scaling and Per-Monitor DPI V2 awareness, out-of-process Rust engine IPC boundary considerations, rejected alternatives, risks, and measurable release gates tied to the 5,000,000-entry universal floor, 60 FPS rendering budget, and accessibility constraints.

---

## 1. Executive Summary & Production Decision Landscape

The accepted PigTree core architecture comprises a high-performance **Rust engine/workers subsystem**, a **WPF (.NET LTS baseline) front-end**, and a **private, short-lived out-of-process Rust session host** providing isolation, privileged scanning coordination, and cross-interface reuse (GUI and CLI).

To satisfy the mandatory product performance targets ([docs/performance-targets.md](../performance-targets.md))—maintaining a target 60 FPS rendering rate and <= 100 ms interactive query/filter latency budgets at a scale floor of **5,000,000 Directory Entries** while delivering full WCAG 2.1 AA and Windows UI Automation accessibility—the production WPF presentation architecture establishes:

1. **Virtualized Hierarchical Tree-Table (Recommended v1 Design):** A **Flattened Virtual Projection Model** implemented via a customized WPF `ListView` with `VirtualizingStackPanel` utilizing container recycling (`VirtualizingStackPanel.VirtualizationMode="Recycling"`), pixel scrolling (`ScrollUnit="Pixel"`), and a local nonblocking **sliding-window page cache**. WPF **never allocates a full 5,000,000-entry descriptor array or unmanaged name arena**; the collection's `Count` is virtualized, and only bounded active page slices (e.g. 200–500 rows, consuming < 500 KB) reside in managed memory. Built-in recursive WPF `TreeView` and full in-memory descriptor/name arrays in WPF are rejected.
2. **Hardware-Accelerated Treemap Visualization:** Direct3D 11 / Direct2D hardware rendering hosted seamlessly in WPF via **`System.Windows.Interop.D3DImage`** using the documented legacy shared surface handle (`D3D11_RESOURCE_MISC_SHARED` via `IDXGIResource::GetSharedHandle`) bound to Direct3D 9Ex. Cushion shading and gradient borders execute on the GPU via HLSL pixel shaders, completely avoiding WPF Airspace clipping bugs. Synchronization follows the documented conservative pipeline: complete D3D11 rendering, invoke `ID3D11DeviceContext::Flush()`, and update `D3DImage` on the UI thread, with a fallback double-buffering/staging copy path if driver behavior is unreliable. Lifecycle handling includes `IsFrontBufferAvailableChanged`, device loss recovery, WARP software rasterization, and an accessible non-GPU fallback.
3. **Seam Placement & In-Process Treemap Layout:** The out-of-process Rust engine provides semantic weights and hierarchy; the WPF presentation layer computes the geometric $(x, y, w, h)$ squarified partitions locally against viewport dimensions. This prevents leaking display pixel dimensions into the session host, eliminates continuous IPC roundtrips during window resizing, and keeps the engine headless and display-agnostic.
4. **Accurate & Conservative UI Automation (UIA) Semantics:** Custom `AutomationPeer` implementations utilizing standard Windows UIA primitives: `ControlType.TreeItem` / `ControlType.DataItem`, fragment navigation (`IRawElementProviderFragment`), `IExpandCollapseProvider`, `ISelectionItemProvider`, `IScrollItemProvider`, and hierarchical properties (`Level`, `PositionInSet`, `SizeOfSet`). `GetChildrenCore()` exposes *only realized visible rows*, preventing 5M peer enumeration. `IItemContainerProvider::FindItemByProperty` is strictly bounded: it resolves exact `AutomationId` / row position tokens and selection states from local tracking structures, and searches `NameProperty` *only across currently realized/cached items*, returning `null` when an item is un-cached. Full-dataset search is handled via the product Search/Filter bar in the Rust engine, not masqueraded as UIA linear scanning. `Realize()` synchronously materializes only already-cached data, otherwise returning a standard `ElementNotAvailableException` / `UIA_E_ELEMENTNOTAVAILABLE`, while `ScrollIntoView()` independently updates the virtual position and initiates sliding-window hydration.
5. **Theme, High Contrast & Text Scaling:** Dynamic resource binding to system theme brushes (`SystemColors`), active query of `SystemParameters.HighContrast` (`SPI_GETHIGHCONTRAST`) to toggle high-contrast luminance palettes and structural border patterns, and full **Per-Monitor V2 DPI** manifest compliance paired with Windows text-scaling factor (`UISettings.TextScaleFactor`) tracking.
6. **Runtime Policy & IPC Alignment:** Built against the **latest supported .NET LTS at implementation and release time** (e.g. .NET 8 LTS or .NET 10 LTS). The presentation layer interface aligns with the emerging IPC direction (framed named pipes with schema-versioned serialization, provisional shared-memory bulk buffers), deferring transport specifics to the authoritative IPC decision.
```
+---------------------------------------------------------------------------------------------------+
|                                   WPF Production UI Architecture                                  |
+---------------------------------------------------------------------------------------------------+
|                                                                                                   |
|   +---------------------------------------+       +-------------------------------------------+   |
|   |   Dense Virtualized Tree-Table        |       |   Hardware-Accelerated Treemap Canvas     |   |
|   |   - Flattened Projection (Virtual)    |       |   - D3DImage (D3D11_RESOURCE_MISC_SHARED) |   |
|   |   - VirtualizingStackPanel (Recycle)  |       |   - Direct3D 11 / Direct2D Render Target  |   |
|   |   - Nonblocking Sliding-Window Cache  |       |   - GPU Cushion Shaders / DirectWrite     |   |
|   |   - Bounded Local State & ItemContainer|     |   - Zero Airspace Defect (WPF Blended)    |   |
|   |   - Pixel Scrolling & Display Text    |       |   - D3D11 Flush() & FrontBuffer Recovery  |   |
|   +---------------------------------------+       +-------------------------------------------+   |
|                       ^                                                 ^                         |
|                       | (Synchronous IList Window Slices)               | (Local Viewport Layout) |
|                       v                                                 v                         |
|   +-------------------------------------------------------------------------------------------+   |
|   |   WPF Presentation Model & IPC Client Layer (C# on latest supported .NET LTS)             |   |
|   |   - HighContrast / Theme Monitor       - Per-Monitor V2 DPI & UISettings Scaler           |   |
|   |   - Bi-directional Node Selection Bus  - Narrator / NVDA / JAWS UIA Peer Dispatcher       |   |
|   |   - Sliding-Window Page Buffer (<500KB)- In-Process Squarified Treemap Partition Engine   |   |
|   |   - Tracked Local Selection/Focus IDs  - Virtualized Collection Projection Adapter        |   |
|   +-------------------------------------------------------------------------------------------+   |
|                                                |                                                  |
|                        IPC Boundary (Referenced to Authoritative IPC Decision)                    |
|                        - Framed Named Pipe (Control / Queries / Events)                           |
|                        - Provisional Shared Memory MMF (Bulk Snapshots if warranted)              |
|                                                |                                                  |
|   +-------------------------------------------------------------------------------------------+   |
|   |   Private Short-Lived Rust Engine & Session Host (Out-of-Process Subsystem)                |   |
|   |   - 5M-Node Immutable Snapshot Store (Arena / Compact Packed Arrays)                      |   |
|   |   - Viewport Slice Generator & Multi-Column Sorting Index                                 |   |
|   |   - Scanning Worker Pool (MFT / USN Journal / Win32 Elevation)                            |   |
|   +-------------------------------------------------------------------------------------------+   |
|                                                                                                   |
+---------------------------------------------------------------------------------------------------+
```

---

## 2. Normative Performance Targets & Modeled Engineering Budgets

Per [PigTree Domain Architecture (CONTEXT.md)](../../CONTEXT.md) and [Product Performance Targets (docs/performance-targets.md)](../performance-targets.md), the rendering and presentation subsystem operates under explicit targets. All numeric performance figures represent **normative design budgets and modeled engineering projections** to be validated in formal benchmarking:

| Constraint Dimension | Normative Reference Budget | Universal Release Floor Gate | Verification Mode |
| :--- | :--- | :--- | :--- |
| **Dataset Scale Boundary** | 1,000,000 Directory Entries (Standard) | **5,000,000 Directory Entries** (Stress boundary: 10M) | Automated scale fixture |
| **Interactive Latency** | <= 100 ms (p95) for sort, filter, expand | <= 150 ms (p99) under active background scan | ETW trace click-to-render |
| **Tree-Table Scroll Frame Rate** | **60 FPS** sustained (<= 16.6 ms frame budget) | No frame drop > 33.3 ms (30 FPS transient floor) | DWM frame presentation clock |
| **Treemap Render Frame Rate** | **60 FPS** during pan/zoom/hover hit-test | GPU render pass <= 8.0 ms; CPU prep <= 5.0 ms | GPU performance counter query |
| **UI Process Working Set** | <= 150 MiB Base Managed Working Set (GUI) | <= 300 MiB Peak Working Set at 5M entries | Process working set telemetry |
| **UIA Search Resolution** | Synchronous local cached resolution | Zero Dispatcher thread deadlocks / IPC stalls | Automated UIA client harness |
| **Assistive Tech Navigation** | Responsive tree walk & speech output | Zero UI freezes under Narrator/NVDA/JAWS | Screen reader automated harness |
| **DPI / Display Scale Transitions** | Crisp text, zero blur, zero visual artifact | Instantaneous re-rasterization on `WM_DPICHANGED` | Visual diff & snapshot audit |

---

## 3. Virtualized Hierarchical Tree-Table Architecture

### 3.1 Failure Analysis of Standard WPF TreeView at Scale
Standard WPF `TreeView` controls instantiate a recursive hierarchy of `TreeViewItem` containers. In benchmarked WPF implementations, hosting > 50,000 hierarchical nodes causes severe degradation:
1. **Visual Tree Explosion:** Each realized `TreeViewItem` introduces 10 to 18 underlying `Visual` elements (`Border`, `ToggleButton`, `ContentPresenter`, `ItemsPresenter`, `StackPanel`). At 5,000,000 entries, naive instantiations would require over 60 million `Visual` instances, exceeding GDI/user object limits and exhausting managed heap memory.
2. **Recursive Virtualization Breakdown:** While WPF's `VirtualizingStackPanel` can virtualize top-level items, nested `TreeView` levels require nested `VirtualizingStackPanel` instances. Scrolling vertically requires evaluating nested layout measurements, creating severe UI thread layout stalls.
3. **Lack of Column Virtualization:** A standard `TreeView` does not natively align multi-column tabular data (Allocated Size, Unique Size, File Counts, % Bars, Timestamps, Coverage Gaps) with horizontal virtualization and header resizing across disparate branch depths.

### 3.2 The Flattened Virtual Projection Model & Bounded Memory Footprint

To achieve O(1) scrolling and rendering overhead regardless of whether the dataset contains 10,000 or 5,000,000 nodes, the presentation architecture separates the **hierarchical graph model** in the Rust engine from the **linear visual projection** in WPF:

```
+---------------------------------------------------------------------------------------------------+
|                                 Flattened Virtual Projection Model                                |
+---------------------------------------------------------------------------------------------------+
|                                                                                                   |
|   Hierarchical Graph (Rust Engine)              Flattened Visible Projection (Virtual Collection) |
|   [Root: C:\]                                   Idx  Depth  Flags  NodeID  Name          Size     |
|     |-- [Windows] (Expanded)                    0    0      Exp    101     C:\           450 GB   |
|     |     |-- [System32] (Expanded)             1    1      Exp    102       Windows      32 GB   |
|     |     |     |-- drivers (Collapsed)   ===>  2    2      Exp    105         System32   18 GB   |
|     |     |     \-- ntoskrnl.exe                3    3      Col    109           drivers   2 GB   |
|     |     \-- [WinSxS] (Collapsed)             4    3      Leaf   110           ntos...  12 MB   |
|     \-- [Users] (Expanded)                     5    2      Col    106         WinSxS     11 GB   |
|                                                 6    1      Exp    103       Users       380 GB   |
|                                                                                                   |
+---------------------------------------------------------------------------------------------------+
```

#### Bounded Managed Memory Strategy:
* **WPF Never Allocates Full 5M Descriptor/Name Array:** The presentation layer exposes an `IList` / `IReadOnlyList` whose `Count` property reflects the total count of projected rows reported by the engine (e.g. 5,000,000), but **no contiguous 5M array or global name arena is allocated in WPF memory**. Allocating full arrays in WPF is an explicit **rejected alternative**.
* **Page-Window Allocation:** WPF retains only a bounded sliding-window page cache of active rows (e.g., 200–500 rows centered around the active viewport, consuming $< 500\text{ KB}$ of managed memory), ensuring the UI working set remains strictly within the $\le 150\text{ MiB}$ budget.
* **Compact Row Schema:** When materialized in active pages, each row uses a 40-byte compact descriptor:
  ```csharp
  [StructLayout(LayoutKind.Sequential, Pack = 8)]
  public struct VirtualRowDescriptor
  {
      public ulong NodeId;          // 8 bytes: Filesystem Object unique identifier
      public ulong AllocatedBytes;  // 8 bytes: Attributable physical allocation
      public ulong UniqueBytes;     // 8 bytes: Unique allocated size
      public uint ParentRowIndex;   // 4 bytes: Flattened index of parent row
      public uint FileCount;        // 4 bytes: Aggregated child files
      public uint DirCount;         // 4 bytes: Aggregated child directories
      public ushort DepthLevel;     // 2 bytes: Indentation level (0..65535)
      public byte NodeFlags;        // 1 byte: Bit 0: IsDir, Bit 1: IsExpanded, Bit 2: HasChildren
      public byte CoverageStatus;   // 1 byte: Known, Unavailable, CoverageGap, Reconciled
  }                                 // Total: Exactly 40 bytes (8-byte aligned)
  ```

### 3.3 Synchronous WPF IList Indexing & Nonblocking Sliding-Window Cache

WPF's `VirtualizingStackPanel` requires synchronous element access via `IList[int index]` during layout and measurement. The indexer **must never execute blocking IPC calls** on the UI thread.

#### Sliding-Window Architecture & Placeholder Transitions:
1. **Sliding-Window Cache:** A local ring buffer retains $N \pm 250$ realized row records surrounding the current viewport.
2. **Immediate Synchronous Placeholder Return:** When `IList[int index]` experiences a cache miss during rapid scrollbar scrubbing:
   - The indexer synchronously returns a lightweight **placeholder record** with `IsPlaceholder = true`.
   - `AutomationProperties.ItemStatus` is set to `"Loading"` and `AutomationProperties.Name` reports `"Loading row {index}..."`.
   - The UI immediately realizes a skeleton/shimmer container without blocking layout.
3. **Asynchronous Range Fetch:** The cache miss triggers an asynchronous range prefetch over IPC.
4. **Data Materialization & UIA Notification:** When row data arrives from the engine:
   - The sliding-window cache populates the row descriptors.
   - Container view models update their properties without triggering a collection reset.
   - `AutomationProperties.ItemStatus` transitions to `"Loaded"` (or `""`) and `AutomationProperties.Name` transitions to the observed filesystem object name, raising `AutomationPropertyChangedEvent` for `NameProperty` and `ItemStatusProperty`.
---

## 4. Hardware-Accelerated Synchronized Treemap Architecture

### 4.1 Comparative Evaluation of Treemap Rendering Options

Rendering an interactive treemap containing 10,000–50,000 visible geometric rectangles with cushion shading, category palettes, selection outlines, and sub-millisecond hover hit-testing requires evaluated graphics primitives:

| Dimension | Option A: WPF `DrawingVisual` / `OnRender` | Option B: Direct3D 11 / Direct2D via `D3DImage` (Recommended) | Option C: DirectComposition / `HwndHost` | Option D: WinRT XAML Islands (`Windows.UI.Composition`) |
| :--- | :--- | :--- | :--- | :--- |
| **Pipeline Architecture** | Retained MILCore command stream (Direct3D 9Ex) | Direct3D 11 surface shared with Direct3D 9Ex | Separate Win32 child HWND with DXGI swapchain | WinRT container hosting WinUI 3 Composition visual |
| **Sustained Frame Rate (Modeled)** | 30–45 FPS at 20k rectangles (CPU command bottleneck) | **60–120+ FPS** (Fully GPU accelerated) | **60–120+ FPS** (Direct DWM swapchain) | 60 FPS (Composition engine) |
| **WPF Airspace Defect** | **None** (Native WPF Visual tree) | **None** (Blended into WPF composition pipeline) | **Severe Airspace Bug** (HWND clips all WPF popups/tooltips) | Moderate (Requires XAML Island boundary management) |
| **Cushion Treemap Shading** | CPU pre-baked bitmaps or radial gradient brushes | **HLSL Pixel Shader** (Hardware procedural evaluation) | Direct2D / HLSL Pixel Shader | Win2D / Composition Pixel Shaders |
| **Hover Hit-Testing Latency (Modeled)** | ~5 to 15 ms (`VisualTreeHelper.HitTest`) | **<= 0.2 ms** (CPU Quadtree / GPU pick buffer) | <= 0.2 ms (CPU spatial index) | ~1 to 5 ms |
| **Per-Monitor DPI & Scaling** | Automatic WPF scaling | Direct surface resizing on `WM_DPICHANGED` | Window message synchronization required | Automatic WinRT DPI handling |
| **Device Loss Recovery** | Handled internally by WPF MILCore | Explicit `D3DERR_DEVICELOST` / `DXGI_ERROR_DEVICE_REMOVED` | Explicit device recreation | Handled by WinRT composition |

### 4.2 Production Direct3D 11 / `D3DImage` Shared Surface Pipeline

The production architecture utilizes **`System.Windows.Interop.D3DImage`** hosting an off-screen **Direct3D 11** render target via the documented legacy shared handle mechanism.

```
+---------------------------------------------------------------------------------------------------+
|                         D3DImage Direct3D 11 Interoperability Pipeline                            |
+---------------------------------------------------------------------------------------------------+
|                                                                                                   |
|   +-------------------------------------------------------------------------------------------+   |
|   |   WPF Visual Tree & Render Loop                                                           |   |
|   |   - Image Source: D3DImage                                                                |   |
|   |   - Lock(), SetBackBuffer(), AddDirtyRect(), Unlock()                                     |   |
|   |   - Native WPF Tooltips, Context Menus, and Overlays layered directly on top (No Airspace)|   |
|   |   - Handles D3DImage.IsFrontBufferAvailableChanged                                        |   |
|   +-------------------------------------------------------------------------------------------+   |
|                                               |                                                   |
|                        Direct3D 9Ex / Direct3D 11 Shared Surface Bridge                           |
|                        - D3D11_RESOURCE_MISC_SHARED legacy handle (IDXGIResource::GetSharedHandle)|
|                                               |                                                   |
|   +-------------------------------------------------------------------------------------------+   |
|   |   Direct3D 9Ex Context (milcore Interop)                                                  |   |
|   |   - IDirect3DDevice9Ex::CreateTexture(..., HANDLE* pSharedHandle, D3DPOOL_DEFAULT)        |   |
|   |   - IDirect3DSurface9 passed to D3DImage.SetBackBuffer()                                  |   |
|   +-------------------------------------------------------------------------------------------+   |
|                                               ^                                                   |
|                        IDXGIResource::GetSharedHandle(HANDLE* pSharedHandle)                      |
|                                               |                                                   |
|   +-------------------------------------------------------------------------------------------+   |
|   |   Direct3D 11 Rendering Engine (PigTree Native Renderer)                                  |   |
|   |   - ID3D11Texture2D (Created with D3D11_RESOURCE_MISC_SHARED)                             |   |
|   |   - Direct2D 1.1 Device Context / DirectWrite Font Layouts                                |   |
|   |   - HLSL Pixel Shader: Fast Procedural Cushion Treemap Shading                            |   |
|   |   - Render Complete -> ID3D11DeviceContext::Flush() -> Signal WPF AddDirtyRect()          |   |
|   |   - Fallback: Double-Buffered Staging Copy & D3D_DRIVER_TYPE_WARP Software Rasterizer     |   |
|   +-------------------------------------------------------------------------------------------+   |
|                                                                                                   |
+---------------------------------------------------------------------------------------------------+
```

#### Documented Synchronization & Surface Pipeline:
1. **Shared Texture Creation:** The Direct3D 11 render target texture is created with `D3D11_RESOURCE_MISC_SHARED` (legacy shared handle).
2. **Surface Sharing:** Query `IDXGIResource` from the texture, retrieve the shared handle via `IDXGIResource::GetSharedHandle(&sharedHandle)`, open it on the Direct3D 9Ex device via `IDirect3DDevice9Ex::CreateTexture`, and pass the `IDirect3DSurface9` pointer to `D3DImage.SetBackBuffer()`.
3. **Documented Execution & Flush Pipeline:**
   - The Direct3D 11 renderer records and executes drawing commands.
   - Upon completion, the renderer calls `ID3D11DeviceContext::Flush()` to submit all queued commands to the GPU command buffer.
   - On the WPF UI thread, `D3DImage.Lock()`, `D3DImage.AddDirtyRect()`, and `D3DImage.Unlock()` are invoked to signal the WPF composition engine.
   - *Conservative Concurrency Note:* `Flush()` ensures command submission but does not guarantee instantaneous cross-API GPU execution completion on all GPU drivers. The implementation will benchmark target hardware for visual tearing; if driver synchronization anomalies are observed, a double-buffered shared surface pair or an explicit staging copy path will be engaged as a robust fallback.

#### Robust Lifecycle & Failure Recovery:
* **Front-Buffer Availability Handling:** When the user locks Windows (Ctrl+Alt+Del), switches users, or initiates full-screen mode changes, WPF raises **`D3DImage.IsFrontBufferAvailableChanged`**. The renderer immediately pauses rendering while `IsFrontBufferAvailable` is `false`, and invalidates/recreates back buffers once `true`.
* **DirectX Device Loss Recovery:** If a GPU driver reset occurs (`D3DERR_DEVICELOST`, `DXGI_ERROR_DEVICE_REMOVED`, or `DXGI_ERROR_DEVICE_RESET`), the renderer releases all outstanding D3D11 texture interfaces and D3D9Ex pointers, reinitializes device contexts, recreates the shared texture, and calls `D3DImage.SetBackBuffer()`.
* **WARP & Non-GPU Fallback:** If hardware GPU initialization fails (e.g. in virtualized environments or remote desktop sessions), the engine initializes using **`D3D_DRIVER_TYPE_WARP`** (Direct3D 11 high-speed CPU rasterizer). If WARP is unavailable, the UI gracefully falls back to an accessible non-GPU tabular view.

### 4.3 Seam Placement: In-Process Treemap Viewport Layout

The production architecture firmly places geometric squarified treemap layout computation ($(x, y, w, h)$) in the **WPF presentation layer**:

```
+---------------------------------------------------------------------------------------------------+
|                            Treemap Layout Ownership Comparison                                    |
+------------------------------------+--------------------------------------------------------------+
| Out-of-Process Layout (Rejected)   | In-Process Viewport Layout (Recommended v1)                  |
+------------------------------------+--------------------------------------------------------------+
| - Rust engine computes (x,y,w,h)   | - Rust engine provides semantic weights & hierarchy only     |
| - Layout depends on UI width/height| - WPF / in-process module computes (x,y,w,h) partitions      |
| - IPC roundtrip on window resize   | - Instantaneous resize without IPC chatter                   |
| - Couples session host to display  | - Strict domain separation (Engine is headless & display-agnostic)|
+------------------------------------+--------------------------------------------------------------+
```

* **Decoupling Display from Domain:** The Rust engine provides semantic node hierarchies and weights (Allocated Size, Unique Size, Filtered status). The WPF presentation layer computes rectangle partitions against current viewport pixel dimensions, eliminating IPC traffic during window resizing and preserving headless engine purity.
---

## 5. UI Automation (UIA) Semantics & Assistive Technology

To satisfy legal accessibility mandates (Section 508, EN 301 549, WCAG 2.1 AA) and ensure seamless navigation with Windows Narrator, NVDA, and JAWS, custom WPF controls must expose standard Windows UI Automation Control Patterns.

### 5.1 Virtualized Tree-Table Automation Architecture
The custom tree-table control exposes a standard `AutomationPeer` hierarchy:

```
+---------------------------------------------------------------------------------------------------+
|                            Tree-Table UI Automation Peer Hierarchy                                |
+---------------------------------------------------------------------------------------------------+
|                                                                                                   |
|   TreeTableAutomationPeer : FrameworkElementAutomationPeer, IRawElementProviderFragmentRoot      |
|   Implements:                                                                                     |
|   - ITableProvider / IGridProvider (RowCount, ColumnCount, GetItem, GetRowHeaders)                |
|   - IItemContainerProvider (FindItemByProperty)                                                   |
|   - ISelectionProvider (GetSelection, CanSelectMultiple)                                          |
|                                                                                                   |
|         |-- GetChildrenCore() exposes ONLY realized visible rows in viewport                      |
|         v                                                                                         |
|   TreeTableRowAutomationPeer : UIElementAutomationPeer, IRawElementProviderFragment              |
|   - ControlType: ControlType.TreeItem or ControlType.DataItem                                     |
|   - Hierarchy Properties: Level, PositionInSet, SizeOfSet                                         |
|   - Loading State: ItemStatus ("Loading" -> "Loaded"), Name ("Loading..." -> Entity Name)         |
|   Implements:                                                                                     |
|   - ITableItemProvider / IGridItemProvider (Row, Column, RowSpan, ColumnSpan)                     |
|   - IExpandCollapseProvider (Expand, Collapse, ExpandCollapseState)                               |
|   - ISelectionItemProvider (Select, AddToSelection, RemoveFromSelection, IsSelected)              |
|   - IScrollItemProvider (ScrollIntoView)                                                          |
|                                                                                                   |
|         |-- Virtualized Placeholder (Returned ONLY via IItemContainerProvider)                    |
|         v                                                                                         |
|   TreeTableVirtualItemPeer : AutomationPeer, IRawElementProviderSimple                            |
|   Implements:                                                                                     |
|   - IVirtualizedItemProvider (Realize)  ===> Realizes IF cached; else throws ElementNotAvailable  |
|                                                                                                   |
+---------------------------------------------------------------------------------------------------+
```

#### Bounded UIA Virtualization & Search Protocol:
1. **`GetChildrenCore()` Boundary:** `GetChildrenCore()` returns **only realized peers** corresponding to rows currently instantiated in the `VirtualizingStackPanel` viewport. It must **never** instantiate or enumerate 5,000,000 peers during a standard UI automation tree walk.
2. **Accurate, Narrowly Defined `IItemContainerProvider` Scope:** Assistive technologies query container items via `IItemContainerProvider::FindItemByProperty(pStartAfter, propertyId, value)`. The WPF implementation handles this strictly within local, in-memory bounded structures:
   - **Exact `AutomationId` / Row Identity Lookup:** Resolved immediately when derivable from a stable row-position token (e.g. index) or small local tracking map of active nodes.
   - **Selection & Focus Properties:** `SelectionItemPattern.IsSelectedProperty` queries are resolved synchronously against WPF's local bounded selection/focus tracking sets (`HashSet<ulong> SelectedNodeIds`).
   - **`NameProperty` Scope Boundary:** `FindItemByProperty` searches `NameProperty` **only across currently realized / cached items in the sliding-window buffer**. If no cached match exists in the local buffer, `FindItemByProperty` immediately returns `null` (`pRetVal = NULL`).
   - **Full-Dataset Search Separation:** Full-dataset name, path, and metadata discovery is an explicit product-level Search/Filter operation executed by the out-of-process Rust engine that generates a new filtered projection index. It is **not masqueraded as a UIA `ItemContainer` linear scan over 5M unhydrated items**, completely eliminating cross-process Dispatcher stalls.
3. **Conservative `IVirtualizedItemProvider::Realize()` Contract:**
   - `Realize()` operates synchronously and deterministically: if the row's data is **already resident in the local sliding-window cache**, `Realize()` immediately materializes the peer into a full `TreeTableRowAutomationPeer`.
   - If the row data is **not cached**, `Realize()` throws a standard **`System.Windows.Automation.ElementNotAvailableException`** (mapping to Win32 `UIA_E_ELEMENTNOTAVAILABLE`), consistent with tested Windows UI Automation provider specifications for unavailable virtual items. `Realize()` does not make unsupported claims of asynchronous completion.
4. **Decoupled Viewport Positioning via `IScrollItemProvider::ScrollIntoView()`:**
   - Scrolling an item into view is handled independently via `IScrollItemProvider::ScrollIntoView()`.
   - `ScrollIntoView()` updates the virtual scroll position in the WPF control and triggers sliding-window page hydration, bringing the container into the realized visual viewport.

### 5.2 Treemap Canvas Automation Architecture
Because the treemap canvas is rendered via Direct3D/Direct2D, it contains no native WPF child visual elements. Accessibility is provided by generating a virtual spatial accessibility tree:

1. **`TreemapCanvasAutomationPeer`:** Derives from `FrameworkElementAutomationPeer` and implements `IGridProvider` and `IItemContainerProvider`.
2. **`TreemapCellAutomationPeer`:** Virtual peers representing visible treemap rectangles.
   - **Control Type:** `ControlType.DataItem` or `ControlType.Custom`.
   - **Supported Patterns:** `IInvokeProvider` (activates zoom/breadcrumb), `ISelectionItemProvider` (synchronizes selection with tree-table), and `IValueProvider` (reports formatted size and percentage).
   - **Bounding Rectangle:** `GetBoundingRectangle()` returns the physical screen coordinates of the treemap cell, enabling Narrator touch exploration and visual focus tracking.
3. **Screen Reader Spatial Navigation:** Up/Down/Left/Right arrow keys navigate the 2D spatial quadtree index, focusing adjacent treemap siblings and reading their labels.

### 5.3 Live Region Announcements & Scan Progress
During background scanning and aggregation, progress updates must not spam screen readers:
* The scan summary bar utilizes `AutomationProperties.LiveSetting = "Polite"`.
* Periodic milestone announcements (e.g., "Scan completed: 4.2 million items, 420 GB accounted") are raised via:
  ```csharp
  var peer = UIElementAutomationPeer.FromElement(ScanStatusTextBlock);
  peer?.RaiseAutomationEvent(AutomationEvents.LiveRegionChanged);
  ```

---

## 6. High Contrast, System Themes, and Visual Accessibility

### 6.1 Windows Contrast Themes Integration
Windows 10 and 11 feature modern Contrast Themes (**Aquatic**, **Desert**, **Dusk**, **Night Sky**, and legacy High Contrast Black/White). Applications must dynamically adapt without requiring a restart:

1. **Dynamic Resource Keys:** All tree-table borders, text colors, selection backgrounds, and header surfaces bind dynamically to system color keys:
   ```xml
   <SolidColorBrush x:Key="RowTextBrush" 
                    Color="{DynamicResource {x:Static SystemColors.WindowTextColorKey}}" />
   <SolidColorBrush x:Key="RowBackgroundBrush" 
                    Color="{DynamicResource {x:Static SystemColors.WindowColorKey}}" />
   <SolidColorBrush x:Key="SelectedHighlightBrush" 
                    Color="{DynamicResource {x:Static SystemColors.HighlightColorKey}}" />
   <SolidColorBrush x:Key="SelectedTextBrush" 
                    Color="{DynamicResource {x:Static SystemColors.HighlightTextColorKey}}" />
   ```
2. **System Theme Detection:** Listen for `SystemEvents.UserPreferenceChanged` and query `SystemParameters.HighContrast` (`SystemParametersInfo` with `SPI_GETHIGHCONTRAST`).

### 6.2 Treemap Accessibility Under High Contrast
In standard mode, treemaps communicate file classifications via subtle color hues and cushion gradients. Under High Contrast mode:

```
+---------------------------------------------------------------------------------------------------+
|                              Treemap High-Contrast Rendering Mode                                 |
+------------------------------------+--------------------------------------------------------------+
| Standard Full-Color Rendering      | High-Contrast Accessible Mode                                |
+------------------------------------+--------------------------------------------------------------+
| - Multi-hue 24-bit palette         | - High-contrast luminance-separated palette (>= 14:1 ratio)  |
| - Soft 3D cushion normal gradients | - Solid high-contrast borders (SystemColors.WindowTextColor) |
| - Anti-aliased subtle borders      | - 2px stark focus rectangles (SystemColors.HotTrackColor)    |
| - Small overlaid file text         | - DirectWrite high-legibility bold labels with solid backdrop|
| - Color-only file type distinction | - Distinct geometric line hatching patterns per file category |
+------------------------------------+--------------------------------------------------------------+
```

* **WCAG 2.1 AA Compliance:** Minimum text contrast ratio of **4.5:1** against backgrounds, and non-text visual boundary contrast of **3:1** against adjacent cells.
* **Hatching & Boundary Patterns:** When `SystemParameters.HighContrast` is `true`, the Direct3D 11 shader pipeline substitutes color gradients with high-contrast diagonal cross-hatching, stippling, and thick distinct borders.

---

## 7. Text Scaling & Per-Monitor DPI V2 Architecture

### 7.1 Windows Text Scaling (`TextScaleFactor`) vs Display DPI
Windows 10 (1809+) and Windows 11 introduce a dedicated **Make text bigger** accessibility setting that scales text independently of display DPI (from 100% up to 225%).
* **Display DPI:** Scales all UI elements (layout, controls, margins, images, text).
* **TextScaleFactor:** Scales only font sizes without enlarging fixed icon layouts.

The presentation layer monitors text scaling via `Windows.UI.ViewManagement.UISettings.TextScaleFactorChanged`.

### 7.2 Per-Monitor V2 DPI Configuration
WPF is configured in `app.manifest` for **PerMonitorV2** awareness:
```xml
<application xmlns="urn:schemas-microsoft-com:asm.v3">
  <windowsSettings>
    <dpiAwareness xmlns="http://schemas.microsoft.com/SMI/2016/WindowsSettings">PerMonitorV2, PerMonitor</dpiAwareness>
    <dpiAware xmlns="http://schemas.microsoft.com/SMI/2005/WindowsSettings">true/PM</dpiAware>
  </windowsSettings>
</application>
```

#### Treemap Direct3D DPI Handling:
On `WM_DPICHANGED` / `DpiChanged`:
1. The Treemap renderer resizes the underlying Direct3D 11 render target to exact physical pixels:
   $$\text{PhysicalWidth} = \lceil \text{LogicalWidth} \times \text{DpiScale.DpiScaleX} \rceil$$
   $$\text{PhysicalHeight} = \lceil \text{LogicalHeight} \times \text{DpiScale.DpiScaleY} \rceil$$
2. DirectWrite text layouts and glyph runs are recalculated with updated DIP scaling, ensuring sharp typography without bitmap scaling blur.

---

## 8. Alignment with In-Review IPC Architecture

The out-of-process IPC mechanism between the WPF GUI and the private Rust session host is currently under active engineering review. The presentation layer design aligns with the current IPC architectural direction:

1. **Command & Viewport Channel:** Length-prefixed framed binary stream over **Windows Named Pipes** (`\\.\pipe\pigtree-session-{UUID}`) using schema-versioned serialization (e.g., Protocol Buffers / FlatBuffers candidate) for queries, viewport slicing, and cancellation.
2. **Provisional Bulk Channel:** Shared memory (Anonymous Memory Mapped Files) remains under evaluation for bulk immutable snapshot transfers if serialization overhead exceeds reference budgets.
3. **Transport Deferral:** Specific framing formats and transport bindings are governed by the authoritative IPC decision rather than duplicated here.

---

## 9. Recommended v1 Production Architecture & Technical Seams

```
+---------------------------------------------------------------------------------------------------+
|                                Recommended v1 Production Stack                                    |
+--------------------------+------------------------------------------------------------------------+
| Subsystem Component      | Selected Technology & Architecture                                     |
+--------------------------+------------------------------------------------------------------------+
| **Target Runtime**       | **Latest Supported .NET LTS** (e.g. .NET 8 LTS or .NET 10 LTS)         |
| **Tree-Table Grid**      | **Customized WPF `ListView` with `VirtualizingStackPanel`**            |
| **Tree Virtualization**  | `VirtualizationMode="Recycling"`, `ScrollUnit="Pixel"`, Display text    |
| **Tree Memory Model**    | **Virtual Count + Bounded Sliding-Window Cache (< 500 KB active data)**|
| **Treemap Renderer**     | **`D3DImage`** hosting **Direct3D 11** / **Direct2D 1.1** Surface     |
| **Treemap Surface Handle**| **`D3D11_RESOURCE_MISC_SHARED`** legacy handle shared with D3D9Ex    |
| **Treemap Sync Pipeline**| **Complete Render -> D3D11 Flush() -> UI Thread D3DImage DirtyRect**   |
| **Treemap Layout Seam**  | **In-Process Presentation Layer** (Decoupled from session host)        |
| **Treemap Shading**      | Custom HLSL Pixel Shader (GPU procedural cushion shading)              |
| **Treemap Fallback**     | **`D3D_DRIVER_TYPE_WARP`** software rasterizer + Non-GPU tabular view |
| **Accessibility**        | Custom `AutomationPeer` with bounded `IItemContainerProvider` scope   |
| **UIA Search Scope**     | **Cached-Item Name Search + Bounded AutomationId / Selection Sets**    |
| **DPI & Scaling**        | **Per-Monitor V2 DPI** + `UISettings.TextScaleFactor` tracking       |
+--------------------------+------------------------------------------------------------------------+
```

---

## 10. Rejected Alternatives & Technical Justifications

### 1. In-Box WPF Recursive `TreeView`
* **Technical Reason for Rejection:** Instantiates recursive visual container trees (`TreeViewItem`) that cause O(N) memory allocations. At 5,000,000 entries, this causes catastrophic out-of-memory crashes (> 4 GB managed allocations) and disables horizontal column virtualization across multi-level hierarchies.
* **Replacement:** Linear Flattened Virtual Projection with `VirtualizingStackPanel.VirtualizationMode="Recycling"`.

### 2. Full 5M Descriptor / Name Array Allocation in WPF
* **Technical Reason for Rejection:** Allocating a contiguous 5,000,000-entry full descriptor array or global string arena in WPF managed memory causes severe heap bloat, GC pauses, and exceeds the $\le 150\text{ MiB}$ GUI working set budget.
* **Replacement:** Virtualized `Count` with a bounded sliding-window page cache (< 500 KB active data) and small local selection tracking sets.

### 3. Direct Win32 / DirectComposition `HwndHost` for Treemap
* **Technical Reason for Rejection:** Hosting an unmanaged Win32 window (`HWND`) inside WPF suffers from the permanent **WPF Airspace Defect**: the child HWND always paints on top of WPF elements, preventing native WPF context menus, tooltips, selection overlays, and flyout sheets from rendering over the treemap without brittle, transparent Win32 layered popup workarounds.
* **Replacement:** Direct3D 11 shared texture surface hosted via `System.Windows.Interop.D3DImage`, which integrates natively into WPF's composition pipeline without airspace clipping.

### 4. Pure WPF `DrawingVisual` / `OnRender` for Treemap
* **Technical Reason for Rejection:** WPF's retained-mode `DrawingVisual` serializes drawing instructions into MILCore command buffers on the UI thread. Rendering > 20,000 distinct rectangles with gradient cushion shading drops frame rates to 20–35 FPS and creates garbage collection churn during interactive resizing and panning.
* **Replacement:** Direct3D 11 / Direct2D GPU rendering via `D3DImage`.

### 5. Out-of-Process Treemap Pixel Layout in Session Host
* **Technical Reason for Rejection:** Computing pixel coordinates $(x, y, w, h)$ inside the Rust session host leaks viewport pixel dimensions and DPI scales across the process boundary, causing continuous IPC roundtrips during window resizing gestures.
* **Replacement:** In-process presentation-layer geometric layout calculation.

### 6. Full-Dataset UIA `ItemContainer` Scanning across IPC
* **Technical Reason for Rejection:** Executing cross-process synchronous IPC or full-dataset linear searches inside `FindItemByProperty` on the WPF Dispatcher thread stalls assistive technologies (Narrator / NVDA) and causes UI deadlocks during screen-reader exploration.
* **Replacement:** Bounded `ItemContainer` scope searching cached items and local selection IDs; full discovery is handled via the product Search/Filter bar.

### 7. WinRT XAML Islands (`WindowsXamlHost` / WinUI 3 Composition)
* **Technical Reason for Rejection:** Introduces significant runtime packaging dependencies (Windows App SDK runtime, DWriteCore, MRT Core), version pinning issues between .NET and WinAppSDK, and additional airspace/focus boundaries without offering rendering throughput advantages over native Direct3D 11 `D3DImage`.
* **Replacement:** Pure WPF on supported .NET LTS with `D3DImage` interop.

---

## 11. Engineering Risks, Failure Modes & Mitigations

| Identified Risk | Severity | Failure Mode | Mitigation Strategy |
| :--- | :--- | :--- | :--- |
| **DirectX Device Loss** | High | GPU driver reset or monitor sleep causes `D3DERR_DEVICELOST` or `DXGI_ERROR_DEVICE_REMOVED`, resulting in black treemap canvas. | Implement explicit device loss recovery in `D3DImage` host: catch device removal, release all D3D11/D3D9Ex texture handles, recreate devices, rebind back buffer, and re-upload cached geometry. |
| **UI Automation Dispatcher Stall** | High | Screen reader searches off-screen item, blocking UI thread on synchronous IPC. | Restrict `FindItemByProperty` to cached items and local selection sets; never perform blocking IPC during UIA property queries; return `null` / `ElementNotAvailableException` when un-cached. |
| **IPC Buffer Saturation on Rapid Scroll** | Medium | User flings scrollbar across 5M rows, flooding Named Pipe with range requests. | Implement request throttling/debouncing in WPF presentation layer: only latest viewport request is dispatched; obsolete in-flight requests are dropped. |
| **D3D11/D3D9Ex Driver Desynchronization** | Medium | GPU driver fails to synchronize shared surface commands despite `Flush()`, causing tearing. | Benchmark target GPUs; if tearing is observed, engage a double-buffered shared surface pair or staging copy fallback path. |
| **Per-Monitor DPI Visual Tearing** | Low | Window dragged across monitors with different DPIs causes momentary blur or clipping. | Handle `WM_DPICHANGED` synchronously; update Direct3D viewport and DirectWrite factory before triggering `AddDirtyRect`. |

---

## 12. Release Gates & Verification Test Plan

Before approving the presentation layer for production release, the implementation must pass the following verifiable automated and empirical gates:

```
+---------------------------------------------------------------------------------------------------+
|                                  Mandatory Release Gates (v1)                                     |
+------------------+-------------------------------------------------------------+------------------+
| Gate Category    | Verification Requirement                                    | Target / Floor   |
+------------------+-------------------------------------------------------------+------------------+
| **Scale Floor**  | Load and display an Analysis Snapshot with 5,000,000 nodes.   | **Zero crash**,  |
|                  | Verify memory stability over 30 minutes of continuous use.  | Working Set <300MB|
|                  | Verify base GUI process working set stays <= 150 MiB.       | Base GUI <=150MB |
+------------------+-------------------------------------------------------------+------------------+
| **Scroll Rate**  | Vertical scroll sweep across 5,000,000 rows at 1,000 px/s.   | **>= 60 FPS**    |
|                  | Measure frame times via Windows Performance Toolkit / ETW.  | Max drop < 33ms  |
+------------------+-------------------------------------------------------------+------------------+
| **Treemap Rate** | Continuous interactive zoom and pan of 50,000 treemap nodes.| **>= 60 FPS**    |
|                  | Measure GPU execution time per frame.                       | GPU time < 8.0ms |
+------------------+-------------------------------------------------------------+------------------+
| **Latency**      | Sort 5,000,000 rows by Allocated Size; filter by extension. | **<= 100 ms**    |
|                  | Measure time from user click to updated viewport render.    | p95 latency      |
+------------------+-------------------------------------------------------------+------------------+
| **UIA Search**   | Execute FindItemByProperty queries across cached/uncached.  | **0 IPC Calls**, |
|                  | Verify zero UI Dispatcher stalls and correct null returns.  | 0 UI Stalls      |
+------------------+-------------------------------------------------------------+------------------+
| **Accessibility**| Run Windows Accessibility Insights for Windows automated    | **0 Rule Violations**|
|                  | scan against Tree-Table and Treemap controls.               | Full UIA Pass    |
+------------------+-------------------------------------------------------------+------------------+
| **Screen Readers**| Complete full navigation workflows using Windows Narrator, | **Zero lockup**, |
|                  | NVDA, and JAWS across tree expansion and treemap cells.     | Correct speech   |
+------------------+-------------------------------------------------------------+------------------+
| **High Contrast**| Toggle Windows High Contrast (Contrast Themes) at runtime.  | Instant update,  |
|                  | Verify text contrast >= 4.5:1 and cell boundaries >= 3:1.   | WCAG 2.1 AA Pass |
+------------------+-------------------------------------------------------------+------------------+
```

---

## 13. Primary-Source Citations & References

1. **Microsoft Learn: What's new in WPF for .NET 9**  
   *URL:* [https://learn.microsoft.com/en-us/dotnet/desktop/wpf/whats-new/net90](https://learn.microsoft.com/en-us/dotnet/desktop/wpf/whats-new/net90)  
   *Citations:* Built-in Fluent theme (`ThemeMode`), modern Windows 11 aesthetics, performance and memory improvements.
2. **Microsoft Learn: Optimizing Performance: Controls and Virtualization (WPF)**  
   *URL:* [https://learn.microsoft.com/en-us/dotnet/desktop/wpf/advanced/optimizing-performance-controls](https://learn.microsoft.com/en-us/dotnet/desktop/wpf/advanced/optimizing-performance-controls)  
   *Citations:* `VirtualizingStackPanel`, `VirtualizationMode.Recycling`, container reuse, memory allocation reduction.
3. **Microsoft Learn: How to Improve the Scrolling Performance of a ListBox / ListView**  
   *URL:* [https://learn.microsoft.com/en-us/dotnet/desktop/wpf/controls/how-to-improve-the-scrolling-performance-of-a-listbox](https://learn.microsoft.com/en-us/dotnet/desktop/wpf/controls/how-to-improve-the-scrolling-performance-of-a-listbox)  
   *Citations:* Pixel-based scrolling (`ScrollUnit="Pixel"`), cache length configuration, content scrolling.
4. **Microsoft Learn: WPF and Direct3D9 Interoperation (`D3DImage`)**  
   *URL:* [https://learn.microsoft.com/en-us/dotnet/desktop/wpf/advanced/wpf-and-direct3d9-interoperation](https://learn.microsoft.com/en-us/dotnet/desktop/wpf/advanced/wpf-and-direct3d9-interoperation)  
   *Citations:* `D3DImage` architecture, `SetBackBuffer`, `AddDirtyRect`, Direct3D 9Ex sharing, dirty region synchronization.
5. **Microsoft Learn: Performance Considerations for Direct3D9 and WPF Interoperability**  
   *URL:* [https://learn.microsoft.com/en-us/dotnet/desktop/wpf/advanced/performance-considerations-for-direct3d9-and-wpf-interoperability](https://learn.microsoft.com/en-us/dotnet/desktop/wpf/advanced/performance-considerations-for-direct3d9-and-wpf-interoperability)  
   *Citations:* Hardware acceleration guidelines, WDDM shared surface handles, multi-monitor considerations.
6. **Microsoft Learn: `D3DImage.IsFrontBufferAvailable` Property**  
   *URL:* [https://learn.microsoft.com/en-us/dotnet/api/system.windows.interop.d3dimage.isfrontbufferavailable](https://learn.microsoft.com/en-us/dotnet/api/system.windows.interop.d3dimage.isfrontbufferavailable)  
   *Citations:* Front-buffer availability tracking, `IsFrontBufferAvailableChanged` event, screen-lock handling.
7. **Microsoft Learn: Using `DrawingVisual` Objects**  
   *URL:* [https://learn.microsoft.com/en-us/dotnet/desktop/wpf/graphics-multimedia/using-drawingvisual-objects](https://learn.microsoft.com/en-us/dotnet/desktop/wpf/graphics-multimedia/using-drawingvisual-objects)  
   *Citations:* Retained-mode visual layer, `VisualCollection`, lightweight rendering constraints, hit testing.
8. **Microsoft Learn: UI Automation TreeItem Control Type**  
   *URL:* [https://learn.microsoft.com/en-us/windows/win32/winauto/uiauto-supporttreeitemcontroltype](https://learn.microsoft.com/en-us/windows/win32/winauto/uiauto-supporttreeitemcontroltype)  
   *Citations:* `UIA_TreeItemControlTypeId`, `ExpandCollapsePattern`, `SelectionItemPattern`, `LevelProperty`, `PositionInSetProperty`.
9. **Microsoft Learn: UI Automation VirtualizedItem Control Pattern (`IVirtualizedItemProvider`)**  
   *URL:* [https://learn.microsoft.com/en-us/windows/win32/winauto/uiauto-implementingvirtualizeditem](https://learn.microsoft.com/en-us/windows/win32/winauto/uiauto-implementingvirtualizeditem)  
   *Citations:* `IVirtualizedItemProvider::Realize`, placeholder automation elements, de-virtualization on demand.
10. **Microsoft Learn: `IVirtualizedItemProvider.Realize` Method (WPF / .NET)**  
    *URL:* [https://learn.microsoft.com/en-us/dotnet/api/system.windows.automation.provider.ivirtualizeditemprovider.realize](https://learn.microsoft.com/en-us/dotnet/api/system.windows.automation.provider.ivirtualizeditemprovider.realize)  
    *Citations:* `Realize()` contract, converting placeholder to full element reference, synchronous behavior.
11. **Microsoft Learn: `ElementNotAvailableException` Class (WPF / .NET)**  
    *URL:* [https://learn.microsoft.com/en-us/dotnet/api/system.windows.automation.elementnotavailableexception](https://learn.microsoft.com/en-us/dotnet/api/system.windows.automation.elementnotavailableexception)  
    *Citations:* Standard UIA exception raised when target automation element or data is unavailable.
12. **Microsoft Learn: UI Automation ItemContainer Control Pattern (`IItemContainerProvider`)**  
    *URL:* [https://learn.microsoft.com/en-us/windows/win32/winauto/uiauto-implementingitemcontainer](https://learn.microsoft.com/en-us/windows/win32/winauto/uiauto-implementingitemcontainer)  
    *Citations:* `FindItemByProperty`, virtualized item lookups, programmatic element discovery without full tree enumeration.
13. **Microsoft Learn: Accessibility Best Practices (WPF / .NET)**  
    *URL:* [https://learn.microsoft.com/en-us/dotnet/framework/ui-automation/accessibility-best-practices](https://learn.microsoft.com/en-us/dotnet/framework/ui-automation/accessibility-best-practices)  
    *Citations:* Programmatic access, custom `AutomationPeer` guidelines, keyboard navigation, focus indications.
14. **Microsoft Learn: High-Contrast Mode & Theming Compatibility**  
    *URL:* [https://learn.microsoft.com/en-us/windows/win32/w8cookbook/high-contrast-mode](https://learn.microsoft.com/en-us/windows/win32/w8cookbook/high-contrast-mode)  
    *Citations:* `SystemParametersInfo` (`SPI_GETHIGHCONTRAST`), dynamic system colors, 14:1 high-contrast ratios.
15. **Microsoft Learn: High DPI Desktop Application Development on Windows & Per-Monitor V2**  
    *URL:* [https://learn.microsoft.com/en-us/windows/win32/hidpi/high-dpi-desktop-application-development-on-windows](https://learn.microsoft.com/en-us/windows/win32/hidpi/high-dpi-desktop-application-development-on-windows)  
    *Citations:* Per-Monitor V2 awareness, `WM_DPICHANGED`, non-client scaling, mixed-mode hosting.
16. **Microsoft Learn: DirectComposition Overview**  
    *URL:* [https://learn.microsoft.com/en-us/windows/win32/directcomp/directcomposition-overview](https://learn.microsoft.com/en-us/windows/win32/directcomp/directcomposition-overview)  
    *Citations:* Visual trees, DWM hardware-accelerated composition, independent animations, HWND target bindings.
