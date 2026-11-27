from __future__ import annotations

import hashlib
import io
import subprocess
import tarfile
import tempfile
import time
import unittest
from pathlib import Path


SCRIPT: Path = Path(__file__).resolve().parents[1] / "install.sh"
ASSET: str = "morphix-x86_64-linux.tar.gz"

# Lệnh giả: ghi lời gọi vào $FAKE_LOG; hành vi điều khiển bằng biến FAKE_*.
STUBS: dict[str, str] = {
    "uname": 'case "$1" in -s) echo Linux ;; -m) echo "${FAKE_ARCH:-x86_64}" ;; *) echo Linux ;; esac\n',
    "getconf": 'echo "glibc ${FAKE_GLIBC:-2.39}"\n',
    "ldconfig": (
        'echo "1 libs found in cache"\n'
        '[ -n "${FAKE_NO_WEBKIT:-}" ] || printf "\\tlibwebkit2gtk-4.1.so.0 (libc6,x86-64) => /usr/lib/libwebkit2gtk-4.1.so.0\\n"\n'
        'printf "\\tlibayatana-appindicator3.so.1 (libc6,x86-64) => /usr/lib/libayatana-appindicator3.so.1\\n"\n'
    ),
    "dpkg-query": '[ -n "${FAKE_DEB:-}" ] && { echo "install ok installed"; exit 0; }; exit 1\n',
    "pgrep": 'echo "pgrep $*" >> "$FAKE_LOG"; [ -n "${FAKE_GUI_RUNNING:-}" ]\n',
    "pkill": 'echo "pkill $* bin=$(sed -n 2p "$MORPHIX_PREFIX/bin/morphix" 2>/dev/null)" >> "$FAKE_LOG"; exit 0\n',
    "sudo": 'echo "sudo $* stdin=$(wc -c)" >> "$FAKE_LOG"; [ -z "${FAKE_SUDO_FAIL:-}" ]\n',
    "apt-get": 'echo "apt-get $* stdin=$(wc -c)" >> "$FAKE_LOG"\n',
    "id": 'echo "${FAKE_UID:-1000}"\n',
    "gnome-extensions": (
        'echo "gnome-extensions $*" >> "$FAKE_LOG"\n'
        'case "$1 ${2:-}" in\n'
        '  "list --enabled") printf "%s\\n" "${FAKE_EXT_ENABLED:-}" ;;\n'
        '  "list "*) printf "%s\\n" "${FAKE_EXT_ALL:-}" ;;\n'
        '  "enable "*) [ -z "${FAKE_EXT_ENABLE_FAIL:-}" ] ;;\n'
        'esac\n'
    ),
    "gsettings": (
        'case "$1" in\n'
        '  get) echo "${FAKE_GSETTINGS:-@as []}" ;;\n'
        '  set) echo "gsettings $*" >> "$FAKE_LOG" ;;\n'
        'esac\n'
    ),
    "curl": (
        'out=""; url=""\n'
        'while [ $# -gt 0 ]; do case "$1" in -o) out="$2"; shift 2 ;; -*) shift ;; *) url="$1"; shift ;; esac; done\n'
        'echo "curl $url" >> "$FAKE_LOG"\n'
        'cp "${url#file://}" "$out"\n'
    ),
}


def build_release(directory: Path, version: str, corrupt_checksum: bool = False) -> None:
    files: dict[str, tuple[bytes, int]] = {
        "morphix/bin/morphix": (
            f'#!/bin/sh\n# NEW {version}\necho "morphix $*" >> "$FAKE_LOG"\n'.encode(),
            0o755,
        ),
        "morphix/bin/morphix-gui": (b'#!/bin/sh\necho "morphix-gui started" >> "$FAKE_LOG"\n', 0o755),
        "morphix/lib/morphix/ffmpeg": (b"ffmpeg", 0o755),
        "morphix/lib/morphix/pdfium/libpdfium.so": (b"pdfium", 0o644),
        "morphix/share/icons/hicolor/128x128/apps/morphix.png": (b"png", 0o644),
        "morphix/share/doc/morphix/LICENSE": (b"GPL", 0o644),
        "morphix/VERSION": (f"{version}\n".encode(), 0o644),
    }
    buffer = io.BytesIO()
    with tarfile.open(fileobj=buffer, mode="w:gz") as archive:
        for name, (data, mode) in files.items():
            info = tarfile.TarInfo(name)
            info.size = len(data)
            info.mode = mode
            archive.addfile(info, io.BytesIO(data))
    payload: bytes = buffer.getvalue()
    (directory / ASSET).write_bytes(payload)
    digest: str = hashlib.sha256(b"tampered" if corrupt_checksum else payload).hexdigest()
    (directory / f"{ASSET}.sha256").write_text(f"{digest}  {ASSET}\n")


class InstallScriptTests(unittest.TestCase):
    def setUp(self) -> None:
        self.tmp = tempfile.TemporaryDirectory()
        root = Path(self.tmp.name)
        self.release: Path = root / "release"
        self.prefix: Path = root / "home/.local"
        self.fakebin: Path = root / "fakebin"
        self.log: Path = root / "calls.log"
        self.os_release: Path = root / "os-release"
        for path in (self.release, self.fakebin):
            path.mkdir(parents=True)
        for name, body in STUBS.items():
            stub = self.fakebin / name
            stub.write_text("#!/bin/sh\n" + body)
            stub.chmod(0o755)
        self.os_release.write_text("ID=ubuntu\nID_LIKE=debian\n")
        self.log.write_text("")
        build_release(self.release, "0.2.0")
        # Mặc định: máy đã có binding Nautilus và không có tty, để test cũ không bị hỏi gì.
        self.binding_dir: Path = root / "nautilus-ext"
        self.binding_dir.mkdir()
        (self.binding_dir / "libnautilus-python.so").write_text("")
        self.empty_dir: Path = root / "empty"
        self.empty_dir.mkdir()
        self.tty: Path = root / "tty"

    def tearDown(self) -> None:
        self.tmp.cleanup()

    def run_install(self, piped: bool = False, **env: str) -> subprocess.CompletedProcess[str]:
        base: dict[str, str] = {
            "PATH": f"{self.fakebin}:/usr/bin:/bin",
            "HOME": str(self.prefix.parent),
            "MORPHIX_PREFIX": str(self.prefix),
            "MORPHIX_RELEASE_BASE": f"file://{self.release}",
            "MORPHIX_OS_RELEASE": str(self.os_release),
            "MORPHIX_SKIP_INTEGRATION": "1",
            "FAKE_LOG": str(self.log),
            "MORPHIX_TTY": str(self.tty),
            "MORPHIX_NAUTILUS_DIRS": str(self.binding_dir),
        }
        base.update(env)
        if piped:
            # Như `curl … | sh`: script đến từ stdin.
            return subprocess.run(["sh"], input=SCRIPT.read_text(), env=base, capture_output=True, text=True, timeout=60)
        return subprocess.run(["sh", str(SCRIPT)], env=base, capture_output=True, text=True, timeout=60)

    def add_nautilus(self) -> None:
        stub = self.fakebin / "nautilus"
        stub.write_text("#!/bin/sh\n")
        stub.chmod(0o755)

    def basic_path(self) -> str:
        basic = Path(self.tmp.name) / "basic-bin"
        if not basic.exists():
            basic.mkdir()
            for tool in ("sh", "grep", "sort", "head", "mktemp", "sha256sum", "tar", "gzip", "cat",
                         "mkdir", "rm", "mv", "cp", "sleep", "nohup", "sed", "df", "wc", "tr", "cut"):
                real = subprocess.run(["sh", "-c", f"command -v {tool}"], capture_output=True, text=True).stdout.strip()
                (basic / tool).symlink_to(real)
        return f"{self.fakebin}:{basic}"

    def install_old(self) -> bytes:
        old: bytes = b"#!/bin/sh\n# OLD\n"
        (self.prefix / "bin").mkdir(parents=True)
        (self.prefix / "bin/morphix").write_bytes(old)
        (self.prefix / "lib/morphix").mkdir(parents=True)
        (self.prefix / "lib/morphix/.morphix-install").write_text("0.1.0\n")
        (self.prefix / "lib/morphix/stale-file").write_text("old")
        return old

    def test_fresh_install_layout(self) -> None:
        result = self.run_install()
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        for relative in (
            "bin/morphix",
            "bin/morphix-gui",
            "lib/morphix/ffmpeg",
            "lib/morphix/pdfium/libpdfium.so",
            "share/icons/hicolor/128x128/apps/morphix.png",
            "share/doc/morphix/LICENSE",
        ):
            self.assertTrue((self.prefix / relative).is_file(), relative)
        self.assertEqual((self.prefix / "lib/morphix/.morphix-install").read_text(), "0.2.0\n")
        self.assertIn("Đã cài Morphix 0.2.0", result.stdout)

    def test_update_replaces_lib_dir(self) -> None:
        self.install_old()
        result = self.run_install()
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertFalse((self.prefix / "lib/morphix/stale-file").exists())
        self.assertIn(b"# NEW 0.2.0", (self.prefix / "bin/morphix").read_bytes())
        self.assertFalse((self.prefix / "lib/morphix.old").exists())
        self.assertFalse((self.prefix / "lib/morphix.new").exists())

    def test_checksum_mismatch_keeps_previous_install(self) -> None:
        old = self.install_old()
        build_release(self.release, "0.2.0", corrupt_checksum=True)
        result = self.run_install()
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual((self.prefix / "lib/morphix/.morphix-install").read_text(), "0.1.0\n")
        self.assertEqual((self.prefix / "bin/morphix").read_bytes(), old)

    def test_missing_webkit_prints_distro_command(self) -> None:
        self.os_release.write_text('NAME="Fedora Linux"\nID=fedora\n')
        result = self.run_install(FAKE_NO_WEBKIT="1")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("sudo dnf install webkit2gtk4.1", result.stdout + result.stderr)
        self.assertFalse(self.prefix.exists())

    def test_deb_install_blocks(self) -> None:
        result = self.run_install(FAKE_DEB="1")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("sudo apt remove morphix", result.stdout + result.stderr)
        self.assertFalse(self.prefix.exists())

    def test_old_glibc_and_arm_rejected(self) -> None:
        for env in ({"FAKE_GLIBC": "2.31"}, {"FAKE_ARCH": "aarch64"}):
            result = self.run_install(**env)
            self.assertNotEqual(result.returncode, 0, env)
            self.assertFalse(self.prefix.exists(), env)

    def test_update_restarts_running_gui(self) -> None:
        self.install_old()
        result = self.run_install(FAKE_GUI_RUNNING="1")
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        kills = [line for line in self.log.read_text().splitlines() if line.startswith("pkill -x morphix-gui")]
        self.assertTrue(kills, self.log.read_text())
        self.assertIn("bin=# OLD", kills[0])
        deadline = time.monotonic() + 5
        while "morphix-gui started" not in self.log.read_text() and time.monotonic() < deadline:
            time.sleep(0.1)
        self.assertIn("morphix-gui started", self.log.read_text())

    def test_finds_ldconfig_outside_user_path(self) -> None:
        # WHY: ldconfig nằm ở /sbin; PATH của người dùng thường trên Debian/openSUSE không có sbin.
        (self.fakebin / "ldconfig").unlink()
        result = self.run_install()
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)

    def test_refuses_foreign_lib_dir(self) -> None:
        foreign = self.prefix / "lib/morphix"
        foreign.mkdir(parents=True)
        (foreign / "data").write_text("của ứng dụng khác")
        result = self.run_install()
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual((foreign / "data").read_text(), "của ứng dụng khác")
        self.assertFalse((self.prefix / "bin/morphix").exists())

    def test_refuses_foreign_binary(self) -> None:
        (self.prefix / "bin").mkdir(parents=True)
        (self.prefix / "bin/morphix").write_text("khác")
        result = self.run_install()
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual((self.prefix / "bin/morphix").read_text(), "khác")

    def test_needs_only_basic_tools(self) -> None:
        # WHY: ảnh tối giản (openSUSE Tumbleweed) không có awk; script phải chạy với bộ lệnh cơ bản.
        result = self.run_install(PATH=self.basic_path())
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)

    def test_refuses_when_disk_is_too_full_and_changes_nothing(self) -> None:
        # WHY: thiếu chỗ giữa chừng làm `mv` lỗi và để lại bản cài dở dang; kiểm dung lượng trước khi ghi gì.
        self.install_old()
        before = {p: p.read_bytes() for p in (self.prefix / "bin/morphix",)}
        (self.fakebin / "df").write_text("#!/bin/sh\nprintf 'Filesystem 1K-blocks Used Available Use%% Mounted\\n/x 100 99 1 99%% /\\n'\n")
        (self.fakebin / "df").chmod(0o755)
        result = self.run_install()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("dung lượng", result.stdout + result.stderr)
        self.assertEqual((self.prefix / "bin/morphix").read_bytes(), before[self.prefix / "bin/morphix"])
        self.assertFalse((self.prefix / "lib/morphix.new").exists())
        self.assertEqual(list(self.prefix.glob(".morphix-stage*")), [])

    def test_path_hint(self) -> None:
        result = self.run_install()
        self.assertIn("export PATH=", result.stdout)

    def test_integration_runs_unless_skipped(self) -> None:
        result = self.run_install(MORPHIX_SKIP_INTEGRATION="")
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("morphix install-integration", self.log.read_text())


    def sudo_calls(self) -> list[str]:
        return [line for line in self.log.read_text().splitlines() if line.startswith("sudo ")]

    def test_missing_nautilus_binding_installed_after_yes(self) -> None:
        self.add_nautilus()
        self.tty.write_text("\n")
        result = self.run_install(piped=True, MORPHIX_NAUTILUS_DIRS=str(self.empty_dir))
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertEqual(self.sudo_calls(), ["sudo apt-get install -y python3-nautilus stdin=0"])
        self.assertIn("Đóng hết cửa sổ Files", result.stdout)
        self.assertIn("Đã cài Morphix 0.2.0", result.stdout)

    def test_declined_binding_still_installs(self) -> None:
        self.add_nautilus()
        self.tty.write_text("n\n")
        result = self.run_install(piped=True, MORPHIX_NAUTILUS_DIRS=str(self.empty_dir))
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertEqual(self.sudo_calls(), [])
        self.assertIn("sudo apt install python3-nautilus", result.stdout + result.stderr)
        self.assertIn("Scripts", result.stdout + result.stderr)
        self.assertIn("Đã cài Morphix 0.2.0", result.stdout)

    def test_no_tty_never_runs_sudo(self) -> None:
        result = self.run_install(piped=True, FAKE_NO_WEBKIT="1")
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(self.sudo_calls(), [])
        self.assertIn("sudo apt install libwebkit2gtk-4.1-0", result.stdout + result.stderr)
        self.assertFalse(self.prefix.exists())

    def test_missing_webkit_installed_after_yes(self) -> None:
        self.tty.write_text("y\n")
        result = self.run_install(piped=True, FAKE_NO_WEBKIT="1")
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertEqual(self.sudo_calls(), ["sudo apt-get install -y libwebkit2gtk-4.1-0 stdin=0"])
        self.assertTrue((self.prefix / "bin/morphix").is_file())

    def test_failed_required_install_stops(self) -> None:
        self.tty.write_text("\n")
        result = self.run_install(piped=True, FAKE_NO_WEBKIT="1", FAKE_SUDO_FAIL="1")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("sudo apt install libwebkit2gtk-4.1-0", result.stdout + result.stderr)
        self.assertFalse(self.prefix.exists())

    def test_root_installs_without_sudo(self) -> None:
        self.add_nautilus()
        self.tty.write_text("\n")
        result = self.run_install(piped=True, FAKE_UID="0", MORPHIX_NAUTILUS_DIRS=str(self.empty_dir))
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertEqual(self.sudo_calls(), [])
        self.assertIn("apt-get install -y python3-nautilus stdin=0", self.log.read_text())

    def test_no_nautilus_no_prompt(self) -> None:
        self.tty.write_text("\n")
        result = self.run_install(PATH=self.basic_path(), MORPHIX_NAUTILUS_DIRS=str(self.empty_dir))
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertEqual(self.sudo_calls(), [])
        self.assertNotIn("cần cài thêm", result.stdout + result.stderr)


    def test_unknown_distro_missing_library_keeps_old_message(self) -> None:
        self.os_release.write_text("ID=void\n")
        self.tty.write_text("\n")
        result = self.run_install(piped=True, FAKE_NO_WEBKIT="1")
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(self.sudo_calls(), [])
        self.assertIn("libwebkit2gtk-4.1.so.0", result.stdout + result.stderr)
        self.assertFalse(self.prefix.exists())

    def test_unknown_distro_missing_binding_still_installs(self) -> None:
        self.os_release.write_text("ID=void\n")
        self.add_nautilus()
        self.tty.write_text("\n")
        result = self.run_install(piped=True, MORPHIX_NAUTILUS_DIRS=str(self.empty_dir))
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertEqual(self.sudo_calls(), [])
        self.assertIn("nautilus-python", result.stderr)

    def test_without_sudo_does_not_ask(self) -> None:
        # Debian chỉ có mật khẩu root, không cài sudo: hỏi "Cài luôn bằng sudo?" rồi bảo chạy sudo là vô lý.
        (self.fakebin / "sudo").unlink()
        self.add_nautilus()
        self.tty.write_text("\n")
        result = self.run_install(piped=True, PATH=self.basic_path(), MORPHIX_NAUTILUS_DIRS=str(self.empty_dir))
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertNotIn("Cài luôn", result.stdout)
        self.assertIn("python3-nautilus", result.stdout)

    def test_no_tty_still_names_missing_packages(self) -> None:
        result = self.run_install(piped=True, FAKE_NO_WEBKIT="1")
        self.assertIn("Morphix cần cài thêm: libwebkit2gtk-4.1-0", result.stdout)


    # --- AppIndicator của GNOME ---
    RG = "appindicatorsupport@rgcjonas.gmail.com"

    def gnome(self, **env: str) -> subprocess.CompletedProcess[str]:
        return self.run_install(piped=True, XDG_CURRENT_DESKTOP="ubuntu:GNOME", **env)

    def test_debian_installs_and_enables_appindicator(self) -> None:
        self.os_release.write_text("ID=debian\n")
        self.tty.write_text("\n")
        result = self.gnome(FAKE_EXT_ENABLE_FAIL="1", FAKE_GSETTINGS="['ding@rastersoft.com']")
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertEqual(self.sudo_calls(), ["sudo apt-get install -y gnome-shell-extension-appindicator stdin=0"])
        self.assertIn(
            f"gsettings set org.gnome.shell enabled-extensions ['ding@rastersoft.com', '{self.RG}']",
            self.log.read_text(),
        )
        self.assertIn("Đăng xuất rồi đăng nhập lại", result.stdout)

    def test_ubuntu_uses_ubuntu_extension_package(self) -> None:
        self.tty.write_text("\n")
        result = self.gnome()
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertEqual(self.sudo_calls(), ["sudo apt-get install -y gnome-shell-ubuntu-extensions stdin=0"])
        self.assertIn("gnome-extensions enable ubuntu-appindicators@ubuntu.com", self.log.read_text())

    def test_gsettings_from_empty_list(self) -> None:
        self.os_release.write_text("ID=fedora\n")
        self.tty.write_text("\n")
        result = self.gnome(FAKE_EXT_ENABLE_FAIL="1")
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn(f"gsettings set org.gnome.shell enabled-extensions ['{self.RG}']", self.log.read_text())

    def test_installed_but_disabled_is_enabled_without_sudo(self) -> None:
        self.tty.write_text("\n")
        result = self.gnome(FAKE_EXT_ALL=f"ding@rastersoft.com\n{self.RG}")
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertEqual(self.sudo_calls(), [])
        self.assertIn(f"gnome-extensions enable {self.RG}", self.log.read_text())
        self.assertNotIn("Cài luôn", result.stdout)

    def test_already_enabled_does_nothing(self) -> None:
        self.tty.write_text("\n")
        result = self.gnome(FAKE_EXT_ENABLED="ubuntu-appindicators@ubuntu.com")
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertEqual(self.sudo_calls(), [])
        self.assertNotIn("gnome-extensions enable", self.log.read_text())

    def test_not_gnome_skips_appindicator(self) -> None:
        self.tty.write_text("\n")
        result = self.run_install(piped=True, XDG_CURRENT_DESKTOP="KDE")
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertEqual(self.sudo_calls(), [])
        self.assertNotIn("gnome-extensions", self.log.read_text())

    def test_opensuse_without_package_prints_hint(self) -> None:
        self.os_release.write_text('ID="opensuse-tumbleweed"\nID_LIKE="opensuse suse"\n')
        self.tty.write_text("\n")
        result = self.gnome()
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertEqual(self.sudo_calls(), [])
        self.assertIn("extensions.gnome.org", result.stdout + result.stderr)

    def test_nautilus_binding_names_per_distro(self) -> None:
        cases = {
            "ID=arch\n": "sudo pacman -S --needed --noconfirm nautilus-python stdin=0",
            'ID="opensuse-tumbleweed"\nID_LIKE="opensuse suse"\n': "sudo zypper --non-interactive install python3-nautilus stdin=0",
        }
        self.add_nautilus()
        for os_release, expected in cases.items():
            self.os_release.write_text(os_release)
            self.tty.write_text("\n")
            self.log.write_text("")
            result = self.run_install(piped=True, MORPHIX_NAUTILUS_DIRS=str(self.empty_dir))
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
            self.assertEqual(self.sudo_calls(), [expected], os_release)


if __name__ == "__main__":
    unittest.main()
