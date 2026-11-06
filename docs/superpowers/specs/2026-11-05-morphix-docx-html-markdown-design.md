# Morphix D3 — Word (DOCX) và HTML ↔ Markdown

Ngày: 2026-11-05 · Dự án con D3 (sau D1 ảnh→PDF, D2 Markdown văn bản/bảng/phụ đề/PDF).

## Mục tiêu

Người dùng muốn: "word -> MD và ngược lại html,…", rồi chọn thêm cả DOCX ↔ HTML và DOCX/HTML → TXT. Nguyên tắc đã chốt ở D2: từ MD sang định dạng khác thì không còn ký hiệu Markdown (thành định dạng thật), từ định dạng khác sang MD thì thêm ký hiệu hợp lý.

## Phạm vi

| Nguồn → đích | Cách làm |
|---|---|
| MD → HTML | `pulldown-cmark` (bật lại tính năng `html`), bọc trang HTML đầy đủ |
| HTML → MD | `html_to_markdown` (đọc DOM bằng `dom_query`) |
| MD → DOCX | `markdown_to_docx` (ghi WordprocessingML tối giản qua `zip`) |
| DOCX → MD | `docx_to_markdown` (đọc `word/document.xml` bằng `quick-xml`) |
| DOCX → HTML | DOCX → MD → HTML |
| HTML → DOCX | HTML → MD → DOCX |
| DOCX → TXT, HTML → TXT | → MD → `markdown_to_plain` (D2) |

Giữ được: tiêu đề 1–6, đoạn, đậm, nghiêng, gạch ngang, link, danh sách số và gạch đầu dòng (lồng nhau), bảng, code (khối và trong dòng), trích dẫn, ngắt dòng.

Không làm: ảnh nhúng, màu/font/căn lề, header/footer, chú thích, comment, track changes, DOC/ODT/RTF, TXT → DOCX/HTML (TXT vẫn chỉ sang MD như D2).

## Thiết kế

### Nhận dạng

- `TextFormat::Html` (đuôi `html`, `htm`) và `TextFormat::Docx` (đuôi `docx`).
- DOCX là zip nên `detect` phải xét trước nhánh file nén: zip có mục `word/document.xml` là `Text(Docx)`. WHY: không thì DOCX bị coi là file nén và chỉ được đề nghị "Giải nén".
- `text_targets`: `Markdown → [Txt, Markdown, Html, Docx]`; `Html → [Markdown, Docx, Txt]`; `Docx → [Markdown, Html, Txt]`. TXT giữ `[Txt, Markdown]`. Đích trùng nguồn bị catalog bỏ như hiện nay.

### Module `ops/html.rs`

- `markdown_to_html(markdown: &str, title: &str) -> String`: trang `<!doctype html>` có `<meta charset="utf-8">` và `<title>` (tên file, đã thoát), thân là HTML từ `pulldown-cmark` (bảng, gạch ngang bật).
- `html_to_markdown(html: &str) -> HtmlToMarkdown { markdown: String, images_dropped: usize }`: h1–h6 → `#`; p; strong/b → `**`; em/i → `*`; del/s → `~~`; a → `[chữ](href)`; ul/ol/li (lồng, giữ `start` của ol); table (hàng đầu hoặc `thead` làm tiêu đề, dùng `table_to_markdown` của D2); pre → khối code; code → `` ` ``; br → ngắt dòng cứng; blockquote → `>`. `script`, `style`, `head` bị bỏ; thẻ lạ lấy chữ bên trong. Chữ thường qua `escape_plain`. `img` giữ alt nếu có, đếm vào `images_dropped`.

### Module `ops/docx.rs`

- `docx_to_markdown(path: &Path) -> Result<DocxToMarkdown { markdown, images_dropped }>`: đọc `word/document.xml`, `word/_rels/document.xml.rels` (đích link), `word/numbering.xml` (số hay gạch đầu dòng), `word/styles.xml` (style id → tên `Heading N`, vì id có thể đã dịch). Đoạn có `w:numPr` là mục danh sách, cấp theo `w:ilvl`. Run có `w:b`/`w:i`/`w:strike` (không có `w:val="0"`/`false`) → ký hiệu tương ứng. `w:tbl` → bảng. `w:br` → ngắt dòng, `w:tab` → tab. `w:drawing`/`w:pict` → đếm ảnh bỏ.
- `markdown_to_docx(markdown: &str, out: &mut impl Write + Seek) -> Result<()>`: zip gồm `[Content_Types].xml`, `_rels/.rels`, `word/document.xml`, `word/_rels/document.xml.rels` (link ngoài), `word/styles.xml` (Normal, Heading1–6, Code, Quote, Hyperlink), `word/numbering.xml` (một định nghĩa số, một gạch đầu dòng, 9 cấp). Mọi chữ được thoát XML.

### Job, catalog, giao diện

- `write_converted` nối các cặp theo bảng trên; DOCX đọc theo đường dẫn, các định dạng khác đọc `read_utf8`.
- Ảnh bị bỏ → cảnh báo qua cơ chế `warnings` của job (đã có ở B/D1): "<tên file>: bỏ N ảnh (chỉ giữ chữ thay thế)".
- CLI `--to html`, `--to docx`; menu GUI qua `text:html`, `text:docx` (id đã theo mẫu `text:<đuôi>`); chuỗi UI tiếng Việt.

### Phụ thuộc

`pulldown-cmark` bật lại tính năng `html`; thêm `quick-xml` và `dom_query` vào `morphix-core` đúng phiên bản đã có trong `Cargo.lock` (0.42 và 0.28) để không tải thêm. Không gọi chương trình ngoài.

## Lỗi và an toàn

- DOCX hỏng, thiếu `word/document.xml` hoặc mã hoá (file OLE có mật khẩu) → `Corrupt` có lý do tiếng Việt và đường dẫn.
- HTML/MD không phải UTF-8 → lỗi như TXT hiện nay.
- Ghi qua file tạm rồi đổi tên; nguồn không bị đụng.
- Zip DOCX: chỉ đọc đúng các mục cần, giới hạn kích thước giải nén mỗi mục (chống zip bomb) theo cách `ops::archive` đang làm.

## Kiểm thử

- Test vòng: MD mẫu (đủ mọi khối trong phạm vi) → DOCX → MD và → HTML → MD ra lại đúng MD (so sau khi chuẩn hoá bằng `markdown_to_plain` và so cấu trúc tiêu đề/danh sách/bảng).
- DOCX dựng trong test bằng `zip` (style id đã dịch, `w:b w:val="0"`, hyperlink qua rels, danh sách lồng, bảng, ảnh).
- HTML: thẻ lạ, script/style, ol start, bảng có/không thead, ký tự đặc biệt cần thoát, ảnh.
- `detect`: DOCX không còn là file nén; zip thường vẫn là file nén.
- Catalog, CLI (`--to docx`, `--to html`, cặp không hỗ trợ báo lỗi), menu GUI, cảnh báo ảnh.

## Rủi ro đã biết

- DOCX ↔ HTML đi qua MD nên mất những gì MD không biểu diễn được (màu, căn lề, ô gộp).
- DOCX từ trình soạn thảo lạ có thể dùng cấu trúc ngoài phạm vi (textbox, field) → chữ trong đó có thể bị bỏ; sẽ lấy chữ thô của các khối không nhận ra thay vì bỏ hẳn.
- `dom_query` là phụ thuộc gián tiếp của Tauri; dùng trực tiếp ở core gắn phiên bản theo Tauri.
