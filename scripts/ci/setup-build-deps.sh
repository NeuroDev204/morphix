#!/usr/bin/env bash
# Cài thư viện dựng Morphix và tải PDFium đã ghim (dùng chung cho CI và container Ubuntu 22.04).
# Chạy bằng root (container) hoặc qua sudo (runner GitHub).
set -euo pipefail

readonly PDFIUM_URL='https://github.com/bblanchon/pdfium-binaries/releases/download/chromium%2F7881/pdfium-linux-x64.tgz'
readonly PDFIUM_TGZ_SHA256='1470e21b8b4a3b4ad7f85684e2da11d94f3b69a86d81dee11b9b6709d927ac1d'

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
SUDO=""
[ "$(id -u)" = 0 ] || SUDO=sudo

$SUDO apt-get update
DEBIAN_FRONTEND=noninteractive $SUDO apt-get install -y --no-install-recommends \
  build-essential pkg-config curl ca-certificates file xz-utils patchelf binutils git \
  libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev libssl-dev libxdo-dev \
  python3 python3-gi gir1.2-gtk-4.0 desktop-file-utils

mkdir -p "$ROOT/third_party/pdfium"
curl -fsSL -o /tmp/pdfium.tgz "$PDFIUM_URL"
echo "$PDFIUM_TGZ_SHA256  /tmp/pdfium.tgz" | sha256sum -c -
tar -xzf /tmp/pdfium.tgz -C "$ROOT/third_party/pdfium"
