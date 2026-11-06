# Morphix D2 — Chuyển đổi Markdown theo bản chất định dạng

Ngày: 2026-11-05 · Dự án con D2 (D1 = ảnh→PDF, D3 = DOCX/HTML ↔ MD).

## Mục tiêu

Người dùng nói: từ Markdown sang mọi định dạng đều phải bỏ ký hiệu Markdown; từ các định dạng khác sang MD đều thêm ký hiệu cho hợp lý. D2 làm phần văn bản, phụ đề, bảng và PDF. DOCX/HTML để D3.

## Hiện trạng

- `TextFormat::Markdown` đã có. `text_targets` cho `Txt|Md → [Txt, Md]`, `write_converted` (`job.rs`) chép nguyên văn nên MD→TXT còn nguyên `#`, `**`.
- Phụ đề chỉ ra SRT/VTT/TXT, bảng chỉ ra CSV/TSV. PDF chỉ ra TXT (`Operation::PdfToText`, `pdf_text.rs`).

## Phạm vi

| Nguồn → đích | Hành vi |
|---|---|
| MD → TXT | Bỏ ký hiệu Markdown, giữ chữ |
| TXT → MD | Thoát ký tự đặc biệt để chữ không bị hiểu thành định dạng |
| CSV/TSV → MD | Bảng `\| a \| b \|`, hàng đầu làm tiêu đề |
| SRT/VTT → MD | Danh sách: `- **HH:MM:SS** nội dung` |
| PDF → MD | Mỗi trang một mục `## Trang N` kèm chữ đã thoát, không đoán cấu trúc |
| MD → MD, TXT → TXT | Giữ như cũ (chép nguyên văn) |

Không làm: MD→CSV/SRT (vô nghĩa), đoán tiêu đề/danh sách từ PDF, DOCX/HTML (D3).

## Thiết kế

### Module `ops/markdown.rs` (hàm thuần, chuỗi vào/ra, không đụng file)

- `markdown_to_plain(&str) -> String` dùng `pulldown-cmark` (phụ thuộc mới) duyệt sự kiện: giữ chữ; link giữ chữ bỏ URL; ảnh giữ alt; code block giữ nội dung bỏ rào; danh sách ra `- ` hoặc `1. `; bảng ra các ô cách nhau bằng tab, mỗi hàng một dòng; tiêu đề thành dòng thường; các khối cách nhau một dòng trống.
- `escape_plain(&str) -> String` thoát `\ * _ # > [ ] ` | ~` và `- `, `+ `, `1. ` đầu dòng. WHY: TXT→MD không được làm đổi nghĩa chữ. Một xuống dòng đơn thành dòng trống để không bị gộp đoạn.
- `table_to_markdown(&[Vec<String>]) -> String`: hàng đầu làm tiêu đề, thoát `|`, ô nhiều dòng đổi `<br>`, hàng ngắn độn ô trống.
- `cues_to_markdown(&[Cue]) -> String`: nội dung nhiều dòng nối `<br>`, thoát ký tự đặc biệt.
- `pages_to_markdown(&[String]) -> String`: trang rỗng vẫn có tiêu đề.

### Catalog và job

- `text_targets`: `Txt|Md → [Txt, Md]`; `Srt|Vtt → [Srt, Vtt, Txt, Md]`; `Csv|Tsv → [Csv, Tsv, Md]`. Các nhóm vẫn khác danh sách nên không lẫn nhau.
- `write_converted` thêm nhánh theo đích. Thông báo lỗi cặp không hỗ trợ giữ nguyên.
- `Operation::PdfToMarkdown` và `ActionKind::PdfToMarkdown` (nhãn "MD"), chỉ hiện khi chọn đúng một PDF, giống PDF→TXT. `pdf_text.rs` tách hàm lấy chữ từng trang dùng chung.
- Cập nhật CLI (`main.rs`), `menu_items.rs` của GUI và chuỗi UI.

### Lỗi và an toàn

Ghi qua file tạm rồi đổi tên (`write_output`), không để lại file cụt, không ghi đè nguồn. Lỗi đọc UTF-8 giữ nguyên thông báo hiện có. TSV→MD không từ chối ô chứa tab vì thoát được.

## Kiểm thử (TDD)

- Test đơn vị cho từng hàm trong `markdown.rs` (tiêu đề, nhấn mạnh, link, ảnh, code, danh sách, bảng, ký tự đặc biệt).
- Test vòng: TXT có đủ ký tự đặc biệt → MD → TXT ra đúng chữ gốc.
- Test catalog (`text_targets`, `text_pair_supported`), job (`tests/text.rs`, `tests/job.rs`), CLI, `menu_items`.
- PDF→MD cần PDFium nên thuộc nhóm test đã fail sẵn trên máy này; hàm gộp trang được test độc lập bằng chuỗi.

## Rủi ro đã biết

- Bỏ ký hiệu MD dùng CommonMark chuẩn; cú pháp mở rộng (footnote, math) có thể còn sót.
- Phụ đề đổi sang MD: mốc kết thúc không được ghi, chỉ giữ mốc bắt đầu.
