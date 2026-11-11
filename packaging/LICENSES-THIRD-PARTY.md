# Giấy phép các thành phần đi kèm (Linux: gói `.deb` và tarball cài bằng lệnh curl)

Tệp này liệt kê mọi thứ mà gói `morphix_<phiên bản>_amd64.deb` và tarball `morphix-x86_64-linux.tar.gz` mang theo,
ngoài mã của Morphix. Đây là bản ghi kỹ thuật để rà soát, **không phải tư vấn pháp lý**.

## 0. Morphix

Mã của Morphix phát hành theo **GPL-3.0-or-later** (`LICENSE`), tương thích với các thành phần GPL-2.0-or-later bên dưới.

## 1. Thành phần chính

Đường dẫn ghi theo gói `.deb`; trong tarball, `/usr/lib/morphix` là `lib/morphix` và `/usr/share/doc/morphix` là
`share/doc/morphix`.

| Thành phần | Phiên bản | Giấy phép | Đường dẫn | Ghi chú |
|---|---|---|---|---|
| ffmpeg gọn của Morphix | FFmpeg 8.1.3 (`morphix.1`) | GPL-3.0-or-later (`--enable-gpl` vì có libx264) | `/usr/lib/morphix/ffmpeg` | Dựng từ nguồn bằng `scripts/build-ffmpeg-slim.sh`, liên kết tĩnh hoàn toàn, chạy như tiến trình riêng. Giấy phép: `/usr/share/doc/morphix/ffmpeg/LICENSE.txt`; cấu hình: `ffmpeg/configuration.txt`; nguồn từng thành phần: `ffmpeg/BUILD-INFO.txt` |
| x264 (trong ffmpeg) | gói Ubuntu 22.04 `libx264-dev` | GPL-2.0-or-later | trong `ffmpeg` | Văn bản: `/usr/share/doc/morphix/ffmpeg/third-party/x264/` |
| libvpx, opus, libogg, libvorbis (trong ffmpeg) | gói Ubuntu 22.04 | BSD-3-Clause (libvpx kèm PATENTS) | trong `ffmpeg` | `ffmpeg/third-party/<thư viện>/` |
| lame (trong ffmpeg) | gói Ubuntu 22.04 `libmp3lame-dev` | LGPL-2.0-or-later | trong `ffmpeg` | `ffmpeg/third-party/lame/` |
| dav1d (trong ffmpeg) | 1.5.1, dựng từ nguồn | BSD-2-Clause | trong `ffmpeg` | `ffmpeg/third-party/dav1d/` |
| zlib (trong ffmpeg) | gói Ubuntu 22.04 `zlib1g-dev` | Zlib | trong `ffmpeg` | `ffmpeg/third-party/zlib/` |
| PDFium (`libpdfium.so`) | chromium/7881, VERSION 151.0.7881.0 | BSD kiểu 3 điều khoản (The PDFium Authors); bản dựng pdfium-binaries là MIT (Benoit Blanchon) | `/usr/lib/morphix/pdfium/libpdfium.so` | Nạp bằng `dlopen`. Văn bản: `/usr/share/doc/morphix/pdfium/` (LICENSE và `licenses/`) |
| Crate Rust và gói npm liên kết vào binary | xem `Cargo.lock`, `package-lock.json` | phần lớn MIT OR Apache-2.0 | trong `morphix`, `morphix-gui` | Văn bản sinh lúc dựng bằng `scripts/collect-bundled-licenses.py`: `/usr/share/doc/morphix/bundled/` |

Văn bản giấy phép của các thư viện trong ffmpeg lấy từ đúng các tarball nguồn đã ghim bằng
`scripts/collect-ffmpeg-licenses.py` (bản lưu trong kho: `packaging/ffmpeg-licenses/`). Bản Windows (`ffmpeg.exe`) dựng
cùng bộ tính năng bằng `scripts/build-ffmpeg-slim-windows.sh`, với các thư viện dựng từ nguồn ghi trong BUILD-INFO.txt.

## 2. Thư viện hệ thống không đóng gói (lấy qua Depends)

Gói `.deb` khai báo `libwebkit2gtk-4.1-0`, `libayatana-appindicator3-1` (cho morphix-gui) cùng các gói tích hợp
desktop; WebKitGTK và GTK không được đóng gói. Mức `libc6` tối thiểu tính lúc dựng từ symbol mà `morphix` và
`morphix-gui` cần (gói `.deb` dựng trên Ubuntu 24.04; tarball dựng trong Ubuntu 22.04 nên cần glibc 2.35).
ffmpeg liên kết tĩnh nên không thêm yêu cầu nào về glibc hay libstdc++.

## 3. Mã nguồn tương ứng

- ffmpeg và các thư viện trong nó: lời đề nghị và đường dẫn nguồn ở `/usr/share/doc/morphix/ffmpeg-source-offer.txt`
  (bản gốc: `packaging/ffmpeg-static.txt`); nguồn và SHA256 của từng thành phần ở `ffmpeg/BUILD-INFO.txt`; script dựng
  ở tag `ffmpeg-8.1.3-morphix.1` của kho Morphix.
- PDFium: tag `chromium/7881` của bblanchon/pdfium-binaries, tệp `pdfium-linux-x64.tgz`,
  sha256 `1470e21b8b4a3b4ad7f85684e2da11d94f3b69a86d81dee11b9b6709d927ac1d`;
  `libpdfium.so` sha256 `f728930966f503652b92acc89b9374a2eeca00ce42e26dccd3e4b5c5161b2d64`.

## 4. Ràng buộc đã biết (Ruling)

- **Ruling: ffmpeg do Morphix tự dựng.** Thay cho ffmpeg của Ubuntu (gói `.deb` cũ mang hơn 50 thư viện `.so`) và bản
  dựng sẵn của BtbN (tarball cũ, 172 MB): chỉ bật bộ mã hoá và muxer Morphix gọi, giữ mọi bộ giải mã. Gói nhẹ hơn
  nhiều và hai gói Linux dùng đúng một binary.
- **Ruling: không đóng gói WebKitGTK và các gói GTK.** Chúng nằm trong Depends.

## 5. Việc còn lại

1. Rà soát giấy phép đầy đủ (crate Rust trong `Cargo.lock`, gói npm) trước mỗi lần phát hành lớn. Đây là việc của người
   phát hành, không phải kết luận của tài liệu này.
2. Cân nhắc đính kèm mã nguồn GPL thực sự cùng bản phát hành, ngoài lời đề nghị.
