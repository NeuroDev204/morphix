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
| Khởi động nguội tới lúc cửa sổ **menu (vòng tròn)** hiện (`morphix-gui menu a.png`, app chưa chạy; đo bằng script, 3 lần, cửa sổ gọi `show()` nhưng nội dung chưa chắc đã vẽ xong) | 283, 243, 263 ms | (vòng tròn bị xoá ở Task 6; đo lại bằng Khay) |
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
