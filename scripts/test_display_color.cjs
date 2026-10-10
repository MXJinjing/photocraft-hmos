const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');
const ts = require(process.argv[2] || 'typescript');
const compiled = ts.transpileModule(fs.readFileSync('entry/src/main/ets/DisplayColor.ets', 'utf8'), {
  compilerOptions: { target: ts.ScriptTarget.ES2020, module: ts.ModuleKind.CommonJS }
});
async function scenario(spaces, { reject = false, actual = undefined, queryFails = false, wide = true } = {}) {
  const calls = [];
  let gamut = 0;
  const win = {
    async isWindowSupportWideGamut() { return wide; },
    async setWindowColorSpace(value) {
      calls.push(value);
      if (value === 1 && reject) throw Error('unsupported');
      gamut = value;
    },
    getWindowColorSpace() { return actual ?? gamut; }
  };
  const mod = { exports: {} };
  vm.runInNewContext(compiled.outputText, {
    module: mod, exports: mod.exports, console: { info() {}, warn() {} },
    require: name => name === '@kit.ArkUI' ? {
      display: { getDefaultDisplaySync() {
        if (queryFails) throw Error('display unavailable');
        return { name: 'test', colorSpaces: spaces };
      } },
      window: { getLastWindow: async () => win, ColorSpace: { DEFAULT: 0, WIDE_GAMUT: 1 } }
    } : { colorSpaceManager: { ColorSpace: { DISPLAY_P3: 3, DCI_P3: 2 } } }
  });
  return { p3: await mod.exports.configureDisplayColor({}), calls };
}
(async () => {
  assert.deepEqual(await scenario([4, 3]), { p3: true, calls: [1] });
  assert.deepEqual(await scenario([2]), { p3: true, calls: [1] });
  assert.deepEqual(await scenario([4]), { p3: false, calls: [0] });
  assert.deepEqual(await scenario(undefined), { p3: true, calls: [1] });
  assert.deepEqual(await scenario(undefined, { wide: false }), { p3: false, calls: [0] });
  assert.deepEqual(await scenario([3], { wide: false }), { p3: false, calls: [0] });
  assert.deepEqual(await scenario([3], { reject: true }), { p3: false, calls: [1, 0] });
  assert.deepEqual(await scenario([3], { actual: 0 }), { p3: false, calls: [1] });
  assert.deepEqual(await scenario([3], { queryFails: true }), { p3: false, calls: [0] });
  console.log('Passed: display capabilities, window negotiation, rejection and sRGB fallback.');
})().catch(error => { console.error(error); process.exitCode = 1; });
