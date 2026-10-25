from __future__ import annotations

import hashlib
import importlib.util
import json
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock


SCRIPT: Path = Path(__file__).resolve().parents[1] / "prepare-ding.py"
SPEC = importlib.util.spec_from_file_location("prepare_ding", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
prepare_ding = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = prepare_ding
SPEC.loader.exec_module(prepare_ding)


class PrepareDingTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary_directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary_directory.cleanup)
        self.root = Path(self.temporary_directory.name)
        self.source = self.root / "source"
        (self.source / "app").mkdir(parents=True)
        (self.source / "app" / "fileItemMenu.js").write_text("before file menu\n")
        (self.source / "app" / "desktopManager.js").write_text("before desktop menu\n")
        (self.source / "metadata.json").write_text("{}\n")
        (self.source / "extension.js").write_text("// extension\n")
        self.patch = self.root / "patch.diff"
        self.patch.write_text(
            "--- a/app/fileItemMenu.js\n"
            "+++ b/app/fileItemMenu.js\n"
            "@@ -1 +1 @@\n"
            "-before file menu\n"
            "+Morphix file menu\n"
            "--- a/app/desktopManager.js\n"
            "+++ b/app/desktopManager.js\n"
            "@@ -1 +1 @@\n"
            "-before desktop menu\n"
            "+Morphix desktop menu\n"
        )
        self.license = self.root / "LICENSE"
        self.license.write_text("test license\n")
        hashes: dict[str, str] = {}
        for relative_path in prepare_ding.SOURCE_FILES:
            path = self.source / relative_path
            hashes[relative_path] = hashlib.sha256(path.read_bytes()).hexdigest()
        self.source_manifest = self.root / "source.json"
        self.source_manifest.write_text(
            json.dumps(
                {
                    "package": "gnome-shell-extension-desktop-icons-ng",
                    "version": "test-version",
                    "source": "https://example.invalid/source",
                    "source_files": hashes,
                    "patched_files": {
                        "app/fileItemMenu.js": hashlib.sha256(b"Morphix file menu\n").hexdigest(),
                        "app/desktopManager.js": hashlib.sha256(
                            b"Morphix desktop menu\n"
                        ).hexdigest(),
                    },
                }
            )
        )
        self.output = self.root / "output"
        self.patches = (
            mock.patch.object(prepare_ding, "SOURCE_MANIFEST", self.source_manifest),
            mock.patch.object(prepare_ding, "PATCH_FILE", self.patch),
            mock.patch.object(prepare_ding, "LICENSE_FILE", self.license),
        )
        for patcher in self.patches:
            patcher.start()
            self.addCleanup(patcher.stop)

    def test_rejects_modified_source_without_publishing_output(self) -> None:
        source_file = self.source / "app" / "fileItemMenu.js"
        source_file.write_text("tampered file menu\n")

        with self.assertRaisesRegex(ValueError, "SHA-256 mismatch"):
            prepare_ding.prepare_ding(self.source, self.output)

        self.assertFalse(self.output.exists())

    def test_refuses_existing_output_without_changing_it(self) -> None:
        self.output.mkdir()
        marker = self.output / "keep.txt"
        marker.write_text("keep\n")

        with self.assertRaisesRegex(ValueError, "already exists"):
            prepare_ding.prepare_ding(self.source, self.output)

        self.assertEqual(marker.read_text(), "keep\n")

    def test_applies_patches_and_records_hashes(self) -> None:
        prepare_ding.prepare_ding(self.source, self.output)

        self.assertEqual(
            (self.output / "app" / "fileItemMenu.js").read_text(), "Morphix file menu\n"
        )
        self.assertEqual(
            (self.output / "app" / "desktopManager.js").read_text(),
            "Morphix desktop menu\n",
        )
        manifest = json.loads((self.output / "manifest.json").read_text())
        self.assertEqual(manifest["version"], "test-version")
        self.assertEqual(
            set(manifest["patched_files"]),
            {"app/fileItemMenu.js", "app/desktopManager.js"},
        )
        self.assertEqual((self.output / "LICENSE").read_text(), "test license\n")
        self.assertEqual(
            {path.relative_to(self.output).as_posix() for path in self.output.rglob("*") if path.is_file()},
            {
                "app/fileItemMenu.js",
                "app/desktopManager.js",
                "LICENSE",
                "manifest.json",
            },
        )

    def test_rejects_patch_output_that_does_not_match_the_manifest(self) -> None:
        self.patch.write_text(
            "--- a/app/fileItemMenu.js\n"
            "+++ b/app/fileItemMenu.js\n"
            "@@ -1 +1 @@\n"
            "-before file menu\n"
            "+unexpected file menu\n"
            "--- a/app/desktopManager.js\n"
            "+++ b/app/desktopManager.js\n"
            "@@ -1 +1 @@\n"
            "-before desktop menu\n"
            "+Morphix desktop menu\n"
        )

        with self.assertRaisesRegex(ValueError, "do not match the approved manifest"):
            prepare_ding.prepare_ding(self.source, self.output)

        self.assertFalse(self.output.exists())

    def test_shipping_patch_has_file_and_background_entry_points(self) -> None:
        patch_text = (SCRIPT.parents[1] / "integration/ding/morphix-menu.patch").read_text()

        self.assertIn("a/app/fileItemMenu.js", patch_text)
        self.assertIn("b/app/fileItemMenu.js", patch_text)
        self.assertIn("a/app/desktopManager.js", patch_text)
        self.assertIn("b/app/desktopManager.js", patch_text)
        self.assertEqual(patch_text.count("_('Morphix')"), 2)
        self.assertIn("/usr/bin/morphix", patch_text)
        self.assertIn("--unsupported-selection", patch_text)
        self.assertIn("--uris", patch_text)

    def test_rejects_symlinked_source_file(self) -> None:
        source_file = self.source / "app" / "fileItemMenu.js"
        source_file.unlink()
        source_file.symlink_to(self.source / "app" / "desktopManager.js")

        with self.assertRaisesRegex(ValueError, "symlink"):
            prepare_ding.prepare_ding(self.source, self.output)

        self.assertFalse(self.output.exists())

    def test_refuses_symlink_output(self) -> None:
        target = self.root / "existing"
        target.mkdir()
        self.output.symlink_to(target, target_is_directory=True)

        with self.assertRaisesRegex(ValueError, "symlink"):
            prepare_ding.prepare_ding(self.source, self.output)

        self.assertEqual(list(target.iterdir()), [])

    def test_rejects_patch_that_changes_an_unapproved_file(self) -> None:
        with self.patch.open("a") as patch_file:
            patch_file.write(
                "--- /dev/null\n"
                "+++ b/unexpected.txt\n"
                "@@ -0,0 +1 @@\n"
                "+unexpected\n"
            )

        with self.assertRaisesRegex(ValueError, "outside the two approved"):
            prepare_ding.prepare_ding(self.source, self.output)

        self.assertFalse(self.output.exists())


if __name__ == "__main__":
    unittest.main()
