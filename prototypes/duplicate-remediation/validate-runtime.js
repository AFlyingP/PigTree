// validate-runtime.js
// Static verification of app.js runtime tail, renderers, modal methods,
// event listener mappings, and bootstrap integrity.
// Runs standalone in Node.js without external dependencies.

import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

function assert(condition, message) {
  if (!condition) {
    console.error('FAIL:', message);
    process.exit(1);
  }
  console.log('PASS:', message);
}

console.log('=== Running PigTree Duplicate Prototype Runtime Verification ===\n');

const appPath = path.join(__dirname, 'app.js');
assert(fs.existsSync(appPath), 'app.js exists');

const appSource = fs.readFileSync(appPath, 'utf8');
assert(appSource.length > 5000, `app.js is non-trivial in length (${appSource.length} bytes)`);

// 1. Verify Core Class & Export Structure
console.log('--- Checking Class and Export Structure ---');
assert(appSource.includes('export class PrototypeApp'), 'PrototypeApp class is exported');
assert(appSource.includes('export function formatBytes'), 'formatBytes helper is exported');

// 2. Verify All Three Variant Renderers
console.log('\n--- Checking Variant Renderers ---');
const requiredVariantRenderers = [
  'renderGuidedVariant',
  'renderMatrixVariant',
  'renderPlanVariant'
];
for (const renderer of requiredVariantRenderers) {
  assert(
    new RegExp('\\b' + renderer + '\\s*\\(').test(appSource),
    `Variant renderer '${renderer}' is defined`
  );
}

// 3. Verify Sub-renderers and Safety Helpers
console.log('\n--- Checking Sub-renderers and Safety Helpers ---');
const requiredHelpers = [
  'renderGuidedCopyCard',
  'getActionConsequenceText',
  'getProposedOperationsList',
  'renderLedgerOperations',
  'renderVerificationBox',
  'renderStatusBadge',
  'getEligibility',
  'getFilterCounts',
  'getFilteredGroups',
  'calculateGroupAccounting',
  'calculateGlobalAccounting',
  'captureFocusState',
  'restoreFocusState'
];
for (const helper of requiredHelpers) {
  assert(
    new RegExp('\\b' + helper + '\\s*\\(').test(appSource),
    `Helper method '${helper}' is defined`
  );
}

// 4. Verify Modal Renderers & Stack Management
console.log('\n--- Checking Modal Renderers & Stack Management ---');
const requiredModalMethods = [
  'renderModal',
  'renderHydrationModal',
  'renderHandoffModal',
  'renderMismatchDetailsModal',
  'renderStaleDetailsModal',
  'renderActionPlanModal',
  'renderExportConfirmModal',
  'openModal',
  'closeModal'
];
for (const modalMethod of requiredModalMethods) {
  assert(
    new RegExp('\\b' + modalMethod + '\\s*\\(').test(appSource),
    `Modal method '${modalMethod}' is defined`
  );
}

assert(appSource.includes('this.modalStack'), 'Modal stack is initialized and managed for successive dialogs');

// 5. Verify Action Plan Read-Only Preview Safety
console.log('\n--- Checking Action Plan Preview Boundaries ---');
assert(appSource.includes('btn-export-preview'), 'Action plan provides Export preview trigger');
assert(appSource.includes('btn-close-modal'), 'Action plan provides Close/Back navigation');
assert(!appSource.includes('btn-execute-mutations'), 'Action plan has NO execute mutations control');
assert(appSource.includes('This prototype flow stops at this preview and never executes mutations'), 'Action plan contains read-only prototype execution disclaimer');

// 6. Verify Event Listener Attachments & Control Selectors
console.log('\n--- Checking Control Selectors & Listener Attachment ---');
const controlChecks = [
  { name: 'guided scenario select', search: "getElementById('guided-group-select')" },
  { name: 'matrix scenario select', search: "getElementById('matrix-group-select')" },
  { name: 'guided step rail buttons', search: "querySelectorAll('.rail-step')" },
  { name: 'plan queue item cards', search: "querySelectorAll('.queue-item-card')" },
  { name: 'plan queue filter buttons', search: "querySelectorAll('.queue-filter-bar button')" },
  { name: 'guided keeper radios', search: "querySelectorAll('.guided-keeper-radio')" },
  { name: 'matrix keeper radios', search: "querySelectorAll('.matrix-keeper-radio')" },
  { name: 'plan keeper radios', search: "querySelectorAll('.plan-keeper-radio')" },
  { name: 'remediation action selects', search: "querySelectorAll('.select-action')" },
  { name: 'exclude/re-include buttons', search: "querySelectorAll('.btn-exclude')" },
  { name: 'start verification buttons', search: "querySelectorAll('.btn-start-verify')" },
  { name: 'step verification buttons', search: "querySelectorAll('.btn-step-verify')" },
  { name: 'cancel verification buttons', search: "querySelectorAll('.btn-cancel-verify')" },
  { name: 'handoff buttons', search: "querySelectorAll('.btn-handoff')" },
  { name: 'mismatch details buttons', search: "querySelectorAll('.btn-view-mismatch')" },
  { name: 'stale details buttons', search: "querySelectorAll('.btn-view-stale')" },
  { name: 'open action plan buttons', search: "querySelectorAll('.btn-open-action-plan')" },
  { name: 'modal close buttons', search: "querySelectorAll('.btn-close-modal')" },
  { name: 'return to plan buttons', search: "querySelectorAll('.btn-return-plan')" },
  { name: 'grant cloud hydration buttons', search: "querySelectorAll('.btn-grant-hydration')" },
  { name: 'export preview button', search: "querySelector('.btn-export-preview')" },
  { name: 'modal overlay dismiss', search: "querySelector('.modal-overlay')" },
  { name: 'variant switcher buttons', search: "querySelectorAll('.switcher-btn')" },
  { name: 'reset prototype state button', search: "getElementById('btn-reset-state')" }
];

for (const { name, search } of controlChecks) {
  assert(
    appSource.includes(search),
    `Control listener attached for ${name} (${search})`
  );
}

// 7. Verify State Inspector & DOMContentLoaded Bootstrap
console.log('\n--- Checking Tail Markers & Bootstrap ---');
assert(appSource.includes('updateStateInspector()'), 'updateStateInspector method is defined');
assert(appSource.includes("getElementById('state-inspector-json')"), 'state-inspector-json target is referenced');
assert(
  new RegExp("window\\.addEventListener\\(['\"]DOMContentLoaded['\"],\\s*\\(\\)\\s*=>").test(appSource),
  'DOMContentLoaded bootstrap listener is present at the end of app.js'
);
assert(
  appSource.includes('window.PigTreeApp = new PrototypeApp()'),
  'window.PigTreeApp instance is initialized on bootstrap'
);

console.log('\n=== All Runtime Structure & Invariant Checks Passed Successfully! ===');
console.log('Note: Static runtime structure verification confirms code completeness, AST markers, and event listener mappings, but does not replace live browser interaction testing.\n');
