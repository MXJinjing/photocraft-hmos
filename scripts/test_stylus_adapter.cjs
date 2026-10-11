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

function setup(failedGesture, granted = false) {
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
  const info = new Map([[1, { name: 'HUAWEI M-Pencil', uniq: '11:22:33:44:55:66', phys: '' }]]);
  const bluetooth = { ids: ['11:22:33:44:55:66'], name: 'HUAWEI M-Pencil', battery: 82, queries: 0, pending: null };
  let permissionRequests = 0;
  const device = {
    KeyboardType: { HANDWRITING_PEN: 13 },
    getKeyboardTypeSync: id => id === 1 ? 13 : 0,
    getDeviceInfoSync: id => info.get(id) || { name: 'Keyboard' },
    getDeviceList: () => new Promise(resolve => pending.push(resolve)),
    on: (name, callback) => devices.set(name, callback),
    off(name, callback) { assert.equal(devices.get(name), callback); devices.delete(name); }
  };
  const { HarmonyStylusAdapter } = load('HarmonyStylusAdapter', {
    './StylusProtocol': protocol, '@kit.Penkit': { stylusInteraction: sdk },
    '@kit.InputKit': { inputDevice: device },
    '@kit.ConnectivityKit': {
      hid: { createHidHostProfile: () => ({ getConnectedDevices: () => bluetooth.ids }) },
      connection: {
        getRemoteDeviceName: () => bluetooth.name,
        getRemoteDeviceBatteryInfo: () => {
          bluetooth.queries++;
          return bluetooth.pending ? new Promise(resolve => bluetooth.pending.push(resolve)) : Promise.resolve({ batteryLevel: bluetooth.battery });
        }
      }
    }
  }, {
    Date: { now: () => 1234567890000 },
    setInterval: callback => { const id = nextTimer++; timers.set(id, callback); return id; },
    clearInterval: id => timers.delete(id)
  });
  const packets = [];
  const adapter = new HarmonyStylusAdapter(packet => packets.push(plain(packet)), async () => { permissionRequests++; return granted; });
  return { adapter, gestures, devices, timers, pending, packets, info, bluetooth, permissionRequests: () => permissionRequests };
}
async function flush() { await new Promise(resolve => setImmediate(resolve)); }

async function main() {
  const s = setup();
  s.adapter.start(); s.adapter.start();
  assert.equal(s.gestures.size, 2); assert.equal(s.devices.size, 1); assert.equal(s.timers.size, 1);
  assert.equal(s.pending.length, 1, 'start is idempotent');
  s.pending.shift()([1]); await flush();
  assert.deepEqual(s.packets.pop(), { version: 1, type: 'connection', connected: true, deviceName: 'HUAWEI M-Pencil' });
  s.gestures.get('doubleTap')({ timestamp: 7 });
  s.gestures.get('squeeze')({ timestamp: 8 });
  assert.deepEqual(s.packets.splice(0), [
    { version: 1, type: 'gesture', gesture: 'doubleTap', timestampMs: 1234567890000 },
    { version: 1, type: 'gesture', gesture: 'longPress', timestampMs: 1234567890000 }
  ], 'SDK-specific names/timestamp units must be normalised');
  s.timers.values().next().value(); s.pending.shift()([1]); await flush();
  assert.equal(s.packets.length, 0, 'unchanged connection does not repeatedly emit');
  s.adapter.publishState(); s.pending.shift()([1]); await flush();
  assert.deepEqual(s.packets.pop(), { version: 1, type: 'connection', connected: true, deviceName: 'HUAWEI M-Pencil' }, 'page reload republishes current state');
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
  const b = setup(undefined, true);
  b.adapter.start(); b.pending.shift()([1]); await flush();
  assert.equal(b.permissionRequests(), 1, 'ask only after detecting a pen');
  b.pending.shift()([1]); await flush();
  assert.equal(b.packets.at(-1).batteryPercent, 82);
  assert.equal(b.packets.at(-1).deviceName, 'HUAWEI M-Pencil');
  b.bluetooth.battery = 0;
  b.timers.values().next().value(); b.pending.shift()([1]); await flush();
  assert.equal(b.packets.at(-1).batteryPercent, 0);
  b.bluetooth.battery = -1;
  b.timers.values().next().value(); b.pending.shift()([1]); await flush();
  assert.equal(b.packets.at(-1).batteryPercent, undefined, 'unknown battery is not 0%');
  b.bluetooth.ids = ['AA:AA:AA:AA:AA:AA']; b.bluetooth.name = 'Headphones';
  const count = b.bluetooth.queries;
  b.timers.values().next().value(); b.pending.shift()([1]); await flush();
  assert.equal(b.bluetooth.queries, count, 'never use an unrelated device battery');
  b.bluetooth.ids = ['11:22:33:44:55:66']; b.bluetooth.name = 'HUAWEI M-Pencil';
  b.bluetooth.pending = [];
  b.timers.values().next().value(); b.pending.shift()([1]); await flush();
  const lateBattery = b.bluetooth.pending.shift();
  b.devices.get('change')({}); b.pending.shift()([]); await flush();
  lateBattery({ batteryLevel: 90 }); await flush();
  assert.deepEqual(b.packets.at(-1), { version: 1, type: 'connection', connected: false }, 'late battery must not reconnect a pen');
  b.adapter.stop();

  const denied = setup();
  denied.adapter.start(); denied.pending.shift()([]); await flush();
  assert.equal(denied.permissionRequests(), 0, 'no permission prompt without a pen');
  denied.devices.get('change')({}); denied.pending.shift()([1]); await flush();
  denied.timers.values().next().value(); denied.pending.shift()([1]); await flush();
  assert.equal(denied.permissionRequests(), 1, 'denial is not repeatedly prompted');
  assert.equal(denied.bluetooth.queries, 0);
  assert.equal(denied.packets.at(-1).deviceName, 'HUAWEI M-Pencil', 'InputKit name survives denial');
  denied.adapter.stop();
  console.log('Passed: packets, lifecycle, connection/name/battery updates, unavailable/zero battery, permission denial, stale queries and unrelated devices.');
}
main().catch(error => { console.error(error); process.exitCode = 1; });
