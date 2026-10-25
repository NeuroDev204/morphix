#!/bin/sh
# Cài Morphix cho người dùng hiện tại, không cần sudo:
#   curl -fsSL https://github.com/NeuroDev204/morphix/releases/latest/download/install.sh | sh
# Chạy lại để cập nhật. Gỡ: morphix uninstall.
# Biến: MORPHIX_VERSION (vd v0.2.0; trống = mới nhất), MORPHIX_PREFIX (mặc định ~/.local),
#       MORPHIX_RELEASE_BASE, MORPHIX_OS_RELEASE, MORPHIX_SKIP_INTEGRATION (chỉ dùng khi test).
set -eu

ASSET=morphix-x86_64-linux.tar.gz
REPO_URL=https://github.com/NeuroDev204/morphix/releases
PREFIX="${MORPHIX_PREFIX:-$HOME/.local}"
OS_RELEASE="${MORPHIX_OS_RELEASE:-/etc/os-release}"
if [ -n "${MORPHIX_RELEASE_BASE:-}" ]; then
    BASE="$MORPHIX_RELEASE_BASE"
elif [ -n "${MORPHIX_VERSION:-}" ]; then
    BASE="$REPO_URL/download/$MORPHIX_VERSION"
else
    BASE="$REPO_URL/latest/download"
fi

fail() {
    printf 'Morphix: %s\n' "$1" >&2
    exit 1
}

# 1. Kiến trúc và hệ điều hành.
os="$(uname -s)"
arch="$(uname -m)"
[ "$os" = Linux ] && [ "$arch" = x86_64 ] \
    || fail "Morphix hiện chỉ hỗ trợ Linux x86_64 (máy này: $os $arch)."

# 2. glibc ≥ 2.35 (binary được dựng trên Ubuntu 22.04).
# WHY không dùng awk: ảnh tối giản (openSUSE Tumbleweed) không có awk.
glibc="$(getconf GNU_LIBC_VERSION 2>/dev/null || true)"
glibc="${glibc#glibc }"
[ -n "$glibc" ] || fail "Không xác định được phiên bản glibc; Morphix cần glibc 2.35 trở lên."
if [ "$(printf '%s\n2.35\n' "$glibc" | sort -V | head -1)" != 2.35 ]; then
    fail "Cần glibc 2.35 trở lên (máy này: $glibc)."
fi

# 3. Bản .deb và bản curl dùng chung đường dẫn tích hợp, không được cùng tồn tại.
if command -v dpkg-query >/dev/null 2>&1 \
    && dpkg-query -W -f='${Status}' morphix 2>/dev/null | grep -q 'install ok installed'; then
    fail "Máy đã cài Morphix bằng gói .deb. Hãy gỡ trước: sudo apt remove morphix"
fi

# 3b. Chỉ thay bản do lệnh cài này tạo (có marker); file trùng tên của ứng dụng khác thì dừng, không đụng vào.
marker="$PREFIX/lib/morphix/.morphix-install"
if [ -e "$PREFIX/lib/morphix" ] && [ ! -f "$marker" ]; then
    fail "$PREFIX/lib/morphix đã có nhưng không phải của Morphix; không ghi đè. Hãy dời thư mục này rồi chạy lại."
fi
if [ ! -f "$marker" ]; then
    for name in morphix morphix-gui; do
        [ ! -e "$PREFIX/bin/$name" ] \
            || fail "$PREFIX/bin/$name đã có nhưng không phải bản cài Morphix; không ghi đè. Hãy dời file này rồi chạy lại."
    done
fi

# 4. Thư viện hệ thống không đóng kèm được: chỉ in lệnh cài, không tự chạy sudo.
# WHY thử cả /sbin: PATH của người dùng thường trên Debian/openSUSE không có sbin, nơi chứa ldconfig.
libs=""
for candidate in ldconfig /sbin/ldconfig /usr/sbin/ldconfig; do
    if found="$("$candidate" -p 2>/dev/null)"; then
        libs="$found"
        break
    fi
done
missing=""
if [ -z "$libs" ]; then
    printf 'Morphix: không chạy được ldconfig để kiểm thư viện; nếu Morphix không mở, hãy cài webkit2gtk-4.1 và libayatana-appindicator3.\n' >&2
else
    for lib in libwebkit2gtk-4.1.so.0 libayatana-appindicator3.so.1; do
        printf '%s\n' "$libs" | grep -q "$lib" || missing="$missing $lib"
    done
fi
if [ -n "$missing" ]; then
    ids="$(. "$OS_RELEASE" 2>/dev/null; printf '%s %s' "${ID:-}" "${ID_LIKE:-}")"
    case " $ids " in
        *" fedora "*) cmd="sudo dnf install webkit2gtk4.1 libayatana-appindicator-gtk3" ;;
        *" arch "*) cmd="sudo pacman -S webkit2gtk-4.1 libayatana-appindicator" ;;
        *" opensuse"*|*" suse "*) cmd="sudo zypper install libwebkit2gtk-4_1-0 libayatana-appindicator3-1" ;;
        *" debian "*|*" ubuntu "*) cmd="sudo apt install libwebkit2gtk-4.1-0 libayatana-appindicator3-1" ;;
        *) cmd="" ;;
    esac
    printf 'Morphix cần thư viện hệ thống còn thiếu:%s\n' "$missing"
    if [ -n "$cmd" ]; then
        printf 'Hãy cài bằng lệnh sau rồi chạy lại lệnh cài Morphix:\n  %s\n' "$cmd"
    else
        printf 'Hãy cài các thư viện trên bằng trình quản lý gói của bản Linux này rồi chạy lại.\n'
    fi
    exit 1
fi

# 5. Tải và kiểm checksum trước khi đụng vào bản đang cài.
# WHY stage trong $PREFIX: cùng hệ tập tin nên các `mv` bên dưới là đổi tên nguyên tử; từ /tmp (tmpfs
# hoặc đĩa khác) `mv` thành copy và có thể để lại bản cài dở dang khi hết dung lượng.
mkdir -p "$PREFIX"
work="$(mktemp -d "$PREFIX/.morphix-stage.XXXXXX")"
trap 'rm -rf "$work"' EXIT
curl -fsSL -o "$work/$ASSET" "$BASE/$ASSET" || fail "Không tải được $BASE/$ASSET"
curl -fsSL -o "$work/$ASSET.sha256" "$BASE/$ASSET.sha256" || fail "Không tải được checksum"
(cd "$work" && sha256sum -c --status "$ASSET.sha256") \
    || fail "Checksum không khớp; bản tải về bị hỏng. Không thay đổi gì."
# WHY ×4: bản nén giải ra khoảng 3–4 lần, cộng thêm bản cũ sẽ được thay.
need_kb=$(( ($(wc -c < "$work/$ASSET") / 1024 + 1) * 4 ))
# WHY không awk: ảnh tối giản không có awk; dòng 2 của df là số liệu, cột 4 là dung lượng trống.
free_kb="$(df -Pk "$PREFIX" | sed -n 2p | tr -s ' ' | cut -d' ' -f4)"
[ "${free_kb:-0}" -ge "$need_kb" ] \
    || fail "Không đủ dung lượng trống trong $PREFIX (cần khoảng $((need_kb / 1024)) MB). Chưa thay đổi gì."
tar -xzf "$work/$ASSET" -C "$work"
new="$work/morphix"
version="$(cat "$new/VERSION")"

# 6. Tắt GUI đang chạy để không có GUI cũ nói chuyện với CLI mới.
gui_was_running=""
if pgrep -x morphix-gui >/dev/null 2>&1; then
    gui_was_running=1
    pkill -x morphix-gui || true
    i=0
    while pgrep -x morphix-gui >/dev/null 2>&1 && [ "$i" -lt 50 ]; do
        sleep 0.1
        i=$((i + 1))
    done
fi

# 7. Thay bản cũ. ponytail: mỗi bước là một rename nên cửa sổ hỏng rất nhỏ; làm giao dịch đầy đủ nếu có báo lỗi thực tế.
mkdir -p "$PREFIX/bin" "$PREFIX/lib" "$PREFIX/share/icons/hicolor/128x128/apps" "$PREFIX/share/doc"
printf '%s\n' "$version" > "$new/lib/morphix/.morphix-install"
rm -rf "$PREFIX/lib/morphix.new" "$PREFIX/lib/morphix.old" "$PREFIX/share/doc/morphix.new"
mv "$new/lib/morphix" "$PREFIX/lib/morphix.new"
mv "$new/share/doc/morphix" "$PREFIX/share/doc/morphix.new"
mv -f "$new/bin/morphix-gui" "$PREFIX/bin/morphix-gui"
mv -f "$new/bin/morphix" "$PREFIX/bin/morphix"
[ ! -e "$PREFIX/lib/morphix" ] || mv "$PREFIX/lib/morphix" "$PREFIX/lib/morphix.old"
mv "$PREFIX/lib/morphix.new" "$PREFIX/lib/morphix"
rm -rf "$PREFIX/lib/morphix.old" "$PREFIX/share/doc/morphix"
mv "$PREFIX/share/doc/morphix.new" "$PREFIX/share/doc/morphix"
mv -f "$new/share/icons/hicolor/128x128/apps/morphix.png" "$PREFIX/share/icons/hicolor/128x128/apps/morphix.png"

# 8. Menu chuột phải, launcher, autostart, Alt+C (tuỳ máy); in nguyên kết quả, kể cả mục không áp dụng.
if [ -z "${MORPHIX_SKIP_INTEGRATION:-}" ]; then
    "$PREFIX/bin/morphix" install-integration \
        || printf 'Morphix: một số mục tích hợp chưa cài được (xem ở trên); chạy lại: morphix install-integration\n' >&2
fi

# 9. Bật lại GUI nếu trước đó đang chạy.
if [ -n "$gui_was_running" ]; then
    nohup "$PREFIX/bin/morphix-gui" >/dev/null 2>&1 &
fi

# 10. Gợi ý PATH.
case ":$PATH:" in
    *":$PREFIX/bin:"*) ;;
    *) printf 'Thêm %s vào PATH để gõ được lệnh morphix:\n  echo '"'"'export PATH="$HOME/.local/bin:$PATH"'"'"' >> ~/.profile\n' "$PREFIX/bin" ;;
esac
printf 'Đã cài Morphix %s. Gỡ: morphix uninstall\n' "$version"
