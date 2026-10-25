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

    def tearDown(self) -> None:
        self.tmp.cleanup()

    def run_install(self, **env: str) -> subprocess.CompletedProcess[str]:
        base: dict[str, str] = {
            "PATH": f"{self.fakebin}:/usr/bin:/bin",
            "HOME": str(self.prefix.parent),
            "MORPHIX_PREFIX": str(self.prefix),
            "MORPHIX_RELEASE_BASE": f"file://{self.release}",
            "MORPHIX_OS_RELEASE": str(self.os_release),
            "MORPHIX_SKIP_INTEGRATION": "1",
            "FAKE_LOG": str(self.log),
        }
        base.update(env)
        return subprocess.run(["sh", str(SCRIPT)], env=base, capture_output=True, text=True, timeout=60)

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
        basic = Path(self.tmp.name) / "basic-bin"
        basic.mkdir()
        for tool in ("sh", "grep", "sort", "head", "mktemp", "sha256sum", "tar", "gzip", "cat",
                     "mkdir", "rm", "mv", "cp", "sleep", "nohup", "sed", "df", "wc", "tr", "cut"):
            real = subprocess.run(["sh", "-c", f"command -v {tool}"], capture_output=True, text=True).stdout.strip()
            (basic / tool).symlink_to(real)
        result = self.run_install(PATH=f"{self.fakebin}:{basic}")
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


if __name__ == "__main__":
    unittest.main()
