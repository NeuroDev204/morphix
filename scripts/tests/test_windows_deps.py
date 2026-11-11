"""Phụ thuộc Windows đã ghim, notice khớp nguồn, và kịch bản tải kiểm SHA256."""
import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
DEPS = ROOT / "packaging" / "windows-deps.txt"
SETUP = ROOT / "scripts" / "ci" / "setup-windows-deps.ps1"


def read_deps() -> dict:
    deps = {}
    for line in DEPS.read_text(encoding="utf-8").splitlines():
        match = re.match(r"^([A-Z0-9_]+)=(.+)$", line.strip())
        if match:
            deps[match.group(1)] = match.group(2)
    return deps


class WindowsDeps(unittest.TestCase):
    def test_every_download_is_pinned_by_https_url_and_sha256(self) -> None:
        deps = read_deps()
        for name in ("PDFIUM", "FFMPEG"):
            self.assertTrue(deps[f"{name}_URL"].startswith("https://"), name)
            self.assertRegex(deps[f"{name}_SHA256"], r"^[0-9a-f]{64}$", name)
        self.assertTrue(deps["WEBVIEW2_URL"].startswith("https://"))

    def test_ffmpeg_is_the_slim_morphix_build_hosted_in_this_repository(self) -> None:
        # WHY bản gọn riêng: bản dựng sẵn của bên thứ ba nặng 105-172 MB vì chứa hàng chục codec Morphix không gọi.
        deps = read_deps()
        url = deps["FFMPEG_URL"]
        self.assertRegex(
            url,
            r"^https://github\.com/NeuroDev204/morphix/releases/download/ffmpeg-(\d+\.\d+\.\d+)-morphix\.(\d+)/ffmpeg-\1-morphix\.\2-win64-gpl\.zip$",
        )
        tag = re.search(r"/download/(ffmpeg-[^/]+)/", url).group(1)
        linux = (ROOT / "packaging" / "ffmpeg-static.txt").read_text(encoding="utf-8")
        self.assertIn(f"/download/{tag}/", linux)
        self.assertRegex(linux, r"(?m)^URL:\s+https://github\.com/NeuroDev204/morphix/releases/download/.+linux64-gpl\.tar\.xz$")
        self.assertRegex(linux, r"(?m)^SHA256:\s+[0-9a-f]{64}$")
        for folder in ("bundle", "store-package"):
            notice = (ROOT / "installer" / folder / "FFMPEG-NOTICE.txt").read_text(encoding="utf-8-sig")
            for needle in ("morphix", "libx264", "libvpx", "libopus", "libmp3lame", "libvorbis", "libdav1d"):
                self.assertIn(needle, notice.lower(), f"{folder}: {needle}")

    def test_the_build_scripts_pin_every_source_by_sha256(self) -> None:
        windows = (ROOT / "scripts" / "build-ffmpeg-slim-windows.sh").read_text(encoding="utf-8")
        entries = re.findall(r'^\s+"([^|"]+)\|https://[^|"]+\|([0-9a-f]{64})"$', windows, re.M)
        names = [name for name, _ in entries]
        for expected in ("ffmpeg-", "zlib-", "libogg-", "libvorbis-", "lame-", "opus-", "x264-", "libvpx-", "dav1d-"):
            self.assertTrue(any(n.startswith(expected) for n in names), expected)
        linux = (ROOT / "scripts" / "build-ffmpeg-slim.sh").read_text(encoding="utf-8")
        self.assertRegex(linux, r"FFMPEG_SHA256='[0-9a-f]{64}'")
        self.assertRegex(linux, r"DAV1D_SHA256='[0-9a-f]{64}'")
        # Cùng một nguồn ffmpeg cho cả hai nền tảng.
        ffmpeg_sha = re.search(r"FFMPEG_SHA256='([0-9a-f]{64})'", linux).group(1)
        self.assertIn(ffmpeg_sha, windows)

    def test_licences_cover_every_component_of_the_slim_ffmpeg(self) -> None:
        licences = ROOT / "packaging" / "ffmpeg-licenses"
        for name in ("ffmpeg", "x264", "libvpx", "opus", "lame", "libogg", "libvorbis", "dav1d", "zlib"):
            folder = licences / name
            self.assertTrue((folder / "SOURCE").is_file(), name)
            texts = [p for p in folder.iterdir() if p.name != "SOURCE"]
            self.assertTrue(texts and all(p.stat().st_size > 100 for p in texts), name)
        self.assertFalse((ROOT / "packaging" / "ffmpeg-static-licenses").exists())
        helper = (ROOT / "scripts" / "install-ffmpeg-slim.sh").read_text(encoding="utf-8")
        self.assertIn("packaging/ffmpeg-licenses", helper)
        for script in (ROOT / "scripts").glob("*.sh"):
            self.assertNotIn("ffmpeg-static-licenses", script.read_text(encoding="utf-8"), script.name)

    def test_setup_script_verifies_hashes_and_the_webview2_signature(self) -> None:
        text = SETUP.read_text(encoding="utf-8-sig")
        self.assertIn("Get-FileHash", text)
        self.assertIn("SHA256", text)
        self.assertIn("Get-AuthenticodeSignature", text)
        self.assertIn("Microsoft Corporation", text)
        for destination in (r"installer\bundle", r"installer\store-package", r"third_party\pdfium\lib"):
            self.assertIn(destination, text)
        for artifact in ("ffmpeg.exe", "pdfium.dll", "MicrosoftEdgeWebview2Setup.exe"):
            self.assertIn(artifact, text)

    def test_downloaded_binaries_are_never_committed(self) -> None:
        ignore = (ROOT / ".gitignore").read_text(encoding="utf-8")
        for pattern in ("installer/bundle/*.exe", "installer/bundle/*.dll", "installer/store-package/*.exe",
                        "installer/store-package/*.dll", "installer/output/"):
            self.assertIn(pattern, ignore)


class LinuxPackagesTests(unittest.TestCase):
    def test_tarball_and_deb_ship_the_same_slim_ffmpeg_through_one_script(self) -> None:
        for name in ("build-tarball.sh", "build-deb.sh"):
            text = (ROOT / "scripts" / name).read_text(encoding="utf-8")
            self.assertIn("scripts/install-ffmpeg-slim.sh", text, name)
            self.assertNotIn("build-ffmpeg-sidecar", text, name)
        helper = (ROOT / "scripts" / "install-ffmpeg-slim.sh").read_text(encoding="utf-8")
        for needle in ("packaging/ffmpeg-static.txt", "sha256sum -c", "packaging/ffmpeg-licenses", "BUILD-INFO.txt", "--enable-libx264"):
            self.assertIn(needle, helper)

    def test_obsolete_ubuntu_ffmpeg_sidecar_is_gone(self) -> None:
        for relative in (
            "scripts/build-ffmpeg-sidecar.sh",
            "scripts/build-ffmpeg-jammy.sh",
            "crates/morphix-gui/packaging/deb/rely-packages.txt",
            "crates/morphix-gui/packaging/deb/rely-packages-jammy.txt",
            "packaging/ffmpeg-source-offer.txt",
            "packaging/ffmpeg-source-offer-jammy.txt",
        ):
            self.assertFalse((ROOT / relative).exists(), relative)
        release = (ROOT / ".github" / "workflows" / "release.yml").read_text(encoding="utf-8")
        self.assertNotIn("ffmpeg=", release)

    def test_deb_check_requires_a_static_ffmpeg_and_its_licences(self) -> None:
        check = (ROOT / "scripts" / "check-deb.sh").read_text(encoding="utf-8")
        for needle in ("./usr/share/doc/morphix/ffmpeg/BUILD-INFO.txt", "./usr/share/doc/morphix/ffmpeg/third-party/x264/COPYING", "not a dynamic executable"):
            self.assertIn(needle, check)
        self.assertNotIn("rely-packages", check)


class TarballFloorTests(unittest.TestCase):
    def test_glibc_floor_check_tolerates_static_binaries(self) -> None:
        # WHY: ffmpeg gọn liên kết tĩnh hoàn toàn nên `objdump -T` báo lỗi trên nó; trước đây điều đó làm cả script dựng tarball dừng.
        import os, shutil, subprocess, tempfile
        script = (ROOT / "scripts" / "build-tarball.sh").read_text(encoding="utf-8")
        start = script.index('floor="$(find')
        end = script.index('if [ "$(printf', start)
        fragment = script[start:end]
        stage = Path(tempfile.mkdtemp())
        (stage / "bin").mkdir()
        (stage / "lib").mkdir()
        shutil.copy("/bin/ls", stage / "bin" / "dynamic")
        (stage / "lib" / "static-or-data").write_bytes(b"khong phai file ELF dong")
        result = subprocess.run(
            ["bash", "-c", "set -euo pipefail\n" + fragment + '\necho "floor=$floor"'],
            env={**os.environ, "STAGE": str(stage)}, capture_output=True, text=True,
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertRegex(result.stdout, r"floor=2\.\d+")


if __name__ == "__main__":
    unittest.main()
