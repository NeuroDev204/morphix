# Số đo dung lượng và thời gian (2026-11-11)

Máy đo: Ubuntu 26.04, rustc 1.99.1, build trên máy dev (không phải Ubuntu 22.04 của CI).

## Trước (không có `[profile.release]`)

| Thành phần | Byte |
|---|---|
| `target/release/morphix` (CLI) | 18159624 |
| `target/release/morphix-gui` | 32939824 (23296200 sau `strip`) |
| `libpdfium.so` | 7645184 |
| ffmpeg tĩnh (`bin/ffmpeg`, BtbN n8.1.3 gpl) | 172254856 |
| ffmpeg tĩnh, file tải về `.tar.xz` | 150360944 |
| `ui/dist` | 263968 |

Nhận xét: ffmpeg chiếm khoảng 80% dung lượng cài đặt. Phần code của Morphix (CLI + GUI) chỉ khoảng 40 MB chưa strip.

Tarball `morphix-x86_64-linux.tar.gz` (dựng bằng `build-tarball.sh` với `MORPHIX_ALLOW_GLIBC_FLOOR=99` vì máy dev là glibc 2.39; ngưỡng thật 2.35 do CI Ubuntu 22.04 kiểm): **87190753** byte (mặc định cargo: không LTO, strip debuginfo, codegen-units 16).

## Thời gian giao diện (spec Phần 2)

Chưa đo được tự động (cần cửa sổ thật). Cách đo tay cho người dùng, ghi vào bảng khi có:

| Phép đo | Trước | Sau cột mốc A |
|---|---|---|
| Khởi động nguội tới lúc cửa sổ **menu (vòng tròn)** hiện (`morphix-gui menu a.png`, app chưa chạy; đo bằng script, 3 lần, cửa sổ gọi `show()` nhưng nội dung chưa chắc đã vẽ xong) | 283, 243, 263 ms (TB 263) | 303, 222, 242 ms (TB 256): không đổi trong sai số. Task 2 (lệnh nền) và 3 (Cài đặt tạo khi cần) không nằm trên đường khởi động này; vòng tròn bị xoá ở Task 6 |
| Khởi động nguội tới lúc Khay hiện (`morphix tray`, app chưa chạy) | chưa đo (không có log mốc này) | |
| Bấm khay tới lúc Khay hiện (app đã chạy) | | |
| Thả 50 file ở thư mục chậm vào Khay: cửa sổ còn kéo/cuộn được | | |

## Sau (`[profile.release]`: `lto = "fat"`, `codegen-units = 1`, `strip = true`)

| Thành phần | Trước | Sau | Giảm |
|---|---|---|---|
| `morphix` (CLI) | 18159624 | 10376952 | 43% |
| `morphix-gui` | 32939824 | 17721280 | 46% |
| tarball `.tar.gz` | 87190753 | 83890749 | 3.8% |

Build release đầy đủ: 1 phút 47 giây (trước) → 2 phút 58 giây (sau) trên máy dev. Dưới ngưỡng 20 phút của plan nên giữ `lto = "fat"`.

## Ứng viên gỡ bớt (`cargo bloat --release -p morphix-gui --crates`, phần `.text`)

| Crate | % `.text` | Kích thước |
|---|---|---|
| std | 13.8% | 1.7 MiB |
| rav1e | 12.4% | 1.6 MiB |
| tauri | 10.5% | 1.3 MiB |
| tokio | 4.8% | 617 KiB |
| morphix_gui | 4.5% | 581 KiB |
| morphix_core | 4.1% | 528 KiB |
| image | 3.8% | 492 KiB |
| zbus | 3.2% | 417 KiB |
| pdfium_render | 2.9% | 374 KiB |
| image_webp + libwebp_sys | 1.9% + 1.9% | 267 + 245 KiB |

Quyết định: không gỡ gì. `rav1e` là bộ mã hoá AVIF (feature `avif` của `image`), gỡ là mất định dạng AVIF đầu ra. Hai đường WebP (`image_webp` và `libwebp_sys`) mỗi đường chưa tới 5% binary, không đáng đổi rủi ro. Ngưỡng của plan: chỉ gỡ khi chiếm hơn 5% binary và không mất tính năng.

## ffmpeg (đề xuất, chưa làm)

ffmpeg tĩnh là 172254856 byte, tức chiếm khoảng 80% dung lượng cài đặt, gấp hơn 7 lần `morphix-gui` đã strip. Đây là nguyên nhân chính của "bản build nặng", không phải code Morphix.

Hướng giảm: dựng ffmpeg riêng chỉ bật các codec mà `MediaFormat` cần (`crates/morphix-core/src/kind.rs`) với `--disable-everything` rồi bật từng encoder/decoder/muxer/demuxer cần thiết; thường còn khoảng 15–30 MB. Phải kiểm: (1) đủ codec cho mọi `MediaFormat` kể cả libx264, (2) vẫn giữ nghĩa vụ giấy phép GPL và `ffmpeg-source-offer.txt`, (3) CI tự dựng được và ghim SHA256. Ngưỡng của plan: chỉ đổi khi giảm ≥ 30% tổng gói; ước tính này vượt xa ngưỡng đó, nhưng đây là việc lớn, đề xuất làm thành plan riêng sau khi các cột mốc A–C xong.

## Ghi chú kiểm Windows

Từ Task 3, `cargo check --workspace --all-targets --target x86_64-pc-windows-gnu` (cần `gcc-mingw-w64-x86-64`) dùng làm cổng biên dịch cho nhánh Windows trên máy Linux. Nó chỉ chứng minh code biên dịch được; chạy thật trên Windows chỉ kiểm được bằng job CI ở Task 13 và kiểm tay.

## Kết quả cuối của kế hoạch (2026-11-11)

| Hạng mục | Trước | Sau |
|---|---|---|
| `morphix` (CLI), byte | 18159624 | 10461656 (-42%) |
| `morphix-gui`, byte | 32939824 | 19132832 (-42%; đã gồm plugin thông báo, +1.4 MB so với Task 1) |
| tarball Linux, byte | 87190753 | 84555473 (-3.0%) |
| ffmpeg tĩnh Linux, byte | 172254856 | 172254856 (chưa đổi; xem đề xuất ở trên) |
| ffmpeg.exe Windows, byte | Gyan "full" 9.0.2 | Gyan "essentials" 9.0.2: 105423872 (zip 114768076 thay vì 257736868) |
| `menu --list` với 20 PNG | n/a | trung vị 1,7 ms |
| khởi động nguội tới cửa sổ menu (vòng tròn) | 263 ms | cửa sổ đã xoá; menu chạy thẳng, không mở cửa sổ |

Số "Sau" đo lại ở cuối kế hoạch, sau khi thêm `tauri-plugin-notification`. Tarball chỉ giảm 3.0% vì ffmpeg tĩnh (172 MB) chiếm gần hết; muốn nhẹ thật phải dựng ffmpeg riêng (đề xuất ở trên), việc này chưa làm.

## Danh sách kiểm tay cho người dùng

Tôi không điều khiển được menu chuột phải hay cửa sổ thật trên các máy này; các mục dưới đây chưa ai kiểm. Đánh dấu khi đã kiểm.

### Đã kiểm (bởi tôi, bằng công cụ)
- [x] Gửi thông báo hệ thống khi việc từ menu xong: một lệnh `org.freedesktop.Notifications.Notify` (GNOME Wayland, Ubuntu 26.04), lịch sử có đúng 1 mục.
- [x] `morphix menu --action convert:webp a.png` tự khởi động GUI và tạo `a.webp`; `morphix menu a.png` mở Khay.
- [x] Extension Nautilus nạp được với thư viện `gi` và typelib Nautilus thật, dựng `Nautilus.Menu`/`MenuItem` đúng.
- [x] `cargo check --workspace --all-targets --target x86_64-pc-windows-gnu` sạch (biên dịch được cho Windows, chưa chạy).

### Linux (chưa kiểm)
- [ ] Nautilus có `python3-nautilus`: submenu Morphix với 1 PNG; 3 PNG; PNG + TXT (chỉ còn nén); 101 file (mục "Chọn quá nhiều file"); đổi lựa chọn nhanh liên tục không làm Files khựng.
- [ ] Nautilus không có `nautilus-python`: Scripts → Morphix việc nhanh → Ảnh → Đổi sang WebP chạy được.
- [ ] Dolphin (KDE 5 và 6): các service menu cùng `X-KDE-Submenu=Morphix` gộp thành một mục Morphix; file có bit thực thi.
- [ ] Thunar: submenu Morphix, lọc theo loại và đuôi file (kể cả chữ hoa).
- [ ] Nemo hoặc Caja: Scripts → Morphix việc nhanh.
- [ ] Thông báo hiện khi xong trên KDE, XFCE (ngoài GNOME).
- [ ] `install.sh` rồi `morphix doctor` sạch trên Fedora, openSUSE, Debian, Arch, Ubuntu và biến thể; cập nhật bảng "Đã kiểm trên" trong README chỉ với dòng đã kiểm.
- [ ] Cập nhật từ bản cũ: các mục menu một mục cũ biến mất (Dolphin `morphix.desktop`, Nemo Action).

### Windows (chưa kiểm)
- [ ] Đẩy nhánh lên GitHub và xem job `test-windows` (clippy và cargo test) xanh; mọi lỗi test chỉ xuất hiện khi chạy thật trên Windows.
- [ ] Job `windows` của workflow phát hành chạy được: các script `.ps1`, `ISCC` biên dịch `morphix.iss`, `makeappx` đóng gói msix, kiểm chữ ký WebView2.
- [ ] `MorphixSetup.exe` trên Windows 10 và 11 sạch: không cần quyền quản trị, tự cài WebView2, SmartScreen hiện đúng như README mô tả.
- [ ] Chuột phải file: mục Morphix và submenu chạy đúng việc; thông báo "xong" hiện (cần AppUserModelID `vn.morphix.app` trên lối tắt Start Menu), cả bản exe và bản Store.
- [ ] Nút "mở thư mục kết quả" trong Khay mở Explorer.
- [ ] Gỡ cài đặt xoá mục menu; `morphix_shell.dll` bị Explorer giữ thì được xoá sau khi khởi động lại.
- [ ] Gói msix được Partner Center chấp nhận; nộp lại với mô tả mới trong `installer/store-package/listing/description.md`.

### Cả hai nền tảng (chưa kiểm)
- [ ] Bản release (custom-protocol): mở Khay và Cài đặt, giao diện vẽ đúng và hoạt động (CSP đã đặt, chỉ kiểm tĩnh trên `dist`).
- [ ] Đo lại thời gian Khay hiện (bấm khay, thả 50 file ở thư mục chậm) và dung lượng bản build cuối.

### Bổ sung sau review cuối (Windows, chưa kiểm)
- [ ] Nâng cấp hoặc gỡ `MorphixSetup.exe` trong lúc Explorer đang giữ `morphix_shell.dll`: không hiện hộp thoại "file đang dùng", không bị đòi đóng Explorer, DLL được thay hoặc xoá (cờ `restartreplace` cần quyền admin nên có thể không có tác dụng ở chế độ người dùng).
- [ ] Nâng cấp từ bản cài cũ chế độ admin (0.1.0): chứng chỉ tự ký trong kho Root và gói sparse còn sót cần gỡ tay.

## Kiểm bằng Docker (2026-11-11)

- **Linux:** Fedora 45, Debian 13, Arch, openSUSE Tumbleweed: `install.sh` → `install-integration` → `doctor` → `desktop-file-validate` → chuyển đổi thật → gỡ. Đều qua. Debian và Fedora có `nautilus-python`: extension `installed`, cây Scripts `not_applicable` (menu động). Container không có giao diện nên chưa thấy menu hiện ra.
- **Windows qua Wine 10 trong Docker** (biên dịch chéo `x86_64-pc-windows-gnu`): `morphix.exe` cài và gỡ registry HKCU đúng thiết kế (CLSID `{2131F98F-…}` → `morphix_shell.dll`, khoá Run), `doctor`, `menu --list`, chuyển PNG → WebP với `pdfium.dll` thật. Toàn bộ 34 binary test (20 của workspace + 14 của GUI) xanh. Wine không phải Windows thật: chưa chạy GUI/WebView2, Explorer, `MorphixSetup.exe`, Store.
- Lượt chạy này lộ ra các test chỉ lỗi trên Windows (tên dir có `"`, test Linux không gắn `cfg`, URI kiểu Unix); đã sửa để job `test-windows` không đỏ vì chúng.
- Đã sửa: Thunar để lại `uca.xml` rỗng làm thư mục cha không dọn được khi gỡ.

## Đã sửa sau review cuối và các mục còn mở

Đã sửa (đều có test): danh sách lỗi của extension Nautilus hết hạn sau 2 giây; Thunar nhân đôi `%` trong đường dẫn cài; quét cây Scripts không đi theo symlink; thư mục *Morphix việc nhanh* có file của người dùng thì báo xung đột thay vì ghi chen vào; gỡ Alt+C dọn `~/.config/nautilus` rỗng; nhãn menu có động từ ("Đổi sang WebP", "Gộp thành PDF") cho Windows, Nautilus, Dolphin, Thunar, Scripts; mọi action trong workflow ghim theo SHA, token mặc định chỉ đọc; test Windows tìm được ffmpeg; job phát hành Windows kiểm UI nhúng; Inno Setup dự phòng ghim phiên bản; test của hàm chạy nền kiểm hành vi thật.

Chưa sửa, và lý do:
- **`restartreplace` của `morphix_shell.dll` cần quyền admin** trong khi installer chạy ở chế độ người dùng. Cách sửa đúng (đặt DLL theo tên có phiên bản) đổi đường dẫn đăng ký menu và chỉ kiểm được trên Windows thật.
- **`taskkill /F` khi gỡ** có thể dừng một lượt chuyển đổi đang chạy. GUI ẩn cửa sổ khi đóng nên không thoát êm được; cần một lệnh "thoát" qua IPC.
- **`all/allfiles`** trong service menu Dolphin: validator báo "sẽ lỗi trong tương lai" nhưng đây là kiểu MIME KDE dùng cho mọi file; đổi có thể làm Dolphin không hiện menu.
- **Phím tắt Alt+C trên Wayland, Dolphin/Thunar/Nemo/Caja, Explorer, `MorphixSetup.exe`, Store**: chưa kiểm được bằng công cụ ở đây.
