const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');
const ts = require(process.argv[2] || 'typescript');
let calls = [], events = {}, failPrint = false, failWrite = false;
const task = { on: (event, cb) => events[event] = cb, off: event => delete events[event] };
const fileIo = {
  OpenMode: { READ_WRITE: 1, CREATE: 2, TRUNC: 4 },
  open: async path => { calls.push(['open', path]); return { fd: 42 }; },
  write: async (_fd, bytes) => { if (failWrite) return 0; return Math.min(7, bytes.byteLength); },
  fsync: async () => {}, close: async () => calls.push(['close']),
  unlink: async path => calls.push(['unlink', path])
};
const source = fs.readFileSync('entry/src/main/ets/NativePrintAdapter.ets', 'utf8');
const result = ts.transpileModule(source, { reportDiagnostics: true, compilerOptions: { target: ts.ScriptTarget.ES2020, module: ts.ModuleKind.CommonJS } });
assert.deepEqual((result.diagnostics || []).filter(d => d.category === ts.DiagnosticCategory.Error), []);
const mod = { exports: {} };
vm.runInNewContext(result.outputText, { module: mod, exports: mod.exports, console, require: name => {
  if (name === '@kit.AbilityKit') return {};
  if (name === '@kit.CoreFileKit') return { fileIo, fileUri: { getUriFromPath: p => 'uri:' + p } };
  if (name === '@kit.BasicServicesKit') return { print: { print: async (uris, context) => {
    calls.push(['print', uris, context]); if (failPrint) throw new Error('service unavailable'); return task;
  } } };
  throw new Error(name);
} });
(async () => {
  const errors = [], context = { cacheDir: '/cache' };
  const adapter = new mod.exports.NativePrintAdapter(context, e => errors.push(e));
  const pdf = new TextEncoder().encode('%PDF-1.4 test document').buffer;
  const metadata = JSON.stringify({ name: '../test.psd', copies: 1 });
  await adapter.open(metadata, pdf);
  const path = calls.find(c => c[0] === 'open')[1];
  assert.ok(path.startsWith('/cache/.._test.psd-'));
  assert.equal(calls.find(c => c[0] === 'print')[2], context);
  assert.equal(calls.filter(c => c[0] === 'unlink').length, 0, 'preview must retain PDF');
  events.block(); assert.equal(errors.length, 1); assert.ok(events.succeed);
  events.cancel(); assert.equal(calls.filter(c => c[0] === 'unlink').length, 1);
  assert.equal(Object.keys(events).length, 0);
  await adapter.open(metadata, pdf); events.succeed();
  await adapter.open(metadata, pdf); events.fail(); assert.equal(errors.length, 2);
  failPrint = true; await assert.rejects(adapter.open(metadata, pdf), /service unavailable/);
  failPrint = false; failWrite = true; await assert.rejects(adapter.open(metadata, pdf), /写入不完整/);
  failWrite = false;
  await assert.rejects(adapter.open(metadata, new ArrayBuffer(0)), /为空/);
  await assert.rejects(adapter.open(metadata, new TextEncoder().encode('invalid').buffer), /不是 PDF/);
  await assert.rejects(adapter.open(JSON.stringify({ copies: 2 }), pdf), /份数/);
  adapter.stop(); await assert.rejects(adapter.open(metadata, pdf), /cancelled/);
  console.log('Passed: PDF validation, partial writes, UIAbility context, preview lifetime, completion/cancel/failure/block, service errors and disposal.');
})().catch(e => { console.error(e); process.exitCode = 1; });
