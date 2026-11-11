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

    def test_ffmpeg_is_a_gpl_release_with_the_codecs_morphix_calls(self) -> None:
        # WHY essentials: đủ libx264, libvpx, libopus, libmp3lame, libvorbis mà Morphix gọi, và nhỏ hơn bản full ~143 MB.
        url = read_deps()["FFMPEG_URL"]
        self.assertIn("essentials_build.zip", url)
        version = re.search(r"ffmpeg-(\d+\.\d+\.\d+)-essentials", url).group(1)
        for folder in ("bundle", "store-package"):
            notice = (ROOT / "installer" / folder / "FFMPEG-NOTICE.txt").read_text(encoding="utf-8-sig")
            self.assertIn(version, notice, folder)
            self.assertIn("essentials", notice, folder)
            self.assertIn("libx264", notice, folder)

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


if __name__ == "__main__":
    unittest.main()
