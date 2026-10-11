#!/usr/bin/env python3
"""Clear this checkout's Rust build caches after a successful native build.

Keep the ARM64 static libraries consumed by HAP/CMake. Clear native host/target
intermediates and the separate upstream UI test caches. Acquire Cargo's profile
locks and preserve their inodes; busy or redirected paths are never cleaned.
The next Rust build recompiles dependencies rather than retaining large caches.
"""

import argparse
import fcntl
import os
from pathlib import Path
import shutil
import stat
import time


ROOT = Path(__file__).resolve().parents[1]
DEBUG = ROOT / "native/rust/target/debug"


def profiles(root=ROOT):
    for relative in ("native/rust/target", "upstream/photocraft/target"):
        target = root / relative
        if not target.exists():
            continue
        if target.resolve() != target.absolute():
            raise RuntimeError("Refusing a symlinked target path")
        parents = [target]
        parents.extend(path for path in target.iterdir()
                       if path.is_dir() and path.name not in ("debug", "release") and not path.name.startswith("."))
        for parent in parents:
            for profile in ("debug", "release"):
                directory = parent / profile
                if directory.exists():
                    keep = {"libphotocraft_hmos.a"} if (relative == "native/rust/target" and
                            parent.name == "aarch64-unknown-linux-ohos") else set()
                    yield directory, keep


def clean(debug, limit_bytes, idle_seconds, dry_run=False, keep_names=()):
    if not debug.exists():
        print("Skip: cache does not exist.")
        return
    # Refuse redirected paths so cleanup stays inside this checkout.
    if debug.resolve() != debug.absolute():
        raise RuntimeError("Refusing a symlinked debug cache path")
    preserved = {debug / name for name in keep_names}
    lock_path = debug / ".cargo-lock"
    preserved.add(lock_path)
    fd = os.open(lock_path, os.O_RDWR | os.O_CREAT | os.O_NOFOLLOW, 0o600)
    with os.fdopen(fd, "r+") as lock:
        try:
            fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError:
            print("Skip: Cargo build lock is busy.")
            return
        size = 0
        newest = 0
        for root, dirs, files in os.walk(debug, followlinks=False):
            for name in dirs + files:
                path = Path(root) / name
                if path in preserved:
                    continue
                info = path.lstat()
                size += info.st_blocks * 512
                newest = max(newest, info.st_mtime)
        gib = size / (1024 ** 3)
        if size <= limit_bytes:
            print(f"Skip: Rust cache is {gib:.2f} GiB, below limit.")
            return
        if time.time() - newest < idle_seconds:
            print(f"Skip: {gib:.2f} GiB cache was updated within the idle interval.")
            return
        if dry_run:
            print(f"Would clear {gib:.2f} GiB from {debug}.")
            return
        for path in debug.iterdir():
            if path in preserved:
                continue
            if stat.S_ISDIR(path.lstat().st_mode):
                shutil.rmtree(path)
            else:
                path.unlink()
        print(f"Cleared {gib:.2f} GiB from {debug}.")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--max-gib", type=float, default=0)
    parser.add_argument("--idle-hours", type=float, default=0)
    parser.add_argument("--dry-run", action="store_true")
    args = parser.parse_args()
    if args.max_gib < 0 or args.idle_hours < 0:
        parser.error("Size and idle thresholds must be nonnegative")
    for directory, keep in profiles():
        clean(directory, args.max_gib * 1024 ** 3, args.idle_hours * 3600, args.dry_run, keep)
