const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');
const ts = require(process.argv[2] || 'typescript');

function load(file, imports, globals = {}) {
  const module = { exports: {} };
  const result = ts.transpileModule(fs.readFileSync(file, 'utf8'), {
    compilerOptions: { target: ts.ScriptTarget.ES2020, module: ts.ModuleKind.CommonJS }
  });
  assert.deepEqual((result.diagnostics || []).filter(d => d.category === ts.DiagnosticCategory.Error), []);
  vm.runInNewContext(result.outputText, {
    module, exports: module.exports, console,
    canIUse: globals.canIUse || (() => true),
    require: name => {
      if (imports[name]) return imports[name];
      throw new Error(name);
    }
  });
  return module.exports;
}

const calls = [];
const OperationMode = { READ_MODE: 1, WRITE_MODE: 2 };
const PolicyErrorCode = { PERMISSION_NOT_PERSISTED: 4 };
const fileShare = {
  OperationMode, PolicyErrorCode,
  async persistPermission(policies) {
    calls.push(['persist', policies[0].uri, policies[0].operationMode]);
  },
  async activatePermission(policies) {
    calls.push(['activate', policies[0].uri, policies[0].operationMode]);
  }
};

const { persistCapable, isFileUri, persistUri, activateUri, FOLDER_AUTHORIZATION } = load(
  'entry/src/main/ets/persistPermission.ets',
  {
    '@kit.CoreFileKit': { fileShare },
    '@kit.BasicServicesKit': { BusinessError: class {} }
  },
  { canIUse: (cap) => { calls.push(['canIUse', cap]); return true; } }
);

assert.equal(FOLDER_AUTHORIZATION.includes('FolderAuthorization'), true);
assert.equal(persistCapable(), true);
assert.equal(isFileUri('file://docs/a.psd'), true);
assert.equal(isFileUri('file://'), false);
assert.equal(isFileUri('/Download/a.psd'), false);

(async () => {
  assert.equal(await persistUri('file://docs/a.psd', true), true);
  assert.deepEqual(calls.find(c => c[0] === 'persist'), ['persist', 'file://docs/a.psd', 3]);
  assert.equal(await persistUri('/not-a-uri', true), false);
  assert.equal(await activateUri('file://docs/a.psd', false), true);
  assert.ok(calls.some(c => c[0] === 'activate' && c[2] === 1));

  // activate → PERMISSION_NOT_PERSISTED → persist then retry
  let activateAttempts = 0;
  fileShare.activatePermission = async (policies) => {
    activateAttempts++;
    calls.push(['activate-retry', policies[0].uri, activateAttempts]);
    if (activateAttempts === 1) {
      const err = new Error('not persisted');
      err.code = 13900001;
      err.data = [{ code: PolicyErrorCode.PERMISSION_NOT_PERSISTED, uri: policies[0].uri, message: 'np' }];
      throw err;
    }
  };
  assert.equal(await activateUri('file://docs/b.psd', true), true);
  assert.equal(activateAttempts, 2);
  assert.ok(calls.some(c => c[0] === 'persist' && c[1] === 'file://docs/b.psd'));

  const off = load('entry/src/main/ets/persistPermission.ets', {
    '@kit.CoreFileKit': { fileShare },
    '@kit.BasicServicesKit': { BusinessError: class {} }
  }, { canIUse: () => false });
  assert.equal(off.persistCapable(), false);
  assert.equal(await off.persistUri('file://docs/a.psd', true), false);

  console.log('Passed: persistCapable, isFileUri, persist/activate modes, not-persisted retry, capability gate.');
})().catch(error => { console.error(error); process.exitCode = 1; });
