"""Chép file giấy phép của mọi crate Rust và gói npm (production) được nhúng vào binary Morphix.

Usage: python3 scripts/collect-bundled-licenses.py <thư mục đầu ra>
WHY: crate MIT/Apache/BSD và gói npm bị liên kết tĩnh hoặc gộp vào ui/dist; giấy phép của chúng yêu cầu
kèm văn bản thông báo khi phân phối binary. Chạy sau `cargo build` và `npm ci` để mã nguồn đã có trên máy.
"""

from __future__ import annotations

import json
import re
import shutil
import subprocess
import sys
from pathlib import Path

ROOT: Path = Path(__file__).resolve().parents[1]
UI: Path = ROOT / "crates/morphix-gui/ui"
LICENSE_NAME = re.compile(r"^(licen[cs]e|copying|copyright|notice|unlicense)([._-].*)?$", re.IGNORECASE)


SPDX: Path = ROOT / "packaging/spdx"


def copy_licenses(source: Path, target: Path, label: str, declared: str) -> bool:
    files = [entry for entry in source.iterdir() if entry.is_file() and LICENSE_NAME.match(entry.name)]
    if not files:
        # WHY: gói chỉ khai báo SPDX (không kèm file) thì kèm văn bản chuẩn của giấy phép đã khai báo.
        files = [SPDX / f"{spdx}.txt" for spdx in re.findall(r"[A-Za-z0-9.-]+", declared)
                 if (SPDX / f"{spdx}.txt").is_file()]
    target.mkdir(parents=True, exist_ok=True)
    (target / "SOURCE").write_text(f"{label}\nlicense: {declared}\n")
    for entry in files:
        shutil.copyfile(entry, target / entry.name)
    return bool(files)


def crates(out: Path) -> list[str]:
    metadata = json.loads(subprocess.run(
        ["cargo", "metadata", "--format-version", "1", "--locked", "--filter-platform", "x86_64-unknown-linux-gnu"],
        cwd=ROOT, check=True, capture_output=True, text=True,
    ).stdout)
    workspace = set(metadata["workspace_members"])
    used = {node["id"] for node in metadata["resolve"]["nodes"]}
    missing: list[str] = []
    for package in metadata["packages"]:
        if package["id"] in workspace or package["id"] not in used:
            continue
        label = f"{package['name']}-{package['version']}"
        if not copy_licenses(Path(package["manifest_path"]).parent, out / "crates" / label, label, package.get("license") or "?"):
            missing.append(f"crate {label} ({package.get('license')})")
    return missing


def npm_packages(out: Path) -> list[str]:
    listing = subprocess.run(["npm", "ls", "--omit=dev", "--all", "--parseable"],
                             cwd=UI, capture_output=True, text=True).stdout.split()
    missing: list[str] = []
    for path in map(Path, listing):
        if path == UI or not (path / "package.json").is_file():
            continue
        manifest = json.loads((path / "package.json").read_text())
        label = f"{manifest['name'].replace('/', '__')}-{manifest.get('version', '?')}"
        if not copy_licenses(path, out / "npm" / label, label, str(manifest.get("license", "?"))):
            missing.append(f"npm {label} ({manifest.get('license')})")
    return missing


def main() -> int:
    out = Path(sys.argv[1])
    missing = crates(out) + npm_packages(out)
    # WHY không dừng build: vài crate chỉ khai báo SPDX trong Cargo.toml; ghi rõ để rà soát, SOURCE vẫn có tên giấy phép.
    (out / "MISSING-LICENSE-FILES.txt").write_text("\n".join(missing) + ("\n" if missing else ""))
    print(f"{sum(1 for _ in out.glob('*/*'))} gói, {len(missing)} gói chỉ có tên giấy phép (xem MISSING-LICENSE-FILES.txt)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
