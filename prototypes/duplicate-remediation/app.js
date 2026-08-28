// PigTree Safe Duplicate Review and Remediation Prototype
// Multi-variant interactive UI prototype for evaluating duplicate review workflows.

import { INITIAL_GROUPS, VERIFICATION_STAGES, ACTION_TYPES } from './mock-data.js';

// Format helper functions
export function formatBytes(bytes) {
  if (bytes === 0) return '0 B';
  if (bytes === null || bytes === undefined) return 'Unknown';
  const k = 1000;
  const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  const val = parseFloat((bytes / Math.pow(k, i)).toFixed(1));
  return `${val} ${sizes[i]}`;
}

// Deep clone initial groups to maintain fresh state
function cloneInitialGroups() {
  return JSON.parse(JSON.stringify(INITIAL_GROUPS));
}

// Central application state
export class PrototypeApp {
  constructor() {
    this.groups = cloneInitialGroups();
    this.currentVariant = this.getVariantFromUrl() || 'guided';
    this.activeGroupId = 'group-vacation';
    this.guidedStep = 1; // 1: Candidate, 2: Verify, 3: Keeper, 4: Actions, 5: Plan Review
    this.planFilter = 'all'; // all | needs_verification | verified | attention
    this.activeModal = null;
    this.modalData = null;
    this.modalTriggerElement = null;

    // Group-level user selections:
    // keeperSelections: { [groupId]: objectId }
    // actionSelections: { [groupId]: { [objectId]: actionId } }
    this.keeperSelections = {};
    this.actionSelections = {};

    this.initDefaultSelections();
    this.initRouting();
    this.initKeyboardNavigation();
    this.render();
  }

  initDefaultSelections() {
    for (const group of this.groups) {
      const rec = group.objects.find(o => o.recommendedKeeper) || group.objects[0];
      this.keeperSelections[group.id] = rec ? rec.id : group.objects[0].id;
      this.actionSelections[group.id] = {};

      for (const obj of group.objects) {
        if (obj.id === this.keeperSelections[group.id]) {
          this.actionSelections[group.id][obj.id] = 'retain';
        } else if (group.status === 'verified') {
          // Default verified non-keeper actions
          if (obj.isProtected) {
            this.actionSelections[group.id][obj.id] = 'native_handoff';
          } else if (obj.isCloud) {
            this.actionSelections[group.id][obj.id] = 'provider_handoff';
          } else if (obj.isStale) {
            this.actionSelections[group.id][obj.id] = 'retain';
          } else {
            this.actionSelections[group.id][obj.id] = 'hardlink_recoverable';
          }
        } else {
          this.actionSelections[group.id][obj.id] = 'retain';
        }
      }
    }
  }

  getVariantFromUrl() {
    const params = new URLSearchParams(window.location.search);
    const v = params.get('variant');
    if (v === 'guided' || v === 'matrix' || v === 'plan') {
      return v;
    }
    return 'guided';
  }

  setVariant(variant, pushHistory = true) {
    if (this.currentVariant === variant) return;
    this.currentVariant = variant;
    const url = new URL(window.location.href);
    url.searchParams.set('variant', variant);
    if (pushHistory) {
      window.history.pushState({ variant }, '', url.toString());
    } else {
      window.history.replaceState({ variant }, '', url.toString());
    }
    const variantNames = {
      guided: 'Guided Review (Variant A)',
      matrix: 'Evidence Comparison Matrix (Variant B)',
      plan: 'Plan-Builder Workspace (Variant C)'
    };
    this.announce(`Switched to ${variantNames[variant] || variant}`);
    this.render();
  }

  initRouting() {
    window.addEventListener('popstate', () => {
      const v = this.getVariantFromUrl();
      this.setVariant(v, false);
    });

    // Reset button
    const btnReset = document.getElementById('btn-reset-state');
    if (btnReset) {
      btnReset.addEventListener('click', () => {
        this.resetState();
      });
    }

    // Switcher buttons in footer
    const switcherBtns = document.querySelectorAll('.switcher-btn');
    switcherBtns.forEach(btn => {
      btn.addEventListener('click', () => {
        const variant = btn.getAttribute('data-variant');
        if (variant) this.setVariant(variant);
      });
    });
  }

  initKeyboardNavigation() {
    window.addEventListener('keydown', (e) => {
      if (e.key === 'Escape' && this.activeModal) {
        this.closeModal();
        return;
      }

      if (e.key !== 'ArrowLeft' && e.key !== 'ArrowRight') return;

      // Do not intercept if modifiers are pressed
      if (e.altKey || e.ctrlKey || e.metaKey || e.shiftKey) return;

      const active = document.activeElement;
      if (!active) return;

      const tag = active.tagName.toLowerCase();
      const isInteractive = (
        tag === 'input' ||
        tag === 'select' ||
        tag === 'textarea' ||
        tag === 'button' ||
        tag === 'a' ||
        tag === 'summary' ||
        tag === 'details' ||
        active.isContentEditable ||
        active.getAttribute('contenteditable') === 'true'
      );

      const isInsideTable = Boolean(active.closest('table, [role="table"], [role="grid"], th, td, tr, thead, tbody, tfoot'));
      const isInsideDialog = Boolean(active.closest('dialog, .modal-dialog, .modal-overlay, [role="dialog"], [role="alertdialog"]')) || this.activeModal !== null;
      const isInsideInteractive = Boolean(active.closest('button, a, select, input, textarea, summary, details, [role="button"], [role="link"], [role="option"], [role="listbox"]'));

      if (isInteractive || isInsideTable || isInsideDialog || isInsideInteractive) {
        return;
      }

      const isSafeRegion = (
        active === document.body ||
        active.id === 'main-content' ||
        active.id === 'app-root' ||
        active.classList.contains('floating-switcher-bar') ||
        active.classList.contains('switcher-hint') ||
        active.classList.contains('app-chrome')
      );

      if (!isSafeRegion) return;

      e.preventDefault();
      const variants = ['guided', 'matrix', 'plan'];
      const idx = variants.indexOf(this.currentVariant);
      const nextVariant = e.key === 'ArrowLeft'
        ? variants[(idx - 1 + variants.length) % variants.length]
        : variants[(idx + 1) % variants.length];

      this.setVariant(nextVariant);
    });
  }

  resetState() {
    this.groups = cloneInitialGroups();
    this.guidedStep = 1;
    this.planFilter = 'all';
    this.initDefaultSelections();
    this.announce('Prototype state reset to defaults.');
    this.render();
  }

  announce(message) {
    const announcer = document.getElementById('aria-live-announcer');
    if (announcer) {
      announcer.textContent = message;
    }
  }

  getActiveGroup() {
    return this.groups.find(g => g.id === this.activeGroupId) || this.groups[0];
  }

  setActiveGroup(groupId) {
    this.activeGroupId = groupId;
    this.guidedStep = 1;
    const group = this.groups.find(g => g.id === groupId);
    if (group) {
      this.announce(`Selected group: ${group.name}`);
    }
    this.render();
  }

  setKeeper(groupId, objectId) {
    const group = this.groups.find(g => g.id === groupId);
    if (!group) return;
    const targetObj = group.objects.find(o => o.id === objectId);
    if (!targetObj || targetObj.excluded) return;

    this.keeperSelections[groupId] = objectId;
    if (!this.actionSelections[groupId]) this.actionSelections[groupId] = {};
    this.actionSelections[groupId][objectId] = 'retain';

    // Auto-assign non-keepers to recoverable hard link if verified and eligible, else retain
    if (group.status === 'verified') {
      for (const obj of group.objects) {
        if (obj.id !== objectId && !obj.excluded) {
          if (obj.isProtected) {
            this.actionSelections[groupId][obj.id] = 'native_handoff';
          } else if (obj.isCloud) {
            this.actionSelections[groupId][obj.id] = 'provider_handoff';
          } else if (!this.actionSelections[groupId][obj.id] || this.actionSelections[groupId][obj.id] === 'retain') {
            this.actionSelections[groupId][obj.id] = 'hardlink_recoverable';
          }
        }
      }
    }
    const keeperName = targetObj.directoryEntries[0]?.name || objectId;
    this.announce(`Selected ${keeperName} as keeper for ${group.name}`);
    this.render();
  }

  setAction(groupId, objectId, actionId) {
    const group = this.groups.find(g => g.id === groupId);
    const obj = group?.objects.find(o => o.id === objectId);
    const objName = obj?.directoryEntries[0]?.name || objectId;

    if (!this.actionSelections[groupId]) this.actionSelections[groupId] = {};
    this.actionSelections[groupId][objectId] = actionId;

    const actDef = ACTION_TYPES[actionId.toUpperCase()] || { label: actionId };
    this.announce(`Action set to ${actDef.label} for ${objName}`);
    this.render();
  }

  excludeObject(groupId, objectId, shouldExclude) {
    const group = this.groups.find(g => g.id === groupId);
    if (!group) return;
    const obj = group.objects.find(o => o.id === objectId);
    if (!obj) return;

    const activeObjects = group.objects.filter(o => !o.excluded);
    if (shouldExclude && activeObjects.length <= 1 && !obj.excluded) {
      this.announce(`Cannot exclude ${obj.directoryEntries[0].name}. At least one active copy must remain in the duplicate group.`);
      return;
    }

    obj.excluded = shouldExclude;

    if (shouldExclude) {
      // Reset that object's action to retain when excluded
      if (!this.actionSelections[groupId]) this.actionSelections[groupId] = {};
      this.actionSelections[groupId][objectId] = 'retain';

      // Never leave keeper pointing to an excluded object
      if (this.keeperSelections[groupId] === objectId) {
        const nextKeeper = group.objects.find(o => !o.excluded);
        if (nextKeeper) {
          this.keeperSelections[groupId] = nextKeeper.id;
          this.actionSelections[groupId][nextKeeper.id] = 'retain';
        }
      }
    }

    const primaryName = obj.directoryEntries[0].name;
    this.announce(`${shouldExclude ? 'Excluded' : 'Re-included'} ${primaryName} in duplicate group.`);
    this.render();
  }

  // Verification Progression Logic
  startVerification(groupId) {
    const group = this.groups.find(g => g.id === groupId);
    if (!group) return;

    if (group.id === 'group-onedrive' && !group.cloudHydrationConsentGiven) {
      this.openModal('hydration', { groupId });
      return;
    }

    group.status = 'verifying';
    group.verificationStepIndex = 0;
    this.announce(`Started content verification for ${group.name}`);
    this.render();
  }

  advanceVerificationStep(groupId) {
    const group = this.groups.find(g => g.id === groupId);
    if (!group) return;

    group.verificationStepIndex = Math.min(group.verificationStepIndex + 1, 4);

    if (group.verificationStepIndex === 4) {
      // Settle verification
      if (group.id === 'group-vacation') {
        const obj3 = group.objects.find(o => o.id === 'obj-vac-3');
        if (obj3 && !obj3.excluded) {
          group.status = 'mismatch';
          this.announce('Verification finished with Content Stream and ACL mismatch on Vacation originals.');
        } else {
          group.status = 'verified';
          group.lastVerified = 'Just now';
          this.announce('Vacation originals successfully verified across remaining copies.');
        }
      } else if (group.id === 'group-stale') {
        group.status = 'stale_error';
        this.announce('Live preflight verification failed: Target changed since snapshot.');
      } else if (group.id === 'group-onedrive') {
        group.status = 'verified';
        group.lastVerified = 'Just now (Hydrated verification complete)';
        this.announce('OneDrive project archive verified across all streams.');
      } else {
        group.status = 'verified';
        group.lastVerified = 'Just now';
        this.announce(`${group.name} verified successfully.`);
      }
    } else {
      this.announce(`Verification stage ${group.verificationStepIndex + 1}: ${VERIFICATION_STAGES[group.verificationStepIndex].name}`);
    }

    this.render();
  }

  cancelVerification(groupId) {
    const group = this.groups.find(g => g.id === groupId);
    if (!group) return;
    group.status = 'candidate';
    group.verificationStepIndex = 0;
    this.announce(`Verification cancelled for ${group.name}. Status retained as candidate.`);
    this.render();
  }

  grantCloudHydrationConsent(groupId) {
    const group = this.groups.find(g => g.id === groupId);
    if (!group) return;
    group.cloudHydrationConsentGiven = true;
    group.status = 'verifying';
    group.verificationStepIndex = 0;
    this.closeModal();
    this.announce('Cloud hydration consent granted. Commencing staged content verification across all streams.');
    this.render();
  }

  // Accounting Engine: Computes Immediate, Conditional, and Retained Reclaim
  calculateGroupAccounting(group) {
    let immediate = 0;
    let conditional = 0;
    let retained = 0;
    let victimCount = 0;

    const keeperId = this.keeperSelections[group.id];
    const actions = this.actionSelections[group.id] || {};

    for (const obj of group.objects) {
      if (obj.id === keeperId || obj.excluded) continue;

      const action = actions[obj.id] || 'retain';
      const alloc = obj.allocatedSize;

      if (action === 'permanent_delete') {
        if (!obj.isCloud && !obj.isProtected && !obj.isStale) {
          immediate += alloc;
          victimCount++;
        }
      } else if (action === 'hardlink_immediate') {
        if (group.status === 'verified' && !obj.isCloud && !obj.isProtected && !obj.isStale) {
          immediate += alloc;
          victimCount++;
        }
      } else if (action === 'recycle') {
        if (!obj.isCloud && !obj.isProtected && !obj.isStale) {
          conditional += alloc;
          victimCount++;
        }
      } else if (action === 'hardlink_recoverable') {
        if (group.status === 'verified' && !obj.isCloud && !obj.isProtected && !obj.isStale) {
          retained += alloc;
          victimCount++;
        }
      }
    }

    return { immediate, conditional, retained, victimCount };
  }

  calculateGlobalAccounting() {
    let immediate = 0;
    let conditional = 0;
    let retained = 0;
    let totalVictims = 0;

    for (const group of this.groups) {
      const g = this.calculateGroupAccounting(group);
      immediate += g.immediate;
      conditional += g.conditional;
      retained += g.retained;
      totalVictims += g.victimCount;
    }

    return { immediate, conditional, retained, totalVictims };
  }

  // Action Eligibility Checks
  getEligibility(group, obj) {
    const isKeeper = this.keeperSelections[group.id] === obj.id;

    if (isKeeper) {
      return {
        canRetain: true,
        canRecycle: false,
        canPermanentDelete: false,
        canHardlinkRecoverable: false,
        canHardlinkImmediate: false,
        canNativeHandoff: false,
        canProviderHandoff: false,
        disabledReason: "Chosen keeper copy must be retained."
      };
    }

    if (obj.isStale) {
      return {
        canRetain: true,
        canRecycle: false,
        canPermanentDelete: false,
        canHardlinkRecoverable: false,
        canHardlinkImmediate: false,
        canNativeHandoff: false,
        canProviderHandoff: false,
        disabledReason: "Target changed since snapshot (File ID / size / mtime divergence). Action Plan execution prohibited."
      };
    }

    if (obj.isProtected) {
      return {
        canRetain: true,
        canRecycle: false,
        canPermanentDelete: false,
        canHardlinkRecoverable: false,
        canHardlinkImmediate: false,
        canNativeHandoff: true,
        canProviderHandoff: false,
        disabledReason: "Protected Windows system resource. Direct mutation blocked; use Native System Handoff."
      };
    }

    if (obj.isCloud) {
      return {
        canRetain: true,
        canRecycle: false,
        canPermanentDelete: false,
        canHardlinkRecoverable: false,
        canHardlinkImmediate: false,
        canNativeHandoff: false,
        canProviderHandoff: true,
        disabledReason: "Cloud-managed storage provider (OneDrive). Direct PigTree mutation is protected; use Provider Handoff."
      };
    }

    if (group.status !== 'verified') {
      return {
        canRetain: true,
        canRecycle: false,
        canPermanentDelete: false,
        canHardlinkRecoverable: false,
        canHardlinkImmediate: false,
        canNativeHandoff: false,
        canProviderHandoff: false,
        disabledReason: group.status === 'mismatch'
          ? "Stream / ACL mismatch detected. Actions locked until excluded or re-verified."
          : "Full content verification across all streams is required before actions unlock."
      };
    }

    return {
      canRetain: true,
      canRecycle: true,
      canPermanentDelete: true,
      canHardlinkRecoverable: true,
      canHardlinkImmediate: true,
      canNativeHandoff: false,
      canProviderHandoff: false,
      disabledReason: null
    };
  }

  getFilterCounts() {
    const all = this.groups.length;
    const needs_verification = this.groups.filter(g => g.status === 'candidate' || g.status === 'verifying').length;
    const verified = this.groups.filter(g => g.status === 'verified').length;
    const attention = this.groups.filter(g => g.status === 'mismatch' || g.status === 'stale_error' || g.objects.some(o => o.isProtected || o.isStale || (o.mismatchDetails && !o.excluded))).length;
    return { all, needs_verification, verified, attention };
  }

  getFilteredGroups() {
    if (this.planFilter === 'needs_verification') {
      return this.groups.filter(g => g.status === 'candidate' || g.status === 'verifying');
    } else if (this.planFilter === 'verified') {
      return this.groups.filter(g => g.status === 'verified');
    } else if (this.planFilter === 'attention') {
      return this.groups.filter(g => g.status === 'mismatch' || g.status === 'stale_error' || g.objects.some(o => o.isProtected || o.isStale || (o.mismatchDetails && !o.excluded)));
    }
    return this.groups;
  }

  setPlanFilter(filter) {
    this.planFilter = filter;
    const filtered = this.getFilteredGroups();
    const filterLabels = {
      all: 'All Groups',
      needs_verification: 'Unverified Groups',
      verified: 'Verified Groups',
      attention: 'Needs Attention'
    };
    if (filtered.length > 0 && !filtered.some(g => g.id === this.activeGroupId)) {
      this.activeGroupId = filtered[0].id;
    }
    this.announce(`Filtered queue by ${filterLabels[filter] || filter}: ${filtered.length} groups available.`);
    this.render();
  }

  // Modals management
  openModal(type, data = null, triggerEl = null) {
    this.activeModal = type;
    this.modalData = data;
    this.modalTriggerElement = triggerEl || document.activeElement;
    this.renderModal();
  }

  closeModal(returnToPreviousModal = false) {
    const prevTrigger = this.modalTriggerElement;
    this.activeModal = null;
    this.modalData = null;
    const container = document.getElementById('modal-container');
    if (container) {
      container.innerHTML = '';
      container.removeAttribute('role');
      container.removeAttribute('aria-label');
    }
    if (prevTrigger && typeof prevTrigger.focus === 'function') {
      prevTrigger.focus();
    }
  }

  // Focus capture and restoration
  captureFocusState() {
    const active = document.activeElement;
    if (!active || active === document.body) return null;

    return {
      id: active.id || null,
      tagName: active.tagName.toLowerCase(),
      groupId: active.getAttribute('data-group-id') || active.closest('[data-group-id]')?.getAttribute('data-group-id') || null,
      objId: active.getAttribute('data-obj-id') || active.closest('[data-obj-id]')?.getAttribute('data-obj-id') || null,
      step: active.getAttribute('data-step') || null,
      filter: active.getAttribute('data-filter') || null,
      variant: active.getAttribute('data-variant') || null,
      className: active.className || null,
      role: active.getAttribute('role') || null,
      selectionStart: typeof active.selectionStart === 'number' ? active.selectionStart : null,
      selectionEnd: typeof active.selectionEnd === 'number' ? active.selectionEnd : null
    };
  }

  restoreFocusState(focusState) {
    if (!focusState) return;

    let target = null;

    if (focusState.id) {
      target = document.getElementById(focusState.id);
    }

    if (!target && focusState.groupId && focusState.objId) {
      if (focusState.tagName === 'select') {
        target = document.querySelector(`select[data-group-id="${focusState.groupId}"][data-obj-id="${focusState.objId}"]`);
      } else if (focusState.tagName === 'input') {
        target = document.querySelector(`input[data-group-id="${focusState.groupId}"][data-obj-id="${focusState.objId}"]`);
      } else if (focusState.className && focusState.className.includes('btn-set-keeper')) {
        target = document.querySelector(`.btn-set-keeper[data-group-id="${focusState.groupId}"][data-obj-id="${focusState.objId}"]`);
      } else if (focusState.className && focusState.className.includes('btn-exclude')) {
        target = document.querySelector(`.btn-exclude[data-group-id="${focusState.groupId}"][data-obj-id="${focusState.objId}"]`);
      } else if (focusState.className && focusState.className.includes('btn-handoff')) {
        target = document.querySelector(`.btn-handoff[data-group-id="${focusState.groupId}"][data-obj-id="${focusState.objId}"]`);
      } else {
        target = document.querySelector(`[data-group-id="${focusState.groupId}"][data-obj-id="${focusState.objId}"]`);
      }
    }

    if (!target && focusState.groupId) {
      if (focusState.className && focusState.className.includes('btn-start-verify')) {
        target = document.querySelector(`.btn-start-verify[data-group-id="${focusState.groupId}"]`);
      } else if (focusState.className && focusState.className.includes('btn-step-verify')) {
        target = document.querySelector(`.btn-step-verify[data-group-id="${focusState.groupId}"]`);
      } else if (focusState.className && focusState.className.includes('btn-cancel-verify')) {
        target = document.querySelector(`.btn-cancel-verify[data-group-id="${focusState.groupId}"]`);
      } else if (focusState.className && focusState.className.includes('queue-item-card')) {
        target = document.querySelector(`.queue-item-card[data-group-id="${focusState.groupId}"]`);
      }
    }

    if (!target && focusState.step) {
      target = document.querySelector(`.rail-step[data-step="${focusState.step}"]`);
    }

    if (!target && focusState.filter) {
      target = document.querySelector(`[data-filter="${focusState.filter}"]`);
    }

    if (!target && focusState.variant) {
      target = document.querySelector(`[data-variant="${focusState.variant}"]`);
    }

    if (target && typeof target.focus === 'function') {
      target.focus();
      if (focusState.selectionStart !== null && typeof target.setSelectionRange === 'function') {
        try { target.setSelectionRange(focusState.selectionStart, focusState.selectionEnd); } catch (_) {}
      }
    }
  }

  // Render entry point
  render() {
    const focusState = this.captureFocusState();

    // Update floating switcher active state
    document.querySelectorAll('.switcher-btn').forEach(btn => {
      const v = btn.getAttribute('data-variant');
      const isActive = v === this.currentVariant;
      btn.classList.toggle('active', isActive);
      btn.setAttribute('aria-pressed', isActive ? 'true' : 'false');
    });

    const root = document.getElementById('app-root');
    if (!root) return;

    if (this.currentVariant === 'guided') {
      root.innerHTML = this.renderGuidedVariant();
    } else if (this.currentVariant === 'matrix') {
      root.innerHTML = this.renderMatrixVariant();
    } else if (this.currentVariant === 'plan') {
      root.innerHTML = this.renderPlanVariant();
    }

    this.attachEventListeners();
    this.updateStateInspector();

    if (!this.activeModal) {
      this.restoreFocusState(focusState);
    }
  }

  // =========================================================================
  // VARIANT A: GUIDED GROUP REVIEW
  // =========================================================================
  renderGuidedVariant() {
    const group = this.getActiveGroup();
    const globalAcct = this.calculateGlobalAccounting();

    return `
      <div class="guided-container">
        <!-- Group Header & Navigation -->
        <section class="group-header-card" aria-label="Group Selection and Overview">
          <div class="group-title-area">
            <h2>
              <span>Group: ${group.name}</span>
              ${this.renderStatusBadge(group.status)}
            </h2>
            <p class="group-story">${group.story}</p>
          </div>
          <div class="group-nav-controls" role="group" aria-label="Duplicate group pagination">
            <label for="guided-group-select" style="font-size: var(--font-size-xs); font-weight: 700; color: var(--text-dim);">Select Scenario:</label>
            <select id="guided-group-select" class="select-input" style="width: auto;">
              ${this.groups.map(g => `
                <option value="${g.id}" ${g.id === group.id ? 'selected' : ''}>${g.name} (${g.formattedSize}) - ${g.status.toUpperCase()}</option>
              `).join('')}
            </select>
          </div>
        </section>

        <!-- Step Rail -->
        <nav class="guided-step-rail" aria-label="Guided Review Steps">
          <button class="rail-step ${this.guidedStep === 1 ? 'active' : ''} ${this.guidedStep > 1 ? 'done' : ''}" data-step="1" aria-current="${this.guidedStep === 1 ? 'step' : 'false'}">
            <span class="step-num">1</span> <span>Candidate Group</span>
          </button>
          <span class="rail-divider" aria-hidden="true">&rarr;</span>
          <button class="rail-step ${this.guidedStep === 2 ? 'active' : ''} ${this.guidedStep > 2 || group.status === 'verified' ? 'done' : ''}" data-step="2" aria-current="${this.guidedStep === 2 ? 'step' : 'false'}">
            <span class="step-num">2</span> <span>Verify Content</span>
          </button>
          <span class="rail-divider" aria-hidden="true">&rarr;</span>
          <button class="rail-step ${this.guidedStep === 3 ? 'active' : ''} ${this.guidedStep > 3 ? 'done' : ''}" data-step="3" aria-current="${this.guidedStep === 3 ? 'step' : 'false'}">
            <span class="step-num">3</span> <span>Choose Keeper</span>
          </button>
          <span class="rail-divider" aria-hidden="true">&rarr;</span>
          <button class="rail-step ${this.guidedStep === 4 ? 'active' : ''} ${this.guidedStep > 4 ? 'done' : ''}" data-step="4" aria-current="${this.guidedStep === 4 ? 'step' : 'false'}">
            <span class="step-num">4</span> <span>Choose Actions</span>
          </button>
          <span class="rail-divider" aria-hidden="true">&rarr;</span>
          <button class="rail-step ${this.guidedStep === 5 ? 'active' : ''}" data-step="5" aria-current="${this.guidedStep === 5 ? 'step' : 'false'}">
            <span class="step-num">5</span> <span>Review Plan</span>
          </button>
        </nav>

        <!-- Verification Console -->
        ${this.renderVerificationBox(group)}

        <!-- Main Step Surface -->
        <section aria-label="Candidate Copies">
          <div class="copies-grid" role="radiogroup" aria-label="Keeper selection for ${group.name}">
            ${group.objects.map(obj => this.renderGuidedCopyCard(group, obj)).join('')}
          </div>
        </section>

        <!-- Sticky Consequence Summary -->
        <aside class="sticky-summary-bar" aria-label="Remediation Consequence Summary">
          <div class="summary-metrics">
            <div class="metric-group">
              <span class="metric-label">Immediate Expected Release</span>
              <span class="metric-value metric-immediate">${formatBytes(globalAcct.immediate)}</span>
            </div>
            <div class="metric-group">
              <span class="metric-label">Conditional Future Release (Recycle Bin)</span>
              <span class="metric-value metric-conditional">${formatBytes(globalAcct.conditional)}</span>
            </div>
            <div class="metric-group">
              <span class="metric-label">Allocation Retained for Recovery</span>
              <span class="metric-value metric-retained">${formatBytes(globalAcct.retained)}</span>
            </div>
          </div>
          <button class="btn btn-primary btn-open-action-plan" id="btn-open-action-plan" style="padding: 0.625rem 1.25rem;">
            <span>Preview Immutable Action Plan</span>
            <span aria-hidden="true">&rarr;</span>
          </button>
        </aside>
      </div>
    `;
  }

  renderGuidedCopyCard(group, obj) {
    const isKeeper = this.keeperSelections[group.id] === obj.id;
    const action = (this.actionSelections[group.id] && this.actionSelections[group.id][obj.id]) || 'retain';
    const elig = this.getEligibility(group, obj);
    const primaryEntry = obj.directoryEntries.find(e => e.isPrimary) || obj.directoryEntries[0];
    const aliases = obj.directoryEntries.filter(e => !e.isPrimary);
    const activeObjectsCount = group.objects.filter(o => !o.excluded).length;
    const canExclude = obj.excluded || activeObjectsCount > 1;

    return `
      <article class="copy-card ${isKeeper ? 'is-keeper' : ''} ${obj.excluded ? 'is-excluded' : ''}" aria-labelledby="card-title-${obj.id}">
        <div class="copy-card-header">
          <div class="copy-path-box">
            <div class="copy-filename" id="card-title-${obj.id}">${primaryEntry.name}</div>
            <div class="copy-parent-path">${primaryEntry.parent}</div>
          </div>
          <div>
            ${isKeeper ? '<span class="badge badge-keeper">Selected Keeper (Retained)</span>' : ''}
            ${obj.excluded ? '<span class="badge badge-mismatch">Excluded</span>' : ''}
            ${obj.isCloud ? '<span class="badge badge-protected">Cloud Placeholder</span>' : ''}
            ${obj.isProtected ? '<span class="badge badge-protected">Protected Resource</span>' : ''}
            ${obj.isStale ? '<span class="badge badge-stale">Stale (Changed)</span>' : ''}
          </div>
        </div>

        ${aliases.length > 0 ? `
          <div class="callout-box callout-info" style="font-size: var(--font-size-xs);">
            <strong>Hard Link Alias Detected:</strong>
            ${aliases.map(a => `<div class="mono-cell" style="margin-top: 2px;">• ${a.path}</div>`).join('')}
            <div style="color: var(--text-dim); margin-top: 4px;">Points to SAME Filesystem Object ID (${obj.fileId}). Reclaim counts object once.</div>
          </div>
        ` : ''}

        <div class="copy-metadata-list">
          <div class="meta-row">
            <span class="meta-label">Filesystem Object ID:</span>
            <span class="meta-val">${obj.fileId}</span>
          </div>
          <div class="meta-row">
            <span class="meta-label">Local Allocated Size:</span>
            <span class="meta-val">${formatBytes(obj.allocatedSize)}</span>
          </div>
          <div class="meta-row">
            <span class="meta-label">Logical Size:</span>
            <span class="meta-val">${formatBytes(obj.logicalSize)}</span>
          </div>
          <div class="meta-row">
            <span class="meta-label">Content Streams (${obj.streamCount}):</span>
            <span class="meta-val">${obj.streams.map(s => s.name.split(' ')[0]).join(', ')}</span>
          </div>
          <div class="meta-row">
            <span class="meta-label">Owner &amp; Security:</span>
            <span class="meta-val" title="${obj.accessRules}">${obj.owner}</span>
          </div>
          <div class="meta-row">
            <span class="meta-label">Storage Characteristic:</span>
            <span class="meta-val">${obj.storageCharacteristic}</span>
          </div>
        </div>

        ${obj.mismatchDetails ? `
          <div class="callout-box callout-danger">
            <strong>Stream &amp; Security Divergence:</strong>
            <div>• Extra stream: ${obj.mismatchDetails.divergentStreams.join(', ')}</div>
            <div>• ACL: ${obj.mismatchDetails.divergentAcl}</div>
            <div style="margin-top: 6px; display: flex; gap: 6px; flex-wrap: wrap;">
              <button class="btn btn-sm btn-danger btn-exclude"
                      data-group-id="${group.id}"
                      data-obj-id="${obj.id}"
                      ${!canExclude ? 'disabled title="Cannot exclude the only remaining active copy in this duplicate group" aria-disabled="true"' : ''}>
                ${obj.excluded ? 'Re-include in group' : 'Exclude from group'}
              </button>
              <button class="btn btn-sm btn-outline btn-view-mismatch" data-group-id="${group.id}" data-obj-id="${obj.id}">
                Mismatch Details
              </button>
            </div>
            ${!canExclude ? '<div style="font-size: var(--font-size-xs); color: var(--accent-amber); margin-top: 4px;">At least one active copy must remain in the duplicate group.</div>' : ''}
          </div>
        ` : ''}

        ${obj.staleReason ? `
          <div class="callout-box callout-danger">
            <strong>Live Preflight Failure:</strong>
            <div>${obj.staleReason}</div>
            <div style="margin-top: 6px;">
              <button class="btn btn-sm btn-outline btn-view-stale" data-group-id="${group.id}" data-obj-id="${obj.id}">
                View Preflight Invalidation Details
              </button>
            </div>
          </div>
        ` : ''}

        <!-- Keeper Choice Radio / Button -->
        <div style="margin-top: 0.25rem;">
          <label style="display: inline-flex; align-items: center; gap: 6px; cursor: ${obj.excluded ? 'not-allowed' : 'pointer'}; font-weight: 700; font-size: var(--font-size-sm);">
            <input type="radio"
                   name="guided-keeper-${group.id}"
                   class="guided-keeper-radio"
                   data-group-id="${group.id}"
                   data-obj-id="${obj.id}"
                   ${isKeeper ? 'checked' : ''}
                   ${obj.excluded ? 'disabled' : ''}>
            <span>${isKeeper ? 'Selected Keeper (Retained)' : 'Choose as Keeper Copy'}</span>
          </label>
        </div>

        <!-- Action Selector -->
        <div class="copy-action-selector">
          <label for="action-select-guided-${group.id}-${obj.id}" class="action-select-label">Cleanup Action for ${primaryEntry.name}:</label>
          ${isKeeper ? `
            <div style="font-size: var(--font-size-sm); font-weight: 600; color: var(--primary);">
              Selected Keeper (Retained)
            </div>
            <p class="action-consequence-note">This file will remain intact on disk without modification.</p>
          ` : elig.canNativeHandoff ? `
            <button class="btn btn-sm btn-handoff" data-group-id="${group.id}" data-obj-id="${obj.id}">
              Open Native System Handoff
            </button>
            <p class="action-consequence-note">${obj.protectionReason}</p>
          ` : elig.canProviderHandoff ? `
            <button class="btn btn-sm btn-handoff" data-group-id="${group.id}" data-obj-id="${obj.id}">
              Open Cloud Provider Handoff
            </button>
            <p class="action-consequence-note">${obj.protectionReason}</p>
          ` : elig.disabledReason ? `
            <div style="font-size: var(--font-size-xs); color: var(--accent-amber); font-weight: 600;">
              ${elig.disabledReason}
            </div>
          ` : `
            <select id="action-select-guided-${group.id}-${obj.id}"
                    aria-label="Cleanup Action for ${primaryEntry.name}"
                    class="select-input select-action"
                    data-group-id="${group.id}"
                    data-obj-id="${obj.id}">
              <option value="retain" ${action === 'retain' ? 'selected' : ''}>Retain copy (No change)</option>
              <option value="hardlink_recoverable" ${action === 'hardlink_recoverable' ? 'selected' : ''}>Hard Link (Recoverable - 0 immediate reclaim)</option>
              <option value="hardlink_immediate" ${action === 'hardlink_immediate' ? 'selected' : ''}>Hard Link (Immediate reclaim - ${formatBytes(obj.allocatedSize)})</option>
              <option value="recycle" ${action === 'recycle' ? 'selected' : ''}>Move to Recycle Bin (Conditional - ${formatBytes(obj.allocatedSize)})</option>
              <option value="permanent_delete" ${action === 'permanent_delete' ? 'selected' : ''}>Permanently delete (Immediate - ${formatBytes(obj.allocatedSize)})</option>
            </select>
            <p class="action-consequence-note">${this.getActionConsequenceText(action, obj)}</p>
          `}
        </div>
      </article>
    `;
  }

  getActionConsequenceText(action, obj) {
    switch (action) {
      case 'retain':
        return 'Leaves this entry and underlying object unchanged.';
      case 'hardlink_recoverable':
        return 'Replaces entry with a Hard Link to keeper. Victim is preserved in PigTree recovery storage (0 immediate release).';
      case 'hardlink_immediate':
        return `Replaces entry with a Hard Link to keeper and immediately purges staging link. Reclaims ${formatBytes(obj.allocatedSize)}. Irreversible.`;
      case 'recycle':
        return `Sends entry to Windows Recycle Bin. Reclaims 0 B immediately; reclaims ${formatBytes(obj.allocatedSize)} when Recycle Bin is emptied.`;
      case 'permanent_delete':
        return `Direct Win32 entry deletion. Reclaims ${formatBytes(obj.allocatedSize)} immediately. Irreversible.`;
      default:
        return '';
    }
  }

  // =========================================================================
  // VARIANT B: EVIDENCE-FIRST COMPARISON MATRIX
  // =========================================================================
  renderMatrixVariant() {
    const group = this.getActiveGroup();
    const keeperId = this.keeperSelections[group.id];
    const actions = this.actionSelections[group.id] || {};
    const globalAcct = this.calculateGlobalAccounting();

    return `
      <div class="matrix-container">
        <!-- Scenario Tabs Bar -->
        <section class="group-header-card" aria-label="Scenario selection">
          <div class="group-title-area">
            <h2>
              <span>Scenario: ${group.name}</span>
              ${this.renderStatusBadge(group.status)}
            </h2>
            <p class="group-story">${group.story}</p>
          </div>
          <div class="group-nav-controls">
            <label for="matrix-group-select" style="font-size: var(--font-size-xs); font-weight: 700; color: var(--text-dim);">Scenario:</label>
            <select id="matrix-group-select" class="select-input" style="width: auto;">
              ${this.groups.map(g => `
                <option value="${g.id}" ${g.id === group.id ? 'selected' : ''}>${g.name} - ${g.status.toUpperCase()}</option>
              `).join('')}
            </select>
          </div>
        </section>

        <!-- Verification Console -->
        ${this.renderVerificationBox(group)}

        <!-- Matrix Table -->
        <section class="matrix-table-wrapper" aria-label="Evidence Comparison Matrix">
          <table class="matrix-table">
            <caption class="sr-only">Evidence Comparison Matrix for Duplicate Group: ${group.name}</caption>
            <thead>
              <tr>
                <th class="category-col" scope="col">Evidence Category</th>
                ${group.objects.map((obj, i) => `
                  <th class="object-header" scope="col">
                    <div style="font-size: var(--font-size-sm); font-weight: 700;">Copy ${i + 1} ${obj.id === keeperId ? '(Keeper)' : ''}</div>
                    <div class="mono-cell" style="font-size: var(--font-size-xs); color: var(--text-dim);">${obj.directoryEntries[0].name}</div>
                  </th>
                `).join('')}
              </tr>
            </thead>
            <tbody>
              <!-- Row 1: Keeper Selection -->
              <tr class="row-keeper-select">
                <th class="category-col" scope="row">Selected Keeper</th>
                ${group.objects.map(obj => `
                  <td>
                    <label style="display: inline-flex; align-items: center; gap: 6px; cursor: ${obj.excluded ? 'not-allowed' : 'pointer'}; font-weight: 700;">
                      <input type="radio"
                             name="matrix-keeper-${group.id}"
                             class="matrix-keeper-radio"
                             data-group-id="${group.id}"
                             data-obj-id="${obj.id}"
                             ${obj.id === keeperId ? 'checked' : ''}
                             ${obj.excluded ? 'disabled' : ''}>
                      <span>${obj.id === keeperId ? 'Selected Keeper (Retained)' : 'Make Keeper'}</span>
                    </label>
                  </td>
                `).join('')}
              </tr>

              <!-- Row 2: Directory Entries & Aliases -->
              <tr>
                <th class="category-col" scope="row">Paths &amp; Hard Link Aliases</th>
                ${group.objects.map(obj => `
                  <td class="mono-cell">
                    ${obj.directoryEntries.map(e => `
                      <div style="margin-bottom: 4px;">
                        ${e.isAlias ? '<span class="badge badge-candidate">Alias</span> ' : ''}${e.path}
                      </div>
                    `).join('')}
                    ${obj.directoryEntries.length > 1 ? `
                      <div style="color: var(--primary); font-size: var(--font-size-xs); font-weight: 600;">
                        (${obj.directoryEntries.length} Directory Entries pointing to 1 Object)
                      </div>
                    ` : ''}
                  </td>
                `).join('')}
              </tr>

              <!-- Row 3: Allocated Size -->
              <tr>
                <th class="category-col" scope="row">Local Allocated Size</th>
                ${group.objects.map(obj => `
                  <td class="mono-cell">
                    <strong>${formatBytes(obj.allocatedSize)}</strong>
                    ${obj.isCloud ? '<div style="color: var(--accent-amber); font-size: var(--font-size-xs);">(0 bytes local allocation)</div>' : ''}
                  </td>
                `).join('')}
              </tr>

              <!-- Row 4: Logical Size -->
              <tr>
                <th class="category-col" scope="row">Logical Size</th>
                ${group.objects.map(obj => `
                  <td class="mono-cell">${formatBytes(obj.logicalSize)}</td>
                `).join('')}
              </tr>

              <!-- Row 5: Content Streams -->
              <tr>
                <th class="category-col" scope="row">Content Streams (${group.verificationScope})</th>
                ${group.objects.map(obj => `
                  <td class="mono-cell">
                    ${obj.streams.map(s => `
                      <div style="margin-bottom: 2px;">
                        • <strong>${s.name}</strong> (${formatBytes(s.logicalSize)})
                      </div>
                    `).join('')}
                    ${obj.streamCount > 1 ? '<span class="badge badge-mismatch">Extra Stream</span>' : ''}
                  </td>
                `).join('')}
              </tr>

              <!-- Row 6: Verification Status -->
              <tr>
                <th class="category-col" scope="row">Verification Status</th>
                ${group.objects.map(obj => `
                  <td>
                    ${group.status === 'verified' ? '<span class="badge badge-verified">Verified (SHA-256 Match)</span>' : ''}
                    ${group.status === 'candidate' ? '<span class="badge badge-candidate">Candidate (Unverified)</span>' : ''}
                    ${group.status === 'mismatch' ? '<span class="badge badge-mismatch">Mismatch Detected</span>' : ''}
                    ${group.status === 'stale_error' ? '<span class="badge badge-stale">Stale (Changed Live)</span>' : ''}
                    ${group.status === 'verifying' ? '<span class="badge badge-candidate">Verifying...</span>' : ''}
                  </td>
                `).join('')}
              </tr>

              <!-- Row 7: Owner & Access Rules -->
              <tr>
                <th class="category-col" scope="row">Owner &amp; Access Rules (ACL)</th>
                ${group.objects.map(obj => `
                  <td class="mono-cell" style="font-size: var(--font-size-xs);">
                    <div><strong>Owner:</strong> ${obj.owner}</div>
                    <div style="color: var(--text-dim); margin-top: 2px;"><strong>ACL:</strong> ${obj.accessRules}</div>
                  </td>
                `).join('')}
              </tr>

              <!-- Row 8: Timestamps -->
              <tr>
                <th class="category-col" scope="row">Timestamp Observations</th>
                ${group.objects.map(obj => `
                  <td class="mono-cell" style="font-size: var(--font-size-xs);">
                    <div><strong>Modified:</strong> ${obj.mtime}</div>
                  </td>
                `).join('')}
              </tr>

              <!-- Row 9: Attributes & Characteristics -->
              <tr>
                <th class="category-col" scope="row">Storage Characteristics</th>
                ${group.objects.map(obj => `
                  <td style="font-size: var(--font-size-xs);">
                    <div>${obj.storageCharacteristic}</div>
                    <div class="mono-cell" style="color: var(--text-dim);">${obj.attributes}</div>
                  </td>
                `).join('')}
              </tr>

              <!-- Row 10: Cloud State -->
              <tr>
                <th class="category-col" scope="row">Cloud &amp; Reparse State</th>
                ${group.objects.map(obj => `
                  <td>
                    ${obj.isCloud ? `
                      <span class="badge badge-protected">OneDrive Online-Only</span>
                      <div style="font-size: var(--font-size-xs); margin-top: 4px; color: var(--text-dim);">Requires explicit hydration consent</div>
                    ` : 'Local NTFS File'}
                  </td>
                `).join('')}
              </tr>

              <!-- Row 11: Link Count & Volume Coverage -->
              <tr>
                <th class="category-col" scope="row">Link Count &amp; Coverage</th>
                ${group.objects.map(obj => `
                  <td>
                    <div><strong>Link Count:</strong> ${obj.linkCount}</div>
                    <div style="font-size: var(--font-size-xs); color: var(--text-dim);">${obj.coverage}</div>
                  </td>
                `).join('')}
              </tr>

              <!-- Row 12: Action Risk Class & Reasons -->
              <tr>
                <th class="category-col" scope="row">Action Risk Class &amp; Safeguards</th>
                ${group.objects.map(obj => `
                  <td>
                    ${obj.isProtected ? '<span class="badge badge-protected">protected</span>' : ''}
                    ${obj.isStale ? '<span class="badge badge-stale">prohibited</span>' : ''}
                    ${!obj.isProtected && !obj.isStale ? '<span class="badge badge-keeper">routine / caution</span>' : ''}
                    <ul style="padding-left: 14px; margin-top: 4px; font-size: var(--font-size-xs); color: var(--text-dim);">
                      ${obj.reasons.map(r => `<li>${r}</li>`).join('')}
                    </ul>
                  </td>
                `).join('')}
              </tr>

              <!-- Row 13: Bottom Action Row -->
              <tr class="row-action-select">
                <th class="category-col" scope="row">Remediation Action</th>
                ${group.objects.map(obj => {
                  const isK = obj.id === keeperId;
                  const act = actions[obj.id] || 'retain';
                  const elig = this.getEligibility(group, obj);
                  const primaryEntry = obj.directoryEntries[0];

                  return `
                    <td>
                      ${isK ? `
                        <span class="badge badge-keeper">Selected Keeper (Retained)</span>
                      ` : elig.canNativeHandoff ? `
                        <button class="btn btn-sm btn-handoff" data-group-id="${group.id}" data-obj-id="${obj.id}">
                          Native Handoff
                        </button>
                      ` : elig.canProviderHandoff ? `
                        <button class="btn btn-sm btn-handoff" data-group-id="${group.id}" data-obj-id="${obj.id}">
                          Provider Handoff
                        </button>
                      ` : elig.disabledReason ? `
                        <span style="font-size: var(--font-size-xs); color: var(--accent-amber); font-weight: 600;">${elig.disabledReason}</span>
                      ` : `
                        <select class="select-input select-action"
                                aria-label="Cleanup action for ${primaryEntry.name}"
                                data-group-id="${group.id}"
                                data-obj-id="${obj.id}">
                          <option value="retain" ${act === 'retain' ? 'selected' : ''}>Retain</option>
                          <option value="hardlink_recoverable" ${act === 'hardlink_recoverable' ? 'selected' : ''}>Hard Link (Recoverable)</option>
                          <option value="hardlink_immediate" ${act === 'hardlink_immediate' ? 'selected' : ''}>Hard Link (Immediate)</option>
                          <option value="recycle" ${act === 'recycle' ? 'selected' : ''}>Recycle Bin</option>
                          <option value="permanent_delete" ${act === 'permanent_delete' ? 'selected' : ''}>Permanently delete</option>
                        </select>
                      `}
                    </td>
                  `;
                }).join('')}
              </tr>
            </tbody>
          </table>
        </section>

        <!-- Sticky Bottom Summary Bar -->
        <aside class="sticky-summary-bar" aria-label="Remediation Consequence Summary">
          <div class="summary-metrics">
            <div class="metric-group">
              <span class="metric-label">Immediate Expected Release</span>
              <span class="metric-value metric-immediate">${formatBytes(globalAcct.immediate)}</span>
            </div>
            <div class="metric-group">
              <span class="metric-label">Conditional Future Release (Recycle)</span>
