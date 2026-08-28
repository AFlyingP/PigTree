// PigTree Core Analysis Workflows - Prototype Script
// Dependency-free single-route interactive prototype

import {
  PRESETS,
  MOCK_VOLUMES,
  HISTORICAL_SNAPSHOTS,
  MOCK_OBJECTS,
  MOCK_TREE_ROOT,
  RECONCILIATION_ITEM,
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
  expandedNodes: new Set(['node_root', 'node_users', 'node_user_alex', 'node_alex_downloads']),
  
  // Modals
  cleanupModalOpen: false,
  cleanupTargetNode: null,
  coverageGapModalOpen: false,
  
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
    return d.toLocaleDateString() + ' ' + d.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
  } catch (e) {
    return isoString;
  }
}

// Flat list collector for searches & tables
function getAllFlatNodes(root) {
  const list = [];
  function traverse(node, parentPath = '') {
    list.push(node);
    if (node.children) {
      for (const child of node.children) {
        traverse(child, node.path);
      }
    }
  }
  traverse(root);
  return list;
}

// Find node by ID
function findNodeById(id, root = MOCK_TREE_ROOT) {
  if (!id) return null;
  if (id === RECONCILIATION_ITEM.id) return RECONCILIATION_ITEM;
  if (root.id === id) return root;
  if (root.children) {
    for (const child of root.children) {
      const found = findNodeById(id, child);
      if (found) return found;
    }
  }
  return null;
}

// Calculate active target root
function getActiveTargetRoot() {
  if (state.activeTargetType === 'directory') {
    const downloadsNode = findNodeById('node_alex_downloads');
    return downloadsNode || MOCK_TREE_ROOT;
  }
  return MOCK_TREE_ROOT;
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
    const size = node.referencedAllocatedBytes || 0;
    if (size < state.filterSizeMin) return false;
  }
  
  // Type filter
  if (state.filterType !== 'all') {
    const ext = (node.fileExt || '').toLowerCase();
    if (state.filterType === 'archives' && !['.zip', '.gz', '.tar', '.iso', '.rar', '.7z'].includes(ext)) return false;
    if (state.filterType === 'apps' && !['.exe', '.dll', '.msi'].includes(ext)) return false;
    if (state.filterType === 'games' && !['.pak', '.exe'].includes(ext) && !node.path.includes('Games')) return false;
    if (state.filterType === 'system' && !['.sys', '.dll'].includes(ext) && !node.path.includes('Windows')) return false;
  }
  
  return true;
}

// Sorting comparator helper
function sortNodes(nodes) {
  const mult = state.sortDirection === 'asc' ? 1 : -1;
  return [...nodes].sort((a, b) => {
    let valA = a[state.sortField];
    let valB = b[state.sortField];

    if (state.sortField === 'name') {
      valA = (a.name || '').toLowerCase();
      valB = (b.name || '').toLowerCase();
      return mult * valA.localeCompare(valB);
    }
    
    valA = valA ?? (a.uniqueAllocatedBytes ?? a.referencedAllocatedBytes ?? 0);
    valB = valB ?? (b.uniqueAllocatedBytes ?? b.referencedAllocatedBytes ?? 0);
    return mult * (valA - valB);
  });
}

// Update URL with current variant
function updateUrlVariant(variantKey) {
  state.variant = variantKey;
  const url = new URL(window.location);
  url.searchParams.set('variant', variantKey);
  window.history.replaceState({}, '', url);
  recordAction(`Switched variant to ${variantKey}`);
  renderApp();
}

function recordAction(actionName) {
  state.lastAction = `${actionName} (${new Date().toLocaleTimeString()})`;
  renderStatePanel();
}

// Main Render Dispatcher
export function renderApp() {
  const container = document.getElementById('app-root');
  if (!container) return;
  
  const currentVolume = MOCK_VOLUMES.find(v => v.id === state.activeVolumeId) || MOCK_VOLUMES[0];
  const isHistorical = state.activeTargetType === 'historical';
  
  container.innerHTML = `
    <div class="app-container">
      <!-- Top Application Header -->
      <header class="app-header" role="banner">
        <div class="brand-section">
          <span class="logo-badge">PigTree</span>
          <h1 class="brand-title">PigTree</h1>
          <span class="brand-tagline">Disk Space &amp; Storage Analyzer</span>
        </div>

        <!-- Scan Target & Analysis Profile Controls -->
        <div class="header-controls" role="region" aria-label="Scan Controls">
          <div class="control-group">
            <label class="control-label" for="target-select">Target:</label>
            <select id="target-select" class="select-input" aria-label="Select Scan Target">
              <option value="volume:vol_c" ${state.activeTargetType === 'volume' ? 'selected' : ''}>Whole Volume: Local Disk (C:)</option>
              <option value="directory:alex_downloads" ${state.activeTargetType === 'directory' ? 'selected' : ''}>Folder: C:\\Users\\Alex\\Downloads</option>
              <option value="historical:snap_c_prev_month" ${state.activeTargetType === 'historical' ? 'selected' : ''}>Saved Snapshot: C:\\ (March 27, 2025)</option>
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
          <span><strong>🕒 Historical Analysis Snapshot:</strong> Showing observations recorded on March 27, 2025. Reopening does not assert that paths or files still exist or remain current.</span>
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
            <span class="status-badge ${currentVolume.coverage === 'complete' ? 'badge-complete' : 'badge-partial'}">
              ${currentVolume.coverage === 'complete' ? 'COMPLETE' : 'PARTIAL'}
            </span>
          </div>
          ${currentVolume.coverageGaps.length > 0 ? `
            <div>
              <span class="gap-link" id="view-coverage-gaps" tabindex="0" role="button" aria-label="View Coverage Gaps">
                ⚠️ ${currentVolume.coverageGaps.length} Coverage Gap (Inaccessible Path)
              </span>
            </div>
          ` : ''}
          <div>
            <span style="color: var(--text-dim);">Observed: ${state.scanEntriesObserved.toLocaleString()} entries in 42s (~24,500/s) • No atomic snapshot claimed</span>
          </div>
        </div>

        <div style="font-size: 11px; color: var(--text-muted); display: flex; align-items: center; gap: 8px;">
          <span>Capacity: <strong>512 GB</strong></span> | 
          <span>Used: <strong>392 GB</strong></span> | 
          <span>Accounted: <strong>368 GB</strong></span> | 
          <span style="color: var(--accent-amber); font-weight: 700;">Unattributed: <strong>24 GB</strong></span>
        </div>
      </div>

      <!-- Main Variant Workspace -->
      <main class="workspace-root" id="workspace-content">
        ${renderVariantContent()}
      </main>

      <!-- Floating Prototype Variant Switcher (Bottom-Center) -->
      <div class="variant-switcher" role="region" aria-label="Prototype Variant Switcher">
        <button id="btn-var-prev" class="switcher-btn" aria-label="Previous Variant">←</button>
        <span>Workflow Variant:</span>
        <span class="switcher-pill">${getVariantDisplayName(state.variant)}</span>
        <button id="btn-var-next" class="switcher-btn" aria-label="Next Variant">→</button>
      </div>

      <!-- Collapsible Prototype State Panel (Required by prototype skill) -->
      <div class="state-panel-container">
        <button id="btn-toggle-state-panel" class="state-toggle-btn" aria-expanded="${state.statePanelOpen}">
          ⚙ Prototype State ${state.statePanelOpen ? '▲' : '▼'}
        </button>
        ${state.statePanelOpen ? `
          <div class="state-panel-card" role="region" aria-label="Prototype Debug State">
            <div class="panel-header">
              <span>In-Memory Prototype State</span>
              <button id="btn-close-state-panel" class="btn btn-sm">✕</button>
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
}

function getVariantDisplayName(key) {
  if (key === 'explorer') return '1: Explorer (Navigation-First)';
  if (key === 'insights') return '2: Insights (Question-First)';
  if (key === 'workbench') return '3: Workbench (Dense Expert)';
  return key;
}

function getDebugState() {
  return {
    variant: state.variant,
    target: state.activeTargetType,
    volumeId: state.activeVolumeId,
    preset: state.activePresetId,
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
    coverage: MOCK_VOLUMES.find(v => v.id === state.activeVolumeId)?.coverage,
    reconciliation: {
      capacityBytes: '512 GB',
      usedBytes: '392 GB',
      accountedUniqueBytes: '368 GB',
      unattributedUsedBytes: '24 GB (Positive Reconciliation Difference)'
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
  
  return `
    <div class="variant-explorer-layout">
      <!-- Left: Folder Tree Navigation -->
      <aside class="explorer-sidebar" aria-label="Directory Tree Navigation">
        <div class="panel-header">
          <span>Folder Navigation</span>
          <span style="font-weight: normal; font-size: 11px; color: var(--text-dim);">Reachable Scopes</span>
        </div>
        <div class="panel-body tree-view" role="tree" aria-label="Folders">
          ${renderTreeNode(rootNode, 0)}
          <!-- Reconciliation Item (NOT a folder!) -->
          <div class="tree-node ${state.selectedNodeId === RECONCILIATION_ITEM.id ? 'selected' : ''}" 
               data-node-id="${RECONCILIATION_ITEM.id}" role="treeitem" tabindex="0">
            <span class="tree-icon" style="color: var(--accent-amber);">⚖</span>
            <span class="tree-title" style="color: var(--accent-amber); font-style: italic;">[Unattributed Used Space]</span>
            <span class="tree-badge">24.0 GB</span>
          </div>
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
        </nav>

        <!-- Search and Quick Filter Bar -->
        <div style="padding: 6px 12px; background: var(--bg-surface); border-bottom: 1px solid var(--border-color); display: flex; gap: 8px; align-items: center; flex-wrap: wrap;">
          <input type="search" id="input-explorer-search" class="text-input" placeholder="Search path or name..." value="${state.filterSearch}" style="flex: 1; min-width: 200px; max-width: 320px;" aria-label="Filter Explorer table by name">
          <select id="select-explorer-size" class="select-input" aria-label="Filter by minimum size">
            <option value="0" ${state.filterSizeMin === 0 ? 'selected' : ''}>All Sizes</option>
            <option value="${1024 * 1024 * 1024}" ${state.filterSizeMin === 1024*1024*1024 ? 'selected' : ''}>&gt; 1 GB</option>
            <option value="${100 * 1024 * 1024}" ${state.filterSizeMin === 100*1024*1024 ? 'selected' : ''}>&gt; 100 MB</option>
            <option value="${10 * 1024 * 1024}" ${state.filterSizeMin === 10*1024*1024 ? 'selected' : ''}>&gt; 10 MB</option>
          </select>
          <select id="select-explorer-type" class="select-input" aria-label="Filter by file type">
            <option value="all" ${state.filterType === 'all' ? 'selected' : ''}>All File Types</option>
            <option value="archives" ${state.filterType === 'archives' ? 'selected' : ''}>Archives &amp; ISOs</option>
            <option value="apps" ${state.filterType === 'apps' ? 'selected' : ''}>Applications (.exe/.dll)</option>
            <option value="games" ${state.filterType === 'games' ? 'selected' : ''}>Game Data (.pak)</option>
            <option value="system" ${state.filterType === 'system' ? 'selected' : ''}>System Files</option>
          </select>
          ${(state.filterSearch || state.filterSizeMin > 0 || state.filterType !== 'all') ? `
            <button id="btn-clear-filters" class="btn btn-sm">Clear Filters</button>
          ` : ''}
        </div>

        <div class="explorer-center">
          ${renderActiveViewTab(selectedNode)}
        </div>

        <!-- Synchronized Bottom Treemap Preview when folder table is shown -->
        ${state.currentViewTab === 'table' ? `
          <div class="explorer-subpanel">
            <div class="panel-header">
              <span>Synchronized Treemap Preview</span>
              <span style="font-size: 11px; font-weight: normal; color: var(--text-dim);">Click block to select / inspect</span>
            </div>
            <div class="panel-body" style="padding: 0;">
              ${renderTreemapSvg(rootNode, 700, 200)}
            </div>
          </div>
        ` : ''}
      </section>

      <!-- Right: Contextual Inspector -->
      <aside class="explorer-inspector" aria-label="Detail Inspector">
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
  
  return `
    <div class="variant-insights-layout">
      <div class="insights-content">
        <!-- Top Plain-Language Cards Grid answering everyday user questions -->
        <div class="insights-card-grid">
          
          <!-- Card 1: What is taking space? -->
          <div class="insight-card">
            <div class="insight-card-header">
              <span class="insight-question">What is taking up your disk space?</span>
              <span class="insight-metric-highlight">368.0 GB</span>
            </div>
            <p class="insight-explanation">
              Major storage areas on your drive:
            </p>
            <ul style="font-size: 12px; padding-left: 18px; color: var(--text-muted); display: flex; flex-direction: column; gap: 4px;">
              <li><strong>Users (Alex):</strong> 107.9 GB allocated (Downloads, Projects, AppData)</li>
              <li><strong>Games (Starfall):</strong> 68.6 GB allocated in game assets</li>
              <li><strong>Windows &amp; System:</strong> 48.2 GB referenced (42.1 GB unique, shared via hard links)</li>
              <li><strong>Virtual Memory / Hibernation:</strong> 28.8 GB (pagefile + hiberfil)</li>
            </ul>
            <div class="insight-actions">
              <button class="btn btn-sm btn-primary btn-select-node" data-node-id="node_users">Inspect Users</button>
              <button class="btn btn-sm btn-select-node" data-node-id="node_games">Inspect Games</button>
            </div>
          </div>

          <!-- Card 2: What changed? (Historical comparison) -->
          <div class="insight-card" style="border-left: 4px solid var(--accent-purple);">
            <div class="insight-card-header">
              <span class="insight-question">What changed since last snapshot?</span>
              <span class="insight-metric-highlight" style="color: var(--accent-purple);">+14.4 GB Net</span>
            </div>
            <p class="insight-explanation">
              ${isHistorical ? 'Historical observation comparison vs current profile baseline:' : 'Differences observed since March 27 snapshot:'}
            </p>
            <ul style="font-size: 12px; padding-left: 18px; color: var(--text-muted); display: flex; flex-direction: column; gap: 4px;">
              <li><strong>+6.2 GB:</strong> New Windows11_Setup_23H2.iso in Downloads</li>
              <li><strong>+8.2 GB:</strong> Temporary build cache growth in AppData\Local\Temp</li>
              <li><strong>0 B:</strong> OneDrive cloud archive (placeholder only)</li>
            </ul>
            <div class="insight-actions">
              <button class="btn btn-sm btn-select-node" data-node-id="node_alex_win11_iso">Inspect New ISO</button>
              <button class="btn btn-sm btn-select-node" data-node-id="node_alex_temp">Inspect Temp Cache</button>
            </div>
          </div>

          <!-- Card 3: What can I safely review? -->
          <div class="insight-card" style="border-left: 4px solid var(--accent-teal);">
            <div class="insight-card-header">
              <span class="insight-question">What can I safely review for cleanup?</span>
              <span class="insight-metric-highlight" style="color: var(--accent-teal);">~37.1 GB</span>
            </div>
            <p class="insight-explanation">
              Items safe for user review with exact reclaimable calculations:
            </p>
            <ul style="font-size: 12px; padding-left: 18px; color: var(--text-muted); display: flex; flex-direction: column; gap: 4px;">
              <li><strong>Downloads Folder:</strong> 28.9 GB (Old ISOs, install packages)</li>
              <li><strong>Temporary Cache Files:</strong> 8.2 GB in AppData\Local\Temp</li>
              <li><strong>Cloud Placeholders:</strong> 4.8 GB OneDrive files (Takes 0 B local disk)</li>
            </ul>
            <div class="insight-actions">
              <button class="btn btn-sm btn-accent btn-open-cleanup" data-node-id="node_alex_downloads">
                Review Downloads Cleanup (28.9 GB)
              </button>
            </div>
          </div>

          <!-- Card 4: Why does disk space not add up? (Unattributed & Inaccessible) -->
          <div class="insight-card" style="border-left: 4px solid var(--accent-amber);">
            <div class="insight-card-header">
              <span class="insight-question">Why is there unattributed or inaccessible space?</span>
              <span class="insight-metric-highlight" style="color: var(--accent-amber);">24.0 GB</span>
            </div>
            <p class="insight-explanation">
              <strong>Unattributed Used Space (24.0 GB):</strong> Volume used space exceeds scanned objects due to NTFS metadata, system restore shadow copies, or restricted system areas.
            </p>
            <p class="insight-explanation" style="margin-top: 4px;">
              <strong>System Volume Information:</strong> Inaccessible under standard user context. <em>More access may reveal additional metadata.</em>
            </p>
            <div class="insight-actions">
              <button class="btn btn-sm btn-warning btn-select-node" data-node-id="${RECONCILIATION_ITEM.id}">
                Inspect Reconciliation Math
              </button>
              <button class="btn btn-sm btn-select-node" data-node-id="node_sysvolinfo">
                View Coverage Gap
              </button>
            </div>
          </div>

        </div>

        <!-- Progressive Detailed Drill-Down Section -->
        <div class="insights-drilldown-section">
          <div style="display: flex; justify-content: space-between; align-items: center; flex-wrap: wrap; gap: 8px;">
            <h2 style="font-size: 14px; font-weight: 700;">Progressive Storage Explorer</h2>
            <div style="display: flex; gap: 6px;">
              <button class="tab-btn ${state.currentViewTab === 'table' ? 'active' : ''}" data-tab="table">Interactive Table</button>
              <button class="tab-btn ${state.currentViewTab === 'treemap' ? 'active' : ''}" data-tab="treemap">Treemap View</button>
              <button class="tab-btn ${state.currentViewTab === 'largest' ? 'active' : ''}" data-tab="largest">Largest Items</button>
            </div>
          </div>

          <div style="min-height: 280px; display: flex; flex-direction: column;">
            ${renderActiveViewTab(selectedNode)}
          </div>
        </div>
      </div>

      <!-- Right: Detail Inspector -->
      <aside class="explorer-inspector" style="width: 340px;">
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
    <div class="variant-workbench-layout">
      <!-- Expert Command & Filter Bar -->
      <div class="workbench-command-bar" role="toolbar" aria-label="Expert Workbench Command Bar">
        <span style="font-weight: 700; font-size: 11px; color: var(--text-dim); text-transform: uppercase; letter-spacing: 0.5px;">Command / Filter:</span>
        <input type="search" id="input-workbench-cmd" class="text-input" placeholder="filter path:C:\\Users size:&gt;100MB type:pak,iso links:&gt;1..." value="${state.filterSearch}" style="min-width: 280px; font-family: var(--font-mono); font-size: 11px;" aria-label="Workbench command search">
        
        <div class="workbench-filter-tokens" role="group" aria-label="Active Filter Tokens">
          <span class="filter-token ${state.filterSizeMin === 1024*1024*1024 ? 'active' : ''}" data-filter-size="${state.filterSizeMin === 1024*1024*1024 ? '0' : '1073741824'}" tabindex="0" role="button">
            size &gt; 1GB
          </span>
          <span class="filter-token ${state.filterType === 'archives' ? 'active' : ''}" data-filter-type="${state.filterType === 'archives' ? 'all' : 'archives'}" tabindex="0" role="button">
            type:archives
          </span>
          <span class="filter-token ${state.filterType === 'apps' ? 'active' : ''}" data-filter-type="${state.filterType === 'apps' ? 'all' : 'apps'}" tabindex="0" role="button">
            type:binaries
          </span>
          <span class="filter-token" id="btn-workbench-preset" tabindex="0" role="button">
            preset:${state.activePresetId}
          </span>
        </div>

        <div style="margin-left: auto; display: flex; gap: 6px;">
          <button id="btn-open-workbench-cleanup" class="btn btn-sm btn-warning" aria-label="Open Guarded Cleanup Plan for selection">
            🛡 Guarded Action Plan
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
              <span style="font-size: 11px;">Active Target: <code>${rootNode.path}</code></span>
              <span style="font-size: 11px; color: var(--text-dim);">Selected: <code>${selectedNode.name || selectedNode.path}</code></span>
            </div>
          </div>

          <div class="data-table-container">
            ${renderWorkbenchExpertTable(rootNode)}
          </div>
        </div>

        <!-- Right Side Panels: Secondary Treemap + Deep Object & Stream Inspector -->
        <aside class="workbench-side-panels">
          <div class="panel-header">
            <span>Visual Spatial Allocation</span>
            <span style="font-size: 10px; color: var(--text-dim);">Proportional Treemap</span>
          </div>
          <div style="height: 180px; background: var(--bg-app); border-bottom: 1px solid var(--border-color);">
            ${renderTreemapSvg(rootNode, 360, 180)}
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
         tabindex="${isSelected ? 0 : -1}">
      <span class="tree-toggle" data-toggle-id="${node.id}">
        ${hasChildren ? (isExpanded ? '▼' : '▶') : '•'}
      </span>
      <span class="tree-icon">${node.kind === 'directory' ? '📁' : (node.kind === 'special' ? '⚙' : '📄')}</span>
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

// Folder Table View (for Explorer / Insights)
function renderFolderTableView(scopeNode) {
  const children = scopeNode.children || [scopeNode];
  const filteredChildren = children.filter(nodeMatchesFilter);
  const sortedChildren = sortNodes(filteredChildren);

  const sortIndicator = (field) => {
    if (state.sortField !== field) return '';
    return state.sortDirection === 'asc' ? ' ▲' : ' ▼';
  };

  return `
    <div class="data-table-container">
      <table class="data-table" role="table" aria-label="Directory Entries Table">
        <thead>
          <tr>
            <th data-sort-field="name">Name${sortIndicator('name')}</th>
            <th data-sort-field="uniqueAllocatedBytes" class="cell-mono">Unique Allocated${sortIndicator('uniqueAllocatedBytes')}</th>
            <th data-sort-field="referencedAllocatedBytes" class="cell-mono">Referenced Allocated${sortIndicator('referencedAllocatedBytes')}</th>
            <th data-sort-field="uniqueLogicalBytes" class="cell-mono">Logical Size${sortIndicator('uniqueLogicalBytes')}</th>
            <th data-sort-field="entryCount" class="cell-mono">Entries${sortIndicator('entryCount')}</th>
            <th data-sort-field="uniqueObjectCount" class="cell-mono">Unique Objects${sortIndicator('uniqueObjectCount')}</th>
            <th>Kind / Category</th>
            <th>Status / Coverage</th>
            <th data-sort-field="modifiedTime">Modified Date${sortIndicator('modifiedTime')}</th>
          </tr>
        </thead>
        <tbody>
          ${sortedChildren.map(child => {
            const isSel = state.selectedNodeId === child.id;
            return `
              <tr class="${isSel ? 'selected' : ''}" data-node-id="${child.id}" role="row" tabindex="0">
                <td class="cell-name">
                  <span>${child.kind === 'directory' ? '📁' : '📄'}</span>
                  <strong>${child.name}</strong>
                </td>
                <td class="cell-mono">${formatBytes(child.uniqueAllocatedBytes)}</td>
                <td class="cell-mono">${formatBytes(child.referencedAllocatedBytes)}</td>
                <td class="cell-mono">${formatBytes(child.uniqueLogicalBytes ?? child.referencedLogicalBytes)}</td>
                <td class="cell-mono">${(child.entryCount || 1).toLocaleString()}</td>
                <td class="cell-mono">${(child.uniqueObjectCount || 1).toLocaleString()}</td>
                <td><span class="fact-tag">${child.category || child.kind}</span></td>
                <td>
                  <span class="status-badge ${child.observationStatus === 'inaccessible' ? 'badge-partial' : 'badge-complete'}">
                    ${child.observationStatus === 'inaccessible' ? 'INACCESSIBLE' : 'OBSERVED'}
                  </span>
                </td>
                <td style="font-size: 11px; color: var(--text-dim);">${formatDate(child.modifiedTime)}</td>
              </tr>
            `;
          }).join('')}

          <!-- Volume Scope Reconciliation Row (When viewing root volume) -->
          ${scopeNode.id === 'node_root' ? `
            <tr class="reconciliation-row ${state.selectedNodeId === RECONCILIATION_ITEM.id ? 'selected' : ''}" 
                data-node-id="${RECONCILIATION_ITEM.id}" role="row" tabindex="0">
              <td class="cell-name">
                <span style="color: var(--accent-amber);">⚖</span>
                <strong style="color: var(--accent-amber);">[Unattributed Used Space]</strong>
              </td>
              <td class="cell-mono" style="color: var(--accent-amber); font-weight: 700;">24.0 GB</td>
              <td class="cell-mono" style="color: var(--accent-amber);">24.0 GB</td>
              <td class="cell-mono">24.0 GB</td>
              <td class="cell-mono">N/A</td>
              <td class="cell-mono">N/A</td>
              <td><span class="fact-tag tag-warning">Reconciliation Difference</span></td>
              <td><span class="status-badge badge-partial">RECONCILED</span></td>
              <td style="font-size: 11px; color: var(--text-dim);">Live Volume Math</td>
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

  return `
    <div class="data-table-container">
      <table class="data-table" role="table" aria-label="Flat Files Table">
        <thead>
          <tr>
            <th data-sort-field="name">File Name &amp; Observed Path</th>
            <th data-sort-field="referencedAllocatedBytes" class="cell-mono">Allocated Size</th>
            <th data-sort-field="referencedLogicalBytes" class="cell-mono">Logical Size</th>
            <th>Characteristics</th>
            <th data-sort-field="modifiedTime">Modified Date</th>
            <th>Action Preview</th>
          </tr>
        </thead>
        <tbody>
          ${sortedFiles.map(file => {
            const isSel = state.selectedNodeId === file.id;
            return `
              <tr class="${isSel ? 'selected' : ''}" data-node-id="${file.id}" role="row" tabindex="0">
                <td class="cell-name">
                  <span>📄</span>
                  <div>
                    <strong>${file.name}</strong>
                    <div style="font-size: 10px; color: var(--text-dim);">${file.path}</div>
                  </div>
                </td>
                <td class="cell-mono" style="font-weight: 600;">${formatBytes(file.referencedAllocatedBytes)}</td>
                <td class="cell-mono">${formatBytes(file.referencedLogicalBytes)}</td>
                <td>
                  <span class="fact-tag ${file.storageCharacteristics?.includes('online-only') ? 'tag-purple' : ''}">
                    ${file.storageCharacteristics?.join(', ') || 'standard'}
                  </span>
                </td>
                <td style="font-size: 11px; color: var(--text-dim);">${formatDate(file.modifiedTime)}</td>
                <td>
                  <button class="btn btn-sm btn-open-cleanup" data-node-id="${file.id}">Guarded Review</button>
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
  allFiles.sort((a, b) => (b.referencedAllocatedBytes || 0) - (a.referencedAllocatedBytes || 0));
  const top10 = allFiles.slice(0, 10);

  return `
    <div class="data-table-container">
      <div style="padding: 8px 12px; background: var(--bg-subtle); border-bottom: 1px solid var(--border-color); font-size: 12px;">
        <strong>Top 10 Largest Storage Consumers</strong> across scanned target.
      </div>
      <table class="data-table" role="table" aria-label="Largest Items List">
        <thead>
          <tr>
            <th>Rank</th>
            <th>Item Name &amp; Path</th>
            <th class="cell-mono">Allocated Physical Size</th>
            <th class="cell-mono">Logical Size</th>
            <th>Safety / Clean Risk</th>
            <th>Action</th>
          </tr>
        </thead>
        <tbody>
          ${top10.map((item, idx) => {
            const isSel = state.selectedNodeId === item.id;
            return `
              <tr class="${isSel ? 'selected' : ''}" data-node-id="${item.id}" role="row" tabindex="0">
                <td style="font-weight: 700; color: var(--text-dim);">#${idx + 1}</td>
                <td class="cell-name">
                  <div>
                    <strong>${item.name}</strong>
                    <div style="font-size: 10px; color: var(--text-dim);">${item.path}</div>
                  </div>
                </td>
                <td class="cell-mono" style="font-weight: 700; color: var(--primary);">${formatBytes(item.referencedAllocatedBytes)}</td>
                <td class="cell-mono">${formatBytes(item.referencedLogicalBytes)}</td>
                <td>
                  <span class="fact-tag ${item.cleanupSafe === 'user_reviewable' ? 'tag-warning' : ''}">
                    ${item.cleanupSafe || 'Review Required'}
                  </span>
                </td>
                <td>
                  <button class="btn btn-sm btn-open-cleanup" data-node-id="${item.id}">Action Plan</button>
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
    'Game & Package Data (.pak)': { ext: ['.pak'], bytes: 0, count: 0, sample: 'Starfall game data' },
    'Disk Images (.iso)': { ext: ['.iso'], bytes: 0, count: 0, sample: 'Windows 11 / Ubuntu ISOs' },
    'Compressed Archives (.zip, .gz)': { ext: ['.zip', '.gz', '.tar'], bytes: 0, count: 0, sample: 'Dataset & project backups' },
    'System Executables & Binaries (.exe, .dll, .sys)': { ext: ['.exe', '.dll', '.sys', '.msi'], bytes: 0, count: 0, sample: 'System32 & App binaries' },
    'Other / Unclassified': { ext: [], bytes: 0, count: 0, sample: 'Documents, configs, caches' }
  };

  for (const f of allFiles) {
    const ext = (f.fileExt || '').toLowerCase();
    let placed = false;
    for (const [groupName, group] of Object.entries(typeMap)) {
      if (group.ext.includes(ext)) {
        group.bytes += (f.referencedAllocatedBytes || 0);
        group.count += 1;
        placed = true;
        break;
      }
    }
    if (!placed) {
      typeMap['Other / Unclassified'].bytes += (f.referencedAllocatedBytes || 0);
      typeMap['Other / Unclassified'].count += 1;
    }
  }

  return `
    <div class="data-table-container">
      <div style="padding: 8px 12px; background: var(--bg-subtle); border-bottom: 1px solid var(--border-color); font-size: 12px;">
        <strong>Observed File Classification Summary</strong> (Aggregated by entry classification rules)
      </div>
      <table class="data-table" role="table" aria-label="File Types Breakdown">
        <thead>
          <tr>
            <th>Classification Category</th>
            <th class="cell-mono">Total Allocation</th>
            <th class="cell-mono">File Count</th>
            <th>Primary Examples</th>
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

// Age Distribution View
function renderAgeDistributionView() {
  return `
    <div class="data-table-container">
      <div style="padding: 8px 12px; background: var(--bg-subtle); border-bottom: 1px solid var(--border-color); font-size: 12px;">
        <strong>Storage Age Distribution</strong> (Based on recorded modified timestamp observations)
      </div>
      <table class="data-table" role="table" aria-label="Age Distribution">
        <thead>
          <tr>
            <th>Time Interval</th>
            <th class="cell-mono">Allocated Physical Size</th>
            <th>Typical Contents</th>
            <th>Candidate Review Strategy</th>
          </tr>
        </thead>
        <tbody>
          <tr>
            <td><strong>Recent (&lt; 7 days)</strong></td>
            <td class="cell-mono" style="font-weight: 700;">14.2 GB</td>
            <td>Active developer builds, current temp caches, newly downloaded installers</td>
            <td>Generally keep; safe temp cache cleanup</td>
          </tr>
          <tr>
            <td><strong>Past 30 Days (7 – 30 days)</strong></td>
            <td class="cell-mono" style="font-weight: 700;">45.8 GB</td>
            <td>Starfall game updates, recent project revisions</td>
            <td>Review completed projects</td>
          </tr>
          <tr>
            <td><strong>Past 1 Year (1 – 12 months)</strong></td>
            <td class="cell-mono" style="font-weight: 700;">165.2 GB</td>
            <td>OS installations, Adobe creative tools, Docker images</td>
            <td>Stable operational software</td>
          </tr>
          <tr>
            <td><strong>Older than 1 Year (&gt; 365 days)</strong></td>
            <td class="cell-mono" style="font-weight: 700;">142.8 GB</td>
            <td>Historical datasets, old Windows 11 setup ISOs, unused tar.gz archives</td>
            <td><span class="fact-tag tag-warning">High Priority Cleanup Review</span></td>
          </tr>
        </tbody>
      </table>
    </div>
  `;
}

// Workbench Expert Table (Dense Grid with all metadata)
function renderWorkbenchExpertTable(rootNode) {
  const allNodes = getAllFlatNodes(rootNode).filter(nodeMatchesFilter);
  const sortedNodes = sortNodes(allNodes);

  return `
    <table class="data-table" role="table" aria-label="Dense Expert Table">
      <thead>
        <tr>
          <th data-sort-field="name">Entry Name &amp; Observed Path</th>
          <th data-sort-field="uniqueAllocatedBytes" class="cell-mono">Unique Alloc (B)</th>
          <th data-sort-field="referencedAllocatedBytes" class="cell-mono">Ref Alloc (B)</th>
          <th data-sort-field="uniqueLogicalBytes" class="cell-mono">Logical (B)</th>
          <th class="cell-mono">Links</th>
          <th>Object ID</th>
          <th>Storage Characteristics</th>
          <th>Owner / Access</th>
          <th>Coverage Status</th>
        </tr>
      </thead>
      <tbody>
        ${sortedNodes.map(node => {
          const isSel = state.selectedNodeId === node.id;
          const obj = node.objectId ? MOCK_OBJECTS[node.objectId] : null;
          return `
            <tr class="${isSel ? 'selected' : ''}" data-node-id="${node.id}" role="row" tabindex="0">
              <td class="cell-name">
                <span>${node.kind === 'directory' ? '📁' : '📄'}</span>
                <div>
                  <strong>${node.name}</strong>
                  <div style="font-size: 10px; color: var(--text-dim); font-family: var(--font-mono);">${node.path}</div>
                </div>
              </td>
              <td class="cell-mono" style="font-weight: 600;">${formatBytes(node.uniqueAllocatedBytes)}</td>
              <td class="cell-mono">${formatBytes(node.referencedAllocatedBytes)}</td>
              <td class="cell-mono">${formatBytes(node.uniqueLogicalBytes ?? node.referencedLogicalBytes)}</td>
              <td class="cell-mono">${obj?.linksCount ?? (node.kind === 'directory' ? '-' : 1)}</td>
              <td style="font-family: var(--font-mono); font-size: 11px;">${node.objectId || '-'}</td>
              <td>
                <span class="fact-tag ${obj?.storageCharacteristics?.includes('online-only') ? 'tag-purple' : ''}">
                  ${obj?.storageCharacteristics?.join(', ') || node.category || 'standard'}
                </span>
              </td>
              <td style="font-size: 11px; color: var(--text-dim);">${obj?.owner || 'Observed Principal'}</td>
              <td>
                <span class="status-badge ${node.observationStatus === 'inaccessible' ? 'badge-partial' : 'badge-complete'}">
                  ${node.observationStatus || 'observed'}
                </span>
              </td>
            </tr>
          `;
        }).join('')}

        <!-- Reconciliation Row -->
        <tr class="reconciliation-row ${state.selectedNodeId === RECONCILIATION_ITEM.id ? 'selected' : ''}" 
            data-node-id="${RECONCILIATION_ITEM.id}" role="row" tabindex="0">
          <td class="cell-name">
            <span style="color: var(--accent-amber);">⚖</span>
            <strong style="color: var(--accent-amber);">[Unattributed Used Space]</strong>
          </td>
          <td class="cell-mono" style="color: var(--accent-amber); font-weight: 700;">24.0 GB</td>
          <td class="cell-mono" style="color: var(--accent-amber);">24.0 GB</td>
          <td class="cell-mono">24.0 GB</td>
          <td class="cell-mono">-</td>
          <td style="font-family: var(--font-mono); font-size: 11px;">reconciliation_diff</td>
          <td><span class="fact-tag tag-warning">Volume Reconciliation Diff</span></td>
          <td style="font-size: 11px;">Volume Accounting Boundary</td>
          <td><span class="status-badge badge-partial">RECONCILED</span></td>
        </tr>
      </tbody>
    </table>
  `;
}

function renderActiveViewTab(selectedNode) {
  if (state.currentViewTab === 'table') return renderFolderTableView(selectedNode.kind === 'directory' ? selectedNode : getActiveTargetRoot());
  if (state.currentViewTab === 'flat') return renderFlatFilesTableView();
  if (state.currentViewTab === 'treemap') {
    return `
      <div style="flex: 1; display: flex; flex-direction: column; height: 100%;">
        <div style="padding: 6px 12px; background: var(--bg-subtle); display: flex; justify-content: space-between; align-items: center; border-bottom: 1px solid var(--border-color);">
          <span style="font-size: 12px; font-weight: 600;">Interactive Proportional Treemap</span>
          <button id="btn-toggle-treemap-text" class="btn btn-sm">
            ${state.treemapShowTextEquivalent ? 'Show Treemap Visual' : 'Show Accessible Text/Table Equivalent'}
          </button>
        </div>
        <div style="flex: 1; overflow: hidden;">
          ${state.treemapShowTextEquivalent ? renderFolderTableView(getActiveTargetRoot()) : renderTreemapSvg(getActiveTargetRoot(), 800, 450)}
        </div>
      </div>
    `;
  }
  if (state.currentViewTab === 'types') return renderFileTypesView();
  if (state.currentViewTab === 'age') return renderAgeDistributionView();
  if (state.currentViewTab === 'largest') return renderLargestItemsView();
  return renderFolderTableView(getActiveTargetRoot());
}

// -------------------------------------------------------------
// Interactive SVG Treemap Generator (Squarified Proportional Layout)
// -------------------------------------------------------------
function renderTreemapSvg(rootNode, width = 700, height = 300) {
  const children = (rootNode.children || []).filter(c => c.observationStatus !== 'inaccessible');
  if (children.length === 0) {
    return `<div style="padding: 20px; text-align: center; color: var(--text-dim);">No sub-items to render in treemap.</div>`;
  }

  // Include reconciliation item in volume treemap to make unattributed space visible!
  const items = [...children];
  if (rootNode.id === 'node_root') {
    items.push({
      id: RECONCILIATION_ITEM.id,
      name: '[Unattributed Used Space]',
      uniqueAllocatedBytes: RECONCILIATION_ITEM.allocatedBytes,
      referencedAllocatedBytes: RECONCILIATION_ITEM.allocatedBytes,
      isReconciliation: true,
      category: 'Reconciliation'
    });
  }

  const totalValue = items.reduce((acc, cur) => acc + (cur.uniqueAllocatedBytes || cur.referencedAllocatedBytes || 1), 0);
  
  // Color palette for treemap blocks
  const colors = ['#2563eb', '#0d9488', '#d97706', '#7c3aed', '#dc2626', '#059669', '#4f46e5', '#ea580c'];

  // Slice-and-dice layout calculation
  let curX = 0;
  let curY = 0;
  let remWidth = width;
  let remHeight = height;

  const rects = [];
  
  items.forEach((item, idx) => {
    const val = item.uniqueAllocatedBytes || item.referencedAllocatedBytes || 1;
    const ratio = val / totalValue;
    const isHorizontal = remWidth >= remHeight;
    
    let w, h, x, y;
    if (isHorizontal) {
      w = Math.max(20, Math.round(remWidth * ratio));
      h = remHeight;
      x = curX;
      y = curY;
      curX += w;
      remWidth -= w;
    } else {
      w = remWidth;
      h = Math.max(20, Math.round(remHeight * ratio));
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
      color: item.isReconciliation ? '#b45309' : colors[idx % colors.length]
    });
  });

  return `
    <div class="treemap-container" style="height: ${height}px;" role="region" aria-label="Disk Allocation Treemap">
      <svg class="treemap-svg" viewBox="0 0 ${width} ${height}" preserveAspectRatio="none">
        ${rects.map(r => {
          const isSel = state.selectedNodeId === r.item.id;
          return `
            <g class="treemap-cell-group" data-node-id="${r.item.id}" tabindex="0" role="button" aria-label="${r.item.name}: ${formatBytes(r.item.uniqueAllocatedBytes || r.item.referencedAllocatedBytes)}">
              <rect class="treemap-cell ${isSel ? 'selected' : ''}" 
                    x="${r.x}" y="${r.y}" width="${r.w}" height="${r.h}" 
                    fill="${r.color}" fill-opacity="0.85" rx="3" />
              ${r.w > 45 && r.h > 24 ? `
                <text class="treemap-label" x="${r.x + 6}" y="${r.y + 16}">
                  ${escapeHtml(truncate(r.item.name, Math.floor(r.w / 8)))}
                </text>
                <text class="treemap-sublabel" x="${r.x + 6}" y="${r.y + 30}">
                  ${formatBytes(r.item.uniqueAllocatedBytes || r.item.referencedAllocatedBytes)}
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
            <span class="fact-value" style="color: var(--accent-amber); font-size: 14px;">24.0 GB</span>
          </div>
          <div class="fact-row">
            <span class="fact-label">Volume Used Space:</span>
            <span class="fact-value">392.0 GB</span>
          </div>
          <div class="fact-row">
            <span class="fact-label">Accounted Unique Allocation:</span>
            <span class="fact-value">368.0 GB</span>
          </div>
        </div>

        <div class="inspector-section">
          <span class="inspector-section-title">Explanation</span>
          <p style="font-size: 12px; color: var(--text-muted); line-height: 1.4;">
            This 24.0 GB discrepancy represents Used Space reported by the filesystem that PigTree cannot defensibly attribute to individual observed filesystem objects.
          </p>
          <p style="font-size: 12px; color: var(--text-muted); line-height: 1.4; margin-top: 6px;">
            Causes include:
          </p>
          <ul style="font-size: 11px; padding-left: 16px; color: var(--text-dim); margin-top: 4px;">
            <li>NTFS Master File Table ($MFT) and filesystem metadata</li>
            <li>Inaccessible System Volume Information (shadow copies/restore points)</li>
            <li>Live filesystem allocations that shifted during traversal</li>
          </ul>
        </div>
      </div>
    `;
  }

  const obj = node.objectId ? MOCK_OBJECTS[node.objectId] : null;
  const aliases = node.objectId ? HARDLINK_ALIASES[node.objectId] : null;

  return `
    <div class="panel-header">
      <span>Detail Inspector</span>
      <button class="btn btn-sm btn-warning btn-open-cleanup" data-node-id="${node.id}" aria-label="Open Guarded Cleanup Action Plan">
        🛡 Cleanup Plan
      </button>
    </div>

    <div class="inspector-card">
      <!-- Section 1: Directory Entry Facts -->
      <div class="inspector-section">
        <span class="inspector-section-title">1. Directory Entry Facts</span>
        <div class="fact-row">
          <span class="fact-label">Entry Name:</span>
          <span class="fact-value">${node.name}</span>
        </div>
        <div class="fact-row">
          <span class="fact-label">Observed Path:</span>
          <span class="fact-value" style="font-size: 11px;">${node.path}</span>
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
          <span class="fact-value">${node.objectId || 'Implicit / Directory Node'}</span>
        </div>
        <div class="fact-row">
          <span class="fact-label">Physical Allocated:</span>
          <span class="fact-value" style="color: var(--primary); font-size: 13px;">
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
          <span class="fact-value" style="font-size: 11px;">${obj?.owner || 'Observed Principal'}</span>
        </div>
        <div class="fact-row">
          <span class="fact-label">Access Rules:</span>
          <span class="fact-value" style="font-size: 10px;">${obj?.accessRules || 'Standard Inherited'}</span>
        </div>
      </div>

      <!-- Section 3: Hardlink Aliases (When Links Count > 1) -->
      ${aliases && aliases.length > 1 ? `
        <div class="inspector-section" style="background-color: var(--accent-purple-bg); padding: 8px; border-radius: var(--radius-sm);">
          <span class="inspector-section-title" style="color: var(--accent-purple);">Hard Link Aliases (${aliases.length} paths share 1 object)</span>
          <p style="font-size: 11px; color: var(--text-muted); margin-bottom: 4px;">
            These paths share the exact same underlying physical allocation on disk:
          </p>
          <ul style="font-size: 10px; font-family: var(--font-mono); padding-left: 14px; display: flex; flex-direction: column; gap: 4px;">
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
          <p style="font-size: 11px; color: var(--accent-purple); margin-top: 4px;">
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
          <span class="fact-value">Known (Provenance: Live NTFS Scan)</span>
        </div>
        <div class="fact-row">
          <span class="fact-label">Observation Interval:</span>
          <span class="fact-value">2025-04-10 (42s duration)</span>
        </div>
      </div>

      <!-- Guarded Cleanup CTA -->
      <div style="margin-top: 8px;">
        <button class="btn btn-primary btn-open-cleanup" data-node-id="${node.id}" style="width: 100%;">
          🛡 Review Guarded Cleanup Action Plan
        </button>
      </div>
    </div>
  `;
}

// -------------------------------------------------------------
// Coverage Gap Explanation Modal
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
          <button id="btn-close-gap-modal" class="btn btn-sm" aria-label="Close dialog">✕</button>
        </div>
        <div class="modal-body">
          <div style="background: var(--accent-amber-bg); border: 1px solid var(--accent-amber); padding: 12px; border-radius: var(--radius-sm);">
            <strong>Inaccessible Scope:</strong> <code>${gap.path}</code><br>
            <strong>Observation Status:</strong> <span class="status-badge badge-partial">INACCESSIBLE</span><br>
            <div style="margin-top: 6px; font-size: 12px;"><strong>Reason:</strong> ${gap.reason}</div>
          </div>

          <div style="font-size: 12px; color: var(--text-muted); line-height: 1.4;">
            <strong>Accounting Impact &amp; Known Subtotals:</strong><br>
            Because this scope was inaccessible, its allocated and logical sizes are recorded as <em>Unavailable</em> rather than zero. 
            The volume's Accounted Unique Allocation (368 GB) is a <em>Known Subtotal</em> that omits these unobserved entries.
          </div>

          <div style="font-size: 12px; color: var(--text-muted); line-height: 1.4;">
            <strong>Defensible Bounds:</strong><br>
            ${gap.defensibleBound}
          </div>

          <div style="background: var(--bg-subtle); padding: 10px; border-radius: var(--radius-sm); border: 1px solid var(--border-color); font-size: 12px;">
            💡 <em>${gap.noncommittalPrompt}</em>
          </div>
        </div>
        <div class="modal-footer">
          <button id="btn-dismiss-gap-modal" class="btn btn-primary">Understood</button>
        </div>
      </div>
    </div>
  `;
}

// -------------------------------------------------------------
// Guarded Cleanup Action Plan Modal
// -------------------------------------------------------------
function renderCleanupModal() {
  const node = state.cleanupTargetNode || findNodeById(state.selectedNodeId) || getActiveTargetRoot();
  const obj = node.objectId ? MOCK_OBJECTS[node.objectId] : null;
  const isHardlinked = obj && obj.linksCount > 1;
  const isOnlineOnly = obj && obj.storageCharacteristics?.includes('online-only');
  const isSystemProtected = node.cleanupSafe === 'protected_system' || node.cleanupSafe === 'protected_dism_only';

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
            <div style="font-size: 11px; color: var(--text-dim); text-transform: uppercase; font-weight: 700;">Target Entry:</div>
            <div style="font-size: 13px; font-weight: 700; margin-top: 2px;">${node.name}</div>
            <div style="font-size: 11px; font-family: var(--font-mono); color: var(--text-muted); word-break: break-all;">${node.path}</div>
          </div>

          <!-- Expected Reclaimable Allocation -->
          <div class="fact-row" style="background: var(--primary-subtle); padding: 10px; border-radius: var(--radius-sm);">
            <span style="font-weight: 700; color: var(--primary);">Expected Reclaimable Allocation:</span>
            <span style="font-weight: 800; font-size: 16px; color: var(--primary); font-family: var(--font-mono);">
              ${reclaimableText}
            </span>
          </div>

          <!-- Uncertainty & Accounting Notice -->
          <div style="font-size: 12px; color: var(--text-muted); line-height: 1.4;">
            <strong>Accounting &amp; Uncertainty Analysis:</strong> ${uncertaintyNote}
          </div>

          <!-- Remediation Guidance -->
          ${isSystemProtected ? `
            <div style="background-color: var(--accent-rose-bg); border: 1px solid var(--accent-rose); padding: 10px; border-radius: var(--radius-sm); color: var(--accent-rose); font-size: 12px;">
              <strong>⚠️ Operating System Protected Path:</strong><br>
              Direct deletion of Windows OS binaries or WinSxS component store files will compromise system stability. 
              Use native OS tools like <code>DISM /Online /Cleanup-Image /StartComponentCleanup</code> or Windows Disk Cleanup instead.
            </div>
          ` : (node.cleanupSafe === 'native_uninstall' ? `
            <div style="background-color: var(--accent-amber-bg); border: 1px solid var(--accent-amber); padding: 10px; border-radius: var(--radius-sm); color: var(--accent-amber); font-size: 12px;">
              <strong>💡 Recommended Remediation:</strong><br>
              This is an installed application / game package. Recommended action is using Windows Settings → Installed Apps or the game launcher to perform a clean native uninstallation.
            </div>
          ` : `
            <div style="background-color: #dcfce7; border: 1px solid #15803d; padding: 10px; border-radius: var(--radius-sm); color: #15803d; font-size: 12px;">
              <strong>✓ Safe User File:</strong><br>
              This entry is within your personal user directory. Can be moved to Recycle Bin or deleted permanently after personal review.
            </div>
          `)}

          <!-- Recovery Expectations -->
          <div style="font-size: 12px;">
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
          <button id="btn-cancel-modal" class="btn">Cancel</button>
          ${isSystemProtected ? `
            <button id="btn-remediate-native" class="btn btn-warning">Launch Windows Native Cleanup Tool</button>
          ` : `
            <button id="btn-confirm-action-plan" class="btn btn-danger">Simulate Action Plan Handoff</button>
          `}
        </div>
      </div>
    </div>
  `;
}

// -------------------------------------------------------------
// Event Listeners & Keyboard Navigation
// -------------------------------------------------------------
function attachEventListeners() {
  // Variant switcher clicks
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

  // Target selector
  const targetSelect = document.getElementById('target-select');
  if (targetSelect) {
    targetSelect.onchange = (e) => {
      const val = e.target.value;
      if (val.startsWith('volume:')) {
        state.activeTargetType = 'volume';
        state.activeVolumeId = 'vol_c';
        state.selectedNodeId = 'node_root';
      } else if (val.startsWith('directory:')) {
        state.activeTargetType = 'directory';
        state.selectedNodeId = 'node_alex_downloads';
      } else if (val.startsWith('historical:')) {
        state.activeTargetType = 'historical';
        state.selectedNodeId = 'node_root';
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
      state.scanProgress = 15;
      state.scanEntriesObserved = 7200;
      renderApp();
      recordAction('Started scan traversal');
      
      const interval = setInterval(() => {
        state.scanProgress += 25;
        state.scanEntriesObserved += 10250;
        if (state.scanProgress >= 100) {
          clearInterval(interval);
          state.scanProgress = 100;
          state.scanEntriesObserved = 48210;
          state.scanStatus = 'finished';
          recordAction('Completed scan traversal');
        }
        renderApp();
      }, 150);
    };
  }

  // Load Historical Snapshot button
  const btnOpenHistorical = document.getElementById('btn-open-historical');
  if (btnOpenHistorical) {
    btnOpenHistorical.onclick = () => {
      state.activeTargetType = 'historical';
      state.selectedNodeId = 'node_root';
      recordAction('Loaded historical snapshot (March 27, 2025)');
      renderApp();
    };
  }

  const btnExitHistorical = document.getElementById('btn-exit-historical');
  if (btnExitHistorical) {
    btnExitHistorical.onclick = () => {
      state.activeTargetType = 'volume';
      state.activeVolumeId = 'vol_c';
      recordAction('Exited historical snapshot view to live C:\\');
      renderApp();
    };
  }

  // Table sorting headers
  document.querySelectorAll('th[data-sort-field]').forEach(th => {
    th.onclick = () => {
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
  document.querySelectorAll('.tree-toggle').forEach(el => {
    el.onclick = (e) => {
      e.stopPropagation();
      const id = el.getAttribute('data-toggle-id');
      if (state.expandedNodes.has(id)) {
        state.expandedNodes.delete(id);
      } else {
        state.expandedNodes.add(id);
      }
      recordAction(`Toggled tree node ${id}`);
      renderApp();
    };
  });

  // Node selection in tree, table, cards
  document.querySelectorAll('[data-node-id]').forEach(el => {
    el.onclick = (e) => {
      // Don't trigger if clicked on sub-button
      if (e.target.closest('button') && !e.target.closest('.treemap-cell-group')) return;
      const id = el.getAttribute('data-node-id');
      if (id) {
        state.selectedNodeId = id;
        recordAction(`Selected node ${id}`);
        renderApp();
      }
    };
  });

  // Treemap toggle text equivalent
  const btnToggleTreemapText = document.getElementById('btn-toggle-treemap-text');
  if (btnToggleTreemapText) {
    btnToggleTreemapText.onclick = () => {
      state.treemapShowTextEquivalent = !state.treemapShowTextEquivalent;
      recordAction(`Toggled treemap text equivalent to ${state.treemapShowTextEquivalent}`);
      renderApp();
    };
  }

  // Coverage Gap link
  const gapLink = document.getElementById('view-coverage-gaps');
  if (gapLink) {
    gapLink.onclick = () => {
      state.coverageGapModalOpen = true;
      recordAction('Opened Coverage Gap explanation modal');
      renderApp();
    };
  }
  const btnCloseGap = document.getElementById('btn-close-gap-modal');
  const btnDismissGap = document.getElementById('btn-dismiss-gap-modal');
  if (btnCloseGap) btnCloseGap.onclick = () => { state.coverageGapModalOpen = false; renderApp(); };
  if (btnDismissGap) btnDismissGap.onclick = () => { state.coverageGapModalOpen = false; renderApp(); };

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
  document.querySelectorAll('.filter-token').forEach(token => {
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

  // Guarded Cleanup Modal Triggers
  document.querySelectorAll('.btn-open-cleanup').forEach(btn => {
    btn.onclick = (e) => {
      e.stopPropagation();
      const nodeId = btn.getAttribute('data-node-id') || state.selectedNodeId;
      state.cleanupTargetNode = findNodeById(nodeId);
      state.cleanupModalOpen = true;
      recordAction(`Opened Guarded Cleanup Preview for ${nodeId}`);
      renderApp();
    };
  });

  // Close modal
  const btnCloseModal = document.getElementById('btn-close-modal');
  const btnCancelModal = document.getElementById('btn-cancel-modal');
  if (btnCloseModal) btnCloseModal.onclick = closeModal;
  if (btnCancelModal) btnCancelModal.onclick = closeModal;

  const btnConfirmPlan = document.getElementById('btn-confirm-action-plan');
  if (btnConfirmPlan) {
    btnConfirmPlan.onclick = () => {
      alert(`Action Plan simulated! Guarded cleanup plan generated for ${state.cleanupTargetNode?.name}.`);
      closeModal();
    };
  }

  const btnRemediateNative = document.getElementById('btn-remediate-native');
  if (btnRemediateNative) {
    btnRemediateNative.onclick = () => {
      alert(`Windows native cleanup tool guidance shown: Launch Cleanmgr.exe or DISM component cleanup.`);
      closeModal();
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

function closeModal() {
  state.cleanupModalOpen = false;
  state.cleanupTargetNode = null;
  recordAction('Closed cleanup dialog');
  renderApp();
}

function renderStatePanel() {
  const display = document.getElementById('state-json-display');
  if (display) {
    display.textContent = JSON.stringify(getDebugState(), null, 2);
  }
}

// Global Keyboard Navigation
window.addEventListener('keydown', (e) => {
  // Do not intercept arrow keys if typing in an input
  const activeTag = document.activeElement?.tagName?.toLowerCase();
  if (['input', 'textarea', 'select'].includes(activeTag)) return;

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
  } else if (e.key === '1') {
    updateUrlVariant('explorer');
  } else if (e.key === '2') {
    updateUrlVariant('insights');
  } else if (e.key === '3') {
    updateUrlVariant('workbench');
  } else if (e.key === 'Escape') {
    if (state.cleanupModalOpen) {
      closeModal();
    }
    if (state.coverageGapModalOpen) {
      state.coverageGapModalOpen = false;
      renderApp();
    }
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
