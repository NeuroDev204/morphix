"""scripts/ci/check_version.py: một nguồn phiên bản là Cargo.toml của workspace."""
import importlib.util
import json
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("check_version", ROOT / "scripts" / "ci" / "check_version.py")
check_version = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(check_version)


def make_tree(cargo_version: str, tauri_version: str) -> Path:
    root = Path(tempfile.mkdtemp())
    (root / "Cargo.toml").write_text(
        f'[workspace]\nmembers = []\n\n[workspace.package]\nversion = "{cargo_version}"\nedition = "2021"\n',
        encoding="utf-8",
    )
    gui = root / "crates" / "morphix-gui"
    gui.mkdir(parents=True)
    (gui / "tauri.conf.json").write_text(json.dumps({"version": tauri_version}), encoding="utf-8")
    return root


class CheckVersion(unittest.TestCase):
    def test_returns_the_workspace_version_when_everything_agrees(self) -> None:
        self.assertEqual(check_version.check(make_tree("0.2.0", "0.2.0"), "v0.2.0"), "0.2.0")
        self.assertEqual(check_version.check(make_tree("0.2.0", "0.2.0"), None), "0.2.0")

    def test_rejects_a_tag_that_differs_from_the_workspace_version(self) -> None:
        with self.assertRaisesRegex(ValueError, "v0.3.0"):
            check_version.check(make_tree("0.2.0", "0.2.0"), "v0.3.0")

    def test_rejects_a_tauri_conf_version_that_drifted(self) -> None:
        with self.assertRaisesRegex(ValueError, "tauri.conf.json"):
            check_version.check(make_tree("0.2.0", "0.1.9"), None)

    def test_the_real_repository_is_consistent(self) -> None:
        self.assertRegex(check_version.check(ROOT, None), r"^\d+\.\d+\.\d+$")


if __name__ == "__main__":
    unittest.main()
