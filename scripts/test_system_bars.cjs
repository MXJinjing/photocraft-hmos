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
const win = {
  setWindowBackgroundColor: color => painted.push(['background', color]),
  setWindowSystemBarProperties: props => { painted.push(['bars', props]); return Promise.resolve(); }
};
const bars = load('SystemBars', {
  '@kit.ArkUI': { window: { getLastWindow: () => Promise.resolve(win) } },
  '@kit.AbilityKit': {}, '@kit.BasicServicesKit': {}
});
for (const [input, expected] of [['#abc', '#FFAABBCC'], ['#f6f6f8', '#FFF6F6F8'], ['#FF141415', '#FF141415'], ['red', ''], ['#12345g', '']]) {
  assert.equal(bars.normalizeChrome(input), expected);
}
for (const [color, content] of [['#141415', '#FFFFFFFF'], ['#F6F6F8', '#FF1A1A1A']]) {
  bars.paintWindow(win, color);
  const props = painted.at(-1)[1];
  assert.equal(painted.at(-2)[1], bars.normalizeChrome(color));
  assert.equal(props.statusBarColor, bars.normalizeChrome(color));
  assert.equal(props.navigationBarColor, props.statusBarColor);
  assert.equal(props.statusBarContentColor, content);
  assert.equal(props.navigationBarContentColor, content);
}
const count = painted.length;
bars.paintWindow(win, '#invalid');
assert.equal(painted.length, count);
const { default: EntryAbility } = load('entryability/EntryAbility', {
  'libphotocraft.so': { default: { active() {} } },
  '@kit.AbilityKit': { UIAbility: class {} }, '@kit.ArkUI': {}, '@kit.BasicServicesKit': {},
  '../SystemBars': bars,
  '../input/NativeCursorAdapter': { restorePointerVisibility() {} },
      '../openInbox': { acceptOpenWant() {} }, '../closeGuard': { resetCloseGuard() {}, requestWindowClose() {} }
});
async function main() {
  const app = new EntryAbility();
  let loaded;
  const stage = { on() {}, getMainWindowSync: () => win, loadContent: (_page, cb) => { loaded = cb; } };
  app.onWindowStageCreate(stage);
  assert.equal(storage.get('photocraftChrome'), bars.LOADING_CHROME);
  // The first native theme arrives before loadContent completes: it must not be overwritten.
  storage.set('photocraftChrome', '#FFF6F6F8');
  loaded({ code: 0 });
  assert.equal(painted.at(-1)[1].statusBarColor, '#FFF6F6F8');
  app.onWindowStageCreate(stage);
  assert.equal(storage.get('photocraftChrome'), '#FFF6F6F8', 'stage recreation retains the current theme');
  loaded({ code: 0 });
  app.onForeground();
  await Promise.resolve();
  assert.equal(painted.at(-1)[1].statusBarColor, '#FFF6F6F8', 'foreground restores theme and dark icons');
  storage.set('photocraftChrome', '#FF141415');
  app.onForeground();
  await Promise.resolve();
  assert.equal(painted.at(-1)[1].statusBarContentColor, '#FFFFFFFF');
  console.log('Passed: color validation, both system bars, light/dark icons, startup race, stage recreation, foreground.');
}
main().catch(error => { console.error(error); process.exitCode = 1; });
