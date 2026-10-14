const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');
const ts = require(process.argv[2] || 'typescript');
// SDK contract: async loading requires disabling ArkUI's automatic prefetch.
const page = fs.readFileSync('entry/src/main/ets/pages/Index.ets', 'utf8');
assert.match(page, /\.onDrop\(\(event: DragEvent\): void => \{ this\.dropImages\(event\); \}, \{ disableDataPrefetch: true \}\)/);
const UTD = { IMAGE: 'general.image', FILE_URI: 'general.file-uri', FILE: 'general.file', OPENHARMONY_PIXEL_MAP: 'openharmony.pixel-map' };
let calls = [], content = new Uint8Array([1, 2, 3, 4, 5]), failPack = false;
let serial = 0;
class UnifiedData { constructor(records = []) { this.records = records; } getRecords() { return this.records; } }
class File { constructor(uri) { this.uri = uri; } getTypes() { return [UTD.FILE]; } }
class Image extends File { constructor(uri) { super(uri); this.imageUri = uri; } getTypes() { return [UTD.IMAGE]; } }
const record = entries => ({ getTypes: () => Object.keys(entries), getEntry: type => entries[type] });
const uri = name => record({ [UTD.FILE_URI]: { oriUri: 'file:///authorized/' + name, fileType: 'general.png' } });
const pixels = (w = 2, h = 2) => record({ [UTD.OPENHARMONY_PIXEL_MAP]: { pixelMap: { getImageInfo: async () => ({ size: { width: w, height: h } }) } } });
const fileIo = {
  OpenMode: { READ_ONLY: 0 },
  mkdtempSync: prefix => { calls.push(['temp', prefix]); return '/cache/drop-' + ++serial; },
  rmdir: async path => { calls.push(['cleanup', path]); },
  open: async path => { const fd = ++serial; calls.push(['open', path, fd]); return { fd }; },
  close: async file => { calls.push(['close', file.fd]); }
};
const code = ts.transpileModule(fs.readFileSync('entry/src/main/ets/input/NativeDropAdapter.ets', 'utf8'), {
  reportDiagnostics: true, compilerOptions: { target: ts.ScriptTarget.ES2020, module: ts.ModuleKind.CommonJS }
});
assert.equal((code.diagnostics || []).filter(d => d.category === ts.DiagnosticCategory.Error).length, 0);
const moduleObject = { exports: {} };
vm.runInNewContext(code.outputText, { module: moduleObject, exports: moduleObject.exports, console, ArrayBuffer, Uint8Array,
  DragResult: { DRAG_FAILED: 1, DRAG_SUCCESSFUL: 0 }, DragBehavior: { COPY: 0 },
  require: name => {
    if (name === '@kit.ArkData') return {
      unifiedDataChannel: { UnifiedData, Image, File, FileConflictOptions: { OVERWRITE: 0 }, ProgressIndicator: { DEFAULT: 1 }, ListenerStatus: { FINISHED: 0, PROCESSING: 1, CANCELED: 2 } },
      uniformTypeDescriptor: { UniformDataType: UTD, getTypeDescriptor: type => ({ belongsTo: parent => parent === UTD.IMAGE && ['general.png', UTD.IMAGE].includes(type), filenameExtensions: ['.png'] }) }
    };
    if (name === '@kit.CoreFileKit') return { fileIo, fileUri: { getUriFromPath: p => 'file://' + p, FileUri: class { constructor(p) { this.name = decodeURIComponent(p.split('/').pop()); } } } };
    if (name === '@kit.ImageKit') return { image: { createImagePacker: () => ({
      packing: async (_pixels, options) => { calls.push(['pack', options]); if (failPack) throw new Error('pack failed'); return content.buffer; },
      release: async () => { calls.push(['release']); }
    }) } };
    throw Error(name);
  }
});
function drop(records, status = 0, delayed = false) {
  const event = { results: [], setResult(result) { this.results.push(result); }, startDataLoading(options) {
    this.options = options;
    this.finish = () => options.dataProgressListener({ status, progress: status === 0 ? 100 : -1 }, status === 0 ? new UnifiedData(records) : null);
    if (!delayed) this.finish();
    return 'drop-key';
  } };
  return event;
}
(async () => {
  const cancelled = [], adapter = new moduleObject.exports.NativeDropAdapter('/cache', key => cancelled.push(key),
    (name, fd) => calls.push(['openFile', name, fd]));
  let event = drop([uri('one%20image.png'), uri('one%20image.png'), new Image('file:///legacy/two.jpg'), new File('file:///legacy/three.psd')]);
  let result = await adapter.receive(event, () => true);
  assert.deepEqual(Array.from(result, i => i.name), [], 'file records are handed to native by descriptor');
  assert.equal(calls.filter(c => c[0] === 'open').length, 3, 'URI records deduplicated');
  assert.deepEqual(Array.from(calls.filter(c => c[0] === 'openFile'), c => c[1]), ['one image.png', 'two.jpg', 'three.psd']);
  assert.equal(calls.filter(c => c[0] === 'close').length, 3);
  assert.equal(event.dragBehavior, 0); assert.deepEqual(event.results, [1, 0]);
  assert.ok(event.options.destUri.startsWith('file:///cache/')); assert.equal(event.options.progressIndicator, 1);
  result = await adapter.receive(drop([pixels()]), () => true);
  assert.equal(result[0].name, 'dropped-image.png');
  assert.equal(calls.find(c => c[0] === 'pack')[1].format, 'image/png'); assert.ok(calls.some(c => c[0] === 'release'));
  result = await adapter.receive(drop([record({ 'general.png': content.buffer })]), () => true); assert.equal(result[0].bytes, content.buffer);
  failPack = true; await assert.rejects(adapter.receive(drop([pixels()]), () => true), /pack failed/); failPack = false;
  await adapter.receive(drop([pixels(100000, 100000)]), () => true);
  await assert.rejects(adapter.receive(drop(Array.from({ length: 17 }, (_, i) => uri(i + '.png'))), () => true), /16/);
  await assert.rejects(adapter.receive(drop([new Image('https://example.com/image.png')]), () => true), /本地文件/);
  await assert.rejects(adapter.receive(drop([record({ 'general.plain-text': 'hello' })]), () => true), /没有可导入/);
  await assert.rejects(adapter.receive(drop([]), () => true), /没有可导入/);
  const large = new ArrayBuffer(257 * 1024 * 1024);
  result = await adapter.receive(drop([record({ 'general.png': large })]), () => true);
  assert.equal(result[0].bytes.byteLength, large.byteLength, 'encoded image data has no 256 MiB cap');
  await assert.rejects(adapter.receive(drop([], 203), () => true), /状态 203/);
  assert.equal((await adapter.receive(drop([], 2), () => true)).length, 0);
  let alive = true; event = drop([uri('late.png')], 0, true);
  const loading = adapter.receive(event, () => alive);
  const busy = drop([uri('busy.png')]); assert.equal((await adapter.receive(busy, () => true)).length, 0); assert.equal(busy.options, undefined);
  adapter.stop(); assert.deepEqual(cancelled, ['drop-key']); alive = false; event.finish();
  assert.equal((await loading).length, 0, 'old page must not import completed transfer');
  const throwing = drop([]); throwing.startDataLoading = () => { throw Error('start failed'); };
  await assert.rejects(adapter.receive(throwing, () => true), /start failed/);
  assert.equal(calls.filter(c => c[0] === 'temp').length, calls.filter(c => c[0] === 'cleanup').length, 'every temp directory is cleaned');
  assert.equal(calls.filter(c => c[0] === 'pack').length, calls.filter(c => c[0] === 'release').length, 'packers released on success and failure');
  assert.equal(calls.filter(c => c[0] === 'open').length, calls.filter(c => c[0] === 'close').length, 'file handles close on all outcomes');
  assert.equal((await adapter.receive(drop([uri('again.png')]), () => true)).length, 0, 'adapter usable after all failures');
  assert.ok(calls.some(c => c[0] === 'openFile' && c[1] === 'again.png'));
  console.log('PASS: URI descriptor handoff, raw/PixelMap data, duplicate records, no 256 MiB cap, cancellation, busy rejection, stale page, and cleanup.');
})().catch(error => { console.error(error); process.exitCode = 1; });
