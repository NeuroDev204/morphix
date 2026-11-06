# Morphix D2 — Chuyển đổi Markdown Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** MD→TXT bỏ ký hiệu Markdown; TXT, CSV/TSV, SRT/VTT và PDF→MD thêm ký hiệu hợp lý.

**Architecture:** Module thuần `ops/markdown.rs` (chuỗi vào/ra) làm mọi biến đổi; `job.rs::write_converted` chọn hàm theo cặp; PDF→MD là `Operation`/`ActionKind` mới dùng chung bước lấy chữ từng trang với PDF→TXT.

**Tech Stack:** Rust 2021, `pulldown-cmark` (phụ thuộc mới, thêm bằng `cargo add -p morphix-core pulldown-cmark`, không sửa tay `Cargo.lock`), `csv`, `pdfium-render`.

**Spec:** `docs/superpowers/specs/2026-11-05-morphix-markdown-conversion-design.md`

## Global Constraints

- Không bao giờ mất/ghi đè file nguồn; ghi qua `write_output` (file tạm rồi đổi tên).
- Lỗi phải hiện cho người dùng; chuỗi UI tiếng Việt; comment `WHY` tiếng Việt.
- Offline; không thêm phụ thuộc nào khác ngoài `pulldown-cmark`.
- Commit theo Conventional Commits có scope crate (`feat(morphix-core): …`). Máy chưa có git identity: nếu `git commit` lỗi thì ghi vào ledger "commit bỏ qua", không tự đặt identity.
- Chỉ rustfmt file đã chạm; `cargo clippy --workspace --all-targets -- -D warnings` phải sạch.
- Baseline test hiện có 36 fail cũ (23 `integration.rs`, 9 `pdf_text` + 2 `cli.rs` thiếu PDFium, ~2 `menu.rs` /tmp); dùng `cargo test --workspace --no-fail-fast`, không được thêm fail mới.
- Spec nói "trang rỗng vẫn có tiêu đề" nhưng `pdf_text` hiện báo lỗi cho trang không có chữ (cần OCR). Plan giữ hành vi lỗi đó cho PDF→MD (không tạo file cụt); `pages_to_markdown` vẫn tạo tiêu đề cho chuỗi rỗng nếu được gọi.

## Review Focus

- TXT có `\`, `*`, `_`, `#`, `<`, `&`, `[`, `]`, `|`, `~`, backtick, `- `, `1. `, `---`, `===` đầu dòng → MD → TXT phải ra đúng chữ (test vòng, Task 1).
- MD có code block, link, ảnh, bảng, danh sách lồng → TXT không mất chữ nào (Task 1).
- HTML thô trong MD (`<br>`, `<div>`) → không được mất chữ im lặng (Task 1).
- Ô CSV chứa `|`, xuống dòng, hàng ngắn hơn hàng tiêu đề, CSV rỗng (Task 2).
- Phụ đề nhiều dòng, mốc ≥ 100 giờ, phụ đề rỗng (Task 2).

---

### Task 1: `ops/markdown.rs` — `markdown_to_plain` và `escape_plain`

**Files:**
- Create: `crates/morphix-core/src/ops/markdown.rs` (có `#[cfg(test)] mod tests` cho test đơn vị)
- Modify: `crates/morphix-core/src/ops/mod.rs` (thêm `pub mod markdown;`), `crates/morphix-core/Cargo.toml` (qua `cargo add`)

**Interfaces:**
- Produces: `pub fn markdown_to_plain(markdown: &str) -> String`, `pub fn escape_plain(text: &str) -> String`.

- [ ] **Step 1: Thêm phụ thuộc** — chạy `cargo add -p morphix-core pulldown-cmark` (từ thư mục workspace); xác nhận `Cargo.toml` có dòng mới và `cargo check -p morphix-core` qua.
- [ ] **Step 2: Viết test thất bại** trong `markdown.rs::tests` (hàm chưa tồn tại → lỗi biên dịch là RED):
  - `headings_and_emphasis_lose_their_marks`: `"# Tiêu đề\n\nchữ **đậm** và *nghiêng*"` → `"Tiêu đề\n\nchữ đậm và nghiêng\n"`.
  - `links_keep_text_and_drop_url`: `"[Rust](https://rust-lang.org)"` → `"Rust\n"`; ảnh `"![logo](a.png)"` → `"logo\n"`.
  - `code_block_keeps_content_without_fences`: ```` "```rust\nlet x = 1;\n```" ```` → `"let x = 1;\n"`.
  - `lists_become_plain_dashes_and_numbers`: `"- a\n- b\n\n1. c\n2. d"` → `"- a\n- b\n\n1. c\n2. d\n"`.
  - `table_cells_are_tab_separated`: `"| a | b |\n|---|---|\n| 1 | 2 |"` → `"a\tb\n1\t2\n"`.
  - `br_html_becomes_newline_and_other_html_is_kept`: `"a<br>b"` → `"a\nb\n"`; `"<div>giữ</div>"` chứa `"giữ"`.
  - `escaped_text_round_trips`: với `t = "a_b *c* # d\n- e\n1. f\n[g](h) | i\n<j> &amp; ~k~ `l` \\m\n---\n===\n\nđoạn hai"`, assert `markdown_to_plain(&escape_plain(t)).trim_end() == t`.
  - `escape_plain_leaves_plain_words_alone`: `"xin chào"` → `"xin chào"`.
- [ ] **Step 3: Chạy** `cargo test -p morphix-core --lib markdown` → FAIL (chưa có hàm).
- [ ] **Step 4: Cài đặt** hai hàm:
  - `markdown_to_plain` duyệt `pulldown_cmark::Parser::new_ext(…, Options::ENABLE_TABLES | ENABLE_STRIKETHROUGH)`. Quy tắc: `Text`/`Code` giữ nguyên; `SoftBreak`/`HardBreak` → `\n`; `End(Paragraph|Heading|CodeBlock|List|Table)` → kết khối bằng một dòng trống; ô bảng cách nhau `\t`, hàng cách nhau `\n`; mục danh sách ra `- ` hoặc `N. ` (đếm theo `Tag::List(Some(start))`, lồng thì thụt hai khoảng trắng mỗi cấp); `Image` giữ alt (là `Text` bên trong), `Link` giữ chữ bên trong; `Html`/`InlineHtml`: `<br>`/`<br/>`/`<br />` → `\n`, loại khác giữ nguyên văn (WHY: không mất chữ im lặng). Kết quả kết thúc bằng đúng một `\n`; markdown rỗng → chuỗi rỗng.
  - `escape_plain`: thoát bằng `\` các ký tự `\ * _ # > < & [ ] | ~` và backtick ở mọi nơi; đầu dòng thêm: `-`, `+`, `=` (thoát ký tự đó), và chữ số liền `.` hoặc `)` (thoát dấu `.`/`)`). Dòng trống giữ nguyên làm ngắt đoạn; hai dòng không trống liền nhau nối bằng `\` + `\n` (ngắt dòng cứng) để xuống dòng không bị gộp. `shortcut:` dòng thụt ≥ 4 khoảng trắng có thể thành code block, nâng cấp nếu gặp thực tế (ghi comment này vào code).
- [ ] **Step 5: Chạy** `cargo test -p morphix-core --lib markdown` → PASS; nếu `escaped_text_round_trips` lệch, sửa quy tắc thoát hoặc quy tắc `markdown_to_plain`, không sửa dữ liệu test.
- [ ] **Step 6: Commit** `feat(morphix-core): markdown to plain text and escaping`.

---

### Task 2: `ops/markdown.rs` — `table_to_markdown`, `cues_to_markdown`, `pages_to_markdown`

**Files:**
- Modify: `crates/morphix-core/src/ops/markdown.rs`

**Interfaces:**
- Consumes: `escape_plain` (Task 1), `crate::ops::text::Cue`.
- Produces: `pub fn table_to_markdown(rows: &[Vec<String>]) -> String`, `pub fn cues_to_markdown(cues: &[Cue]) -> String`, `pub fn pages_to_markdown(pages: &[String]) -> String`.

- [ ] **Step 1: Viết test thất bại** (trong `tests`):
  - `table_has_header_separator_and_escaped_pipes`: `[["a","b|c"],["1","2"]]` → `"| a | b\\|c |\n| --- | --- |\n| 1 | 2 |\n"`.
  - `short_rows_are_padded_and_multiline_cells_use_br`: `[["a","b"],["x"],["l1\nl2","y"]]` → hàng 2 là `"| x |  |"`, hàng 3 là `"| l1<br>l2 | y |"`.
  - `empty_table_is_empty_string`: `[]` → `""`.
  - `cues_become_bold_time_list`: cue `{start_ms: 1000, text: "Xin chào"}` → `"- **00:00:01** Xin chào\n"`; mốc `360_000_000` ms → `**100:00:00**`.
  - `multiline_cue_uses_br_and_escapes_text`: text `"a*\nb"` → `"- **00:00:00** a\\*<br>b\n"`.
  - `no_cues_gives_empty_string`.
  - `pages_get_numbered_headings`: `["một","hai_"]` → `"## Trang 1\n\nmột\n\n## Trang 2\n\nhai\\_\n"`; trang `""` vẫn có `## Trang N`.
- [ ] **Step 2: Chạy** → FAIL.
- [ ] **Step 3: Cài đặt** ba hàm. Bảng: số cột = số cột lớn nhất; ô qua `escape_plain` rồi đổi `\n` thành `<br>` (WHY: ô một dòng trong bảng MD); `|` được `escape_plain` thoát sẵn. Phụ đề: mốc `HH:MM:SS` từ `start_ms` (giờ không giới hạn 2 chữ số, bỏ mili giây). Trang: chữ qua `escape_plain`, `trim_end`.
- [ ] **Step 4: Chạy** `cargo test -p morphix-core --lib markdown` → PASS.
- [ ] **Step 5: Commit** `feat(morphix-core): markdown writers for tables, cues and pages`.

---

### Task 3: Nối vào catalog và job cho TXT/MD/CSV/TSV/SRT/VTT

**Files:**
- Modify: `crates/morphix-core/src/catalog.rs` (`text_targets` dòng 35-41 + test dòng ~367), `crates/morphix-core/src/job.rs` (`write_converted` dòng 234-247, lý do lỗi dòng 218-223)
- Test: `crates/morphix-core/tests/text.rs` (và cập nhật test cũ nào khẳng định danh sách đích/ MD→TXT chép nguyên, ví dụ quanh dòng 341)

**Interfaces:**
- Consumes: `markdown_to_plain`, `escape_plain`, `table_to_markdown`, `cues_to_markdown` (Task 1-2).
- Produces: `text_targets` mới: `Txt|Markdown → [Txt, Markdown]`, `Srt|Vtt → [Srt, Vtt, Txt, Markdown]`, `Csv|Tsv → [Csv, Tsv, Markdown]`.

- [ ] **Step 1: Viết test thất bại** trong `tests/text.rs` (theo phong cách file: ghi file vào `tempdir`, gọi `run_job` với `Operation::ConvertText`, đọc kết quả):
  - `markdown_to_txt_strips_syntax`: `# T\n\n**b**` → file `.txt` bằng `"T\n\nb\n"`.
  - `txt_to_markdown_escapes_special_characters`: `a_b *c*` → `a\_b \*c\*`.
  - `markdown_to_markdown_and_txt_to_txt_stay_verbatim`: chép nguyên.
  - `csv_to_markdown_makes_a_table` và `tsv_to_markdown_makes_a_table`: `a,b\n1,2` → `| a | b |\n| --- | --- |\n| 1 | 2 |\n`.
  - `srt_to_markdown_lists_start_times` và `vtt_to_markdown_lists_start_times`.
  - `txt_to_csv_and_markdown_to_srt_are_still_rejected`: `Err(InvalidInput)`.
  - Sửa test catalog: `text_targets(Csv)` chứa `Markdown`; `text_pair_supported(Srt, Markdown)` true; `(Markdown, Csv)` false; `(Txt, Srt)` false.
- [ ] **Step 2: Chạy** `cargo test -p morphix-core --test text --lib catalog` → FAIL.
- [ ] **Step 3: Cài đặt:** sửa `text_targets`; trong `write_converted` rẽ theo `(from, to)`: Markdown→Txt dùng `markdown_to_plain`; Txt→Markdown dùng `escape_plain`; cặp cùng loại chép nguyên; Srt|Vtt→Markdown dùng `cues_to_markdown`; Csv|Tsv→Markdown dùng `table_to_markdown(&read_table(…)?)`. Các cặp còn lại giữ nhánh cũ. Cập nhật câu lỗi cặp không hợp lệ cho đúng (giữ lý do "không có mốc thời gian" cho TXT/MD→phụ đề).
- [ ] **Step 4: Chạy** lại → PASS; chạy `cargo test -p morphix-core --no-fail-fast` đối chiếu baseline.
- [ ] **Step 5: Commit** `feat(morphix-core): markdown conversions for text, tables and subtitles`.

---

### Task 4: PDF → MD

**Files:**
- Modify: `crates/morphix-core/src/ops/pdf_text.rs` (tách `pub fn pdf_pages_text(path: &Path) -> Result<Vec<String>>` chứa vòng lặp đọc trang và lỗi trang rỗng, `pdf_to_text` gọi lại; thêm `pub fn pdf_to_markdown(path: &Path, out: &mut impl Write) -> Result<()>` dùng `pages_to_markdown`), `crates/morphix-core/src/job.rs` (`Operation::PdfToMarkdown` + doc comment, `run_item` dòng ~168, hàm `pdf_to_md` như `pdf_to_txt`, `planned_outputs` dòng ~405 với đuôi `md`), `crates/morphix-core/src/catalog.rs` (`ActionKind::PdfToMarkdown`, `action_id` → `"pdf-to-markdown"`, `type_actions` dòng ~126 trả hai việc `PdfToText` "TXT" và `PdfToMarkdown` "MD" cho một PDF), `crates/morphix-core/src/lib.rs` nếu cần xuất hàm
- Test: `crates/morphix-core/tests/pdf_text.rs` (nhóm cần PDFium, giữ phong cách file), `crates/morphix-core/tests/job.rs` (planned_outputs), unit test catalog

**Interfaces:**
- Consumes: `pages_to_markdown` (Task 2).
- Produces: `Operation::PdfToMarkdown`, `ActionKind::PdfToMarkdown`, id `"pdf-to-markdown"`.

- [ ] **Step 1: Viết test thất bại:**
  - Catalog: một PDF → hai việc, `kind` lần lượt `PdfToText`, `PdfToMarkdown`; nhiều PDF vẫn không có việc; `action_id(PdfToMarkdown) == "pdf-to-markdown"`.
  - `planned_outputs` của job `PdfToMarkdown` trên `a.pdf` → `a.md`.
  - `tests/pdf_text.rs`: `pdf_to_markdown` trên PDF mẫu có sẵn của file ra `## Trang 1` rồi chữ; PDF trang không có chữ → cùng lỗi `Corrupt` như `pdf_to_text` và `out` rỗng. (Các test này fail trên máy thiếu PDFium như 9 test cũ; ghi rõ trong ledger.)
- [ ] **Step 2: Chạy** → FAIL (biên dịch).
- [ ] **Step 3: Cài đặt** như phần Files. Sửa mọi `match` đầy đủ trên `Operation`/`ActionKind` mà trình biên dịch báo (theo mẫu `PdfToText`).
- [ ] **Step 4: Chạy** `cargo test -p morphix-core --no-fail-fast` → test không cần PDFium PASS, fail PDFium khớp baseline.
- [ ] **Step 5: Commit** `feat(morphix-core): PDF to Markdown`.

---

### Task 5: CLI, menu GUI và chuỗi UI

**Files:**
- Modify: `crates/morphix-cli/src/main.rs` (dòng ~306: PDF đầu vào + `Markdown` → `Operation::PdfToMarkdown`; dòng ~484 và ~496: thêm nhánh `PdfToMarkdown` giống `PdfToText`, thông báo "chỉ PDF mới chuyển được sang MD bằng việc này"), `crates/morphix-gui/src/menu_items.rs` (`"pdf-to-markdown" => Ok(Operation::PdfToMarkdown)` + doc comment), `crates/morphix-gui/src/commands.rs` (`operation_name`: `"Đổi sang MD"`)
- Test: `crates/morphix-gui/tests/menu_items.rs`, `crates/morphix-cli/tests/cli.rs`

**Interfaces:**
- Consumes: `Operation::PdfToMarkdown`, id `"pdf-to-markdown"` (Task 4); `text_pair_supported` mới (Task 3).

- [ ] **Step 1: Viết test thất bại:**
  - `menu_items.rs`: `operation_for("pdf-to-markdown", …) == Operation::PdfToMarkdown`; mục menu của một PDF gồm cả `pdf-to-text` và `pdf-to-markdown`; `operation_for("text:md", …) == ConvertText { to: Markdown }` (đã có thì giữ).
  - `cli.rs`: `convert --to md` với CSV tạo `.md` có bảng; `--to txt` với `.md` bỏ ký hiệu; `--to md` với PNG báo lỗi cặp không hỗ trợ (mã thoát như các lỗi hiện có). Test PDF→MD CLI cần PDFium nên thuộc nhóm fail sẵn.
- [ ] **Step 2: Chạy** `cargo test -p morphix-gui --test menu_items -p morphix-cli --test cli` → FAIL.
- [ ] **Step 3: Cài đặt** như phần Files, sửa mọi `match` đầy đủ mà trình biên dịch báo.
- [ ] **Step 4: Kiểm tra cuối** theo thứ tự: rustfmt các file đã chạm; `cargo clippy --workspace --all-targets -- -D warnings` sạch; `cargo test --workspace --no-fail-fast` — tổng fail ≤ 36 + số test PDFium mới thêm (liệt kê riêng); `cargo check --target x86_64-pc-windows-gnu -p morphix-cli -p morphix-gui` nếu target có (không có thì ghi chưa kiểm).
- [ ] **Step 5: Commit** `feat(morphix-cli): markdown targets in CLI and menus` (kèm GUI trong cùng commit hoặc `feat(morphix-gui)` riêng).
