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

Cài một phiên bản cụ thể: `curl -fsSL …/install.sh | MORPHIX_VERSION=v0.2.0 sh`.

## Gỡ cài đặt

```bash
morphix uninstall
```

Gỡ chương trình và các mục menu; dữ liệu của bạn (`~/.local/share/morphix`: lịch sử, cài đặt, log) được giữ lại. Bản cài bằng `.deb`: `sudo apt remove morphix`.

## Dòng lệnh

`morphix --help` liệt kê mọi lệnh, ví dụ `morphix convert --to webp ảnh.png`, `morphix doctor` để kiểm tra cài đặt.

## Giấy phép

Morphix phát hành theo [GPL-3.0-or-later](LICENSE). ffmpeg và PDFium đi kèm có giấy phép riêng, xem `packaging/LICENSES-THIRD-PARTY.md`.
