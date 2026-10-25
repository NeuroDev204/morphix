#!/usr/bin/env python3
"""Render Morphix dpkg maintainer scripts from the approved DING manifest."""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path
from typing import Sequence


ROOT: Path = Path(__file__).resolve().parents[1]
TEMPLATES: Path = ROOT / "packaging/deb"
MANIFEST: Path = ROOT / "integration/ding/source.json"
EXPECTED_FILES: tuple[str, ...] = ("app/fileItemMenu.js", "app/desktopManager.js")


def render_scripts(manifest_path: Path, output_directory: Path) -> None:
    manifest: dict[str, object] = json.loads(manifest_path.read_text(encoding="utf-8"))
    package: object = manifest.get("package")
    version: object = manifest.get("version")
    source_files: object = manifest.get("source_files")
    patched_files: object = manifest.get("patched_files")
    if (
        package != "gnome-shell-extension-desktop-icons-ng"
        or not isinstance(version, str)
        or not re.fullmatch(r"[A-Za-z0-9.+:~_-]+", version)
        or not isinstance(source_files, dict)
        or not isinstance(patched_files, dict)
        or not set(EXPECTED_FILES).issubset(source_files)
        or set(patched_files) != set(EXPECTED_FILES)
    ):
        raise ValueError("DING manifest does not contain the supported package, version and files")
    replacements: dict[str, str] = {"@DING_VERSION@": version}
    for relative, placeholder in zip(
        EXPECTED_FILES, ("@SOURCE_FILE_MENU_SHA256@", "@SOURCE_DESKTOP_MENU_SHA256@")
    ):
        source_hash: object = source_files[relative]
        patched_hash: object = patched_files[relative]
        if not isinstance(source_hash, str) or not re.fullmatch(r"[0-9a-f]{64}", source_hash):
            raise ValueError(f"invalid source hash for {relative}")
        if not isinstance(patched_hash, str) or not re.fullmatch(r"[0-9a-f]{64}", patched_hash):
            raise ValueError(f"invalid patched hash for {relative}")
        replacements[placeholder] = source_hash

    output_directory.mkdir(parents=True, exist_ok=True)
    for name in ("preinst", "postrm"):
        template: str = (TEMPLATES / f"{name}.in").read_text(encoding="utf-8")
        rendered: str = template
        for placeholder, value in replacements.items():
            rendered = rendered.replace(placeholder, value)
        if re.search(r"@[A-Z_]+@", rendered):
            raise ValueError(f"unexpanded placeholder in {name}")
        destination: Path = output_directory / name
        destination.write_text(rendered, encoding="utf-8")
        destination.chmod(0o755)


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest", type=Path, default=MANIFEST)
    parser.add_argument("--output", required=True, type=Path)
    arguments: argparse.Namespace = parser.parse_args(argv)
    try:
        render_scripts(arguments.manifest, arguments.output)
    except (OSError, json.JSONDecodeError, ValueError) as error:
        print(f"render-dpkg-scripts: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
