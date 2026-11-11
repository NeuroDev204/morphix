"""Kiểm kịch bản Inno Setup của bản exe tải về: không chạm kho chứng chỉ, cài theo người dùng, đăng ký menu qua CLI."""
import json
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
ISS = ROOT / "installer" / "morphix.iss"
# GUID của WebView2 Runtime trong EdgeUpdate\Clients; cố định theo tài liệu của Microsoft.
WEBVIEW2_CLIENT = "{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}"


class InstallerScript(unittest.TestCase):
    def setUp(self) -> None:
        self.text = ISS.read_text(encoding="utf-8-sig")

    def test_touches_no_certificate_store_and_ships_no_sparse_package(self) -> None:
        lowered = self.text.lower()
        for banned in ("certutil", ".cer", "addstore", "msix", "register-package", "sign-package"):
            self.assertNotIn(banned, lowered)
        for obsolete in ("register-package.ps1", "sign-package.ps1"):
            self.assertFalse((ROOT / "installer" / obsolete).exists(), obsolete)

    def test_installs_per_user_without_admin(self) -> None:
        self.assertIn("PrivilegesRequired=lowest", self.text)

    def test_version_comes_from_the_command_line(self) -> None:
        self.assertNotRegex(self.text, r'#define AppVersion "')
        self.assertIn("#ifndef AppVersion", self.text)
        self.assertIn("#error", self.text)
        self.assertIn("AppVersion={#AppVersion}", self.text)

    def test_registers_and_unregisters_the_menu_through_the_cli(self) -> None:
        self.assertIn('Parameters: "install-integration"', self.text)
        self.assertIn('Parameters: "uninstall-integration"', self.text)

    def test_start_menu_shortcut_carries_the_tauri_app_id_so_toasts_show(self) -> None:
        identifier = json.loads((ROOT / "crates/morphix-gui/tauri.conf.json").read_text("utf-8"))[
            "identifier"
        ]
        self.assertIn(f'AppUserModelID: "{identifier}"', self.text)

    def test_installs_webview2_only_when_missing(self) -> None:
        self.assertIn("MicrosoftEdgeWebview2Setup.exe", self.text)
        self.assertIn(WEBVIEW2_CLIENT, self.text)
        self.assertIn("Check: not WebView2Installed", self.text)

    def test_shell_dll_survives_being_locked_by_explorer(self) -> None:
        line = next(l for l in self.text.splitlines() if "morphix_shell.dll" in l)
        self.assertIn("restartreplace", line)
        self.assertIn("uninsrestartdelete", line)


if __name__ == "__main__":
    unittest.main()
