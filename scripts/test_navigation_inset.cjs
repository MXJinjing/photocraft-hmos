// Safe-inset watcher and immersive-layout wiring with mocked HarmonyOS window services.
// Usage: node scripts/test_navigation_inset.cjs <typescript module>
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');
const ts = require(process.argv[2] || 'typescript');

const AvoidAreaType = { TYPE_SYSTEM: 0, TYPE_CUTOUT: 1, TYPE_NAVIGATION_INDICATOR: 4 };
const ets = name => fs.readFileSync(path.resolve(__dirname, '../entry/src/main/ets', name), 'utf8');
function compile(source, context) {
  const module = { exports: {} };
  const result = ts.transpileModule(source, { compilerOptions: { target: ts.ScriptTarget.ES2020, module: ts.ModuleKind.CommonJS } });
  vm.runInNewContext(result.outputText, { module, exports: module.exports, console: { info() {}, error() {}, warn() {} }, ...context });
  return module.exports;
}

function scenario(options = {}) {
  const listeners = new Map();
  const timers = new Map();
  let serial = 0;
  const storage = new Map(options.storage ?? []);
  const state = { sysTop: 24, sysBottom: 0, cutoutTop: 0, navBottom: 28, height: 1600, fail: false };
  const win = {
    on: (key, callback) => { if (options.listenFails) throw new Error('on'); listeners.set(key, callback); },
    off: key => listeners.delete(key),
    getWindowAvoidArea: type => {
      if (state.fail) throw new Error('transient');
      if (type === AvoidAreaType.TYPE_SYSTEM) {
        return { topRect: { height: state.sysTop }, bottomRect: { height: state.sysBottom } };
      }
      if (type === AvoidAreaType.TYPE_CUTOUT) {
        return { topRect: { height: state.cutoutTop }, bottomRect: { height: 0 } };
      }
      return { topRect: { height: 0 }, bottomRect: { height: state.navBottom } };
    },
    getWindowProperties: () => ({ windowRect: { height: state.height } }),
    setWindowLayoutFullScreen: () => Promise.resolve()
  };
  const immersive = compile(ets('Immersive.ets'), {
    AppStorage: { setOrCreate: (k, v) => storage.set(k, v), get: k => storage.get(k) },
    require: name => name === '@kit.ArkUI'
      ? { window: { AvoidAreaType } }
      : { BusinessError: Error, deviceInfo: { deviceType: options.deviceType ?? 'tablet' } }
  });
  const mod = compile(ets('NavigationInset.ets'), {
    AppStorage: { setOrCreate: (k, v) => storage.set(k, v), get: k => storage.get(k) },
    setTimeout: callback => { timers.set(++serial, callback); return serial; },
    clearTimeout: id => timers.delete(id),
    require: name => {
      if (name === '@kit.ArkUI') return { window: { AvoidAreaType } };
      if (name === '@kit.BasicServicesKit') return { BusinessError: Error };
      if (name === './Immersive') return immersive;
      throw new Error(name);
    }
  });
  const published = [];
  const reports = [];
  const inset = new mod.NavigationIndicatorInset(i => published.push({ top: i.top, bottom: i.bottom }), t => reports.push(t));
  const flush = () => { for (const callback of [...timers.values()]) callback(); timers.clear(); };
  return { inset, win, state, listeners, published, reports, timers, flush, mod, immersive, storage };
}

// Pure rule: top is always the full status-bar height; bottom subtracts the layout gap.
{
  const { mod } = scenario();
  const f = (...a) => { const r = mod.insetsFor(...a); return [r.top, r.bottom]; };
  assert.deepEqual(f(24, 28, 1600, 0, 1600), [24, 28], 'full-window layout');
  // A non-zero contentTop must NOT cancel the status-bar inset (the bug that left the menu
  // under the system icons while the bottom strip still worked).
  assert.deepEqual(f(24, 28, 1600, 24, 1548), [24, 0], 'stale contentTop must not zero the top inset');
  assert.deepEqual(f(24, 28, 1600, 0, 1000), [24, 0], 'keyboard (RESIZE) shrank the page: nothing below');
  assert.deepEqual(f(24, 28, 1600, 0, 1590), [24, 18], 'partly avoided at the bottom');
  assert.deepEqual(f(0, 0, 1600, 0, 1600), [0, 0], 'floating / split windows report none');
  assert.deepEqual(f(0, 28, 0, 0, 1600), [0, 28], 'unknown window height assumes the surface reaches the edge');
}

// Immersive publishes the full top from the first frame; contentTop never cancels it.
{
  const s = scenario();
  s.inset.start(s.win);
  assert.deepEqual(s.published, [{ top: 0, bottom: 0 }], 'not immersive: no inset');
  s.inset.setImmersive(true);
  assert.deepEqual(s.published.at(-1), { top: 24, bottom: 28 });
  s.inset.setContentBounds(24, 1548);
  assert.deepEqual(s.published.at(-1), { top: 24, bottom: 0 }, 'keyboard-like shrink zeros only the bottom');
  s.inset.setContentBounds(0, 1600);
  assert.deepEqual(s.published.at(-1), { top: 24, bottom: 28 });
  assert.ok(s.reports.some(r => r.includes('sysTopUsed=24') && r.includes('insets=24/28')));
}

// Live TYPE_SYSTEM.topRect of 0 falls back to the pre-fullscreen cache and to TYPE_CUTOUT.
{
  const s = scenario();
  s.storage.set(s.immersive.STATUS_BAR_PX_KEY, 36);
  s.state.sysTop = 0; s.state.navBottom = 28;
  s.inset.start(s.win); s.inset.setImmersive(true); s.inset.setContentBounds(0, 1600);
  assert.deepEqual(s.published.at(-1), { top: 36, bottom: 28 }, 'cached status-bar height');
  assert.ok(s.reports.at(-1).includes('sysTopLive=0') && s.reports.at(-1).includes('sysTopUsed=36'));

  const cut = scenario();
  cut.state.sysTop = 0; cut.state.cutoutTop = 40; cut.state.navBottom = 28;
  cut.inset.start(cut.win); cut.inset.setImmersive(true); cut.inset.setContentBounds(0, 1600);
  assert.deepEqual(cut.published.at(-1), { top: 40, bottom: 28 }, 'TYPE_CUTOUT top');
}

// enableImmersiveLayout caches the status-bar height before fullscreen.
{
  (async () => {
    const storage = new Map();
    let afterFullscreenTop = 0;
    const immersive = compile(ets('Immersive.ets'), {
      AppStorage: { setOrCreate: (k, v) => storage.set(k, v), get: k => storage.get(k) },
      require: name => name === '@kit.ArkUI'
        ? { window: { AvoidAreaType } }
        : { BusinessError: Error, deviceInfo: { deviceType: 'tablet' } }
    });
    const win = {
      getWindowAvoidArea: type => ({
        topRect: { height: type === AvoidAreaType.TYPE_SYSTEM ? (afterFullscreenTop || 48) : 0 },
        bottomRect: { height: 0 }
      }),
      setWindowLayoutFullScreen: () => { afterFullscreenTop = 0; return Promise.resolve(); }
    };
    await immersive.enableImmersiveLayout(win);
    assert.equal(storage.get(immersive.IMMERSIVE_KEY), true);
    assert.equal(storage.get(immersive.STATUS_BAR_PX_KEY), 48, 'pre-fullscreen height kept when post-fullscreen is 0');

    const skip = compile(ets('Immersive.ets'), {
      AppStorage: { setOrCreate: (k, v) => storage.set(k, v), get: k => storage.get(k) },
      require: name => name === '@kit.ArkUI'
        ? { window: { AvoidAreaType } }
        : { BusinessError: Error, deviceInfo: { deviceType: '2in1' } }
    });
    await skip.enableImmersiveLayout(win);
    assert.equal(storage.get(skip.IMMERSIVE_KEY), false);

    const page = ets('pages/Index.ets');
    assert.doesNotMatch(page, /\.(expandSafeArea|ignoreLayoutSafeArea)\s*\(/);
    assert.match(page, /input\.kind = 'safe_inset'/);
    assert.match(page, /backgroundColor\(this\.chrome\)/);
    assert.match(page, /const INSET_DEBUG: boolean = true;/);
    assert.match(page, /setKeyboardAvoidMode\(KeyboardAvoidMode\.RESIZE\)/);
    const ability = ets('entryability/EntryAbility.ets');
    assert.ok(ability.indexOf('enableImmersiveLayout') < ability.indexOf('windowStage.loadContent'));
    console.log('navigation inset: ok');
  })().catch(e => { console.error(e); process.exit(1); });
}
