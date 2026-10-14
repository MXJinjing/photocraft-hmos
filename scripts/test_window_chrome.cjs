const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');
const ts = require(process.argv[2] || 'typescript');

const WindowStatusType = { UNDEFINED: 0, FULL_SCREEN: 1, MAXIMIZE: 2, MINIMIZE: 3, FLOATING: 4 };

function runScenario(options = {}) {
  const storage = new Map();
  const timers = new Map();
  const listeners = new Map();
  let serial = 0;
  let status = options.status ?? WindowStatusType.FLOATING;
  let rect = options.rect ?? { width: 120, height: 38 };
  let fail = false;
  let decorCalls = 0;
  let decorThrow = options.decorThrow ?? false;
  const win = {
    setWindowDecorHeight() {
      decorCalls++;
      if (decorThrow) { const err = new Error('abnormal'); err.code = 1300002; throw err; }
    },
    setWindowDecorVisible() {
      if (decorThrow) { const err = new Error('abnormal'); err.code = 1300002; throw err; }
    },
    setWindowTitleMoveEnabled() {
      if (decorThrow) { const err = new Error('abnormal'); err.code = 1300002; throw err; }
    },
    on: (key, callback) => listeners.set(key, callback),
    off: key => listeners.delete(key),
    getWindowStatus: () => status,
    getTitleButtonRect: () => { if (fail) throw Error('transient'); return rect; }
  };
  const moduleObject = { exports: {} };
  const compiled = ts.transpileModule(fs.readFileSync('entry/src/main/ets/WindowChrome.ets', 'utf8'), {
    compilerOptions: { target: ts.ScriptTarget.ES2020, module: ts.ModuleKind.CommonJS }
  });
  vm.runInNewContext(compiled.outputText, {
    module: moduleObject, exports: moduleObject.exports, console: { info() {} },
    AppStorage: { setOrCreate: (key, value) => storage.set(key, value) },
    setTimeout: callback => { timers.set(++serial, callback); return serial; },
    clearTimeout: id => timers.delete(id),
    require: name => name === '@kit.ArkUI'
      ? { window: { WindowStatusType } }
      : { deviceInfo: { deviceType: options.deviceType ?? 'tablet' }, BusinessError: Error }
  });
  const chrome = new moduleObject.exports.WindowChrome(win);
  return {
    chrome, storage, timers, listeners, win,
    get status() { return status; },
    set status(value) { status = value; },
    get rect() { return rect; },
    set rect(value) { rect = value; },
    get fail() { return fail; },
    set fail(value) { fail = value; },
    get decorThrow() { return decorThrow; },
    set decorThrow(value) { decorThrow = value; },
    get decorCalls() { return decorCalls; },
    flushTimers() { for (const callback of [...timers.values()]) callback(); }
  };
}

// --- Existing recovery regressions (realistic WindowStatusType values) ---
{
  const s = runScenario();
  s.chrome.start();
  assert.equal(s.storage.get('photocraftTitleRight'), 128);
  s.rect = { width: 0, height: 0 };
  s.listeners.get('windowStatusChange')(WindowStatusType.FLOATING);
  assert.equal(s.storage.get('photocraftTitleRight'), 128, 'transition zero retains valid geometry');
  s.fail = true;
  s.chrome.refresh();
  assert.equal(s.storage.get('photocraftTitleRight'), 128, 'query failure retains inset');
  s.fail = false;
  s.rect = { width: 150, height: 38 };
  s.flushTimers();
  assert.equal(s.storage.get('photocraftTitleRight'), 158, 'delayed geometry is recovered without another event');
  s.status = WindowStatusType.FULL_SCREEN;
  s.listeners.get('windowSizeChange')();
  assert.equal(s.storage.get('photocraftTitleRight'), 158, 'visible buttons override lagging status');
  s.rect = { width: 0, height: 0 };
  s.chrome.refresh();
  assert.equal(s.storage.get('photocraftTitleRight'), 0, 'non-floating window with no buttons clears inset');
  s.chrome.stop();
  assert.equal(s.timers.size, 0);
  assert.equal(s.listeners.size, 0);
}

// --- Start outside freeform: decor APIs throw, listeners still watch; enter freeform recovers ---
{
  const s = runScenario({
    status: WindowStatusType.FULL_SCREEN,
    rect: { width: 0, height: 0 },
    decorThrow: true
  });
  s.chrome.start();
  assert.equal(s.listeners.size, 3, 'status/size/rect listeners survive decor failure outside freeform');
  assert.equal(s.storage.get('photocraftTitleRight') ?? 0, 0, 'fullscreen start keeps zero inset');
  // Enter freeform: decor becomes available, caption rect arrives on a delayed retry.
  s.decorThrow = false;
  s.status = WindowStatusType.FLOATING;
  s.listeners.get('windowStatusChange')(WindowStatusType.FLOATING);
  assert.ok(s.decorCalls >= 2, 'decor is re-applied on freeform status change');
  assert.equal(s.storage.get('photocraftTitleRight') ?? 0, 0, 'empty freeform transition retains prior zero until geometry exists');
  s.rect = { width: 139, height: 37 };
  s.flushTimers();
  assert.equal(s.storage.get('photocraftTitleRight'), 147, 'freeform entry recovers caption clearance after delayed rect');
  assert.equal(s.storage.get('photocraftTitleHeight'), 38);
  s.chrome.stop();
}

// --- Start fullscreen without throwing; size-driven freeform entry also re-applies decor ---
{
  const s = runScenario({
    status: WindowStatusType.FULL_SCREEN,
    rect: { width: 0, height: 0 }
  });
  s.chrome.start();
  assert.notEqual(
    s.listeners.get('windowStatusChange'),
    s.listeners.get('windowSizeChange'),
    'status and size must use distinct typed callbacks for ArkTS overloads'
  );
  const decorAfterStart = s.decorCalls;
  s.status = WindowStatusType.FLOATING;
  s.rect = { width: 120, height: 38 };
  s.listeners.get('windowSizeChange')({ width: 1280, height: 800 });
  assert.ok(s.decorCalls > decorAfterStart, 'size change into freeform re-applies decor');
  assert.equal(s.storage.get('photocraftTitleRight'), 128, 'size-driven freeform entry publishes caption inset');
  s.chrome.stop();
}

// --- Phone / non-tablet: chrome stays idle ---
{
  const s = runScenario({ deviceType: 'phone' });
  s.chrome.start();
  assert.equal(s.listeners.size, 0);
  assert.equal(s.storage.get('photocraftTitleRight'), undefined);
}

console.log('Passed: title geometry transitions, freeform-entry recovery after fullscreen start, distinct status/size callbacks and cleanup.');
