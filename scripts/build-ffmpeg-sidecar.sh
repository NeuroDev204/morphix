#!/usr/bin/env bash
# Chép ffmpeg GPL (có libx264) và các .so mà nó cần vào cây đóng gói.
# Usage: [MORPHIX_TARGET=noble|jammy] scripts/build-ffmpeg-sidecar.sh <thư mục gốc đóng gói>
set -euo pipefail

# WHY: noble dùng ffmpeg của Ubuntu; jammy không có ffmpeg 6.x nên dùng bản build từ nguồn ở prefix riêng.
TARGET="${MORPHIX_TARGET:-noble}"
case "$TARGET" in
  noble)
    FFMPEG_BIN=/usr/bin/ffmpeg
    FFMPEG_PREFIX=""
    RELY_FILE=rely-packages.txt
    ;;
  jammy)
    FFMPEG_BIN=/opt/ffmpeg-6.1.1/bin/ffmpeg
    FFMPEG_PREFIX=/opt/ffmpeg-6.1.1
    RELY_FILE=rely-packages-jammy.txt
    ;;
  *) echo "MORPHIX_TARGET không hợp lệ: $TARGET" >&2; exit 1 ;;
esac
readonly FFMPEG_BIN FFMPEG_PREFIX

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
STAGE="${1:?usage: build-ffmpeg-sidecar.sh <thư mục gốc đóng gói>}"
RELY_LIST="$ROOT/crates/morphix-gui/packaging/deb/$RELY_FILE"
LIB_DIR="$STAGE/usr/lib/morphix"
DOC_DIR="$STAGE/usr/share/doc/morphix/third-party"

# WHY: chỉ đóng gói đúng bản đã kiểm (cấu hình GPL có libx264). Bản khác có thể khác encoder,
# và giấy phép cùng nguồn mã nguồn cần khớp với gói đã ghi trong LICENSES-THIRD-PARTY.md.
if [ "$TARGET" = noble ]; then
  readonly FFMPEG_PKG_VERSION='7:6.1.1-3ubuntu5'
  installed_version="$(dpkg-query -W -f='${Version}' ffmpeg)"
  if [ "$installed_version" != "$FFMPEG_PKG_VERSION" ]; then
    echo "ffmpeg đang cài là $installed_version, cần $FFMPEG_PKG_VERSION" >&2
    exit 1
  fi
else
  version_line="$("$FFMPEG_BIN" -hide_banner -version | head -1)"
  case "$version_line" in
    "ffmpeg version 6.1.1"*) ;;
    *) echo "ffmpeg tại $FFMPEG_BIN không phải 6.1.1: $version_line" >&2; exit 1 ;;
  esac
fi

# Bao đóng ldd, không dùng LD_LIBRARY_PATH: lấy đúng thư viện hệ thống mà ffmpeg sẽ dùng.
declare -A seen=()
queue=("$FFMPEG_BIN")
closure=()
while [ "${#queue[@]}" -gt 0 ]; do
  current="${queue[0]}"
  queue=("${queue[@]:1}")
  while read -r soname arrow resolved _; do
    [ "$arrow" = "=>" ] || continue
    [ "$resolved" = "not" ] && { echo "thiếu thư viện cho $current: $soname" >&2; exit 1; }
    case "$resolved" in /*) ;; *) continue ;; esac
    real="$(readlink -f "$resolved")"
    if [ -z "${seen[$real]:-}" ]; then
      seen[$real]=1
      closure+=("$resolved")
      queue+=("$real")
    fi
  done < <(env -u LD_LIBRARY_PATH ldd "$current")
done

# WHY: thư viện thuộc gói đã có trong Depends (WebKitGTK kéo về) không đóng gói lại, để không
# hai bản cùng soname trên máy. Gói ngoài danh sách thì đóng gói kèm, kể cả libx264, libvpx.
mkdir -p "$LIB_DIR/lib" "$DOC_DIR"
declare -A documented=()
for resolved in "${closure[@]}"; do
  real="$(readlink -f "$resolved")"
  # WHY: libav* tự build nằm trong prefix ffmpeg, không có gói dpkg nào sở hữu; gom chúng thành một
  # "gói" tên riêng để giữ đúng cơ chế kiểm và ghi giấy phép như các gói hệ thống.
  if [ -n "$FFMPEG_PREFIX" ] && [[ "$real" == "$FFMPEG_PREFIX"/* ]]; then
    owner=ffmpeg-morphix-build
    copyright_src="$FFMPEG_PREFIX/share/doc/ffmpeg-morphix/copyright"
  else
    # WHY: jammy không usrmerge nên dpkg ghi đường dẫn như ldd in ra (/lib/...), không phải đường dẫn đã resolve.
    owner="$(dpkg -S "$resolved" 2>/dev/null | head -1 | cut -d: -f1 || true)"
    [ -n "$owner" ] || owner="$(dpkg -S "$real" 2>/dev/null | head -1 | cut -d: -f1 || true)"
    copyright_src="/usr/share/doc/$owner/copyright"
  fi
  if [ -z "$owner" ]; then
    echo "không tìm thấy gói sở hữu $resolved, dừng để không đóng gói mù" >&2
    exit 1
  fi
  if grep -qxF "$owner" <(grep -v '^#' "$RELY_LIST"); then
    continue
  fi
  soname="$(basename "$resolved")"
  install -m 0644 "$real" "$LIB_DIR/lib/$soname"
  if [ -z "${documented[$owner]:-}" ]; then
    documented[$owner]=1
    mkdir -p "$DOC_DIR/$owner"
    install -m 0644 "$copyright_src" "$DOC_DIR/$owner/copyright"
  fi
done

# WHY: $ORIGIN làm cho thư viện tìm đồng đội cạnh nó, không phụ thuộc đường dẫn cài đặt.
# Mọi .so đóng gói cần RUNPATH này, vì ffmpeg chỉ có RUNPATH cho chính nó.
shopt -s nullglob
for lib in "$LIB_DIR"/lib/*.so*; do
  strip --strip-unneeded "$lib"
  patchelf --set-rpath '$ORIGIN' "$lib"
done

install -m 0755 "$FFMPEG_BIN" "$LIB_DIR/ffmpeg"
strip --strip-unneeded "$LIB_DIR/ffmpeg"
patchelf --set-rpath '$ORIGIN/lib' "$LIB_DIR/ffmpeg"

echo "ffmpeg đóng gói: ${#closure[@]} thư viện trong bao đóng, ${#documented[@]} gói được đóng gói kèm"

# WHY: bản jammy build từ nguồn nên cấu hình biên dịch không có sẵn ở đâu khác trên máy; ghi kèm để
# người nhận có đủ thông tin đối chiếu với mã nguồn (điều kiện GPL).
if [ "$TARGET" = jammy ]; then
  "$FFMPEG_BIN" -hide_banner -buildconf > "$STAGE/usr/share/doc/morphix/ffmpeg-buildconf.txt"
fi
