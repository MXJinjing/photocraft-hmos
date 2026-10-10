const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');
const ts = require(process.argv[2] || 'typescript');
let resultUris = ['file://docs/storage/Users/currentUser/Download/test.bundle'];
let calls = 0;
const result = ts.transpileModule(fs.readFileSync('entry/src/main/ets/documentDirectory.ets', 'utf8'), {
  compilerOptions: { target: ts.ScriptTarget.ES2020, module: ts.ModuleKind.CommonJS }
});
const moduleShim = { exports: {} };
const context = {};
vm.runInNewContext(result.outputText, {
  module: moduleShim, exports: moduleShim.exports,
  require: name => {
    if (name === '@kit.CoreFileKit') return {
      picker: { DocumentSaveOptions: class {}, DocumentPickerMode: { DOWNLOAD: 1 },
        DocumentViewPicker: class {
          constructor(received) { assert.equal(received, context); }
          async save(options) {
            calls++;
            assert.equal(options.pickerMode, 1);
            assert.equal(options.newFileNames, undefined);
            assert.equal(options.defaultFilePathUri, undefined);
            return resultUris;
          }
        }
      }, fileUri: { FileUri: class { constructor(uri) { this.path = uri.slice('file://docs'.length); } } }
    };
    throw new Error(name);
  }
});
(async () => {
  const { documentDirectory } = moduleShim.exports;
  assert.equal(await documentDirectory(context), '/storage/Users/currentUser/Download/test.bundle');
  resultUris = [];
  await assert.rejects(documentDirectory(context), /Unable to obtain/);
  assert.equal(calls, 2);
  console.log('Passed: API 12 DOWNLOAD directory, exact system-returned path, no location selection or silent fallback.');
})().catch(error => { console.error(error); process.exitCode = 1; });
