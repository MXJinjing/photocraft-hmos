// Exercise the real ArkTS adapter with mocked SDK callbacks and verify its wire packets against
// fixtures consumed by Rust. Usage: node scripts/test_stylus_adapter.cjs <typescript module>
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');
const ts = require(process.argv[2] || 'typescript');
const root = path.resolve(__dirname, '..');
const modules = path.join(root, 'entry/src/main/ets/stylus');

function load(name, dependencies, globals = {}) {
  const filename = path.join(modules, name + '.ets');
  const result = ts.transpileModule(fs.readFileSync(filename, 'utf8'), {
    fileName: name + '.ts', reportDiagnostics: true,
    compilerOptions: { target: ts.ScriptTarget.ES2020, module: ts.ModuleKind.CommonJS }
  });
  const errors = (result.diagnostics || []).filter(d => d.category === ts.DiagnosticCategory.Error);
  assert.deepEqual(errors, [], `${name}: TypeScript transpilation failed`);
  const module = { exports: {} };
  vm.runInNewContext(result.outputText, {
    module, exports: module.exports,
    require: name => { assert.ok(name in dependencies, `unexpected SDK dependency ${name}`); return dependencies[name]; },
    console: { error() {} }, ...globals
  }, { filename });
  return module.exports;
}
const protocol = load('StylusProtocol', {});
const plain = value => JSON.parse(JSON.stringify(value));
const sample = new protocol.StylusSample();
Object.assign(sample, { pressure: 0.25, tiltX: 30, tiltY: -20, rotation: 90 });
const factoryPackets = [
  protocol.StylusPacket.connection(true),
  protocol.StylusPacket.pointer(protocol.StylusPointerSource.Pen, sample),
  protocol.StylusPacket.barrelButton(true, 500),
  protocol.StylusPacket.barrelButton(false, 550),
  protocol.StylusPacket.action(protocol.StylusAction.DoubleTap, 1000),
  protocol.StylusPacket.action(protocol.StylusAction.LongPress, 2000),
  protocol.StylusPacket.pointer(protocol.StylusPointerSource.Touch, null)
];
const fixture = JSON.parse(fs.readFileSync(path.join(root,
  'upstream/photocraft/crates/ui-egui/tests/fixtures/stylus-packets.json'), 'utf8'));
assert.deepEqual(plain(factoryPackets), fixture, 'ArkTS factories must match the Rust protocol fixture');

// Execute the actual injection script, including early connection delivery before WASM starts.
const events = [];
const window = { dispatchEvent: event => events.push({ type: event.type, detail: plain(event.detail) }) };
function CustomEvent(type, options) { this.type = type; this.detail = options.detail; }
for (const packet of factoryPackets) {
  vm.runInNewContext(protocol.stylusPacketScript(packet), { window, CustomEvent });
}
assert.equal(events.length, factoryPackets.length);
assert.ok(events.every(event => event.type === 'photocraft-stylus-input'));
assert.deepEqual(events.map(event => event.detail), fixture);
assert.deepEqual(plain(window[protocol.STYLUS_STATE_PROPERTY]), fixture[0], 'only connection is cached for startup replay');

function setup(failedGesture) {
  const gestures = new Map();
  const devices = new Map();
  const timers = new Map();
  const pending = [];
  let nextTimer = 0;
  const sdk = {
    on(name, callback) {
      if (name === failedGesture) throw { code: 1, message: 'not supported' };
      assert.ok(!gestures.has(name), 'register each gesture once');
      gestures.set(name, callback);
    },
    off(name, callback) { assert.equal(gestures.get(name), callback); gestures.delete(name); }
  };
  const device = {
    KeyboardType: { HANDWRITING_PEN: 13 },
    getKeyboardTypeSync: id => id === 1 ? 13 : 0,
    getDeviceInfoSync: () => ({ name: 'Keyboard' }),
    getDeviceList: () => new Promise(resolve => pending.push(resolve)),
    on: (name, callback) => devices.set(name, callback),
    off(name, callback) { assert.equal(devices.get(name), callback); devices.delete(name); }
  };
  const { HarmonyStylusAdapter } = load('HarmonyStylusAdapter', {
    './StylusProtocol': protocol, '@kit.Penkit': { stylusInteraction: sdk },
    '@kit.InputKit': { inputDevice: device }
  }, {
    Date: { now: () => 1234567890000 },
    setInterval: callback => { const id = nextTimer++; timers.set(id, callback); return id; },
    clearInterval: id => timers.delete(id)
  });
  const packets = [];
  const adapter = new HarmonyStylusAdapter(packet => packets.push(plain(packet)));
  return { adapter, gestures, devices, timers, pending, packets };
}
async function flush() { await new Promise(resolve => setImmediate(resolve)); }

async function main() {
  const s = setup();
  s.adapter.start(); s.adapter.start();
  assert.equal(s.gestures.size, 2); assert.equal(s.devices.size, 1); assert.equal(s.timers.size, 1);
  assert.equal(s.pending.length, 1, 'start is idempotent');
  s.pending.shift()([1]); await flush();
  assert.deepEqual(s.packets.pop(), { version: 1, type: 'connection', connected: true });
  s.gestures.get('doubleTap')({ timestamp: 7 });
  s.gestures.get('squeeze')({ timestamp: 8 });
  assert.deepEqual(s.packets.splice(0), [
    { version: 1, type: 'gesture', gesture: 'doubleTap', timestampMs: 1234567890000 },
    { version: 1, type: 'gesture', gesture: 'longPress', timestampMs: 1234567890000 }
  ], 'SDK-specific names/timestamp units must be normalised');
  s.timers.values().next().value(); s.pending.shift()([1]); await flush();
  assert.equal(s.packets.length, 0, 'unchanged connection does not repeatedly emit');
  s.adapter.publishState(); s.pending.shift()([1]); await flush();
  assert.deepEqual(s.packets.pop(), { version: 1, type: 'connection', connected: true }, 'page reload republishes current state');
  s.devices.get('change')({}); s.pending.shift()([]); await flush();
  assert.deepEqual(s.packets.pop(), { version: 1, type: 'connection', connected: false });

  // An asynchronous scan from before stop/restart must not update the new adapter lifetime.
  const oldGesture = s.gestures.get('doubleTap');
  s.timers.values().next().value();
  const stale = s.pending.shift();
  s.adapter.stop(); s.adapter.stop();
  assert.equal(s.gestures.size, 0); assert.equal(s.devices.size, 0); assert.equal(s.timers.size, 0);
  oldGesture({ timestamp: 9 }); assert.equal(s.packets.length, 0);
  s.adapter.start();
  stale([1]); await flush(); assert.equal(s.packets.length, 0, 'ignore old asynchronous result after restart');
  s.pending.shift()([]); await flush();
  assert.deepEqual(s.packets.pop(), { version: 1, type: 'connection', connected: false });
  s.adapter.stop();

  const partial = setup('squeeze');
  partial.adapter.start(); partial.pending.shift()([]); await flush();
  assert.equal(partial.gestures.size, 1, 'one unsupported feature must not disable the other');
  partial.gestures.get('doubleTap')({});
  assert.equal(partial.packets.at(-1).gesture, 'doubleTap');
  partial.adapter.stop(); assert.equal(partial.gestures.size, 0); assert.equal(partial.timers.size, 0);
  console.log('Passed: shared packet contract, transport/state replay, gesture mapping, lifecycle, partial SDK support.');
}
main().catch(error => { console.error(error); process.exitCode = 1; });
