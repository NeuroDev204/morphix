#!/bin/bash
# Dựng ffmpeg.exe gọn cho Morphix trên Windows: biên dịch chéo bằng mingw-w64, liên kết tĩnh hoàn toàn, tự dựng các thư viện
# codec từ nguồn đã ghim SHA256 (Windows không có gói dev sẵn như Ubuntu). Cùng bộ tính năng với scripts/build-ffmpeg-slim.sh.
# Chạy trong container debian:13 với quyền root:
#   docker run --rm -v "$PWD:/repo:ro" -v "$OUT:/out" debian:13 bash /repo/scripts/build-ffmpeg-slim-windows.sh
# Kết quả: /out/ffmpeg-<ver>-morphix.<rev>-win64-gpl.zip (bin/ffmpeg.exe, LICENSE.txt, configuration.txt, BUILD-INFO.txt)
set -euo pipefail

readonly FFMPEG_VERSION=8.1.3
readonly REVISION="${FFMPEG_REVISION:-1}"
readonly HOST=x86_64-w64-mingw32
readonly PREFIX=/deps
NAME="ffmpeg-${FFMPEG_VERSION}-morphix.${REVISION}-win64-gpl"
OUT="${OUT_DIR:-/out}"

# tên file|URL|SHA256 của mọi nguồn; thay đổi bất kỳ chữ nào ở nguồn thì bản dựng dừng.
SOURCES=(
  "ffmpeg-8.1.3.tar.xz|https://ffmpeg.org/releases/ffmpeg-8.1.3.tar.xz|7138d28c96d9d3e3af4ee3d8cad72741f8ffb40da90c1112235dea3ecd3178a3"
  "libvpx-1.14.1.tar.gz|https://github.com/webmproject/libvpx/archive/refs/tags/v1.14.1.tar.gz|901747254d80a7937c933d03bd7c5d41e8e6c883e0665fadcb172542167c7977"
  "opus-1.5.2.tar.gz|https://downloads.xiph.org/releases/opus/opus-1.5.2.tar.gz|65c1d2f78b9f2fb20082c38cbe47c951ad5839345876e46941612ee87f9a7ce1"
  "lame-3.100.tar.gz|https://downloads.sourceforge.net/project/lame/lame/3.100/lame-3.100.tar.gz|ddfe36cab873794038ae2c1210557ad34857a4b6bdc515785d1da9e175b1da1e"
  "zlib-1.3.1.tar.xz|https://github.com/madler/zlib/releases/download/v1.3.1/zlib-1.3.1.tar.xz|38ef96b8dfe510d42707d9c781877914792541133e1870841463bfa73f883e32"
  "libogg-1.3.5.tar.xz|https://downloads.xiph.org/releases/ogg/libogg-1.3.5.tar.xz|c4d91be36fc8e54deae7575241e03f4211eb102afb3fc0775fbbc1b740016705"
  "libvorbis-1.3.7.tar.xz|https://downloads.xiph.org/releases/vorbis/libvorbis-1.3.7.tar.xz|b33cc4934322bcbf6efcbacf49e3ca01aadbea4114ec9589d1b1e9d20f72954b"
  "x264-0480cb05fa188d37ae87e8f4fd8f1aea3711f7ee.tar.bz2|https://code.videolan.org/videolan/x264/-/archive/0480cb05fa188d37ae87e8f4fd8f1aea3711f7ee/x264-0480cb05fa188d37ae87e8f4fd8f1aea3711f7ee.tar.bz2|f05c59f2e83d494c36307025dca2d3afc6b4d185f3a3453d06cc4fecd7094057"
  "dav1d-1.5.1.tar.xz|https://downloads.videolan.org/pub/videolan/dav1d/1.5.1/dav1d-1.5.1.tar.xz|401813f1f89fa8fd4295805aa5284d9aed9bc7fc1fdbe554af4292f64cbabe21"
)

export DEBIAN_FRONTEND=noninteractive
apt-get update -qq
apt-get install -y -qq --no-install-recommends build-essential gcc-mingw-w64-x86-64-posix g++-mingw-w64-x86-64-posix \
  binutils-mingw-w64-x86-64 nasm yasm pkg-config curl xz-utils bzip2 ca-certificates meson ninja-build zip perl >/dev/null
# WHY posix: x264, libvpx và ffmpeg dùng pthread; mô hình luồng win32 mặc định của Debian thiếu nó.
update-alternatives --set x86_64-w64-mingw32-gcc /usr/bin/x86_64-w64-mingw32-gcc-posix >/dev/null
update-alternatives --set x86_64-w64-mingw32-g++ /usr/bin/x86_64-w64-mingw32-g++-posix >/dev/null

WORK="$(mktemp -d)"
cd "$WORK"
for entry in "${SOURCES[@]}"; do
  IFS='|' read -r file url sha <<<"$entry"
  curl -fsSL --retry 5 --retry-delay 3 --retry-all-errors -o "$file" "$url"
  echo "$sha  $file" | sha256sum -c --status - || { echo "$file sai SHA256" >&2; exit 1; }
  mkdir -p "src/${file%.tar.*}"
  tar -xf "$file" -C "src/${file%.tar.*}" --strip-components=1
done

export PKG_CONFIG_LIBDIR="$PREFIX/lib/pkgconfig" PKG_CONFIG_PATH="$PREFIX/lib/pkgconfig"
JOBS="$(nproc)"
autotools() {  # autotools <thư mục nguồn> <tuỳ chọn configure...>
  local dir="$1"; shift
  ( cd "src/$dir" && ./configure --host="$HOST" --prefix="$PREFIX" --disable-shared --enable-static "$@" >/dev/null \
      && make -j"$JOBS" >/dev/null && make install >/dev/null )
}

# WHY zlib: bộ giải mã png/apng và phần nén `cmov` của file MOV (kể cả ảnh bìa PNG nhúng trong MP3/M4A) cần zlib.
( cd src/zlib-1.3.1 && CROSS_PREFIX="$HOST-" ./configure --static --prefix="$PREFIX" >/dev/null && make -j"$JOBS" >/dev/null && make install >/dev/null )
autotools libogg-1.3.5
autotools libvorbis-1.3.7 --with-ogg="$PREFIX" --disable-examples --disable-docs --disable-oggtest
autotools lame-3.100 --disable-frontend --disable-gtktest
autotools opus-1.5.2 --disable-doc --disable-extra-programs
( cd src/x264-0480cb05fa188d37ae87e8f4fd8f1aea3711f7ee && ./configure --host="$HOST" --cross-prefix="$HOST-" --prefix="$PREFIX" \
    --enable-static --disable-cli --disable-opencl >/dev/null && make -j"$JOBS" >/dev/null && make install >/dev/null )
( cd src/libvpx-1.14.1 && CROSS="$HOST-" ./configure --target=x86_64-win64-gcc --prefix="$PREFIX" --disable-examples --disable-tools \
    --disable-docs --disable-unit-tests --enable-vp9 --disable-install-bins >/dev/null && make -j"$JOBS" >/dev/null && make install >/dev/null )
cat > cross.txt <<CROSS
[binaries]
c = '$HOST-gcc'
ar = '$HOST-ar'
strip = '$HOST-strip'
windres = '$HOST-windres'
[host_machine]
system = 'windows'
cpu_family = 'x86_64'
cpu = 'x86_64'
endian = 'little'
CROSS
( cd src/dav1d-1.5.1 && meson setup build --cross-file ../../cross.txt --default-library=static --prefix="$PREFIX" --libdir=lib \
    -Denable_tools=false -Denable_tests=false -Dbuildtype=release >/dev/null && ninja -C build install >/dev/null )

ENCODERS=libx264,libvpx_vp9,aac,libmp3lame,libopus,libvorbis,flac,pcm_s16le,pcm_s16be,gif
MUXERS=mp4,mov,ipod,matroska,webm,avi,mp3,wav,flac,ogg,opus,aiff,gif
cd "src/ffmpeg-${FFMPEG_VERSION}"
./configure \
  --arch=x86_64 --target-os=mingw32 --cross-prefix="$HOST-" --enable-cross-compile --pkg-config=pkg-config \
  --enable-gpl --disable-autodetect --disable-doc --disable-ffplay --disable-ffprobe --disable-network --disable-debug \
  --disable-shared --enable-static \
  --enable-libx264 --enable-libvpx --enable-libopus --enable-libmp3lame --enable-libvorbis --enable-libdav1d --enable-zlib \
  --disable-encoders "--enable-encoder=${ENCODERS}" \
  --disable-muxers "--enable-muxer=${MUXERS}" \
  --disable-protocols --enable-protocol=file,pipe \
  --disable-indevs --disable-outdevs --enable-indev=lavfi \
  --pkg-config-flags=--static --extra-cflags="-I$PREFIX/include -O2" --extra-ldflags="-L$PREFIX/lib" --extra-ldexeflags=-static >/dev/null
make -j"$JOBS" >/dev/null
"$HOST-strip" ffmpeg.exe

PKG="$WORK/$NAME"
mkdir -p "$PKG/bin" "$OUT"
install -m0755 ffmpeg.exe "$PKG/bin/ffmpeg.exe"
cp COPYING.GPLv3 "$PKG/LICENSE.txt"
grep '^#define FFMPEG_CONFIGURATION' config.h | sed 's/^#define FFMPEG_CONFIGURATION //' > "$PKG/configuration.txt"
{
  echo "ffmpeg ${FFMPEG_VERSION}, dựng chéo bằng $("$HOST-gcc" --version | head -1) trong debian:13."
  echo "Nguồn của mọi thành phần (tên file, URL, SHA256):"
  for entry in "${SOURCES[@]}"; do IFS='|' read -r file url sha <<<"$entry"; echo "  $file  $url  $sha"; done
} > "$PKG/BUILD-INFO.txt"
( cd "$WORK" && zip -qr "$OUT/$NAME.zip" "$NAME" )
( cd "$OUT" && sha256sum "$NAME.zip" | tee "$NAME.zip.sha256" )
ls -l "$PKG/bin/ffmpeg.exe"
