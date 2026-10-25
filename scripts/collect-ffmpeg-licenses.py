"""Lấy file giấy phép của mọi thư viện mà bản ffmpeg tĩnh BtbN đóng kèm, đúng commit BtbN đã dựng.

Usage: python3 scripts/collect-ffmpeg-licenses.py <thư mục clone FFmpeg-Builds ở tag đã ghim> <thư mục đầu ra>
WHY: giấy phép BSD/MIT/ISC của các thư viện (dav1d, libvpx, aom...) yêu cầu giữ nguyên văn bản thông báo
khi phân phối bản dựng; ffmpeg tĩnh chỉ mang LICENSE.txt của FFmpeg.
"""

from __future__ import annotations

import re
import subprocess
import sys
import tempfile
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

# WHY bỏ qua: chỉ dùng cho bản Windows/mingw, không có trong bản linux64.
WINDOWS_ONLY: set[str] = {"10-mingw", "10-mingw-std-threads", "50-schannel"}
# WHY: bốn thư viện này không đặt giấy phép ở thư mục gốc; văn bản nằm ở file dưới đây.
LICENSE_ELSEWHERE: dict[str, list[str]] = {
    "45-x11__60-libglvnd": ["README.md"],
    "50-avisynth": ["distrib/gpl.txt"],
    "50-ffnvcodec": ["include/ffnvcodec/nvEncodeAPI.h"],
    "50-vaapi__40-libdrm": ["xf86drm.c"],
}
LICENSE_NAME = re.compile(r"^(licen[cs]e|copying|copyright|notice|authors)([._-].*)?$", re.IGNORECASE)


def components(builds: Path) -> list[tuple[str, str, str]]:
    found: list[tuple[str, str, str]] = []
    for script in sorted(builds.joinpath("scripts.d").rglob("*.sh")):
        name = str(script.relative_to(builds / "scripts.d")).removesuffix(".sh").replace("/", "__")
        if name in WINDOWS_ONLY:
            continue
        text = script.read_text()
        repo = re.search(r'^SCRIPT_REPO="([^"]+)"', text, re.MULTILINE)
        commit = re.search(r'^SCRIPT_COMMIT="([^"]+)"', text, re.MULTILINE)
        if repo and commit:
            found.append((name, repo.group(1), commit.group(1)))
    return found


def git(*args: str, cwd: Path) -> subprocess.CompletedProcess[str]:
    return subprocess.run(["git", *args], cwd=cwd, capture_output=True, text=True, timeout=600)


def fetch(component: tuple[str, str, str], out: Path) -> str:
    name, repo, commit = component
    with tempfile.TemporaryDirectory() as tmp:
        work = Path(tmp)
        git("init", "-q", cwd=work)
        fetched = git("fetch", "-q", "--depth", "1", "--filter=blob:none", repo, commit, cwd=work)
        if fetched.returncode != 0:
            return f"LỖI {name}: không tải được {repo}@{commit}: {fetched.stderr.strip()[:200]}"
        listing = git("ls-tree", "--name-only", "FETCH_HEAD", cwd=work).stdout.split()
        licenses = [entry for entry in listing if LICENSE_NAME.match(entry)] + LICENSE_ELSEWHERE.get(name, [])
        if not licenses:
            return f"LỖI {name}: không thấy file giấy phép ở gốc {repo}@{commit}"
        target = out / name
        target.mkdir(parents=True, exist_ok=True)
        (target / "SOURCE").write_text(f"{repo}\n{commit}\n")
        for entry in licenses:
            blob = subprocess.run(["git", "show", f"FETCH_HEAD:{entry}"], cwd=work, capture_output=True, timeout=600)
            if blob.returncode != 0:
                return f"LỖI {name}: không đọc được {entry}"
            (target / entry.replace("/", "__")).write_bytes(blob.stdout)
        return f"ok {name}: {', '.join(licenses)}"


def main() -> int:
    builds, out = Path(sys.argv[1]), Path(sys.argv[2])
    out.mkdir(parents=True, exist_ok=True)
    with ThreadPoolExecutor(max_workers=8) as pool:
        results = list(pool.map(lambda c: fetch(c, out), components(builds)))
    for line in results:
        print(line)
    return 1 if any(line.startswith("LỖI") for line in results) else 0


if __name__ == "__main__":
    sys.exit(main())
