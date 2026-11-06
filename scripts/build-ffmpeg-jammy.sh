#!/bin/bash
# Build ffmpeg 6.1.1 (tarball upstream) trong container ubuntu:22.04 cho bản .deb jammy.
# Cấu hình giống bản Noble; flag nào jammy không có thư viện thì tự bỏ và ghi lại trong /work/dropped-flags.txt.
# Cài ra /opt/ffmpeg-6.1.1, để scripts/build-ffmpeg-sidecar.sh (MORPHIX_TARGET=jammy) đóng gói.
# Usage (trong container, có quyền root và mạng): bash build-ffmpeg-jammy.sh
set -euo pipefail
export DEBIAN_FRONTEND=noninteractive
apt-get update -qq
apt-get build-dep -y -qq ffmpeg >/dev/null
# WHY: build-dep kéo mọi thư viện mà configure của ffmpeg 4.4 của jammy cần; phần còn lại bỏ theo configure.
# WHY: libx264 là lý do bản này phải là GPL; nasm/yasm để bật asm cho x86.
apt-get install -y -qq --no-install-recommends libx264-dev nasm yasm >/dev/null
cd /work
curl -fsSL -o ffmpeg-6.1.1.tar.xz https://ffmpeg.org/releases/ffmpeg-6.1.1.tar.xz
sha256sum ffmpeg-6.1.1.tar.xz | tee /work/ffmpeg-tarball.sha256
rm -rf ffmpeg-6.1.1 && tar -xJf ffmpeg-6.1.1.tar.xz && cd ffmpeg-6.1.1
# WHY: flag lấy từ ffmpeg -buildconf của gói Noble 7:6.1.1-3ubuntu5; bỏ --libdir/--incdir riêng của Ubuntu.
FLAGS=(--prefix=/opt/ffmpeg-6.1.1 --extra-version=morphix-ubuntu22.04 --toolchain=hardened --arch=amd64
--enable-gpl --disable-stripping --disable-omx --enable-gnutls --enable-libaom
--enable-libass --enable-libbs2b --enable-libcaca --enable-libcdio --enable-libcodec2
--enable-libdav1d --enable-libflite --enable-libfontconfig --enable-libfreetype
--enable-libfribidi --enable-libglslang --enable-libgme --enable-libgsm --enable-libharfbuzz
--enable-libmp3lame --enable-libmysofa --enable-libopenjpeg --enable-libopenmpt
--enable-libopus --enable-librubberband --enable-libshine --enable-libsnappy
--enable-libsoxr --enable-libspeex --enable-libtheora --enable-libtwolame
--enable-libvidstab --enable-libvorbis --enable-libvpx --enable-libwebp --enable-libx265
--enable-libxml2 --enable-libxvid --enable-libzimg --enable-openal --enable-opencl
--enable-opengl --disable-sndio --enable-libvpl --disable-libmfx --enable-libdc1394
--enable-libdrm --enable-libiec61883 --enable-chromaprint --enable-frei0r --enable-ladspa
--enable-libbluray --enable-libjack --enable-libpulse --enable-librabbitmq --enable-librist
--enable-libsrt --enable-libssh --enable-libsvtav1 --enable-libx264 --enable-libzmq
--enable-libzvbi --enable-lv2 --enable-sdl2 --enable-libplacebo --enable-librav1e
--enable-pocketsphinx --enable-librsvg --enable-libjxl --enable-shared --enable-rpath)
# WHY: configure dừng ở lib đầu tiên thiếu; bỏ đúng flag đó rồi chạy lại, để ghi rõ flag nào jammy không có.
dropped=()
while :; do
  if ./configure "${FLAGS[@]}" >/work/configure.log 2>&1; then break; fi
  missing="$(grep -m1 '^ERROR: .* not found' /work/configure.log || true)"
  if [ -z "$missing" ]; then tail -30 /work/configure.log; exit 1; fi
  lib="$(sed -E 's/^ERROR: ([A-Za-z0-9_.+-]+).*/\1/' <<<"$missing")"
  # WHY: configure báo tên nội bộ (spirv_compiler, SvtAv1Enc, rav1e) thay vì tên lib trong flag.
  case "$lib" in spirv_compiler) lib=libglslang ;; SvtAv1Enc) lib=libsvtav1 ;; rav1e) lib=librav1e ;; esac
  flag="$(grep -o -- "--enable-$lib\b\|--enable-lib$lib\b" <<<"${FLAGS[*]}" | head -1 || true)"
  [ -n "$flag" ] || { echo "không map được lỗi: $missing"; tail -30 /work/configure.log; exit 1; }
  kept=(); for f in "${FLAGS[@]}"; do [ "$f" = "$flag" ] || kept+=("$f"); done; FLAGS=("${kept[@]}")
  dropped+=("$flag")
  echo "bỏ $flag (thiếu: $missing)"
done
printf '%s\n' "${dropped[@]}" > /work/dropped-flags.txt
make -j"$(nproc)" >/work/make.log 2>&1
make install >/work/install.log 2>&1
mkdir -p /opt/ffmpeg-6.1.1/share/doc/ffmpeg-morphix
{ echo "ffmpeg 6.1.1 (upstream FFmpeg), build cho Morphix trên Ubuntu 22.04. Cấu hình GPL: --enable-gpl."; echo; cat COPYING.GPLv2 COPYING.LGPLv2.1; } > /opt/ffmpeg-6.1.1/share/doc/ffmpeg-morphix/copyright
echo "ok: $(/opt/ffmpeg-6.1.1/bin/ffmpeg -version | head -1)"
