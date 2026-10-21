const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');
const ts = require(process.argv[2] || 'typescript');
const files = new Map([['/Download/test/photo.psd', Buffer.from('original')]]);
const handles = new Map();
let fd = 0, fail = false, failPublish = false;
const io = {
  OpenMode: { READ_ONLY: 0, READ_WRITE: 2, CREATE: 64, TRUNC: 512 },
  async access(path) { return files.has(path); },
  async open(path, mode) {
    if (!files.has(path) && !(mode & 64)) throw Object.assign(Error('missing'), {code:13900002});
    if (!files.has(path) || (mode & 512)) files.set(path, Buffer.alloc(0));
    handles.set(++fd, path); return {fd};
  },
  async write(fd, bytes) {
    if (fail) throw Error('disk full');
    const chunk = Buffer.from(bytes).subarray(0, 2); // exercise short writes
    const path = handles.get(fd);
    files.set(path, Buffer.concat([files.get(path), chunk]));
    return chunk.length;
  },
  async mkdir(path) { files.set(path, Buffer.alloc(0)); },
  async fsync() {}, async close(file) { handles.delete(file.fd); },
  async rename(from, to) { files.set(to, files.get(from)); files.delete(from); },
  async copyFile(from, to) {
    from = typeof from === 'number' ? handles.get(from) : from;
    to = typeof to === 'number' ? handles.get(to) : to;
    if (failPublish && to.startsWith('/Download/')) {
      failPublish = false; files.set(to, Buffer.from('partial')); throw Error('publish failed');
    }
    files.set(to, Buffer.from(files.get(from)));
  },
  async unlink(path) { files.delete(path); }
};
const moduleShim = {exports: {}};
const compiled = ts.transpileModule(fs.readFileSync('entry/src/main/ets/LocalDocuments.ets', 'utf8'), {
  compilerOptions: {target: ts.ScriptTarget.ES2020, module: ts.ModuleKind.CommonJS}
});
vm.runInNewContext(compiled.outputText, {
  module: moduleShim, exports: moduleShim.exports,
  require: () => ({fileIo: io})
});
(async () => {
  const docs = new moduleShim.exports.LocalDocuments('/Download/test', '/sandbox');
  assert.equal(await docs.available('photo.psd'), '/Download/test/photo1.psd');
  const copied = await docs.available('photo.psd');
  await docs.write(copied, Uint8Array.from([1,2,3,4,5]).buffer);
  assert.equal(copied, '/Download/test/photo1.psd');
  assert.deepEqual(files.get(copied), Buffer.from([1,2,3,4,5]));
  assert.equal(files.get('/Download/test/photo.psd').toString(), 'original');
  await docs.write(copied, Uint8Array.from([7,8,9]).buffer);
  assert.deepEqual(files.get(copied), Buffer.from([7,8,9]));
  failPublish = true;
  await assert.rejects(docs.write(copied, Uint8Array.from([0]).buffer), /publish failed/);
  assert.deepEqual(files.get(copied), Buffer.from([7,8,9]));
  fail = true;
  await assert.rejects(docs.write(copied, Uint8Array.from([0]).buffer), /disk full/);
  assert.deepEqual(files.get(copied), Buffer.from([7,8,9]));
  assert.equal(files.size, 2);
  assert.equal(handles.size, 0);
  await assert.rejects(docs.write('/outside/photo.psd', new ArrayBuffer(0)), /Invalid/);
  fail = false;
  await docs.writeOutput('/Download/test/processed/image.jpg', Uint8Array.from([1,2,3]).buffer);
  assert.deepEqual(files.get('/Download/test/processed/image.jpg'), Buffer.from([1,2,3]));
  await docs.writeOutput('/Download/test/processed/image.jpg', Uint8Array.from([4]).buffer);
  assert.deepEqual(files.get('/Download/test/processed/image.jpg'), Buffer.from([4]));
  failPublish = true;
  await assert.rejects(docs.writeOutput('/Download/test/processed/image.jpg', new ArrayBuffer(0)), /publish failed/);
  assert.deepEqual(files.get('/Download/test/processed/image.jpg'), Buffer.from([4]));
  for (const path of ['/Download/test/../outside.jpg', '/Download/test//bad.png', '/Download/test2/a.png', '/outside/a.png']) {
    await assert.rejects(docs.writeOutput(path, new ArrayBuffer(0)), /Invalid export/);
  }
  docs.grantOutput('/chosen');
  await docs.writeOutput('/chosen/exports/layer.png', Uint8Array.from([9]).buffer);
  assert.deepEqual(files.get('/chosen/exports/layer.png'), Buffer.from([9]));
  assert.equal([...files.keys()].some(path => path.endsWith('.tmp')), false);
  assert.equal(handles.size, 0);
  console.log('Passed: collision suffixes, source preservation, short writes, repeat saves, failed-save cleanup.');
})().catch(error => { console.error(error); process.exitCode = 1; });
