#!/usr/bin/env python3
"""Build verified, patched DING menu files for the Morphix package."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Sequence


ROOT: Path = Path(__file__).resolve().parents[1]
SOURCE_MANIFEST: Path = ROOT / "integration/ding/source.json"
PATCH_FILE: Path = ROOT / "integration/ding/morphix-menu.patch"
LICENSE_FILE: Path = ROOT / "integration/ding/LICENSE"
SOURCE_FILES: tuple[str, ...] = (
    "app/fileItemMenu.js",
    "app/desktopManager.js",
    "metadata.json",
    "extension.js",
)
PATCHED_FILES: tuple[str, ...] = ("app/fileItemMenu.js", "app/desktopManager.js")


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def read_source_manifest(path: Path) -> dict[str, object]:
    try:
        manifest = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise ValueError(f"Cannot read DING source manifest {path}: {error}") from error
    if (
        not isinstance(manifest, dict)
        or not isinstance(manifest.get("source_files"), dict)
        or not isinstance(manifest.get("patched_files"), dict)
    ):
        raise ValueError(f"DING source manifest {path} has an invalid format")
    if not all(isinstance(manifest.get(key), str) for key in ("package", "version", "source")):
        raise ValueError(f"DING source manifest {path} is missing package metadata")
    return manifest


def verify_source(source: Path, source_files: dict[str, object]) -> dict[str, str]:
    if source.is_symlink() or not source.is_dir():
        raise ValueError(f"DING source must be a real directory: {source}")
    if set(source_files) != set(SOURCE_FILES):
        raise ValueError("DING source manifest must hash exactly the supported source files")

    actual_hashes: dict[str, str] = {}
    for relative_path in SOURCE_FILES:
        path: Path = source / relative_path
        for component_index in range(1, len(Path(relative_path).parts)):
            parent: Path = source.joinpath(*Path(relative_path).parts[:component_index])
            if parent.is_symlink():
                raise ValueError(f"DING source path must not traverse a symlink: {parent}")
        if path.is_symlink():
            raise ValueError(f"DING source file must not be a symlink: {relative_path}")
        if not path.is_file():
            raise ValueError(f"DING source file is missing: {relative_path}")
        actual_hash: str = sha256(path)
        expected_hash: object = source_files[relative_path]
        if not isinstance(expected_hash, str) or actual_hash != expected_hash:
            raise ValueError(
                f"SHA-256 mismatch for {relative_path}: expected {expected_hash}, got {actual_hash}"
            )
        actual_hashes[relative_path] = actual_hash
    return actual_hashes


def prepare_ding(source: Path, output: Path) -> None:
    if output.exists() or output.is_symlink():
        reason: str = "symlink" if output.is_symlink() else "already exists"
        raise ValueError(f"DING output {output} {reason}; it must not be replaced")
    if not output.parent.is_dir():
        raise ValueError(f"DING output parent directory does not exist: {output.parent}")

    source_manifest: dict[str, object] = read_source_manifest(SOURCE_MANIFEST)
    source_file_hashes: object = source_manifest["source_files"]
    if not isinstance(source_file_hashes, dict):
        raise ValueError("DING source manifest must contain a source_files object")
    source_hashes: dict[str, str] = verify_source(source, source_file_hashes)

    with tempfile.TemporaryDirectory(prefix=".morphix-ding-", dir=output.parent) as temporary:
        staging: Path = Path(temporary) / "payload"
        patch_root: Path = staging
        (patch_root / "app").mkdir(parents=True)
        for relative_path in PATCHED_FILES:
            shutil.copyfile(source / relative_path, patch_root / relative_path)

        try:
            subprocess.run(
                ["patch", "--batch", "--fuzz=0", "--strip=1", f"--input={PATCH_FILE}"],
                cwd=patch_root,
                check=True,
                capture_output=True,
                text=True,
            )
        except subprocess.CalledProcessError as error:
            details: str = (error.stderr or error.stdout).strip()
            raise ValueError(f"DING patch did not apply cleanly: {details}") from error

        actual_patch_files: set[str] = {
            path.relative_to(patch_root).as_posix()
            for path in patch_root.rglob("*")
            if path.is_file()
        }
        if actual_patch_files != set(PATCHED_FILES) or any(
            path.is_symlink() for path in patch_root.rglob("*")
        ):
            raise ValueError("DING patch changed files outside the two approved menu modules")

        patched_hashes: dict[str, str] = {
            relative_path: sha256(patch_root / relative_path)
            for relative_path in PATCHED_FILES
        }
        expected_patched_hashes: object = source_manifest["patched_files"]
        if not isinstance(expected_patched_hashes, dict) or patched_hashes != expected_patched_hashes:
            raise ValueError("DING patch output hashes do not match the approved manifest")
        (patch_root / "LICENSE").write_bytes(LICENSE_FILE.read_bytes())
        output_manifest: dict[str, object] = {
            "package": source_manifest["package"],
            "version": source_manifest["version"],
            "source": source_manifest["source"],
            "source_hashes": source_hashes,
            "patched_files": patched_hashes,
        }
        (patch_root / "manifest.json").write_text(
            json.dumps(output_manifest, indent=2, sort_keys=True) + "\n",
            encoding="utf-8",
        )

        if output.exists() or output.is_symlink():
            raise ValueError(f"DING output {output} appeared during preparation")
        os.rename(patch_root, output)


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    arguments: argparse.Namespace = parser.parse_args(argv)
    try:
        prepare_ding(arguments.source, arguments.output)
    except (OSError, ValueError) as error:
        print(f"prepare-ding: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
