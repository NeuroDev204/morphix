#!/usr/bin/env bash
# Dựng gói tar cho bản cài bằng lệnh curl: binary, ffmpeg tĩnh, PDFium, icon và giấy phép, theo bố cục
# `<prefix>/bin` → `<prefix>/lib/morphix` mà `bundled_lib_dir()` tìm.
# Usage: scripts/build-tarball.sh <thư mục đầu ra>
# Biến: MORPHIX_FFMPEG_STATIC_TARBALL (file .tar.xz đã tải sẵn, khỏi tải lại),
#       MORPHIX_ALLOW_GLIBC_FLOOR (nới ngưỡng glibc khi thử trên máy mới; CI không đặt).
set -euo pipefail

readonly PDFIUM_SHA256='f728930966f503652b92acc89b9374a2eeca00ce42e26dccd3e4b5c5161b2d64'
readonly GLIBC_LIMIT="${MORPHIX_ALLOW_GLIBC_FLOOR:-2.35}"

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUT="$(realpath -m "${1:?thiếu thư mục đầu ra}")"
TARGET_DIR="${CARGO_TARGET_DIR:-$ROOT/target}"
VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' "$ROOT/Cargo.toml" | head -1)"
NAME=morphix-x86_64-linux.tar.gz

# WHY: sản phẩm không được nằm trong repo, để không lỡ commit binary.
case "$OUT" in
  "$ROOT"|"$ROOT"/*) echo "thư mục đầu ra không được nằm trong repo: $OUT" >&2; exit 1 ;;
esac
mkdir -p "$OUT"
WORK="$(mktemp -d "$OUT/tarball.XXXXXX")"
trap 'rm -rf "$WORK"' EXIT
STAGE="$WORK/morphix"

(cd "$ROOT/crates/morphix-gui/ui" && npm run build)
# WHY: custom-protocol để binary nhúng ui/dist; thiếu nó thì cửa sổ nạp localhost:5173.
(cd "$ROOT" && cargo build --locked --release -p morphix-cli -p morphix-gui --features morphix-gui/custom-protocol)
install -Dm0755 "$TARGET_DIR/release/morphix" "$STAGE/bin/morphix"
install -Dm0755 "$TARGET_DIR/release/morphix-gui" "$STAGE/bin/morphix-gui"
for bin in "$STAGE/bin/morphix" "$STAGE/bin/morphix-gui"; do
  patchelf --set-rpath '$ORIGIN/../lib/morphix/pdfium' "$bin"
done
bash "$ROOT/scripts/check-gui-embeds-ui.sh" "$STAGE/bin/morphix-gui"

echo "$PDFIUM_SHA256  $ROOT/third_party/pdfium/lib/libpdfium.so" | sha256sum -c --status -
install -Dm0644 "$ROOT/third_party/pdfium/lib/libpdfium.so" "$STAGE/lib/morphix/pdfium/libpdfium.so"

FFMPEG_URL="$(sed -n 's/^URL: *//p' "$ROOT/packaging/ffmpeg-static.txt")"
FFMPEG_SHA256="$(sed -n 's/^SHA256: *//p' "$ROOT/packaging/ffmpeg-static.txt")"
FFMPEG_TAR="${MORPHIX_FFMPEG_STATIC_TARBALL:-$WORK/ffmpeg.tar.xz}"
[ -f "$FFMPEG_TAR" ] || curl -fsSL -o "$FFMPEG_TAR" "$FFMPEG_URL"
echo "$FFMPEG_SHA256  $FFMPEG_TAR" | sha256sum -c --status - \
  || { echo "ffmpeg tĩnh sai SHA256: $FFMPEG_TAR" >&2; exit 1; }
mkdir "$WORK/ffmpeg"
tar -xJf "$FFMPEG_TAR" -C "$WORK/ffmpeg" --strip-components=1 --wildcards '*/bin/ffmpeg' '*/LICENSE.txt'
install -Dm0755 "$WORK/ffmpeg/bin/ffmpeg" "$STAGE/lib/morphix/ffmpeg"
"$STAGE/lib/morphix/ffmpeg" -hide_banner -buildconf | grep -q -- '--enable-libx264' \
  || { echo "ffmpeg tĩnh thiếu libx264" >&2; exit 1; }

install -Dm0644 "$ROOT/crates/morphix-gui/icons/128x128.png" \
  "$STAGE/share/icons/hicolor/128x128/apps/morphix.png"
DOC="$STAGE/share/doc/morphix"
install -Dm0644 "$ROOT/LICENSE" "$DOC/LICENSE"
install -Dm0644 "$ROOT/packaging/LICENSES-THIRD-PARTY.md" "$DOC/LICENSES-THIRD-PARTY.md"
install -Dm0644 "$ROOT/packaging/ffmpeg-static.txt" "$DOC/ffmpeg-source-offer.txt"
install -Dm0644 "$WORK/ffmpeg/LICENSE.txt" "$DOC/ffmpeg/LICENSE.txt"
# WHY: ffmpeg tĩnh gộp sẵn hơn trăm thư viện BSD/MIT/LGPL; văn bản lấy bằng scripts/collect-ffmpeg-licenses.py.
cp -r "$ROOT/packaging/ffmpeg-static-licenses" "$DOC/ffmpeg/third-party"
# WHY: crate Rust và gói npm bị liên kết tĩnh/gộp vào binary cũng cần kèm giấy phép.
python3 "$ROOT/scripts/collect-bundled-licenses.py" "$DOC/bundled"
install -Dm0644 "$ROOT/third_party/pdfium/LICENSE" "$DOC/pdfium/LICENSE"
for licence in "$ROOT"/third_party/pdfium/licenses/*; do
  install -Dm0644 "$licence" "$DOC/pdfium/licenses/$(basename "$licence")"
done
printf '%s\n' "$VERSION" > "$STAGE/VERSION"

# WHY: ngưỡng glibc là điều kiện "chạy trên mọi distro" của spec; vượt ngưỡng thì dừng thay vì phát hành.
floor="$(find "$STAGE/bin" "$STAGE/lib" -type f -print0 | xargs -0 -r -n1 objdump -T 2>/dev/null \
  | awk '/UND/' | grep -o 'GLIBC_2\.[0-9]*' | sort -uV | tail -1 | sed 's/GLIBC_//')"
if [ "$(printf '%s\n%s\n' "$floor" "$GLIBC_LIMIT" | sort -V | tail -1)" != "$GLIBC_LIMIT" ]; then
  echo "glibc floor $floor vượt ngưỡng $GLIBC_LIMIT; hãy dựng trên Ubuntu 22.04" >&2
  exit 1
fi

tar -C "$WORK" --owner=0 --group=0 -czf "$OUT/$NAME" morphix
(cd "$OUT" && sha256sum "$NAME" > "$NAME.sha256")
echo "built $OUT/$NAME (glibc floor $floor)"
