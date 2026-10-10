const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');
const ts = require(process.argv[2] || 'typescript');
const moduleShim = {exports: {}};
vm.runInNewContext(ts.transpileModule(fs.readFileSync('entry/src/main/ets/saveFormats.ets', 'utf8'), {
  compilerOptions: {target: ts.ScriptTarget.ES2020, module: ts.ModuleKind.CommonJS}
}).outputText, {module: moduleShim, exports: moduleShim.exports});
const choices = moduleShim.exports.saveFormatChoices;
for (const ext of ['psd','psb','pcraft','png','jpg','webp','tif','tga','exr']) {
  const list = choices('/folder/画.' + ext.toUpperCase());
  assert.equal(list.length, 9);
  assert.equal(new Set(list).size, 9);
  assert.ok(list[0].endsWith('|.' + ext));
  assert.ok(list.includes('Photoshop PSD|.psd'));
  assert.ok(list.includes('PhotoCraft|.pcraft'));
  assert.ok(!list.includes('PhotoCraft|.psd'));
}
assert.equal(choices('photo.jpeg')[0], 'JPEG 图像|.jpg');
assert.equal(choices('photo.tiff')[0], 'TIFF 图像|.tif');
assert.equal(choices('Untitled')[0], 'Photoshop PSD|.psd');
assert.deepEqual(Array.from(choices('drawing.PDF')), ['PDF 文档|.pdf']);
console.log('Passed: real Save As formats, correct labels, suggested extension first, aliases and PSD default.');
