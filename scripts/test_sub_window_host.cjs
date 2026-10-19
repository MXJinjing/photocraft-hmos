const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');
const ts = require(process.argv[2] || 'typescript');

const WindowEventType = { WINDOW_SHOWN: 1, WINDOW_ACTIVE: 2, WINDOW_DESTROYED: 7 };

function load() {
  const compiled = ts.transpileModule(fs.readFileSync('entry/src/main/ets/SubWindowHost.ets', 'utf8'), {
    compilerOptions: { target: ts.ScriptTarget.ES2020, module: ts.ModuleKind.CommonJS }
  });
  const moduleObject = { exports: {} };
  class LocalStorage {
    constructor() { this.values = new Map(); }
    setOrCreate(key, value) { this.values.set(key, value); }
  }
  vm.runInNewContext(compiled.outputText, {
    module: moduleObject, exports: moduleObject.exports, console: { info() {}, error() {} },
    LocalStorage, Promise, Map, Set, Array,
    require: name => name === '@kit.ArkUI'
      ? { window: { WindowEventType } }
      : { BusinessError: Error }
  });
  return moduleObject.exports;
}

function scenario({ failCreate = false } = {}) {
  const api = load();
  const sent = [];
  const created = [];
  const stage = {
    createSubWindowWithOptions: async (name, options) => {
      if (failCreate) throw Object.assign(new Error('no window'), { code: 1300002 });
      const win = {
        name, options, listeners: new Map(), calls: [], destroyed: false,
        on(type, callback) { this.listeners.set(type, callback); },
        resize: async (w, h) => { win.calls.push(['resize', w, h]); },
        moveWindowTo: async (x, y) => { win.calls.push(['move', x, y]); },
        loadContent: async (page, storage) => { win.calls.push(['load', page, storage.values.get(api.SUB_WINDOW_KEY)]); },
        showWindow: async () => { win.calls.push(['show']); },
        setWindowTitle: async title => { win.calls.push(['title', title]); },
        destroyWindow: async () => { win.destroyed = true; }
      };
      created.push(win);
      return win;
    }
  };
  const host = new api.SubWindowHost();
  host.attach(stage, message => sent.push(JSON.parse(message)));
  return { api, host, sent, created };
}
const tick = () => new Promise(resolve => setTimeout(resolve, 0));
const openRequest = (extra = {}) => Object.assign(
  { op: 'open', win: 4, title: 'a — window 4', width: 1600, height: 1200, x: null, y: null }, extra);

(async () => {
  // --- Open: system decoration, sized before content, then shown ---
  {
    const s = scenario();
    s.host.handle(openRequest());
    await tick(); await tick();
    assert.equal(s.created.length, 1);
    const win = s.created[0];
    assert.equal(win.name, 'pc-doc-4');
    assert.equal(win.options.decorEnabled, true, 'system min/max/close come from decorEnabled');
    assert.equal(win.options.title, 'a — window 4');
    assert.deepEqual(win.calls, [['resize', 1600, 1200], ['load', 'pages/SubWindow', 4], ['show']]);
    assert.equal(s.host.count(), 1);

    // Positioned windows are moved before content loads.
    const s2 = scenario();
    s2.host.handle(openRequest({ win: 5, x: 40, y: 60 }));
    await tick(); await tick();
    assert.deepEqual(s2.created[0].calls.slice(0, 2), [['resize', 1600, 1200], ['move', 40, 60]]);

    // --- System close only asks Rust: veto the close, let the app decide ---
    const veto = await win.listeners.get('windowWillClose')();
    assert.equal(veto, true);
    assert.deepEqual(s.sent, [{ kind: 'win_close', win: 4 }]);
    assert.equal(win.destroyed, false);

    // --- Rust closes the viewport: window destroyed without echoing a close back ---
    s.host.handle({ op: 'close', win: 4 });
    await tick();
    assert.equal(win.destroyed, true);
    assert.equal(s.host.count(), 0);
    assert.equal(s.sent.length, 1);
    win.listeners.get('windowEvent')(WindowEventType.WINDOW_DESTROYED);
    assert.equal(s.sent.length, 1, 'our own destroy is not reported as a user close');
  }

  // --- The window disappears behind our back (destroyed by the system) ---
  {
    const s = scenario();
    s.host.handle(openRequest());
    await tick(); await tick();
    s.created[0].listeners.get('windowEvent')(WindowEventType.WINDOW_DESTROYED);
    assert.deepEqual(s.sent, [{ kind: 'win_close', win: 4 }]);
    assert.equal(s.host.count(), 0);
  }

  // --- Close racing creation destroys the new window ---
  {
    const s = scenario();
    s.host.handle(openRequest());
    s.host.handle({ op: 'close', win: 4 });
    await tick(); await tick();
    assert.equal(s.created[0].destroyed, true);
    assert.equal(s.host.count(), 0);
  }

  // --- Failure to create tells Rust to drop the viewport ---
  {
    const s = scenario({ failCreate: true });
    s.host.handle(openRequest());
    await tick(); await tick();
    assert.deepEqual(s.sent, [{ kind: 'win_close', win: 4 }]);
  }

  // --- Title / resize / closeAll ---
  {
    const s = scenario();
    s.host.handle(openRequest());
    s.host.handle(openRequest({ win: 9 }));
    await tick(); await tick();
    s.host.handle({ op: 'title', win: 4, title: 'renamed' });
    s.host.handle({ op: 'resize', win: 4, width: 800, height: 600 });
    await tick();
    assert.deepEqual(s.created[0].calls.slice(-2), [['title', 'renamed'], ['resize', 800, 600]]);
    s.host.closeAll();
    await tick();
    assert.ok(s.created.every(win => win.destroyed));
    assert.equal(s.host.count(), 0);
  }

  // --- Input tagging ---
  {
    const { api } = scenario();
    assert.equal(api.taggedForWindow('{"kind":"blur"}', 3), '{"win":3,"kind":"blur"}');
    assert.equal(api.taggedForWindow('{"kind":"blur"}', 0), '{"kind":"blur"}');
    assert.equal(api.subWindowName(12), 'pc-doc-12');
  }
  console.log('Passed: native sub-window open/decor/close/veto/destroy/race handling and input tagging.');
})().catch(error => { console.error(error); process.exit(1); });
