#!/usr/bin/env python3
"""Tests for scripts/prepare_craft_i18n.py (craft-i18n + HarmonyOS overlay for the HAP build)."""
import importlib.util
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location("prepare_craft_i18n", ROOT / "scripts/prepare_craft_i18n.py")
prep = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(prep)


class MergeCatalogTest(unittest.TestCase):
    def test_appends_only_missing_rows(self):
        catalog = "# header\n\tOpen\t打开\n"
        overlay = "# HarmonyOS\n\tOpen\t打开\n\tStylus\t手写笔\n"
        text, added, warnings = prep.merge_catalog(catalog, overlay, "zh-hans")
        self.assertEqual(added, 1)
        self.assertEqual(warnings, [])
        self.assertTrue(text.startswith(catalog))
        self.assertEqual(text.count("\tOpen\t"), 1)
        self.assertIn("\tStylus\t手写笔\n", text)

    def test_catalog_translation_wins_and_warns(self):
        text, added, warnings = prep.merge_catalog("\tOpen\t打开\n", "\tOpen\t开启\n", "zh-hans")
        self.assertEqual(added, 0)
        self.assertEqual(text, "\tOpen\t打开\n")
        self.assertEqual(len(warnings), 1)

    def test_context_is_part_of_the_key(self):
        _, added, _ = prep.merge_catalog("\tRed\t红\n", "cameraRaw\tRed\t红色\n")
        self.assertEqual(added, 1)

    def test_rejects_malformed_rows(self):
        with self.assertRaises(ValueError):
            prep.merge_catalog("", "Stylus 手写笔\n")

    def test_catalog_without_trailing_newline(self):
        text, added, _ = prep.merge_catalog("\tOpen\t打开", "\tStylus\t手写笔\n")
        self.assertEqual(added, 1)
        self.assertIn("\tOpen\t打开\n", text)
        self.assertEqual(len(prep.rows(text)), 2)


class MergeTreeTest(unittest.TestCase):
    def make_source(self, root: Path) -> Path:
        src = root / "src"
        for code in ("zh-hans", "ja"):
            (src / "locales" / code).mkdir(parents=True)
            (src / "locales" / code / "photocraft.tsv").write_text("\tOpen\tX\n", encoding="utf-8")
        (src / "locales/zh-hans/common.tsv").write_text("\tOK\t好\n", encoding="utf-8")
        return src

    def test_merges_overlay_without_touching_source(self):
        with tempfile.TemporaryDirectory() as d:
            root = Path(d)
            src = self.make_source(root)
            overlay = root / "overlay"
            overlay.mkdir()
            (overlay / "zh-hans.tsv").write_text("\tStylus\t手写笔\n", encoding="utf-8")
            merged = root / "merged"
            self.assertEqual(prep.merge_tree(src, merged, overlay), 1)
            self.assertIn("手写笔", (merged / "locales/zh-hans/photocraft.tsv").read_text(encoding="utf-8"))
            self.assertNotIn("手写笔", (src / "locales/zh-hans/photocraft.tsv").read_text(encoding="utf-8"))
            self.assertTrue((merged / "locales/zh-hans/common.tsv").is_file())
            self.assertEqual((merged / "locales/ja/photocraft.tsv").read_text(encoding="utf-8"), "\tOpen\tX\n")

    def test_overlay_locale_missing_from_catalogs_fails(self):
        with tempfile.TemporaryDirectory() as d:
            root = Path(d)
            src = self.make_source(root)
            overlay = root / "overlay"
            overlay.mkdir()
            (overlay / "xx.tsv").write_text("\tStylus\tS\n", encoding="utf-8")
            with self.assertRaises(FileNotFoundError):
                prep.merge_tree(src, root / "merged", overlay)

    def test_not_a_checkout_fails(self):
        with tempfile.TemporaryDirectory() as d:
            with self.assertRaises(FileNotFoundError):
                prep.merge_tree(Path(d), Path(d) / "merged", Path(d))


class RepositoryTest(unittest.TestCase):
    def test_rev_is_pinned(self):
        self.assertEqual(len(prep.read_rev()), 40)

    def test_every_overlay_row_parses(self):
        files = sorted(prep.OVERLAY.glob("*.tsv"))
        self.assertEqual(len(files), 16)
        for overlay in files:
            with self.subTest(locale=overlay.stem):
                self.assertTrue(prep.rows(overlay.read_text(encoding="utf-8")))

    def test_cli_uses_explicit_source_offline(self):
        with tempfile.TemporaryDirectory() as d:
            src = Path(d) / "craft-i18n"
            for overlay in prep.OVERLAY.glob("*.tsv"):
                (src / "locales" / overlay.stem).mkdir(parents=True)
                (src / "locales" / overlay.stem / "photocraft.tsv").write_text("", encoding="utf-8")
            env = dict(os.environ, PHOTOCRAFT_CRAFT_I18N_SRC=str(src),
                       PHOTOCRAFT_CRAFT_I18N_CACHE=str(Path(d) / "cache"))
            env.pop("CRAFT_I18N_DIR", None)
            result = subprocess.run([sys.executable, str(ROOT / "scripts/prepare_craft_i18n.py")],
                                    env=env, capture_output=True, text=True)
            self.assertEqual(result.returncode, 0, result.stderr)
            merged = Path(result.stdout.strip())
            self.assertTrue(merged.is_absolute())
            self.assertIn("手写笔", (merged / "locales/zh-hans/photocraft.tsv").read_text(encoding="utf-8"))


if __name__ == "__main__":
    unittest.main()
