#!/usr/bin/env bash
# Kiểm một file .deb của Morphix: payload desktop, maintainer scripts, Depends và ldd.
# Usage: scripts/check-deb.sh <file.deb>
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DEB="${1:?usage: check-deb.sh <file.deb>}"
# WHY: cùng MORPHIX_TARGET với build-deb.sh, để danh sách Depends kiểm đúng bản Ubuntu đã đóng gói.
case "${MORPHIX_TARGET:-noble}" in
  noble) RELY_FILE=rely-packages.txt ;;
  jammy) echo "Jammy chưa được hỗ trợ: tích hợp Desktop ghim DING của Ubuntu 24.04" >&2; exit 1 ;;
  *) echo "MORPHIX_TARGET không hợp lệ: ${MORPHIX_TARGET:-}" >&2; exit 1 ;;
esac
RELY_LIST="$ROOT/crates/morphix-gui/packaging/deb/$RELY_FILE"
WORK="$(mktemp -d "$(dirname "$DEB")/check.XXXXXX")"
trap 'python3 -c "import shutil,sys; shutil.rmtree(sys.argv[1])" "$WORK"' EXIT

expected=(
  ./usr/bin/morphix
  ./usr/bin/morphix-gui
  ./usr/lib/morphix/ffmpeg
  ./usr/lib/morphix/pdfium/libpdfium.so
  ./usr/share/nautilus-python/extensions/morphix_menu.py
  ./usr/share/doc/morphix/ubuntu-install.md
  ./usr/share/applications/morphix.desktop
  ./usr/share/icons/hicolor/128x128/apps/morphix.png
  ./usr/share/gnome-shell/extensions/ding@rastersoft.com/app/fileItemMenu.js
  ./usr/share/gnome-shell/extensions/ding@rastersoft.com/app/desktopManager.js
  ./usr/share/morphix/ding/manifest.json
  ./usr/share/doc/morphix/ding/LICENSE
  ./usr/share/doc/morphix/copyright
  ./usr/share/doc/morphix/LICENSES-THIRD-PARTY.md
  ./usr/share/doc/morphix/ffmpeg-source-offer.txt
)

listing="$(dpkg-deb -c "$DEB")"
for path in "${expected[@]}"; do
  if ! grep -qF " $path" <<<"$listing"; then
    echo "thiếu trong gói: $path" >&2
    exit 1
  fi
done
echo "== dpkg-deb -c: $(wc -l <<<"$listing") mục, đủ file bắt buộc"
echo "== dpkg-deb -I"
dpkg-deb -I "$DEB"

dpkg-deb -x "$DEB" "$WORK"
dpkg-deb -e "$DEB" "$WORK/control"
for script in preinst postrm; do
  [ -x "$WORK/control/$script" ] || { echo "thiếu maintainer script executable: $script" >&2; exit 1; }
  sh -n "$WORK/control/$script"
done
python3 -m py_compile "$WORK/usr/share/nautilus-python/extensions/morphix_menu.py"
desktop-file-validate "$WORK/usr/share/applications/morphix.desktop"
if grep -nE '/home/|/root/|/tmp/[^[:space:]]*stage\.' "$WORK/control/preinst" "$WORK/control/postrm"; then
  echo "maintainer scripts chứa đường dẫn máy build" >&2
  exit 1
fi
python3 - "$WORK/usr/share/morphix/ding/manifest.json" "$WORK/usr/share/gnome-shell/extensions/ding@rastersoft.com" "$WORK/control" <<'PY'
import hashlib
import json
import pathlib
import sys

manifest_path, extension_root, control_dir = map(pathlib.Path, sys.argv[1:])
manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
expected = {"app/fileItemMenu.js", "app/desktopManager.js"}
if (
    manifest.get("package") != "gnome-shell-extension-desktop-icons-ng"
    or set(manifest.get("patched_files", {})) != expected
    or set(manifest.get("source_hashes", {})) != {
        "app/fileItemMenu.js",
        "app/desktopManager.js",
        "metadata.json",
        "extension.js",
    }
):
    raise SystemExit("DING manifest does not describe exactly the two supported menu files")
for relative, digest in manifest["patched_files"].items():
    actual = hashlib.sha256((extension_root / relative).read_bytes()).hexdigest()
    if actual != digest:
        raise SystemExit(f"packaged DING hash mismatch: {relative}")
version = manifest["version"]
preinst = (control_dir / "preinst").read_text(encoding="utf-8")
postrm = (control_dir / "postrm").read_text(encoding="utf-8")
if version not in preinst or "dpkg-divert --add --rename" not in preinst:
    raise SystemExit("preinst does not pin DING or install the diversion")
if "dpkg-divert --remove --rename" not in postrm:
    raise SystemExit("postrm does not restore DING diversions")
if any(
    manifest["source_hashes"][relative] not in preinst
    or manifest["source_hashes"][relative] not in postrm
    for relative in expected
):
    raise SystemExit("maintainer scripts do not match the DING source hashes")
PY

depends="$(dpkg-deb -f "$DEB" Depends)"
ding_version="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1], encoding="utf-8"))["version"])' "$ROOT/integration/ding/source.json")"
for dependency in python3-nautilus python3-gi gir1.2-gtk-4.0 libglib2.0-bin; do
  grep -qF "$dependency" <<<"$depends" || { echo "Depends thiếu $dependency" >&2; exit 1; }
done
grep -qF "gnome-shell-extension-desktop-icons-ng (= $ding_version)" <<<"$depends" \
  || { echo "Depends không ghim đúng DING $ding_version" >&2; exit 1; }
echo "== desktop files, DING manifest/hash, maintainer scripts và Depends hợp lệ"

# WHY: Depends được apt giải theo bao đóng đệ quy (WebKitGTK kéo libxcb, libatk... về), nên gói
# nằm trong bao đóng đó vẫn coi là có sẵn trên máy đã cài .deb.
{
  apt-cache depends --recurse --no-recommends --no-suggests --no-conflicts --no-breaks \
    --no-replaces --no-enhances libwebkit2gtk-4.1-0 libayatana-appindicator3-1 \
    libglib2.0-bin python3-nautilus python3-gi gir1.2-gtk-4.0 \
    | awk '/^[a-z0-9]/ {print $1}' | sed 's/:.*//'
  grep -v '^#' "$RELY_LIST"
} | sort -u > "$WORK/depends-closure.txt"

# WHY: env -i để ldd không dùng thư viện của môi trường dev (LD_LIBRARY_PATH, PATH cục bộ).
# Mỗi đường dẫn resolve phải nằm trong gói, hoặc thuộc bao đóng Depends.
bins=("$WORK/usr/bin/morphix" "$WORK/usr/bin/morphix-gui" "$WORK/usr/lib/morphix/ffmpeg" "$WORK/usr/lib/morphix/pdfium/libpdfium.so")
for lib in "$WORK"/usr/lib/morphix/lib/*.so*; do bins+=("$lib"); done
failed=0
for bin in "${bins[@]}"; do
  while read -r soname arrow resolved _; do
    [ "$arrow" = "=>" ] || continue
    if [ "$resolved" = "not" ]; then
      echo "KHÔNG TÌM THẤY: $soname cho ${bin#"$WORK"}" >&2
      failed=1
      continue
    fi
    case "$resolved" in
      "$WORK"/*) ;;
      /*)
        # WHY: thử đường dẫn như ldd in ra trước, vì jammy không usrmerge nên dpkg ghi đường dẫn đó.
        owner="$(dpkg -S "$resolved" 2>/dev/null | head -1 | cut -d: -f1 || true)"
        [ -n "$owner" ] || owner="$(dpkg -S "$(readlink -f "$resolved")" 2>/dev/null | head -1 | cut -d: -f1 || true)"
        if [ -z "$owner" ] || ! grep -qxF "$owner" "$WORK/depends-closure.txt"; then
          echo "NGOÀI GÓI VÀ NGOÀI DEPENDS: $soname -> $resolved (gói: ${owner:-?})" >&2
          failed=1
        fi
        ;;
    esac
  done < <(env -i PATH=/usr/bin:/bin ldd "$bin")
done
[ "$failed" -eq 0 ] || exit 1
echo "== ldd: mọi binary đóng gói resolve trong gói hoặc trong bao đóng Depends"

echo "== ffmpeg đóng gói:"
env -i PATH=/usr/bin:/bin "$WORK/usr/lib/morphix/ffmpeg" -hide_banner -buildconf | grep -- "--enable-libx264\|--enable-gpl" | sed 's/^ *//' | head -2
env -i PATH=/usr/bin:/bin "$WORK/usr/lib/morphix/ffmpeg" -hide_banner -version | head -1
