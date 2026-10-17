// Safe-inset watcher and immersive-layout wiring with mocked HarmonyOS window services.
// Usage: node scripts/test_navigation_inset.cjs <typescript module>
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');
const ts = require(process.argv[2] || 'typescript');

const AvoidAreaType = { TYPE_SYSTEM: 0, TYPE_NAVIGATION_INDICATOR: 4 };
const ets = name => fs.readFileSync(path.resolve(__dirname, '../entry/src/main/ets', name), 'utf8');
function compile(source, context) {
  const module = { exports: {} };
  const result = ts.transpileModule(source, { compilerOptions: { target: ts.ScriptTarget.ES2020, module: ts.ModuleKind.CommonJS } });
  vm.runInNewContext(result.outputText, { module, exports: module.exports, console: { info() {}, error() {} }, ...context });
  return module.exports;
}

function scenario(options = {}) {
  const listeners = new Map();
  const timers = new Map();
  let serial = 0;
  const state = { sysTop: 24, sysBottom: 0, navBottom: 28, height: 1600, fail: false, noHeight: false };
  const win = {
    on: (key, callback) => { if (options.listenFails) throw new Error('on'); listeners.set(key, callback); },
    off: key => listeners.delete(key),
    getWindowAvoidArea: type => {
      if (state.fail) throw new Error('transient');
      return type === AvoidAreaType.TYPE_SYSTEM
        ? { topRect: { height: state.sysTop }, bottomRect: { height: state.sysBottom } }
        : { topRect: { height: 0 }, bottomRect: { height: state.navBottom } };
    },
    getWindowProperties: () => { if (state.noHeight) throw new Error('x'); return { windowRect: { height: state.height } }; }
  };
  const mod = compile(ets('NavigationInset.ets'), {
    setTimeout: callback => { timers.set(++serial, callback); return serial; },
    clearTimeout: id => timers.delete(id),
    require: name => name === '@kit.ArkUI' ? { window: { AvoidAreaType } } : { BusinessError: Error }
  });
  const published = [];
  const reports = [];
  const inset = new mod.NavigationIndicatorInset(i => published.push({ top: i.top, bottom: i.bottom }), t => reports.push(t));
  const flush = () => { for (const callback of [...timers.values()]) callback(); timers.clear(); };
  return { inset, win, state, listeners, published, reports, timers, flush, mod };
}

// Pure rule: inset = system area - what the layout already avoids.
{
  const { mod } = scenario();
  const f = (...a) => { const r = mod.insetsFor(...a); return [r.top, r.bottom]; };
  assert.deepEqual(f(24, 28, 1600, 0, 1600), [24, 28], 'full-window layout: both strips covered');
  assert.deepEqual(f(24, 28, 1600, 24, 1548), [0, 0], 'page already laid out inside the safe area');
  assert.deepEqual(f(24, 28, 1600, 0, 1000), [24, 0], 'keyboard (RESIZE) shrank the page: nothing below');
  assert.deepEqual(f(24, 28, 1600, 0, 1590), [24, 18], 'partly avoided');
  assert.deepEqual(f(0, 0, 1600, 0, 1600), [0, 0], 'floating / split windows report none');
  assert.deepEqual(f(0, 28, 0, 0, 1600), [0, 28], 'unknown window height assumes the surface reaches the edge');
}

// Waits for layout, honours the immersive flag, publishes only changes.
{
  const s = scenario();
  s.inset.start(s.win);
  assert.deepEqual(s.published, [{ top: 0, bottom: 0 }], 'not immersive: no inset');
  s.inset.setImmersive(true);
  assert.deepEqual(s.published.at(-1), { top: 24, bottom: 28 }, 'before the first layout the page is assumed to fill the window');
  s.inset.setContentBounds(0, 1600);
  assert.deepEqual(s.published.at(-1), { top: 24, bottom: 28 });
  const n = s.published.length;
  s.inset.setContentBounds(0, 1600);
  assert.equal(s.published.length, n, 'unchanged insets are not re-sent');
  assert.ok(s.reports.some(r => r.includes('navBottom=28') && r.includes('insets=24/28')), 'diagnostic line carries the raw numbers');
}

// The page may lay out before the window query resolves (start() runs later): bounds must survive.
{
  const s = scenario();
  s.inset.setImmersive(true);
  s.inset.setContentBounds(0, 1600);
  s.inset.start(s.win);
  assert.deepEqual(s.published.at(-1), { top: 24, bottom: 28 });
  assert.ok(s.reports.at(-1).includes('laidOut=true'), 'bounds were not wiped by start()');
  // A status bar that the system publishes late is still picked up without any event.
  const late = scenario();
  late.state.sysTop = 0; late.state.navBottom = 0;
  late.inset.setImmersive(true); late.inset.setContentBounds(0, 1600); late.inset.start(late.win);
  assert.deepEqual(late.published.at(-1), { top: 0, bottom: 0 });
  late.state.sysTop = 24; late.state.navBottom = 28;
  late.flush();
  assert.deepEqual(late.published.at(-1), { top: 24, bottom: 28 });
}

// Navigation bar reported under SYSTEM instead of NAVIGATION_INDICATOR still counts.
{
  const s = scenario();
  s.state.navBottom = 0; s.state.sysBottom = 40;
  s.inset.start(s.win); s.inset.setImmersive(true); s.inset.setContentBounds(0, 1600);
  assert.deepEqual(s.published.at(-1), { top: 24, bottom: 40 });
}

// Avoid-area and size events refresh; keyboard shrink and recovery are followed.
{
  const s = scenario();
  s.inset.start(s.win); s.inset.setImmersive(true); s.inset.setContentBounds(0, 1600);
  s.inset.setContentBounds(0, 900);
  assert.deepEqual(s.published.at(-1), { top: 24, bottom: 0 });
  s.inset.setContentBounds(0, 1600);
  assert.deepEqual(s.published.at(-1), { top: 24, bottom: 28 });
  s.state.sysTop = 0; s.state.navBottom = 0;
  s.listeners.get('avoidAreaChange')({ type: AvoidAreaType.TYPE_NAVIGATION_INDICATOR });
  assert.deepEqual(s.published.at(-1), { top: 0, bottom: 0 }, 'floating / split');
  s.state.sysTop = 20; s.state.navBottom = 30; s.state.height = 1700;
  s.inset.setContentBounds(0, 1700);
  s.listeners.get('windowSizeChange')({});
  assert.deepEqual(s.published.at(-1), { top: 20, bottom: 30 });
  s.state.navBottom = 50;
  s.flush();
  assert.equal(s.published.at(-1).bottom, 50, 'settle timer picks up late transactions');
}

// Failures keep the last value; resend replays; stop releases everything.
{
  const s = scenario();
  s.inset.start(s.win); s.inset.setImmersive(true); s.inset.setContentBounds(0, 1600);
  const n = s.published.length;
  s.state.fail = true;
  s.listeners.get('windowSizeChange')({});
  assert.equal(s.published.length, n, 'a transient query failure keeps the last insets');
  s.state.fail = false;
  s.inset.resend();
  assert.equal(s.published.length, n + 1);
  s.inset.stop();
  assert.equal(s.listeners.size, 0);
  assert.equal(s.timers.size, 0);
  s.inset.setContentBounds(0, 1000);
  assert.equal(s.published.length, n + 1, 'stopped watcher is silent');
}
{
  const s = scenario({ listenFails: true });
  s.inset.start(s.win);
  assert.equal(s.published.length, 1, 'listener registration failure is survivable');
}

// Immersive helper: enables on tablets, skipped on 2-in-1, never rejects.
{
  const run = async (deviceType, outcome) => {
    const storage = new Map();
    const calls = [];
    const mod = compile(ets('Immersive.ets'), {
      AppStorage: { setOrCreate: (k, v) => storage.set(k, v) },
      require: name => name === '@kit.ArkUI' ? { window: {} } : { BusinessError: Error, deviceInfo: { deviceType } }
    });
    const win = { setWindowLayoutFullScreen: v => { calls.push(v); return outcome === 'ok' ? Promise.resolve() : Promise.reject(Object.assign(new Error('x'), { code: 1300002 })); } };
    await mod.enableImmersiveLayout(win);
    return { flag: storage.get(mod.IMMERSIVE_KEY), calls };
  };
  (async () => {
    assert.deepEqual(await run('tablet', 'ok'), { flag: true, calls: [true] });
    assert.deepEqual(await run('2in1', 'ok'), { flag: false, calls: [] });
    assert.deepEqual(await run('tablet', 'fail'), { flag: false, calls: [true] });
    console.log('navigation inset: ok');
  })().catch(e => { console.error(e); process.exit(1); });
}

// Source wiring: layout fullscreen before loadContent; Index no longer relies on safe-area expansion.
{
  const page = ets('pages/Index.ets');
  assert.doesNotMatch(page, /\.(expandSafeArea|ignoreLayoutSafeArea)\s*\(/);
  assert.match(page, /input\.kind = 'safe_inset'/);
  assert.match(page, /backgroundColor\(this\.chrome\)/);
  assert.match(page, /const INSET_DEBUG: boolean = false;/);
  assert.match(page, /setKeyboardAvoidMode\(KeyboardAvoidMode\.RESIZE\)/);
  const ability = ets('entryability/EntryAbility.ets');
  assert.ok(ability.indexOf('enableImmersiveLayout') < ability.indexOf("windowStage.loadContent"), 'immersive is applied before the page loads' );
  assert.match(ability, /enableImmersiveLayout\(windowStage\.getMainWindowSync\(\)\)\.then\(load\)/);
}
