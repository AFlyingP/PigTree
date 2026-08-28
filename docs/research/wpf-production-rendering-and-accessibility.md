# Research Note: WPF Production Rendering, Tree-Table Virtualization, Hardware-Accelerated Treemap, and Accessibility Architecture

**Ticket:** Prerequisite for [AFlyingP/PigTree#14](https://github.com/AFlyingP/PigTree/issues/14) (Select the production technology architecture)  
**Date:** March 2025  
**Scope:** Authoritative engineering investigation and decision-ready architectural design for the PigTree Windows Presentation Foundation (WPF on .NET 8 / .NET 9) user interface. Covers dense virtualized hierarchical tree-table rendering, hardware-accelerated synchronized treemap visualization, UI Automation (UIA) custom peer semantics, high contrast and theme integration, text scaling and Per-Monitor DPI V2 awareness, out-of-process Rust engine IPC boundary considerations, rejected alternatives, risks, and measurable release gates tied to the 5,000,000-entry universal floor, 60 FPS rendering budget, and accessibility constraints.

---

## 1. Executive Summary & Production Decision Landscape

The accepted PigTree core architecture comprises a high-performance **Rust engine/workers subsystem**, a **WPF (.NET 8 / .NET 9) front-end**, and a **private, short-lived out-of-process Rust session host** providing isolation, privileged scanning coordination, and cross-interface reuse (GUI and CLI).

To satisfy the mandatory product performance targets ([docs/performance-targets.md](../performance-targets.md))—maintaining a target 60 FPS rendering rate and <= 100 ms interactive query/filter latency budgets at a scale floor of **5,000,000 Directory Entries** while delivering full WCAG 2.1 AA and Windows UI Automation accessibility—the production WPF presentation architecture evaluates and defines:

1. **Virtualized Hierarchical Tree-Table:** A **Flattened Virtual Projection Model** rendered via container recycling (`VirtualizingStackPanel.VirtualizationMode="Recycling"`), pixel scrolling (`ScrollUnit="Pixel"`), and a local nonblocking sliding-window cache that satisfies synchronous WPF `IList` indexer access without blocking the UI thread during IPC fetches. Built-in recursive WPF `TreeView` is rejected due to visual tree explosion.
2. **Hardware-Accelerated Treemap Visualization:** Direct3D 11 / Direct2D hardware rendering hosted seamlessly in WPF via **`System.Windows.Interop.D3DImage`** utilizing DXGI shared surface handles (`IDXGIResource::GetSharedHandle`) bound to Direct3D 9Ex. Cushion shading and gradient borders execute on the GPU via HLSL pixel shaders, completely avoiding WPF Airspace clipping bugs while maintaining 60–120+ FPS continuous panning, zooming, and resizing. Robust lifecycle management includes keyed mutex / double-buffering synchronization, `IsFrontBufferAvailableChanged` handling, explicit device loss recovery, WARP software rasterization, and an accessible non-GPU fallback.
3. **Seam Placement & Treemap Layout Ownership:** Challenging the necessity of out-of-process treemap pixel layout. Computing geometric partitions (x, y, w, h) inside the presentation layer avoids coupling the Rust engine to viewport pixel dimensions, eliminates IPC chatter on window resize, and preserves clear architectural boundaries.
4. **Comprehensive UI Automation (UIA) Semantics:** Custom `AutomationPeer` implementations utilizing standard Windows UIA primitives: `ControlType.TreeItem` / `ControlType.DataItem`, fragment navigation (`IRawElementProviderFragment`), `IExpandCollapseProvider`, `ISelectionItemProvider`, `IScrollItemProvider`, hierarchical properties (`Level`, `PositionInSet`, `SizeOfSet`), and critically **`IItemContainerProvider`** with **`IVirtualizedItemProvider`**. `GetChildrenCore()` exposes *only realized rows* (never allocating 5M peers), off-screen discovery is delegated to `ItemContainer`, and `Realize()` only materializes items while viewport scrolling is handled separately by `ScrollIntoView()`.
5. **Theme, High Contrast & Text Scaling:** Dynamic resource binding to system theme brushes (`SystemColors`), active query of `SystemParameters.HighContrast` (`SPI_GETHIGHCONTRAST`) to toggle high-contrast luminance palettes and structural border patterns, and full **Per-Monitor V2 DPI** manifest compliance paired with Windows text-scaling factor (`UISettings.TextScaleFactor`) tracking.
6. **Alignment with In-Review IPC Architecture:** The presentation layer interface aligns with the emerging IPC direction (framed named pipes with schema-versioned serialization, provisional shared-memory bulk buffers), deferring binding transport specifics to the authoritative IPC decision.
```
+---------------------------------------------------------------------------------------------------+
|                                   WPF Production UI Architecture                                  |
+---------------------------------------------------------------------------------------------------+
|                                                                                                   |
|   +---------------------------------------+       +-------------------------------------------+   |
|   |   Dense Virtualized Tree-Table        |       |   Hardware-Accelerated Treemap Canvas     |   |
|   |   - Flattened Projection (1D List)    |       |   - D3DImage (DXGI Shared Handle)         |   |
|   |   - VirtualizingStackPanel (Recycle)  |       |   - Direct3D 11 / Direct2D Render Target  |   |
|   |   - Nonblocking Sliding-Window Cache  |       |   - GPU Cushion Shaders / DirectWrite     |   |
|   |   - IItemContainer / IVirtualized UIA |       |   - Zero Airspace Defect (WPF Blended)    |   |
|   |   - Pixel Scrolling & Display Text    |       |   - Keyed Mutex / FrontBuffer Recovery    |   |
|   +---------------------------------------+       +-------------------------------------------+   |
|                       ^                                                 ^                         |
|                       | (Synchronous IList Window Slices)               | (Local Viewport Layout) |
|                       v                                                 v                         |
|   +-------------------------------------------------------------------------------------------+   |
|   |   WPF Presentation Model & IPC Client Layer (C# / .NET 8 LTS or .NET 9 STS)               |   |
|   |   - HighContrast / Theme Monitor       - Per-Monitor V2 DPI & UISettings Scaler           |   |
|   |   - Bi-directional Node Selection Bus  - Narrator / NVDA UIA Fragment Peer Dispatcher     |   |
|   |   - Sliding-Window Page Buffer         - In-Process Squarified Treemap Partition Engine   |   |
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

Per [PigTree Domain Architecture (CONTEXT.md)](../../CONTEXT.md) and [Product Performance Targets (docs/performance-targets.md)](../performance-targets.md), the rendering and presentation subsystem must operate within strict deterministic limits. All performance figures below represent **normative design budgets and modeled engineering projections** to be validated in formal benchmarking:

| Constraint Dimension | Normative Reference Budget | Universal Release Floor Gate | Verification Mode |
| :--- | :--- | :--- | :--- |
| **Dataset Scale Boundary** | 1,000,000 Directory Entries (Standard) | **5,000,000 Directory Entries** (Stress boundary: 10M) | Automated scale fixture |
| **Interactive Latency** | <= 100 ms (p95) for sort, filter, expand | <= 150 ms (p99) under active background scan | ETW trace click-to-render |
| **Tree-Table Scroll Frame Rate** | **60 FPS** sustained (<= 16.6 ms frame budget) | No frame drop > 33.3 ms (30 FPS transient floor) | DWM frame presentation clock |
| **Treemap Render Frame Rate** | **60 FPS** during pan/zoom/hover hit-test | GPU render pass <= 8.0 ms; CPU prep <= 5.0 ms | GPU performance counter query |
| **UI Process Memory Footprint** | <= 150 MB Managed Working Set (WPF GUI) | <= 300 MB Peak Working Set at 5M entries | Process working set telemetry |
| **Assistive Tech Realization** | <= 50 ms to realize virtualized UIA item | Narrator/NVDA must not cause UI thread freeze | Automated UIA client harness |
| **DPI / Display Scale Transitions** | Crisp text, zero blur, zero visual artifact | Instantaneous re-rasterization on `WM_DPICHANGED` | Visual diff & snapshot audit |

---

## 3. Virtualized Hierarchical Tree-Table Architecture

### 3.1 Failure Analysis of Standard WPF TreeView at Scale
Standard WPF `TreeView` controls (e.g., `System.Windows.Controls.TreeView`) instantiate a recursive hierarchy of `TreeViewItem` containers. In benchmarked WPF implementations, hosting > 50,000 hierarchical nodes causes severe degradation:
1. **Visual Tree Explosion:** Each realized `TreeViewItem` introduces 10 to 18 underlying `Visual` elements (`Border`, `ToggleButton`, `ContentPresenter`, `ItemsPresenter`, `StackPanel`). At 5,000,000 entries, naive instantiations would require over 60 million `Visual` instances, exceeding 32-bit GDI/user object limits and consuming multiple gigabytes of managed memory.
2. **Recursive Virtualization Breakdown:** While WPF's `VirtualizingStackPanel` can virtualize top-level items, nested `TreeView` levels require nested `VirtualizingStackPanel` instances. Scrolling vertically requires evaluating nested layout measurements, creating UI thread layout stalls.
3. **Lack of Column Virtualization:** A standard `TreeView` does not natively align multi-column tabular data (Allocated Size, Unique Size, File Counts, % Bars, Timestamps, Coverage Gaps) with horizontal virtualization and header resizing across disparate branch depths.

### 3.2 The Flattened Virtual Projection Model
To achieve O(1) scrolling and rendering overhead regardless of whether the dataset contains 10,000 or 5,000,000 nodes, the presentation architecture separates the **hierarchical graph model** from the **linear visual projection**:

```
+---------------------------------------------------------------------------------------------------+
|                                 Flattened Virtual Projection Model                                |
+---------------------------------------------------------------------------------------------------+
|                                                                                                   |
|   Hierarchical Graph (Rust Engine)              Flattened Visible Projection (WPF Virtual View)   |
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

#### Structural Mechanics & Memory Sizing:
* **Linear Array of Visible Rows:** Only expanded, visible nodes occupy an index in the projected array. Collapsed subtrees are omitted from the projection.
* **Compact Row Descriptor:** Each projected row is described by a fixed-size struct:
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
* **Memory Footprint Arithmetic:** At the universal release floor of 5,000,000 entries, holding a complete 1D array of `VirtualRowDescriptor` in memory requires 5,000,000 * 40 bytes = 200,000,000 bytes (~190.7 MiB). When combined with windowed paging where the WPF process only retains active viewport slices, the managed working set remains well below the <= 150 MB budget.

### 3.3 Synchronous WPF IList Indexing & Nonblocking Sliding-Window Cache

WPF's `VirtualizingStackPanel` requires synchronous element access via `IList[int index]` during its measurement and layout passes. If the requested row is not locally available, the indexer **must not perform synchronous, blocking IPC calls** on the WPF UI thread, which would produce deadlocks and catastrophic UI jank.

#### Sliding-Window Architecture:
1. **Sliding-Window Buffer:** The presentation model maintains a localized sliding window of realized row data (e.g., N +/- 250 rows centered on the current scroll position).
2. **Immediate Synchronous Placeholder Return:** When `IList[int index]` experiences a cache miss during rapid scrollbar flings:
   - The indexer synchronously returns a lightweight **placeholder descriptor** marked with `IsPlaceholder = true` and default placeholder text (e.g. "Loading...").
   - The UI immediately renders a shimmer / skeleton row container without stalling the layout engine.
3. **Asynchronous Prefetch Dispatch:** The cache miss triggers an asynchronous range prefetch request over IPC for the missing page window.
4. **Non-Destructive Container Realization:** When the batch arrives from the Rust engine, the sliding window updates its entries and raises `PropertyChanged` notifications on the realized container view models, smoothly populating the row contents without triggering a full collection reset.
---

## 4. Hardware-Accelerated Synchronized Treemap Architecture

### 4.1 Comparative Evaluation of Treemap Rendering Options

Rendering an interactive, dense hierarchical treemap containing 10,000–50,000 visible geometric rectangles with cushion shading, category palettes, selection outlines, and sub-millisecond hover hit-testing requires evaluated graphics primitives:

| Dimension | Option A: WPF `DrawingVisual` / `OnRender` | Option B: Direct3D 11 / Direct2D via `D3DImage` | Option C: DirectComposition / `HwndHost` | Option D: WinRT XAML Islands (`Windows.UI.Composition`) |
| :--- | :--- | :--- | :--- | :--- |
| **Pipeline Architecture** | Retained MILCore command stream (Direct3D 9Ex) | Direct3D 11 surface shared with Direct3D 9Ex | Separate Win32 child HWND with DXGI swapchain | WinRT container hosting WinUI 3 Composition visual |
| **Sustained Frame Rate (Modeled)** | 30–45 FPS at 20k rectangles (CPU command bottleneck) | **60–120+ FPS** (Fully GPU accelerated) | **60–120+ FPS** (Direct DWM swapchain) | 60 FPS (Composition engine) |
| **WPF Airspace Defect** | **None** (Native WPF Visual tree) | **None** (Blended into WPF composition pipeline) | **Severe Airspace Bug** (HWND clips all WPF popups/tooltips) | Moderate (Requires XAML Island boundary management) |
| **Cushion Treemap Shading** | CPU pre-baked bitmaps or radial gradient brushes | **HLSL Pixel Shader** (Hardware procedural evaluation) | Direct2D / HLSL Pixel Shader | Win2D / Composition Pixel Shaders |
| **Hover Hit-Testing Latency (Modeled)** | ~5 to 15 ms (`VisualTreeHelper.HitTest`) | **<= 0.2 ms** (CPU Quadtree / GPU pick buffer) | <= 0.2 ms (CPU spatial index) | ~1 to 5 ms |
| **Per-Monitor DPI & Scaling** | Automatic WPF scaling | Direct surface resizing on `WM_DPICHANGED` | Window message synchronization required | Automatic WinRT DPI handling |
| **Device Loss Recovery** | Handled internally by WPF MILCore | Explicit `D3DERR_DEVICELOST` / `DXGI_ERROR_DEVICE_REMOVED` | Explicit device recreation | Handled by WinRT composition |

### 4.2 Production Direct3D 11 / `D3DImage` Interoperability Pipeline

The recommended architecture utilizes **`System.Windows.Interop.D3DImage`** hosting an off-screen **Direct3D 11** render target.

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
|                        - Keyed Mutex Synchronization (IDXGIKeyedMutex)                            |
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
|   |   - ID3D11Texture2D (D3D11_RESOURCE_MISC_SHARED_KEYEDMUTEX)                               |   |
|   |   - Direct2D 1.1 Device Context / DirectWrite Font Layouts                                |   |
|   |   - HLSL Pixel Shader: Fast Procedural Cushion Treemap Shading                            |   |
|   |   - Instance Buffer: 50,000 Rectangles (2.0 MB) uploaded via dynamic GPU buffer           |   |
|   |   - Fallback: D3D_DRIVER_TYPE_WARP Software Rasterizer                                     |   |
|   +-------------------------------------------------------------------------------------------+   |
|                                                                                                   |
+---------------------------------------------------------------------------------------------------+
```

#### Keyed Mutex & Generation Synchronization:
To prevent screen tearing and race conditions between the Direct3D 11 render engine and WPF's Direct3D 9Ex compositor:
1. Create the shared `ID3D11Texture2D` with `D3D11_RESOURCE_MISC_SHARED_KEYEDMUTEX`.
2. Query `IDXGIKeyedMutex` from the Direct3D 11 texture and Direct3D 9Ex shared texture.
3. Direct3D 11 acquires mutex with key `0` (`pKeyedMutex->AcquireSync(0, INFINITE)`), renders geometry/shaders, and releases mutex with key `1` (`pKeyedMutex->ReleaseSync(1)`).
4. WPF's composition loop locks `D3DImage`, acquires key `1`, signals `AddDirtyRect()`, releases key `0`, and unlocks `D3DImage`.

#### Robust Lifecycle & Failure Recovery:
* **Front-Buffer Availability Handling:** When the user locks Windows (Ctrl+Alt+Del), switches users, or initiates full-screen transitions, WPF raises **`D3DImage.IsFrontBufferAvailableChanged`**. The renderer listens to this event, immediately pauses active render dispatching while `IsFrontBufferAvailable` is `false`, and invalidates/recreates back buffers once `true`.
* **DirectX Device Loss Recovery:** If a GPU driver reset occurs (`D3DERR_DEVICELOST`, `DXGI_ERROR_DEVICE_REMOVED`, or `DXGI_ERROR_DEVICE_RESET`), the renderer catches the error, releases all outstanding D3D11 texture interfaces and D3D9Ex pointers, reinitializes the device factories, recreates the shared texture, and calls `D3DImage.SetBackBuffer()`.
* **WARP & Non-GPU Fallback:** If hardware GPU initialization fails (e.g. in restricted virtualized environments or remote desktop sessions without GPU passthrough), the engine attempts initialization using **`D3D_DRIVER_TYPE_WARP`** (Direct3D 11 high-speed CPU rasterizer). If WARP is unavailable, the UI gracefully falls back to an accessible non-GPU tabular view.

### 4.3 Challenging the Seam: Treemap Layout Ownership

An important architectural question is whether the out-of-process Rust engine should compute pixel-space treemap rectangle coordinates (x, y, w, h).

```
+---------------------------------------------------------------------------------------------------+
|                            Treemap Layout Ownership Comparison                                    |
+------------------------------------+--------------------------------------------------------------+
| Option A: Out-of-Process Layout     | Option B: In-Process Viewport Layout (Recommended)           |
+------------------------------------+--------------------------------------------------------------+
| - Rust engine computes (x,y,w,h)   | - Rust engine provides semantic weights & hierarchy only     |
| - Layout depends on UI width/height| - WPF / in-process module computes (x,y,w,h) partitions      |
| - IPC roundtrip on window resize   | - Instantaneous resize without IPC chatter                   |
| - Couples session host to display  | - Strict domain separation (Engine is headless & display-agnostic)|
+------------------------------------+--------------------------------------------------------------+
```

#### Architectural Assessment:
* **Coupling & IPC Overhead:** Having the Rust engine compute (x, y, w, h) bounding boxes requires transmitting window dimensions, DPI scales, and aspect ratios across the IPC boundary whenever the user resizes the window, creating unnecessary IPC roundtrips.
* **Recommended Seam:** The Rust engine provides the semantic hierarchy and node metrics (Allocated Size, Unique Size, Filtered status). The WPF presentation layer (or an in-process native C#/C++ layout helper) computes the squarified treemap geometric partition directly against local viewport bounds. This maintains clean architectural separation and ensures the Rust session host remains entirely headless and display-agnostic.
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
|   - IVirtualizedItemProvider (Realize)  ===> Realizes item into data model (No auto-scroll)       |
|                                                                                                   |
+---------------------------------------------------------------------------------------------------+
```

#### Strict Virtualization Protocol (Preventing 5M-Peer Enumeration):
1. **`GetChildrenCore()` Boundary:** `GetChildrenCore()` returns **only realized peers** corresponding to rows currently instantiated in the `VirtualizingStackPanel`. It must **never** instantiate or enumerate 5,000,000 peers during a standard UI automation tree walk.
2. **Off-Screen Item Search via `IItemContainerProvider`:** When assistive technologies search for off-screen rows (e.g. by name or automation ID), they invoke `IItemContainerProvider::FindItemByProperty(pStartAfter, propertyId, value)`.
3. **Placeholder Returning:** If the searched item is off-screen, `FindItemByProperty` returns a lightweight `TreeTableVirtualItemPeer` placeholder implementing `IRawElementProviderSimple` and `IVirtualizedItemProvider`.
4. **De-virtualization via `IVirtualizedItemProvider::Realize()`:** Calling `Realize()` materializes the data for the virtualized item into the presentation collection. **`Realize()` does not scroll the viewport.** Bringing the row into view is explicitly handled as a separate step via `IScrollItemProvider::ScrollIntoView()`.

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

## 9. Decision Options for Production Grilling

To support the architecture selection milestone ([AFlyingP/PigTree#14](https://github.com/AFlyingP/PigTree/issues/14)), key presentation choices are framed with explicit tradeoffs:

### 9.1 Target Runtime Framework: .NET 8 LTS vs .NET 9 STS
* **Option A: .NET 8 LTS (Long Term Support)**
  - *Strengths:* 3-year support lifecycle (supported through November 2026); established enterprise deployment baseline; fully mature WPF runtime.
  - *Tradeoffs:* Requires manual inclusion of modern Windows 11 Fluent styling resource dictionaries; lacks built-in `ThemeMode` API.
* **Option B: .NET 9 STS (Standard Term Support - Recommended for Prototype/v1)**
  - *Strengths:* Built-in Windows 11 Fluent theme via `ThemeMode="System"`; improved Per-Monitor DPI V2 non-client area handling; updated DirectWrite integration; experimental Native AOT packaging improvements.
  - *Tradeoffs:* Shorter 18-month support window (requires upgrading to .NET 10 LTS in late 2025).

### 9.2 Tree-Table UI Implementation Strategy
* **Option A: Customized `ListView` / `GridView` with `VirtualizingStackPanel` (Recommended)**
  - *Strengths:* Standard WPF control surface; mature container recycling (`VirtualizationMode.Recycling`); native keyboard navigation and accessibility peer integration; low implementation complexity.
  - *Tradeoffs:* Requires flattened projection view model and custom indent templates.
* **Option B: Fully Custom `VirtualizingPanel` with Direct `DrawingVisual` / Text Formatting**
  - *Strengths:* Maximal layout control; eliminates intermediate `ListViewItem` container allocations.
  - *Tradeoffs:* Substantial engineering overhead; must manually re-implement focus navigation, column resizing, mouse tracking, and full UI Automation provider pattern hierarchy.

### 9.3 Treemap Geometric Layout Computation Placement
* **Option A: In-Process Presentation Layer (Recommended)**
  - *Strengths:* Decoupled from session host; zero IPC roundtrips on window resize or DPI changes; Rust engine remains headless and display-agnostic.
  - *Tradeoffs:* Computes squarified partitions in C# (or an in-process native helper) on the UI client.
* **Option B: Out-of-Process Rust Session Host**
  - *Strengths:* Leverages multi-threaded Rust Rayon partition engine.
  - *Tradeoffs:* Leaks viewport pixel dimensions into session host; generates continuous IPC requests during window resize gestures.

---

## 10. Rejected Alternatives & Technical Justifications

### 1. In-Box WPF Recursive `TreeView`
* **Technical Reason for Rejection:** Instantiates recursive visual container trees (`TreeViewItem`) that cause O(N) memory allocations. At 5,000,000 entries, this causes catastrophic out-of-memory crashes (> 4 GB managed allocations) and disables horizontal column virtualization across multi-level hierarchies.
* **Replacement:** Linear Flattened Virtual Projection with `VirtualizingStackPanel.VirtualizationMode="Recycling"`.

### 2. Direct Win32 / DirectComposition `HwndHost` for Treemap
* **Technical Reason for Rejection:** Hosting an unmanaged Win32 window (`HWND`) inside WPF suffers from the permanent **WPF Airspace Defect**: the child HWND always paints on top of WPF elements, preventing native WPF context menus, tooltips, selection overlays, and flyout sheets from rendering over the treemap without brittle, transparent Win32 layered popup workarounds.
* **Replacement:** Direct3D 11 shared texture surface hosted via `System.Windows.Interop.D3DImage`, which integrates natively into WPF's composition pipeline without airspace clipping.

### 3. Pure WPF `DrawingVisual` / `OnRender` for Treemap
* **Technical Reason for Rejection:** WPF's retained-mode `DrawingVisual` serializes drawing instructions into MILCore command buffers on the UI thread. Rendering > 20,000 distinct rectangles with gradient cushion shading drops frame rates to 20–35 FPS and creates garbage collection churn during interactive resizing and panning.
* **Replacement:** Direct3D 11 / Direct2D GPU rendering via `D3DImage`.

### 4. WinRT XAML Islands (`WindowsXamlHost` / WinUI 3 Composition)
* **Technical Reason for Rejection:** Introduces significant runtime packaging dependencies (Windows App SDK runtime, DWriteCore, MRT Core), version pinning issues between .NET and WinAppSDK, and additional airspace/focus boundaries without offering rendering throughput advantages over native Direct3D 11 `D3DImage`.
* **Replacement:** Pure WPF .NET 9 with `D3DImage` interop.

### 5. Chromium / WebView2 Canvas Treemap
* **Technical Reason for Rejection:** Introduces large memory overhead (> 150 MB base WebView2 runtime), complex multi-process IPC hops (Rust -> C# -> WebView2 renderer process), and high serialization latency for 5M-node datasets across the web-message boundary.
* **Replacement:** In-process Direct3D 11 GPU rendering via `D3DImage`.

---

## 11. Engineering Risks, Failure Modes & Mitigations

| Identified Risk | Severity | Failure Mode | Mitigation Strategy |
| :--- | :--- | :--- | :--- |
| **DirectX Device Loss** | High | GPU driver reset or monitor sleep causes `D3DERR_DEVICELOST` or `DXGI_ERROR_DEVICE_REMOVED`, resulting in black treemap canvas. | Implement explicit device loss recovery in `D3DImage` host: catch device removal, release all D3D11/D3D9Ex texture handles, recreate devices, rebind back buffer, and re-upload cached geometry. |
| **UI Automation Freeze at 5M Nodes** | High | Screen reader (Narrator/NVDA) attempts full tree walk, causing UI thread lockup. | Implement `IItemContainerProvider` and `IVirtualizedItemProvider` strictly returning placeholder peers; do not instantiate full peers for off-screen items until `Realize()` is called. |
| **IPC Buffer Saturation on Rapid Scroll** | Medium | User flings scrollbar across 5M rows, flooding Named Pipe with range requests. | Implement request throttling/debouncing in WPF presentation layer: only latest viewport request is dispatched; obsolete in-flight requests are dropped. |
| **Shared Memory Handle Leak** | Medium | Abnormal termination of Rust engine leaves dangling memory map handles. | Use anonymous shared memory with process-lifetime binding; ensure WPF monitors Rust child process handle (`Process.Exited`) and tears down memory views immediately. |
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
| **Accessibility**| Run Windows Accessibility Insights for Windows automated    | **0 Rule Violations**|
|                  | scan against Tree-Table and Treemap controls.               | Full UIA Pass    |
+------------------+-------------------------------------------------------------+------------------+
| **Screen Reader**| Complete full navigation workflow using Windows Narrator    | **Zero lockup**, |
|                  | and NVDA across tree-table expansion and treemap cells.     | Correct speech   |
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
7. **Microsoft Learn: `IDXGIKeyedMutex` Interface**  
   *URL:* [https://learn.microsoft.com/en-us/windows/win32/api/dxgi/nn-dxgi-idxgikeyedmutex](https://learn.microsoft.com/en-us/windows/win32/api/dxgi/nn-dxgi-idxgikeyedmutex)  
   *Citations:* `AcquireSync`, `ReleaseSync`, cross-device Direct3D shared resource synchronization.
8. **Microsoft Learn: Using `DrawingVisual` Objects**  
   *URL:* [https://learn.microsoft.com/en-us/dotnet/desktop/wpf/graphics-multimedia/using-drawingvisual-objects](https://learn.microsoft.com/en-us/dotnet/desktop/wpf/graphics-multimedia/using-drawingvisual-objects)  
   *Citations:* Retained-mode visual layer, `VisualCollection`, lightweight rendering constraints, hit testing.
9. **Microsoft Learn: UI Automation TreeItem Control Type**  
   *URL:* [https://learn.microsoft.com/en-us/windows/win32/winauto/uiauto-supporttreeitemcontroltype](https://learn.microsoft.com/en-us/windows/win32/winauto/uiauto-supporttreeitemcontroltype)  
   *Citations:* `UIA_TreeItemControlTypeId`, `ExpandCollapsePattern`, `SelectionItemPattern`, `LevelProperty`, `PositionInSetProperty`.
10. **Microsoft Learn: UI Automation VirtualizedItem Control Pattern (`IVirtualizedItemProvider`)**  
    *URL:* [https://learn.microsoft.com/en-us/windows/win32/winauto/uiauto-implementingvirtualizeditem](https://learn.microsoft.com/en-us/windows/win32/winauto/uiauto-implementingvirtualizeditem)  
    *Citations:* `IVirtualizedItemProvider::Realize`, placeholder automation elements, de-virtualization on demand.
11. **Microsoft Learn: UI Automation ItemContainer Control Pattern (`IItemContainerProvider`)**  
    *URL:* [https://learn.microsoft.com/en-us/windows/win32/winauto/uiauto-implementingitemcontainer](https://learn.microsoft.com/en-us/windows/win32/winauto/uiauto-implementingitemcontainer)  
    *Citations:* `FindItemByProperty`, virtualized item lookups, programmatic element discovery without full tree enumeration.
12. **Microsoft Learn: Accessibility Best Practices (WPF / .NET)**  
    *URL:* [https://learn.microsoft.com/en-us/dotnet/framework/ui-automation/accessibility-best-practices](https://learn.microsoft.com/en-us/dotnet/framework/ui-automation/accessibility-best-practices)  
    *Citations:* Programmatic access, custom `AutomationPeer` guidelines, keyboard navigation, focus indications.
13. **Microsoft Learn: High-Contrast Mode & Theming Compatibility**  
    *URL:* [https://learn.microsoft.com/en-us/windows/win32/w8cookbook/high-contrast-mode](https://learn.microsoft.com/en-us/windows/win32/w8cookbook/high-contrast-mode)  
    *Citations:* `SystemParametersInfo` (`SPI_GETHIGHCONTRAST`), dynamic system colors, 14:1 high-contrast ratios.
14. **Microsoft Learn: High DPI Desktop Application Development on Windows & Per-Monitor V2**  
    *URL:* [https://learn.microsoft.com/en-us/windows/win32/hidpi/high-dpi-desktop-application-development-on-windows](https://learn.microsoft.com/en-us/windows/win32/hidpi/high-dpi-desktop-application-development-on-windows)  
    *Citations:* Per-Monitor V2 awareness, `WM_DPICHANGED`, non-client scaling, mixed-mode hosting.
15. **Microsoft Learn: DirectComposition Overview**  
    *URL:* [https://learn.microsoft.com/en-us/windows/win32/directcomp/directcomposition-overview](https://learn.microsoft.com/en-us/windows/win32/directcomp/directcomposition-overview)  
    *Citations:* Visual trees, DWM hardware-accelerated composition, independent animations, HWND target bindings.
