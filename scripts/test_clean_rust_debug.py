#!/usr/bin/env python3
"""Exercise cache deletion, retained libraries and concurrent-build protection."""
import contextlib
import fcntl
import importlib.util
import io
from pathlib import Path
import tempfile
import unittest

spec = importlib.util.spec_from_file_location('cleanup', Path(__file__).with_name('clean_rust_debug.py'))
cleanup = importlib.util.module_from_spec(spec)
spec.loader.exec_module(cleanup)


class CleanupTests(unittest.TestCase):
    def artifact(self, directory):
        (directory / 'deps').mkdir(parents=True)
        artifact = directory / 'deps/test.rlib'
        artifact.write_bytes(b'x' * 8192)
        return artifact

    def test_keep_library_and_lock_inode(self):
        with tempfile.TemporaryDirectory() as tmp:
            directory = Path(tmp).resolve() / 'debug'
            artifact = self.artifact(directory)
            library = directory / 'libphotocraft_hmos.a'
            library.write_bytes(b'library')
            lock = directory / '.cargo-lock'
            lock.touch()
            inode = lock.stat().st_ino
            cleanup.clean(directory, 0, 0, keep_names={library.name})
            self.assertFalse(artifact.exists())
            self.assertEqual(library.read_bytes(), b'library')
            self.assertEqual(lock.stat().st_ino, inode)

    def test_skip_busy_cargo_lock(self):
        with tempfile.TemporaryDirectory() as tmp:
            directory = Path(tmp).resolve() / 'debug'
            artifact = self.artifact(directory)
            with (directory / '.cargo-lock').open('w') as lock:
                fcntl.flock(lock, fcntl.LOCK_EX)
                cleanup.clean(directory, 0, 0)
                self.assertTrue(artifact.exists())

    def test_dry_run_and_thresholds(self):
        with tempfile.TemporaryDirectory() as tmp:
            directory = Path(tmp).resolve() / 'debug'
            artifact = self.artifact(directory)
            for kwargs in ({'dry_run': True}, {'limit_bytes': 10**9}, {'idle_seconds': 3600}):
                args = dict(limit_bytes=0, idle_seconds=0)
                args.update(kwargs)
                cleanup.clean(directory, **args)
                self.assertTrue(artifact.exists())

    def test_refuse_redirected_profile(self):
        with tempfile.TemporaryDirectory() as tmp:
            outside = Path(tmp).resolve() / 'outside'
            artifact = self.artifact(outside)
            redirected = Path(tmp).resolve() / 'debug'
            redirected.symlink_to(outside, target_is_directory=True)
            with self.assertRaises(RuntimeError):
                cleanup.clean(redirected, 0, 0)
            self.assertTrue(artifact.exists())

    def test_refuse_redirected_target(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp).resolve()
            (root / 'native/rust').mkdir(parents=True)
            outside = root / 'outside'
            outside.mkdir()
            (root / 'native/rust/target').symlink_to(outside, target_is_directory=True)
            with self.assertRaises(RuntimeError):
                list(cleanup.profiles(root))


if __name__ == '__main__':
    with contextlib.redirect_stdout(io.StringIO()):
        unittest.main()
