#!/usr/bin/env python3
"""Prepare the craft-i18n catalogs that the HarmonyOS native build embeds.

Upstream `crates/ui-egui/build.rs` embeds interface translations from `CRAFT_I18N_DIR`, a
storytold/craft-i18n checkout at the commit in `upstream/photocraft/craft-i18n.rev`. Without it
the UI is English only. This script, run by `build_native.sh`, leaves `build.rs` untouched and:

1. takes a craft-i18n source tree: `PHOTOCRAFT_CRAFT_I18N_SRC` if set (an existing checkout, for
   offline builds), otherwise the pinned revision's tarball from GitHub, cached per revision;
2. copies it to `<cache>/<rev>/merged` and appends the HarmonyOS-only rows of
   `native/i18n/hmos-overlay/<locale>.tsv` to `locales/<locale>/photocraft.tsv`. A row already in
   the catalog (same context and English) keeps the catalog's translation; the overlay only fills
   gaps, and a differing translation is reported as a warning;
3. prints the absolute merged directory, for `CRAFT_I18N_DIR`.
"""
from __future__ import annotations

import io
import os
import shutil
import sys
import tarfile
import tempfile
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
REV_FILE = ROOT / "upstream/photocraft/craft-i18n.rev"
OVERLAY = ROOT / "native/i18n/hmos-overlay"
APP_CATALOG = "photocraft.tsv"
TARBALL = "https://codeload.github.com/storytold/craft-i18n/tar.gz/{rev}"
STAMP = ".photocraft-overlay-stamp"


def log(message: str) -> None:
    print(f"prepare_craft_i18n: {message}", file=sys.stderr)


def read_rev(path: Path = REV_FILE) -> str:
    rev = path.read_text(encoding="utf-8").strip()
    if len(rev) != 40 or any(c not in "0123456789abcdef" for c in rev):
        raise ValueError(f"{path} does not hold a full commit SHA: {rev!r}")
    return rev


def rows(text: str) -> list[tuple[str, str, str]]:
    """Catalog rows as (context, English, translation); comments and blanks skipped."""
    out = []
    for number, line in enumerate(text.splitlines(), 1):
        if not line.strip() or line.startswith("#"):
            continue
        cols = line.split("\t")
        if len(cols) != 3:
            raise ValueError(f"line {number}: expected 3 TAB-separated columns, got {len(cols)}")
        out.append((cols[0], cols[1], cols[2]))
    return out


def merge_catalog(catalog: str, overlay: str, locale: str = "") -> tuple[str, int, list[str]]:
    """Append overlay rows missing from catalog. Returns (text, rows added, warnings)."""
    existing = {(c, e): t for c, e, t in rows(catalog)}
    added, warnings, seen = [], [], set()
    for context, english, translation in rows(overlay):
        key = (context, english)
        if key in seen:
            continue
        seen.add(key)
        if key in existing:
            if existing[key] != translation:
                warnings.append(f"{locale}: keeping craft-i18n's translation of {english!r}")
            continue
        added.append(f"{context}\t{english}\t{translation}")
    if not added:
        return catalog, 0, warnings
    text = catalog if catalog.endswith("\n") or not catalog else catalog + "\n"
    text += "\n# HarmonyOS host additions (native/i18n/hmos-overlay)\n" + "\n".join(added) + "\n"
    return text, len(added), warnings


def merge_tree(source: Path, merged: Path, overlay_dir: Path = OVERLAY) -> int:
    """Copy source to merged and apply every overlay locale. Returns rows added."""
    if not (source / "locales").is_dir():
        raise FileNotFoundError(f"{source} is not a craft-i18n checkout (no locales/)")
    tmp = merged.with_name(merged.name + ".tmp")
    shutil.rmtree(tmp, ignore_errors=True)
    shutil.copytree(source, tmp, ignore=shutil.ignore_patterns(".git"))
    total = 0
    for overlay in sorted(overlay_dir.glob("*.tsv")):
        target = tmp / "locales" / overlay.stem / APP_CATALOG
        if not target.is_file():
            raise FileNotFoundError(f"overlay locale {overlay.stem} has no {APP_CATALOG} in craft-i18n")
        text, count, warnings = merge_catalog(
            target.read_text(encoding="utf-8"), overlay.read_text(encoding="utf-8"), overlay.stem)
        for warning in warnings:
            log(f"warning: {warning}")
        target.write_text(text, encoding="utf-8")
        total += count
    shutil.rmtree(merged, ignore_errors=True)
    tmp.rename(merged)
    return total


def download(rev: str, dest: Path) -> None:
    url = TARBALL.format(rev=rev)
    log(f"downloading craft-i18n {rev[:8]}")
    with urllib.request.urlopen(url, timeout=120) as response:
        data = response.read()
    tmp = Path(tempfile.mkdtemp(prefix="craft-i18n-", dir=dest.parent))
    try:
        with tarfile.open(fileobj=io.BytesIO(data), mode="r:gz") as archive:
            for member in archive.getmembers():
                parts = Path(member.name).parts[1:]
                if not parts or ".." in parts or member.issym() or member.islnk():
                    continue
                member.name = str(Path(*parts))
                if hasattr(tarfile, "data_filter"):
                    archive.extract(member, tmp, filter="data")
                else:
                    archive.extract(member, tmp)
        if not (tmp / "locales").is_dir():
            raise RuntimeError(f"{url} has no locales/ directory")
        shutil.rmtree(dest, ignore_errors=True)
        tmp.rename(dest)
    finally:
        shutil.rmtree(tmp, ignore_errors=True)


def overlay_stamp(rev: str, source: Path) -> str:
    parts = [rev, str(source)]
    for overlay in sorted(OVERLAY.glob("*.tsv")):
        stat = overlay.stat()
        parts.append(f"{overlay.name}:{stat.st_size}:{stat.st_mtime_ns}")
    return "\n".join(parts)


def prepare(cache: Path) -> Path:
    rev = read_rev()
    base = cache / rev
    base.mkdir(parents=True, exist_ok=True)
    # An existing checkout: PHOTOCRAFT_CRAFT_I18N_SRC, or a CRAFT_I18N_DIR the caller already set
    # (the overlay is still merged on top of it).
    explicit = (os.environ.get("PHOTOCRAFT_CRAFT_I18N_SRC", "").strip()
                or os.environ.get("CRAFT_I18N_DIR", "").strip())
    if explicit:
        source = Path(explicit).expanduser().resolve()
    else:
        source = base / "src"
        if not (source / "locales").is_dir():
            download(rev, source)
    merged = base / "merged"
    stamp = overlay_stamp(rev, source)
    stamp_file = merged / STAMP
    if not explicit and stamp_file.is_file() and stamp_file.read_text(encoding="utf-8") == stamp:
        return merged
    added = merge_tree(source, merged)
    (merged / STAMP).write_text(stamp, encoding="utf-8")
    log(f"craft-i18n {rev[:8]} ready with {added} HarmonyOS overlay rows")
    return merged


def main() -> int:
    cache = Path(os.environ.get("PHOTOCRAFT_CRAFT_I18N_CACHE", ROOT / "native/i18n/.craft-i18n-cache"))
    try:
        print(prepare(cache.resolve()))
    except Exception as error:  # noqa: BLE001 - one clear message for the shell script
        log(f"error: {error}")
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
