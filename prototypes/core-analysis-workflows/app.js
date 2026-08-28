// PigTree Core Analysis Workflows - Prototype Script
// Dependency-free single-route interactive prototype
// Conforms to CONTEXT.md domain specifications

import {
  PRESETS,
  MOCK_VOLUMES,
  HISTORICAL_SNAPSHOTS,
  MOCK_OBJECTS,
  MOCK_TREE_ROOT,
  MOCK_TREE_ROOT_D,
  MOCK_TREE_DOWNLOADS_HISTORICAL,
  RECONCILIATION_ITEM,
  RECONCILIATION_ITEM_D,
  HARDLINK_ALIASES
} from './mock-data.js';

// Global In-Memory Prototype State
const state = {
  variant: 'explorer', // 'explorer' | 'insights' | 'workbench'
  activeTargetType: 'volume', // 'volume' | 'directory' | 'historical'
  activeVolumeId: 'vol_c',
  activeDirectoryPath: 'C:\\Users\\Alex\\Downloads',
  activeSnapshotId: null,
  activePresetId: 'standard',
  
  // Browsing scope state (Directory currently displayed in Folder Analysis)
  browsedDirectoryId: 'node_root',
  
  // Scan execution state
  scanStatus: 'finished', // 'idle' | 'scanning' | 'finished' | 'cancelled'
  scanProgress: 100,
  scanEntriesObserved: 48210,
  scanRate: 24500, // entries/sec
  
  // Sorting state
  sortField: 'uniqueAllocatedBytes', // 'name' | 'uniqueAllocatedBytes' | 'referencedAllocatedBytes' | 'uniqueLogicalBytes' | 'entryCount' | 'modifiedTime'
  sortDirection: 'desc', // 'asc' | 'desc'

  // Active selection & view
  selectedNodeId: 'node_alex_win11_iso',
  currentViewTab: 'table', // 'table' | 'flat' | 'treemap' | 'types' | 'age' | 'largest'
  treemapShowTextEquivalent: false,
  
  // Filter states
  filterSearch: '',
  filterSizeMin: 0, // 0 = all
  filterType: 'all', // 'all' | 'apps' | 'archives' | 'games' | 'media' | 'system'
  filterAge: 'all', // 'all' | '7d' | '30d' | '1y' | 'older_1y'
  
  // Expanded tree node IDs
  expandedNodes: new Set(['node_root', 'node_users', 'node_user_alex', 'node_alex_downloads', 'node_root_d', 'node_d_backups', 'node_snap_downloads_root']),
  
  // Modals
  cleanupModalOpen: false,
  cleanupTargetNode: null,
  coverageGapModalOpen: false,
  modalTriggerElement: null,
  
  // Responsive inspector drawer
  inspectorDrawerOpen: false,
  
  // Debug prototype state drawer
  statePanelOpen: false,
  lastAction: 'Initialized prototype'
};

// Format utilities
function formatBytes(bytes) {
  if (bytes === null || bytes === undefined || isNaN(bytes)) return 'Unavailable';
  if (bytes === 0) return '0 B';
  const k = 1024;
  const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return (bytes / Math.pow(k, i)).toFixed(1) + ' ' + sizes[i];
}

function formatDate(isoString) {
  if (!isoString) return 'Unavailable';
  try {
    const d = new Date(isoString);
    if (isNaN(d.getTime())) return isoString;
    return d.toLocaleDateString() + ' ' + d.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
  } catch (e) {
    return isoString;
  }
}

// Flat list collector for searches & tables
function getAllFlatNodes(root) {
  const list = [];
  function traverse(node) {
    if (!node) return;
    list.push(node);
    if (node.children) {
      for (const child of node.children) {
        traverse(child);
      }
    }
  }
  traverse(root);
  return list;
}

// Active scan target root calculation
function getActiveTargetRoot() {
  if (state.activeTargetType === 'volume') {
    if (state.activeVolumeId === 'vol_d') return MOCK_TREE_ROOT_D;
    return MOCK_TREE_ROOT;
  }
  if (state.activeTargetType === 'historical') {
    if (state.activeSnapshotId === 'snap_downloads_archive') return MOCK_TREE_DOWNLOADS_HISTORICAL;
    return MOCK_TREE_ROOT;
  }
  if (state.activeTargetType === 'directory') {
    if (state.activeSnapshotId === 'snap_downloads_archive') return MOCK_TREE_DOWNLOADS_HISTORICAL;
    const downloadsNode = findNodeByIdInRoot('node_alex_downloads', MOCK_TREE_ROOT);
    return downloadsNode || MOCK_TREE_ROOT;
  }
  return MOCK_TREE_ROOT;
}

function getActiveReconciliationItem() {
  if (state.activeTargetType === 'directory') return null;
  if (state.activeSnapshotId === 'snap_downloads_archive') return null;
  if (state.activeVolumeId === 'vol_d') return RECONCILIATION_ITEM_D;
  return RECONCILIATION_ITEM;
}

// Find node by ID across active and auxiliary trees
function findNodeById(id) {
  if (!id) return null;
  const recon = getActiveReconciliationItem();
  if (recon && id === recon.id) return recon;
  if (id === RECONCILIATION_ITEM.id) return RECONCILIATION_ITEM;
  if (id === RECONCILIATION_ITEM_D.id) return RECONCILIATION_ITEM_D;
  
  const root = getActiveTargetRoot();
  const foundInActive = findNodeByIdInRoot(id, root);
  if (foundInActive) return foundInActive;
  
  return findNodeByIdInRoot(id, MOCK_TREE_ROOT) || 
         findNodeByIdInRoot(id, MOCK_TREE_ROOT_D) || 
         findNodeByIdInRoot(id, MOCK_TREE_DOWNLOADS_HISTORICAL);
}

function findNodeByIdInRoot(id, root) {
  if (!root || !id) return null;
  if (root.id === id) return root;
  if (root.children) {
    for (const child of root.children) {
      const found = findNodeByIdInRoot(id, child);
      if (found) return found;
    }
  }
  return null;
}

// Find parent directory of a node
function findParentNode(targetId, root = getActiveTargetRoot()) {
  if (!root || !root.children) return null;
  for (const child of root.children) {
    if (child.id === targetId) return root;
    const foundInChild = findParentNode(targetId, child);
    if (foundInChild) return foundInChild;
  }
  return null;
}

// Check if node matches active filters
function nodeMatchesFilter(node) {
  if (!node) return false;
  if (node.isReconciliation) return true;
  
  // Search query
  if (state.filterSearch) {
    const q = state.filterSearch.toLowerCase();
    const matchesName = node.name && node.name.toLowerCase().includes(q);
    const matchesPath = node.path && node.path.toLowerCase().includes(q);
    if (!matchesName && !matchesPath) return false;
  }
  
  // Size filter
  if (state.filterSizeMin > 0) {
    const size = node.uniqueAllocatedBytes ?? node.referencedAllocatedBytes ?? 0;
    if (size < state.filterSizeMin) return false;
  }
  
  // Type filter
  if (state.filterType !== 'all') {
    const ext = (node.fileExt || '').toLowerCase();
    if (state.filterType === 'archives' && !['.zip', '.gz', '.tar', '.iso', '.rar', '.7z', '.zst'].includes(ext)) return false;
    if (state.filterType === 'apps' && !['.exe', '.dll', '.msi', '.vhdx'].includes(ext)) return false;
    if (state.filterType === 'games' && !['.pak', '.exe'].includes(ext) && !node.path?.includes('Games')) return false;
    if (state.filterType === 'system' && !['.sys', '.dll'].includes(ext) && !node.path?.includes('Windows')) return false;
    if (state.filterType === 'media' && !['.mp4', '.mp3', '.mov', '.wav', '.flac'].includes(ext) && !node.path?.includes('Media')) return false;
  }
  
  return true;
}

// Sorting comparator helper (parses timestamps and numbers correctly)
function sortNodes(nodes) {
  const mult = state.sortDirection === 'asc' ? 1 : -1;
  return [...nodes].sort((a, b) => {
    if (state.sortField === 'name') {
      const nameA = (a.name || '').toLowerCase();
      const nameB = (b.name || '').toLowerCase();
      return mult * nameA.localeCompare(nameB);
    }

    if (state.sortField === 'modifiedTime') {
      const timeA = a.modifiedTime ? new Date(a.modifiedTime).getTime() : 0;
      const timeB = b.modifiedTime ? new Date(b.modifiedTime).getTime() : 0;
      return mult * (timeA - timeB);
    }
    
    let valA = a[state.sortField];
    let valB = b[state.sortField];

    valA = valA ?? (a.uniqueAllocatedBytes ?? a.referencedAllocatedBytes ?? 0);
    valB = valB ?? (b.uniqueAllocatedBytes ?? b.referencedAllocatedBytes ?? 0);
    return mult * (valA - valB);
  });
}

// Update URL with current variant using browser history pushState
function updateUrlVariant(variantKey) {
  if (state.variant === variantKey) return;
  state.variant = variantKey;
  const url = new URL(window.location);
  url.searchParams.set('variant', variantKey);
  window.history.pushState({ variant: variantKey }, '', url);
  recordAction(`Switched variant to ${variantKey}`);
  renderApp();
}

function recordAction(actionName) {
  state.lastAction = `${actionName} (${new Date().toLocaleTimeString()})`;
  renderStatePanel();
}

// Focus State Preservation Across Full Rerenders
function captureFocusState() {
  const active = document.activeElement;
  if (!active || active === document.body) return null;
  
  return {
    id: active.id || null,
    nodeId: active.getAttribute('data-node-id') || null,
    toggleId: active.getAttribute('data-toggle-id') || null,
    tab: active.getAttribute('data-tab') || null,
    sortField: active.getAttribute('data-sort-field') || null,
    filterType: active.getAttribute('data-filter-type') || null,
    filterSize: active.getAttribute('data-filter-size') || null,
    role: active.getAttribute('role') || null,
    tag: active.tagName.toLowerCase(),
    className: active.className || null
  };
}

function restoreFocusState(saved) {
  if (!saved) return;
  
  // If modal is open, focus inside modal
  if (state.cleanupModalOpen) {
    const modalFocusable = document.querySelector('.modal-card button, .modal-card [tabindex="0"]');
    if (modalFocusable) {
      modalFocusable.focus();
      return;
    }
  }
  if (state.coverageGapModalOpen) {
    const gapFocusable = document.querySelector('.modal-card button, .modal-card [tabindex="0"]');
    if (gapFocusable) {
      gapFocusable.focus();
      return;
    }
  }

  // Exact ID match
  if (saved.id) {
    const el = document.getElementById(saved.id);
    if (el) { el.focus(); return; }
  }
  
  // Node ID match with role
  if (saved.nodeId) {
    let selector = `[data-node-id="${saved.nodeId}"]`;
    if (saved.role) selector += `[role="${saved.role}"]`;
    const el = document.querySelector(selector);
    if (el) { el.focus(); return; }
  }
  
  // Tab match
  if (saved.tab) {
    const el = document.querySelector(`[data-tab="${saved.tab}"]`);
    if (el) { el.focus(); return; }
  }

  // Sort header match
  if (saved.sortField) {
    const el = document.querySelector(`th[data-sort-field="${saved.sortField}"]`);
    if (el) { el.focus(); return; }
  }

  // Tree toggle button
  if (saved.toggleId) {
    const el = document.querySelector(`[data-toggle-id="${saved.toggleId}"]`);
    if (el) { el.focus(); return; }
  }
}

// Main Render Dispatcher
export function renderApp() {
  const focusState = captureFocusState();
  const container = document.getElementById('app-root');
  if (!container) return;
  
  const rootNode = getActiveTargetRoot();
  const currentVolume = MOCK_VOLUMES.find(v => v.id === state.activeVolumeId) || MOCK_VOLUMES[0];
  const isHistorical = state.activeTargetType === 'historical';
  const isDirectoryTarget = state.activeTargetType === 'directory' || state.activeSnapshotId === 'snap_downloads_archive';
  const reconItem = getActiveReconciliationItem();
  
  // Validate browsed directory exists in active target; if not, reset to target root
  if (!findNodeByIdInRoot(state.browsedDirectoryId, rootNode)) {
    state.browsedDirectoryId = rootNode.id;
  }
  
  container.innerHTML = `
    <div class="app-container">
      <!-- Top Application Header -->
      <header class="app-header" role="banner">
        <div class="brand-section">
          <span class="logo-badge" aria-hidden="true">PT</span>
          <h1 class="brand-title">PigTree</h1>
          <span class="brand-tagline">Disk Space &amp; Storage Analyzer</span>
        </div>

        <!-- Scan Target & Analysis Profile Controls -->
        <div class="header-controls" role="region" aria-label="Scan Controls">
          <div class="control-group">
            <label class="control-label" for="target-select">Target:</label>
            <select id="target-select" class="select-input" aria-label="Select Scan Target">
              <option value="volume:vol_c" ${state.activeTargetType === 'volume' && state.activeVolumeId === 'vol_c' ? 'selected' : ''}>Whole Volume: Local Disk (C:) (512 GB NTFS)</option>
              <option value="volume:vol_d" ${state.activeTargetType === 'volume' && state.activeVolumeId === 'vol_d' ? 'selected' : ''}>Whole Volume: Data Volume (D:) (1 TB ReFS)</option>
              <option value="directory:alex_downloads" ${state.activeTargetType === 'directory' && state.activeSnapshotId !== 'snap_downloads_archive' ? 'selected' : ''}>Directory Scope: C:\\Users\\Alex\\Downloads</option>
              <option value="historical:snap_c_prev_month" ${state.activeTargetType === 'historical' && state.activeSnapshotId === 'snap_c_prev_month' ? 'selected' : ''}>Saved Snapshot: C:\\ (March 27, 2025)</option>
              <option value="historical:snap_downloads_archive" ${state.activeSnapshotId === 'snap_downloads_archive' ? 'selected' : ''}>Saved Snapshot: Downloads (Feb 10, 2025)</option>
            </select>
          </div>

          <div class="control-group">
            <label class="control-label" for="preset-select">Preset:</label>
            <select id="preset-select" class="select-input" aria-label="Select Analysis Profile Preset">
              ${PRESETS.map(p => `<option value="${p.id}" ${state.activePresetId === p.id ? 'selected' : ''}>${p.name}</option>`).join('')}
            </select>
          </div>

          <button id="btn-scan-action" class="btn btn-primary" aria-label="Start or repeat scan">
            ${state.scanStatus === 'scanning' ? '⏳ Scanning (' + state.scanProgress + '%)...' : '▶ Start Scan'}
          </button>
          
          <button id="btn-open-historical" class="btn" aria-label="Open saved historical snapshot">
            📂 Reopen Snapshot
          </button>
        </div>
      </header>

      <!-- Historical Snapshot Warning Banner (when viewing historical snapshot) -->
      ${isHistorical ? `
        <div class="historical-banner" role="alert">
          <span><strong>🕒 Historical Analysis Snapshot:</strong> Showing observations recorded on ${state.activeSnapshotId === 'snap_downloads_archive' ? 'February 10, 2025' : 'March 27, 2025'}. Reopening does not assert that paths or files still exist or remain current.</span>
          <button id="btn-exit-historical" class="btn btn-sm" aria-label="Return to live volume scan">Switch to Live Volume</button>
        </div>
      ` : ''}

      <!-- Status Strip: Outcome, Coverage, and Knowledge -->
      <div class="status-strip" role="region" aria-label="Analysis Status">
        <div class="status-indicators">
          <div>
            <strong>Run Outcome:</strong> 
            <span class="status-badge ${state.scanStatus === 'finished' ? 'badge-complete' : 'badge-active'}">
              ${state.scanStatus.toUpperCase()}
            </span>
          </div>
          <div>
            <strong>Coverage:</strong>
            <span class="status-badge ${(isDirectoryTarget || currentVolume.coverage === 'complete') ? 'badge-complete' : 'badge-partial'}">
              ${(isDirectoryTarget || currentVolume.coverage === 'complete') ? 'COMPLETE' : 'PARTIAL'}
            </span>
          </div>
          ${(!isDirectoryTarget && currentVolume.coverageGaps.length > 0) ? `
            <div>
              <button class="gap-link-btn" id="view-coverage-gaps" aria-label="View Coverage Gap details">
                ⚠️ ${currentVolume.coverageGaps.length} Coverage Gap (Inaccessible Path)
              </button>
            </div>
          ` : ''}
          <div>
            <span style="color: var(--text-dim);">Observed: ${(rootNode.entryCount || state.scanEntriesObserved).toLocaleString()} entries • No atomic snapshot claimed</span>
          </div>
        </div>

        <!-- Scope-Accurate Accounting in Status Strip -->
        ${isDirectoryTarget ? `
          <div class="status-strip-metrics" aria-label="Directory Target Scope Totals">
            <span>Scope: <strong>${rootNode.path}</strong></span> | 
            <span>Scoped Allocation: <strong>${formatBytes(rootNode.uniqueAllocatedBytes ?? rootNode.referencedAllocatedBytes)}</strong></span> | 
            <span>Logical: <strong>${formatBytes(rootNode.uniqueLogicalBytes ?? rootNode.referencedLogicalBytes)}</strong></span> | 
            <span>Entries: <strong>${(rootNode.entryCount || 1).toLocaleString()}</strong></span> | 
            <span style="color: var(--accent-purple); font-weight: 600;" title="Reclaimable allocation cannot assume removal of the final filesystem reference without wider evidence.">External Ref Uncertainty Applies</span>
          </div>
        ` : `
          <div class="status-strip-metrics" aria-label="Volume Capacity Reconciliation">
            <span>Capacity: <strong>${formatBytes(currentVolume.capacityBytes)}</strong></span> | 
            <span>Used: <strong>${formatBytes(currentVolume.usedBytes)}</strong></span> | 
            <span>Accounted: <strong>${formatBytes(currentVolume.accountedUniqueBytes)}</strong></span> | 
            <span style="color: var(--accent-amber); font-weight: 700;">Unattributed: <strong>${formatBytes(currentVolume.unattributedUsedBytes)}</strong></span>
          </div>
        `}
      </div>

      <!-- Main Variant Workspace -->
      <main class="workspace-root" id="workspace-content">
        ${renderVariantContent()}
      </main>

      <!-- Floating Prototype Variant Switcher (Bottom-Center) -->
      <div class="variant-switcher" role="region" aria-label="Prototype Variant Switcher">
        <button id="btn-var-prev" class="switcher-btn" aria-label="Previous Variant (Shortcut: Left Arrow)">←</button>
        <span class="switcher-label">Variant:</span>
        <span class="switcher-pill">${getVariantDisplayName(state.variant)}</span>
        <button id="btn-var-next" class="switcher-btn" aria-label="Next Variant (Shortcut: Right Arrow)">→</button>
      </div>

      <!-- Collapsible Prototype State Panel (Required by prototype skill) -->
      <div class="state-panel-container">
        <button id="btn-toggle-state-panel" class="state-toggle-btn" aria-expanded="${state.statePanelOpen}" aria-label="Toggle Prototype State Debug Panel">
          ⚙ State ${state.statePanelOpen ? '▲' : '▼'}
        </button>
        ${state.statePanelOpen ? `
          <div class="state-panel-card" role="region" aria-label="Prototype Debug State">
            <div class="panel-header">
              <span>In-Memory Prototype State</span>
              <button id="btn-close-state-panel" class="btn btn-sm" aria-label="Close debug state panel">✕</button>
            </div>
            <div class="state-json-view" id="state-json-display">
              ${JSON.stringify(getDebugState(), null, 2)}
            </div>
          </div>
        ` : ''}
      </div>

      <!-- Coverage Gap Explanation Modal -->
      ${state.coverageGapModalOpen ? renderCoverageGapModal(currentVolume) : ''}

      <!-- Guarded Cleanup Modal (if open) -->
      ${state.cleanupModalOpen ? renderCleanupModal() : ''}
    </div>
  `;

  attachEventListeners();
  restoreFocusState(focusState);
}

function getVariantDisplayName(key) {
  if (key === 'explorer') return '1: Explorer (Navigation-First)';
  if (key === 'insights') return '2: Insights (Question-First)';
  if (key === 'workbench') return '3: Workbench (Dense Expert)';
  return key;
}

function getDebugState() {
  const rootNode = getActiveTargetRoot();
  const isDirectory = state.activeTargetType === 'directory' || state.activeSnapshotId === 'snap_downloads_archive';
  const vol = MOCK_VOLUMES.find(v => v.id === state.activeVolumeId) || MOCK_VOLUMES[0];
  
  return {
    variant: state.variant,
    target: state.activeTargetType,
    volumeId: state.activeVolumeId,
    snapshotId: state.activeSnapshotId,
    preset: state.activePresetId,
    browsedDirectoryId: state.browsedDirectoryId,
    scanStatus: state.scanStatus,
    scanProgress: state.scanProgress,
    selectedNodeId: state.selectedNodeId,
    selectedPath: findNodeById(state.selectedNodeId)?.path || 'None',
    filters: {
      search: state.filterSearch,
      sizeMin: state.filterSizeMin,
      type: state.filterType,
      age: state.filterAge
    },
    sorting: {
      field: state.sortField,
      direction: state.sortDirection
    },
    coverage: isDirectory ? 'complete (directory scope)' : vol.coverage,
    accounting: isDirectory ? {
      scope: rootNode.path,
      scopedAllocated: formatBytes(rootNode.uniqueAllocatedBytes ?? rootNode.referencedAllocatedBytes),
      scopedLogical: formatBytes(rootNode.uniqueLogicalBytes ?? rootNode.referencedLogicalBytes),
      entries: rootNode.entryCount
    } : {
      capacity: formatBytes(vol.capacityBytes),
      used: formatBytes(vol.usedBytes),
      accountedUnique: formatBytes(vol.accountedUniqueBytes),
      unattributedUsed: formatBytes(vol.unattributedUsedBytes)
    },
    lastAction: state.lastAction
  };
}

// -------------------------------------------------------------
// VARIANT 1: Explorer (Familiar, Calm, Navigation-First)
// -------------------------------------------------------------
function renderExplorerVariant() {
  const rootNode = getActiveTargetRoot();
  const selectedNode = findNodeById(state.selectedNodeId) || rootNode;
  const reconItem = getActiveReconciliationItem();
  
  return `
    <div class="variant-explorer-layout ${state.inspectorDrawerOpen ? 'drawer-open' : ''}">
      <!-- Left: Folder Tree Navigation -->
      <aside class="explorer-sidebar" aria-label="Directory Tree Navigation">
        <div class="panel-header">
          <span>Folder Navigation</span>
          <span style="font-weight: normal; font-size: 0.75rem; color: var(--text-dim);">Reachable Scopes</span>
        </div>
        <div class="panel-body tree-view" role="tree" aria-label="Directory Tree">
          ${renderTreeNode(rootNode, 0)}
          <!-- Reconciliation Item (NOT a folder, shown only for volume targets) -->
          ${reconItem ? `
            <div class="tree-node ${state.selectedNodeId === reconItem.id ? 'selected' : ''}" 
                 data-node-id="${reconItem.id}" role="treeitem" tabindex="${state.selectedNodeId === reconItem.id ? 0 : -1}" aria-selected="${state.selectedNodeId === reconItem.id}">
              <span class="tree-icon" style="color: var(--accent-amber);">⚖</span>
              <span class="tree-title" style="color: var(--accent-amber); font-style: italic;">${reconItem.name}</span>
              <span class="tree-badge">${formatBytes(reconItem.allocatedBytes)}</span>
            </div>
          ` : ''}
        </div>
      </aside>

      <!-- Center: Sortable Table & Secondary Treemap -->
      <section class="explorer-main">
        <!-- View selection tabs -->
        <nav class="view-tabs-bar" role="tablist" aria-label="Explorer Analysis Views">
          <button class="tab-btn ${state.currentViewTab === 'table' ? 'active' : ''}" data-tab="table" role="tab" aria-selected="${state.currentViewTab === 'table'}">
            📁 Folder Analysis
          </button>
          <button class="tab-btn ${state.currentViewTab === 'flat' ? 'active' : ''}" data-tab="flat" role="tab" aria-selected="${state.currentViewTab === 'flat'}">
            📄 Flat Files
          </button>
          <button class="tab-btn ${state.currentViewTab === 'treemap' ? 'active' : ''}" data-tab="treemap" role="tab" aria-selected="${state.currentViewTab === 'treemap'}">
            📊 Treemap Visualization
          </button>
          <button class="tab-btn ${state.currentViewTab === 'types' ? 'active' : ''}" data-tab="types" role="tab" aria-selected="${state.currentViewTab === 'types'}">
            🏷 File Types
          </button>
          <button class="tab-btn ${state.currentViewTab === 'age' ? 'active' : ''}" data-tab="age" role="tab" aria-selected="${state.currentViewTab === 'age'}">
            ⏳ Age Breakdown
          </button>
          <button class="tab-btn ${state.currentViewTab === 'largest' ? 'active' : ''}" data-tab="largest" role="tab" aria-selected="${state.currentViewTab === 'largest'}">
            🐘 Largest Items
          </button>
          
          <button id="btn-toggle-inspector-drawer" class="btn btn-sm btn-drawer-toggle" aria-label="Toggle Detail Inspector Pane">
            ${state.inspectorDrawerOpen ? 'Hide Inspector ✕' : 'Show Inspector ℹ️'}
          </button>
        </nav>

        <!-- Search and Quick Filter Bar -->
        <div class="filter-toolbar" role="region" aria-label="Filter Controls">
          <label for="input-explorer-search" class="visually-hidden">Search entries</label>
          <input type="search" id="input-explorer-search" class="text-input" placeholder="Search path or name..." value="${state.filterSearch}" style="flex: 1; min-width: 180px; max-width: 320px;" aria-label="Filter Explorer table by name">
          
          <label for="select-explorer-size" class="visually-hidden">Filter by minimum size</label>
          <select id="select-explorer-size" class="select-input" aria-label="Filter by minimum size">
            <option value="0" ${state.filterSizeMin === 0 ? 'selected' : ''}>All Sizes</option>
            <option value="${1024 * 1024 * 1024}" ${state.filterSizeMin === 1024*1024*1024 ? 'selected' : ''}>&gt; 1 GB</option>
            <option value="${100 * 1024 * 1024}" ${state.filterSizeMin === 100*1024*1024 ? 'selected' : ''}>&gt; 100 MB</option>
            <option value="${10 * 1024 * 1024}" ${state.filterSizeMin === 10*1024*1024 ? 'selected' : ''}>&gt; 10 MB</option>
          </select>
          
          <label for="select-explorer-type" class="visually-hidden">Filter by file type</label>
          <select id="select-explorer-type" class="select-input" aria-label="Filter by file type">
            <option value="all" ${state.filterType === 'all' ? 'selected' : ''}>All File Types</option>
            <option value="archives" ${state.filterType === 'archives' ? 'selected' : ''}>Archives &amp; ISOs</option>
            <option value="apps" ${state.filterType === 'apps' ? 'selected' : ''}>Applications (.exe/.dll)</option>
            <option value="games" ${state.filterType === 'games' ? 'selected' : ''}>Game Data (.pak)</option>
            <option value="system" ${state.filterType === 'system' ? 'selected' : ''}>System Files</option>
            <option value="media" ${state.filterType === 'media' ? 'selected' : ''}>Media (.mp4/.mov)</option>
          </select>
          ${(state.filterSearch || state.filterSizeMin > 0 || state.filterType !== 'all') ? `
            <button id="btn-clear-filters" class="btn btn-sm">Clear Filters</button>
          ` : ''}
        </div>

        <!-- Center View Area -->
        <div class="explorer-center">
          ${renderActiveViewTab(selectedNode)}
        </div>

        <!-- Synchronized Bottom Treemap Preview when folder table is shown -->
        ${state.currentViewTab === 'table' ? `
          <div class="explorer-subpanel">
            <div class="panel-header">
              <span>Synchronized Spatial Preview</span>
              <div style="display: flex; align-items: center; gap: 8px;">
                <button id="btn-toggle-subpanel-text" class="btn btn-sm" aria-label="Toggle text/table equivalent for spatial preview">
                  ${state.treemapShowTextEquivalent ? 'Show Treemap Visual' : 'Show Accessible Table'}
                </button>
                <span style="font-size: 0.75rem; font-weight: normal; color: var(--text-dim);">Click block to inspect</span>
              </div>
            </div>
            <div class="panel-body" style="padding: 0;">
              ${state.treemapShowTextEquivalent ? renderFolderTableView(findNodeById(state.browsedDirectoryId) || rootNode) : renderTreemapSvg(rootNode, 700, 180)}
            </div>
          </div>
        ` : ''}
      </section>

      <!-- Right: Contextual Inspector -->
      <aside class="explorer-inspector ${state.inspectorDrawerOpen ? 'drawer-visible' : ''}" role="region" aria-label="Detail Inspector">
        ${renderDetailInspector(selectedNode)}
      </aside>
    </div>
  `;
}

// -------------------------------------------------------------
// VARIANT 2: Insights (Everyday-User, Question-First)
// -------------------------------------------------------------
function renderInsightsVariant() {
  const rootNode = getActiveTargetRoot();
  const selectedNode = findNodeById(state.selectedNodeId) || rootNode;
  const isHistorical = state.activeTargetType === 'historical';
  const reconItem = getActiveReconciliationItem();
  
  return `
    <div class="variant-insights-layout ${state.inspectorDrawerOpen ? 'drawer-open' : ''}">
      <div class="insights-content">
        <!-- Top Plain-Language Cards Grid answering everyday user questions -->
        <div class="insights-card-grid">
          
          <!-- Card 1: What is taking space? -->
          <div class="insight-card">
            <div class="insight-card-header">
              <span class="insight-question">What is taking up your disk space?</span>
              <span class="insight-metric-highlight">${formatBytes(rootNode.uniqueAllocatedBytes ?? rootNode.referencedAllocatedBytes)}</span>
            </div>
            <p class="insight-explanation">
              Major storage allocations reachable within ${rootNode.name}:
            </p>
            <ul class="insight-bullets">
              <li><strong>Users (Alex):</strong> 107.9 GB allocated (Downloads, Projects, AppData)</li>
              <li><strong>Games (Starfall):</strong> 68.6 GB allocated in game data packages</li>
              <li><strong>Windows &amp; System:</strong> 48.2 GB referenced (42.1 GB unique, shared via hard links)</li>
              <li><strong>Virtual Memory / Hibernation:</strong> 28.8 GB (pagefile + hiberfil)</li>
            </ul>
            <div class="insight-actions">
              <button class="btn btn-sm btn-primary btn-select-node" data-node-id="node_users" aria-label="Inspect Users folder">Inspect Users</button>
              <button class="btn btn-sm btn-select-node" data-node-id="node_games" aria-label="Inspect Games folder">Inspect Games</button>
            </div>
          </div>

          <!-- Card 2: What changed? (Historical comparison) -->
          <div class="insight-card card-purple">
            <div class="insight-card-header">
              <span class="insight-question">What changed since last snapshot?</span>
              <span class="insight-metric-highlight highlight-purple">+14.4 GB Net</span>
            </div>
            <p class="insight-explanation">
              ${isHistorical ? 'Historical observation comparison vs current profile baseline:' : 'Differences observed since March 27 snapshot:'}
            </p>
            <ul class="insight-bullets">
              <li><strong>+6.2 GB:</strong> New Windows11_Setup_23H2.iso in Downloads</li>
              <li><strong>+8.2 GB:</strong> Temporary build cache growth in AppData\\Local\\Temp</li>
              <li><strong>0 B:</strong> OneDrive cloud archive (placeholder only, takes 0 B local disk)</li>
            </ul>
            <div class="insight-actions">
              <button class="btn btn-sm btn-select-node" data-node-id="node_alex_win11_iso" aria-label="Inspect New Windows 11 ISO">Inspect New ISO</button>
              <button class="btn btn-sm btn-select-node" data-node-id="node_alex_temp" aria-label="Inspect Local Temp Cache">Inspect Temp Cache</button>
            </div>
          </div>

          <!-- Card 3: What can I safely review? -->
          <div class="insight-card card-teal">
            <div class="insight-card-header">
              <span class="insight-question">What can I safely review for cleanup?</span>
              <span class="insight-metric-highlight highlight-teal">~37.1 GB</span>
            </div>
            <p class="insight-explanation">
              Items safe for user review with exact reclaimable calculations:
            </p>
            <ul class="insight-bullets">
              <li><strong>Downloads Folder:</strong> 28.9 GB (Old ISOs, install packages)</li>
              <li><strong>Temporary Cache Files:</strong> 8.2 GB in AppData\\Local\\Temp</li>
              <li><strong>Cloud Placeholders:</strong> 4.8 GB OneDrive files (0 B local disk)</li>
            </ul>
            <div class="insight-actions">
              <button class="btn btn-sm btn-accent btn-open-cleanup" data-node-id="node_alex_downloads" aria-label="Review Downloads Cleanup Action Plan">
                Review Downloads Cleanup (28.9 GB)
              </button>
            </div>
          </div>

          <!-- Card 4: Why does disk space not add up? (Unattributed & Inaccessible) -->
          <div class="insight-card card-amber">
            <div class="insight-card-header">
              <span class="insight-question">Why is there unattributed or inaccessible space?</span>
              <span class="insight-metric-highlight highlight-amber">${reconItem ? formatBytes(reconItem.allocatedBytes) : '0 B'}</span>
            </div>
            <p class="insight-explanation">
              <strong>Unattributed Used Space (${reconItem ? formatBytes(reconItem.allocatedBytes) : '0 B'}):</strong> Volume used space exceeds scanned objects due to NTFS metadata, system restore shadow copies, or restricted system areas.
            </p>
            <p class="insight-explanation" style="margin-top: 4px;">
              <strong>System Volume Information:</strong> Inaccessible under standard user context. <em>More access may reveal additional metadata.</em>
            </p>
            <div class="insight-actions">
              <button class="btn btn-sm btn-warning btn-select-node" data-node-id="${reconItem ? reconItem.id : 'node_root'}" aria-label="Inspect Volume Reconciliation Math">
                Inspect Reconciliation Math
              </button>
              <button class="btn btn-sm btn-select-node" data-node-id="node_sysvolinfo" id="btn-insights-view-gap" aria-label="View Coverage Gap explanation dialog">
                View Coverage Gap
              </button>
            </div>
          </div>

        </div>

        <!-- Progressive Detailed Drill-Down Section -->
        <div class="insights-drilldown-section" id="insights-drilldown">
          <div style="display: flex; justify-content: space-between; align-items: center; flex-wrap: wrap; gap: 8px; margin-bottom: 8px;">
            <h2 style="font-size: 0.9375rem; font-weight: 700;">Progressive Storage Explorer</h2>
            <div style="display: flex; gap: 6px; align-items: center;">
              <button class="tab-btn ${state.currentViewTab === 'table' ? 'active' : ''}" data-tab="table" role="tab" aria-selected="${state.currentViewTab === 'table'}">Interactive Table</button>
              <button class="tab-btn ${state.currentViewTab === 'treemap' ? 'active' : ''}" data-tab="treemap" role="tab" aria-selected="${state.currentViewTab === 'treemap'}">Treemap View</button>
              <button class="tab-btn ${state.currentViewTab === 'largest' ? 'active' : ''}" data-tab="largest" role="tab" aria-selected="${state.currentViewTab === 'largest'}">Largest Items</button>
              <button id="btn-toggle-insights-inspector" class="btn btn-sm btn-drawer-toggle" aria-label="Toggle Detail Inspector">
                ${state.inspectorDrawerOpen ? 'Hide Inspector ✕' : 'Show Inspector ℹ️'}
              </button>
            </div>
          </div>

          <div style="min-height: 280px; display: flex; flex-direction: column;">
            ${renderActiveViewTab(selectedNode)}
          </div>
        </div>
      </div>

      <!-- Right: Detail Inspector -->
      <aside class="explorer-inspector ${state.inspectorDrawerOpen ? 'drawer-visible' : ''}" id="insights-inspector-panel" role="region" aria-label="Detail Inspector">
        ${renderDetailInspector(selectedNode)}
      </aside>
    </div>
  `;
}

// -------------------------------------------------------------
// VARIANT 3: Workbench (Dense Expert Workspace)
// -------------------------------------------------------------
function renderWorkbenchVariant() {
  const rootNode = getActiveTargetRoot();
  const selectedNode = findNodeById(state.selectedNodeId) || rootNode;
  
  return `
    <div class="variant-workbench-layout ${state.inspectorDrawerOpen ? 'drawer-open' : ''}">
      <!-- Expert Command & Filter Bar -->
      <div class="workbench-command-bar" role="toolbar" aria-label="Expert Workbench Command Bar">
        <label for="input-workbench-cmd" class="workbench-bar-label">Command / Filter:</label>
        <input type="search" id="input-workbench-cmd" class="text-input" placeholder="filter path:C:\\Users size:>100MB type:pak,iso links:>1..." value="${state.filterSearch}" style="min-width: 240px; font-family: var(--font-mono); font-size: 0.75rem;" aria-label="Workbench command filter">
        
        <div class="workbench-filter-tokens" role="group" aria-label="Active Filter Tokens">
          <button type="button" class="filter-token ${state.filterSizeMin === 1024*1024*1024 ? 'active' : ''}" data-filter-size="${state.filterSizeMin === 1024*1024*1024 ? '0' : '1073741824'}" aria-pressed="${state.filterSizeMin === 1024*1024*1024}" aria-label="Toggle filter size greater than 1GB">
            size &gt; 1GB
          </button>
          <button type="button" class="filter-token ${state.filterType === 'archives' ? 'active' : ''}" data-filter-type="${state.filterType === 'archives' ? 'all' : 'archives'}" aria-pressed="${state.filterType === 'archives'}" aria-label="Toggle filter archives">
            type:archives
          </button>
          <button type="button" class="filter-token ${state.filterType === 'apps' ? 'active' : ''}" data-filter-type="${state.filterType === 'apps' ? 'all' : 'apps'}" aria-pressed="${state.filterType === 'apps'}" aria-label="Toggle filter binaries">
            type:binaries
          </button>
          <button type="button" class="filter-token token-preset" id="btn-workbench-preset" aria-label="Cycle active profile preset (Current: ${state.activePresetId})">
            preset:${state.activePresetId} ⟳
          </button>
        </div>

        <div style="margin-left: auto; display: flex; gap: 6px; align-items: center;">
          <button id="btn-open-workbench-cleanup" class="btn btn-sm btn-warning" aria-label="Open Guarded Action Plan for selected item: ${selectedNode.name}">
            🛡 Guarded Action Plan
          </button>
          <button id="btn-toggle-workbench-panels" class="btn btn-sm btn-drawer-toggle" aria-label="Toggle Side Panels">
            ${state.inspectorDrawerOpen ? 'Hide Side Panels ✕' : 'Side Panels ℹ️'}
          </button>
        </div>
      </div>

      <!-- Workbench Main Body -->
      <div class="workbench-body">
        <!-- Center Table Area -->
        <div class="workbench-table-area">
          <div class="panel-header">
            <span>Configurable Analysis Matrix (Referenced vs Unique Allocation &amp; Entry Counts)</span>
            <div style="display: flex; gap: 8px;">
              <span style="font-size: 0.75rem;">Active Target: <code>${rootNode.path}</code></span>
              <span style="font-size: 0.75rem; color: var(--text-dim);">Selected: <code>${selectedNode.name || selectedNode.path}</code></span>
            </div>
          </div>

          <div class="data-table-container">
            ${renderWorkbenchExpertTable(rootNode)}
          </div>
        </div>

        <!-- Right Side Panels: Secondary Treemap + Deep Object & Stream Inspector -->
        <aside class="workbench-side-panels ${state.inspectorDrawerOpen ? 'drawer-visible' : ''}" role="region" aria-label="Workbench Side Panels and Detail Inspector">
          <div class="panel-header">
            <span>Spatial Allocation Treemap</span>
            <button id="btn-toggle-workbench-treemap-text" class="btn btn-sm" aria-label="Toggle text table for side treemap">
              ${state.treemapShowTextEquivalent ? 'Treemap View' : 'Table View'}
            </button>
          </div>
          <div style="height: 180px; background: var(--bg-app); border-bottom: 1px solid var(--border-color); overflow: hidden;">
            ${state.treemapShowTextEquivalent ? renderFolderTableView(findNodeById(state.browsedDirectoryId) || rootNode) : renderTreemapSvg(rootNode, 360, 180)}
          </div>

          <!-- Deep Inspector -->
          <div style="flex: 1; overflow-y: auto;">
            ${renderDetailInspector(selectedNode)}
          </div>
        </aside>
      </div>
    </div>
  `;
}

function renderVariantContent() {
  if (state.variant === 'explorer') return renderExplorerVariant();
  if (state.variant === 'insights') return renderInsightsVariant();
  if (state.variant === 'workbench') return renderWorkbenchVariant();
  return renderExplorerVariant();
}

// -------------------------------------------------------------
// Component Renderers: Tree Node, Tables, Views, Treemap, Inspector
// -------------------------------------------------------------

// Recursive Tree Node Renderer
function renderTreeNode(node, depth = 0) {
  if (!node) return '';
  const isExpanded = state.expandedNodes.has(node.id);
  const isSelected = state.selectedNodeId === node.id;
  const hasChildren = node.children && node.children.length > 0;
  const indent = depth * 14;

  let html = `
    <div class="tree-node ${isSelected ? 'selected' : ''}" 
         data-node-id="${node.id}" 
         style="padding-left: ${indent + 8}px;"
         role="treeitem" 
         aria-expanded="${hasChildren ? isExpanded : 'undefined'}"
         aria-selected="${isSelected}"
         tabindex="${isSelected ? 0 : -1}">
      ${hasChildren ? `
        <button type="button" class="tree-toggle-btn" data-toggle-id="${node.id}" aria-label="${isExpanded ? 'Collapse' : 'Expand'} ${node.name}">
          ${isExpanded ? '▼' : '▶'}
        </button>
      ` : `
        <span class="tree-toggle-spacer" aria-hidden="true">•</span>
      `}
      <span class="tree-icon" aria-hidden="true">${node.kind === 'directory' ? '📁' : (node.kind === 'summary_remainder' ? '📦' : (node.kind === 'special' ? '⚙' : '📄'))}</span>
      <span class="tree-title" title="${node.path}">${node.name}</span>
      <span class="tree-badge">${formatBytes(node.uniqueAllocatedBytes ?? node.referencedAllocatedBytes)}</span>
    </div>
  `;

  if (hasChildren && isExpanded) {
    for (const child of node.children) {
      html += renderTreeNode(child, depth + 1);
    }
  }

  return html;
}

// Folder Table View (Preserves browsed directory so selecting a file does NOT snap to root)
function renderFolderTableView(scopeNode) {
  const browsedDir = (scopeNode && scopeNode.kind === 'directory') ? scopeNode : (findParentNode(scopeNode?.id) || getActiveTargetRoot());
  const children = browsedDir.children || [browsedDir];
  const filteredChildren = children.filter(nodeMatchesFilter);
  const sortedChildren = sortNodes(filteredChildren);
  const reconItem = getActiveReconciliationItem();
  const rootNode = getActiveTargetRoot();
  const isAtTargetRoot = browsedDir.id === rootNode.id;

  const sortAria = (field) => {
    if (state.sortField !== field) return 'aria-sort="none"';
    return state.sortDirection === 'asc' ? 'aria-sort="ascending"' : 'aria-sort="descending"';
  };

  const sortIndicator = (field) => {
    if (state.sortField !== field) return '';
    return state.sortDirection === 'asc' ? ' ▲' : ' ▼';
  };

  return `
    <div class="data-table-container">
      <div class="folder-browsing-header" style="padding: 6px 12px; background: var(--bg-subtle); border-bottom: 1px solid var(--border-color); display: flex; justify-content: space-between; align-items: center; font-size: 0.75rem;">
        <div>
          <span>Browsing Scope: <strong><code>${browsedDir.path || browsedDir.name}</code></strong></span>
          <span style="color: var(--text-dim); margin-left: 8px;">(${filteredChildren.length} items shown)</span>
        </div>
        ${!isAtTargetRoot ? `
          <button class="btn btn-sm btn-select-node" data-node-id="${rootNode.id}" aria-label="Jump to top-level scan target root">
            ↑ Jump to Root
          </button>
        ` : ''}
      </div>

      <table class="data-table" role="table" aria-label="Directory Entries Table">
        <thead>
          <tr>
            <th scope="col" data-sort-field="name" ${sortAria('name')} tabindex="0" role="columnheader">Name${sortIndicator('name')}</th>
            <th scope="col" data-sort-field="uniqueAllocatedBytes" ${sortAria('uniqueAllocatedBytes')} class="cell-mono" tabindex="0" role="columnheader">Unique Allocated${sortIndicator('uniqueAllocatedBytes')}</th>
            <th scope="col" data-sort-field="referencedAllocatedBytes" ${sortAria('referencedAllocatedBytes')} class="cell-mono" tabindex="0" role="columnheader">Referenced Allocated${sortIndicator('referencedAllocatedBytes')}</th>
            <th scope="col" data-sort-field="uniqueLogicalBytes" ${sortAria('uniqueLogicalBytes')} class="cell-mono" tabindex="0" role="columnheader">Logical Size${sortIndicator('uniqueLogicalBytes')}</th>
            <th scope="col" data-sort-field="entryCount" ${sortAria('entryCount')} class="cell-mono" tabindex="0" role="columnheader">Entries${sortIndicator('entryCount')}</th>
            <th scope="col" data-sort-field="uniqueObjectCount" ${sortAria('uniqueObjectCount')} class="cell-mono" tabindex="0" role="columnheader">Unique Objects${sortIndicator('uniqueObjectCount')}</th>
            <th scope="col">Kind / Category</th>
            <th scope="col">Status / Coverage</th>
            <th scope="col" data-sort-field="modifiedTime" ${sortAria('modifiedTime')} tabindex="0" role="columnheader">Modified Date${sortIndicator('modifiedTime')}</th>
          </tr>
        </thead>
        <tbody>
          ${sortedChildren.map(child => {
            const isSel = state.selectedNodeId === child.id;
            const isSummary = child.isSummaryRemainder || child.kind === 'summary_remainder';
            return `
              <tr class="${isSel ? 'selected' : ''} ${isSummary ? 'summary-row' : ''}" data-node-id="${child.id}" role="row" aria-selected="${isSel}" tabindex="0">
                <td class="cell-name">
                  <span>${child.kind === 'directory' ? '📁' : (isSummary ? '📦' : '📄')}</span>
                  <strong style="${isSummary ? 'font-style: italic; color: var(--text-dim);' : ''}">${child.name}</strong>
                </td>
                <td class="cell-mono">${formatBytes(child.uniqueAllocatedBytes)}</td>
                <td class="cell-mono">${formatBytes(child.referencedAllocatedBytes)}</td>
                <td class="cell-mono">${formatBytes(child.uniqueLogicalBytes ?? child.referencedLogicalBytes)}</td>
                <td class="cell-mono">${(child.entryCount !== undefined ? child.entryCount : 1).toLocaleString()}</td>
                <td class="cell-mono">${(child.uniqueObjectCount !== undefined ? child.uniqueObjectCount : 1).toLocaleString()}</td>
                <td>
                  <span class="fact-tag ${isSummary ? 'tag-purple' : ''}">${child.category || child.kind}</span>
                </td>
                <td>
                  <span class="status-badge ${child.observationStatus === 'inaccessible' ? 'badge-partial' : 'badge-complete'}">
                    ${child.observationStatus === 'inaccessible' ? 'INACCESSIBLE' : 'OBSERVED'}
                  </span>
                </td>
                <td style="font-size: 0.75rem; color: var(--text-dim);">${formatDate(child.modifiedTime)}</td>
              </tr>
            `;
          }).join('')}

          <!-- Volume Scope Reconciliation Row (Shown only when viewing volume root) -->
          ${(isAtTargetRoot && reconItem) ? `
            <tr class="reconciliation-row ${state.selectedNodeId === reconItem.id ? 'selected' : ''}" 
                data-node-id="${reconItem.id}" role="row" aria-selected="${state.selectedNodeId === reconItem.id}" tabindex="0">
              <td class="cell-name">
                <span style="color: var(--accent-amber);">⚖</span>
                <strong style="color: var(--accent-amber);">${reconItem.name}</strong>
              </td>
              <td class="cell-mono" style="color: var(--accent-amber); font-weight: 700;">${formatBytes(reconItem.allocatedBytes)}</td>
              <td class="cell-mono" style="color: var(--accent-amber);">${formatBytes(reconItem.allocatedBytes)}</td>
              <td class="cell-mono">${formatBytes(reconItem.logicalBytes)}</td>
              <td class="cell-mono">-</td>
              <td class="cell-mono">-</td>
              <td><span class="fact-tag tag-warning">Reconciliation Difference</span></td>
              <td><span class="status-badge badge-partial">RECONCILED</span></td>
              <td style="font-size: 0.75rem; color: var(--text-dim);">Volume Accounting Boundary</td>
            </tr>
          ` : ''}
        </tbody>
      </table>
    </div>
  `;
}

// Flat Files Table View
function renderFlatFilesTableView() {
  const rootNode = getActiveTargetRoot();
  const allNodes = getAllFlatNodes(rootNode).filter(n => n.kind === 'file' || n.kind === 'special').filter(nodeMatchesFilter);
  const sortedFiles = sortNodes(allNodes);

  const sortAria = (field) => {
    if (state.sortField !== field) return 'aria-sort="none"';
    return state.sortDirection === 'asc' ? 'aria-sort="ascending"' : 'aria-sort="descending"';
  };
  const sortIndicator = (field) => {
    if (state.sortField !== field) return '';
    return state.sortDirection === 'asc' ? ' ▲' : ' ▼';
  };

  return `
    <div class="data-table-container">
      <table class="data-table" role="table" aria-label="Flat Files Table">
        <thead>
          <tr>
            <th scope="col" data-sort-field="name" ${sortAria('name')} tabindex="0" role="columnheader">File Name &amp; Observed Path${sortIndicator('name')}</th>
            <th scope="col" data-sort-field="uniqueAllocatedBytes" ${sortAria('uniqueAllocatedBytes')} class="cell-mono" tabindex="0" role="columnheader">Allocated Size${sortIndicator('uniqueAllocatedBytes')}</th>
            <th scope="col" data-sort-field="uniqueLogicalBytes" ${sortAria('uniqueLogicalBytes')} class="cell-mono" tabindex="0" role="columnheader">Logical Size${sortIndicator('uniqueLogicalBytes')}</th>
            <th scope="col">Characteristics</th>
            <th scope="col" data-sort-field="modifiedTime" ${sortAria('modifiedTime')} tabindex="0" role="columnheader">Modified Date${sortIndicator('modifiedTime')}</th>
            <th scope="col">Action Preview</th>
          </tr>
        </thead>
        <tbody>
          ${sortedFiles.map(file => {
            const isSel = state.selectedNodeId === file.id;
            return `
              <tr class="${isSel ? 'selected' : ''}" data-node-id="${file.id}" role="row" aria-selected="${isSel}" tabindex="0">
                <td class="cell-name">
                  <span>📄</span>
                  <div>
                    <strong>${file.name}</strong>
                    <div style="font-size: 0.6875rem; color: var(--text-dim);">${file.path}</div>
                  </div>
                </td>
                <td class="cell-mono" style="font-weight: 600;">${formatBytes(file.uniqueAllocatedBytes ?? file.referencedAllocatedBytes)}</td>
                <td class="cell-mono">${formatBytes(file.uniqueLogicalBytes ?? file.referencedLogicalBytes)}</td>
                <td>
                  <span class="fact-tag ${file.storageCharacteristics?.includes('online-only') ? 'tag-purple' : ''}">
                    ${file.storageCharacteristics?.join(', ') || 'standard'}
                  </span>
                </td>
                <td style="font-size: 0.75rem; color: var(--text-dim);">${formatDate(file.modifiedTime)}</td>
                <td>
                  <button class="btn btn-sm btn-open-cleanup" data-node-id="${file.id}" aria-label="Open Guarded Cleanup Action Plan for ${file.name}">Guarded Review</button>
                </td>
              </tr>
            `;
          }).join('')}
        </tbody>
      </table>
    </div>
  `;
}

// Largest Items View
function renderLargestItemsView() {
  const rootNode = getActiveTargetRoot();
  const allFiles = getAllFlatNodes(rootNode).filter(n => n.kind === 'file' || n.kind === 'special');
  allFiles.sort((a, b) => (b.uniqueAllocatedBytes ?? b.referencedAllocatedBytes ?? 0) - (a.uniqueAllocatedBytes ?? a.referencedAllocatedBytes ?? 0));
  const top10 = allFiles.slice(0, 10);

  return `
    <div class="data-table-container">
      <div style="padding: 8px 12px; background: var(--bg-subtle); border-bottom: 1px solid var(--border-color); font-size: 0.75rem;">
        <strong>Top 10 Largest Storage Consumers</strong> across scanned target <code>${rootNode.path}</code>.
      </div>
      <table class="data-table" role="table" aria-label="Largest Items List">
        <thead>
          <tr>
            <th scope="col">Rank</th>
            <th scope="col">Item Name &amp; Path</th>
            <th scope="col" class="cell-mono">Allocated Physical Size</th>
            <th scope="col" class="cell-mono">Logical Size</th>
            <th scope="col">Safety / Clean Risk</th>
            <th scope="col">Action</th>
          </tr>
        </thead>
        <tbody>
          ${top10.map((item, idx) => {
            const isSel = state.selectedNodeId === item.id;
            return `
              <tr class="${isSel ? 'selected' : ''}" data-node-id="${item.id}" role="row" aria-selected="${isSel}" tabindex="0">
                <td style="font-weight: 700; color: var(--text-dim);">#${idx + 1}</td>
                <td class="cell-name">
                  <div>
                    <strong>${item.name}</strong>
                    <div style="font-size: 0.6875rem; color: var(--text-dim);">${item.path}</div>
                  </div>
                </td>
                <td class="cell-mono" style="font-weight: 700; color: var(--primary);">${formatBytes(item.uniqueAllocatedBytes ?? item.referencedAllocatedBytes)}</td>
                <td class="cell-mono">${formatBytes(item.uniqueLogicalBytes ?? item.referencedLogicalBytes)}</td>
                <td>
                  <span class="fact-tag ${item.cleanupSafe === 'user_reviewable' ? 'tag-warning' : ''}">
                    ${item.cleanupSafe || 'Review Required'}
                  </span>
                </td>
                <td>
                  <button class="btn btn-sm btn-open-cleanup" data-node-id="${item.id}" aria-label="Action Plan for ${item.name}">Action Plan</button>
                </td>
              </tr>
            `;
          }).join('')}
        </tbody>
      </table>
    </div>
  `;
}

// File Types Breakdown View
function renderFileTypesView() {
  const rootNode = getActiveTargetRoot();
  const allFiles = getAllFlatNodes(rootNode).filter(n => n.kind === 'file');
  
  const typeMap = {
    'Game & Package Data (.pak)': { ext: ['.pak'], bytes: 0, count: 0, sample: 'Game asset packages' },
    'Disk Images (.iso, .vhdx)': { ext: ['.iso', '.vhdx'], bytes: 0, count: 0, sample: 'Setup ISOs, VM virtual disks' },
    'Compressed Archives (.zip, .gz, .zst)': { ext: ['.zip', '.gz', '.tar', '.zst', '.7z'], bytes: 0, count: 0, sample: 'Dataset & project backups' },
    'System Executables & Binaries (.exe, .dll, .sys)': { ext: ['.exe', '.dll', '.sys', '.msi'], bytes: 0, count: 0, sample: 'System & app binaries' },
    'Media Assets (.mp4, .mov, .wav)': { ext: ['.mp4', '.mov', '.mp3', '.wav', '.flac'], bytes: 0, count: 0, sample: 'Raw video and audio' },
    'Other / Unclassified': { ext: [], bytes: 0, count: 0, sample: 'Documents, configs, local caches' }
  };

  for (const f of allFiles) {
    const ext = (f.fileExt || '').toLowerCase();
    let placed = false;
    for (const [groupName, group] of Object.entries(typeMap)) {
      if (group.ext.includes(ext)) {
        group.bytes += (f.uniqueAllocatedBytes ?? f.referencedAllocatedBytes ?? 0);
        group.count += 1;
        placed = true;
        break;
      }
    }
    if (!placed) {
      typeMap['Other / Unclassified'].bytes += (f.uniqueAllocatedBytes ?? f.referencedAllocatedBytes ?? 0);
      typeMap['Other / Unclassified'].count += 1;
    }
  }

  return `
    <div class="data-table-container">
      <div style="padding: 8px 12px; background: var(--bg-subtle); border-bottom: 1px solid var(--border-color); font-size: 0.75rem;">
        <strong>Observed File Classification Summary</strong> (Aggregated by entry classification rules within <code>${rootNode.path}</code>)
      </div>
      <table class="data-table" role="table" aria-label="File Types Breakdown">
        <thead>
          <tr>
            <th scope="col">Classification Category</th>
            <th scope="col" class="cell-mono">Total Allocation</th>
            <th scope="col" class="cell-mono">File Count</th>
            <th scope="col">Primary Examples</th>
          </tr>
        </thead>
        <tbody>
          ${Object.entries(typeMap).map(([name, data]) => `
            <tr>
              <td><strong>${name}</strong></td>
              <td class="cell-mono" style="font-weight: 700;">${formatBytes(data.bytes)}</td>
              <td class="cell-mono">${data.count}</td>
              <td style="color: var(--text-dim);">${data.sample}</td>
            </tr>
          `).join('')}
        </tbody>
      </table>
    </div>
  `;
}

// Dynamic Age Distribution View (Requirement 8)
function renderAgeDistributionView() {
  const rootNode = getActiveTargetRoot();
  const allFiles = getAllFlatNodes(rootNode).filter(n => n.kind === 'file' || n.kind === 'special').filter(nodeMatchesFilter);
  
  // Reference observation anchor time: April 10, 2025
  const refTime = new Date('2025-04-10T14:20:00Z').getTime();
  const dayMs = 24 * 60 * 60 * 1000;
  
  const buckets = [
    {
      id: 'recent',
      name: 'Recent (< 7 days)',
      minDays: 0,
      maxDays: 7,
      bytes: 0,
      count: 0,
      typical: 'Active developer builds, current temp caches, newly downloaded installers',
      strategy: 'Generally keep; safe temp cache cleanup'
    },
    {
      id: 'month',
      name: 'Past 30 Days (7 – 30 days)',
      minDays: 7,
      maxDays: 30,
      bytes: 0,
      count: 0,
      typical: 'Game updates, monthly project revisions, recent downloads',
      strategy: 'Review completed projects and temporary downloads'
    },
    {
      id: 'year',
      name: 'Past 1 Year (30 – 365 days)',
      minDays: 30,
      maxDays: 365,
      bytes: 0,
      count: 0,
      typical: 'Operating system files, application suites, container images',
      strategy: 'Stable operational software; review unused applications'
    },
    {
      id: 'older',
      name: 'Older than 1 Year (> 365 days)',
      minDays: 365,
      maxDays: Infinity,
      bytes: 0,
      count: 0,
      typical: 'Historical datasets, old setup ISOs, unused tar archives',
      strategy: 'High Priority Cleanup Review'
    }
  ];
  
  for (const f of allFiles) {
    const mTime = new Date(f.modifiedTime || '2025-04-01T00:00:00Z').getTime();
    const ageDays = Math.max(0, (refTime - mTime) / dayMs);
    const bytes = f.uniqueAllocatedBytes ?? f.referencedAllocatedBytes ?? 0;
    
    for (const b of buckets) {
      if (ageDays >= b.minDays && (b.maxDays === Infinity ? true : ageDays < b.maxDays)) {
        b.bytes += bytes;
        b.count += 1;
        break;
      }
    }
  }

  return `
    <div class="data-table-container">
      <div style="padding: 8px 12px; background: var(--bg-subtle); border-bottom: 1px solid var(--border-color); font-size: 0.75rem; display: flex; justify-content: space-between; align-items: center; flex-wrap: wrap; gap: 8px;">
        <div>
          <strong>Storage Age Distribution</strong> — dynamically derived from reachable entries in <code>${rootNode.path}</code>
        </div>
        <div style="color: var(--text-dim); font-size: 0.6875rem;">
          <span>Timestamp Kind: <strong>Recorded Last Modified Time (mtime)</strong></span>
        </div>
      </div>
      <table class="data-table" role="table" aria-label="Age Distribution Table">
        <thead>
          <tr>
            <th scope="col">Time Interval</th>
            <th scope="col" class="cell-mono">Allocated Physical Size</th>
            <th scope="col" class="cell-mono">File Count</th>
            <th scope="col">Typical Contents</th>
            <th scope="col">Candidate Review Strategy</th>
          </tr>
        </thead>
        <tbody>
          ${buckets.map(b => `
            <tr>
              <td><strong>${b.name}</strong></td>
              <td class="cell-mono" style="font-weight: 700;">${formatBytes(b.bytes)}</td>
              <td class="cell-mono">${b.count}</td>
              <td style="color: var(--text-muted);">${b.typical}</td>
              <td>
                ${b.id === 'older' && b.bytes > 0 ? 
                  `<span class="fact-tag tag-warning">${b.strategy}</span>` : 
                  `<span>${b.strategy}</span>`
                }
              </td>
            </tr>
          `).join('')}
        </tbody>
      </table>
    </div>
  `;
}

// Workbench Expert Table (Dense Grid with all metadata)
function renderWorkbenchExpertTable(rootNode) {
  const allNodes = getAllFlatNodes(rootNode).filter(nodeMatchesFilter);
  const sortedNodes = sortNodes(allNodes);
  const reconItem = getActiveReconciliationItem();

  const sortAria = (field) => {
    if (state.sortField !== field) return 'aria-sort="none"';
    return state.sortDirection === 'asc' ? 'aria-sort="ascending"' : 'aria-sort="descending"';
  };
  const sortIndicator = (field) => {
    if (state.sortField !== field) return '';
    return state.sortDirection === 'asc' ? ' ▲' : ' ▼';
  };

  return `
    <table class="data-table" role="table" aria-label="Dense Expert Table">
      <thead>
        <tr>
          <th scope="col" data-sort-field="name" ${sortAria('name')} tabindex="0" role="columnheader">Entry Name &amp; Observed Path${sortIndicator('name')}</th>
          <th scope="col" data-sort-field="uniqueAllocatedBytes" ${sortAria('uniqueAllocatedBytes')} class="cell-mono" tabindex="0" role="columnheader">Unique Alloc (B)${sortIndicator('uniqueAllocatedBytes')}</th>
          <th scope="col" data-sort-field="referencedAllocatedBytes" ${sortAria('referencedAllocatedBytes')} class="cell-mono" tabindex="0" role="columnheader">Ref Alloc (B)${sortIndicator('referencedAllocatedBytes')}</th>
          <th scope="col" data-sort-field="uniqueLogicalBytes" ${sortAria('uniqueLogicalBytes')} class="cell-mono" tabindex="0" role="columnheader">Logical (B)${sortIndicator('uniqueLogicalBytes')}</th>
          <th scope="col" class="cell-mono">Links</th>
          <th scope="col">Object ID</th>
          <th scope="col">Storage Characteristics</th>
          <th scope="col">Owner / Access</th>
          <th scope="col">Coverage Status</th>
        </tr>
      </thead>
      <tbody>
        ${sortedNodes.map(node => {
          const isSel = state.selectedNodeId === node.id;
          const obj = node.objectId ? MOCK_OBJECTS[node.objectId] : null;
          const isSummary = node.isSummaryRemainder || node.kind === 'summary_remainder';
          return `
            <tr class="${isSel ? 'selected' : ''} ${isSummary ? 'summary-row' : ''}" data-node-id="${node.id}" role="row" aria-selected="${isSel}" tabindex="0">
              <td class="cell-name">
                <span>${node.kind === 'directory' ? '📁' : (isSummary ? '📦' : '📄')}</span>
                <div>
                  <strong>${node.name}</strong>
                  <div style="font-size: 0.625rem; color: var(--text-dim); font-family: var(--font-mono); font-weight: normal;">${node.path}</div>
                </div>
              </td>
              <td class="cell-mono" style="font-weight: 600;">${formatBytes(node.uniqueAllocatedBytes)}</td>
              <td class="cell-mono">${formatBytes(node.referencedAllocatedBytes)}</td>
              <td class="cell-mono">${formatBytes(node.uniqueLogicalBytes ?? node.referencedLogicalBytes)}</td>
              <td class="cell-mono">${obj?.linksCount ?? (node.kind === 'directory' || isSummary ? '-' : 1)}</td>
              <td style="font-family: var(--font-mono); font-size: 0.6875rem;">${node.objectId || '-'}</td>
              <td>
                <span class="fact-tag ${obj?.storageCharacteristics?.includes('online-only') ? 'tag-purple' : (isSummary ? 'tag-purple' : '')}">
                  ${obj?.storageCharacteristics?.join(', ') || node.category || 'standard'}
                </span>
              </td>
              <td style="font-size: 0.6875rem; color: var(--text-dim); font-weight: normal;">${obj?.owner || 'Observed Principal'}</td>
              <td>
                <span class="status-badge ${node.observationStatus === 'inaccessible' ? 'badge-partial' : 'badge-complete'}">
                  ${node.observationStatus || 'observed'}
                </span>
              </td>
            </tr>
          `;
        }).join('')}

        <!-- Reconciliation Row (When viewing volume targets) -->
        ${reconItem ? `
          <tr class="reconciliation-row ${state.selectedNodeId === reconItem.id ? 'selected' : ''}" 
              data-node-id="${reconItem.id}" role="row" aria-selected="${state.selectedNodeId === reconItem.id}" tabindex="0">
            <td class="cell-name">
              <span style="color: var(--accent-amber);">⚖</span>
              <strong style="color: var(--accent-amber);">${reconItem.name}</strong>
            </td>
            <td class="cell-mono" style="color: var(--accent-amber); font-weight: 700;">${formatBytes(reconItem.allocatedBytes)}</td>
            <td class="cell-mono" style="color: var(--accent-amber);">${formatBytes(reconItem.allocatedBytes)}</td>
            <td class="cell-mono">${formatBytes(reconItem.logicalBytes)}</td>
            <td class="cell-mono">-</td>
            <td style="font-family: var(--font-mono); font-size: 0.6875rem;">reconciliation_diff</td>
            <td><span class="fact-tag tag-warning">Volume Reconciliation Diff</span></td>
            <td style="font-size: 0.6875rem;">Volume Accounting Boundary</td>
            <td><span class="status-badge badge-partial">RECONCILED</span></td>
          </tr>
        ` : ''}
      </tbody>
    </table>
  `;
}

function renderActiveViewTab(selectedNode) {
  const browsedDir = findNodeById(state.browsedDirectoryId) || getActiveTargetRoot();
  if (state.currentViewTab === 'table') return renderFolderTableView(browsedDir);
  if (state.currentViewTab === 'flat') return renderFlatFilesTableView();
  if (state.currentViewTab === 'treemap') {
    return `
      <div style="flex: 1; display: flex; flex-direction: column; height: 100%;">
        <div style="padding: 6px 12px; background: var(--bg-subtle); display: flex; justify-content: space-between; align-items: center; border-bottom: 1px solid var(--border-color);">
          <span style="font-size: 0.75rem; font-weight: 600;">Interactive Spatial Allocation Treemap</span>
          <button id="btn-toggle-treemap-text" class="btn btn-sm" aria-label="Toggle between visual Treemap and accessible Table">
            ${state.treemapShowTextEquivalent ? 'Show Treemap Visual' : 'Show Accessible Text/Table Equivalent'}
          </button>
        </div>
        <div style="flex: 1; overflow: hidden;">
          ${state.treemapShowTextEquivalent ? renderFolderTableView(browsedDir) : renderTreemapSvg(getActiveTargetRoot(), 800, 450)}
        </div>
      </div>
    `;
  }
  if (state.currentViewTab === 'types') return renderFileTypesView();
  if (state.currentViewTab === 'age') return renderAgeDistributionView();
  if (state.currentViewTab === 'largest') return renderLargestItemsView();
  return renderFolderTableView(browsedDir);
}

// -------------------------------------------------------------
// Interactive SVG Treemap Generator (Squarified Proportional Layout)
// -------------------------------------------------------------
function renderTreemapSvg(rootNode, width = 700, height = 300) {
  const children = (rootNode.children || []).filter(c => c.observationStatus !== 'inaccessible');
  if (children.length === 0) {
    return `<div style="padding: 20px; text-align: center; color: var(--text-dim);">No sub-items to render in treemap.</div>`;
  }

  // Include reconciliation item in volume treemap to make unattributed space visible
  const items = [...children];
  const recon = getActiveReconciliationItem();
  if (recon && (rootNode.id === 'node_root' || rootNode.id === 'node_root_d')) {
    items.push({
      id: recon.id,
      name: recon.name,
      uniqueAllocatedBytes: recon.allocatedBytes,
      referencedAllocatedBytes: recon.allocatedBytes,
      isReconciliation: true,
      category: 'Reconciliation'
    });
  }

  const totalValue = items.reduce((acc, cur) => acc + (cur.uniqueAllocatedBytes ?? cur.referencedAllocatedBytes ?? 1), 0);
  
  // Color palette for treemap blocks
  const colors = ['#2563eb', '#0d9488', '#d97706', '#7c3aed', '#dc2626', '#059669', '#4f46e5', '#ea580c'];

  // Slice-and-dice layout calculation
  let curX = 0;
  let curY = 0;
  let remWidth = width;
  let remHeight = height;

  const rects = [];
  
  items.forEach((item, idx) => {
    const val = item.uniqueAllocatedBytes ?? item.referencedAllocatedBytes ?? 1;
    const ratio = totalValue > 0 ? (val / totalValue) : (1 / items.length);
    const isHorizontal = remWidth >= remHeight;
    
    let w, h, x, y;
    if (isHorizontal) {
      w = Math.max(16, Math.round(remWidth * ratio));
      h = remHeight;
      x = curX;
      y = curY;
      curX += w;
      remWidth -= w;
    } else {
      w = remWidth;
      h = Math.max(16, Math.round(remHeight * ratio));
      x = curX;
      y = curY;
      curY += h;
      remHeight -= h;
    }

    rects.push({
      item,
      x,
      y,
      w,
      h,
      color: item.isReconciliation ? '#b45309' : (item.isSummaryRemainder ? '#64748b' : colors[idx % colors.length])
    });
  });

  return `
    <div class="treemap-container" style="height: ${height}px;" role="region" aria-label="Disk Allocation Treemap">
      <svg class="treemap-svg" viewBox="0 0 ${width} ${height}" preserveAspectRatio="none" role="img" aria-labelledby="treemap-title treemap-desc">
        <title id="treemap-title">Disk Storage Allocation Treemap</title>
        <desc id="treemap-desc">Proportional visual breakdown of storage sizes for entries in ${rootNode.path || rootNode.name}</desc>
        ${rects.map(r => {
          const isSel = state.selectedNodeId === r.item.id;
          const bytesFormatted = formatBytes(r.item.uniqueAllocatedBytes ?? r.item.referencedAllocatedBytes);
          return `
            <g class="treemap-cell-group" data-node-id="${r.item.id}" tabindex="0" role="button" aria-label="${r.item.name}: ${bytesFormatted}" aria-pressed="${isSel}">
              <rect class="treemap-cell ${isSel ? 'selected' : ''}" 
                    x="${r.x}" y="${r.y}" width="${r.w}" height="${r.h}" 
                    fill="${r.color}" fill-opacity="${isSel ? '1.0' : '0.85'}" rx="3" />
              ${r.w > 40 && r.h > 24 ? `
                <text class="treemap-label" x="${r.x + 6}" y="${r.y + 16}">
                  ${escapeHtml(truncate(r.item.name, Math.floor(r.w / 7)))}
                </text>
                <text class="treemap-sublabel" x="${r.x + 6}" y="${r.y + 30}">
                  ${bytesFormatted}
                </text>
              ` : ''}
            </g>
          `;
        }).join('')}
      </svg>
    </div>
  `;
}

function truncate(str, maxLen) {
  if (!str) return '';
  return str.length > maxLen ? str.slice(0, maxLen - 1) + '…' : str;
}

function escapeHtml(str) {
  if (!str) return '';
  return str.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');
}

// -------------------------------------------------------------
// Detail Inspector (Entry Facts vs Object Facts vs Coverage)
// -------------------------------------------------------------
function renderDetailInspector(node) {
  if (!node) {
    return `
      <div class="panel-header"><span>Detail Inspector</span></div>
      <div style="padding: 16px; color: var(--text-dim); text-align: center;">Select an item to inspect facts.</div>
    `;
  }

  // Handle Reconciliation Item
  if (node.isReconciliation) {
    const vol = MOCK_VOLUMES.find(v => v.id === state.activeVolumeId) || MOCK_VOLUMES[0];
    return `
      <div class="panel-header">
        <span>Volume Reconciliation Detail</span>
        <span class="fact-tag tag-warning">Not an Ordinary Directory</span>
      </div>
      <div class="inspector-card">
        <div class="inspector-section">
          <span class="inspector-section-title">Reconciliation Difference</span>
          <div class="fact-row">
            <span class="fact-label">Discrepancy Magnitude:</span>
            <span class="fact-value" style="color: var(--accent-amber); font-size: 0.875rem; font-weight: 700;">${formatBytes(node.allocatedBytes)}</span>
          </div>
          <div class="fact-row">
            <span class="fact-label">Volume Used Space:</span>
            <span class="fact-value">${formatBytes(vol.usedBytes)}</span>
          </div>
          <div class="fact-row">
            <span class="fact-label">Accounted Unique Allocation:</span>
            <span class="fact-value">${formatBytes(vol.accountedUniqueBytes)}</span>
          </div>
        </div>

        <div class="inspector-section">
          <span class="inspector-section-title">Explanation</span>
          <p style="font-size: 0.75rem; color: var(--text-muted); line-height: 1.4;">
            This ${formatBytes(node.allocatedBytes)} discrepancy represents Used Space reported by the filesystem that PigTree cannot defensibly attribute to individual observed filesystem objects.
          </p>
          <p style="font-size: 0.75rem; color: var(--text-muted); line-height: 1.4; margin-top: 6px;">
            Causes include:
          </p>
          <ul style="font-size: 0.6875rem; padding-left: 16px; color: var(--text-dim); margin-top: 4px;">
            <li>NTFS Master File Table ($MFT) and filesystem metadata reserves</li>
            <li>Inaccessible System Volume Information (shadow copies/restore points)</li>
            <li>Live filesystem allocations that shifted during traversal</li>
          </ul>
        </div>
      </div>
    `;
  }

  // Handle Mock Aggregate Summary Rows
  if (node.isSummaryRemainder || node.kind === 'summary_remainder') {
    return `
      <div class="panel-header">
        <span>Mock Aggregate Summary</span>
        <span class="fact-tag tag-purple">Summarized Remainder</span>
      </div>
      <div class="inspector-card">
        <div class="inspector-section">
          <span class="inspector-section-title">Scope Aggregate Facts</span>
          <div class="fact-row">
            <span class="fact-label">Group Name:</span>
            <span class="fact-value">${node.name}</span>
          </div>
          <div class="fact-row">
            <span class="fact-label">Summary Kind:</span>
            <span class="fact-value">Mock Aggregate Summary Row</span>
          </div>
          <div class="fact-row">
            <span class="fact-label">Aggregated Unique:</span>
            <span class="fact-value" style="color: var(--primary); font-weight: 700;">${formatBytes(node.uniqueAllocatedBytes)}</span>
          </div>
          <div class="fact-row">
            <span class="fact-label">Aggregated Logical:</span>
            <span class="fact-value">${formatBytes(node.uniqueLogicalBytes ?? node.referencedLogicalBytes)}</span>
          </div>
          <div class="fact-row">
            <span class="fact-label">Contained Entries:</span>
            <span class="fact-value">${(node.entryCount || 1).toLocaleString()} unexpanded files/folders</span>
          </div>
        </div>

        <div class="inspector-section">
          <span class="inspector-section-title">Prototype Dataset Note</span>
          <p style="font-size: 0.75rem; color: var(--text-muted); line-height: 1.4;">
            To represent a realistic 48,000+ entry filesystem within an in-memory prototype without shipping tens of thousands of individual DOM elements, unexpanded sibling entries are aggregated into this defensible summary row.
          </p>
          <p style="font-size: 0.75rem; color: var(--text-dim); line-height: 1.4; margin-top: 6px;">
            ⚠️ This summary item is <strong>non-selectable for cleanup</strong>. It is distinct from Coverage Gaps (inaccessible scopes) and Unattributed Used Space.
          </p>
        </div>
      </div>
    `;
  }

  const obj = node.objectId ? MOCK_OBJECTS[node.objectId] : null;
  const aliases = node.objectId ? HARDLINK_ALIASES[node.objectId] : null;

  return `
    <div class="panel-header">
      <span>Detail Inspector</span>
      <button class="btn btn-sm btn-warning btn-open-cleanup" data-node-id="${node.id}" aria-label="Open Guarded Cleanup Action Plan for ${node.name}">
        🛡 Cleanup Plan
      </button>
    </div>

    <div class="inspector-card">
      <!-- Section 1: Directory Entry Facts -->
      <div class="inspector-section">
        <span class="inspector-section-title">1. Directory Entry Facts</span>
        <div class="fact-row">
          <span class="fact-label">Entry Name:</span>
          <span class="fact-value"><strong>${node.name}</strong></span>
        </div>
        <div class="fact-row">
          <span class="fact-label">Observed Path:</span>
          <span class="fact-value" style="font-size: 0.6875rem; font-family: var(--font-mono); word-break: break-all;">${node.path}</span>
        </div>
        <div class="fact-row">
          <span class="fact-label">Entry Classification:</span>
          <span class="fact-value">${node.category || node.kind}</span>
        </div>
        <div class="fact-row">
          <span class="fact-label">Modified:</span>
          <span class="fact-value">${formatDate(node.modifiedTime)}</span>
        </div>
      </div>

      <!-- Section 2: Filesystem Object Facts -->
      <div class="inspector-section">
        <span class="inspector-section-title">2. Filesystem Object Facts</span>
        <div class="fact-row">
          <span class="fact-label">Object Identity:</span>
          <span class="fact-value" style="font-family: var(--font-mono);">${node.objectId || 'Implicit / Directory Node'}</span>
        </div>
        <div class="fact-row">
          <span class="fact-label">Physical Allocated:</span>
          <span class="fact-value" style="color: var(--primary); font-size: 0.8125rem; font-weight: 700;">
            ${formatBytes(node.uniqueAllocatedBytes ?? node.referencedAllocatedBytes)}
          </span>
        </div>
        <div class="fact-row">
          <span class="fact-label">Logical Size:</span>
          <span class="fact-value">${formatBytes(node.uniqueLogicalBytes ?? node.referencedLogicalBytes)}</span>
        </div>
        <div class="fact-row">
          <span class="fact-label">Links Count:</span>
          <span class="fact-value">${obj?.linksCount ?? 1} ${(obj?.linksCount > 1) ? '(Hardlinked)' : ''}</span>
        </div>
        <div class="fact-row">
          <span class="fact-label">Owner:</span>
          <span class="fact-value" style="font-size: 0.6875rem;">${obj?.owner || 'Observed Principal'}</span>
        </div>
        <div class="fact-row">
          <span class="fact-label">Access Rules:</span>
          <span class="fact-value" style="font-size: 0.6875rem;">${obj?.accessRules || 'Standard Inherited'}</span>
        </div>
      </div>

      <!-- Section 3: Hardlink Aliases (When Links Count > 1) -->
      ${aliases && aliases.length > 1 ? `
        <div class="inspector-section hardlink-section">
          <span class="inspector-section-title" style="color: var(--accent-purple);">Hard Link Aliases (${aliases.length} paths share 1 object)</span>
          <p style="font-size: 0.6875rem; color: var(--text-muted); margin-bottom: 4px;">
            These paths share the exact same underlying physical allocation on disk:
          </p>
          <ul style="font-size: 0.625rem; font-family: var(--font-mono); padding-left: 14px; display: flex; flex-direction: column; gap: 4px;">
            ${aliases.map(a => `
              <li style="${a === node.path ? 'font-weight: 700; color: var(--primary);' : ''}">${a}</li>
            `).join('')}
          </ul>
        </div>
      ` : ''}

      <!-- Section 4: Storage Characteristics (Cloud Online-Only, Reparse, Sparse) -->
      <div class="inspector-section">
        <span class="inspector-section-title">3. Storage Characteristics</span>
        <div class="fact-badge-list">
          ${obj?.storageCharacteristics ? obj.storageCharacteristics.map(c => `
            <span class="fact-tag ${c === 'online-only' ? 'tag-purple' : ''}">${c}</span>
          `).join('') : '<span class="fact-tag">standard</span>'}
        </div>
        ${obj?.storageCharacteristics?.includes('online-only') ? `
          <p style="font-size: 0.6875rem; color: var(--accent-purple); margin-top: 4px;">
            ℹ️ <strong>OneDrive Online-Only Placeholder:</strong> File content resides in the cloud. Allocated disk space is 0 B despite 4.8 GB logical size.
          </p>
        ` : ''}
      </div>

      <!-- Section 5: Observation Status & Value Knowledge -->
      <div class="inspector-section">
        <span class="inspector-section-title">4. Observation &amp; Knowledge State</span>
        <div class="fact-row">
          <span class="fact-label">Observation Status:</span>
          <span class="fact-value">${node.observationStatus || 'observed'}</span>
        </div>
        <div class="fact-row">
          <span class="fact-label">Value Knowledge:</span>
          <span class="fact-value">Known (Provenance: Live Scan)</span>
        </div>
        <div class="fact-row">
          <span class="fact-label">Observation Interval:</span>
          <span class="fact-value">2025-04-10 (42s duration)</span>
        </div>
      </div>

      <!-- Guarded Cleanup CTA -->
      <div style="margin-top: 8px;">
        <button class="btn btn-primary btn-open-cleanup" data-node-id="${node.id}" style="width: 100%;" aria-label="Review Guarded Cleanup Action Plan for ${node.name}">
          🛡 Review Guarded Cleanup Action Plan
        </button>
      </div>
    </div>
  `;
}

// -------------------------------------------------------------
// Coverage Gap Explanation Modal (Focus trapped, Escape close, Accessible)
// -------------------------------------------------------------
function renderCoverageGapModal(volume) {
  const gap = volume.coverageGaps[0] || {
    path: 'C:\\System Volume Information',
    reason: 'STATUS_ACCESS_DENIED under current security context',
    defensibleBound: 'Unknown allocation; volume shadow copies & system restore points reside here.',
    noncommittalPrompt: 'Additional security privileges or backup-intent read may reveal more metadata.'
  };

  return `
    <div class="modal-overlay" role="dialog" aria-modal="true" aria-labelledby="modal-gap-title">
      <div class="modal-card">
        <div class="modal-header">
          <h3 class="modal-title" id="modal-gap-title">⚠️ Coverage Gap Details</h3>
          <button id="btn-close-gap-modal" class="btn btn-sm" aria-label="Close coverage gap dialog">✕</button>
        </div>
        <div class="modal-body">
          <div class="callout-amber">
            <strong>Inaccessible Scope:</strong> <code>${gap.path}</code><br>
            <strong>Observation Status:</strong> <span class="status-badge badge-partial">INACCESSIBLE</span><br>
            <div style="margin-top: 6px; font-size: 0.75rem;"><strong>Reason:</strong> ${gap.reason}</div>
          </div>

          <div style="font-size: 0.75rem; color: var(--text-muted); line-height: 1.4;">
            <strong>Accounting Impact &amp; Known Subtotals:</strong><br>
            Because this scope was inaccessible, its allocated and logical sizes are recorded as <em>Unavailable</em> rather than zero. 
            The volume's Accounted Unique Allocation (${formatBytes(volume.accountedUniqueBytes)}) is a <em>Known Subtotal</em> that omits these unobserved entries.
          </div>

          <div style="font-size: 0.75rem; color: var(--text-muted); line-height: 1.4;">
            <strong>Defensible Bounds:</strong><br>
            ${gap.defensibleBound}
          </div>

          <div style="background: var(--bg-subtle); padding: 10px; border-radius: var(--radius-sm); border: 1px solid var(--border-color); font-size: 0.75rem;">
            💡 <em>${gap.noncommittalPrompt}</em>
          </div>
        </div>
        <div class="modal-footer">
          <button id="btn-dismiss-gap-modal" class="btn btn-primary" aria-label="Dismiss coverage gap dialog">Understood</button>
        </div>
      </div>
    </div>
  `;
}

// -------------------------------------------------------------
// Guarded Cleanup Action Plan Modal (Focus trapped, Escape close, Accessible)
// -------------------------------------------------------------
function renderCleanupModal() {
  const node = state.cleanupTargetNode || findNodeById(state.selectedNodeId) || getActiveTargetRoot();
  const obj = node.objectId ? MOCK_OBJECTS[node.objectId] : null;
  const isHardlinked = obj && obj.linksCount > 1;
  const isOnlineOnly = obj && obj.storageCharacteristics?.includes('online-only');
  const isSystemProtected = node.cleanupSafe === 'protected_system' || node.cleanupSafe === 'protected_dism_only' || node.cleanupSafe === 'system_critical_lock';

  // Compute defensible reclaimable allocation
  let reclaimableText = formatBytes(node.uniqueAllocatedBytes ?? node.referencedAllocatedBytes);
  let uncertaintyNote = 'Exact unshared allocation (single reference).';

  if (isHardlinked) {
    reclaimableText = '0 B (Reference Decrement Only)';
    uncertaintyNote = 'This object has multiple hard link references (e.g. WinSxS and System32). Deleting this single directory entry will NOT free physical disk allocation until the final reference is deleted.';
  } else if (isOnlineOnly) {
    reclaimableText = '0 B (Cloud File)';
    uncertaintyNote = 'This is an online-only placeholder with 0 B physical allocation on local disk. Deleting it will remove the cloud link without increasing free disk capacity.';
  } else if (node.kind === 'directory') {
    uncertaintyNote = 'Defensible bound across reachable entries. If any descendant object has external references outside this folder, actual freed bytes may be lower.';
  }

  return `
    <div class="modal-overlay" role="dialog" aria-modal="true" aria-labelledby="modal-cleanup-title">
      <div class="modal-card">
        <div class="modal-header">
          <h3 class="modal-title" id="modal-cleanup-title">🛡 Guarded Cleanup Action Plan Preview</h3>
          <button id="btn-close-modal" class="btn btn-sm" aria-label="Close cleanup dialog">✕</button>
        </div>

        <div class="modal-body">
          <div style="padding: 10px; background: var(--bg-subtle); border-radius: var(--radius-sm); border: 1px solid var(--border-color);">
            <div style="font-size: 0.6875rem; color: var(--text-dim); text-transform: uppercase; font-weight: 700;">Target Entry:</div>
            <div style="font-size: 0.875rem; font-weight: 700; margin-top: 2px;">${node.name}</div>
            <div style="font-size: 0.6875rem; font-family: var(--font-mono); color: var(--text-muted); word-break: break-all;">${node.path}</div>
          </div>

          <!-- Expected Reclaimable Allocation -->
          <div class="fact-row" style="background: var(--primary-subtle); padding: 10px; border-radius: var(--radius-sm);">
            <span style="font-weight: 700; color: var(--primary);">Expected Reclaimable Allocation:</span>
            <span style="font-weight: 800; font-size: 1rem; color: var(--primary); font-family: var(--font-mono);">
              ${reclaimableText}
            </span>
          </div>

          <!-- Uncertainty & Accounting Notice -->
          <div style="font-size: 0.75rem; color: var(--text-muted); line-height: 1.4;">
            <strong>Accounting &amp; Uncertainty Analysis:</strong> ${uncertaintyNote}
          </div>

          <!-- Remediation Guidance (Accessible dark/light friendly colors) -->
          ${isSystemProtected ? `
            <div class="callout-rose">
              <strong>⚠️ Operating System Protected Path:</strong><br>
              Direct deletion of Windows OS binaries or WinSxS component store files will compromise system stability. 
              Use native OS tools like <code>DISM /Online /Cleanup-Image /StartComponentCleanup</code> or Windows Disk Cleanup instead.
            </div>
          ` : (node.cleanupSafe === 'native_uninstall' ? `
            <div class="callout-amber">
              <strong>💡 Recommended Remediation:</strong><br>
              This is an installed application / game package. Recommended action is using Windows Settings → Installed Apps or the game launcher to perform a clean native uninstallation.
            </div>
          ` : `
            <div class="callout-emerald">
              <strong>✓ Safe User File:</strong><br>
              This entry is within your personal user directory. Can be moved to Recycle Bin or deleted permanently after personal review.
            </div>
          `)}

          <!-- Recovery Expectations -->
          <div style="font-size: 0.75rem;">
            <strong>Recovery Options:</strong>
            <div style="display: flex; gap: 12px; margin-top: 6px;">
              <label style="display: flex; align-items: center; gap: 4px;">
                <input type="radio" name="cleanup_mode" checked> Move to Recycle Bin (Recoverable)
              </label>
              <label style="display: flex; align-items: center; gap: 4px;">
                <input type="radio" name="cleanup_mode"> Permanent Deletion (Irreversible)
              </label>
            </div>
          </div>
        </div>

        <div class="modal-footer">
          <button id="btn-cancel-modal" class="btn" aria-label="Cancel action plan">Cancel</button>
          ${isSystemProtected ? `
            <button id="btn-remediate-native" class="btn btn-warning" aria-label="Launch Windows Native Cleanup Tool guidance">Launch Windows Native Cleanup Tool</button>
          ` : `
            <button id="btn-confirm-action-plan" class="btn btn-danger" aria-label="Confirm simulated action plan handoff">Simulate Action Plan Handoff</button>
          `}
        </div>
      </div>
    </div>
  `;
}

// -------------------------------------------------------------
// Event Listeners, Focus Trap, and Keyboard Navigation
// -------------------------------------------------------------
function attachEventListeners() {
  // Variant switcher buttons
  const btnPrev = document.getElementById('btn-var-prev');
  const btnNext = document.getElementById('btn-var-next');
  if (btnPrev && btnNext) {
    const variants = ['explorer', 'insights', 'workbench'];
    btnPrev.onclick = () => {
      const idx = variants.indexOf(state.variant);
      const nextIdx = (idx - 1 + variants.length) % variants.length;
      updateUrlVariant(variants[nextIdx]);
    };
    btnNext.onclick = () => {
      const idx = variants.indexOf(state.variant);
      const nextIdx = (idx + 1) % variants.length;
      updateUrlVariant(variants[nextIdx]);
    };
  }

  // Target selector (Requirement 6)
  const targetSelect = document.getElementById('target-select');
  if (targetSelect) {
    targetSelect.onchange = (e) => {
      const val = e.target.value;
      if (val === 'volume:vol_c') {
        state.activeTargetType = 'volume';
        state.activeVolumeId = 'vol_c';
        state.activeSnapshotId = null;
        state.selectedNodeId = 'node_root';
        state.browsedDirectoryId = 'node_root';
        state.scanEntriesObserved = 48210;
      } else if (val === 'volume:vol_d') {
        state.activeTargetType = 'volume';
        state.activeVolumeId = 'vol_d';
        state.activeSnapshotId = null;
        state.selectedNodeId = 'node_root_d';
        state.browsedDirectoryId = 'node_root_d';
        state.scanEntriesObserved = 20950;
      } else if (val === 'directory:alex_downloads') {
        state.activeTargetType = 'directory';
        state.activeVolumeId = 'vol_c';
        state.activeSnapshotId = null;
        state.selectedNodeId = 'node_alex_downloads';
        state.browsedDirectoryId = 'node_alex_downloads';
        state.scanEntriesObserved = 124;
      } else if (val === 'historical:snap_c_prev_month') {
        state.activeTargetType = 'historical';
        state.activeVolumeId = 'vol_c';
        state.activeSnapshotId = 'snap_c_prev_month';
        state.selectedNodeId = 'node_root';
        state.browsedDirectoryId = 'node_root';
        state.scanEntriesObserved = 46800;
      } else if (val === 'historical:snap_downloads_archive') {
        state.activeTargetType = 'historical';
        state.activeVolumeId = 'vol_c';
        state.activeSnapshotId = 'snap_downloads_archive';
        state.selectedNodeId = 'node_snap_downloads_root';
        state.browsedDirectoryId = 'node_snap_downloads_root';
        state.scanEntriesObserved = 85;
      }
      recordAction(`Changed scan target to ${val}`);
      renderApp();
    };
  }

  // Preset selector
  const presetSelect = document.getElementById('preset-select');
  if (presetSelect) {
    presetSelect.onchange = (e) => {
      state.activePresetId = e.target.value;
      recordAction(`Selected profile preset ${state.activePresetId}`);
      renderApp();
    };
  }

  // Scan action button with progress animation simulation
  const btnScan = document.getElementById('btn-scan-action');
  if (btnScan) {
    btnScan.onclick = () => {
      if (state.scanStatus === 'scanning') return;
      state.scanStatus = 'scanning';
      state.scanProgress = 20;
      state.scanEntriesObserved = Math.round(getActiveTargetRoot().entryCount * 0.2);
      renderApp();
      recordAction('Started scan traversal');
      
      const targetEntries = getActiveTargetRoot().entryCount || 48210;
      const interval = setInterval(() => {
        state.scanProgress += 25;
        state.scanEntriesObserved = Math.min(targetEntries, Math.round(targetEntries * (state.scanProgress / 100)));
        if (state.scanProgress >= 100) {
          clearInterval(interval);
          state.scanProgress = 100;
          state.scanEntriesObserved = targetEntries;
          state.scanStatus = 'finished';
          recordAction('Completed scan traversal');
        }
        renderApp();
      }, 120);
    };
  }

  // Load Historical Snapshot button
  const btnOpenHistorical = document.getElementById('btn-open-historical');
  if (btnOpenHistorical) {
    btnOpenHistorical.onclick = () => {
      state.activeTargetType = 'historical';
      state.activeSnapshotId = 'snap_c_prev_month';
      state.selectedNodeId = 'node_root';
      state.browsedDirectoryId = 'node_root';
      recordAction('Loaded historical snapshot (March 27, 2025)');
      renderApp();
    };
  }

  const btnExitHistorical = document.getElementById('btn-exit-historical');
  if (btnExitHistorical) {
    btnExitHistorical.onclick = () => {
      state.activeTargetType = 'volume';
      state.activeVolumeId = 'vol_c';
      state.activeSnapshotId = null;
      state.selectedNodeId = 'node_root';
      state.browsedDirectoryId = 'node_root';
      recordAction('Exited historical snapshot view to live C:\\');
      renderApp();
    };
  }

  // Table sorting headers (Click and Keyboard Enter/Space)
  document.querySelectorAll('th[data-sort-field]').forEach(th => {
    const handleSort = () => {
      const field = th.getAttribute('data-sort-field');
      if (state.sortField === field) {
        state.sortDirection = state.sortDirection === 'asc' ? 'desc' : 'asc';
      } else {
        state.sortField = field;
        state.sortDirection = 'desc';
      }
      recordAction(`Sorted table by ${field} (${state.sortDirection})`);
      renderApp();
    };

    th.onclick = handleSort;
    th.onkeydown = (e) => {
      if (e.key === 'Enter' || e.key === ' ') {
        e.preventDefault();
        handleSort();
      }
    };
  });

  // View tab buttons
  document.querySelectorAll('.tab-btn').forEach(btn => {
    btn.onclick = () => {
      const tab = btn.getAttribute('data-tab');
      if (tab) {
        state.currentViewTab = tab;
        recordAction(`Switched view tab to ${tab}`);
        renderApp();
      }
    };
  });

  // Tree toggle buttons (expand/collapse)
  document.querySelectorAll('.tree-toggle-btn').forEach(btn => {
    btn.onclick = (e) => {
      e.stopPropagation();
      const id = btn.getAttribute('data-toggle-id');
      if (state.expandedNodes.has(id)) {
        state.expandedNodes.delete(id);
      } else {
        state.expandedNodes.add(id);
      }
      recordAction(`Toggled tree node ${id}`);
      renderApp();
    };
  });

  // Tree items keyboard navigation (Up/Down/Left/Right/Enter/Space)
  document.querySelectorAll('.tree-node').forEach(item => {
    item.onkeydown = (e) => {
      const id = item.getAttribute('data-node-id');
      const allTreeItems = Array.from(document.querySelectorAll('.tree-view .tree-node'));
      const idx = allTreeItems.indexOf(item);

      if (e.key === 'ArrowDown') {
        e.preventDefault();
        if (idx < allTreeItems.length - 1) {
          allTreeItems[idx + 1].focus();
        }
      } else if (e.key === 'ArrowUp') {
        e.preventDefault();
        if (idx > 0) {
          allTreeItems[idx - 1].focus();
        }
      } else if (e.key === 'ArrowRight') {
        e.preventDefault();
        if (id && !state.expandedNodes.has(id)) {
          state.expandedNodes.add(id);
          renderApp();
        }
      } else if (e.key === 'ArrowLeft') {
        e.preventDefault();
        if (id && state.expandedNodes.has(id)) {
          state.expandedNodes.delete(id);
          renderApp();
        } else {
          const parentNode = findParentNode(id);
          if (parentNode) {
            const parentEl = document.querySelector(`.tree-node[data-node-id="${parentNode.id}"]`);
            if (parentEl) parentEl.focus();
          }
        }
      } else if (e.key === 'Enter' || e.key === ' ') {
        e.preventDefault();
        selectNode(id);
      }
    };
  });

  // Node selection in tree, table, treemap, cards (Requirement 1 & 5)
  document.querySelectorAll('[data-node-id]').forEach(el => {
    el.onclick = (e) => {
      if (e.target.closest('.tree-toggle-btn') || e.target.closest('.btn-open-cleanup')) return;
      const id = el.getAttribute('data-node-id');
      if (id) {
        selectNode(id);
      }
    };

    if (el.getAttribute('role') === 'row' || el.classList.contains('treemap-cell-group')) {
      el.onkeydown = (e) => {
        if (e.key === 'Enter' || e.key === ' ') {
          e.preventDefault();
          const id = el.getAttribute('data-node-id');
          if (id) selectNode(id);
        }
      };
    }
  });

  function selectNode(id) {
    state.selectedNodeId = id;
    const targetNode = findNodeById(id);
    if (targetNode) {
      if (targetNode.kind === 'directory') {
        state.browsedDirectoryId = targetNode.id;
      } else {
        const parent = findParentNode(targetNode.id);
        if (parent) state.browsedDirectoryId = parent.id;
      }
    }
    recordAction(`Selected node ${id}`);
    renderApp();
  }

  // Treemap toggle text equivalent
  const btnToggleTreemapText = document.getElementById('btn-toggle-treemap-text');
  if (btnToggleTreemapText) {
    btnToggleTreemapText.onclick = () => {
      state.treemapShowTextEquivalent = !state.treemapShowTextEquivalent;
      recordAction(`Toggled treemap text equivalent to ${state.treemapShowTextEquivalent}`);
      renderApp();
    };
  }
  const btnToggleSubpanelText = document.getElementById('btn-toggle-subpanel-text');
  if (btnToggleSubpanelText) {
    btnToggleSubpanelText.onclick = () => {
      state.treemapShowTextEquivalent = !state.treemapShowTextEquivalent;
      recordAction(`Toggled spatial preview text equivalent`);
      renderApp();
    };
  }
  const btnToggleWorkbenchTreemap = document.getElementById('btn-toggle-workbench-treemap-text');
  if (btnToggleWorkbenchTreemap) {
    btnToggleWorkbenchTreemap.onclick = () => {
      state.treemapShowTextEquivalent = !state.treemapShowTextEquivalent;
      recordAction(`Toggled workbench treemap text equivalent`);
      renderApp();
    };
  }

  // Coverage Gap dialog triggers (Requirement 1)
  const gapLink = document.getElementById('view-coverage-gaps');
  if (gapLink) {
    gapLink.onclick = () => openCoverageGapModal(gapLink);
  }
  const btnInsightsGap = document.getElementById('btn-insights-view-gap');
  if (btnInsightsGap) {
    btnInsightsGap.onclick = () => openCoverageGapModal(btnInsightsGap);
  }

  function openCoverageGapModal(triggerEl) {
    state.modalTriggerElement = triggerEl;
    state.coverageGapModalOpen = true;
    recordAction('Opened Coverage Gap explanation modal');
    renderApp();
  }

  const btnCloseGap = document.getElementById('btn-close-gap-modal');
  const btnDismissGap = document.getElementById('btn-dismiss-gap-modal');
  if (btnCloseGap) btnCloseGap.onclick = closeCoverageGapModal;
  if (btnDismissGap) btnDismissGap.onclick = closeCoverageGapModal;

  function closeCoverageGapModal() {
    state.coverageGapModalOpen = false;
    recordAction('Closed coverage gap modal');
    renderApp();
    if (state.modalTriggerElement) {
      state.modalTriggerElement.focus();
      state.modalTriggerElement = null;
    }
  }

  // Filter Search Input in Explorer
  const explorerSearch = document.getElementById('input-explorer-search');
  if (explorerSearch) {
    explorerSearch.oninput = (e) => {
      state.filterSearch = e.target.value;
      renderApp();
    };
  }

  // Filter Size Select in Explorer
  const explorerSize = document.getElementById('select-explorer-size');
  if (explorerSize) {
    explorerSize.onchange = (e) => {
      state.filterSizeMin = parseInt(e.target.value, 10) || 0;
      recordAction(`Set min size filter to ${state.filterSizeMin}`);
      renderApp();
    };
  }

  // Filter Type Select in Explorer
  const explorerType = document.getElementById('select-explorer-type');
  if (explorerType) {
    explorerType.onchange = (e) => {
      state.filterType = e.target.value;
      recordAction(`Set type filter to ${state.filterType}`);
      renderApp();
    };
  }

  // Clear filters button
  const btnClearFilters = document.getElementById('btn-clear-filters');
  if (btnClearFilters) {
    btnClearFilters.onclick = () => {
      state.filterSearch = '';
      state.filterSizeMin = 0;
      state.filterType = 'all';
      recordAction('Cleared all filters');
      renderApp();
    };
  }

  // Workbench Command input
  const workbenchCmd = document.getElementById('input-workbench-cmd');
  if (workbenchCmd) {
    workbenchCmd.oninput = (e) => {
      state.filterSearch = e.target.value;
      renderApp();
    };
  }

  // Workbench Filter Tokens
  document.querySelectorAll('.filter-token[data-filter-size], .filter-token[data-filter-type]').forEach(token => {
    token.onclick = () => {
      const sizeVal = token.getAttribute('data-filter-size');
      const typeVal = token.getAttribute('data-filter-type');
      if (sizeVal !== null) {
        state.filterSizeMin = parseInt(sizeVal, 10);
      }
      if (typeVal !== null) {
        state.filterType = typeVal;
      }
      recordAction('Toggled filter token');
      renderApp();
    };
  });

  // Workbench Preset Token: Cycle through presets on click or Enter/Space (Requirement 3)
  const btnWorkbenchPreset = document.getElementById('btn-workbench-preset');
  if (btnWorkbenchPreset) {
    const cyclePreset = () => {
      const presetIds = PRESETS.map(p => p.id);
      const curIdx = presetIds.indexOf(state.activePresetId);
      const nextIdx = (curIdx + 1) % presetIds.length;
      state.activePresetId = presetIds[nextIdx];
      recordAction(`Cycled active preset to ${state.activePresetId}`);
      renderApp();
    };
    btnWorkbenchPreset.onclick = cyclePreset;
    btnWorkbenchPreset.onkeydown = (e) => {
      if (e.key === 'Enter' || e.key === ' ') {
        e.preventDefault();
        cyclePreset();
      }
    };
  }

  // Workbench Guarded Action Plan Button: Wired to selected item (Requirement 2)
  const btnWorkbenchCleanup = document.getElementById('btn-open-workbench-cleanup');
  if (btnWorkbenchCleanup) {
    btnWorkbenchCleanup.onclick = (e) => {
      e.stopPropagation();
      const node = findNodeById(state.selectedNodeId) || getActiveTargetRoot();
      openCleanupModal(node, btnWorkbenchCleanup);
    };
  }

  // Guarded Cleanup Modal Triggers
  document.querySelectorAll('.btn-open-cleanup').forEach(btn => {
    btn.onclick = (e) => {
      e.stopPropagation();
      const nodeId = btn.getAttribute('data-node-id') || state.selectedNodeId;
      const targetNode = findNodeById(nodeId) || getActiveTargetRoot();
      openCleanupModal(targetNode, btn);
    };
  });

  function openCleanupModal(node, triggerEl) {
    if (node?.isSummaryRemainder || node?.kind === 'summary_remainder') {
      alert('Mock Aggregate Summary rows represent groups of unexpanded items and cannot be targeted directly for cleanup. Select a specific file or folder.');
      return;
    }
    state.modalTriggerElement = triggerEl;
    state.cleanupTargetNode = node;
    state.cleanupModalOpen = true;
    recordAction(`Opened Guarded Cleanup Preview for ${node?.name}`);
    renderApp();
  }

  // Close Cleanup Modal (Requirement 12)
  const btnCloseModal = document.getElementById('btn-close-modal');
  const btnCancelModal = document.getElementById('btn-cancel-modal');
  if (btnCloseModal) btnCloseModal.onclick = closeCleanupModal;
  if (btnCancelModal) btnCancelModal.onclick = closeCleanupModal;

  const btnConfirmPlan = document.getElementById('btn-confirm-action-plan');
  if (btnConfirmPlan) {
    btnConfirmPlan.onclick = () => {
      alert(`Action Plan simulated! Guarded cleanup plan generated for ${state.cleanupTargetNode?.name}.`);
      closeCleanupModal();
    };
  }

  const btnRemediateNative = document.getElementById('btn-remediate-native');
  if (btnRemediateNative) {
    btnRemediateNative.onclick = () => {
      alert('Windows native cleanup tool guidance: Launch Cleanmgr.exe or DISM /Online /Cleanup-Image /StartComponentCleanup.');
      closeCleanupModal();
    };
  }

  function closeCleanupModal() {
    state.cleanupModalOpen = false;
    state.cleanupTargetNode = null;
    recordAction('Closed cleanup dialog');
    renderApp();
    if (state.modalTriggerElement) {
      state.modalTriggerElement.focus();
      state.modalTriggerElement = null;
    }
  }

  // Responsive Inspector Drawer Toggles (Requirement 16)
  const btnToggleDrawer = document.getElementById('btn-toggle-inspector-drawer');
  if (btnToggleDrawer) {
    btnToggleDrawer.onclick = () => {
      state.inspectorDrawerOpen = !state.inspectorDrawerOpen;
      renderApp();
    };
  }
  const btnToggleInsightsInspector = document.getElementById('btn-toggle-insights-inspector');
  if (btnToggleInsightsInspector) {
    btnToggleInsightsInspector.onclick = () => {
      state.inspectorDrawerOpen = !state.inspectorDrawerOpen;
      renderApp();
    };
  }
  const btnToggleWorkbenchPanels = document.getElementById('btn-toggle-workbench-panels');
  if (btnToggleWorkbenchPanels) {
    btnToggleWorkbenchPanels.onclick = () => {
      state.inspectorDrawerOpen = !state.inspectorDrawerOpen;
      renderApp();
    };
  }

  // Toggle Prototype State Debug Panel
  const btnToggleState = document.getElementById('btn-toggle-state-panel');
  if (btnToggleState) {
    btnToggleState.onclick = () => {
      state.statePanelOpen = !state.statePanelOpen;
      renderApp();
    };
  }
  const btnCloseState = document.getElementById('btn-close-state-panel');
  if (btnCloseState) {
    btnCloseState.onclick = () => {
      state.statePanelOpen = false;
      renderApp();
    };
  }
}

function renderStatePanel() {
  const display = document.getElementById('state-json-display');
  if (display) {
    display.textContent = JSON.stringify(getDebugState(), null, 2);
  }
}

// Global Keyboard Navigation & Focus Trap (Requirement 10 & 12)
window.addEventListener('keydown', (e) => {
  // Modal Focus Trap
  if (state.cleanupModalOpen || state.coverageGapModalOpen) {
    if (e.key === 'Escape') {
      e.preventDefault();
      if (state.cleanupModalOpen) {
        state.cleanupModalOpen = false;
        state.cleanupTargetNode = null;
      }
      if (state.coverageGapModalOpen) {
        state.coverageGapModalOpen = false;
      }
      renderApp();
      if (state.modalTriggerElement) {
        state.modalTriggerElement.focus();
        state.modalTriggerElement = null;
      }
      return;
    }

    if (e.key === 'Tab') {
      const modal = document.querySelector('.modal-card');
      if (modal) {
        const focusables = Array.from(modal.querySelectorAll('button, input, [tabindex="0"], a')).filter(el => !el.disabled);
        if (focusables.length > 0) {
          const first = focusables[0];
          const last = focusables[focusables.length - 1];
          if (e.shiftKey && document.activeElement === first) {
            e.preventDefault();
            last.focus();
          } else if (!e.shiftKey && document.activeElement === last) {
            e.preventDefault();
            first.focus();
          }
        }
      }
      return;
    }
  }

  // Do NOT globally hijack Left/Right when focus is on ANY interactive widget
  const activeEl = document.activeElement;
  const isInteractive = activeEl && (
    activeEl.tagName === 'INPUT' ||
    activeEl.tagName === 'TEXTAREA' ||
    activeEl.tagName === 'SELECT' ||
    activeEl.tagName === 'BUTTON' ||
    activeEl.tagName === 'A' ||
    activeEl.getAttribute('role') === 'treeitem' ||
    activeEl.getAttribute('role') === 'tab' ||
    activeEl.getAttribute('role') === 'row' ||
    activeEl.getAttribute('role') === 'columnheader' ||
    activeEl.getAttribute('role') === 'button' ||
    activeEl.closest('.modal-card')
  );

  // Variant shortcuts 1, 2, 3 outside inputs
  const isTyping = activeEl && ['INPUT', 'TEXTAREA', 'SELECT'].includes(activeEl.tagName);
  if (!isTyping) {
    if (e.key === '1') { updateUrlVariant('explorer'); return; }
    if (e.key === '2') { updateUrlVariant('insights'); return; }
    if (e.key === '3') { updateUrlVariant('workbench'); return; }
  }

  // Left/Right variant cycling ONLY when focus is outside interactive controls (e.g. on body/container)
  if (!isInteractive) {
    if (e.key === 'ArrowLeft') {
      const variants = ['explorer', 'insights', 'workbench'];
      const idx = variants.indexOf(state.variant);
      const nextIdx = (idx - 1 + variants.length) % variants.length;
      updateUrlVariant(variants[nextIdx]);
    } else if (e.key === 'ArrowRight') {
      const variants = ['explorer', 'insights', 'workbench'];
      const idx = variants.indexOf(state.variant);
      const nextIdx = (idx + 1) % variants.length;
      updateUrlVariant(variants[nextIdx]);
    }
  }
});

// History popstate navigation (Requirement 19)
window.addEventListener('popstate', (e) => {
  const params = new URLSearchParams(window.location.search);
  const variantParam = params.get('variant');
  if (['explorer', 'insights', 'workbench'].includes(variantParam)) {
    state.variant = variantParam;
    renderApp();
  }
});

// Initialization
document.addEventListener('DOMContentLoaded', () => {
  // Read URL search params for initial variant
  const params = new URLSearchParams(window.location.search);
  const variantParam = params.get('variant');
  if (['explorer', 'insights', 'workbench'].includes(variantParam)) {
    state.variant = variantParam;
  }
  renderApp();
});
