#!/usr/bin/env python3
"""Check workflow profile propagation without SDK downloads or changing the checkout."""
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import textwrap
import unittest

ROOT = Path(__file__).resolve().parents[1]
WORKFLOW = ROOT / '.github/workflows/build-hap.yml'


class NativeProfileTests(unittest.TestCase):
    def test_workflow_prepares_matching_cmake_profile(self):
        workflow = WORKFLOW.read_text()
        prepare = workflow.split('      - name: Prepare project\n', 1)[1].split('      - name:', 1)[0]
        self.assertIn('BUILD_MODE: ${{ steps.cfg.outputs.build_mode }}', prepare)
        python = textwrap.dedent(prepare.split("          python3 - <<'PY'\n", 1)[1].split('\n          PY', 1)[0])
        rust_step = workflow.split('      - name: Build Rust native libraries\n', 1)[1].split('      - name:', 1)[0]
        self.assertIn('PHOTOCRAFT_RUST_PROFILE: ${{ steps.cfg.outputs.build_mode }}', rust_step)
        self.assertIn('targets: aarch64-unknown-linux-ohos\n', workflow)
        self.assertNotIn('x86_64-unknown-linux-ohos', workflow)
        cmake = (ROOT / 'entry/src/main/cpp/CMakeLists.txt').read_text()
        self.assertIn('${RUST_TARGET}/${PHOTOCRAFT_RUST_PROFILE}/libphotocraft_hmos.a', cmake)
        for mode in ('debug', 'release'):
            with self.subTest(mode=mode), tempfile.TemporaryDirectory() as directory:
                path = Path(directory)
                (path / 'entry').mkdir()
                (path / 'AppScope').mkdir()
                shutil.copyfile(ROOT / 'entry/build-profile.json5', path / 'entry/build-profile.json5')
                shutil.copyfile(ROOT / 'AppScope/app.json5', path / 'AppScope/app.json5')
                env = dict(os.environ, BUILD_MODE=mode, VERSION_NAME='0.5.0.2', VERSION_CODE='50002')
                subprocess.run(['python3', '-c', python], cwd=path, env=env, check=True, capture_output=True)
                config = json.loads((path / 'entry/build-profile.json5').read_text())
                self.assertIn(f'-DPHOTOCRAFT_RUST_PROFILE={mode}', config['buildOption']['externalNativeOptions']['arguments'])
                self.assertEqual(config['buildOption']['externalNativeOptions']['abiFilters'], ['arm64-v8a'])

    def test_native_script_selects_cargo_profile_for_arm64(self):
        for mode in ('debug', 'release'):
            with self.subTest(mode=mode), tempfile.TemporaryDirectory() as directory:
                path = Path(directory)
                (path / 'scripts').mkdir()
                shutil.copyfile(ROOT / 'scripts/build_native.sh', path / 'scripts/build_native.sh')
                bin_dir = path / 'bin'
                bin_dir.mkdir()
                sdk = path / 'sdk'
                (sdk / 'llvm/bin').mkdir(parents=True)
                (sdk / 'llvm/bin/clang').write_text('#!/bin/sh\nexit 0\n')
                (sdk / 'llvm/bin/clang').chmod(0o755)
                for target in ('aarch64-unknown-linux-ohos',):
                    (path / 'sysroot/lib/rustlib' / target).mkdir(parents=True)
                for name, body in {
                    'rustc': '#!/bin/sh\nprintf "%s\\n" "$TEST_SYSROOT"\n',
                    'cargo': '#!/bin/sh\nprintf "%s\\n" "$*" >> "$TEST_CARGO_LOG"\n',
                }.items():
                    (bin_dir / name).write_text(body)
                    (bin_dir / name).chmod(0o755)
                log = path / 'cargo.log'
                env = dict(os.environ, PATH=str(bin_dir) + os.pathsep + os.environ['PATH'],
                           PHOTOCRAFT_NATIVE_SDK=str(sdk), PHOTOCRAFT_RUST_PROFILE=mode,
                           TEST_SYSROOT=str(path / 'sysroot'), TEST_CARGO_LOG=str(log))
                env.pop('PHOTOCRAFT_NATIVE_TARGETS', None)
                result = subprocess.run(['bash', str(path / 'scripts/build_native.sh')], env=env, capture_output=True, text=True)
                self.assertEqual(result.returncode, 0, result.stderr)
                calls = log.read_text().splitlines()
                self.assertEqual(len(calls), 1)
                for call, target in zip(calls, ('aarch64-unknown-linux-ohos',)):
                    self.assertIn('--target ' + target, call)
                    self.assertEqual('--release' in call.split(), mode == 'release')


if __name__ == '__main__':
    unittest.main()
