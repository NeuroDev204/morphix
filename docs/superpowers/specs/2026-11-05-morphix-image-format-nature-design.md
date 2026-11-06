# Morphix: convert ảnh đúng bản chất định dạng

Status: thiết kế đã được người dùng duyệt trong hội thoại (2026-11-05); chờ duyệt bản viết này trước khi lập plan.

Đây là dự án con **B** trong lần thiết kế lại Morphix. Các dự án con còn lại có spec riêng:
A (menu Windows kiểu chuột phải, bỏ hình quạt), C (video/audio), D (PDF/tài liệu). Thứ tự: B → A → C → D.

## Mục tiêu

Khi convert ảnh, kết quả phải đúng tác dụng của định dạng đích. Ví dụ ảnh sang WebP thì dung lượng giảm mà chất lượng nhìn vẫn như cũ, thay vì luôn nén lossy q=85 như hiện nay.

## Vấn đề hiện tại

- `encode_image` (`crates/morphix-core/src/ops/image_io.rs`) luôn mã hoá WebP lossy với `Quality::webp()` (q=85 ở mức Balanced), bất kể nguồn.
- Nguồn lossless (PNG, BMP, TIFF, GIF) qua WebP lossy bị mất chất lượng không cần thiết.
- Nguồn JPEG nén lại bằng WebP lossy vừa mất thêm chất lượng, vừa có thể không nhỏ hơn gốc.
- Không có bước nào kiểm tra file ra có nhỏ hơn file gốc.
- `ItemReport.result` (`job.rs`) chỉ là `Result<PathBuf>`, không có chỗ cho cảnh báo.

## Quyết định của người dùng

- Chọn kiểu mã hoá **tự động theo nguồn** (không thêm lựa chọn lossless thủ công, không thử nhiều lần để chọn bản nhỏ nhất).
- Khi file ra WebP/AVIF/JPEG không nhỏ hơn file gốc: **vẫn giữ file ra và cảnh báo rõ**, không tự ý xoá. File nguồn không bao giờ bị ghi đè hay mất.

## Quy tắc mã hoá

| Nguồn | → WebP | → AVIF | → JPEG |
|---|---|---|---|
| PNG, BMP, TIFF, GIF (lossless) | WebP lossless | quality cao nhất của bộ mã hoá AVIF; không bảo đảm lossless thật | quality 95 |
| JPEG, WebP, AVIF (đã lossy) | lossy mức `High` | lossy mức `High` | giữ preset người dùng chọn |

- Đích WebP/AVIF luôn giữ kênh alpha nếu nguồn có.
- Đích JPEG vẫn dàn phẳng nền trong suốt lên nền trắng như hiện tại (`flatten_on_white`).
- Preset Nhỏ/Vừa/Đẹp vẫn hoạt động cho nguồn lossy. Với nguồn lossless sang WebP thì không áp dụng, vì lossless không có mức chất lượng.
- Phân loại "lossless" hay "đã lossy" dựa trên `ImgFormat` của nguồn, không đoán từ nội dung ảnh. WebP nguồn được coi là đã lossy (không phân biệt được WebP lossless từ metadata đơn giản; chọn phía an toàn là không nâng chất lượng giả).

## Thay đổi theo thành phần

- `ops/image_io.rs`: `encode_image` nhận thêm định dạng nguồn và chọn kiểu mã hoá theo bảng trên. `convert_image` truyền định dạng nguồn vào. `compress_image` và `resize_image` (`ops/compress.rs`) cũng gọi `encode_image` và phải giữ nguyên hành vi (nén lại ảnh WebP vẫn theo preset đã chọn), nên `encode_image` giữ nguyên và chỉ `convert_image` đi qua hàm mới `encode_converted`. `compress_image` đã tự từ chối kết quả không nhỏ hơn (`NotSmaller`, không tạo file); khác với convert vì mục đích của nó là giảm dung lượng.
- `quality.rs`: thêm phương thức cho mức `High` khi nguồn đã lossy; không đổi preset hiện có.
- `job.rs`: `ItemReport` có thêm `warnings: Vec<String>`. Mọi nơi dựng `ItemReport` bằng struct literal (lõi, test GUI) phải thêm `warnings: Vec::new()`; các nơi chỉ đọc thì không cần đổi. `run_item` đo kích thước file vào và file ra cho `ConvertImage` có đích WebP/AVIF/JPEG; nếu file ra ≥ file gốc thì thêm cảnh báo "file mới lớn hơn gốc X%".
- Hiển thị cảnh báo: CLI in ra stderr; GUI hiện trong khay. Thông điệp cảnh báo dựng một lần trong lõi (tiếng Việt, kèm tên file gốc); CLI và GUI in nguyên văn. GUI đưa cảnh báo đầu tiên vào dòng chi tiết của lịch sử (`history_entry_for_report`), giống cách lý do lỗi đang được đưa vào, nên không cần đổi `ui/src/strings.ts`. Không được nuốt cảnh báo.
- Đích PNG/TIFF/BMP/GIF không cảnh báo, vì to hơn là bình thường.

## Luồng dữ liệu

`run_item` gọi `convert_image` → nhận `PathBuf` → so sánh `metadata().len()` của file vào và file ra → nếu cần, thêm vào `ItemReport.warnings` → CLI/GUI hiển thị.

## Kiểm thử

- PNG → WebP: pixel giải mã ra giống hệt gốc (bằng chứng lossless).
- PNG có alpha → WebP: alpha được giữ.
- JPEG nén kỹ → WebP: có cảnh báo khi file ra lớn hơn gốc.
- PNG → PNG hoặc đích không thuộc nhóm nén: không có cảnh báo.
- Test hiện có ở `crates/morphix-core/tests/convert.rs` và `compress_resize.rs` vẫn pass.
- `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, và kiểm tra `cargo check --target x86_64-pc-windows-gnu` cho `morphix-cli` và `morphix-gui`.

## Ngoài phạm vi

- Video, audio, PDF, tài liệu (dự án C và D).
- Giao diện Windows (dự án A).
- Lựa chọn lossless thủ công trong menu.
- Thử nhiều cài đặt rồi chọn bản nhỏ nhất.

## Rủi ro đã biết

- Bộ mã hoá AVIF của crate `image` có thể không hỗ trợ lossless thật; spec chỉ hứa "quality cao nhất", không hứa lossless cho AVIF.
- WebP lossless trên ảnh lớn chậm hơn lossy. Chấp nhận, vì chạy ngầm trong job và có thể huỷ bằng `CancelToken`.
