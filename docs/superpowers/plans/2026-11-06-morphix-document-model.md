# Morphix D4: mô hình tài liệu chung — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** DOCX ↔ HTML và DOCX/HTML → TXT chuyển thẳng sang đích qua một cây tài liệu chung. Giữ gạch chân, trên/dưới, ô gộp, ô nhiều khối, ảnh nhúng và chú thích, không còn bị giới hạn ở mức Markdown.

**Architecture:** Module mới `ops/doc.rs` chứa cây `Document`. Mỗi định dạng có `read(...) -> Document` và `write(&Document, ...)`. Bộ đọc DOCX/HTML ở D3 được sửa để dựng cây thay vì chuỗi MD; phần dựng chuỗi MD chuyển về `markdown::write`. `job.rs::write_document` chỉ còn đọc → cây → ghi.

**Tech Stack:** Rust 2021; pulldown-cmark 0.13.6, dom_query 0.28.0, quick-xml 0.42.0, zip 9, image 0.25, infer 0.22; thêm `base64 = "0.22.1"` (đã có sẵn trong Cargo.lock).

**Spec:** `docs/superpowers/specs/2026-11-06-morphix-document-model-design.md`

## Global Constraints

- Không bao giờ ghi đè hay làm mất file gốc; mọi đầu ra đi qua `write_output` (file tạm rồi đổi tên).
- Offline: không tải ảnh `http(s):`.
- Ảnh đọc từ đĩa chỉ là file thường, nằm trong thư mục của file nguồn (so sau `canonicalize`), mỗi ảnh tối đa `MAX_PART_BYTES` (256 MiB).
- Kiểu ảnh giữ: PNG, JPEG, GIF, BMP, WebP, nhận theo bytes bằng `infer`. Ghi DOCX thì đổi WebP sang PNG (WHY: Word cũ không đọc WebP).
- Đầu ra MD cho nội dung D3 phải giữ y như cũ. Test hiện có chỉ được đổi kỳ vọng khi ghi lý do vào ledger.
- Giữ các hàm `docx_to_markdown`, `html_to_markdown`, `markdown_to_docx`, `markdown_to_html` cùng struct kết quả (`DocxToMarkdown`, `HtmlToMarkdown`), nay là lớp vỏ mỏng gọi read/write mới. WHY: các test có sẵn gọi chúng, và đó là lưới an toàn của việc viết lại.
- Comment trong code là ghi chú `WHY` bằng tiếng Việt; chuỗi người dùng thấy (cảnh báo, lỗi) bằng tiếng Việt.
- `cargo clippy --workspace --all-targets -- -D warnings` sạch. `cargo test --workspace --no-fail-fast` đúng 39 lỗi gốc (23 integration.rs Linux, 11 pdf_text + 3 cli.rs PDFium, 2 menu.rs /tmp).
- Sửa file nào thì `rustfmt --edition 2021` file đó, không format cả workspace.
- `colspan`/`rowspan` đọc vào được kẹp trong `1..=MAX_SPAN` (`MAX_SPAN = 1000`). WHY: `colspan="99999999"` không được làm nổ lưới.

## Review Focus

1. Ảnh tương đối là symlink trỏ ra ngoài thư mục HTML: không được đọc. Kiểm bằng `canonicalize` + `starts_with`; test `relative_image_outside_the_folder_is_not_read` (Task 6) dùng `../`, test symlink chỉ chạy cfg(unix).
2. `rowspan` lớn hơn số hàng còn lại, hoặc `colspan` rất lớn: không panic, không cấp phát khổng lồ. Test `oversized_spans_are_clamped` (Task 8).
3. DOCX có `w:vMerge` "continue" mà không có ô "restart" phía trên: thành ô thường, không mất chữ. Test `vmerge_continue_without_restart_keeps_its_text` (Task 8).
4. Tham chiếu chú thích tới id không có trong `footnotes.xml`, hoặc không có file đó: không panic, tính là chú thích bị bỏ (giữ thông báo D3). Test hiện có `docx_with_footnotes_and_header_warns_what_was_left_out` phải giữ xanh (Task 7).
5. Ảnh nằm trong ô bảng hoặc trong chú thích: vẫn được nhúng khi ghi HTML/DOCX. Test `image_inside_table_cell_is_embedded` (Task 6).

---

## File Structure

| File | Vai trò |
|---|---|
| `crates/morphix-core/src/ops/doc.rs` (mới) | `Document`, `Block`, `Inline`, `Style`, `Cell`, `Image`, `Dropped`, `grid()`, `plain_inlines()`, `load_image_src()`, `add_image()`, `MAX_PART_BYTES`, `MAX_SPAN` |
| `ops/markdown.rs` | thêm `read(md, base) -> Document`, `write(&Document) -> (String, Dropped)`; hàm thoát ký tự giữ ở đây |
| `ops/html.rs` | `read(html, base) -> Document` (từ `Walker`), `write(&Document, title) -> String`; vỏ `html_to_markdown`, `markdown_to_html` (giữ `push_html`) |
| `ops/docx.rs` | `read(path) -> Result<Document>` (từ `convert_document`), `write(&Document, out) -> Result<Dropped>` (từ `DocxWriter`); vỏ `docx_to_markdown`, `markdown_to_docx` |
| `ops/text.rs` | thêm `write_plain(&Document) -> String` |
| `job.rs` | `write_document` đọc → cây → ghi; cảnh báo từ `Dropped` |
| `Cargo.toml` (morphix-core) | `base64 = "0.22.1"` (Task 6) |

## Interfaces chung (định nghĩa ở Task 1, mọi task dùng)

```rust
// ops/doc.rs
pub(crate) const MAX_PART_BYTES: u64 = 256 << 20;   // chuyển từ docx.rs
pub(crate) const MAX_SPAN: u32 = 1000;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Document { pub blocks: Vec<Block>, pub footnotes: Vec<Vec<Block>>, pub images: Vec<Image>, pub dropped: Dropped }
#[derive(Debug, Clone, PartialEq)]
pub struct Image { pub bytes: Vec<u8>, pub mime: &'static str }       // "image/png" | "image/jpeg" | "image/gif" | "image/bmp" | "image/webp"
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Dropped { pub images: usize, pub notes: usize, pub headers: bool, pub md_downgraded: bool }
#[derive(Debug, Clone, PartialEq)]
pub enum Block {
    Heading(u8, Vec<Inline>), Paragraph(Vec<Inline>),
    List { start: Option<u64>, items: Vec<Vec<Block>> },   // None = gạch đầu dòng; Some(n) = danh sách số bắt đầu từ n
    Table(Vec<Vec<Cell>>), Code(String), Quote(Vec<Block>), Rule,
}
#[derive(Debug, Clone, PartialEq)]
pub struct Cell { pub blocks: Vec<Block>, pub colspan: u32, pub rowspan: u32, pub header: bool }
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Style { pub bold: bool, pub italic: bool, pub underline: bool, pub strike: bool, pub sup: bool, pub sub: bool, pub code: bool }
#[derive(Debug, Clone, PartialEq)]
pub enum Inline { Text(String, Style), Link(String, Vec<Inline>), Image { index: Option<usize>, alt: String }, FootnoteRef(usize), Break }

impl Dropped { pub fn add(&mut self, other: Dropped) }               // cộng số, OR các cờ
pub enum Slot<'a> { Cell(&'a Cell), Below { origin: &'a Cell, leftmost: bool }, Spanned }
pub fn grid(rows: &[Vec<Cell>]) -> Vec<Vec<Slot<'_>>>                 // mọi hàng cùng độ rộng; ô bị rowspan đè = Below, ô bị colspan đè = Spanned
pub fn plain_inlines(inlines: &[Inline]) -> String                    // chữ, chữ link, alt ảnh; Break → "\n"; FootnoteRef → ""
pub fn plain_blocks(blocks: &[Block]) -> String                       // các khối nối bằng "\n"
```

Chỗ lệch spec có chủ ý: `List.ordered: bool` thành `start: Option<u64>`, vì số bắt đầu của `<ol start>` và MD `3.` cần được giữ.

---

### Task 1: `doc.rs` và `markdown::write`

**Files:**
- Create: `crates/morphix-core/src/ops/doc.rs`
- Modify: `crates/morphix-core/src/ops/mod.rs` (`pub mod doc;`), `crates/morphix-core/src/ops/markdown.rs`
- Test: unit tests trong `doc.rs` và `markdown.rs`

**Interfaces:**
- Produces: toàn bộ "Interfaces chung"; `pub fn write(doc: &Document) -> (String, Dropped)` trong `markdown.rs`.

Quy tắc của `markdown::write`, chép đúng dạng mà D3 sinh ra:
- Khối nối bằng `"\n\n"`, có `"\n"` cuối nếu không rỗng.
- Heading: `"#"*n + " " + inline`.
- Paragraph: `escape_lines_start(inline.trim())`.
- Quote: các khối bên trong, mỗi dòng có tiền tố `"> "`, dòng rỗng thành `">"`.
- Code: `code_fence`. Rule: `"---"`.
- List: thụt 4 khoảng trắng mỗi cấp. Dấu mục `"- "` hoặc `"{n}. "`, n tăng từ `start`. Khối đầu của mục nằm trên dòng dấu mục; List con theo ngay sau, sâu thêm một cấp.
- Table: `table_to_markdown` với chữ ô = `plain_blocks` (Slot khác Cell → `""`, và đặt `md_downgraded` khi có ô gộp).
- Inline:
  - gộp các Text liền nhau cùng Style rồi bọc bằng `wrap()` (chuyển từ docx.rs, giữ tách khoảng trắng mép và thứ tự `~~`, `*`, `**`; `code` dùng `code_span`);
  - underline/sup/sub không có ký hiệu: chữ thường, đặt `md_downgraded`;
  - Link: `[inner](link_destination(url))`, inner rỗng thì chỉ inner;
  - Image: `escape_specials(alt)` và `dropped.images += 1`;
  - Break: `"\\\n"`;
  - FootnoteRef(i): `[^{i+1}]`; chú thích ghi ở cuối dạng `[^{i+1}]: ` + khối.

- [ ] **Step 1: Write the failing tests** (trong `markdown.rs` `mod tests`, dựng `Document` bằng tay)

```rust
#[test] fn write_paragraph_heading_and_marks() {
    // Heading(1,[Text("T")]), Paragraph([Text("a ",default), Text("b",bold), Text(" *c*",default)])
    assert_eq!(write(&doc).0, "# T\n\na **b** \\*c\\*\n");
}
#[test] fn write_nested_lists_use_four_spaces() {
    // List{start:Some(1), items:[[P("một"), List{start:None, items:[[P("con")]]}], [P("hai")]]}
    assert_eq!(write(&doc).0, "1. một\n    - con\n2. hai\n");
}
#[test] fn write_underline_and_merged_cells_downgrade() {
    // P([Text("u", underline)]) + Table 2x2 có ô colspan 2
    let (md, dropped) = write(&doc);
    assert!(md.starts_with("u\n\n"));
    assert!(dropped.md_downgraded);
}
#[test] fn write_image_keeps_alt_and_counts() {
    // P([Image{index:None, alt:"logo"}])
    assert_eq!(write(&doc), ("logo\n".to_string(), Dropped { images: 1, ..Default::default() }));
}
#[test] fn write_footnotes_at_end() {
    // P([Text("a"), FootnoteRef(0)]), footnotes:[[P("ghi chú")]]
    assert_eq!(write(&doc).0, "a[^1]\n\n[^1]: ghi chú\n");
}
```

Trong `doc.rs`:

```rust
#[test] fn grid_marks_spanned_and_below_slots() {
    // hàng 0: A(colspan 2), B(rowspan 2); hàng 1: C, D
    // lưới: [[Cell A, Spanned, Cell B], [Cell C, Cell D, Below{origin:B, leftmost:true}]]
}
#[test] fn plain_inlines_takes_link_text_and_alt() {
    assert_eq!(plain_inlines(&[Link("u".into(), vec![Text("x")]), Break, Image{index:None, alt:"y".into()}]), "x\ny");
}
```

- [ ] **Step 2:** `cargo test -p morphix-core --lib ops::` → FAIL (chưa có `doc`, `write`).
- [ ] **Step 3:** Viết `doc.rs` theo Interfaces chung và `markdown::write`. Chuyển `wrap` từ docx.rs sang `markdown.rs` dạng `pub(crate) fn wrap(text: &str, style: Style) -> String`, giữ một bản. `grid()`: đặt ô theo thứ tự đọc vào cột trống đầu tiên, kẹp span theo `MAX_SPAN` và theo số hàng còn lại.
- [ ] **Step 4:** `cargo test -p morphix-core --lib ops::` → PASS.
- [ ] **Step 5: Commit** `feat(morphix-core): mô hình tài liệu chung và bộ ghi Markdown`

---

### Task 2: `html::read` trả `Document`

**Files:**
- Modify: `crates/morphix-core/src/ops/html.rs`
- Test: unit tests có sẵn trong `html.rs` cùng `tests/text.rs` (HTML)

**Interfaces:**
- Consumes: Task 1.
- Produces: `pub fn read(html: &str, base: Option<&Path>) -> Document`. `base` là thư mục chứa file HTML (dùng ở Task 6, giờ bỏ qua). `html_to_markdown(html)` = `let doc = read(html, None); let (markdown, d) = markdown::write(&doc);` với `images_dropped = doc.dropped.images + d.images`. WHY: ảnh tính một lần, ở bộ ghi MD.

- [ ] **Step 1:** Thêm test `read_builds_tree_not_markdown`: `read("<p>a <b>b</b></p><ul><li>x</li></ul>", None).blocks == [Paragraph([Text("a ",d), Text("b",bold)]), List{start:None, items:[[Paragraph([Text("x",d)])]]}]`.
- [ ] **Step 2:** `cargo test -p morphix-core --lib ops::html` → FAIL.
- [ ] **Step 3:** Sửa `Walker` thành dựng `Vec<Block>` / `Vec<Inline>`, giữ `SKIPPED`, `BLOCKS`, `MAX_DEPTH`, `collapse`.
  - Khoảng trắng: Text đã `collapse`; đầu/cuối đoạn và quanh Break được cắt ở bộ đọc.
  - Ô bảng: `Cell { blocks, colspan:1, rowspan:1, header: th }` (span đọc ở Task 8).
  - `caption` thành đoạn trước bảng; `pre` thành Code; chữ rời trong `ul` thành mục.
  - `<img>` thành `Inline::Image { index: None, alt }` (Task 6 điền ảnh).
- [ ] **Step 4:** `cargo test -p morphix-core --lib ops:: && cargo test -p morphix-core --test text` → mọi test có sẵn PASS, đầu ra MD không đổi.
- [ ] **Step 5: Commit** `refactor(morphix-core): bộ đọc HTML dựng cây tài liệu`

---

### Task 3: `docx::read` trả `Document`

**Files:**
- Modify: `crates/morphix-core/src/ops/docx.rs`
- Test: unit tests có sẵn trong `docx.rs` cùng `tests/text.rs` (DOCX)

**Interfaces:**
- Consumes: Task 1.
- Produces:
  - `pub fn read(path: &Path) -> Result<Document>` và `pub(crate) fn read_from<R: Read + Seek>(path: &Path, reader: R) -> Result<Document>`;
  - `docx_to_markdown`, `docx_bytes_to_markdown` thành vỏ (`notes_dropped = dropped.notes`, `headers_dropped = dropped.headers`);
  - `fn parse_body(path: &Path, part: &str, xml: &str, context: &Context, doc: &mut Document) -> Result<Vec<Block>>`, đọc được mọi part chứa `w:p`/`w:tbl` (Task 7 dùng cho footnotes.xml).

- [ ] **Step 1:** Test `read_builds_tree`: thân `<w:p><w:pPr><w:numPr><w:ilvl w:val="1"/><w:numId w:val="2"/></w:numPr></w:pPr><w:r><w:t>x</w:t></w:r></w:p>` có numbering bullet, cho ra `List{start:None, items:[[List{start:None, items:[[Paragraph([Text("x")])]]}]]}` (mục cấp 0 rỗng bao cấp 1).
- [ ] **Step 2:** `cargo test -p morphix-core --lib ops::docx` → FAIL.
- [ ] **Step 3:** Thay `Piece`/`render`/`Blocks` bằng dựng cây.
  - Đoạn (`Kind` giữ nguyên): Heading, Quote(`[Paragraph]`), Paragraph; các đoạn Code liền nhau gộp thành một `Code`.
  - Mục danh sách:
    - một ngăn xếp `(numId, level, List)`;
    - đổi numId ở cấp 0 thì đóng danh sách;
    - cấp sâu hơn thì List con nằm trong khối cuối của mục cuối (chưa có mục thì tạo mục rỗng);
    - `start = Some(1)` nếu `ordered`.
  - Link có đích thành `Inline::Link`, không đích thì chỉ chữ.
  - Ô thành `Cell` với các khối của mọi `w:p` trong `w:tc`; bảng lồng góp khối vào ô như D3.
  - `drawing`/`pict` thành `Inline::Image{index:None, alt}`, alt lấy từ `docPr descr|title` như D3.
  - Đếm `footnoteReference|endnoteReference|commentReference` vào `dropped.notes` (Task 7 đổi), header thành `dropped.headers`.
  - Giữ mọi xử lý D3: Fallback, `*PrChange`, tabs trong pPr, noBreakHyphen, softHyphen, sym, textbox `outer`, OLE, `read_part`, giới hạn part.
- [ ] **Step 4:** `cargo test -p morphix-core --lib ops:: && cargo test -p morphix-core --test text` → PASS, đầu ra MD không đổi. Nếu một test đổi là do cấu trúc mục rỗng, ghi ruling kèm lý do vào ledger.
- [ ] **Step 5: Commit** `refactor(morphix-core): bộ đọc DOCX dựng cây tài liệu`

---

### Task 4: `markdown::read` và `docx::write` từ cây

**Files:**
- Modify: `crates/morphix-core/src/ops/markdown.rs`, `crates/morphix-core/src/ops/docx.rs`
- Test: unit tests ghi DOCX có sẵn (`markdown_round_trips_through_docx`, `docx_has_required_parts`, …)

**Interfaces:**
- Consumes: Task 1, 3.
- Produces:
  - `pub fn read(markdown: &str, base: Option<&Path>) -> Document` (pulldown-cmark, options TABLES|STRIKETHROUGH|FOOTNOTES);
  - `pub fn write(doc: &Document, out: &mut (impl Write + Seek)) -> Result<Dropped>` trong `docx.rs`;
  - `markdown_to_docx(md, out)` = `let doc = markdown::read(md, None); Ok(doc.dropped.images + write(&doc, out)?.images)`.

- [ ] **Step 1:** Test `markdown_read_builds_tree`: `read("1. a\n\n   b\n2. c\n", None).blocks == [List{start:Some(1), items:[[P(a), P(b)], [P(c)]]}]`. Test `continuation_paragraph_in_list_item_is_indented_not_numbered`: ghi DOCX từ cây đó; `document.xml` có đúng 2 `w:numPr` và đoạn "b" có `<w:ind w:left="720"/>`.
- [ ] **Step 2:** `cargo test -p morphix-core --lib ops::` → FAIL.
- [ ] **Step 3:** Hai bộ chuyển:
  - **`markdown::read`:** HTML inline `<br>` thành Break, HTML inline khác bị bỏ; HTML khối thành `Paragraph(Text(plain_blocks(html::read(html,None).blocks)))`; ảnh thành `Image{index:None, alt}` (Task 6 nạp ảnh); footnote tạm thành chữ `[label]` như D3 (Task 7 đổi).
  - **`DocxWriter`** đổi đầu vào từ `MdEvent` sang duyệt cây, ghi vào `String` để dùng lại cho chú thích. Giữ:
    - styles, numbering, `word_level`, `xml_escape`;
    - mỗi List một `w:num`;
    - viền bảng, ô rỗng có `<w:p/>`.
  - Đoạn thứ 2 trở đi trong một mục: `<w:ind w:left="{720*(level+1)}"/>`, không có numPr.
- [ ] **Step 4:** `cargo test -p morphix-core --lib ops:: && cargo test -p morphix-core --test text` → PASS.
- [ ] **Step 5: Commit** `refactor(morphix-core): ghi DOCX từ cây tài liệu`

---

### Task 5: `html::write`, `text::write_plain`, nối `job.rs`

**Files:**
- Modify: `ops/html.rs`, `ops/text.rs`, `job.rs:286-339` (`write_document`)
- Test: `tests/text.rs`

**Interfaces:**
- Consumes: Task 1–4.
- Produces: `pub fn write(doc: &Document, title: &str) -> String` (html.rs) và `pub fn write_plain(doc: &Document) -> String` (text.rs).

`write_document` mới:
- `from`: Html → `html::read(&read_utf8(input)?, input.parent())`; Docx → `docx::read(input)?`; Markdown → `markdown::read(&read_utf8(input)?, input.parent())`.
- Riêng (Markdown, Html) vẫn gọi `markdown_to_html` (giữ push_html).
- Đích: Txt → `write_plain`; Html → `html::write(&doc, stem)`; Docx → `docx::write`; Markdown → `markdown::write`.
- `dropped` = của cây + của bộ ghi. Cảnh báo:
  - `images > 0` → `"{name}: bỏ {n} ảnh (chỉ giữ chữ thay thế)"`;
  - các phần `["{n} chú thích/ghi chú", "đầu/chân trang", "định dạng MD không có (gạch chân, trên/dưới, ô gộp)"]` có mặt thì nối `" và "` → `"{name}: bỏ {phần} (chỉ giữ nội dung chính)"` (khớp chuỗi D3 khi chỉ có hai phần đầu).

`html::write`:
- Khung trang:

  ```html
  <!doctype html>
  <html><head><meta charset="utf-8"><title>{title}</title><style>table{border-collapse:collapse}td,th{border:1px solid #999;padding:4px}</style></head><body>
  ```
- Khối: `h1–h6`, `p`, `ul`/`ol` (có `start` khi khác 1), `table`/`tr`/`td`/`th` (thuộc tính colspan/rowspan chỉ khi > 1), `pre><code`, `blockquote`, `hr`.
- Inline: `strong em u s sup sub code`, `a href`, `br`.
- Mọi chữ qua `escape_html`.

`write_plain`:
- Heading/đoạn thành một dòng (Break thành `"\n"`); khối nối `"\n\n"`, có `"\n"` cuối.
- List: `"- "` hoặc `"{n}. "`, mỗi cấp thụt 2 khoảng trắng.
- Table: hàng gồm các ô nối `"\t"` theo `grid` (ô gộp = `""`), chữ ô = `plain_blocks` với `"\n"` thay bằng `" "`.
- Link: `"{chữ} ({url})"`, trừ khi chữ == url.
- Image: `"[ảnh: {alt}]"`; Code nguyên văn; Rule bị bỏ.

- [ ] **Step 1: Failing tests** (`tests/text.rs`)

```rust
#[test] fn docx_to_html_writes_html_directly() {
    // MD_SAMPLE → DOCX → HTML: chứa "<h1>Tiêu đề</h1>", "<strong>đậm</strong>", "<a href=\"https://a.b\">link</a>", "border-collapse"
}
#[test] fn docx_and_html_to_txt_keep_link_urls() {
    // MD_SAMPLE → DOCX → TXT == "Tiêu đề\n\nĐoạn có đậm và link (https://a.b).\n\n- một\n- hai\n"
    // "<p><a href=\"https://x.y\">x</a> và <a href=\"https://x.y\">https://x.y</a></p>" → TXT == "x (https://x.y) và https://x.y\n"
}
```

Sửa kỳ vọng `docx_to_txt_has_no_markdown_marks` thành `link (https://a.b)`, và ghi ruling: spec yêu cầu TXT giữ URL.
- [ ] **Step 2:** `cargo test -p morphix-core --test text` → FAIL.
- [ ] **Step 3:** Viết `html::write`, `write_plain`, `write_document` như trên. Bỏ import thừa trong job.rs.
- [ ] **Step 4:** `cargo test -p morphix-core --lib --test text --test job` → PASS.
- [ ] **Step 5: Commit** `feat(morphix-core): DOCX/HTML chuyển thẳng sang HTML/DOCX/TXT không qua Markdown`

---

### Task 6: Ảnh

**Files:**
- Modify: `crates/morphix-core/Cargo.toml` (`base64 = "0.22.1"`), `ops/doc.rs`, `ops/html.rs`, `ops/docx.rs`, `ops/markdown.rs`
- Test: unit tests và `tests/text.rs`

**Interfaces:**
- Produces:
  - `pub(crate) fn add_image(doc: &mut Document, bytes: Vec<u8>) -> Option<usize>`: mime theo `infer`, chỉ 5 kiểu trên, sai thì `None`.
  - `pub(crate) fn load_image_src(doc: &mut Document, src: &str, base: Option<&Path>) -> Option<usize>`:
    - `data:image/...;base64,` → giải mã;
    - đường dẫn tương đối (không có `:` trước `/`, không bắt đầu bằng `/` hay `\`) → `base.join(src)`, `canonicalize`, phải `starts_with(canonicalize(base))`, `is_file`, và `len <= MAX_PART_BYTES`;
    - còn lại → `None`.
  - Mọi lỗi I/O trả `None` kèm `tracing::warn!`. Bộ đọc gọi được thì `Image{index:Some(i)}`, không thì `index: None` và `dropped.images += 1`.

Theo định dạng:
- **Đọc HTML/MD:** `load_image_src` với `base`.
- **Đọc DOCX:** `a:blip r:embed` → rels Target (tương đối `word/`) → `read_part_bytes` (bản bytes của `read_part`, cùng giới hạn) → `add_image`.
- **Ghi HTML:** `<img src="data:{mime};base64,{…}" alt="…">`.
- **Ghi DOCX:**
  - `word/media/image{n}.{png|jpg|gif|bmp}`, WebP đổi sang PNG bằng `image`;
  - rel `{REL_TYPE}/image`; `<Default Extension=…>` trong `[Content_Types].xml` cho từng đuôi đã dùng;
  - kích thước: `image::ImageReader::new(Cursor).with_guessed_format()?.into_dimensions()`, EMU = px × 9525, thu nhỏ theo tỉ lệ nếu cx > 5486400. Không đọc được kích thước thì bỏ ảnh và đếm.
  - Run ảnh:

  ```xml
  <w:r><w:drawing><wp:inline><wp:extent cx="{cx}" cy="{cy}"/><wp:docPr id="{n}" name="Ảnh {n}" descr="{alt}"/><a:graphic xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"><a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/picture"><pic:pic xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture"><pic:nvPicPr><pic:cNvPr id="{n}" name="image{n}"/><pic:cNvPicPr/></pic:nvPicPr><pic:blipFill><a:blip r:embed="rIdI{n}"/><a:stretch><a:fillRect/></a:stretch></pic:blipFill><pic:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="{cx}" cy="{cy}"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></pic:spPr></pic:pic></a:graphicData></a:graphic></wp:inline></w:drawing></w:r>
  ```

  `w:document` thêm `xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing"`.
- **Ghi TXT:** `[ảnh: alt]`. **Ghi MD:** như Task 1 (alt và đếm).

- [ ] **Step 1: Failing tests**

```rust
// tests/text.rs — make_png từ tests/common
#[test] fn html_with_relative_image_to_docx_embeds_it()      // a.html "<p><img src=\"x.png\" alt=\"l\"></p>" + x.png cạnh → DOCX có "word/media/image1.png", rel kiểu image, warnings rỗng
#[test] fn docx_image_to_html_becomes_data_uri()             // DOCX vừa tạo ở trên → HTML chứa "src=\"data:image/png;base64,"
#[test] fn data_uri_image_round_trips_to_docx()               // <img src="data:image/png;base64,{b64 của make_png}">
#[test] fn relative_image_outside_the_folder_is_not_read()   // sub/a.html với src="../x.png", src="/abs/x.png", src="file:///…", src="http://h/x.png" → mỗi cái 1 ảnh bị bỏ, không có word/media
#[test] fn broken_image_is_dropped_with_warning()             // x.png chứa b"not png" → "a.html: bỏ 1 ảnh (chỉ giữ chữ thay thế)"
#[test] fn image_inside_table_cell_is_embedded()              // <table><tr><td><img src="x.png"></td></tr></table> → DOCX → HTML có data URI
#[cfg(unix)] #[test] fn symlinked_image_pointing_outside_is_not_read()
```

`html_with_image_warns_about_dropped_images` (đích MD) và `md_with_image_to_docx_warns_about_dropped_images` (x.png không tồn tại) phải giữ nguyên.
- [ ] **Step 2:** `cargo test -p morphix-core --test text image` → FAIL.
- [ ] **Step 3:** Viết như Interfaces.
- [ ] **Step 4:** `cargo test -p morphix-core --lib --test text` → PASS. Kiểm tay: Word mở DOCX có ảnh qua COM ở chế độ chỉ đọc.
- [ ] **Step 5: Commit** `feat(morphix-core): nhúng ảnh khi chuyển DOCX/HTML/MD`

---

### Task 7: Chú thích cuối trang

**Files:**
- Modify: `ops/docx.rs`, `ops/markdown.rs`, `ops/html.rs`, `ops/text.rs`
- Test: unit tests và `tests/text.rs`

Theo định dạng:
- **Đọc DOCX:**
  - Trước `document.xml`, đọc `word/footnotes.xml` và `word/endnotes.xml` bằng `parse_body` thành `HashMap<(bool /*endnote*/, String /*id*/), Vec<Block>>`, bỏ `w:footnote`/`w:endnote` có `w:type`.
  - Gặp `footnoteReference`/`endnoteReference` có trong map thì đẩy vào `doc.footnotes` (theo thứ tự gặp) và thêm `FootnoteRef(i)`. Không có trong map thì `dropped.notes += 1`.
  - `commentReference` vẫn `dropped.notes += 1`.
- **Đọc MD:** `FootnoteReference(label)` → `FootnoteRef` theo thứ tự gặp label; `FootnoteDefinition(label)` → khối của chú thích. Label không có định nghĩa → chữ `[label]`.
- **Ghi DOCX:**
  - `word/footnotes.xml`:

    ```xml
    <w:footnotes xmlns:w=… xmlns:r=…><w:footnote w:type="separator" w:id="-1"><w:p><w:r><w:separator/></w:r></w:p></w:footnote><w:footnote w:type="continuationSeparator" w:id="0"><w:p><w:r><w:continuationSeparator/></w:r></w:p></w:footnote>
    ```

    Rồi mỗi chú thích là `<w:footnote w:id="{i+1}">`. Đoạn đầu mở bằng `<w:r><w:rPr><w:rStyle w:val="FootnoteReference"/></w:rPr><w:footnoteRef/></w:r><w:r><w:t xml:space="preserve"> </w:t></w:r>`; đoạn có `pStyle FootnoteText`.
  - Tham chiếu trong thân: `<w:r><w:rPr><w:rStyle w:val="FootnoteReference"/></w:rPr><w:footnoteReference w:id="{i+1}"/></w:r>`.
  - `styles.xml` thêm char style `FootnoteReference` (`<w:vertAlign w:val="superscript"/>`) và paragraph style `FootnoteText`.
  - Rel `{REL_TYPE}/footnotes`; override `/word/footnotes.xml` = `application/vnd.openxmlformats-officedocument.wordprocessingml.footnotes+xml`.
  - Chỉ ghi khi có chú thích.
- **Ghi HTML:** `<sup><a href="#fn{n}" id="ref{n}">{n}</a></sup>`; cuối body là `<section class="footnotes"><ol><li id="fn{n}">…</li></ol></section>`.
- **Ghi TXT:** `[{n}]`; cuối file là `"\n\n"` cùng các dòng `[{n}] {plain_blocks}`.
- **Ghi MD:** như Task 1.

- [ ] **Step 1: Failing tests**

```rust
#[test] fn docx_footnote_survives_to_html_and_docx() // build DOCX có footnotes.xml (id 1 "ghi chú") → HTML chứa "<li id=\"fn1\">" và "ghi chú"; → MD == "a[^1]\n\n[^1]: ghi chú\n"; warnings rỗng
#[test] fn markdown_footnote_becomes_word_footnote()  // "a[^x]\n\n[^x]: n\n" → DOCX có "word/footnotes.xml" chứa "n" và document.xml chứa "w:footnoteReference w:id=\"1\""
#[test] fn footnotes_go_to_the_end_of_txt()           // → TXT == "a[1]\n\n[1] ghi chú\n"
```

Test có sẵn `docx_with_footnotes_and_header_warns_what_was_left_out` (id không có footnotes.xml) phải giữ nguyên chuỗi cảnh báo.
- [ ] **Step 2:** `cargo test -p morphix-core --test text footnote` → FAIL.
- [ ] **Step 3:** Viết như trên.
- [ ] **Step 4:** `cargo test -p morphix-core --lib --test text` → PASS. Kiểm tay: Word mở DOCX có chú thích.
- [ ] **Step 5: Commit** `feat(morphix-core): giữ chú thích cuối trang giữa DOCX, HTML, MD và TXT`

---

### Task 8: Gạch chân, trên/dưới, ô gộp

**Files:**
- Modify: `ops/docx.rs`, `ops/html.rs`, `ops/markdown.rs` (đọc `<u>` thô thì không cần), `ops/text.rs`
- Test: unit tests và `tests/text.rs`

Theo định dạng:
- **Đọc DOCX:**
  - `w:u` có `w:val` khác `none` → underline;
  - `w:vertAlign` `superscript` → sup, `subscript` → sub;
  - `w:gridSpan w:val` → colspan (kẹp);
  - `w:vMerge`:
    - `w:val="restart"` mở gộp tại cột lưới hiện tại;
    - không có val (continue) và cột đó đang mở → `rowspan += 1` cho ô gốc, bỏ ô này;
    - continue mà cột không mở → ô thường.
- **Đọc HTML:** `u`, `ins` → underline; `sup`, `sub`; `colspan`, `rowspan` (parse u32, kẹp `1..=MAX_SPAN`).
- **Ghi DOCX:** `<w:u w:val="single"/>`, `<w:vertAlign w:val="superscript|subscript"/>`.
  - Ô theo `grid`:
    - `Slot::Cell` → `<w:tc><w:tcPr>[<w:gridSpan w:val=n/>][<w:vMerge w:val="restart"/>]</w:tcPr>…`;
    - `Slot::Below{leftmost:true}` → `<w:tc><w:tcPr>[gridSpan của origin]<w:vMerge/></w:tcPr><w:p/></w:tc>`;
    - `Slot::Spanned` và `Below{leftmost:false}` → bỏ qua.
  - Thêm `<w:tblGrid>` với số `<w:gridCol/>` bằng độ rộng lưới.
- **Ghi HTML:** đã có từ Task 5. **Ghi TXT/MD:** đã có từ Task 1/5.

- [ ] **Step 1: Failing tests**

```rust
#[test] fn underline_and_superscript_survive_docx_and_html() // "<p><u>u</u> x<sup>2</sup> H<sub>2</sub>O</p>" → DOCX (document.xml có w:u, superscript, subscript) → HTML chứa "<u>u</u>", "<sup>2</sup>", "<sub>2</sub>"
#[test] fn merged_cells_survive_html_to_docx_and_back()    // <table><tr><td colspan=2>a</td><td rowspan=2>b</td></tr><tr><td>c</td><td>d</td></tr></table> → DOCX có gridSpan và vMerge restart → HTML có colspan="2" và rowspan="2"
#[test] fn merged_cells_line_up_in_txt()                  // cùng bảng → TXT == "a\t\tb\nc\td\t\n"
#[test] fn markdown_target_warns_about_downgraded_formatting() // "<p><u>u</u></p>" → MD: warnings == ["a.html: bỏ định dạng MD không có (gạch chân, trên/dưới, ô gộp) (chỉ giữ nội dung chính)"]
#[test] fn oversized_spans_are_clamped()                  // colspan="99999999" rowspan="99999999" → chạy xong, ô gốc có colspan <= 1000
#[test] fn vmerge_continue_without_restart_keeps_its_text() // unit test docx.rs: ô vMerge continue đứng đầu bảng có chữ "x" → Cell có "x"
```

- [ ] **Step 2:** `cargo test -p morphix-core --lib --test text` → FAIL ở các test mới.
- [ ] **Step 3:** Viết như trên.
- [ ] **Step 4:** Chạy toàn bộ: `cargo clippy --workspace --all-targets -- -D warnings`, rồi `cargo test --workspace --no-fail-fast` → đúng 39 lỗi gốc. Kiểm tay: Word mở DOCX có bảng gộp, chữ gạch chân, chỉ số trên.
- [ ] **Step 5: Commit** `feat(morphix-core): giữ gạch chân, chỉ số trên/dưới và ô gộp`
