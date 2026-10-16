// Run the actual cold-start policy and EntryAbility against mocked HarmonyOS services.
// Usage: node scripts/test_host_mode.cjs <typescript module>
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');
const ts = require(process.argv[2] || 'typescript');
const root = path.resolve(__dirname, '../entry/src/main/ets');
function load(name, dependencies) {
  const filename = path.join(root, name + '.ets');
  const result = ts.transpileModule(fs.readFileSync(filename, 'utf8'), {
    fileName: name + '.ts', reportDiagnostics: true,
    compilerOptions: { target: ts.ScriptTarget.ES2020, module: ts.ModuleKind.CommonJS }
  });
  assert.deepEqual((result.diagnostics || []).filter(d => d.category === ts.DiagnosticCategory.Error), []);
  const module = { exports: {} };
  vm.runInNewContext(result.outputText, {
    module, exports: module.exports, console: { info() {}, error() {} },
    AppStorage: { get() {}, setOrCreate() {} },
    require: name => { assert.ok(name in dependencies, `unexpected dependency ${name}`); return dependencies[name]; }
  }, { filename });
  return module.exports;
}
  for (const request of [undefined, false, true, 'true', 1]) {
    const calls = [];
    const { default: EntryAbility } = load('entryability/EntryAbility', {
      'libphotocraft.so': {
        default: {
          active: value => calls.push(['active', value]),
          input: json => calls.push(['input', JSON.parse(json)])
        }
      },
      '@kit.AbilityKit': { UIAbility: class {} },
      '@kit.ArkUI': {}, '@kit.BasicServicesKit': {},
      '../SystemBars': { LOADING_CHROME: '#FF262626', paintWindow() {}, applySystemBars() {} },
      // SystemTheme arrived with dark-mode support; host-mode must mock it like the other EntryAbility deps.
      '../SystemTheme': {
        systemThemeFromColorMode: colorMode => (colorMode === 0 ? 'dark' : colorMode === 1 ? 'light' : ''),
        publishSystemTheme: theme => calls.push(['theme', theme])
      },
      '../WindowChrome': { WindowChrome: class { start() {} stop() {} refresh() {} } },
      '../input/NativeCursorAdapter': { restorePointerVisibility() {} },
      '../openInbox': { acceptOpenWant: want => calls.push(['open', want.uri]) },
      '../closeGuard': { resetCloseGuard() {}, requestWindowClose() {} },
    });
    const app = new EntryAbility();
    // UIAbility.context is framework-owned; provide a minimal config so onCreate/onForeground theme sync works.
    app.context = { config: { colorMode: 0 } };
    app.onCreate({ parameters: { 'photocraft.dev': request } }, {});
    assert.ok(calls.some(c => c[0] === 'theme' && c[1] === 'dark'), 'onCreate publishes the system theme');
    assert.ok(calls.some(c => c[0] === 'input' && c[1].kind === 'system_theme' && c[1].text === 'dark'),
      'onCreate pushes system_theme into the native worker');
    const pages = [];
    const stage = { on() {}, getMainWindowSync() { return {}; }, loadContent(page, cb) { pages.push(page); cb({ code: 0 }); } };
    app.onWindowStageCreate(stage);
    const expected = 'pages/Index';
    assert.equal(pages[0], expected);
    app.onForeground(); app.onBackground();
    assert.deepEqual(calls.filter(c => c[0] === 'active'), [['active', true], ['active', false]]);
    app.onNewWant({ uri: 'file://example.psd', parameters: { 'photocraft.dev': request !== true } });
    assert.equal(pages.length, 1, 'new Wants must not replace a live editor');
    assert.ok(calls.some(c => c[0] === 'open' && c[1] === 'file://example.psd'), 'file Wants still reach the active editor');
    app.onWindowStageCreate(stage);
    assert.equal(pages[1], expected, 'the selected host survives window-stage recreation');
  }
console.log('Passed: native-only routing regardless of legacy Want flags, lifecycle, stage recreation and file Wants.');
