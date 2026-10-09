#!/usr/bin/env python3
"""Check the packaged offline web assets and their ArkWeb references."""

import re
from pathlib import Path

project = Path(__file__).resolve().parents[1]
rawfile = project / "entry/src/main/resources/rawfile"

# The offline package is built by scripts/package_offline.sh; there is no archived
# upstream release copy to compare against, so verify the package is self-consistent.
html = (rawfile / "index.html").read_text(encoding="utf-8")
references = set(re.findall(r"photocraft-web-[a-f0-9]+(?:_bg)?\.(?:js|wasm)", html))
assert len(references) == 2, f"index.html must reference exactly one JS and one WASM: {references}"
assert any(name.endswith(".js") for name in references), references
assert any(name.endswith("_bg.wasm") for name in references), references

packaged = {p.name for p in rawfile.iterdir() if p.is_file()}
expected = references | {"index.html"}
assert packaged == expected, f"rawfile/HTML mismatch: {packaged ^ expected}"

page = (project / "entry/src/main/ets/pages/Index.ets").read_text(encoding="utf-8")
for name in references:
    assert name in page, f"ArkWeb map missing {name}"
assert "application/wasm" in page and "text/javascript" in page
assert "/index.html?webgl" in page
print("Verified offline assets, HTML references, ArkWeb asset map, and MIME types")
