// Execute the ArkTS cursor adapter against mocked HarmonyOS cursor services.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');
const ts = require(process.argv[2] || 'typescript');
const calls = [];
let failStyle = false;
const styles = new Proxy({}, { get: (_target, name) => name });
const pointer = { PointerStyle: styles, setPointerVisibleSync: visible => calls.push(['visible', visible]) };
const context = { getCursorController: () => ({
  setCursor: style => { if (failStyle) throw { message: 'unsupported' }; calls.push(['style', style]); },
  restoreDefault: () => calls.push(['default'])
}) };
const source = fs.readFileSync('entry/src/main/ets/input/NativeCursorAdapter.ets', 'utf8');
const result = ts.transpileModule(source, { reportDiagnostics: true, compilerOptions: { target: ts.ScriptTarget.ES2020, module: ts.ModuleKind.CommonJS } });
assert.deepEqual((result.diagnostics || []).filter(d => d.category === ts.DiagnosticCategory.Error), []);
const moduleShim = { exports: {} };
vm.runInNewContext(result.outputText, {
  module: moduleShim, exports: moduleShim.exports, console: { error() {} },
  require: name => {
    if (name === '@kit.InputKit') return { pointer };
    if (name === '@kit.ArkUI' || name === '@kit.BasicServicesKit') return {};
    throw new Error(name);
  }
});
const { NativeCursorAdapter, restorePointerVisibility } = moduleShim.exports;
const adapter = new NativeCursorAdapter(context);
adapter.update('None');
assert.deepEqual(calls.splice(0), [['visible', false]], 'egui tip must not show an OS arrow');
for (const [icon, expected] of [
  ['Crosshair', 'CROSS'], ['Text', 'TEXT_CURSOR'], ['Grab', 'HAND_OPEN'], ['Grabbing', 'HAND_GRABBING'],
  ['Move', 'MOVE'], ['ResizeHorizontal', 'RESIZE_LEFT_RIGHT'], ['ResizeVertical', 'RESIZE_UP_DOWN'],
  ['ResizeNeSw', 'NORTH_EAST_SOUTH_WEST'], ['ResizeNwSe', 'NORTH_WEST_SOUTH_EAST'], ['ZoomIn', 'ZOOM_IN']
]) {
  adapter.update(icon);
  assert.deepEqual(calls.splice(0), [['style', expected], ['visible', true]], icon);
}
adapter.update('Default');
assert.deepEqual(calls.splice(0), [['default'], ['visible', true]], 'leaving canvas restores default');
adapter.update('None'); adapter.reset();
assert.deepEqual(calls.splice(0), [['visible', false], ['visible', true], ['default']], 'blur/disposal restores cursor');
restorePointerVisibility();
assert.deepEqual(calls.splice(0), [['visible', true]], 'background must never leave the system pointer hidden');
failStyle = true;
adapter.update('Crosshair');
assert.deepEqual(calls.splice(0), [['visible', true]], 'failed style restores visibility');
console.log('Passed: egui tip hides arrow, tool cursor styles, leave/blur/background restoration and SDK failure fallback.');
