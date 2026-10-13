const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');
const ts = require(process.argv[2] || 'typescript');
const UTD = { IMAGE: 'general.image', FILE_URI: 'general.file-uri', FILE: 'general.file', OPENHARMONY_PIXEL_MAP: 'openharmony.pixel-map' };
let calls = [], content = new Uint8Array([1, 2, 3, 4, 5]), offset = 0, size = 5, zeroRead = false, failRead = false, failPack = false;
let serial = 0, pendingRead, releaseRead;
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
  open: async path => { calls.push(['open', path]); offset = 0; return { fd: 1 }; },
  stat: async () => ({ size }),
  read: async (_fd, buffer) => {
    if (pendingRead) await pendingRead;
    if (failRead) throw new Error('read failed');
    if (zeroRead) return 0;
    const count = Math.min(2, content.length - offset);
    new Uint8Array(buffer).set(content.subarray(offset, offset + count)); offset += count;
    return count;
  },
  close: async () => { calls.push(['close']); }
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
  const cancelled = [], adapter = new moduleObject.exports.NativeDropAdapter('/cache', key => cancelled.push(key));
  let event = drop([uri('one%20image.png'), uri('one%20image.png'), new Image('file:///legacy/two.jpg'), new File('file:///legacy/three.psd')]);
  let result = await adapter.receive(event, () => true);
  assert.deepEqual(Array.from(result, i => i.name), ['one image.png', 'two.jpg', 'three.psd']);
  assert.deepEqual(Array.from(new Uint8Array(result[0].bytes)), [1, 2, 3, 4, 5], 'partial reads assemble exactly');
  assert.equal(calls.filter(c => c[0] === 'open').length, 3, 'URI records deduplicated');
  assert.equal(calls.filter(c => c[0] === 'close').length, 3);
  assert.equal(event.dragBehavior, 0); assert.deepEqual(event.results, [1, 0]);
  assert.ok(event.options.destUri.startsWith('file:///cache/')); assert.equal(event.options.progressIndicator, 1);
  result = await adapter.receive(drop([pixels()]), () => true);
  assert.equal(result[0].name, 'dropped-image.png');
  assert.equal(calls.find(c => c[0] === 'pack')[1].format, 'image/png'); assert.ok(calls.some(c => c[0] === 'release'));
  result = await adapter.receive(drop([record({ 'general.png': content.buffer })]), () => true); assert.equal(result[0].bytes, content.buffer);
  for (const badSize of [0, -1, 256 * 1024 * 1024 + 1, Infinity]) {
    size = badSize; await assert.rejects(adapter.receive(drop([uri('bad.png')]), () => true), /256 MiB/);
  }
  size = 5;
  zeroRead = true; await assert.rejects(adapter.receive(drop([uri('short.png')]), () => true), /不完整/); zeroRead = false;
  failRead = true; await assert.rejects(adapter.receive(drop([uri('bad.png')]), () => true), /read failed/); failRead = false;
  failPack = true; await assert.rejects(adapter.receive(drop([pixels()]), () => true), /pack failed/); failPack = false;
  await assert.rejects(adapter.receive(drop([pixels(100000, 100000)]), () => true), /256 MiB/);
  await assert.rejects(adapter.receive(drop(Array.from({ length: 17 }, (_, i) => uri(i + '.png'))), () => true), /16/);
  await assert.rejects(adapter.receive(drop([new Image('https://example.com/image.png')]), () => true), /本地文件/);
  await assert.rejects(adapter.receive(drop([record({ 'general.plain-text': 'hello' })]), () => true), /没有可导入/);
  await assert.rejects(adapter.receive(drop([]), () => true), /没有可导入/);
  const large = new ArrayBuffer(129 * 1024 * 1024);
  await assert.rejects(adapter.receive(drop([record({ 'general.png': large }), record({ 'general.png': large })]), () => true), /256 MiB/, 'aggregate byte budget applies across records');
  await assert.rejects(adapter.receive(drop([], 203), () => true), /状态 203/);
  assert.equal((await adapter.receive(drop([], 2), () => true)).length, 0);
  let alive = true; event = drop([uri('late.png')], 0, true);
  const loading = adapter.receive(event, () => alive);
  const busy = drop([uri('busy.png')]); assert.equal((await adapter.receive(busy, () => true)).length, 0); assert.equal(busy.options, undefined);
  adapter.stop(); assert.deepEqual(cancelled, ['drop-key']); alive = false; event.finish();
  assert.equal((await loading).length, 0, 'old page must not import completed transfer');
  alive = true; pendingRead = new Promise(resolve => releaseRead = resolve);
  const reading = adapter.receive(drop([uri('late-read.png')]), () => alive);
  await new Promise(resolve => setImmediate(resolve)); alive = false; releaseRead();
  assert.equal((await reading).length, 0, 'old page must not import completed file reads'); pendingRead = undefined;
  const throwing = drop([]); throwing.startDataLoading = () => { throw Error('start failed'); };
  await assert.rejects(adapter.receive(throwing, () => true), /start failed/);
  assert.equal(calls.filter(c => c[0] === 'temp').length, calls.filter(c => c[0] === 'cleanup').length, 'every temp directory is cleaned');
  assert.equal(calls.filter(c => c[0] === 'pack').length, calls.filter(c => c[0] === 'release').length, 'packers released on success and failure');
  assert.equal(calls.filter(c => c[0] === 'open').length, calls.filter(c => c[0] === 'close').length, 'file handles close on all outcomes');
  assert.equal((await adapter.receive(drop([uri('again.png')]), () => true)).length, 1, 'adapter usable after all failures');
  console.log('PASS: URI/legacy/raw/PixelMap, partial reads, duplicate records, limits, copy semantics, cancellation, busy rejection, stale page, and cleanup.');
})().catch(error => { console.error(error); process.exitCode = 1; });
