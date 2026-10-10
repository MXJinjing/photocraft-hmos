const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');
const ts = require(process.argv[2] || 'typescript');
function load(file, imports) {
  const module = { exports: {} };
  const result = ts.transpileModule(fs.readFileSync(file, 'utf8'), {
    compilerOptions: { target: ts.ScriptTarget.ES2020, module: ts.ModuleKind.CommonJS }
  });
  vm.runInNewContext(result.outputText, { module, exports: module.exports,
    require: name => { if (imports[name]) return imports[name]; throw new Error(name); } });
  return module.exports;
}
const formats = load('entry/src/main/ets/saveFormats.ets', {});
const { savePickerOptions, SaveTargets } = load('entry/src/main/ets/savePicker.ets', {
  '@kit.CoreFileKit': { picker: { DocumentSaveOptions: class {} } }, './saveFormats': formats
});
const directory = 'file://docs/storage/Users/currentUser/Download/test.bundle';
for (const suggested of ['Untitled-1.psd', '/elsewhere/中文.psd', 'save-as/42/renamed.psd']) {
  const options = savePickerOptions(suggested, directory);
  assert.equal(options.defaultFilePathUri, directory);
  assert.equal(options.pickerMode, undefined); // Full picker, not silent DOWNLOAD mode.
  assert.equal(options.newFileNames[0], suggested.split('/').at(-1));
  assert.ok(options.fileSuffixChoices.length > 0);
}
const targets = new SaveTargets();
const first = targets.select(1, 'same.psd', 'uri-one', true);
const second = targets.select(2, 'same.psd', 'uri-two', true);
assert.equal(targets.take(first), 'uri-one');
assert.equal(targets.take(first), 'uri-one'); // Subsequent save or retry uses the same grant.
assert.equal(targets.take(second), 'uri-two');
const copy = targets.select(3, 'copy.psd', 'uri-copy', false);
assert.equal(targets.take(copy), 'uri-copy');
assert.equal(targets.take(copy), undefined);
assert.equal(targets.take(first), 'uri-one'); // Save As does not replace ordinary Save.
targets.clear();
assert.equal(targets.take(first), undefined);
console.log('Passed: full picker defaults to system Download/package URI, names, formats, repeat saves, independent documents, one-shot Save As, lifecycle cleanup.');
