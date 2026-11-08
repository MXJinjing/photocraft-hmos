// NativeConfig must carry cacheDir across the ArkTS -> Rust JSON boundary.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');
const ts = require('typescript');
const source = fs.readFileSync('entry/src/main/ets/pages/Index.ets', 'utf8');
const config = source.slice(source.indexOf('class NativeConfig {'), source.indexOf('class NativeMessage {'));
const code = ts.transpileModule(config + '\n;globalThis.NativeConfig = NativeConfig;', {
  compilerOptions: { target: ts.ScriptTarget.ES2020, module: ts.ModuleKind.CommonJS }
}).outputText;
const scope = {};
vm.runInNewContext(code, scope);
const value = JSON.parse(JSON.stringify(new scope.NativeConfig('/files', '/documents', 2, 'zh', false, 'light', true, '/cache')));
assert.equal(value.cache, '/cache');
assert.equal(value.files, '/files');
assert.match(source, /fullscreenAvailable\(\), this\.context\.cacheDir/);
const rust = fs.readFileSync('native/rust/src/lib.rs', 'utf8');
assert.match(rust, /#\[serde\(default\)\]\s+cache: String/);
assert.ok(rust.indexOf('scratch::initialize(') < rust.indexOf('let mut app = PhotocraftApp::new('));
const after = rust.slice(rust.indexOf('let mut app = PhotocraftApp::new('));
assert.ok(after.indexOf('memory_probe::log_current();') < after.indexOf('let host_fullscreen'));
console.log('Passed: cacheDir JSON bridge, legacy config default, scratch-before-session and post-preference diagnostics.');
