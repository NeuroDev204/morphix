# install.sh tự cài và bật AppIndicator của GNOME: thiết kế (bản ngắn)

Ngày: 2026-11-27. Tiếp nối `2026-11-27-morphix-install-auto-deps-design.md`.

## Ý định

Chủ dự án muốn: trên các bản Linux khác, thiếu bất kỳ thứ gì như Nautilus binding thì cũng hỏi rồi cài.
Thứ còn thiếu duy nhất cần gói hệ thống là extension AppIndicator của GNOME (không có nó, icon khay không hiện).
Chủ dự án chọn phạm vi A: chỉ 4 họ đã hỗ trợ (Debian/Ubuntu, Fedora, openSUSE, Arch); bản khác giữ nguyên.

## Đã kiểm trong container (2026-11-27)

| Họ | Nautilus binding | Gói AppIndicator | UUID |
|---|---|---|---|
| Ubuntu 26.04 | python3-nautilus | gnome-shell-ubuntu-extensions | ubuntu-appindicators@ubuntu.com |
| Debian 13 | python3-nautilus | gnome-shell-extension-appindicator | appindicatorsupport@rgcjonas.gmail.com |
| Fedora | nautilus-python | gnome-shell-extension-appindicator | appindicatorsupport@rgcjonas.gmail.com |
| openSUSE TW | **python3-nautilus** (capability → python313-nautilus; `nautilus-python` không tồn tại) | **không có gói** | — |
| Arch | **nautilus-python** (`python-nautilus` không tồn tại) | gnome-shell-extension-appindicator | appindicatorsupport@rgcjonas.gmail.com |

→ Sửa luôn tên gói Nautilus sai của openSUSE và Arch trong `install.sh`, README và gợi ý Rust (`linux.rs:1132`).

## Cách làm (một cách hợp lý)

Mở rộng bước 4 của `install.sh` (đã có hỏi/sudo/fallback). Rust bị loại như lần trước.

## Thiết kế

1. Chỉ khi `XDG_CURRENT_DESKTOP` chứa `GNOME` và có `gnome-extensions`.
2. `gnome-extensions list --enabled` có một trong 2 UUID → không làm gì.
3. `gnome-extensions list` có UUID nhưng chưa bật → bật ngay, không sudo, không hỏi.
4. Chưa cài và họ có gói → thêm gói vào câu hỏi chung; cài xong thì bật UUID của họ, cuối in
   "Đăng xuất rồi đăng nhập lại để thấy icon Morphix trên khay".
5. Bật: `gnome-extensions enable`; lỗi (extension vừa cài chưa được Shell nạp) → thêm UUID vào
   `gsettings org.gnome.shell enabled-extensions`.
6. Chưa cài và họ không có gói (openSUSE) → cuối in gợi ý cài từ extensions.gnome.org; không chặn.
7. Từ chối/lỗi cài, không thiếu thư viện bắt buộc → vẫn cài Morphix, in phần nào sẽ thiếu (menu Scripts / icon khay).

## Không làm

Tự đăng xuất; extension khác; thêm bản Linux mới; `apt update`.
