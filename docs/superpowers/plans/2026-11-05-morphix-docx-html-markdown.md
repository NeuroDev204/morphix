# Morphix D3 — DOCX và HTML ↔ Markdown Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Đổi qua lại MD, HTML, DOCX (giữ định dạng thật) và lấy TXT từ HTML/DOCX, không gọi chương trình ngoài.

**Architecture:** Markdown là định dạng trung gian. Hai bộ đọc (`html_to_markdown`, `docx_to_markdown`) và hai bộ ghi (`markdown_to_html`, `markdown_to_docx`); mọi cặp còn lại ghép từ chúng và từ `markdown_to_plain` của D2. `job.rs::write_converted` chọn đường đi theo `(from, to)`.

**Tech Stack:** Rust 2021; `pulldown-cmark` 0.13 bật lại feature `html`; `dom_query` 0.28.0 và `quick-xml` 0.42.0 (đã có trong `Cargo.lock` qua Tauri, thêm bằng `cargo add`, không sửa tay lock); `zip` (đã có).

**Spec:** `docs/superpowers/specs/2026-11-05-morphix-docx-html-markdown-design.md`

## Global Constraints

- Không bao giờ ghi đè/mất file nguồn; ghi qua `write_output` (file tạm rồi đổi tên).
- Không mất chữ âm thầm: khối không nhận ra thì lấy chữ thô; ảnh bị bỏ thì giữ alt và cảnh báo.
- Lỗi hiện cho người dùng, lý do tiếng Việt; comment `WHY` tiếng Việt.
- Offline; phụ thuộc mới chỉ `dom_query = "0.28.0"`, `quick-xml = "0.42.0"`, feature `html` của `pulldown-cmark`. Nếu `cargo add` kéo phiên bản khác bản trong lock, ghim đúng bản trong lock.
- Không commit được (máy chưa có git identity): bước commit ghi "commit bỏ qua" vào ledger.
- Chỉ rustfmt file đã chạm; `cargo clippy --workspace --all-targets -- -D warnings` sạch; `cargo test --workspace --no-fail-fast` không thêm fail ngoài baseline 39 (36 cũ + 3 PDFium của D2).
- Dạng MD chuẩn mà hai bộ đọc phải sinh ra (để test vòng so bằng chuỗi): tiêu đề `# `…`###### `; `**đậm**`, `*nghiêng*`, `~~gạch~~`, `` `code` ``; `[chữ](url)`; gạch đầu dòng `- `, số `N. `, lồng thụt 2 khoảng trắng mỗi cấp; bảng như `table_to_markdown`; khối code bằng rào ```` ``` ```` không tên ngôn ngữ; trích dẫn `> `; ngắt dòng cứng là `\` cuối dòng; các khối cách nhau một dòng trống; kết thúc bằng đúng một `\n`.

## Review Focus

- DOCX do Word thật tạo: style id đã dịch (`Kop1`, `Titre1`) và `w:b w:val="0"` — phải đúng tiêu đề và không đậm sai (Task 3).
- HTML có chữ ngay trong `<body>`/`<div>` không có `<p>`, khoảng trắng thừa giữa thẻ, entity `&nbsp;` `&amp;` — không mất chữ, không sinh dòng rỗng lung tung (Task 2).
- Chữ chứa ký tự Markdown (`*`, `_`, `#`, `|`) trong DOCX/HTML → MD phải được thoát (Task 2, 3).
- DOCX hỏng / zip thường đổi đuôi `.docx` / file DOCX có mật khẩu (OLE) → `Corrupt` hoặc không nhận là DOCX, không panic (Task 1, 3).
- Zip thường (không phải DOCX) vẫn là file nén và vẫn "Giải nén" được (Task 1).

---

### Task 1: Định dạng `Html`, `Docx` và nhận dạng

**Files:**
- Modify: `crates/morphix-core/src/kind.rs` (`TextFormat` thêm `Html`, `Docx`; `extension` → `"html"`, `"docx"`; `from_extension` nhận `html`, `htm`, `docx`; `sniff_binary` thêm nhánh MIME `"application/vnd.openxmlformats-officedocument.wordprocessingml.document" => FileKind::Text(TextFormat::Docx)` trước nhánh zip, kèm WHY), `crates/morphix-core/src/catalog.rs` (`text_targets`: tạm `Html | Docx => &[]`), `crates/morphix-core/src/job.rs` (`write_converted`: tạm nhánh `(Html | Docx, _)` trả `Unsupported`, sẽ thay ở Task 5)
- Test: unit test trong `kind.rs`; `crates/morphix-core/tests/` file đang test `detect` (tìm bằng `grep -l "detect(" crates/morphix-core/tests`)

**Interfaces:**
- Produces: `TextFormat::Html`, `TextFormat::Docx`; `detect` trả `FileKind::Text(TextFormat::Docx)` cho DOCX thật.

- [ ] **Step 1: Viết test thất bại:**
  - `html_and_docx_extensions_round_trip`: `from_extension("HTM") == Some(Html)`, `from_extension("docx") == Some(Docx)`, `Html.extension() == "html"`.
  - `docx_is_detected_as_word_not_archive`: dựng bằng `zip` một file `a.docx` có `[Content_Types].xml` (mục đầu) và `word/document.xml` → `detect == Text(Docx)`; cùng nội dung đặt tên `a.zip` vẫn `Text(Docx)` (theo nội dung).
  - `plain_zip_is_still_an_archive`: zip chỉ có `x.txt` đặt tên `x.docx` → `Archive(Zip)`.
  - `utf8_html_file_is_text_html`: `a.html` chứa `<p>chào</p>` → `Text(Html)`.
- [ ] **Step 2: Chạy** `cargo test -p morphix-core detect` và test kind → FAIL (biến thể chưa có).
- [ ] **Step 3: Cài đặt** như phần Files. Nếu `infer` không nhận DOCX khi `[Content_Types].xml` đứng đầu, thêm ở nhánh zip: mở bằng `zip::ZipArchive` và có mục `word/document.xml` → `Text(Docx)` (ledger ruling). Sửa mọi `match` đầy đủ trên `TextFormat` mà trình biên dịch báo (CLI `Target` để Task 5).
- [ ] **Step 4: Chạy** lại → PASS; `cargo test -p morphix-core --no-fail-fast` không thêm fail.
- [ ] **Step 5: Commit** `feat(morphix-core): recognise HTML and Word documents`.

---

### Task 2: `ops/html.rs`

**Files:**
- Create: `crates/morphix-core/src/ops/html.rs` (unit test trong file)
- Modify: `crates/morphix-core/src/ops/mod.rs`, `crates/morphix-core/Cargo.toml` (`cargo add -p morphix-core dom_query@0.28.0`; thêm `features = ["html"]` cho `pulldown-cmark`)

**Interfaces:**
- Consumes: `escape_plain`, `table_to_markdown` (D2, `ops/markdown.rs`).
- Produces: `pub fn markdown_to_html(markdown: &str, title: &str) -> String`; `pub struct HtmlToMarkdown { pub markdown: String, pub images_dropped: usize }`; `pub fn html_to_markdown(html: &str) -> HtmlToMarkdown`; `#[cfg(test)] pub(crate) const SAMPLE: &str` trong `ops/markdown.rs` (dùng chung với Task 4).

- [ ] **Step 1: Viết test thất bại:**
  - `html_page_has_charset_title_and_formatting`: `markdown_to_html("# A\n\n**b**", "t<x>")` chứa `<meta charset="utf-8">`, `<title>t&lt;x&gt;</title>`, `<h1>A</h1>`, `<strong>b</strong>`.
  - `markdown_round_trips_through_html`: `SAMPLE` đúng dạng MD chuẩn ở Global Constraints, gồm tiêu đề 1 và 2, đoạn có đậm/nghiêng/gạch/link/code, danh sách gạch lồng 2 cấp, danh sách số, bảng 2×2, khối code, trích dẫn, ngắt dòng cứng; `html_to_markdown(&markdown_to_html(SAMPLE, "x")).markdown == SAMPLE`.
  - `scripts_styles_and_head_are_dropped`: `<head><title>T</title></head><script>x()</script><style>p{}</style><p>giữ</p>` → `"giữ\n"`.
  - `unknown_tags_keep_their_text`: `<div>a <span>b</span></div><section>c</section>` → chứa `a b` và `c`.
  - `text_with_markdown_characters_is_escaped`: `<p>a*b_c #1</p>` → `"a\\*b\\_c \\#1\n"`.
  - `entities_and_whitespace_collapse`: `<p>  x&nbsp;&amp;\n  y </p>` → `"x\u{a0}\\& y\n"`.
  - `ordered_list_keeps_start`: `<ol start="3"><li>a</li><li>b</li></ol>` → `"3. a\n4. b\n"`.
  - `table_without_thead_uses_first_row_as_header`: `<table><tr><td>a</td><td>b</td></tr><tr><td>1</td><td>2</td></tr></table>` → bảng như `table_to_markdown`.
  - `images_keep_alt_and_are_counted`: `<p><img src="a.png" alt="logo"> x</p><img src="b.png">` → markdown chứa `logo`, `images_dropped == 2`.
- [ ] **Step 2: Chạy** `cargo test -p morphix-core --lib html` → FAIL.
- [ ] **Step 3: Cài đặt:**
  - `markdown_to_html`: `pulldown_cmark::html::push_html` với `ENABLE_TABLES | ENABLE_STRIKETHROUGH | ENABLE_FOOTNOTES`; bọc `<!doctype html>\n<html><head><meta charset="utf-8"><title>…</title></head><body>\n…</body></html>\n`; `title` thoát `& < > "`.
  - `html_to_markdown`: `dom_query::Document::from(html)`, đi đệ quy từ `body` (không có thì từ gốc). Khối (`h1-6`, `p`, `ul`, `ol`, `table`, `pre`, `blockquote`, `div`/`section`/khối lạ) kết bằng dòng trống; inline gom chữ. Chữ thường: gộp mọi chuỗi khoảng trắng ASCII thành một dấu cách (không đụng `\u{a0}`), rồi `escape_plain`; trong `pre`/`code` giữ nguyên không thoát. `br` → `\` + `\n`. Kết quả chuẩn hoá: không có dòng trống thừa, kết thúc đúng một `\n`.
- [ ] **Step 4: Chạy** → PASS. Nếu test vòng lệch, chỉnh bộ đọc cho ra đúng dạng MD chuẩn, không sửa `SAMPLE`.
- [ ] **Step 5: Commit** `feat(morphix-core): HTML to and from Markdown`.

---

### Task 3: `ops/docx.rs` — đọc DOCX ra Markdown

**Files:**
- Create: `crates/morphix-core/src/ops/docx.rs` (unit test trong file, có helper `fn build_docx(document_xml: &str, extra: &[(&str, &str)]) -> Vec<u8>` dựng zip trong bộ nhớ)
- Modify: `crates/morphix-core/src/ops/mod.rs`, `Cargo.toml` (`cargo add -p morphix-core quick-xml@0.42.0`)

**Interfaces:**
- Consumes: `escape_plain`, `table_to_markdown`.
- Produces: `pub struct DocxToMarkdown { pub markdown: String, pub images_dropped: usize }`; `pub fn docx_to_markdown(path: &Path) -> Result<DocxToMarkdown>`; nội bộ `fn docx_bytes_to_markdown(path: &Path, bytes: &[u8]) -> Result<DocxToMarkdown>` để test không cần file (path chỉ để gắn vào lỗi).

- [ ] **Step 1: Viết test thất bại:**
  - `headings_follow_style_names_even_when_ids_are_translated`: `styles.xml` có style id `Kop1` tên `heading 1`; đoạn `w:pStyle w:val="Kop1"` → `"# Chào\n"`.
  - `run_formatting_respects_val_false`: run `<w:b/>` → `**a**`; run `<w:b w:val="0"/>` → `a` không đậm; `<w:i/>` → `*`; `<w:strike/>` → `~~`.
  - `hyperlink_target_comes_from_rels`: `<w:hyperlink r:id="rId5">` + rels `rId5 → https://a.b` → `[chữ](https://a.b)`.
  - `numbered_and_bullet_lists_nest_by_ilvl`: `numbering.xml` có `numId 1 → abstract` với `ilvl 0 numFmt decimal`, `numId 2 → bullet`; ba đoạn (num 2 lvl 0, num 2 lvl 1, num 1 lvl 0) → `"- a\n  - b\n\n1. c\n"`.
  - `table_becomes_markdown_table`: `w:tbl` 2×2 → như `table_to_markdown`.
  - `text_with_markdown_characters_is_escaped`: `a*b` → `a\*b`.
  - `breaks_tabs_and_images`: `w:br` → ngắt dòng cứng, `w:tab` → `\t`, `w:drawing` với `wp:docPr descr="logo"` → chữ `logo`, `images_dropped == 1`.
  - `missing_document_xml_is_corrupt`: zip không có `word/document.xml` → `Err(Corrupt)` lý do chứa `"word/document.xml"`.
  - `not_a_zip_is_corrupt`: bytes `b"nope"` → `Err(Corrupt)`.
  - `oversized_part_is_corrupt`: `read_part` với `limit` nhỏ hơn mục → `Err(Corrupt)`.
- [ ] **Step 2: Chạy** `cargo test -p morphix-core --lib docx` → FAIL.
- [ ] **Step 3: Cài đặt:**
  - Đọc từng mục bằng `fn read_part(archive, name, limit: u64) -> Result<Option<String>>` (`take(limit + 1)`, vượt → `Corrupt` "phần … quá lớn"); `const MAX_PART_BYTES: u64 = 256 << 20` kèm WHY chống bom nén (cùng ý `ops::archive`). File OLE (bắt đầu `D0 CF 11 E0`) → `Corrupt` "file Word có mật khẩu hoặc định dạng .doc cũ, chưa hỗ trợ".
  - Duyệt `document.xml` bằng `quick_xml::Reader` theo sự kiện: `w:p` (đọc `w:pPr/w:pStyle`, `w:numPr/w:ilvl`, `w:numId`), `w:r` (`w:rPr` b/i/strike với `w:val` ∈ {`0`,`false`} là tắt), `w:t` (chữ), `w:hyperlink`, `w:tbl/w:tr/w:tc`, `w:br`, `w:tab`, `w:drawing`/`w:pict` (`wp:docPr@descr` hoặc `@title` làm alt). Tiêu đề nhận theo tên style (`heading N`, không phân biệt hoa thường) hoặc id `HeadingN` khi không có `styles.xml`. Style tên `Quote`/`Intense Quote` → `> `; style tên `Code` hoặc `HTML Preformatted` → khối code.
  - Đoạn liền nhau cùng danh sách không có dòng trống giữa; đổi danh sách hoặc ra khỏi danh sách thì có dòng trống. Không bỏ `w:t` nào, kể cả trong cấu trúc không nhận ra.
- [ ] **Step 4: Chạy** → PASS.
- [ ] **Step 5: Commit** `feat(morphix-core): read Word documents as Markdown`.

---

### Task 4: `ops/docx.rs` — ghi Markdown ra DOCX

**Files:**
- Modify: `crates/morphix-core/src/ops/docx.rs`

**Interfaces:**
- Consumes: Task 3 (`docx_bytes_to_markdown` cho test vòng), `SAMPLE` (Task 2).
- Produces: `pub fn markdown_to_docx(markdown: &str, out: &mut (impl std::io::Write + std::io::Seek)) -> Result<()>`.

- [ ] **Step 1: Viết test thất bại:**
  - `markdown_round_trips_through_docx`: ghi `SAMPLE` vào `Cursor<Vec<u8>>` rồi `docx_bytes_to_markdown(..).markdown == SAMPLE`.
  - `docx_has_required_parts`: zip ra có đủ `[Content_Types].xml`, `_rels/.rels`, `word/document.xml`, `word/_rels/document.xml.rels`, `word/styles.xml`, `word/numbering.xml`.
  - `xml_special_characters_are_escaped`: MD `a < b & "c"` → `word/document.xml` parse được bằng `quick_xml` không lỗi và đọc lại ra `a < b & "c"` (sau `markdown_to_plain`).
- [ ] **Step 2: Chạy** → FAIL.
- [ ] **Step 3: Cài đặt:** duyệt sự kiện `pulldown-cmark` (cùng options như `markdown_to_plain`), sinh `w:p` với `w:pStyle` `Heading1..6`/`Quote`/`Code`, run có `w:b`/`w:i`/`w:strike`/`w:rStyle` `CodeChar`, `w:hyperlink r:id` + quan hệ `TargetMode="External"`, danh sách dùng `numId 1` (số) / `2` (gạch) và `w:ilvl` theo độ sâu, bảng `w:tbl` có `w:tblBorders`, ngắt dòng cứng `w:br`, khối code mỗi dòng một `w:p` style `Code`. `styles.xml` khai báo các style đó với tên chuẩn (`heading 1`…, `Quote`, `Code`). `numbering.xml`: abstract 0 `decimal` 9 cấp, abstract 1 `bullet` 9 cấp. Mọi chữ thoát `& < > "`; `w:t` có `xml:space="preserve"`.
- [ ] **Step 4: Chạy** → PASS. Mở thử file sinh ra bằng Word/LibreOffice nếu máy có (không có thì ghi "chưa mở thử" vào ledger).
- [ ] **Step 5: Commit** `feat(morphix-core): write Markdown as Word documents`.

---

### Task 5: Nối job, catalog, cảnh báo ảnh, CLI và menu

**Files:**
- Modify: `crates/morphix-core/src/catalog.rs` (`text_targets`: `Markdown → [Txt, Markdown, Html, Docx]`; `Html → [Markdown, Docx, Txt]`; `Docx → [Markdown, Html, Txt]`), `crates/morphix-core/src/job.rs` (`convert_text` nhận thêm `warnings: &mut Vec<String>` từ `run_item`; `write_converted` thay nhánh tạm của Task 1: đọc nguồn thành MD — `read_utf8` cho MD, `html_to_markdown` cho HTML, `docx_to_markdown` cho DOCX — rồi ghi: `Txt` → `markdown_to_plain`, `Markdown` → MD, `Html` → `markdown_to_html(md, stem)`, `Docx` → `markdown_to_docx`; MD→MD giữ chép nguyên văn như D2; `images_dropped > 0` → push cảnh báo `"{tên}: bỏ {n} ảnh (chỉ giữ chữ thay thế)"`), `crates/morphix-cli/src/main.rs` (`Target` thêm `Html`, `Docx` + `text_format`), `crates/morphix-gui/ui/src/strings.ts` nếu có nhãn định dạng cần thêm
- Test: `crates/morphix-core/tests/text.rs` (mục mới "DOCX/HTML"), `crates/morphix-cli/tests/cli.rs`, `crates/morphix-gui/tests/menu_items.rs`, test catalog

**Interfaces:**
- Consumes: Task 1-4.

- [ ] **Step 1: Viết test thất bại:**
  - `tests/text.rs` (qua `run_job`, theo helper `convert` của D2; với DOCX đọc kết quả bằng `docx_to_markdown`): `md_to_html_and_back`, `md_to_docx_and_back`, `docx_to_html_keeps_heading_and_bold`, `html_to_docx_keeps_list`, `docx_to_txt_has_no_markdown_marks`, `html_to_txt_has_no_markdown_marks`, `html_with_image_warns_about_dropped_images` (`report.items[0].warnings == ["a.html: bỏ 1 ảnh (chỉ giữ chữ thay thế)"]`), `txt_to_docx_is_rejected` (`InvalidInput`).
  - Catalog: nhãn việc cho `Docx` là `["MD", "HTML", "TXT"]`, cho `Html` là `["MD", "DOCX", "TXT"]`, cho `Markdown` là `["TXT", "HTML", "DOCX"]`.
  - CLI: `convert a.md --to docx` tạo `a.docx`; `convert a.md --to html` tạo `a.html` chứa `<h1>`; `convert a.txt --to docx` exit 2, không tạo file.
  - GUI: `operation_for("text:docx", …) == ConvertText { to: Docx }`, `text:html` tương tự.
- [ ] **Step 2: Chạy** các test trên → FAIL.
- [ ] **Step 3: Cài đặt** như phần Files; sửa mọi `match` đầy đủ mà trình biên dịch báo; cập nhật hai comment cũ D2 để lại (`catalog.rs` "Không thêm PDF sang MD", `main.rs` "chỉ tạo PdfToText") vì file đang chạm.
- [ ] **Step 4: Kiểm tra cuối:** rustfmt file đã chạm; clippy `-D warnings` sạch; `cargo test --workspace --no-fail-fast` → fail chỉ là baseline 39; `npm run check` và `npm test` trong `crates/morphix-gui/ui` nếu đã sửa `strings.ts`.
- [ ] **Step 5: Commit** `feat(morphix-core): Word and HTML conversions` (CLI/GUI cùng commit hoặc tách theo scope).
