// Exercise Fullscreen.ets (optional window full screen) with mocked HarmonyOS window APIs.
// Usage: node scripts/test_fullscreen.cjs <typescript module>
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');
const ts = require(process.argv[2] || 'typescript');
const root = path.resolve(__dirname, '../entry/src/main/ets');
const Status = { FULL_SCREEN: 1, MAXIMIZE: 2, FLOATING: 4, SPLIT_SCREEN: 5 };

function load(deviceType, sent) {
  const storage = new Map();
  const module = { exports: {} };
  const result = ts.transpileModule(fs.readFileSync(path.join(root, 'Fullscreen.ets'), 'utf8'), {
    reportDiagnostics: true, fileName: 'Fullscreen.ts',
    compilerOptions: { target: ts.ScriptTarget.ES2020, module: ts.ModuleKind.CommonJS }
  });
  assert.deepEqual((result.diagnostics || []).filter(d => d.category === ts.DiagnosticCategory.Error), []);
  vm.runInNewContext(result.outputText, {
    module, exports: module.exports, console: { info() {} },
    AppStorage: { get: k => storage.get(k), setOrCreate: (k, v) => storage.set(k, v) },
    require: name => {
      if (name === 'libphotocraft.so') return { default: { input: s => sent.push(JSON.parse(s)) } };
      if (name === '@kit.ArkUI') return { window: { WindowStatusType: Status } };
      if (name === '@kit.BasicServicesKit') return { deviceInfo: { deviceType }, BusinessError: Error };
      throw new Error('unexpected ' + name);
    }
  });
  return { api: module.exports, storage };
}

function fakeWindow(status = Status.FULL_SCREEN, calls = []) {
  const state = { status };
  const handlers = {};
  return {
    calls, handlers, state,
    getWindowStatus: () => state.status,
    setWindowLayoutFullScreen: async v => { calls.push(['layout', v]); },
    setWindowSystemBarEnable: async names => { calls.push(['bars', [...names]]); },
    on: (name, cb) => { handlers[name] = cb; },
    off: name => { delete handlers[name]; }
  };
}

async function main() {
  // Availability: everywhere but 2in1 (WindowChrome owns the freeform title strip there).
  assert.equal(load('tablet', []).api.fullscreenAvailable(), true);
  assert.equal(load('2in1', []).api.fullscreenAvailable(), false);

  // Default: off, nothing touched until requested.
  {
    const sent = [];
    const { api } = load('tablet', sent);
    assert.equal(api.fullscreenOn(), false);
    assert.equal(sent.length, 0);
  }

  // Enter, then leave: layout + bars sequence, state, report, and recolour only on exit.
  {
    const sent = []; let repaints = 0;
    const { api, storage } = load('tablet', sent);
    const win = fakeWindow();
    await api.requestFullscreen(win, true, () => repaints++);
    assert.deepEqual(win.calls, [['layout', true], ['bars', []]]);
    assert.equal(storage.get('photocraftFullscreen'), true);
    assert.equal(repaints, 0);
    assert.deepEqual(sent.at(-1), { kind: 'fullscreen_state', text: 'on' });
    win.calls.length = 0;
    await api.requestFullscreen(win, true, () => repaints++);
    assert.deepEqual(win.calls, [['layout', true], ['bars', []]], 'already on: re-apply immersion');
    assert.deepEqual(sent.at(-1), { kind: 'fullscreen_state', text: 'on' });
    win.calls.length = 0;
    await api.requestFullscreen(win, false, () => repaints++);
    assert.deepEqual(win.calls, [['layout', false], ['bars', ['status', 'navigation']]]);
    assert.equal(storage.get('photocraftFullscreen'), false);
    assert.equal(repaints, 1, 'theme colour is repainted on the restored bars');
    assert.deepEqual(sent.at(-1), { kind: 'fullscreen_state', text: 'off' });
  }

  // Refused for floating / split / freeform-maximized / 2in1: stays off and says so.
  for (const [type, status] of [
    ['tablet', Status.FLOATING],
    ['tablet', Status.SPLIT_SCREEN],
    ['tablet', Status.MAXIMIZE],
    ['2in1', Status.FULL_SCREEN]
  ]) {
    const sent = [];
    const { api, storage } = load(type, sent);
    const win = fakeWindow(status);
    await api.requestFullscreen(win, true, () => {});
    assert.deepEqual(win.calls, [], `${type}/${status}`);
    assert.equal(storage.get('photocraftFullscreen'), undefined, `${type}/${status} desire unset`);
    assert.deepEqual(sent.at(-1), { kind: 'fullscreen_state', text: 'off' });
  }

  // A failing window API leaves the state as it was and reports it.
  {
    const sent = [];
    const { api } = load('tablet', sent);
    const win = fakeWindow();
    win.setWindowLayoutFullScreen = async () => { throw Object.assign(new Error('boom'), { code: 1 }); };
    await api.requestFullscreen(win, true, () => {});
    assert.equal(api.fullscreenOn(), false);
    assert.deepEqual(sent.at(-1), { kind: 'fullscreen_state', text: 'off' });
  }

  // Free multi-window pause keeps desire; returning to FULL_SCREEN restores immersion.
  {
    const sent = []; let repaints = 0;
    const { api, storage } = load('tablet', sent);
    const win = fakeWindow();
    const sync = new api.FullscreenSync(win, () => repaints++);
    sync.start();
    assert.equal(typeof win.handlers.windowStatusChange, 'function');
    await api.requestFullscreen(win, true, () => {});
    win.handlers.windowStatusChange(Status.FULL_SCREEN);
    await new Promise(r => setImmediate(r));
    assert.equal(api.fullscreenOn(), true, 'still tablet full screen: stays on');
    win.calls.length = 0;
    sync.reassert();
    await new Promise(r => setImmediate(r));
    assert.deepEqual(win.calls, [['layout', true], ['bars', []]]);
    assert.deepEqual(sent.at(-1), { kind: 'fullscreen_state', text: 'on' });

    // Tablet → freeform maximized: pause bars/layout, keep desire.
    win.calls.length = 0;
    win.state.status = Status.MAXIMIZE;
    win.handlers.windowStatusChange(Status.MAXIMIZE);
    await new Promise(r => setImmediate(r));
    assert.equal(storage.get('photocraftFullscreen'), true, 'desire kept across freeform');
    assert.deepEqual(win.calls, [['layout', false], ['bars', ['status', 'navigation']]]);
    assert.equal(repaints, 1);
    assert.deepEqual(sent.at(-1), { kind: 'fullscreen_state', text: 'off' });

    // Freeform → tablet: re-hide bars (the reported bug).
    win.calls.length = 0;
    win.state.status = Status.FULL_SCREEN;
    win.handlers.windowStatusChange(Status.FULL_SCREEN);
    await new Promise(r => setImmediate(r));
    assert.equal(api.fullscreenOn(), true);
    assert.deepEqual(win.calls, [['layout', true], ['bars', []]], 'restore immersion on tablet return');
    assert.deepEqual(sent.at(-1), { kind: 'fullscreen_state', text: 'on' });

    // Floating pause likewise keeps desire.
    win.calls.length = 0;
    win.state.status = Status.FLOATING;
    win.handlers.windowStatusChange(Status.FLOATING);
    await new Promise(r => setImmediate(r));
    assert.equal(api.fullscreenOn(), true);
    assert.deepEqual(win.calls, [['layout', false], ['bars', ['status', 'navigation']]]);

    // reassert while paused in freeform: do not hide bars.
    win.calls.length = 0;
    sync.reassert();
    assert.deepEqual(win.calls, [], 'paused freeform: resume does not hide bars');
    assert.deepEqual(sent.at(-1), { kind: 'fullscreen_state', text: 'off' });

    // User exit clears desire.
    win.state.status = Status.FULL_SCREEN;
    win.calls.length = 0;
    await api.requestFullscreen(win, false, () => repaints++);
    assert.equal(api.fullscreenOn(), false);
    assert.deepEqual(win.calls, [['layout', false], ['bars', ['status', 'navigation']]]);

    win.calls.length = 0;
    sync.reassert();
    assert.deepEqual(win.calls, [], 'off by default path: resume does nothing');

    sync.stop();
    assert.equal(win.handlers.windowStatusChange, undefined);
  }

  console.log('Passed: availability, default off, enter/exit, refuse floating/split/maximize/2in1, failure, freeform pause+restore, resume.');
}
main().catch(error => { console.error(error); process.exitCode = 1; });
