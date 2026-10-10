// Execute the dialog's actual filename validation and confirmation callbacks.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');
const ts = require(process.argv[2] || 'typescript');
let source = fs.readFileSync('entry/src/main/ets/LocalSaveDialog.ets', 'utf8');
source = source.replace('@CustomDialog', '').replace('export struct', 'export class').replace(/@State /g, '');
source = source.slice(0, source.indexOf('  build()')) + '}';
const result = ts.transpileModule(source, { compilerOptions: { target: ts.ScriptTarget.ES2020, module: ts.ModuleKind.CommonJS } });
const moduleShim = { exports: {} };
vm.runInNewContext(result.outputText, { module: moduleShim, exports: moduleShim.exports });
const { LocalSaveDialog } = moduleShim.exports;
for (const [input, expected] of [['照片', '照片.psd'], [' edited.PSD ', 'edited.PSD'], ['x.png', 'x.png.psd'], ['', null], ['../bad', null], ['a/b', null], ['a\\b', null], ['..', null], ['a\0b', null]]) {
  let received = null, closed = 0;
  const dialog = new LocalSaveDialog();
  dialog.controller = { close: () => closed++ };
  dialog.initialName = input;
  dialog.onConfirm = name => { received = name; };
  dialog.aboutToAppear(); dialog.confirm();
  assert.equal(received, expected);
  assert.equal(closed, expected === null ? 0 : 1);
}
console.log('Passed: editable PSD filename, confirmation, invalid names keep dialog open.');
