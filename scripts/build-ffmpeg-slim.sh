#!/bin/bash
# Dựng ffmpeg gọn cho Morphix từ mã nguồn, liên kết tĩnh hoàn toàn (không phụ thuộc glibc nên chạy trên mọi distro).
# Chỉ bật các bộ mã hoá và muxer mà Morphix gọi (xem crates/morphix-core/src/ops/media.rs); giữ mọi bộ giải mã và demuxer
# vì người dùng có thể đưa vào file bất kỳ. Chạy trong container ubuntu:22.04 với quyền root:
#   docker run --rm -v "$PWD:/repo" -v "$OUT:/out" ubuntu:22.04 bash /repo/scripts/build-ffmpeg-slim.sh
# Biến: FFMPEG_REVISION (mặc định 1) đặt hậu tố tên gói; kết quả: /out/ffmpeg-<ver>-morphix.<rev>-linux64-gpl.tar.xz
set -euo pipefail

readonly FFMPEG_VERSION=8.1.3
readonly FFMPEG_SHA256='7138d28c96d9d3e3af4ee3d8cad72741f8ffb40da90c1112235dea3ecd3178a3'
readonly DAV1D_VERSION=1.5.1
readonly DAV1D_SHA256='401813f1f89fa8fd4295805aa5284d9aed9bc7fc1fdbe554af4292f64cbabe21'
readonly REVISION="${FFMPEG_REVISION:-1}"
NAME="ffmpeg-${FFMPEG_VERSION}-morphix.${REVISION}-linux64-gpl"
OUT="${OUT_DIR:-/out}"

export DEBIAN_FRONTEND=noninteractive
apt-get update -qq
# WHY thư viện của distro: bản dev của Ubuntu 22.04 có sẵn .a; phiên bản và nguồn của chúng được ghi vào BUILD-INFO.txt.
apt-get install -y -qq --no-install-recommends build-essential nasm yasm pkg-config curl xz-utils ca-certificates \
  libx264-dev libvpx-dev libopus-dev libmp3lame-dev libvorbis-dev libogg-dev meson ninja-build >/dev/null

WORK="$(mktemp -d)"
cd "$WORK"
# WHY dav1d dựng từ nguồn: ffmpeg không có bộ giải mã AV1 phần mềm gốc, và gói dav1d của Ubuntu 22.04 (0.9) quá cũ cho ffmpeg 8.
curl -fsSL -o dav1d.tar.xz "https://downloads.videolan.org/pub/videolan/dav1d/${DAV1D_VERSION}/dav1d-${DAV1D_VERSION}.tar.xz"
echo "${DAV1D_SHA256}  dav1d.tar.xz" | sha256sum -c --status - || { echo "dav1d nguồn sai SHA256" >&2; exit 1; }
tar -xJf dav1d.tar.xz
( cd "dav1d-${DAV1D_VERSION}" && meson setup build --default-library=static --prefix=/opt/dav1d --libdir=lib \
    -Denable_tools=false -Denable_tests=false -Dbuildtype=release >/dev/null && ninja -C build install >/dev/null )
export PKG_CONFIG_PATH=/opt/dav1d/lib/pkgconfig

curl -fsSL -o ffmpeg.tar.xz "https://ffmpeg.org/releases/ffmpeg-${FFMPEG_VERSION}.tar.xz"
echo "${FFMPEG_SHA256}  ffmpeg.tar.xz" | sha256sum -c --status - || { echo "ffmpeg nguồn sai SHA256" >&2; exit 1; }
tar -xJf ffmpeg.tar.xz
cd "ffmpeg-${FFMPEG_VERSION}"

# WHY giữ indev lavfi: bộ test của repo tạo file mẫu bằng `-f lavfi testsrc/sine`; nó nhỏ và không ảnh hưởng người dùng.
# Bộ mã hoá/muxer khớp ops/media.rs: H.264 (MP4, MOV, MKV, AVI), VP9 (WebM), AAC, MP3, Opus, Vorbis, FLAC, PCM, GIF.
ENCODERS=libx264,libvpx_vp9,aac,libmp3lame,libopus,libvorbis,flac,pcm_s16le,pcm_s16be,gif
MUXERS=mp4,mov,ipod,matroska,webm,avi,mp3,wav,flac,ogg,opus,aiff,gif

./configure \
  --prefix=/usr/local --enable-gpl --disable-autodetect \
  --disable-doc --disable-ffplay --disable-ffprobe --disable-network --disable-debug --disable-shared --enable-static \
  --enable-libx264 --enable-libvpx --enable-libopus --enable-libmp3lame --enable-libvorbis --enable-libdav1d \
  --disable-encoders "--enable-encoder=${ENCODERS}" \
  --disable-muxers "--enable-muxer=${MUXERS}" \
  --disable-protocols --enable-protocol=file,pipe \
  --disable-indevs --disable-outdevs --enable-indev=lavfi \
  --pkg-config-flags=--static --extra-ldexeflags=-static \
  --extra-cflags=-O2 >/dev/null
make -j"$(nproc)" >/dev/null
strip ffmpeg

PKG="$WORK/$NAME"
mkdir -p "$PKG/bin"
install -m0755 ffmpeg "$PKG/bin/ffmpeg"
cp COPYING.GPLv3 "$PKG/LICENSE.txt"
./ffmpeg -hide_banner -buildconf > "$PKG/configuration.txt"
{
  echo "ffmpeg ${FFMPEG_VERSION} (nguồn: https://ffmpeg.org/releases/ffmpeg-${FFMPEG_VERSION}.tar.xz, sha256 ${FFMPEG_SHA256})"
  echo "dav1d ${DAV1D_VERSION} (nguồn: https://downloads.videolan.org/pub/videolan/dav1d/${DAV1D_VERSION}/dav1d-${DAV1D_VERSION}.tar.xz, sha256 ${DAV1D_SHA256})"
  echo "Dựng trong ubuntu:22.04 với các gói tĩnh sau (mã nguồn: https://packages.ubuntu.com/source/jammy/<tên gói nguồn>):"
  dpkg-query -W -f='  ${Package} ${Version}  (nguồn: ${Source})\n' libx264-dev libvpx-dev libopus-dev libmp3lame-dev libvorbis-dev libogg-dev
} > "$PKG/BUILD-INFO.txt"
mkdir -p "$OUT"
tar -C "$WORK" -cJf "$OUT/$NAME.tar.xz" "$NAME"
sha256sum "$OUT/$NAME.tar.xz" | tee "$OUT/$NAME.tar.xz.sha256"
ls -l "$PKG/bin/ffmpeg"
file "$PKG/bin/ffmpeg" 2>/dev/null || true
