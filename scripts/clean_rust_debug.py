#!/usr/bin/env python3
"""Bound the native adapter's host debug cache without touching target builds.

Called after each successful native build. Clear the entire host debug cache
(including Cargo fingerprints). Never remove Cargo's lock file: concurrent
builds must continue to lock the same inode. ARM64 and release outputs stay.
"""

import argparse
import fcntl
import os
from pathlib import Path
import shutil
import stat
import time


DEBUG = Path(__file__).resolve().parents[1] / "native/rust/target/debug"


def clean(debug, limit_bytes, idle_seconds, dry_run=False):
    if not debug.exists():
        print("Skip: host debug cache does not exist.")
        return
    # Refuse redirected paths so cleanup stays inside this checkout.
    if debug.resolve() != debug.absolute():
        raise RuntimeError("Refusing a symlinked debug cache path")
    lock_path = debug / ".cargo-lock"
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
                if path == lock_path:
                    continue
                info = path.lstat()
                size += info.st_blocks * 512
                newest = max(newest, info.st_mtime)
        gib = size / (1024 ** 3)
        if size <= limit_bytes:
            print(f"Skip: host debug cache is {gib:.2f} GiB, below limit.")
            return
        if time.time() - newest < idle_seconds:
            print(f"Skip: {gib:.2f} GiB cache was updated within the idle interval.")
            return
        if dry_run:
            print(f"Would clear {gib:.2f} GiB from {debug}.")
            return
        for path in debug.iterdir():
            if path == lock_path:
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
    clean(DEBUG, args.max_gib * 1024 ** 3, args.idle_hours * 3600, args.dry_run)
