# Morphix D4: chuyển tài liệu thẳng sang định dạng đích (mô hình tài liệu chung)

Ngày: 2026-11-06. Nối tiếp D3 (`2026-11-05-morphix-docx-html-markdown-design.md`); thay phần "DOCX ↔ HTML qua MD" của D3.

## Mục tiêu

Người dùng nói: "không phải chuyển hoàn toàn về định dạng đích chứ không vướng md". Ở D3, DOCX ↔ HTML và DOCX/HTML → TXT đi vòng qua Markdown, nên mất những gì Markdown không chứa được. Phạm vi đã chốt là **cấu trúc + ảnh**:

- gạch chân, chỉ số trên/dưới;
- ô gộp (colspan/rowspan), ô có nhiều đoạn hoặc danh sách;
- ảnh nhúng;
- chú thích cuối trang;
- link trong TXT giữ cả URL.

Không làm: màu, font, cỡ chữ, căn lề, đầu/chân trang, comment, track changes, tải ảnh từ mạng.

## Cách làm đã chọn

Dùng một mô hình tài liệu chung (`ops/doc.rs`). Mỗi định dạng có một bộ đọc ra `Document` và một bộ ghi từ `Document`; Markdown chỉ là một đích như mọi đích khác. Đã cân nhắc và bỏ hai cách: viết bộ chuyển riêng cho từng cặp (chép gần y hệt 4 bộ duyệt), và nhồi HTML thô vào MD (file MD xuất ra lẫn thẻ, đúng kiểu "vướng" người dùng không muốn).

## Mô hình

```rust
pub struct Document {
    pub blocks: Vec<Block>,
    pub footnotes: Vec<Vec<Block>>,   // chú thích, chỉ số = thứ tự xuất hiện
    pub images: Vec<Image>,           // Image { bytes, mime }
    pub dropped: Dropped,             // đếm thứ bị bỏ để cảnh báo
}
pub enum Block {
    Heading(u8, Vec<Inline>),
    Paragraph(Vec<Inline>),
    List { ordered: bool, items: Vec<Vec<Block>> },
    Table(Vec<Vec<Cell>>),            // Cell { blocks, colspan, rowspan, header }
    Code(String),
    Quote(Vec<Block>),
    Rule,
}
pub enum Inline {
    Text(String, Style),              // Style: bold, italic, underline, strike, sup, sub, code
    Link(String, Vec<Inline>),
    Image { index: Option<usize>, alt: String }, // None = không lấy được ảnh, chỉ giữ chữ thay thế
    FootnoteRef(usize),
    Break,
}
pub struct Dropped { images: usize, notes: usize, headers: bool, md_downgraded: bool }
```

Trong `Table`, ô bị ô khác gộp đè lên không có mặt; lưới được dựng lại từ colspan/rowspan khi ghi.

## Luồng

| Cặp | Cách làm |
|---|---|
| MD → HTML | giữ nguyên: `push_html` của pulldown-cmark |
| MD → TXT, TXT → MD | giữ nguyên: `markdown_to_plain`, `escape_plain` (D2) |
| MD → DOCX | `markdown::read` → `docx::write` |
| HTML/DOCX → MD | `html::read` / `docx::read` → `markdown::write` |
| DOCX → HTML, HTML → DOCX | đọc → `Document` → ghi thẳng ra đích |
| DOCX/HTML → TXT | đọc → `Document` → `text::write_plain` |

`job.rs::write_document` chỉ còn đọc thành cây, ghi ra đích, rồi biến `dropped` thành cảnh báo. Các hàm công khai cũ (`docx_to_markdown`, `html_to_markdown`, `markdown_to_docx`) được thay bằng các bộ đọc/ghi mới; mọi chỗ gọi đều nằm trong `job.rs` và các test.

## Bộ đọc

- **DOCX:** từ bộ đọc D3, nay trả về `Document`. Thêm:
  - `w:u` (khác `none`) → underline; `w:vertAlign` superscript/subscript → sup/sub; `w:dstrike` → strike;
  - `w:gridSpan` → colspan; `w:vMerge` restart/continue → rowspan (ô continue gộp vào ô trên);
  - ô chứa được nhiều khối;
  - `w:drawing` → `a:blip r:embed` → rels → bytes trong `word/media/*`, giới hạn `MAX_PART_BYTES`;
  - `w:footnoteReference` / `w:endnoteReference` → `FootnoteRef`, nội dung lấy từ `footnotes.xml` / `endnotes.xml`, bỏ các mục phân cách (`w:type` separator/continuationSeparator).
  - Comment và đầu/chân trang vẫn bị bỏ và đếm như D3.
- **HTML:** từ bộ duyệt D3 (giữ `MAX_DEPTH`), nay trả về `Document`. Thêm:
  - `<u>`, `<ins>` → underline; `<sup>`, `<sub>`; `<s>`, `<del>`, `<strike>` → strike;
  - thuộc tính `colspan`/`rowspan`; khối trong ô;
  - `<img>`:
    - `data:` base64: giải mã;
    - đường dẫn tương đối: đọc file thường cạnh trang HTML (`read_html(path)` biết thư mục gốc), tối đa `MAX_PART_BYTES`;
    - `http(s):`, `file:` tuyệt đối, đường dẫn tuyệt đối, đường dẫn ra ngoài thư mục bằng `..`: không đọc, chỉ giữ chữ thay thế và đếm. WHY: một trang HTML lạ không được kéo file tuỳ ý trên máy vào DOCX.
  - Không đoán chú thích trong HTML.
- **Markdown:** chuỗi sự kiện của pulldown-cmark → `Document`. Footnote `[^1]` → `FootnoteRef`; HTML thô → chữ như D3. Ảnh `![alt](src)` theo cùng quy tắc với `<img>` của HTML (`data:` và đường dẫn tương đối cạnh file .md), để MD → DOCX nhúng được ảnh.

Kiểu ảnh giữ được: PNG, JPEG, GIF, BMP, WebP (nhận bằng `infer` theo bytes). Loại khác, cũng như ảnh hỏng, bị bỏ và đếm.

## Bộ ghi

- **HTML:** HTML5 có `meta charset` và `title`, kèm một đoạn CSS nhỏ kẻ viền bảng.
  - `<u> <sup> <sub> <s> <code>`, colspan/rowspan.
  - `<img src="data:…" alt="…">`, để kết quả vẫn là một file duy nhất.
  - Chú thích: `<sup><a href="#fn1" id="ref1">1</a></sup>` trong chữ, `<section class="footnotes"><ol>` ở cuối.
  - Mọi chữ và thuộc tính đều thoát ký tự HTML.
- **DOCX:** từ bộ ghi D3 (styles, numbering, `xml_escape`, `word_level`). Thêm:
  - `w:u`, `w:vertAlign`;
  - `w:gridSpan`; `w:vMerge` có chèn ô continue theo lưới;
  - đoạn tiếp nối trong mục danh sách: thụt như mục, không đánh số;
  - ảnh vào `word/media/imageN.<đuôi>` có rel và content type, kích thước lấy bằng `image::ImageReader::into_dimensions`, thu nhỏ theo tỉ lệ để rộng tối đa 6 inch;
  - `word/footnotes.xml` thật, có hai mục phân cách id −1/0 và rel, cùng `w:footnoteReference`.
- **TXT** (`text::write_plain`):
  - tiêu đề thành một dòng;
  - danh sách `- ` / `1. `, mỗi cấp thụt 2 khoảng trắng;
  - bảng: các ô cách nhau bằng tab, ô gộp đệm ô trống, nhiều đoạn trong ô nối bằng dấu cách;
  - link `chữ (url)`, trừ khi chữ trùng URL;
  - ảnh `[ảnh: chữ thay thế]`;
  - chú thích `[1]` trong chữ và danh sách `[1] …` ở cuối;
  - code giữ nguyên văn.
- **Markdown** (`markdown::write`): chuyển phần thoát ký tự hiện đang nằm trong docx.rs/html.rs về đây (`escape_specials`, `wrap_marks`, `code_span`, `code_fence`, `link_destination`, …). Đầu ra cho nội dung D3 giữ đúng như cũ. Thứ MD không chứa được thì hạ cấp và đặt `md_downgraded`:
  - gạch chân/trên/dưới → chữ thường;
  - ô gộp → ô trống;
  - khối trong ô → nối bằng `<br>`;
  - ảnh nhúng → chỉ giữ chữ thay thế như D3, và được tính vào cảnh báo ảnh.

## Lỗi và cảnh báo

- Lỗi đọc vẫn là `MorphixError::Corrupt`/`Io` mang đường dẫn file gốc. Mật khẩu, OLE, part quá lớn, thiếu `document.xml` báo y như D3.
- Ghi qua `write_output` (file tạm rồi đổi tên): file gốc không bao giờ bị đụng, và không có file cụt.
- Ảnh hỏng hoặc thiếu không làm hỏng cả việc, chỉ bị bỏ và đếm.
- Cảnh báo, tối đa 2 dòng:
  - `"{tên}: bỏ N ảnh (chỉ giữ chữ thay thế)"`;
  - `"{tên}: bỏ … (chỉ giữ nội dung chính)"` gồm comment, đầu/chân trang và, khi ra MD, "định dạng MD không có (gạch chân, trên/dưới, ô gộp)".

## Kiểm thử

- Test hiện có trong docx.rs, html.rs, markdown.rs, tests/text.rs và tests/job.rs giữ nguyên đầu ra MD và là lưới an toàn cho việc viết lại. Test nào phải đổi kỳ vọng thì ghi lý do vào ledger.
- Test mới, viết đỏ trước rồi mới xanh:
  - DOCX → HTML giữ `<u>`, `<sup>`, colspan/rowspan, ô nhiều đoạn, ảnh data URI, chú thích;
  - HTML → DOCX giữ gạch chân, gridSpan/vMerge, ảnh trong `word/media` có rel và content type, `footnotes.xml`;
  - DOCX/HTML → TXT: link `chữ (url)`, `[ảnh: …]`, chú thích `[1]`, bảng có ô gộp thẳng cột;
  - HTML có ảnh `http:`, `file:` tuyệt đối, `../`, ảnh hỏng: không đọc, có cảnh báo;
  - ra MD từ nguồn có gạch chân hoặc ô gộp: có cảnh báo hạ cấp.
- Kiểm tay: Word 16 mở các DOCX sinh ra qua COM ở chế độ chỉ đọc, dùng file mẫu có ảnh, bảng gộp và chú thích.
- Mốc chung của bộ test: 39 lỗi sẵn có (Linux/PDFium) không đổi; clippy `-D warnings` sạch.

## Thứ tự làm

1. `doc.rs` và `markdown::write`, kèm `markdown::read` để MD → DOCX dùng cây.
2. Bộ đọc HTML/DOCX trả về `Document`; đầu ra MD giữ nguyên.
3. Bộ ghi HTML và TXT.
4. Bộ ghi DOCX từ `Document`.
5. Ảnh (đọc và ghi, cả ba định dạng).
6. Chú thích.
7. Kiểu chữ mới (gạch chân, trên/dưới) và ô gộp.

## Phụ thuộc

`base64 = "0.22.1"` thêm thành dependency trực tiếp của morphix-core. Crate này đã có trong Cargo.lock qua phụ thuộc gián tiếp nên không phải tải gì mới. Không thêm phụ thuộc nào khác.
