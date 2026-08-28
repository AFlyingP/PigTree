// PigTree Prototype Fixture & Workflow Verification
// Standalone dependency-free verification script for prototype data models and accounting

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

let errors = 0;
function assert(condition, message) {
  if (!condition) {
    console.error('FAIL:', message);
    errors++;
  }
}

console.log('Validating Mock Fixtures, Accounting & Domain Models...');

// 1. Presets validation
assert(PRESETS.length >= 4, 'Should have at least 4 presets');
for (const p of PRESETS) {
  assert(p.id && p.name && p.profile, `Preset ${p.id} must have id, name, profile`);
}

// 2. Volumes & Reconciliation
for (const v of MOCK_VOLUMES) {
  assert(v.capacityBytes > 0, `Volume ${v.id} must have positive capacity`);
  assert(v.freeBytes + v.usedBytes === v.capacityBytes, `Volume ${v.id} Capacity == Free + Used`);
  assert(v.accountedUniqueBytes + v.unattributedUsedBytes - v.overAccountedBytes === v.usedBytes, `Volume ${v.id} Used == Accounted + Unattributed - OverAccounted`);
}

const volC = MOCK_VOLUMES.find(v => v.id === 'vol_c');
assert(volC.accountedUniqueBytes === 368 * 1024 * 1024 * 1024, 'Whole-volume C Accounted Unique Allocation must be exactly 368 GB');
assert(volC.unattributedUsedBytes === 24 * 1024 * 1024 * 1024, 'Volume C Unattributed Used Space must be exactly 24 GB');

const volD = MOCK_VOLUMES.find(v => v.id === 'vol_d');
assert(volD && volD.capacityBytes === 1024 * 1024 * 1024 * 1024, 'Volume D ReFS must be 1 TB');
assert(volD.accountedUniqueBytes === 570 * 1024 * 1024 * 1024, 'Volume D Accounted Unique Allocation must be 570 GB');

// 3. Tree Recursion and Summary Remainder Row Validation
function validateNode(node, path = '') {
  assert(node.id, `Node at ${path} must have an ID`);
  assert(node.name, `Node ${node.id} must have a name`);
  assert(typeof node.uniqueAllocatedBytes === 'number', `Node ${node.id} must have numeric uniqueAllocatedBytes`);
  assert(typeof node.referencedAllocatedBytes === 'number', `Node ${node.id} must have numeric referencedAllocatedBytes`);
  
  if (node.objectId) {
    assert(MOCK_OBJECTS[node.objectId], `Node ${node.id} references missing object ${node.objectId}`);
  }
  
  if (node.children && node.children.length > 0) {
    let childUniqueSum = 0;
    let childRefSum = 0;
    let childEntrySum = 0;
    
    for (const child of node.children) {
      validateNode(child, `${path}/${node.name}`);
      childUniqueSum += (child.uniqueAllocatedBytes || 0);
      childRefSum += (child.referencedAllocatedBytes || 0);
      childEntrySum += (child.entryCount !== undefined ? child.entryCount : 1);
    }
    
    const mb = 1024 * 1024;
    const diffUniqueMb = Math.abs(node.uniqueAllocatedBytes - childUniqueSum) / mb;
    const diffRefMb = Math.abs(node.referencedAllocatedBytes - childRefSum) / mb;
    
    assert(diffUniqueMb < 1, `Node ${node.path || node.name} unique allocated (${node.uniqueAllocatedBytes}) does not match sum of children (${childUniqueSum}), diff: ${diffUniqueMb.toFixed(2)} MB`);
    assert(diffRefMb < 1, `Node ${node.path || node.name} referenced allocated (${node.referencedAllocatedBytes}) does not match sum of children (${childRefSum}), diff: ${diffRefMb.toFixed(2)} MB`);
    assert(node.entryCount === childEntrySum, `Node ${node.path || node.name} entryCount (${node.entryCount}) does not match sum of children (${childEntrySum})`);
  }
}

console.log('Validating MOCK_TREE_ROOT (C:\\)...');
validateNode(MOCK_TREE_ROOT);

console.log('Validating MOCK_TREE_ROOT_D (D:\\)...');
validateNode(MOCK_TREE_ROOT_D);

console.log('Validating MOCK_TREE_DOWNLOADS_HISTORICAL...');
validateNode(MOCK_TREE_DOWNLOADS_HISTORICAL);

// 4. Hardlink aliases validation
for (const [objId, aliases] of Object.entries(HARDLINK_ALIASES)) {
  assert(MOCK_OBJECTS[objId], `Hardlink alias references unknown object ${objId}`);
  assert(aliases.length >= 2, `Hardlink alias ${objId} must have at least 2 paths`);
}

// 5. Cloud placeholder validation
const zipObj = MOCK_OBJECTS['obj_onedrive_zip'];
assert(zipObj && zipObj.allocatedBytes === 0, 'OneDrive cloud placeholder must have 0 physical allocated bytes');
assert(zipObj.logicalBytes === 4.8 * 1024 * 1024 * 1024, 'OneDrive cloud placeholder must have 4.8 GB logical size');

console.log(`Validation completed with ${errors} error(s).`);
if (errors > 0) {
  process.exit(1);
} else {
  console.log('ALL PROTOTYPE FIXTURES AND ACCOUNTING EQUATIONS PASSED!');
}
