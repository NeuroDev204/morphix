#!/usr/bin/env bash
# Dựng gói .deb tự chứa: morphix, morphix-gui, ffmpeg GPL và PDFium đi kèm trong /usr/lib/morphix.
# Dùng dpkg-deb trực tiếp (không qua Tauri bundler) vì Tauri không đặt được ffmpeg và các .so
# vào đúng chỗ, và cần RPATH $ORIGIN cho từng thư viện.
# Usage: MORPHIX_TARGET=noble scripts/build-deb.sh [thư mục đầu ra]
# Menu Desktop ghim DING của Ubuntu 24.04; các bản Ubuntu khác chưa được đóng gói.
set -euo pipefail

readonly PDFIUM_SHA256='f728930966f503652b92acc89b9374a2eeca00ce42e26dccd3e4b5c5161b2d64'

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUT="${1:-$HOME/Documents/morphix-deb}"
OUT="$(realpath -m "$OUT")"
TARGET_DIR="${CARGO_TARGET_DIR:-$ROOT/target}"
VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' "$ROOT/Cargo.toml" | head -1)"
export MORPHIX_TARGET="${MORPHIX_TARGET:-noble}"
case "$MORPHIX_TARGET" in
  noble) ;;
  jammy) echo "Jammy chưa được hỗ trợ: tích hợp Desktop ghim DING của Ubuntu 24.04" >&2; exit 1 ;;
  *) echo "MORPHIX_TARGET không hợp lệ: $MORPHIX_TARGET" >&2; exit 1 ;;
esac
readonly DEB_REVISION=3
DEB="$OUT/morphix_${VERSION}-${DEB_REVISION}_amd64.deb"

# WHY: sản phẩm và cây đóng gói không được nằm trong repo, để không lỡ commit binary.
case "$OUT" in
  "$ROOT"|"$ROOT"/*) echo "thư mục đầu ra không được nằm trong repo: $OUT" >&2; exit 1 ;;
esac

mkdir -p "$OUT"
if [ -e "$DEB" ] || [ -L "$DEB" ]; then
  echo "tệp đầu ra đã tồn tại, không ghi đè: $DEB" >&2
  exit 1
fi
STAGE="$(mktemp -d "$OUT/stage.XXXXXX")"
mkdir -p "$STAGE/DEBIAN"

(cd "$ROOT/crates/morphix-gui/ui" && npm run build)
# WHY: custom-protocol để binary nhúng ui/dist; thiếu nó thì cửa sổ nạp localhost:5173 và lỗi kết nối.
(cd "$ROOT" && cargo build --locked --release -p morphix-cli -p morphix-gui --features morphix-gui/custom-protocol)

install -Dm0755 "$TARGET_DIR/release/morphix" "$STAGE/usr/bin/morphix"
install -Dm0755 "$TARGET_DIR/release/morphix-gui" "$STAGE/usr/bin/morphix-gui"
# WHY: $ORIGIN/../lib/morphix đi từ /usr/bin tới thư viện đi kèm, không phụ thuộc nơi cài.
for bin in "$STAGE/usr/bin/morphix" "$STAGE/usr/bin/morphix-gui"; do
  patchelf --set-rpath '$ORIGIN/../lib/morphix/lib:$ORIGIN/../lib/morphix/pdfium' "$bin"
done
# WHY: chặn gói lỗi ngay trong lúc build; thiếu custom-protocol thì cửa sổ Khay không mở được.
bash "$ROOT/scripts/check-gui-embeds-ui.sh" "$STAGE/usr/bin/morphix-gui"

# WHY: PDFium được kiểm theo sha256 đã ghi trong Ruling; file khác thì không đóng gói.
echo "$PDFIUM_SHA256  $ROOT/third_party/pdfium/lib/libpdfium.so" | sha256sum -c --status -
install -Dm0644 "$ROOT/third_party/pdfium/lib/libpdfium.so" "$STAGE/usr/lib/morphix/pdfium/libpdfium.so"

bash "$ROOT/scripts/build-ffmpeg-sidecar.sh" "$STAGE"

DING_SOURCE="${MORPHIX_DING_SOURCE:-/usr/share/gnome-shell/extensions/ding@rastersoft.com}"
DING_PREPARE_ROOT="$(mktemp -d "$STAGE/.ding-build.XXXXXX")"
DING_PREPARED="$DING_PREPARE_ROOT/payload"
python3 "$ROOT/scripts/prepare-ding.py" --source "$DING_SOURCE" --output "$DING_PREPARED"
for relative in app/fileItemMenu.js app/desktopManager.js; do
  install -Dm0644 "$DING_PREPARED/$relative" "$STAGE/usr/share/gnome-shell/extensions/ding@rastersoft.com/$relative"
done
install -Dm0644 "$DING_PREPARED/manifest.json" "$STAGE/usr/share/morphix/ding/manifest.json"
install -Dm0644 "$DING_PREPARED/LICENSE" "$STAGE/usr/share/doc/morphix/ding/LICENSE"
python3 -c 'import shutil,sys; shutil.rmtree(sys.argv[1])' "$DING_PREPARE_ROOT"

install -Dm0644 "$ROOT/integration/nautilus/morphix_menu.py" \
  "$STAGE/usr/share/nautilus-python/extensions/morphix_menu.py"
install -Dm0644 "$ROOT/packaging/morphix.desktop" "$STAGE/usr/share/applications/morphix.desktop"
install -Dm0644 "$ROOT/docs/ubuntu-install.md" "$STAGE/usr/share/doc/morphix/ubuntu-install.md"
install -Dm0644 "$ROOT/crates/morphix-gui/icons/128x128.png" \
  "$STAGE/usr/share/icons/hicolor/128x128/apps/morphix.png"

DOC="$STAGE/usr/share/doc/morphix"
install -Dm0644 "$ROOT/crates/morphix-gui/packaging/deb/copyright" "$DOC/copyright"
install -Dm0644 "$ROOT/packaging/LICENSES-THIRD-PARTY.md" "$DOC/LICENSES-THIRD-PARTY.md"
python3 "$ROOT/scripts/collect-bundled-licenses.py" "$DOC/bundled"
OFFER="$ROOT/packaging/ffmpeg-source-offer.txt"
install -Dm0644 "$OFFER" "$DOC/ffmpeg-source-offer.txt"
for licence in "$ROOT"/third_party/pdfium/licenses/*; do
  install -Dm0644 "$licence" "$DOC/pdfium/licenses/$(basename "$licence")"
done
install -Dm0644 "$ROOT/third_party/pdfium/LICENSE" "$DOC/pdfium/LICENSE"

# WHY: phiên bản glibc tối thiểu lấy từ symbol thực sự mà binary và thư viện đóng gói cần.
# ffmpeg bản Noble cần GLIBC_2.38, nên libc6 phải khai báo đúng mức này để apt từ chối cài trên
# máy cũ thay vì để lỗi xuất hiện lúc chạy.
glibc_floor="$(find "$STAGE/usr/bin" "$STAGE/usr/lib" -type f -print0 | xargs -0 -r -n1 objdump -T \
  | awk '/UND/' | grep -o 'GLIBC_2\.[0-9]*' | sort -uV | tail -1 | sed 's/GLIBC_//')"
# WHY: GLIBCXX_3.4.N xuất hiện từ GCC (N - 19) trở đi, nên libstdc++6 phải đủ mới. Thiếu mục này
# thì apt vẫn cho cài trên máy có glibc đủ mà libstdc++ cũ, và ffmpeg lỗi khi chạy.
cxx_minor="$(find "$STAGE/usr/bin" "$STAGE/usr/lib" -type f -print0 | xargs -0 -r -n1 objdump -T \
  | awk '/UND/' | grep -o 'GLIBCXX_3\.4\.[0-9]*' | sort -uV | tail -1 | sed 's/GLIBCXX_3\.4\.//')"
cxx_major="$((cxx_minor - 19))"
# WHY: Noble cung cấp GLIBCXX_3.4.32 từ GCC 13; lấy mức tối thiểu trực tiếp từ symbol binary cần.
installed_kb="$(du -sk --exclude=DEBIAN "$STAGE" | cut -f1)"
ding_version="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1], encoding="utf-8"))["version"])' "$ROOT/integration/ding/source.json")"
sed -e "s/@VERSION@/${VERSION}-${DEB_REVISION}/" \
    -e "s/@INSTALLED_SIZE@/${installed_kb}/" \
    -e "s/@DEPENDS@/libwebkit2gtk-4.1-0, libayatana-appindicator3-1, libglib2.0-bin, python3-nautilus, python3-gi, gir1.2-gtk-4.0, gnome-shell-extension-desktop-icons-ng (= ${ding_version}), libc6 (>= ${glibc_floor}), libstdc++6 (>= ${cxx_major})/" \
    "$ROOT/crates/morphix-gui/packaging/deb/control.in" > "$STAGE/DEBIAN/control"
python3 "$ROOT/scripts/render-dpkg-scripts.py" --output "$STAGE/DEBIAN"

# WHY: ngày sửa file cố định theo commit, để chạy lại cho cùng một file .deb (tái lập được).
# WHY: container build không có .git của worktree, nên cho phép truyền epoch từ ngoài vào.
epoch="${SOURCE_DATE_EPOCH:-$(git -C "$ROOT" log -1 --format=%ct)}"
find "$STAGE" -exec touch -h -d "@$epoch" {} +

dpkg-deb --root-owner-group --build "$STAGE" "$DEB"
sha256sum "$DEB" > "$DEB.sha256"
python3 -c 'import shutil,sys; shutil.rmtree(sys.argv[1])' "$STAGE"
echo "built $DEB ($(stat -c %s "$DEB") bytes), libc floor GLIBC_$glibc_floor, libstdc++ floor GCC $cxx_major"
