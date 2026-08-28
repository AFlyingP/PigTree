// validate-prototype.js
// Deterministic validation of mock data, accounting invariants, eligibility, and state progression
// Runs standalone in Node.js without external dependencies.

import { INITIAL_GROUPS, VERIFICATION_STAGES, ACTION_TYPES } from './mock-data.js';

function assert(condition, message) {
  if (!condition) {
    console.error('FAIL:', message);
    process.exit(1);
  }
  console.log('PASS:', message);
}

function assertEqual(actual, expected, message) {
  if (actual !== expected) {
    console.error(`FAIL: ${message} (expected ${expected}, got ${actual})`);
    process.exit(1);
  }
  console.log(`PASS: ${message}`);
}

console.log('=== Running PigTree Duplicate Remediation Prototype Validation ===\n');

// 1. Validate Scenarios Structure & Invariants
assert(Array.isArray(INITIAL_GROUPS) && INITIAL_GROUPS.length === 5, 'Must contain exactly 5 scenarios');

const vacationGroup = INITIAL_GROUPS.find(g => g.id === 'group-vacation');
assert(vacationGroup, 'Scenario 1: Vacation originals exists');
assertEqual(vacationGroup.objects.length, 3, 'Vacation originals has 3 objects');
assertEqual(vacationGroup.objects[0].allocatedSize, 4800000000, 'Vacation object 1 allocated size is 4.8 GB');
assertEqual(vacationGroup.objects[2].streamCount, 2, 'Vacation object 3 has 2 streams (unnamed + Zone.Identifier)');
assert(vacationGroup.objects[2].accessRules.includes('DESKTOP-ALEX\\Alex:(R)'), 'Vacation object 3 has divergent ACL');

const buildGroup = INITIAL_GROUPS.find(g => g.id === 'group-build');
assert(buildGroup, 'Scenario 2: Build artifacts exists');
assertEqual(buildGroup.objects.length, 4, 'Build artifacts has 4 objects');
const linkedObj = buildGroup.objects.find(o => o.directoryEntries.length > 1);
assert(linkedObj, 'Build artifacts contains object with Hard Link aliases');
assertEqual(linkedObj.linkCount, 2, 'Linked object has linkCount = 2');
assertEqual(linkedObj.directoryEntries.length, 2, 'Linked object has 2 directory entries for 1 object');

const oneDriveGroup = INITIAL_GROUPS.find(g => g.id === 'group-onedrive');
assert(oneDriveGroup, 'Scenario 3: OneDrive project archive exists');
assertEqual(oneDriveGroup.objects.length, 2, 'OneDrive archive has 2 objects');
assertEqual(oneDriveGroup.objects[0].allocatedSize, 7400000000, 'OneDrive local object allocated size is 7.4 GB');
assertEqual(oneDriveGroup.objects[1].allocatedSize, 0, 'OneDrive cloud placeholder allocated size is 0 bytes');
assert(oneDriveGroup.objects[1].isCloud === true, 'OneDrive cloud placeholder is marked isCloud');

const installerGroup = INITIAL_GROUPS.find(g => g.id === 'group-installer');
assert(installerGroup, 'Scenario 4: Installer cache lookalikes exists');
assertEqual(installerGroup.objects.length, 2, 'Installer lookalikes has 2 objects');
assert(installerGroup.objects[0].isProtected === true, 'WindowsApps installer object is marked isProtected');
assert(installerGroup.objects[1].isProtected === true, 'SystemTemp installer object is marked isProtected');

const staleGroup = INITIAL_GROUPS.find(g => g.id === 'group-stale');
assert(staleGroup, 'Scenario 5: Changed since scan exists');
assertEqual(staleGroup.objects.length, 2, 'Stale pair has 2 objects');
assert(staleGroup.objects[1].isStale === true, 'Modified object is marked isStale');
assert(staleGroup.objects[1].fileId !== staleGroup.objects[0].fileId, 'Divergent File ID is present');

// 2. Validate No Raw Command Strings in Handoffs
console.log('\n--- Testing Safe Structured Handoffs ---');
for (const group of INITIAL_GROUPS) {
  for (const obj of group.objects) {
    if (obj.handoffInfo) {
      assert(!obj.handoffInfo.command, `Object ${obj.id} handoffInfo must NOT contain raw executable command strings`);
      assert(typeof obj.handoffInfo.toolName === 'string', `Object ${obj.id} handoffInfo contains toolName`);
      assert(typeof obj.handoffInfo.instructions === 'string', `Object ${obj.id} handoffInfo contains instructions`);
    }
  }
}

// 3. Accounting & Invariant Calculations
console.log('\n--- Testing Accounting & Invariants ---');

function computeAccounting(group, keeperId, actions) {
  let immediate = 0;
  let conditional = 0;
  let retained = 0;
  let victimCount = 0;

  for (const obj of group.objects) {
    if (obj.id === keeperId || obj.excluded) continue;
    const act = actions[obj.id] || 'retain';
    const alloc = obj.allocatedSize;

    if (act === 'permanent_delete') {
      if (!obj.isCloud && !obj.isProtected && !obj.isStale) {
        immediate += alloc;
        victimCount++;
      }
    } else if (act === 'hardlink_immediate') {
      if (group.status === 'verified' && !obj.isCloud && !obj.isProtected && !obj.isStale) {
        immediate += alloc;
        victimCount++;
      }
    } else if (act === 'recycle') {
      if (!obj.isCloud && !obj.isProtected && !obj.isStale) {
        conditional += alloc;
        victimCount++;
      }
    } else if (act === 'hardlink_recoverable') {
      if (group.status === 'verified' && !obj.isCloud && !obj.isProtected && !obj.isStale) {
        retained += alloc;
        victimCount++;
      }
    }
  }

  return { immediate, conditional, retained, victimCount };
}

// Test A: Build artifacts consolidation (3 victim objects of 1.2 GB each -> 3.6 GB reclaim, NOT inflated by aliases)
const buildKeeper = buildGroup.objects[0].id;
const buildActions = {
  [buildGroup.objects[1].id]: 'hardlink_immediate',
  [buildGroup.objects[2].id]: 'hardlink_immediate',
  [buildGroup.objects[3].id]: 'hardlink_immediate'
};
const buildRes = computeAccounting(buildGroup, buildKeeper, buildActions);
assertEqual(buildRes.victimCount, 3, 'Distinct victim objects count is 3');
assertEqual(buildRes.immediate, 3600000000, 'Immediate reclaim is 3.6 GB (3 * 1.2 GB, distinct objects)');
assertEqual(buildRes.conditional, 0, 'Conditional reclaim is 0');
assertEqual(buildRes.retained, 0, 'Retained reclaim is 0');

// Test B: Vacation originals - Recoverable vs Immediate vs Recycle
const vacKeeper = vacationGroup.objects[0].id;
const verifiedVacationGroup = { ...vacationGroup, status: 'verified' };

// Recoverable Hard Link
const vacActionsRecoverable = {
  [vacationGroup.objects[1].id]: 'hardlink_recoverable'
};
const vacResRecoverable = computeAccounting(verifiedVacationGroup, vacKeeper, vacActionsRecoverable);
assertEqual(vacResRecoverable.immediate, 0, 'Recoverable Hard Link immediate reclaim is 0 B');
assertEqual(vacResRecoverable.retained, 4800000000, 'Recoverable Hard Link retained for recovery is 4.8 GB');

// Immediate Hard Link
const vacActionsImmediate = {
  [vacationGroup.objects[1].id]: 'hardlink_immediate'
};
const vacResImmediate = computeAccounting(verifiedVacationGroup, vacKeeper, vacActionsImmediate);
assertEqual(vacResImmediate.immediate, 4800000000, 'Immediate Hard Link immediate reclaim is 4.8 GB');
assertEqual(vacResImmediate.retained, 0, 'Immediate Hard Link retained is 0 B');

// Recycle Bin
const vacActionsRecycle = {
  [vacationGroup.objects[1].id]: 'recycle'
};
const vacResRecycle = computeAccounting(verifiedVacationGroup, vacKeeper, vacActionsRecycle);
assertEqual(vacResRecycle.immediate, 0, 'Recycle Bin immediate reclaim is 0 B');
assertEqual(vacResRecycle.conditional, 4800000000, 'Recycle Bin conditional future reclaim is 4.8 GB');

// Test C: OneDrive Online-only placeholder deletion does NOT reclaim local disk space
const oneDriveKeeper = oneDriveGroup.objects[0].id;
const oneDriveActions = {
  [oneDriveGroup.objects[1].id]: 'permanent_delete'
};
const oneDriveRes = computeAccounting(oneDriveGroup, oneDriveKeeper, oneDriveActions);
assertEqual(oneDriveRes.immediate, 0, 'Cloud placeholder deletion yields 0 B local disk reclaim');

// 4. Verification Stages & Safety Invariants
console.log('\n--- Testing Verification & Safety Invariants ---');
assertEqual(VERIFICATION_STAGES.length, 5, 'Verification workflow has 5 explicit stages');
assert(ACTION_TYPES.HARDLINK_RECOVERABLE.recoveryClass === 'retained', 'Hardlink Recoverable uses retained recovery class');
assert(ACTION_TYPES.HARDLINK_IMMEDIATE.recoveryClass === 'permanent', 'Hardlink Immediate uses permanent recovery class');
assert(ACTION_TYPES.PERMANENT_DELETE.recoveryClass === 'permanent', 'Permanent Delete uses permanent recovery class');
assert(ACTION_TYPES.RECYCLE.recoveryClass === 'conditional', 'Recycle uses conditional recovery class');
assert(ACTION_TYPES.RETAIN.recoveryClass === 'none', 'Retain uses none recovery class');

// 5. Test File ID Format
console.log('\n--- Testing Mock File IDs ---');
for (const group of INITIAL_GROUPS) {
  for (const obj of group.objects) {
    assert(typeof obj.fileId === 'string' && obj.fileId.startsWith('0x'), `Object ${obj.id} has valid hex File ID: ${obj.fileId}`);
  }
}

// 6. Test Final-Object Exclusion Invariant
console.log('\n--- Testing Final-Object Exclusion Invariant ---');
function canExcludeObject(group, objectId) {
  const activeObjects = group.objects.filter(o => !o.excluded);
  const target = group.objects.find(o => o.id === objectId);
  if (!target) return false;
  if (!target.excluded && activeObjects.length <= 1) {
    return false; // Cannot exclude final active object
  }
  return true;
}

const twoCopyGroup = JSON.parse(JSON.stringify(oneDriveGroup));
assert(canExcludeObject(twoCopyGroup, twoCopyGroup.objects[0].id), 'Can exclude first copy when 2 active copies exist');
twoCopyGroup.objects[0].excluded = true;
assert(!canExcludeObject(twoCopyGroup, twoCopyGroup.objects[1].id), 'CANNOT exclude the only remaining active copy');

console.log('\n=== All Prototype Invariant Checks Passed Successfully! ===');
