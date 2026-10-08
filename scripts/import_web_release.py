#!/usr/bin/env python3
"""Import the fixed upstream PhotoCraft web release into its archive folder."""

import argparse
import hashlib
import json
import urllib.request
import zipfile
from pathlib import Path

VERSION = "0.3.0"
ARCHIVE_NAME = f"photocraft-web-{VERSION}.zip"
ARCHIVE_URL = (
    "https://github.com/storytold/photocraft/releases/download/"
    f"v{VERSION}/{ARCHIVE_NAME}"
)
ARCHIVE_SHA256 = "1825b2beb2b84331124f6cb78e5eb6eb9584612e8ad6a4e817b7f27cb3f93d06"
PREFIX = f"photocraft-web-{VERSION}/"
WEB_FILES = (
    "index.html",
    "photocraft-web-72921900975c7e6f.js",
    "photocraft-web-72921900975c7e6f_bg.wasm",
)
LICENSE_FILES = ("LICENSE-MIT", "LICENSE-APACHE", "README.md", "HOSTING.md")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--archive", type=Path, help="Use an already downloaded release ZIP")
    args = parser.parse_args()
    project = Path(__file__).resolve().parents[1]
    archive = args.archive.read_bytes() if args.archive else urllib.request.urlopen(
        ARCHIVE_URL, timeout=60
    ).read()
    digest = hashlib.sha256(archive).hexdigest()
    if digest != ARCHIVE_SHA256:
        raise SystemExit(f"Archive SHA-256 mismatch: {digest}")

    release_web = project / "third_party/photocraft/release-web"
    licenses = project / "third_party/photocraft"
    release_web.mkdir(parents=True, exist_ok=True)
    licenses.mkdir(parents=True, exist_ok=True)
    manifest = {
        "version": VERSION,
        "source": ARCHIVE_URL,
        "archive_sha256": digest,
        "files": {},
    }
    from io import BytesIO
    with zipfile.ZipFile(BytesIO(archive)) as zf:
        for name in WEB_FILES + LICENSE_FILES:
            contents = zf.read(PREFIX + name)
            destination = (release_web if name in WEB_FILES else licenses) / name
            destination.write_bytes(contents)
            manifest["files"][name] = hashlib.sha256(contents).hexdigest()
    current_manifest = licenses / "manifest.json"
    if current_manifest.exists():
        old = json.loads(current_manifest.read_text(encoding="utf-8"))
        if "adapted_web" in old:
            manifest["adapted_web"] = old["adapted_web"]
    current_manifest.write_text(
        json.dumps(manifest, indent=2, ensure_ascii=False) + "\n", encoding="utf-8"
    )
    print(f"Imported PhotoCraft web v{VERSION} (SHA-256 verified)")


if __name__ == "__main__":
    main()

