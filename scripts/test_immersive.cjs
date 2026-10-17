// Exercise Immersive.ets with mocked HarmonyOS window / device APIs.
// Usage: node scripts/test_immersive.cjs <typescript module>
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const util = require('node:util');
const vm = require('node:vm');
const ts = require(process.argv[2] || 'typescript');
const root = path.resolve(__dirname, '../entry/src/main/ets');
const storage = new Map();
const AppStorage = { get: key => storage.get(key), setOrCreate: (key, value) => storage.set(key, value) };

function load(deviceType) {
  storage.clear();
  const moduleObject = { exports: {} };
  const result = ts.transpileModule(fs.readFileSync(path.join(root, 'Immersive.ets'), 'utf8'), {
    compilerOptions: { target: ts.ScriptTarget.ES2020, module: ts.ModuleKind.CommonJS }
  });
  vm.runInNewContext(result.outputText, {
    module: moduleObject, exports: moduleObject.exports, AppStorage,
    console: { info() {}, error() {} },
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
  assert.equal(load('tablet').immersiveWanted(), true);
  assert.equal(load('phone').immersiveWanted(), true);
  assert.equal(load('2in1').immersiveWanted(), false, '2in1 skips immersive to avoid WindowChrome conflict');

  const skipped = load('2in1');
  const skipCalls = [];
  const skipWin = {
    setWindowLayoutFullScreen: async (v) => { skipCalls.push(['layout', v]); },
    setWindowSystemBarEnable: async (names) => { skipCalls.push(['bars', [...names]]); }
  };
  await skipped.enableImmersiveFullscreen(skipWin);
  sameCalls(skipCalls, []);
  assert.equal(storage.get('photocraftImmersive'), false);

  const calls = [];
  const win = {
    setWindowLayoutFullScreen: async (v) => { calls.push(['layout', v]); },
    setWindowSystemBarEnable: async (names) => { calls.push(['bars', [...names]]); }
  };
  const api = load('tablet');
  await api.enableImmersiveFullscreen(win);
  sameCalls(calls, [['layout', true], ['bars', []]]);
  assert.equal(storage.get('photocraftImmersive'), true);

  calls.length = 0;
  const failing = {
    setWindowLayoutFullScreen: async () => { throw Object.assign(new Error('denied'), { code: 1300001 }); },
    setWindowSystemBarEnable: async () => { calls.push(['bars']); }
  };
  await load('phone').enableImmersiveFullscreen(failing);
  sameCalls(calls, []);
  assert.equal(storage.get('photocraftImmersive'), false, 'failure leaves immersive off');

  console.log('Passed: availability by deviceType, enter layout+hide bars, 2in1 skip, failure path.');
}
main().catch(error => { console.error(error); process.exitCode = 1; });
