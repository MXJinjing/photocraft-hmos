const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');
const ts = require(process.argv[2] || 'typescript');
const storage = new Map();
const timers = new Map();
const listeners = new Map();
let serial = 0, status = 1, rect = { width: 120, height: 38 }, fail = false;
const win = {
  setWindowDecorHeight() {}, setWindowDecorVisible() {}, setWindowTitleMoveEnabled() {},
  on: (key, callback) => listeners.set(key, callback),
  off: key => listeners.delete(key),
  getWindowStatus: () => status,
  getTitleButtonRect: () => { if (fail) throw Error('transient'); return rect; }
};
const moduleObject = { exports: {} };
const compiled = ts.transpileModule(fs.readFileSync('entry/src/main/ets/WindowChrome.ets', 'utf8'), {
  compilerOptions: { target: ts.ScriptTarget.ES2020, module: ts.ModuleKind.CommonJS }
});
vm.runInNewContext(compiled.outputText, {
  module: moduleObject, exports: moduleObject.exports, console: { info() {} },
  AppStorage: { setOrCreate: (key, value) => storage.set(key, value) },
  setTimeout: callback => { timers.set(++serial, callback); return serial; },
  clearTimeout: id => timers.delete(id),
  require: name => name === '@kit.ArkUI' ? { window: { WindowStatusType: { FLOATING: 1, MAXIMIZE: 2 } } } : { deviceInfo: { deviceType: '2in1' } }
});
const chrome = new moduleObject.exports.WindowChrome(win);
chrome.start();
assert.equal(storage.get('photocraftTitleRight'), 128);
rect = { width: 0, height: 0 };
listeners.get('windowStatusChange')();
assert.equal(storage.get('photocraftTitleRight'), 128, 'transition zero retains valid geometry');
fail = true;
chrome.refresh();
assert.equal(storage.get('photocraftTitleRight'), 128, 'query failure retains inset');
fail = false;
rect = { width: 150, height: 38 };
for (const callback of [...timers.values()]) callback();
assert.equal(storage.get('photocraftTitleRight'), 158, 'delayed geometry is recovered without another event');
status = 3;
listeners.get('windowSizeChange')();
assert.equal(storage.get('photocraftTitleRight'), 158, 'visible buttons override lagging status');
rect = { width: 0, height: 0 };
chrome.refresh();
assert.equal(storage.get('photocraftTitleRight'), 0, 'non-floating window with no buttons clears inset');
chrome.stop();
assert.equal(timers.size, 0);
assert.equal(listeners.size, 0);
console.log('Passed: title geometry transitions, query failures, delayed recovery, size changes and cleanup.');
