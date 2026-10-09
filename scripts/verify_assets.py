#!/usr/bin/env python3
"""Check upstream and emulator-compatible web assets and ArkWeb references."""

import hashlib
import json
import re
from pathlib import Path

project = Path(__file__).resolve().parents[1]
third_party = project / "third_party/photocraft"
rawfile = project / "entry/src/main/resources/rawfile"
manifest = json.loads((third_party / "manifest.json").read_text())
original_web = third_party / "release-web"
for name, expected in manifest["files"].items():
    source = original_web if name.endswith((".html", ".js", ".wasm")) else third_party
    actual = hashlib.sha256((source / name).read_bytes()).hexdigest()
    assert actual == expected, f"Changed upstream asset: {name}"

adapted = manifest["adapted_web"]
for name, expected in adapted["files"].items():
    actual = hashlib.sha256((rawfile / name).read_bytes()).hexdigest()
    assert actual == expected, f"Changed compatible asset: {name}"
patch = project / adapted["patch"]
assert hashlib.sha256(patch.read_bytes()).hexdigest() == adapted["patch_sha256"]
dependency_patch = project / adapted["dependency_patch"]
assert hashlib.sha256(dependency_patch.read_bytes()).hexdigest() == adapted["dependency_patch_sha256"]

html = (rawfile / "index.html").read_text()
references = set(re.findall(r"photocraft-web-[a-f0-9]+(?:_bg)?\.(?:js|wasm)", html))
assert references == set(adapted["files"]) - {"index.html"}, references
assert {p.name for p in rawfile.iterdir() if p.is_file()} == set(adapted["files"])
page = (project / "entry/src/main/ets/pages/Index.ets").read_text()
for name in references:
    assert name in page, f"ArkWeb map missing {name}"
assert "application/wasm" in page and "text/javascript" in page
assert "/index.html?webgl" in page
print("Verified upstream and compatible hashes, HTML references, ArkWeb asset map, and MIME types")
