# Morphix

Đổi định dạng, nén, đổi cỡ và xử lý file ngay trên máy Linux, không gửi gì ra ngoài. Bấm chuột phải lên file, chọn một mục trên vòng tròn, file kết quả nằm cạnh file gốc; file gốc không bao giờ bị ghi đè.

Hỗ trợ: ảnh (JPG, PNG, WebP, AVIF, GIF, TIFF, BMP, sang PDF), âm thanh và video (MP3, M4A, WAV, FLAC, OGG, OPUS, MP4, MKV, WebM, MOV, AVI, GIF), văn bản và phụ đề (TXT, MD, SRT, VTT, CSV, TSV), PDF sang TXT, nén/giải nén ZIP, TAR.GZ, 7Z.

## Cài đặt

```bash
curl -fsSL https://github.com/NeuroDev204/morphix/releases/latest/download/install.sh | sh
```

- Cài cho người dùng hiện tại vào `~/.local`, **không cần sudo**. Chạy lại đúng lệnh trên để cập nhật.
- Cần Linux x86_64, glibc 2.35 trở lên (Ubuntu 22.04+, Debian 12+, Fedora 36+, Arch, openSUSE Tumbleweed, Mint 21+…) và hai thư viện hệ thống `webkit2gtk-4.1`, `libayatana-appindicator3`. Thiếu thì lệnh cài in đúng lệnh `sudo` cho bản Linux của bạn để bạn tự chạy.
- Menu chuột phải được thêm cho trình quản lý file có trên máy: Nautilus (Files), Dolphin, Nemo, Thunar, Caja.
- Lệnh trên cần `curl` (Kubuntu 24.04 live không có sẵn; cài bằng `sudo apt install curl`).
- Menu Morphix trong Files (Nautilus) cần gói Python của Nautilus: `nautilus-python` (Fedora, Arch, openSUSE) hoặc `python3-nautilus` (Debian, Ubuntu). `morphix doctor` sẽ in đúng lệnh nếu thiếu.
- Trên GNOME không có icon khay (AppIndicator), cài extension `gnome-shell-extension-appindicator` rồi bật bằng `gnome-extensions enable appindicatorsupport@rgcjonas.gmail.com`, hoặc dùng `morphix tray`.
- Phím **Alt+C** (mở Morphix với các file đang chọn) chỉ có trên GNOME chạy X11. Trên Wayland và các desktop khác, dùng menu chuột phải.
- Ubuntu 24.04 còn có gói `.deb` trong mục Releases, kèm menu chuột phải ngay trên desktop.

### Theo từng bản Linux

Lệnh cài giống nhau cho mọi bản. Khác nhau là thư viện hệ thống và gói để Files (Nautilus) hiện menu Morphix. Các lệnh dưới đây là lệnh đã dùng khi kiểm thử; nếu `install.sh` in ra lệnh khác cho máy bạn, hãy chạy lệnh đó.

| Bản Linux | Thư viện còn thiếu | Gói Nautilus cho menu Files | Đã kiểm trên |
|---|---|---|---|
| Fedora 44 | `sudo dnf install webkit2gtk4.1 libayatana-appindicator-gtk3` | `sudo dnf install nautilus-python` | GNOME, Wayland |
| openSUSE Tumbleweed | `sudo zypper install libwebkit2gtk-4_1-0 libayatana-appindicator3-1` | `sudo zypper install nautilus-python` | GNOME, Wayland |
| Debian 13 | `sudo apt install libwebkit2gtk-4.1-0 libayatana-appindicator3-1` | `sudo apt install python3-nautilus` | GNOME, Wayland |
| Arch Linux | `sudo pacman -S webkit2gtk-4.1 libayatana-appindicator` | `sudo pacman -S nautilus-python` (chưa kiểm tên gói này) | XFCE, X11 |
| Ubuntu 24.04 và các biến thể (Kubuntu, Xubuntu, Ubuntu MATE, Linux Mint 22) | `sudo apt install libwebkit2gtk-4.1-0 libayatana-appindicator3-1` | `sudo apt install python3-nautilus` (chưa kiểm trên Ubuntu GNOME) | Kubuntu KDE, Xubuntu XFCE, Ubuntu MATE, Mint Cinnamon, X11 |

- **Lệnh curl cần `curl`.** Kubuntu 24.04 live không có sẵn: `sudo apt update && sudo apt install curl`.
- **Menu theo trình quản lý file:** GNOME Files → Scripts → Morphix (chuột phải); KDE Dolphin → Morphix; Xfce Thunar → Morphix. Cinnamon (Nemo), MATE (Caja) chưa kiểm được vì máy thử không có các trình quản lý này.
- **GNOME Wayland:** không có phím Alt+C; dùng chuột phải.
- **GNOME X11:** Alt+C mở Morphix với các file đang chọn (chưa kiểm trên máy thật).
- **Tray (icon khay) trên GNOME:** GNOME Fedora, Debian và openSUSE không có sẵn AppIndicator. Cài `gnome-shell-extension-appindicator`, chạy `gnome-extensions enable appindicatorsupport@rgcjonas.gmail.com`, rồi đăng nhập lại. Không muốn cài thì dùng `morphix tray`.
- **Kiểm tra lại:** `morphix doctor` liệt kê từng tích hợp và báo đúng lệnh còn thiếu.

Cài một phiên bản cụ thể: `curl -fsSL …/install.sh | MORPHIX_VERSION=v0.1.2 sh`.

## Gỡ cài đặt

```bash
morphix uninstall
```

Gỡ chương trình và các mục menu; dữ liệu của bạn (`~/.local/share/morphix`: lịch sử, cài đặt, log) được giữ lại. Bản cài bằng `.deb`: `sudo apt remove morphix`.

## Dòng lệnh

`morphix --help` liệt kê mọi lệnh, ví dụ `morphix convert --to webp ảnh.png`, `morphix doctor` để kiểm tra cài đặt.

## Giấy phép

Morphix phát hành theo [GPL-3.0-or-later](LICENSE). ffmpeg và PDFium đi kèm có giấy phép riêng, xem `packaging/LICENSES-THIRD-PARTY.md`.
