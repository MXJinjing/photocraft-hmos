// Exercise the ArkTS pen-hover encoder that Index wires to onHoverMove / onHover.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');
const ts = require(process.argv[2] || 'typescript');
const root = path.resolve(__dirname, '..');
const source = fs.readFileSync(path.join(root, 'entry/src/main/ets/input/PenHoverPreview.ets'), 'utf8');
const result = ts.transpileModule(source, {
  reportDiagnostics: true,
  compilerOptions: { target: ts.ScriptTarget.ES2020, module: ts.ModuleKind.CommonJS }
});
assert.deepEqual((result.diagnostics || []).filter(d => d.category === ts.DiagnosticCategory.Error), []);
const moduleShim = { exports: {} };
vm.runInNewContext(result.outputText, { module: moduleShim, exports: moduleShim.exports });
const {
  isFiniteHoverPoint,
  isStaleHoverEcho,
  penHoverLeaveMessage,
  penHoverMoveMessage
} = moduleShim.exports;

assert.equal(isFiniteHoverPoint(1, 2), true);
assert.equal(isFiniteHoverPoint(Number.NaN, 2), false);
assert.equal(isFiniteHoverPoint(1, Number.POSITIVE_INFINITY), false);

const move = JSON.parse(penHoverMoveMessage(120, 80));
assert.deepEqual(move, {
  kind: 'pointer',
  source: 'pen',
  action: 2,
  x: 120,
  y: 80,
  sample: { pressure: 0, tiltX: 0, tiltY: 0, eraser: false }
}, 'hover must be a move without press and zero pressure');

const leave = JSON.parse(penHoverLeaveMessage(120, 80));
assert.deepEqual(leave, { kind: 'hover_leave', x: 120, y: 80 });
assert.equal(isStaleHoverEcho(120, 80, 120, 80), true, 'leave-point echo is stale');
assert.equal(isStaleHoverEcho(300, 150, 120, 80), false, 'lift-point hover is fresh');
assert.equal(isStaleHoverEcho(120, 80, 120, 80), true, 'multi-frame echo stays stale');
console.log('pen hover preview encoder ok');
