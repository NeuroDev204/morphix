from __future__ import annotations

import hashlib
import importlib.util
import json
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path


SCRIPT: Path = Path(__file__).resolve().parents[1] / "render-dpkg-scripts.py"
SPEC = importlib.util.spec_from_file_location("render_dpkg_scripts", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
render_dpkg_scripts = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = render_dpkg_scripts
SPEC.loader.exec_module(render_dpkg_scripts)


class MaintainerScriptTests(unittest.TestCase):
    def setUp(self) -> None:
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root: Path = Path(temporary.name)
        self.ding_root: Path = self.root / "usr/share/gnome-shell/extensions/ding@rastersoft.com"
        (self.ding_root / "app").mkdir(parents=True)
        self.relative_files: tuple[str, ...] = (
            "app/fileItemMenu.js",
            "app/desktopManager.js",
        )
        self.originals: dict[str, bytes] = {
            self.relative_files[0]: b"original file menu\n",
            self.relative_files[1]: b"original desktop menu\n",
        }
        self.patched: dict[str, bytes] = {
            self.relative_files[0]: b"patched file menu\n",
            self.relative_files[1]: b"patched desktop menu\n",
        }
        for relative, contents in self.originals.items():
            (self.ding_root / relative).write_bytes(contents)

        manifest: dict[str, object] = {
            "package": "gnome-shell-extension-desktop-icons-ng",
            "version": "test-version",
            "source_files": {
                relative: hashlib.sha256(contents).hexdigest()
                for relative, contents in self.originals.items()
            },
            "patched_files": {
                relative: hashlib.sha256(contents).hexdigest()
                for relative, contents in self.patched.items()
            },
        }
        self.manifest_path: Path = self.root / "source.json"
        self.manifest_path.write_text(json.dumps(manifest), encoding="utf-8")
        self.scripts: Path = self.root / "DEBIAN"
        render_dpkg_scripts.render_scripts(self.manifest_path, self.scripts)
        ding_system_path = "/usr/share/gnome-shell/extensions/ding@rastersoft.com"
        for script_name in ("preinst", "postrm"):
            script = self.scripts / script_name
            script.write_text(
                script.read_text(encoding="utf-8").replace(ding_system_path, str(self.ding_root)),
                encoding="utf-8",
            )

        self.admin_dir: Path = self.root / "var/lib/dpkg"
        self.admin_dir.mkdir(parents=True)
        self.bin_dir: Path = self.root / "bin"
        self.bin_dir.mkdir()
        self._write_executable(
            self.bin_dir / "dpkg-divert",
            '#!/bin/sh\nexec /usr/bin/dpkg-divert --admindir "$MORPHIX_TEST_ADMINDIR" "$@"\n',
        )
        self._write_executable(
            self.bin_dir / "dpkg-query",
            '#!/bin/sh\nprintf "%s\\n" "${MORPHIX_TEST_DING_VERSION:-test-version}"\n',
        )
        self._write_executable(
            self.bin_dir / "dpkg",
            '#!/bin/sh\nprintf "%s" "${MORPHIX_TEST_VERIFY_OUTPUT:-}"\n',
        )
        self.environment: dict[str, str] = {
            **os.environ,
            "PATH": f"{self.bin_dir}:/usr/bin:/bin",
            "MORPHIX_TEST_ADMINDIR": str(self.admin_dir),
        }

    @staticmethod
    def _write_executable(path: Path, contents: str) -> None:
        path.write_text(contents, encoding="utf-8")
        path.chmod(0o755)

    def _run_script(self, name: str, *arguments: str, **environment: str) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            [str(self.scripts / name), *arguments],
            env={**self.environment, **environment},
            check=False,
            capture_output=True,
            text=True,
        )

    def _diverted_by_morphix(self, relative: str) -> bool:
        result = subprocess.run(
            [
                "/usr/bin/dpkg-divert",
                "--admindir",
                str(self.admin_dir),
                "--listpackage",
                str(self.ding_root / relative),
            ],
            check=True,
            capture_output=True,
            text=True,
        )
        return result.stdout.strip() == "morphix"

    def test_install_upgrade_and_remove_restore_original_files(self) -> None:
        installed = self._run_script("preinst", "install")
        self.assertEqual(installed.returncode, 0, installed.stderr)
        for relative, original in self.originals.items():
            path = self.ding_root / relative
            backup = Path(f"{path}.morphix-original")
            self.assertFalse(path.exists())
            self.assertEqual(backup.read_bytes(), original)
            self.assertTrue(self._diverted_by_morphix(relative))
            path.write_bytes(self.patched[relative])

        upgraded = self._run_script("preinst", "upgrade", "old-version")
        self.assertEqual(upgraded.returncode, 0, upgraded.stderr)
        untouched = self._run_script("postrm", "upgrade", "new-version")
        self.assertEqual(untouched.returncode, 0, untouched.stderr)
        for relative, patched in self.patched.items():
            self.assertEqual((self.ding_root / relative).read_bytes(), patched)

        for relative in self.relative_files:
            (self.ding_root / relative).unlink()
        removed = self._run_script("postrm", "remove")
        self.assertEqual(removed.returncode, 0, removed.stderr)
        for relative, original in self.originals.items():
            self.assertEqual((self.ding_root / relative).read_bytes(), original)
            self.assertFalse(Path(f"{self.ding_root / relative}.morphix-original").exists())
            self.assertFalse(self._diverted_by_morphix(relative))

        repeated = self._run_script("postrm", "remove")
        self.assertEqual(repeated.returncode, 0, repeated.stderr)

    def test_modified_source_blocks_install_without_diverting_either_file(self) -> None:
        (self.ding_root / self.relative_files[0]).write_text("local edit\n", encoding="utf-8")

        result = self._run_script("preinst", "install")

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("SHA-256 mismatch", result.stderr)
        self.assertFalse(self._diverted_by_morphix(self.relative_files[0]))
        self.assertFalse(self._diverted_by_morphix(self.relative_files[1]))

    def test_unsupported_ding_version_blocks_install(self) -> None:
        result = self._run_script("preinst", "install", MORPHIX_TEST_DING_VERSION="new-version")

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("unsupported DING version", result.stderr)
        self.assertFalse(self._diverted_by_morphix(self.relative_files[0]))

    def test_abort_install_is_noop_when_no_diversion_was_created(self) -> None:
        (self.ding_root / self.relative_files[0]).write_text("local edit\n", encoding="utf-8")

        result = self._run_script("postrm", "abort-install")

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(
            (self.ding_root / self.relative_files[0]).read_text(encoding="utf-8"), "local edit\n"
        )
        self.assertFalse(self._diverted_by_morphix(self.relative_files[0]))

    def test_foreign_diversion_blocks_install_and_preserves_its_backup(self) -> None:
        path = self.ding_root / self.relative_files[0]
        foreign_backup = Path(f"{path}.other-package")
        subprocess.run(
            [
                "/usr/bin/dpkg-divert",
                "--admindir",
                str(self.admin_dir),
                "--add",
                "--rename",
                "--package",
                "other-package",
                "--divert",
                str(foreign_backup),
                str(path),
            ],
            check=True,
            capture_output=True,
            text=True,
        )

        result = self._run_script("preinst", "install")

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("diverted by other-package", result.stderr)
        self.assertEqual(foreign_backup.read_bytes(), self.originals[self.relative_files[0]])
        self.assertFalse(self._diverted_by_morphix(self.relative_files[0]))

    def test_modified_installed_patch_blocks_upgrade(self) -> None:
        self.assertEqual(self._run_script("preinst", "install").returncode, 0)
        (self.ding_root / self.relative_files[0]).write_bytes(self.patched[self.relative_files[0]])
        (self.ding_root / self.relative_files[1]).write_bytes(self.patched[self.relative_files[1]])
        changed_file = str(self.ding_root / self.relative_files[0])

        result = self._run_script(
            "preinst",
            "upgrade",
            "old-version",
            MORPHIX_TEST_VERIFY_OUTPUT=f"??5?????? {changed_file}\n",
        )

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("was modified", result.stderr)
        self.assertTrue(self._diverted_by_morphix(self.relative_files[0]))

    def test_modified_backup_blocks_remove_without_overwriting_or_undiverting(self) -> None:
        self.assertEqual(self._run_script("preinst", "install").returncode, 0)
        backup = Path(f"{self.ding_root / self.relative_files[0]}.morphix-original")
        backup.write_text("unexpected local edit\n", encoding="utf-8")

        result = self._run_script("postrm", "remove")

        self.assertNotEqual(result.returncode, 0)
        self.assertTrue(self._diverted_by_morphix(self.relative_files[0]))
        self.assertEqual(backup.read_text(encoding="utf-8"), "unexpected local edit\n")


if __name__ == "__main__":
    unittest.main()
