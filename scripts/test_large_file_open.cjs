// Usage: node scripts/test_large_file_open.cjs <typescript module>
const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');
const ts = require(process.argv[2] || 'typescript');
const handles = new Map();
const calls = [];
let nextFd = 10, failNative = false, changeGeneration = false;
let selected = ['file:///Download/large.bmp'];
const choice = { source: 'files' };
let gallerySelected = ['file:///Download/gallery.png'];
let host;
const io = {
  OpenMode: { READ_ONLY: 0 },
  openSync(uri) {
    if (uri.includes('missing')) throw Error('missing');
    const file = { fd: ++nextFd };
    handles.set(file.fd, { uri, size: 5 * 1024 ** 3 });
    return file;
  },
  async open(uri) {
    const file = this.openSync(uri);
    if (changeGeneration) host.generation++;
    return file;
  },
  closeSync(file) { assert(handles.delete(file.fd), 'close exactly once'); },
  async close(file) { this.closeSync(file); },
  stat() { assert.fail('File size must not gate opening'); },
  statSync() { assert.fail('The inbox must not stat/read whole files'); },
  read() { assert.fail('Reading belongs on the Rust IO thread'); },
  readSync() { assert.fail('The inbox must not block on reading file data'); }
};
class FileUri {
  constructor(uri) { this.path = uri.replace('file://', ''); this.name = this.path.split('/').pop(); }
}
const picker = {
  DocumentSelectOptions: class {},
  DocumentViewPicker: class { async select() { return selected; } }
};
const photoAccessHelper = {
  PhotoSelectOptions: class {},
  PhotoViewMIMETypes: { IMAGE_TYPE: 'image' },
  PhotoViewPicker: class { async select() { return { photoUris: gallerySelected }; } }
};
const native = {
  openFile(name, fd) {
    assert(handles.has(fd), 'descriptor must remain open during native handoff');
    if (failNative) throw Error('native failure');
    calls.push(['openFile', name, fd]);
  },
  replyFile(id, name, fd) {
    assert(handles.has(fd), 'picker descriptor must remain open during handoff');
    if (failNative) throw Error('native failure');
    calls.push(['replyFile', id, name, fd]);
  },
  reply(id, ok, text) { calls.push(['reply', id, ok, text]); }
};
function compile(source, globals) {
  const module = { exports: {} };
  const output = ts.transpileModule(source, {
    compilerOptions: { target: ts.ScriptTarget.ES2020, module: ts.ModuleKind.CommonJS }
  });
  vm.runInNewContext(output.outputText, { module, exports: module.exports, console, Array, ...globals });
  return module.exports;
}
const inbox = compile(fs.readFileSync('entry/src/main/ets/openInbox.ets', 'utf8'), {
  require: () => ({ fileIo: io, fileUri: { FileUri } })
});
const page = fs.readFileSync('entry/src/main/ets/pages/Index.ets', 'utf8');
const drain = page.slice(page.indexOf('  private drainOpens()'), page.indexOf('  aboutToAppear()'));
const open = page.slice(page.indexOf('  private async openFile('), page.indexOf('  private async selectSave('));
const { Harness } = compile(`export class Harness {
  alive = true; generation = 1; context = {}; failure = ''; focused = 0;
  ime = { stop() {} }; cursor = { reset() {} };
  documents = { owns: (path: string) => path.startsWith('/Download/owned/') };
  restoreEditorFocus() { this.focused++; }
  ${drain} ${open}
  chooseOpenSource() { return Promise.resolve(choice.source); }
}`, { fileIo: io, fileUri: { FileUri }, picker, photoAccessHelper, choice, native, takeOpens: inbox.takeOpens });

(async () => {
  host = new Harness();
  // Cold-start multi-file Wants keep authorized handles, with no eager reads.
  inbox.acceptOpenWant({ uri: 'file:///Download/large.bmp', parameters: {
    'ability.params.stream': ['file:///Download/large.bmp', 'file:///Download/owned/second.bmp']
  } });
  assert.equal(handles.size, 2);
  assert.equal(calls.length, 0);
  host.drainOpens();
  assert.equal(handles.size, 0);
  assert.deepEqual(calls.map(call => call.slice(0, 2)), [
    ['openFile', 'large.bmp'], ['openFile', '/Download/owned/second.bmp']
  ]);
  inbox.setOpenListener(() => host.drainOpens());
  inbox.acceptOpenWant({ uri: 'file:///Download/warm.bmp' });
  assert.equal(calls.at(-1)[1], 'warm.bmp');
  assert.equal(handles.size, 0);

  await host.openFile(42);
  assert.deepEqual(calls.at(-1).slice(0, 3), ['replyFile', 42, 'large.bmp']);
  assert.equal(handles.size, 0);
  selected = ['file:///Download/owned/project.psd'];
  await host.openFile(43);
  assert.equal(calls.at(-1)[2], '/Download/owned/project.psd');

  selected = [];
  await host.openFile(44);
  assert.deepEqual(calls.at(-1), ['reply', 44, false, 'cancelled']);
  selected = ['file:///Download/large.bmp'];
  changeGeneration = true;
  await host.openFile(45);
  assert.deepEqual(calls.at(-1), ['reply', 45, false, 'cancelled']);
  assert.equal(handles.size, 0);
  changeGeneration = false;

  failNative = true;
  await host.openFile(46);
  assert.deepEqual(calls.at(-1), ['reply', 46, false, 'native failure']);
  inbox.acceptOpenWant({ uri: 'file:///Download/failure.bmp' });
  assert.equal(host.failure, 'native failure');
  assert.equal(handles.size, 0);
  failNative = false;
  choice.source = 'gallery';
  await host.openFile(47);
  assert.deepEqual(calls.at(-1).slice(0, 3), ['replyFile', 47, 'gallery.png']);
  choice.source = '';
  await host.openFile(48);
  assert.deepEqual(calls.at(-1), ['reply', 48, false, 'cancelled']);
  assert.equal(host.focused, 7);
  console.log('PASS: large file descriptor handoff, cold/warm Wants, source names, cancellation, errors and handle cleanup.');
})().catch(error => { console.error(error); process.exitCode = 1; });
