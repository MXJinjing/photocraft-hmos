// Exercise real window helpers and lifecycle ordering with mocked HarmonyOS services.
// Usage: node scripts/test_system_bars.cjs <typescript module>
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');
const ts = require(process.argv[2] || 'typescript');
const root = path.resolve(__dirname, '../entry/src/main/ets');
const storage = new Map();
const AppStorage = { get: key => storage.get(key), setOrCreate: (key, value) => storage.set(key, value) };
function load(name, dependencies) {
  const module = { exports: {} };
  const result = ts.transpileModule(fs.readFileSync(path.join(root, name + '.ets'), 'utf8'), {
    compilerOptions: { target: ts.ScriptTarget.ES2020, module: ts.ModuleKind.CommonJS }
  });
  vm.runInNewContext(result.outputText, {
    module, exports: module.exports, AppStorage, console,
    require: name => { assert.ok(name in dependencies, name); return dependencies[name]; }
  });
  return module.exports;
}
const painted = [];
const immersiveCalls = [];
const win = {
  setWindowBackgroundColor: color => painted.push(['background', color]),
  setDecorButtonStyle: style => painted.push(['decor', style]),
  setWindowSystemBarProperties: props => { painted.push(['bars', props]); return Promise.resolve(); },
  setWindowLayoutFullScreen: async (v) => { immersiveCalls.push(['layout', v]); },
  setWindowSystemBarEnable: async (names) => { immersiveCalls.push(['bars', [...names]]); }
};
const bars = load('SystemBars', {
  '@kit.ArkUI': { window: { getLastWindow: () => Promise.resolve(win) } },
  '@kit.AbilityKit': {
    ConfigurationConstant: { ColorMode: { COLOR_MODE_NOT_SET: -1, COLOR_MODE_DARK: 0, COLOR_MODE_LIGHT: 1 } }
  },
  '@kit.BasicServicesKit': {}
});
for (const [input, expected] of [['#abc', '#FFAABBCC'], ['#f6f6f8', '#FFF6F6F8'], ['#FF141415', '#FF141415'], ['red', ''], ['#12345g', '']]) {
  assert.equal(bars.normalizeChrome(input), expected);
}
for (const color of ['#141415', '#F6F6F8']) {
  const before = painted.length;
  bars.paintWindow(win, color);
  assert.equal(painted.at(-1)[0], 'background');
  assert.equal(painted.at(-1)[1], bars.normalizeChrome(color));
  assert.equal(painted.at(-2)[0], 'decor');
  assert.ok(!painted.slice(before).some(entry => entry[0] === 'bars'), 'status/navigation dyeing must not run');
}
const count = painted.length;
bars.paintWindow(win, '#invalid');
assert.equal(painted.length, count);
const ConfigurationConstant = {
  ColorMode: { COLOR_MODE_NOT_SET: -1, COLOR_MODE_DARK: 0, COLOR_MODE_LIGHT: 1 }
};
const systemTheme = load('SystemTheme', {
  '@kit.AbilityKit': { ConfigurationConstant }
});
const immersive = load('Immersive', {
  '@kit.ArkUI': { window: {} },
  '@kit.BasicServicesKit': { deviceInfo: { deviceType: 'tablet' }, BusinessError: Error }
});
const { default: EntryAbility } = load('entryability/EntryAbility', {
  'libphotocraft.so': { default: { active() {}, input() {} } },
  '@kit.AbilityKit': { UIAbility: class {}, AbilityConstant: {}, ConfigurationConstant },
  '@kit.ArkUI': { window: { getLastWindow: () => Promise.resolve(win) } },
  '@kit.BasicServicesKit': {},
  '../SystemBars': bars,
  '../SystemTheme': systemTheme,
  '../WindowChrome': { WindowChrome: class { start() {} stop() {} refresh() {} } },
  '../Immersive': immersive,
  '../input/NativeCursorAdapter': { restorePointerVisibility() {} },
  '../openInbox': { acceptOpenWant() {} }, '../closeGuard': { resetCloseGuard() {}, requestWindowClose() {} }
});
async function flush(times = 8) {
  for (let i = 0; i < times; i++) await Promise.resolve();
}
async function waitFor(get, label) {
  for (let i = 0; i < 20; i++) {
    const value = get();
    if (value !== undefined && value !== null) return value;
    await Promise.resolve();
  }
  assert.fail(label);
}
async function main() {
  const app = new EntryAbility();
  app.context = { config: { colorMode: ConfigurationConstant.ColorMode.COLOR_MODE_DARK } };
  let loaded;
  const stage = { on() {}, getMainWindowSync: () => win, loadContent: (_page, cb) => { loaded = cb; } };
  app.onWindowStageCreate(stage);
  await waitFor(() => loaded, 'loadContent after immersive enable');
  assert.equal(storage.get('photocraftChrome'), bars.LOADING_CHROME);
  assert.ok(immersiveCalls.some(c => c[0] === 'layout' && c[1] === true), 'fullscreen before loadContent');
  assert.ok(immersiveCalls.some(c => c[0] === 'bars' && c[1].length === 0), 'system bars hidden');
  // The first native theme arrives before loadContent completes: it must not be overwritten.
  storage.set('photocraftChrome', '#FFF6F6F8');
  loaded({ code: 0 });
  assert.equal(painted.at(-1)[0], 'background');
  assert.equal(painted.at(-1)[1], '#FFF6F6F8');
  assert.ok(!painted.some(entry => entry[0] === 'bars'), 'no system-bar dyeing after load');
  loaded = undefined;
  app.onWindowStageCreate(stage);
  await waitFor(() => loaded, 'loadContent on stage recreation');
  assert.equal(storage.get('photocraftChrome'), '#FFF6F6F8', 'stage recreation retains the current theme');
  loaded({ code: 0 });
  const beforeFg = immersiveCalls.length;
  app.onForeground();
  await flush();
  assert.equal(painted.at(-1)[1], '#FFF6F6F8', 'foreground restores theme background');
  assert.ok(immersiveCalls.length > beforeFg, 'foreground re-asserts immersive');
  storage.set('photocraftChrome', '#FF141415');
  app.onForeground();
  await flush();
  assert.equal(painted.at(-1)[1], '#FF141415');
  console.log('Passed: color validation, background-only chrome, immersive before load, no bar dyeing, foreground.');
}
main().catch(error => { console.error(error); process.exitCode = 1; });
