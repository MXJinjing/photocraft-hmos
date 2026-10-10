#!/usr/bin/env python3
"""Verify ARM64-only HAP contents with synthetic ELF headers and real metadata."""
import json
from pathlib import Path
import struct
import tempfile
import unittest
import zipfile

from verify_native import verify

ROOT = Path(__file__).resolve().parents[1]


class NativePackageTests(unittest.TestCase):
    def package(self, path, libraries):
        config = json.loads((ROOT / 'entry/src/main/module.json5').read_text())
        config['app'] = json.loads((ROOT / 'AppScope/app.json5').read_text())['app']
        with zipfile.ZipFile(path, 'w') as archive:
            archive.writestr('module.json', json.dumps(config))
            archive.writestr('resources/base/profile/main_pages.json', json.dumps({'src': ['pages/Index']}))
            archive.writestr('resources/rawfile/arkdata/utd/utd.json5',
                             (ROOT / 'entry/src/main/resources/rawfile/arkdata/utd/utd.json5').read_text())
            for abi, machine in libraries:
                elf = bytearray(64)
                elf[:5] = b'\x7fELF\x02'
                struct.pack_into('<H', elf, 18, machine)
                archive.writestr(f'libs/{abi}/libphotocraft.so', elf)

    def test_arm64_only_package_passes(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'native.hap'
            self.package(path, [('arm64-v8a', 183)])
            verify(path)

    def test_missing_wrong_or_extra_architecture_fails(self):
        for libraries in [[], [('arm64-v8a', 62)], [('x86_64', 62)],
                          [('arm64-v8a', 183), ('x86_64', 62)]]:
            with self.subTest(libraries=libraries), tempfile.TemporaryDirectory() as directory:
                path = Path(directory) / 'native.hap'
                self.package(path, libraries)
                with self.assertRaises(ValueError):
                    verify(path)


if __name__ == '__main__':
    unittest.main()
