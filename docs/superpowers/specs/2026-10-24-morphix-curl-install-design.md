# Morphix cài bằng một lệnh curl cho mọi distro Linux

Ngày: 2026-10-24. Trạng thái: chờ duyệt. Đã qua một vòng soát của người build độc lập.

## Ý định

Phát hành Morphix cho mọi người dùng Linux, không chỉ Ubuntu. Người dùng chỉ cần dán một lệnh, giống cách cài Claude Code hay nvm:

```bash
curl -fsSL https://github.com/<owner>/morphix/releases/latest/download/install.sh | sh
```

Hiện Morphix chỉ có `.deb` cho Ubuntu 22.04/24.04 (amd64), cài vào `/usr`, và Alt+C chỉ chạy trên X11. Người dùng Fedora, Arch, openSUSE, Debian… chưa có cách cài.

## Mục tiêu

1. Một lệnh `curl … | sh` cài Morphix riêng cho người dùng hiện tại, vào thư mục home, không cần `sudo`.
2. Chạy trên mọi distro amd64 có glibc ≥ 2.35 và `webkit2gtk-4.1`: Ubuntu 22.04+, Debian 12+, Fedora 36+, Arch, openSUSE Tumbleweed, Mint 21+ và các bản tương đương.
3. Menu chuột phải trong Nautilus, Dolphin, Nemo, Thunar, Caja; chỉ cài cho trình quản lý file có trên máy.
4. Alt+C chỉ trên GNOME chạy X11, như hiện tại. Wayland và mọi desktop không phải GNOME (KDE, XFCE, Cinnamon, MATE) chỉ có menu chuột phải; script ghi rõ phần bị bỏ qua.
5. Chạy lại đúng lệnh cài là cập nhật. `morphix uninstall` gỡ sạch.
6. Repo GitHub công khai riêng cho Morphix, giấy phép GPL-3.0-or-later, file phát hành trên GitHub Releases, build tự động bằng GitHub Actions khi gắn tag.

## Không làm (bản này)

- ARM/aarch64 (để sau; ffmpeg đóng kèm cho ARM tốn công nhất).
- Menu trên desktop kiểu DING cho bản curl: cần sửa file hệ thống, trái nguyên tắc không-admin. Bản `.deb` vẫn giữ tính năng này.
- Alt+C trên Wayland. Trên Wayland app không được giả lập phím hay đọc lựa chọn của app khác.
- Alt+C trên KDE, XFCE, Cinnamon, MATE (kể cả X11): cần cơ chế phím tắt và cách lấy lựa chọn riêng cho từng desktop; để sau.
- Flatpak, Snap, AppImage, `.rpm`, kho AUR. Có thể thêm sau, không thuộc bản này.
- Tự động cập nhật ngầm.

## Phản mục tiêu (làm thế này là hỏng, dù chạy được)

- Script tự chạy `sudo` hoặc ghi ra ngoài home người dùng. Thiếu thư viện hệ thống thì chỉ **in** lệnh cài cho người dùng tự chạy.
- Ghi đè hoặc xoá file của ứng dụng khác. Mọi file tích hợp phải mang ownership marker như hiện tại; gỡ chỉ xoá file có marker.
- Cài dở dang: tải hỏng, sai checksum hoặc thiếu thư viện thì dừng trước khi đụng vào bản đang cài, và báo rõ lý do.
- Gửi dữ liệu người dùng ra ngoài. Mạng chỉ dùng lúc tải bản cài; Morphix sau khi cài vẫn hoàn toàn offline, không telemetry.
- Vi phạm GPL: phải kèm giấy phép, thông tin bên thứ ba và lời đề nghị cung cấp mã nguồn ffmpeg.

## Ràng buộc

- Giữ nguyên quy ước repo: `cargo clippy -D warnings`, `cargo test --workspace`, `cargo check` cho Windows; lỗi phải hiện cho người dùng; chuỗi giao diện tiếng Việt.
- `bundled_lib_dir()` (`crates/morphix-core/src/paths.rs`) đã tìm thư viện theo vị trí binary: `<prefix>/bin/morphix` → `<prefix>/lib/morphix`. Với prefix `~/.local` thì ffmpeg nằm ở `~/.local/lib/morphix/ffmpeg`, PDFium ở `~/.local/lib/morphix/pdfium`. Bố cục bản curl phải khớp quy ước này.
- `webkit2gtk-4.1` và `libayatana-appindicator3` là thư viện hệ thống, không đóng kèm.
- ffmpeg đóng kèm có libx264 (GPL). Đây là lý do Morphix dùng GPL.

## Đã quyết (của chủ dự án)

| Vấn đề | Quyết định |
|---|---|
| Cách cài | Một lệnh `curl … \| sh` |
| Phạm vi | Mọi người dùng Linux |
| Thiếu `webkit2gtk` | Script dừng, in lệnh cài đúng distro (`apt`, `dnf`, `pacman`, `zypper`…) kèm `sudo` để người dùng tự chạy |
| Wayland | Chỉ menu chuột phải, không có Alt+C |
| Alt+C | Chỉ GNOME trên X11; desktop khác chỉ menu chuột phải |
| Trình quản lý file | Nautilus, Dolphin, Nemo, Thunar, Caja |
| Nơi phát hành | Repo GitHub công khai riêng, GitHub Releases |
| Giấy phép | GPL-3.0-or-later, giữ libx264 để có MP4 H.264. Đặt `license` trong mọi crate, file `LICENSE`, giữ thông tin bên thứ ba |

## Do người viết thiết kế chọn (chưa được chủ dự án duyệt riêng)

- Chỉ amd64 ở bản đầu.
- Build trên nền Ubuntu 22.04 (glibc 2.35), bản cũ nhất có `webkit2gtk-4.1`. Dùng lại `scripts/build-ffmpeg-jammy.sh` cho ffmpeg.
- Chạy lại lệnh cài là cập nhật; thêm lệnh `morphix uninstall`.
- GitHub Actions build khi gắn tag `v*`.
- Vẫn giữ `.deb` cho Ubuntu (có menu DING). Workflow phát hành build cả tarball cho bản curl lẫn `.deb` (bản 24.04 và jammy, chạy `check-deb.sh`) và đính kèm vào cùng Release.
- Máy đã có bản `.deb` (`/usr/bin/morphix` thuộc gói `morphix`): script curl dừng, nhắc chạy `sudo apt remove morphix` trước. Hai bản không cùng tồn tại vì chung đường dẫn và marker tích hợp. `morphix uninstall` chạy từ bản `.deb` thì chỉ gỡ tích hợp và nhắc dùng `apt remove`, không đụng `/usr`.
- `morphix-gui` đang chạy: khi cập nhật, script tắt nó trước khi thay file rồi khởi động lại; khi gỡ, tắt nó trước khi xoá. Nhờ vậy không có GUI cũ nói chuyện với CLI mới qua IPC.

## Hành vi script cài

1. Kiểm tra: Linux, amd64, glibc ≥ 2.35, có `curl`/`tar`. Sai thì dừng, nói rõ lý do.
2. Kiểm tra `webkit2gtk-4.1` (và appindicator). Thiếu thì nhận diện distro qua `/etc/os-release` và in lệnh cài cho người dùng tự chạy, rồi dừng.
3. Tải gói tar của bản mới nhất (hoặc phiên bản chỉ định) cùng checksum; kiểm checksum trước khi giải nén.
4. Giải nén vào thư mục tạm, rồi thay bản cũ một lần để không có trạng thái nửa cũ nửa mới.
5. Nếu `~/.local/bin` chưa có trong `PATH`, in hướng dẫn thêm vào.
6. Gọi `morphix install-integration` để cài phần tích hợp phù hợp với máy (trình quản lý file có mặt, X11 hay Wayland, autostart, launcher trong danh sách ứng dụng).
7. In tóm tắt: đã cài gì, cái gì bị bỏ qua và vì sao (ví dụ “Wayland: không có Alt+C”).

## Hành vi gỡ

`morphix uninstall` gỡ phần tích hợp (như `uninstall-integration` hiện tại) rồi xoá binary và thư viện đi kèm trong `~/.local`. Mặc định giữ dữ liệu người dùng (`~/.local/share/morphix`: log, lịch sử, cài đặt) và nói rõ là đã giữ.

## Để lại cho người build

- Định dạng gói (tar.gz hay tar.zst), tên file, cấu trúc thư mục trong gói, miễn khớp quy ước `bundled_lib_dir`.
- Cách nhận diện từng trình quản lý file và vị trí file tích hợp cho Nemo (actions), Thunar (custom actions `uca.xml`), Caja (scripts/actions).
- Cách kiểm thử trên nhiều distro (container, VM), miễn có kiểm thật ít nhất Ubuntu 24.04, Fedora mới nhất, Arch, và ghi rõ môi trường nào chưa kiểm.
- Tách Morphix khỏi repo `linux-tools` sang repo mới, có giữ lịch sử git hay không.
