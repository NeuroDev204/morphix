#!/usr/bin/env python3
"""Lấy văn bản giấy phép của mọi thành phần trong ffmpeg gọn của Morphix, từ đúng các tarball nguồn đã ghim.

Usage: collect-ffmpeg-licenses.py <thư mục chứa các tarball nguồn> <thư mục đầu ra>
WHY: giấy phép BSD/LGPL/GPL của các thư viện đóng kèm yêu cầu giữ nguyên văn bản thông báo khi phân phối bản dựng.
Danh sách thành phần khớp scripts/build-ffmpeg-slim.sh và scripts/build-ffmpeg-slim-windows.sh.
"""
from __future__ import annotations

import re
import sys
import tarfile
from pathlib import Path

LICENSE_NAME = re.compile(r"^(licen[cs]e|copying|patents|notice)([._-].*)?$", re.IGNORECASE)

# (thư mục đầu ra, tiền tố tên tarball, giấy phép, trang chủ, file giấy phép chỉ lấy ở gốc)
COMPONENTS: list[tuple[str, str, str, str]] = [
    ("ffmpeg", "ffmpeg-", "GPL-3.0-or-later (bật --enable-gpl vì có libx264)", "https://ffmpeg.org"),
    ("x264", "x264-", "GPL-2.0-or-later", "https://www.videolan.org/developers/x264.html"),
    ("libvpx", "libvpx-", "BSD-3-Clause (kèm PATENTS)", "https://www.webmproject.org"),
    ("opus", "opus-", "BSD-3-Clause", "https://opus-codec.org"),
    ("lame", "lame-", "LGPL-2.0-or-later", "https://lame.sourceforge.io"),
    ("libogg", "libogg-", "BSD-3-Clause", "https://xiph.org/ogg/"),
    ("libvorbis", "libvorbis-", "BSD-3-Clause", "https://xiph.org/vorbis/"),
    ("dav1d", "dav1d-", "BSD-2-Clause", "https://code.videolan.org/videolan/dav1d"),
    ("zlib", "zlib-", "Zlib", "https://zlib.net"),
]


def find_tarball(source_dir: Path, prefix: str) -> Path:
    matches = sorted(p for p in source_dir.iterdir() if p.name.startswith(prefix) and ".tar" in p.name)
    if not matches:
        raise SystemExit(f"không thấy tarball nguồn {prefix}* trong {source_dir}")
    return matches[0]


def collect(source_dir: Path, out_dir: Path) -> None:
    out_dir.mkdir(parents=True, exist_ok=True)
    for name, prefix, license_id, homepage in COMPONENTS:
        tarball = find_tarball(source_dir, prefix)
        target = out_dir / name
        target.mkdir(exist_ok=True)
        copied: list[str] = []
        with tarfile.open(tarball) as archive:
            for member in archive.getmembers():
                parts = Path(member.name).parts
                # Chỉ file ở gốc của cây nguồn: parts[0] là thư mục gốc của tarball.
                if len(parts) == 2 and member.isfile() and LICENSE_NAME.match(parts[1]):
                    (target / parts[1]).write_bytes(archive.extractfile(member).read())
                    copied.append(parts[1])
        if not copied:
            raise SystemExit(f"{tarball.name} không có file giấy phép ở gốc")
        (target / "SOURCE").write_text(
            f"Thành phần: {name}\nGiấy phép: {license_id}\nTrang chủ: {homepage}\nNguồn đã dùng: {tarball.name}\n"
            f"File giấy phép: {', '.join(sorted(copied))}\n",
            encoding="utf-8",
        )
        print(f"{name}: {', '.join(sorted(copied))}")


if __name__ == "__main__":
    if len(sys.argv) != 3:
        raise SystemExit(__doc__)
    collect(Path(sys.argv[1]), Path(sys.argv[2]))
