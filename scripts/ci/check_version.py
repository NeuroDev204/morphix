#!/usr/bin/env python3
"""In phiên bản của workspace; thoát mã 1 nếu tauri.conf.json hoặc tag phát hành không khớp.

Usage: check_version.py [tag]   (ví dụ: check_version.py v0.1.2)
Nguồn phiên bản duy nhất là `version` trong `[workspace.package]` của Cargo.toml.
"""
import json
import re
import sys
from pathlib import Path


def workspace_version(root: Path) -> str:
    text = (root / "Cargo.toml").read_text(encoding="utf-8")
    section = re.search(r"^\[workspace\.package\]\s*$(.*?)(?=^\[|\Z)", text, re.M | re.S)
    match = section and re.search(r'^version\s*=\s*"([^"]+)"', section.group(1), re.M)
    if not match:
        raise ValueError("Cargo.toml không có version trong [workspace.package]")
    return match.group(1)


def check(root: Path, tag: str | None) -> str:
    version = workspace_version(root)
    tauri = json.loads((root / "crates" / "morphix-gui" / "tauri.conf.json").read_text(encoding="utf-8"))["version"]
    if tauri != version:
        raise ValueError(f"tauri.conf.json ghi {tauri} nhưng workspace ghi {version}")
    if tag is not None and tag != f"v{version}":
        raise ValueError(f"tag {tag} không khớp phiên bản workspace v{version}")
    return version


def main(argv: list[str]) -> int:
    root = Path(__file__).resolve().parents[2]
    try:
        print(check(root, argv[1] if len(argv) > 1 else None))
    except ValueError as error:
        print(f"lỗi: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
