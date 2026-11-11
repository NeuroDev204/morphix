#!/usr/bin/env bash
# Tải ffmpeg gọn đã ghim trong packaging/ffmpeg-static.txt, kiểm SHA256, rồi cài binary và giấy phép của nó.
# Dùng chung cho tarball (build-tarball.sh) và gói .deb (build-deb.sh), để hai gói Linux mang đúng một bản ffmpeg.
# Usage: scripts/install-ffmpeg-slim.sh <đường dẫn binary đích> <thư mục doc đích> <thư mục làm việc>
# Biến: MORPHIX_FFMPEG_STATIC_TARBALL (tarball tải sẵn, khỏi tải lại).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DEST="${1:?thiếu đường dẫn binary đích}"
DOC="${2:?thiếu thư mục doc đích}"
WORK="${3:?thiếu thư mục làm việc}"

URL="$(sed -n 's/^URL: *//p' "$ROOT/packaging/ffmpeg-static.txt")"
SHA256="$(sed -n 's/^SHA256: *//p' "$ROOT/packaging/ffmpeg-static.txt")"
TARBALL="${MORPHIX_FFMPEG_STATIC_TARBALL:-$WORK/ffmpeg.tar.xz}"
mkdir -p "$WORK"
[ -f "$TARBALL" ] || curl -fsSL --retry 5 --retry-all-errors -o "$TARBALL" "$URL"
echo "$SHA256  $TARBALL" | sha256sum -c --status - \
  || { echo "ffmpeg tĩnh sai SHA256: $TARBALL" >&2; exit 1; }

mkdir -p "$WORK/ffmpeg"
tar -xJf "$TARBALL" -C "$WORK/ffmpeg" --strip-components=1 --wildcards \
  '*/bin/ffmpeg' '*/LICENSE.txt' '*/BUILD-INFO.txt' '*/configuration.txt'
install -Dm0755 "$WORK/ffmpeg/bin/ffmpeg" "$DEST"
"$DEST" -hide_banner -buildconf | grep -q -- '--enable-libx264' \
  || { echo "ffmpeg tĩnh thiếu libx264" >&2; exit 1; }

install -Dm0644 "$ROOT/packaging/ffmpeg-static.txt" "$DOC/ffmpeg-source-offer.txt"
for file in LICENSE.txt BUILD-INFO.txt configuration.txt; do
  install -Dm0644 "$WORK/ffmpeg/$file" "$DOC/ffmpeg/$file"
done
# WHY: ffmpeg tĩnh gộp sẵn x264, libvpx, opus, lame, ogg/vorbis, dav1d, zlib; văn bản lấy bằng scripts/collect-ffmpeg-licenses.py.
mkdir -p "$DOC/ffmpeg"
cp -r "$ROOT/packaging/ffmpeg-licenses" "$DOC/ffmpeg/third-party"
