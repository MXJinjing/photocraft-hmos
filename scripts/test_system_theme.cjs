// Exercise system colour-scheme mapping and EntryAbility configuration updates.
// Usage: node scripts/test_system_theme.cjs <typescript module>
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');
const ts = require(process.argv[2] || 'typescript');
const root = path.resolve(__dirname, '../entry/src/main/ets');
const storage = new Map();
const AppStorage = { get: key => storage.get(key), setOrCreate: (key, value) => storage.set(key, value) };
const ConfigurationConstant = {
  ColorMode: { COLOR_MODE_NOT_SET: -1, COLOR_MODE_DARK: 0, COLOR_MODE_LIGHT: 1 }
};
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
const theme = load('SystemTheme', {
  '@kit.AbilityKit': { ConfigurationConstant }
});
assert.equal(theme.systemThemeFromColorMode(0), 'dark');
assert.equal(theme.systemThemeFromColorMode(1), 'light');
assert.equal(theme.systemThemeFromColorMode(-1), '');
assert.equal(theme.systemThemeFromColorMode(undefined), '');
assert.equal(theme.SYSTEM_THEME_DARK, 'dark');
assert.equal(theme.SYSTEM_THEME_LIGHT, 'light');
theme.publishSystemTheme('light');
assert.equal(storage.get('photocraftSystemTheme'), 'light');

const inputs = [];
const native = { active() {}, input(json) { inputs.push(JSON.parse(json)); } };
const bars = {
  LOADING_CHROME: '#FF262626',
  applySystemBars() {},
  paintWindow() {}
};
const { default: EntryAbility } = load('entryability/EntryAbility', {
  'libphotocraft.so': { default: native },
  '@kit.AbilityKit': {
    UIAbility: class {},
    AbilityConstant: {},
    ConfigurationConstant
  },
  '@kit.ArkUI': {},
  '@kit.BasicServicesKit': {},
  '../SystemBars': bars,
  '../SystemTheme': theme,
  '../WindowChrome': { WindowChrome: class { start() {} stop() {} refresh() {} } },
  '../input/NativeCursorAdapter': { restorePointerVisibility() {} },
  '../openInbox': { acceptOpenWant() {} },
  '../closeGuard': { resetCloseGuard() {}, requestWindowClose() {} }
});

const app = new EntryAbility();
app.context = { config: { colorMode: ConfigurationConstant.ColorMode.COLOR_MODE_DARK } };
app.onCreate({}, {});
assert.equal(storage.get('photocraftSystemTheme'), 'dark');
assert.equal(inputs.at(-1).kind, 'system_theme');
assert.equal(inputs.at(-1).text, 'dark');

app.onConfigurationUpdate({ colorMode: ConfigurationConstant.ColorMode.COLOR_MODE_LIGHT });
assert.equal(storage.get('photocraftSystemTheme'), 'light');
assert.equal(inputs.at(-1).text, 'light');

app.onConfigurationUpdate({ colorMode: ConfigurationConstant.ColorMode.COLOR_MODE_NOT_SET });
assert.equal(storage.get('photocraftSystemTheme'), '');
assert.equal(inputs.at(-1).text, '');

app.context = { config: { colorMode: ConfigurationConstant.ColorMode.COLOR_MODE_LIGHT } };
app.onForeground();
assert.equal(storage.get('photocraftSystemTheme'), 'light');
assert.ok(inputs.some(item => item.kind === 'system_theme' && item.text === 'light'));

console.log('Passed: colorMode mapping, AppStorage publish, configuration update, foreground re-sync.');
