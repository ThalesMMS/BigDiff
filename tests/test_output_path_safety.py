import tempfile
import unittest
from pathlib import Path

from bigdiff.core import Options, bigdiff


class OutputPathSafetyTests(unittest.TestCase):
    def test_rejects_symlinked_output_subdirectories(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            base = root / "base"
            target = root / "target"
            out = root / "out"
            escape = root / "escape"

            (base / "nested").mkdir(parents=True)
            (target / "nested").mkdir(parents=True)
            out.mkdir()
            escape.mkdir()

            (base / "nested" / "demo.txt").write_text("old\n", encoding="utf-8")
            (target / "nested" / "demo.txt").write_text("new\n", encoding="utf-8")

            (out / "nested").symlink_to(escape, target_is_directory=True)

            opts = Options(
                normalize_eol=False,
                max_text_size=1_000_000,
                ignore_patterns=[],
                dry_run=False,
            )

            with self.assertRaisesRegex(ValueError, "symlinked output path component"):
                bigdiff(base, target, out, opts)

            self.assertFalse((escape / "demo.txt.modified").exists())

    def test_rejects_symlinked_output_ancestors(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            base = root / "base"
            target = root / "target"
            real_parent = root / "real-parent"
            link_parent = root / "link-parent"
            out = link_parent / "out"

            (base / "nested").mkdir(parents=True)
            (target / "nested").mkdir(parents=True)
            real_parent.mkdir()

            (base / "nested" / "demo.txt").write_text("old\n", encoding="utf-8")
            (target / "nested" / "demo.txt").write_text("new\n", encoding="utf-8")

            link_parent.symlink_to(real_parent, target_is_directory=True)

            opts = Options(
                normalize_eol=False,
                max_text_size=1_000_000,
                ignore_patterns=[],
                dry_run=False,
            )

            with self.assertRaisesRegex(ValueError, "symlinked output path component"):
                bigdiff(base, target, out, opts)

            self.assertFalse((real_parent / "out" / "nested" / "demo.txt.modified").exists())


if __name__ == "__main__":
    unittest.main()
