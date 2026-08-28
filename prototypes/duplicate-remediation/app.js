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
    this.modalStack = [];

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
        e.preventDefault();
        this.closeModal(this.modalStack && this.modalStack.length > 0);
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
    const activeTrigger = triggerEl || document.activeElement;
    if (this.activeModal) {
      if (!this.modalStack) this.modalStack = [];
      this.modalStack.push({
        type: this.activeModal,
        data: this.modalData,
        triggerElement: this.modalTriggerElement
      });
    }
    this.activeModal = type;
    this.modalData = data;
    this.modalTriggerElement = activeTrigger;
    this.renderModal();
  }

  closeModal(returnToPreviousModal = false) {
    const currentTrigger = this.modalTriggerElement;

    if (returnToPreviousModal && this.modalStack && this.modalStack.length > 0) {
      const prev = this.modalStack.pop();
      this.activeModal = prev.type;
      this.modalData = prev.data;
      this.modalTriggerElement = prev.triggerElement;
      this.renderModal();
      return;
    }

    const rootTrigger = (this.modalStack && this.modalStack.length > 0)
      ? this.modalStack[0].triggerElement
      : this.modalTriggerElement;

    this.activeModal = null;
    this.modalData = null;
    this.modalStack = [];

    const container = document.getElementById('modal-container');
    if (container) {
      container.innerHTML = '';
      container.removeAttribute('role');
      container.removeAttribute('aria-label');
    }

    if (rootTrigger && typeof rootTrigger.focus === 'function') {
      rootTrigger.focus();
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
              <span class="metric-label">Conditional Future Release (Recycle)</span>
              <span class="metric-value metric-conditional">${formatBytes(globalAcct.conditional)}</span>
            </div>
            <div class="metric-group">
              <span class="metric-label">Allocation Retained for Recovery</span>
              <span class="metric-value metric-retained">${formatBytes(globalAcct.retained)}</span>
            </div>
          </div>
          <button class="btn btn-primary btn-open-action-plan" id="btn-open-action-plan-matrix" style="padding: 0.625rem 1.25rem;">
            <span>Preview Immutable Action Plan</span>
            <span aria-hidden="true">&rarr;</span>
          </button>
        </aside>
      </div>
    `;
  }

  // =========================================================================
  // VARIANT C: PLAN-BUILDER WORKSPACE
  // =========================================================================
  renderPlanVariant() {
    const group = this.getActiveGroup();
    const keeperId = this.keeperSelections[group.id];
    const actions = this.actionSelections[group.id] || {};
    const globalAcct = this.calculateGlobalAccounting();
    const filterCounts = this.getFilterCounts();
    const filteredGroups = this.getFilteredGroups();

    return `
      <div class="plan-workspace-container">
        <!-- Left Pane: Group Queue & Filters -->
        <aside class="plan-queue-pane" aria-label="Duplicate Groups Queue">
          <div style="font-weight: 700; font-size: var(--font-size-md);">Duplicate Queue</div>

          <!-- Filters -->
          <div class="queue-filter-bar" role="toolbar" aria-label="Queue Filter">
            <button class="btn btn-sm ${this.planFilter === 'all' ? 'btn-primary' : ''}" data-filter="all" aria-pressed="${this.planFilter === 'all' ? 'true' : 'false'}">
              All (${filterCounts.all})
            </button>
            <button class="btn btn-sm ${this.planFilter === 'needs_verification' ? 'btn-primary' : ''}" data-filter="needs_verification" aria-pressed="${this.planFilter === 'needs_verification' ? 'true' : 'false'}">
              Unverified (${filterCounts.needs_verification})
            </button>
            <button class="btn btn-sm ${this.planFilter === 'verified' ? 'btn-primary' : ''}" data-filter="verified" aria-pressed="${this.planFilter === 'verified' ? 'true' : 'false'}">
              Verified (${filterCounts.verified})
            </button>
            <button class="btn btn-sm ${this.planFilter === 'attention' ? 'btn-primary' : ''}" data-filter="attention" aria-pressed="${this.planFilter === 'attention' ? 'true' : 'false'}">
              Attention (${filterCounts.attention})
            </button>
          </div>

          <!-- List -->
          <div class="queue-list" role="list">
            ${filteredGroups.map(g => {
              const isActive = g.id === group.id;

              return `
                <div class="queue-item-card ${isActive ? 'active' : ''}" data-group-id="${g.id}" role="listitem" tabindex="0" aria-current="${isActive ? 'true' : 'false'}">
                  <div class="queue-item-title">
                    <span>${g.name}</span>
                    <span style="font-family: var(--font-mono); font-size: var(--font-size-xs);">${g.formattedSize}</span>
                  </div>
                  <div class="queue-item-sub">
                    <span>${g.objects.length} copies</span>
                    ${this.renderStatusBadge(g.status)}
                  </div>
                </div>
              `;
            }).join('')}
          </div>
        </aside>

        <!-- Center Pane: Synchronized Inspector & Controls -->
        <section class="plan-center-pane" aria-label="Selected Group Inspector">
          <div style="display: flex; align-items: flex-start; justify-content: space-between; gap: 1rem; border-bottom: 1px solid var(--border-subtle); padding-bottom: 0.75rem;">
            <div>
              <h2 style="font-size: var(--font-size-lg); font-weight: 700; display: flex; align-items: center; gap: 0.5rem;">
                <span>${group.name}</span>
                ${this.renderStatusBadge(group.status)}
              </h2>
              <p style="font-size: var(--font-size-xs); color: var(--text-muted); margin-top: 2px;">${group.story}</p>
            </div>
            <div style="font-size: var(--font-size-xs); text-align: right; color: var(--text-dim);">
              <div>Volume: <strong>${group.volume} (${group.filesystem})</strong></div>
              <div>Scope: <strong>${group.verificationScope}</strong></div>
            </div>
          </div>

          <!-- Staged Verification Box -->
          ${this.renderVerificationBox(group)}

          <!-- File List & Per-Item Actions -->
          <div style="display: flex; flex-direction: column; gap: 0.75rem;" role="radiogroup" aria-label="Keeper selection for ${group.name}">
            ${group.objects.map(obj => {
              const isK = obj.id === keeperId;
              const act = actions[obj.id] || 'retain';
              const elig = this.getEligibility(group, obj);
              const activeCount = group.objects.filter(o => !o.excluded).length;
              const canExclude = obj.excluded || activeCount > 1;

              return `
                <article class="card ${isK ? 'is-keeper' : ''} ${obj.excluded ? 'is-excluded' : ''}" style="padding: 0.875rem;">
                  <div style="display: flex; justify-content: space-between; align-items: flex-start; gap: 0.5rem; margin-bottom: 0.5rem;">
                    <div style="font-weight: 700; font-size: var(--font-size-sm);">
                      ${obj.directoryEntries[0].path}
                    </div>
                    <div>
                      ${isK ? '<span class="badge badge-keeper">Selected Keeper (Retained)</span>' : ''}
                      ${obj.excluded ? '<span class="badge badge-mismatch">Excluded</span>' : ''}
                      ${obj.isCloud ? '<span class="badge badge-protected">Cloud</span>' : ''}
                      ${obj.isProtected ? '<span class="badge badge-protected">Protected</span>' : ''}
                      ${obj.isStale ? '<span class="badge badge-stale">Stale</span>' : ''}
                    </div>
                  </div>

                  ${obj.directoryEntries.length > 1 ? `
                    <div style="font-size: var(--font-size-xs); color: var(--primary); margin-bottom: 6px;">
                      <strong>Hard Link Aliases:</strong> ${obj.directoryEntries.map(e => e.path).join(' &bull; ')}
                    </div>
                  ` : ''}

                  <div style="display: grid; grid-template-columns: repeat(auto-fit, minmax(160px, 1fr)); gap: 0.5rem; font-size: var(--font-size-xs); background: var(--bg-subtle); padding: 6px 8px; border-radius: var(--radius-sm); margin-bottom: 0.625rem;">
                    <div><strong>Allocated:</strong> ${formatBytes(obj.allocatedSize)}</div>
                    <div><strong>Logical:</strong> ${formatBytes(obj.logicalSize)}</div>
                    <div><strong>File ID:</strong> ${obj.fileId}</div>
                    <div><strong>Owner:</strong> ${obj.owner.split('\\')[1] || obj.owner}</div>
                  </div>

                  ${obj.mismatchDetails ? `
                    <div class="callout-box callout-danger" style="margin-bottom: 0.5rem;">
                      <strong>Mismatch:</strong> Extra stream ${obj.mismatchDetails.divergentStreams.join(', ')} &amp; divergent ACL.
                      <div style="margin-top: 4px; display: flex; gap: 6px; flex-wrap: wrap;">
                        <button class="btn btn-sm btn-danger btn-exclude"
                                data-group-id="${group.id}"
                                data-obj-id="${obj.id}"
                                ${!canExclude ? 'disabled title="Cannot exclude the only remaining active copy in this duplicate group" aria-disabled="true"' : ''}>
                          ${obj.excluded ? 'Re-include in group' : 'Exclude from group'}
                        </button>
                        <button class="btn btn-sm btn-outline btn-view-mismatch" data-group-id="${group.id}" data-obj-id="${obj.id}">
                          View Details
                        </button>
                      </div>
                      ${!canExclude ? '<div style="font-size: var(--font-size-xs); color: var(--accent-amber); margin-top: 4px;">At least one active copy must remain in the duplicate group.</div>' : ''}
                    </div>
                  ` : ''}

                  ${obj.staleReason ? `
                    <div class="callout-box callout-danger" style="margin-bottom: 0.5rem;">
                      <strong>Live Preflight Invalidation:</strong>
                      <div>${obj.staleReason}</div>
                      <div style="margin-top: 4px;">
                        <button class="btn btn-sm btn-outline btn-view-stale" data-group-id="${group.id}" data-obj-id="${obj.id}">
                          Preflight Report
                        </button>
                      </div>
                    </div>
                  ` : ''}

                  <!-- Action Bar -->
                  <div style="display: flex; align-items: center; justify-content: space-between; gap: 0.75rem; flex-wrap: wrap;">
                    <label style="display: inline-flex; align-items: center; gap: 6px; cursor: ${obj.excluded ? 'not-allowed' : 'pointer'}; font-weight: 700; font-size: var(--font-size-xs);">
                      <input type="radio"
                             name="plan-keeper-${group.id}"
                             class="plan-keeper-radio"
                             data-group-id="${group.id}"
                             data-obj-id="${obj.id}"
                             ${isK ? 'checked' : ''}
                             ${obj.excluded ? 'disabled' : ''}>
                      <span>${isK ? 'Selected Keeper (Retained)' : 'Set as Keeper'}</span>
                    </label>

                    <div style="display: flex; align-items: center; gap: 0.5rem;">
                      <label for="action-select-plan-${group.id}-${obj.id}" style="font-size: var(--font-size-xs); font-weight: 600;">Action:</label>
                      ${isK ? `
                        <span style="font-size: var(--font-size-xs); color: var(--primary); font-weight: 700;">Retain (Keeper)</span>
                      ` : elig.canNativeHandoff ? `
                        <button class="btn btn-sm btn-handoff" data-group-id="${group.id}" data-obj-id="${obj.id}">Native Handoff</button>
                      ` : elig.canProviderHandoff ? `
                        <button class="btn btn-sm btn-handoff" data-group-id="${group.id}" data-obj-id="${obj.id}">Provider Handoff</button>
                      ` : elig.disabledReason ? `
                        <span style="font-size: var(--font-size-xs); color: var(--accent-amber); font-weight: 600;">${elig.disabledReason}</span>
                      ` : `
                        <select id="action-select-plan-${group.id}-${obj.id}"
                                aria-label="Action for ${obj.directoryEntries[0].name}"
                                class="select-input select-action"
                                style="width: auto; font-size: var(--font-size-xs);"
                                data-group-id="${group.id}"
                                data-obj-id="${obj.id}">
                          <option value="retain" ${act === 'retain' ? 'selected' : ''}>Retain</option>
                          <option value="hardlink_recoverable" ${act === 'hardlink_recoverable' ? 'selected' : ''}>Hard Link (Recoverable)</option>
                          <option value="hardlink_immediate" ${act === 'hardlink_immediate' ? 'selected' : ''}>Hard Link (Immediate)</option>
                          <option value="recycle" ${act === 'recycle' ? 'selected' : ''}>Recycle Bin</option>
                          <option value="permanent_delete" ${act === 'permanent_delete' ? 'selected' : ''}>Permanently delete</option>
                        </select>
                      `}
                    </div>
                  </div>
                </article>
              `;
            }).join('')}
          </div>
        </section>

        <!-- Right Pane: Action Plan Ledger -->
        <aside class="plan-ledger-pane" aria-label="Action Plan Ledger">
          <div style="font-weight: 700; font-size: var(--font-size-md); border-bottom: 1px solid var(--border-subtle); padding-bottom: 0.5rem;">
            Action Plan Ledger
          </div>

          <!-- Reclaim Metrics Card -->
          <div class="card" style="background-color: var(--bg-subtle);">
            <div class="metric-group" style="margin-bottom: 0.5rem;">
              <span class="metric-label">Immediate Expected Release</span>
              <span class="metric-value metric-immediate">${formatBytes(globalAcct.immediate)}</span>
            </div>
            <div class="metric-group" style="margin-bottom: 0.5rem;">
              <span class="metric-label">Conditional Future Release</span>
              <span class="metric-value metric-conditional">${formatBytes(globalAcct.conditional)}</span>
            </div>
            <div class="metric-group" style="margin-bottom: 0.5rem;">
              <span class="metric-label">Allocation Retained for Recovery</span>
              <span class="metric-value metric-retained">${formatBytes(globalAcct.retained)}</span>
            </div>
          </div>

          <!-- Grouped Operations Summary -->
          <div class="ledger-section">
            <div class="ledger-section-title">
              <span>Proposed Operations</span>
              <span>${this.getProposedOperationsList().length} Ops</span>
            </div>
            <div style="display: flex; flex-direction: column; gap: 0.375rem; max-height: 240px; overflow-y: auto;">
              ${this.renderLedgerOperations()}
            </div>
          </div>

          <!-- Action Plan Preview Button -->
          <div style="margin-top: auto; padding-top: 0.5rem;">
            <button class="btn btn-primary btn-open-action-plan" id="btn-open-action-plan-ledger" style="width: 100%; padding: 0.625rem;">
              <span>Preview Action Plan</span>
              <span aria-hidden="true">&rarr;</span>
            </button>
          </div>
        </aside>
      </div>
    `;
  }

  getProposedOperationsList() {
    const list = [];
    for (const group of this.groups) {
      const keeperId = this.keeperSelections[group.id];
      const keeperObj = group.objects.find(o => o.id === keeperId);
      const actions = this.actionSelections[group.id] || {};

      for (const obj of group.objects) {
        // Excluded objects are skipped
        if (obj.excluded) continue;
        // Keeper object is never mutated
        if (obj.id === keeperId) continue;

        const act = actions[obj.id] || 'retain';
        if (act === 'retain') continue;

        // Ensure action is eligible / possible
        const elig = this.getEligibility(group, obj);
        if (act === 'hardlink_recoverable' && !elig.canHardlinkRecoverable) continue;
        if (act === 'hardlink_immediate' && !elig.canHardlinkImmediate) continue;
        if (act === 'recycle' && !elig.canRecycle) continue;
        if (act === 'permanent_delete' && !elig.canPermanentDelete) continue;
        if (act === 'native_handoff' || act === 'provider_handoff') {
          // Protected handoffs are not mutation steps in action plan
          continue;
        }

        list.push({
          group,
          obj,
          keeperObj,
          action: act
        });
      }
    }
    return list;
  }

  renderLedgerOperations() {
    const ops = this.getProposedOperationsList();
    if (ops.length === 0) {
      return '<div style="font-size: var(--font-size-xs); color: var(--text-dim);">No active remediation actions configured yet.</div>';
    }

    return ops.map(op => {
      const actDef = ACTION_TYPES[op.action.toUpperCase()] || { label: op.action };
      return `
        <div class="ledger-op-card">
          <div style="font-weight: 700; display: flex; justify-content: space-between;">
            <span>${actDef.label}</span>
            <span style="font-family: var(--font-mono);">${formatBytes(op.obj.allocatedSize)}</span>
          </div>
          <div class="mono-cell" style="font-size: var(--font-size-xs); color: var(--text-dim);">${op.obj.directoryEntries[0].name}</div>
        </div>
      `;
    }).join('');
  }

  // Verification Box shared across variants
  renderVerificationBox(group) {
    const isCandidate = group.status === 'candidate';
    const isVerifying = group.status === 'verifying';
    const isVerified = group.status === 'verified';
    const isMismatch = group.status === 'mismatch';
    const isStale = group.status === 'stale_error';

    return `
      <section class="verification-box" aria-label="Content Verification Console">
        <div class="verification-box-header">
          <div class="verification-title">
            <span aria-hidden="true">&#x1F50D;</span>
            <span>All-Stream Content Verification</span>
            ${this.renderStatusBadge(group.status)}
          </div>
          <div style="display: flex; gap: 0.5rem; align-items: center;">
            ${isCandidate || isMismatch ? `
              <button class="btn btn-sm btn-primary btn-start-verify" data-group-id="${group.id}">
                ${isCandidate ? 'Start Verification' : 'Re-run Verification'}
              </button>
            ` : ''}
            ${isVerifying ? `
              <button class="btn btn-sm btn-primary btn-step-verify" data-group-id="${group.id}">
                Advance Step (${group.verificationStepIndex + 1}/5)
              </button>
              <button class="btn btn-sm btn-cancel-verify" data-group-id="${group.id}">
                Cancel
              </button>
            ` : ''}
            ${isVerified ? `
              <span style="font-size: var(--font-size-xs); color: var(--accent-emerald); font-weight: 600;">
                Verified (${group.lastVerified})
              </span>
            ` : ''}
          </div>
        </div>

        <div style="font-size: var(--font-size-xs); color: var(--text-muted); margin-bottom: 0.5rem;">
          <div><strong>Method:</strong> ${group.verificationMethod}</div>
          <div><strong>Scope:</strong> ${group.verificationScope}</div>
        </div>

        <!-- Stepper -->
        <div class="verification-stepper" role="progressbar" aria-valuenow="${group.verificationStepIndex + 1}" aria-valuemin="1" aria-valuemax="5">
          ${VERIFICATION_STAGES.map((stage, idx) => {
            let stateClass = '';
            if (isVerified || (isVerifying && group.verificationStepIndex > idx)) {
              stateClass = 'completed';
            } else if (isVerifying && group.verificationStepIndex === idx) {
              stateClass = 'active';
            } else if (isMismatch && idx === 4) {
              stateClass = 'failed';
            } else if (isStale && idx === 4) {
              stateClass = 'failed';
            }

            return `
              <div class="step-item ${stateClass}">
                <span class="step-num">${idx + 1}</span>
                <span>${stage.name}</span>
              </div>
            `;
          }).join('')}
        </div>

        ${isMismatch ? `
          <div class="callout-box callout-danger" style="margin-top: 0.5rem;">
            <strong>Verification Result: Mismatch Detected.</strong>
            <div>${group.mismatchReason}</div>
            <div style="margin-top: 0.25rem; font-size: var(--font-size-xs);">
              You can exclude the mismatched copy from the group to unlock hard link consolidation for the verified copies.
            </div>
          </div>
        ` : ''}

        ${isStale ? `
          <div class="callout-box callout-danger" style="margin-top: 0.5rem;">
            <strong>Verification Result: Target Stale / Changed Since Scan.</strong>
            <div>${group.mismatchReason}</div>
            <div style="margin-top: 0.25rem; font-size: var(--font-size-xs);">
              Per ADR 0002 safety rules, in-place overrides are prohibited when target File IDs or content diverge. A fresh scan and Action Plan are required.
            </div>
          </div>
        ` : ''}
      </section>
    `;
  }

  renderStatusBadge(status) {
    switch (status) {
      case 'verified':
        return '<span class="badge badge-verified">&#x2714; Verified</span>';
      case 'candidate':
        return '<span class="badge badge-candidate">&#x26A0; Candidate Only</span>';
      case 'verifying':
        return '<span class="badge badge-candidate">&#x23F3; Verifying...</span>';
      case 'mismatch':
        return '<span class="badge badge-mismatch">&#x2718; Mismatch</span>';
      case 'stale_error':
        return '<span class="badge badge-stale">&#x26D4; Stale / Prohibited</span>';
      default:
        return `<span class="badge">${status}</span>`;
    }
  }

  // =========================================================================
  // MODALS & DIALOGS
  // =========================================================================
  renderModal() {
    const container = document.getElementById('modal-container');
    if (!container || !this.activeModal) return;

    container.setAttribute('role', 'region');
    container.setAttribute('aria-label', 'Dialogs and Overlays');

    if (this.activeModal === 'hydration') {
      container.innerHTML = this.renderHydrationModal();
    } else if (this.activeModal === 'action_plan') {
      container.innerHTML = this.renderActionPlanModal();
    } else if (this.activeModal === 'handoff') {
      container.innerHTML = this.renderHandoffModal();
    } else if (this.activeModal === 'mismatch_details') {
      container.innerHTML = this.renderMismatchDetailsModal();
    } else if (this.activeModal === 'stale_details') {
      container.innerHTML = this.renderStaleDetailsModal();
    } else if (this.activeModal === 'export_confirm') {
      container.innerHTML = this.renderExportConfirmModal();
    }

    this.attachModalEventListeners();

    // Trap focus inside modal
    const dialog = container.querySelector('.modal-dialog');
    if (dialog) {
      const focusable = dialog.querySelectorAll('button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex="0"]');
      if (focusable.length > 0) {
        focusable[0].focus();
      }

      dialog.addEventListener('keydown', (e) => {
        if (e.key === 'Tab') {
          const first = focusable[0];
          const last = focusable[focusable.length - 1];
          if (e.shiftKey) {
            if (document.activeElement === first) {
              e.preventDefault();
              last.focus();
            }
          } else {
            if (document.activeElement === last) {
              e.preventDefault();
              first.focus();
            }
          }
        }
      });
    }
  }

  renderHydrationModal() {
    const group = this.groups.find(g => g.id === this.modalData.groupId) || this.groups[2];
    const req = group.hydrationRequirements;

    return `
      <div class="modal-overlay" role="presentation">
        <div class="modal-dialog" role="dialog" aria-modal="true" aria-labelledby="hydration-modal-title">
          <div class="modal-header">
            <h3 id="hydration-modal-title">
              <span aria-hidden="true">&#x2601;</span>
              <span>Cloud Hydration Consent Required</span>
            </h3>
            <button class="btn btn-sm btn-close-modal" aria-label="Close dialog">&times;</button>
          </div>
          <div class="modal-body">
            <p>
              Target duplicate candidate <code>C:\\Users\\Alex\\OneDrive\\Projects\\2023_RenderProject.zip</code> is currently an <strong>online-only cloud placeholder</strong> (Allocated Size: 0 bytes).
            </p>
            <div class="callout-box callout-warning">
              <strong>Hydration Resource Impact:</strong>
              <div style="margin-top: 4px;">• Method &amp; Scope: <strong>${group.verificationMethod} (${group.verificationScope})</strong></div>
              <div>• Estimated Network Download: <strong>${req.formattedDownload}</strong></div>
              <div>• Estimated Local Disk Allocation: <strong>${req.formattedDownload} on C: Volume</strong></div>
              <div>• Cloud Provider &amp; Client: <strong>${req.provider}</strong></div>
            </div>
            <p style="font-size: var(--font-size-xs); color: var(--text-muted);">
              Full byte-by-byte cryptographic verification requires downloading the cloud-managed content. Declining consent leaves the item unverified. Note: direct deletion or hard linking of cloud-managed files in PigTree is protected in v1 and routes to OneDrive / Explorer handoffs.
            </p>
          </div>
          <div class="modal-footer">
            <button class="btn btn-close-modal">Decline (Keep Unverified)</button>
            <button class="btn btn-primary btn-grant-hydration" data-group-id="${group.id}">
              Consent to Hydration &amp; Verify
            </button>
          </div>
        </div>
      </div>
    `;
  }

  renderHandoffModal() {
    const { groupId, objId } = this.modalData;
    const group = this.groups.find(g => g.id === groupId);
    const obj = group ? group.objects.find(o => o.id === objId) : null;
    const handoff = (obj && obj.handoffInfo) || {
      toolName: "Windows Settings",
      instructions: "Use native Windows tools to safely manage system resources.",
      rescanExpectation: "Perform a fresh PigTree scan after making changes."
    };

    return `
      <div class="modal-overlay" role="presentation">
        <div class="modal-dialog" role="dialog" aria-modal="true" aria-labelledby="handoff-modal-title">
          <div class="modal-header">
            <h3 id="handoff-modal-title">
              <span aria-hidden="true">&#x1F6E1;</span>
              <span>Protected Resource Native Handoff</span>
            </h3>
            <button class="btn btn-sm btn-close-modal" aria-label="Close dialog">&times;</button>
          </div>
          <div class="modal-body">
            <div class="callout-box callout-info">
              <strong>Safe System / Provider Handoff:</strong>
              <div>Target: <code>${obj ? obj.directoryEntries[0].path : ''}</code></div>
              <div style="margin-top: 4px;">Reason: ${obj ? obj.protectionReason : 'Protected system or cloud resource.'}</div>
            </div>
            <div style="margin-top: 0.75rem;">
              <strong>Supported Administrative Tool:</strong>
              <div style="font-size: var(--font-size-md); font-weight: 700; color: var(--primary); margin-top: 2px;">
                ${handoff.toolName}
              </div>
              <p style="margin-top: 6px; font-size: var(--font-size-xs); color: var(--text-muted);">
                ${handoff.instructions}
              </p>
              ${handoff.rescanExpectation ? `
                <div class="callout-box callout-warning" style="margin-top: 8px; font-size: var(--font-size-xs);">
                  <strong>Rescan Expectation:</strong>
                  <div>${handoff.rescanExpectation}</div>
                </div>
              ` : ''}
            </div>
          </div>
          <div class="modal-footer">
            <button class="btn btn-primary btn-close-modal">Understood / Return to Review</button>
          </div>
        </div>
      </div>
    `;
  }

  renderMismatchDetailsModal() {
    const { groupId, objId } = this.modalData;
    const group = this.groups.find(g => g.id === groupId);
    const obj = group?.objects.find(o => o.id === objId);
    const keeper = group?.objects.find(o => o.id === this.keeperSelections[groupId]);

    return `
      <div class="modal-overlay" role="presentation">
        <div class="modal-dialog" role="dialog" aria-modal="true" aria-labelledby="mismatch-modal-title" style="max-width: 680px;">
          <div class="modal-header">
            <h3 id="mismatch-modal-title">
              <span aria-hidden="true">&#x2718;</span>
              <span>Stream &amp; Security Mismatch Details</span>
            </h3>
            <button class="btn btn-sm btn-close-modal" aria-label="Close dialog">&times;</button>
          </div>
          <div class="modal-body">
            <p>Verification detected content stream or security descriptor divergence between this candidate and the candidate keeper.</p>
            <div class="callout-box callout-danger" style="margin-top: 0.5rem;">
              <strong>Target Entry:</strong> ${obj?.directoryEntries[0].path}
              <div style="margin-top: 4px;">• Divergent Streams: <strong>${obj?.mismatchDetails?.divergentStreams?.join(', ') || 'None'}</strong></div>
              <div>• Access Control (ACL): <strong>${obj?.mismatchDetails?.divergentAcl || obj?.accessRules}</strong></div>
              <div>• Safeguard Rule: <strong>${obj?.mismatchDetails?.actionBlockReason || 'Hard link substitution blocked.'}</strong></div>
            </div>
            <div style="margin-top: 0.75rem; font-size: var(--font-size-xs); color: var(--text-muted);">
              Reference Keeper (${keeper?.directoryEntries[0].name}): Stream count ${keeper?.streamCount}, ACL: ${keeper?.accessRules}.
            </div>
          </div>
          <div class="modal-footer">
            <button class="btn btn-primary btn-close-modal">Return to Review</button>
          </div>
        </div>
      </div>
    `;
  }

  renderStaleDetailsModal() {
    const { groupId, objId } = this.modalData;
    const group = this.groups.find(g => g.id === groupId);
    const obj = group?.objects.find(o => o.id === objId);

    return `
      <div class="modal-overlay" role="presentation">
        <div class="modal-dialog" role="dialog" aria-modal="true" aria-labelledby="stale-modal-title" style="max-width: 680px;">
          <div class="modal-header">
            <h3 id="stale-modal-title">
              <span aria-hidden="true">&#x26D4;</span>
              <span>Live Preflight Invalidation Report</span>
            </h3>
            <button class="btn btn-sm btn-close-modal" aria-label="Close dialog">&times;</button>
          </div>
          <div class="modal-body">
            <p>Target object identity or content has changed since the baseline Analysis Snapshot. In-place overrides are prohibited by ADR 0002 safety rules.</p>
            <div class="callout-box callout-danger" style="margin-top: 0.5rem;">
              <strong>Invalidation Reason:</strong>
              <div style="margin-top: 4px;">${obj?.staleReason || group?.mismatchReason}</div>
              <div style="margin-top: 4px;">Observed Live File ID: <code>${obj?.fileId}</code> | Observed MTime: <code>${obj?.mtime}</code></div>
            </div>
            <p style="font-size: var(--font-size-xs); color: var(--text-muted); margin-top: 0.5rem;">
              To remediate this set, trigger a fresh volume scan to obtain a new synchronized snapshot.
            </p>
          </div>
          <div class="modal-footer">
            <button class="btn btn-primary btn-close-modal">Return to Review</button>
          </div>
        </div>
      </div>
    `;
  }

  renderActionPlanModal() {
    const globalAcct = this.calculateGlobalAccounting();
    const ops = this.getProposedOperationsList();

    return `
      <div class="modal-overlay" role="presentation">
        <div class="modal-dialog" role="dialog" aria-modal="true" aria-labelledby="plan-preview-modal-title" style="max-width: 960px;">
          <div class="modal-header">
            <div>
              <h3 id="plan-preview-modal-title">
                <span aria-hidden="true">&#x1F4CB;</span>
                <span>Action Plan Preview (Immutable Manifest)</span>
              </h3>
              <div style="font-size: var(--font-size-xs); color: var(--text-dim); margin-top: 2px;">
                Source Snapshot: C: (Today 10:42) &bull; Mode: Read-Only Review &bull; Commit Point: Rename-Over &amp; Staging Purge
              </div>
            </div>
            <button class="btn btn-sm btn-close-modal" aria-label="Close dialog">&times;</button>
          </div>
          <div class="modal-body">
            <!-- Summary Metrics Header -->
            <div style="display: grid; grid-template-columns: repeat(auto-fit, minmax(200px, 1fr)); gap: 0.75rem;">
              <div class="card" style="border-color: var(--accent-emerald-border); background: var(--accent-emerald-bg);">
                <div class="metric-label" style="color: var(--accent-emerald);">Immediate Expected Release</div>
                <div class="metric-value metric-immediate">${formatBytes(globalAcct.immediate)}</div>
                <div style="font-size: var(--font-size-xs); color: var(--text-dim);">Directly reclaimed upon Commit Point</div>
              </div>
              <div class="card" style="border-color: var(--accent-amber-border); background: var(--accent-amber-bg);">
                <div class="metric-label" style="color: var(--accent-amber);">Conditional Future Release</div>
                <div class="metric-value metric-conditional">${formatBytes(globalAcct.conditional)}</div>
                <div style="font-size: var(--font-size-xs); color: var(--text-dim);">Windows Recycle Bin (reclaim on empty)</div>
              </div>
              <div class="card" style="border-color: var(--primary-border); background: var(--primary-subtle);">
                <div class="metric-label" style="color: var(--primary);">Retained for Recovery</div>
                <div class="metric-value metric-retained">${formatBytes(globalAcct.retained)}</div>
                <div style="font-size: var(--font-size-xs); color: var(--text-dim);">PigTree Recovery Vault (durable)</div>
              </div>
            </div>

            <!-- Preconditions & Revalidation Policy -->
            <div class="callout-box callout-info" style="margin-top: 0.75rem;">
              <strong>Immutable Preflight Safety Policy &amp; Execution Boundaries (ADR 0002):</strong>
              <div style="margin-top: 2px; font-size: var(--font-size-xs);">
                • <strong>Target Re-observation:</strong> Before each mutation, helper verifies Volume IDs, File IDs, link counts, parent paths, and cryptographic digests without following symlinks.<br>
                • <strong>Commit Point:</strong> Hard link staging link creation &rarr; atomic replace &rarr; purge staging link. Group failure safely aborts without partial corruption.<br>
                • <strong>Protected Resources:</strong> System and cloud items require native handoffs and are excluded from mutation operations.
              </div>
            </div>

            <!-- Grouped Operations Manifest -->
            <section aria-label="Authorized Operations Ledger" style="margin-top: 0.75rem;">
              <h4 style="font-size: var(--font-size-md); font-weight: 700; margin-bottom: 0.5rem;">
                Authorized Step Manifest (${ops.length} Operations across ${globalAcct.totalVictims} Distinct Victim Objects)
              </h4>
              <div style="display: flex; flex-direction: column; gap: 0.5rem; max-height: 280px; overflow-y: auto;">
                ${ops.length === 0 ? `
                  <div style="padding: 1rem; text-align: center; color: var(--text-dim);">No active cleanup operations configured. All duplicate copies are set to Retain.</div>
                ` : ops.map((op, i) => {
                  const keeper = op.keeperObj || op.group.objects.find(o => o.id === this.keeperSelections[op.group.id]);
                  const actDef = ACTION_TYPES[op.action.toUpperCase()] || { label: op.action, recoveryClass: 'permanent', riskClass: 'caution' };

                  return `
                    <div class="card" style="padding: 0.75rem; font-size: var(--font-size-xs);">
                      <div style="display: flex; justify-content: space-between; align-items: flex-start; gap: 0.5rem;">
                        <div>
                          <strong>Step ${i + 1}: ${actDef.label}</strong>
                          <div class="mono-cell" style="margin-top: 2px;">Target: ${op.obj.directoryEntries[0].path}</div>
                          <div class="mono-cell" style="color: var(--text-dim);">Expected File ID: ${op.obj.fileId} | Size: ${formatBytes(op.obj.allocatedSize)} | Recovery Class: ${actDef.recoveryClass}</div>
                        </div>
                        <span class="badge badge-keeper">${actDef.riskClass}</span>
                      </div>
                      ${op.action.includes('hardlink') ? `
                        <div style="margin-top: 6px; padding-top: 6px; border-top: 1px dashed var(--border-subtle); color: var(--text-muted);">
                          <strong>Redirect Target:</strong> Points to Keeper <code>${keeper ? keeper.directoryEntries[0].path : ''}</code> (File ID: ${keeper ? keeper.fileId : ''})
                          <div style="font-size: var(--font-size-xs); color: var(--text-dim); margin-top: 2px;">
                            Recovery Artifact: ${op.action === 'hardlink_recoverable' ? 'PigTree Recovery Vault preservation link (Retained)' : 'None (Immediate staging link purge - Permanent)'}
                          </div>
                        </div>
                      ` : ''}
                    </div>
                  `;
                }).join('')}
              </div>
            </section>
          </div>
          <div class="modal-footer" style="justify-content: space-between;">
            <div style="font-size: var(--font-size-xs); color: var(--text-dim);">
              Note: This prototype flow stops at this preview and never executes mutations.
            </div>
            <div style="display: flex; gap: 0.5rem;">
              <button class="btn btn-close-modal">Back to Review</button>
              <button class="btn btn-primary btn-export-preview">Export Preview (JSON)</button>
            </div>
          </div>
        </div>
      </div>
    `;
  }

  renderExportConfirmModal() {
    const operations = this.getProposedOperationsList().map((op, i) => {
      const keeper = op.keeperObj || op.group.objects.find(o => o.id === this.keeperSelections[op.group.id]);
      const actDef = ACTION_TYPES[op.action.toUpperCase()] || { label: op.action, recoveryClass: 'permanent', riskClass: 'caution' };

      return {
        stepIndex: i + 1,
        groupName: op.group.name,
        groupId: op.group.id,
        targetEntry: {
          path: op.obj.directoryEntries[0].path,
          parent: op.obj.directoryEntries[0].parent,
          name: op.obj.directoryEntries[0].name,
          expectedFileId: op.obj.fileId,
          allocatedBytes: op.obj.allocatedSize,
          logicalBytes: op.obj.logicalSize
        },
        action: op.action,
        actionLabel: actDef.label,
        recoveryClass: actDef.recoveryClass,
        riskClass: actDef.riskClass,
        keeperTarget: {
          keeperFileId: keeper ? keeper.fileId : null,
          keeperPrimaryPath: keeper ? keeper.directoryEntries[0].path : null,
          directoryEntries: keeper ? keeper.directoryEntries.map(e => ({ path: e.path, isPrimary: e.isPrimary || false })) : []
        },
        preconditions: [
          "Re-open target entry without following symlinks/reparse points",
          `Verify Volume ID and File ID matches expected ${op.obj.fileId}`,
          "Verify link count and directory entry parent identity",
          "Verify all-stream cryptographic hash matches verified snapshot digest"
        ],
        commitPointSemantics: op.action === 'hardlink_immediate'
          ? "Hard link created under staging name, atomic replace via SetFileInformationByHandle (FileRenameInfo), immediate purge of staging reference."
          : op.action === 'hardlink_recoverable'
          ? "Victim entry linked to recovery vault, atomic hard link replace to keeper."
          : op.action === 'recycle'
          ? "Entry moved to Windows Recycle Bin via IFileOperation."
          : "Direct Win32 entry deletion without recycling."
      };
    });

    const manifest = {
      actionPlanVersion: "1.0.0",
      generatedAt: new Date().toISOString(),
      sourceSnapshot: {
        target: "C:",
        filesystem: "NTFS",
        snapshotTimestamp: "Today 10:42",
        coverage: "Complete"
      },
      accounting: this.calculateGlobalAccounting(),
      totalOperations: operations.length,
      operations
    };

    return `
      <div class="modal-overlay" role="presentation">
        <div class="modal-dialog" role="dialog" aria-modal="true" aria-labelledby="export-modal-title">
          <div class="modal-header">
            <h3 id="export-modal-title">
              <span aria-hidden="true">&#x2714;</span>
              <span>Action Plan Preview Exported</span>
            </h3>
            <button class="btn btn-sm btn-close-modal" aria-label="Close dialog">&times;</button>
          </div>
          <div class="modal-body">
            <p>The immutable Action Plan manifest has been compiled in memory. You can inspect or copy the JSON payload below:</p>
            <pre class="mono-cell" style="background: var(--bg-subtle); padding: 0.75rem; border-radius: var(--radius-sm); max-height: 280px; overflow-y: auto; font-size: var(--font-size-xs);">${JSON.stringify(manifest, null, 2)}</pre>
          </div>
          <div class="modal-footer" style="justify-content: space-between;">
            <button class="btn btn-secondary btn-return-plan">Back to Action Plan</button>
            <button class="btn btn-primary btn-close-modal">Close</button>
          </div>
        </div>
      </div>
    `;
  }

  // =========================================================================
  // EVENT LISTENERS ATTACHMENT
  // =========================================================================
  attachEventListeners() {
    // Guided Scenario Selector
    const guidedSelect = document.getElementById('guided-group-select');
    if (guidedSelect) {
      guidedSelect.addEventListener('change', (e) => {
        this.setActiveGroup(e.target.value);
      });
    }

    // Matrix Scenario Selector
    const matrixSelect = document.getElementById('matrix-group-select');
    if (matrixSelect) {
      matrixSelect.addEventListener('change', (e) => {
        this.setActiveGroup(e.target.value);
      });
    }

    // Guided Step Rail Clicks
    document.querySelectorAll('.rail-step').forEach(btn => {
      btn.addEventListener('click', () => {
        const step = parseInt(btn.getAttribute('data-step'), 10);
        if (!isNaN(step)) {
          this.guidedStep = step;
          this.announce(`Navigated to Step ${step}`);
          this.render();
        }
      });
    });

    // Queue Item Clicks (Plan Variant)
    document.querySelectorAll('.queue-item-card').forEach(card => {
      card.addEventListener('click', () => {
        const gid = card.getAttribute('data-group-id');
        if (gid) this.setActiveGroup(gid);
      });
      card.addEventListener('keydown', (e) => {
        if (e.key === 'Enter' || e.key === ' ') {
          e.preventDefault();
          const gid = card.getAttribute('data-group-id');
          if (gid) this.setActiveGroup(gid);
        }
      });
    });

    // Queue Filter Buttons (Plan Variant)
    document.querySelectorAll('.queue-filter-bar button').forEach(btn => {
      btn.addEventListener('click', () => {
        const f = btn.getAttribute('data-filter');
        if (f) {
          this.setPlanFilter(f);
        }
      });
    });

    // Guided Keeper Radios
    document.querySelectorAll('.guided-keeper-radio').forEach(radio => {
      radio.addEventListener('change', () => {
        const gid = radio.getAttribute('data-group-id');
        const oid = radio.getAttribute('data-obj-id');
        if (gid && oid) this.setKeeper(gid, oid);
      });
    });

    // Matrix Keeper Radios
    document.querySelectorAll('.matrix-keeper-radio').forEach(radio => {
      radio.addEventListener('change', () => {
        const gid = radio.getAttribute('data-group-id');
        const oid = radio.getAttribute('data-obj-id');
        if (gid && oid) this.setKeeper(gid, oid);
      });
    });

    // Plan Keeper Radios
    document.querySelectorAll('.plan-keeper-radio').forEach(radio => {
      radio.addEventListener('change', () => {
        const gid = radio.getAttribute('data-group-id');
        const oid = radio.getAttribute('data-obj-id');
        if (gid && oid) this.setKeeper(gid, oid);
      });
    });

    // Action Select Dropdowns
    document.querySelectorAll('.select-action').forEach(sel => {
      sel.addEventListener('change', (e) => {
        const gid = sel.getAttribute('data-group-id');
        const oid = sel.getAttribute('data-obj-id');
        if (gid && oid) this.setAction(gid, oid, e.target.value);
      });
    });

    // Exclude / Re-include buttons
    document.querySelectorAll('.btn-exclude').forEach(btn => {
      btn.addEventListener('click', () => {
        const gid = btn.getAttribute('data-group-id');
        const oid = btn.getAttribute('data-obj-id');
        const group = this.groups.find(g => g.id === gid);
        const obj = group ? group.objects.find(o => o.id === oid) : null;
        if (gid && oid && obj) {
          this.excludeObject(gid, oid, !obj.excluded);
        }
      });
    });

    // Verification Buttons
    document.querySelectorAll('.btn-start-verify').forEach(btn => {
      btn.addEventListener('click', () => {
        const gid = btn.getAttribute('data-group-id');
        if (gid) this.startVerification(gid);
      });
    });

    document.querySelectorAll('.btn-step-verify').forEach(btn => {
      btn.addEventListener('click', () => {
        const gid = btn.getAttribute('data-group-id');
        if (gid) this.advanceVerificationStep(gid);
      });
    });

    document.querySelectorAll('.btn-cancel-verify').forEach(btn => {
      btn.addEventListener('click', () => {
        const gid = btn.getAttribute('data-group-id');
        if (gid) this.cancelVerification(gid);
      });
    });

    // Handoff buttons
    document.querySelectorAll('.btn-handoff').forEach(btn => {
      btn.addEventListener('click', () => {
        const gid = btn.getAttribute('data-group-id');
        const oid = btn.getAttribute('data-obj-id');
        if (gid && oid) {
          this.openModal('handoff', { groupId: gid, objId: oid }, btn);
        }
      });
    });

    // Mismatch Details buttons
    document.querySelectorAll('.btn-view-mismatch').forEach(btn => {
      btn.addEventListener('click', () => {
        const gid = btn.getAttribute('data-group-id');
        const oid = btn.getAttribute('data-obj-id');
        if (gid && oid) {
          this.openModal('mismatch_details', { groupId: gid, objId: oid }, btn);
        }
      });
    });

    // Stale Details buttons
    document.querySelectorAll('.btn-view-stale').forEach(btn => {
      btn.addEventListener('click', () => {
        const gid = btn.getAttribute('data-group-id');
        const oid = btn.getAttribute('data-obj-id');
        if (gid && oid) {
          this.openModal('stale_details', { groupId: gid, objId: oid }, btn);
        }
      });
    });

    // Open Action Plan Preview Buttons (across all views)
    document.querySelectorAll('.btn-open-action-plan').forEach(btn => {
      btn.addEventListener('click', () => {
        this.openModal('action_plan', null, btn);
      });
    });
  }

  attachModalEventListeners() {
    // Close modal buttons (close all / return to root trigger)
    document.querySelectorAll('.btn-close-modal').forEach(btn => {
      btn.addEventListener('click', () => {
        this.closeModal(false);
      });
    });

    // Return to previous modal button (e.g. from Export Confirm back to Action Plan)
    document.querySelectorAll('.btn-return-plan').forEach(btn => {
      btn.addEventListener('click', () => {
        this.closeModal(true);
      });
    });

    // Grant Cloud Hydration button
    document.querySelectorAll('.btn-grant-hydration').forEach(btn => {
      btn.addEventListener('click', () => {
        const gid = btn.getAttribute('data-group-id');
        if (gid) this.grantCloudHydrationConsent(gid);
      });
    });

    // Export preview button in modal
    const btnExport = document.querySelector('.btn-export-preview');
    if (btnExport) {
      btnExport.addEventListener('click', () => {
        this.openModal('export_confirm', null, btnExport);
      });
    }

    // Modal background click to close
    const overlay = document.querySelector('.modal-overlay');
    if (overlay) {
      overlay.addEventListener('click', (e) => {
        if (e.target === overlay) {
          this.closeModal();
        }
      });
    }
  }

  // Update Collapsible State Inspector JSON
  updateStateInspector() {
    const target = document.getElementById('state-inspector-json');
    if (!target) return;

    const stateDump = {
      currentVariant: this.currentVariant,
      activeGroupId: this.activeGroupId,
      guidedStep: this.guidedStep,
      planFilter: this.planFilter,
      globalAccounting: this.calculateGlobalAccounting(),
      keeperSelections: this.keeperSelections,
      actionSelections: this.actionSelections,
      groups: this.groups.map(g => ({
        id: g.id,
        name: g.name,
        status: g.status,
        verificationStepIndex: g.verificationStepIndex,
        cloudConsent: g.cloudHydrationConsentGiven || false,
        accounting: this.calculateGroupAccounting(g),
        objects: g.objects.map(o => ({
          id: o.id,
          fileId: o.fileId,
          excluded: o.excluded || false,
          action: (this.actionSelections[g.id] && this.actionSelections[g.id][o.id]) || 'retain',
          isKeeper: this.keeperSelections[g.id] === o.id
        }))
      }))
    };

    target.textContent = JSON.stringify(stateDump, null, 2);
  }
}

// Bootstrap application on DOMContentLoaded
window.addEventListener('DOMContentLoaded', () => {
  window.PigTreeApp = new PrototypeApp();
});
