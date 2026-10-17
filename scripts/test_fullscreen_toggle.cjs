// Exercise FullscreenToggle helpers with mocked HarmonyOS window APIs.
// Usage: node scripts/test_fullscreen_toggle.cjs <typescript module>
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const util = require('node:util');
const vm = require('node:vm');
const ts = require(process.argv[2] || 'typescript');
const root = path.resolve(__dirname, '../entry/src/main/ets');

function load(deviceType) {
  const moduleObject = { exports: {} };
  const result = ts.transpileModule(fs.readFileSync(path.join(root, 'FullscreenToggle.ets'), 'utf8'), {
    compilerOptions: { target: ts.ScriptTarget.ES2020, module: ts.ModuleKind.CommonJS }
  });
  vm.runInNewContext(result.outputText, {
    module: moduleObject, exports: moduleObject.exports, console: { info() {} },
    require: name => {
      if (name === '@kit.ArkUI') return { window: {} };
      if (name === '@kit.BasicServicesKit') return { deviceInfo: { deviceType }, BusinessError: Error };
      throw new Error('unexpected ' + name);
    }
  });
  return moduleObject.exports;
}

function sameCalls(actual, expected) {
  assert.equal(JSON.stringify(actual), JSON.stringify(expected), util.inspect({ actual, expected }));
}

async function main() {
  assert.equal(load('tablet').fullscreenToggleAvailable(), true);
  assert.equal(load('phone').fullscreenToggleAvailable(), true);
  assert.equal(load('2in1').fullscreenToggleAvailable(), false, '2in1 hides the control to avoid WindowChrome freeform conflict');

  const calls = [];
  const win = {
    setWindowLayoutFullScreen: async (v) => { calls.push(['layout', v]); },
    setWindowSystemBarEnable: async (names) => { calls.push(['bars', [...names]]); }
  };
  const api = load('tablet');
  await api.enterFullscreen(win);
  sameCalls(calls, [['layout', true], ['bars', []]]);
  calls.length = 0;
  await api.exitFullscreen(win);
  sameCalls(calls, [['layout', false], ['bars', ['status', 'navigation']]]);
  calls.length = 0;
  await api.applyFullscreen(win, true);
  await api.applyFullscreen(win, false);
  sameCalls(calls, [
    ['layout', true], ['bars', []],
    ['layout', false], ['bars', ['status', 'navigation']]
  ]);
  console.log('Passed: availability by deviceType, enter/exit layout+system-bar sequence.');
}
main().catch(error => { console.error(error); process.exitCode = 1; });
