// Navigation-indicator geometry watcher with mocked HarmonyOS window services.
// Usage: node scripts/test_navigation_inset.cjs <typescript module>
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');
const ts = require(process.argv[2] || 'typescript');

const AvoidAreaType = { TYPE_SYSTEM: 0, TYPE_NAVIGATION_INDICATOR: 4 };

function scenario(options = {}) {
  const listeners = new Map();
  const timers = new Map();
  let serial = 0;
  const state = { indicator: options.indicator ?? 60, height: options.height ?? 2000, fail: false };
  const win = {
    on: (key, callback) => { if (options.listenFails) throw new Error('on'); listeners.set(key, callback); },
    off: key => listeners.delete(key),
    getWindowAvoidArea: type => {
      assert.equal(type, AvoidAreaType.TYPE_NAVIGATION_INDICATOR);
      if (state.fail) throw new Error('transient');
      return { bottomRect: { height: state.indicator } };
    },
    getWindowProperties: () => ({ windowRect: { height: state.height } })
  };
  const module = { exports: {} };
  const result = ts.transpileModule(fs.readFileSync(path.resolve(__dirname, '../entry/src/main/ets/NavigationInset.ets'), 'utf8'), {
    compilerOptions: { target: ts.ScriptTarget.ES2020, module: ts.ModuleKind.CommonJS }
  });
  vm.runInNewContext(result.outputText, {
    module, exports: module.exports, console: { info() {} },
    setTimeout: callback => { timers.set(++serial, callback); return serial; },
    clearTimeout: id => timers.delete(id),
    require: name => name === '@kit.ArkUI'
      ? { window: { AvoidAreaType } }
      : { BusinessError: Error }
  });
  const published = [];
  const inset = new module.exports.NavigationIndicatorInset(g => published.push({ ...g }));
  const flush = () => { for (const callback of [...timers.values()]) callback(); timers.clear(); };
  return { inset, win, state, listeners, published, timers, flush };
}

// Reports pixels, rounds, and publishes only changes.
{
  const s = scenario();
  s.inset.setContentTop(0);
  assert.equal(s.published.length, 0, 'nothing before the window is known');
  s.inset.start(s.win);
  assert.deepEqual(s.published, [{ indicator: 60, windowHeight: 2000, contentTop: 0 }]);
  s.inset.setContentTop(0);
  s.inset.setContentTop(100.4);
  assert.deepEqual(s.published.at(-1), { indicator: 60, windowHeight: 2000, contentTop: 100 });
  assert.equal(s.published.length, 2, 'unchanged geometry is not re-sent');
}

// avoidAreaChange for other types is ignored; indicator changes and size changes are followed.
{
  const s = scenario();
  s.inset.start(s.win);
  const before = s.published.length;
  s.state.indicator = 0;
  s.listeners.get('avoidAreaChange')({ type: AvoidAreaType.TYPE_SYSTEM });
  assert.equal(s.published.length, before, 'status-bar changes do not matter');
  s.listeners.get('avoidAreaChange')({ type: AvoidAreaType.TYPE_NAVIGATION_INDICATOR });
  assert.deepEqual(s.published.at(-1), { indicator: 0, windowHeight: 0, contentTop: 0 }, 'floating / split: no indicator');
  s.state.indicator = 48; s.state.height = 1500;
  s.listeners.get('windowSizeChange')({ width: 1000, height: 1500 });
  assert.deepEqual(s.published.at(-1), { indicator: 48, windowHeight: 1500, contentTop: 0 });
  // A late transaction is picked up by the settle timer without another event.
  s.state.height = 1600;
  s.flush();
  assert.equal(s.published.at(-1).windowHeight, 1600);
}

// Failures keep the last value; resend replays it; stop releases listeners and timers.
{
  const s = scenario();
  s.inset.start(s.win);
  const count = s.published.length;
  s.state.fail = true;
  s.listeners.get('windowSizeChange')({});
  assert.equal(s.published.length, count, 'a transient query failure does not erase the inset');
  s.state.fail = false;
  s.inset.resend();
  assert.equal(s.published.length, count + 1, 'native lost its state: replayed');
  s.inset.stop();
  assert.equal(s.listeners.size, 0);
  assert.equal(s.timers.size, 0);
  s.inset.setContentTop(10);
  assert.equal(s.published.length, count + 1, 'stopped watcher is silent');
}

// Listener registration failure is survivable.
{
  const s = scenario({ listenFails: true });
  s.inset.start(s.win);
  assert.equal(s.published.length, 1);
}
console.log('navigation inset: ok');
