//! Markdown: bỏ ký hiệu khi ra văn bản thuần, thêm ký hiệu khi từ định dạng khác vào. Chỉ làm việc
//! trên chuỗi, không đụng file.

use std::{collections::HashMap, path::Path};

use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};

use crate::ops::{
    doc::{
        grid, load_image_src, plain_blocks, plain_inlines, Block, Cell, Document, Dropped, Image,
        Inline, Slot, Style,
    },
    html::{read_blocks as read_html_blocks, tidy},
    text::Cue,
};

/// Ký tự có nghĩa trong Markdown ở bất kỳ đâu trên dòng; văn bản thuần phải thoát để giữ nguyên chữ.
const SPECIAL: &[char] = &['\\', '*', '_', '#', '>', '<', '&', '[', ']', '|', '~', '`'];

/// Bỏ ký hiệu Markdown, giữ chữ. Giữa các khối có một dòng trống; kết quả kết thúc bằng đúng một `\n`.
pub fn markdown_to_plain(markdown: &str) -> String {
    let mut out = String::new();
    // WHY: mỗi cấp danh sách nhớ số thứ tự kế tiếp (`None` là danh sách gạch đầu dòng).
    let mut lists: Vec<Option<u64>> = Vec::new();
    // Mục danh sách đang mở: (thụt lề của phần nội dung, đã có nội dung chưa).
    let mut items: Vec<(String, bool)> = Vec::new();
    let mut code_indent: Option<String> = None;
    let mut in_table = false;
    let mut cell = 0usize;
    // WHY: bật footnote để `[^1]: ghi chú` không bị hiểu là định nghĩa link rồi bỏ mất chữ.
    let options =
        Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_FOOTNOTES;
    for event in Parser::new_ext(markdown, options) {
        match event {
            Event::Text(text) if code_indent.is_some() => {
                let indent = code_indent.as_deref().unwrap_or_default();
                for line in text.split_inclusive('\n') {
                    out.push_str(indent);
                    out.push_str(line);
                }
            }
            Event::Text(text) | Event::Code(text) => out.push_str(&text),
            Event::FootnoteReference(label) => out.push_str(&format!("[{label}]")),
            Event::Start(Tag::FootnoteDefinition(label)) => {
                start_line(&mut out);
                out.push_str(&format!("[{label}]: "));
            }
            // WHY: một hàng bảng là một dòng chữ (các ô cách nhau tab); xuống dòng trong ô làm vỡ hàng.
            Event::SoftBreak | Event::HardBreak if in_table => out.push(' '),
            Event::Html(html) | Event::InlineHtml(html) if in_table && is_br(&html) => {
                out.push(' ')
            }
            Event::SoftBreak | Event::HardBreak => out.push('\n'),
            Event::Html(html) | Event::InlineHtml(html) => {
                if is_br(&html) {
                    out.push('\n');
                } else {
                    // WHY: HTML thô có thể chứa chữ; bỏ đi là mất nội dung âm thầm.
                    out.push_str(&html);
                }
            }
            Event::Start(Tag::List(start)) => {
                start_line(&mut out);
                lists.push(start);
            }
            Event::End(TagEnd::List(_)) => {
                lists.pop();
                if lists.is_empty() {
                    end_block(&mut out);
                }
            }
            Event::Start(Tag::Item) => {
                start_line(&mut out);
                let indent = "  ".repeat(lists.len().saturating_sub(1));
                let marker = match lists.last_mut() {
                    Some(Some(number)) => {
                        let marker = format!("{number}. ");
                        *number += 1;
                        marker
                    }
                    _ => "- ".to_string(),
                };
                out.push_str(&indent);
                out.push_str(&marker);
                items.push((format!("{indent}{}", " ".repeat(marker.len())), false));
            }
            Event::End(TagEnd::Item) => {
                items.pop();
                start_line(&mut out);
            }
            // WHY: đoạn thứ hai trở đi của một mục thụt theo nội dung mục, không dính ra lề.
            Event::Start(Tag::Paragraph) if items.last().is_some_and(|item| item.1) => {
                start_line(&mut out);
                out.push_str(&items.last().map(|item| item.0.clone()).unwrap_or_default());
            }
            Event::End(TagEnd::Paragraph) if !lists.is_empty() => {
                if let Some(item) = items.last_mut() {
                    item.1 = true;
                }
                start_line(&mut out);
            }
            Event::Start(Tag::CodeBlock(_)) if !items.is_empty() => {
                start_line(&mut out);
                code_indent = items.last().map(|item| item.0.clone());
            }
            Event::End(TagEnd::CodeBlock) if code_indent.is_some() => {
                code_indent = None;
                if let Some(item) = items.last_mut() {
                    item.1 = true;
                }
                start_line(&mut out);
            }
            Event::Start(Tag::Table(_)) => in_table = true,
            Event::End(TagEnd::Table) => {
                in_table = false;
                end_block(&mut out);
            }
            Event::Start(Tag::TableHead | Tag::TableRow) => cell = 0,
            Event::Start(Tag::TableCell) => {
                if cell > 0 {
                    out.push('\t');
                }
                cell += 1;
            }
            Event::End(TagEnd::TableHead | TagEnd::TableRow) => out.push('\n'),
            Event::End(
                TagEnd::Paragraph
                | TagEnd::Heading(_)
                | TagEnd::CodeBlock
                | TagEnd::BlockQuote(_)
                | TagEnd::HtmlBlock,
            )
            | Event::Rule => end_block(&mut out),
            _ => {}
        }
    }
    let kept = out.trim_end_matches('\n').len();
    out.truncate(kept);
    if !out.is_empty() {
        out.push('\n');
    }
    out
}

/// Thoát văn bản thuần để Markdown hiển thị đúng chữ gốc. Dòng liền nhau nối bằng ngắt dòng cứng
/// (`\` cuối dòng) để không bị gộp thành một dòng; dòng trống giữ làm ngắt đoạn.
/// WHY: Markdown gộp nhiều dòng trống thành một và bỏ dòng trống đầu file, nên mỗi dòng trống thừa
/// ghi thành một khối `<br>` (đọc lại ra đúng một dòng trống). Giữ kiểu xuống dòng CRLF của nguồn.
/// shortcut: dòng trống ở cuối file vẫn gộp lại, nâng cấp nếu người dùng cần giữ.
pub fn escape_plain(text: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let mut out = String::with_capacity(text.len() + text.len() / 8);
    let mut seen_text = false;
    let mut blank_run = 0usize;
    for (index, line) in lines.iter().enumerate() {
        if line.is_empty() {
            blank_run += 1;
            // Dòng trống đầu tiên giữa hai đoạn là ngắt đoạn; các dòng sau (và dòng trống đầu file) là `<br>`.
            if !seen_text || blank_run > 1 {
                let next_is_text = lines[index + 1..].iter().any(|l| !l.is_empty());
                if next_is_text {
                    out.push_str("<br>\n\n");
                }
            } else {
                out.push('\n');
            }
            continue;
        }
        seen_text = true;
        blank_run = 0;
        out.push_str(&escape_line(line));
        if lines.get(index + 1).is_some_and(|l| !l.is_empty()) {
            out.push('\\');
        }
        if index + 1 < lines.len() {
            out.push('\n');
        }
    }
    if text.ends_with('\n') && !out.ends_with('\n') {
        out.push('\n');
    }
    if text.contains("\r\n") {
        out = out.replace('\n', "\r\n");
    }
    out
}

/// Bảng Markdown: hàng đầu làm tiêu đề, hàng ngắn được độn ô trống. Bảng rỗng ra chuỗi rỗng.
pub fn table_to_markdown(rows: &[Vec<String>]) -> String {
    let columns = rows.iter().map(Vec::len).max().unwrap_or(0);
    if columns == 0 {
        return String::new();
    }
    let mut out = String::new();
    for (index, row) in rows.iter().enumerate() {
        let cells: Vec<String> = (0..columns)
            .map(|c| row.get(c).map_or(String::new(), |cell| inline(cell)))
            .collect();
        out.push_str(&format!("| {} |\n", cells.join(" | ")));
        if index == 0 {
            out.push_str(&format!("|{}\n", " --- |".repeat(columns)));
        }
    }
    out
}

/// Phụ đề thành danh sách `- **HH:MM:SS** nội dung`, chỉ giữ mốc bắt đầu.
pub fn cues_to_markdown(cues: &[Cue]) -> String {
    cues.iter()
        .map(|cue| {
            let seconds = cue.start_ms / 1000;
            format!(
                "- **{:02}:{:02}:{:02}** {}\n",
                seconds / 3600,
                seconds / 60 % 60,
                seconds % 60,
                inline(&cue.text)
            )
        })
        .collect()
}

/// Mỗi trang PDF một mục `## Trang N`; không đoán tiêu đề hay danh sách trong trang.
pub fn pages_to_markdown(pages: &[String]) -> String {
    let mut out = String::new();
    for (index, page) in pages.iter().enumerate() {
        out.push_str(&format!("## Trang {}\n\n", index + 1));
        let body = escape_plain(page.trim_end());
        if !body.is_empty() {
            out.push_str(&body);
            out.push_str("\n\n");
        }
    }
    let kept = out.trim_end_matches('\n').len();
    out.truncate(kept);
    if !out.is_empty() {
        out.push('\n');
    }
    out
}

/// Markdown của một cây tài liệu, kèm số thứ Markdown không chứa được. WHY: ảnh (chỉ giữ chữ thay thế)
/// được đếm ở đây, không ở bộ đọc, để mỗi ảnh chỉ đếm một lần dù nguồn là HTML hay DOCX.
pub fn write(doc: &Document) -> (String, Dropped) {
    let mut writer = MdWriter::default();
    let mut blocks: Vec<String> = doc
        .blocks
        .iter()
        .map(|block| writer.block(block, 0))
        .filter(|text| !text.is_empty())
        .collect();
    for (index, notes) in doc.footnotes.iter().enumerate() {
        let body = notes
            .iter()
            .map(|block| writer.block(block, 0))
            .filter(|text| !text.is_empty())
            .collect::<Vec<_>>()
            .join("\n\n");
        // WHY: dòng nối tiếp của chú thích thụt 4 khoảng trắng, nếu không chúng thoát khỏi định nghĩa.
        let mut lines = body.split('\n');
        let mut note = format!("[^{}]: {}", index + 1, lines.next().unwrap_or_default());
        for line in lines {
            note.push('\n');
            if !line.is_empty() {
                note.push_str("    ");
                note.push_str(line);
            }
        }
        blocks.push(note);
    }
    let mut markdown = blocks.join("\n\n");
    if !markdown.is_empty() {
        markdown.push('\n');
    }
    let mut dropped = doc.dropped;
    dropped.add(writer.dropped);
    (markdown, dropped)
}

#[derive(Default)]
struct MdWriter {
    dropped: Dropped,
}

impl MdWriter {
    /// Một khối; `depth` là cấp thụt của danh sách bao ngoài (0 = ngoài cùng).
    fn block(&mut self, block: &Block, depth: usize) -> String {
        match block {
            Block::Heading(level, inlines) => {
                let text = self.inline(inlines);
                let text = text.trim();
                if text.is_empty() {
                    String::new()
                } else {
                    format!("{} {text}", "#".repeat(usize::from(*level)))
                }
            }
            Block::Paragraph(inlines) => escape_lines_start(self.inline(inlines).trim()),
            Block::List { start, items } => self.list(*start, items, depth),
            Block::Table(rows) => self.table(rows),
            Block::Code(code) if code.is_empty() => String::new(),
            Block::Code(code) => code_fence(code),
            Block::Quote(blocks) => {
                let inner = blocks
                    .iter()
                    .map(|block| self.block(block, depth))
                    .filter(|text| !text.is_empty())
                    .collect::<Vec<_>>()
                    .join("\n\n");
                inner
                    .lines()
                    .map(|line| {
                        if line.is_empty() {
                            ">".to_string()
                        } else {
                            format!("> {line}")
                        }
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            }
            Block::Rule => "---".to_string(),
        }
    }

    /// Mỗi cấp thụt 4 khoảng trắng: CommonMark cần thụt bằng độ rộng dấu mục cha (`1. ` là 3).
    fn list(&mut self, start: Option<u64>, items: &[Vec<Block>], depth: usize) -> String {
        let indent = "    ".repeat(depth);
        let mut lines: Vec<String> = Vec::new();
        for (index, item) in items.iter().enumerate() {
            let marker = match start {
                Some(n) => format!("{}. ", n + index as u64),
                None => "- ".to_string(),
            };
            let mut row: Vec<String> = Vec::new();
            for block in item {
                let nested = matches!(block, Block::List { .. });
                let text = self.block(block, depth + 1);
                if text.is_empty() {
                    continue;
                }
                if !row.is_empty() && !nested {
                    row.push(String::new());
                }
                for line in text.split('\n') {
                    // WHY: dòng sau của khối trong mục thụt thêm để vẫn thuộc mục; danh sách con đã có
                    // thụt riêng; dòng đầu của mục nằm cạnh dấu mục.
                    let keep = nested || line.is_empty() || row.is_empty();
                    row.push(if keep {
                        line.to_string()
                    } else {
                        format!("    {line}")
                    });
                }
            }
            let starts_with_list = matches!(item.first(), Some(Block::List { .. }));
            if row.is_empty() || starts_with_list {
                lines.push(format!("{indent}{}", marker.trim_end()));
                lines.extend(row);
            } else {
                row[0] = format!("{indent}{marker}{}", row[0]);
                lines.extend(row);
            }
        }
        lines.join("\n")
    }

    /// Bảng theo lưới: ô gộp không biểu diễn được trong Markdown nên chỉ giữ ô gốc và báo hạ cấp.
    fn table(&mut self, rows: &[Vec<Cell>]) -> String {
        if rows
            .iter()
            .flatten()
            .any(|cell| cell.colspan > 1 || cell.rowspan > 1)
        {
            self.dropped.md_downgraded = true;
        }
        let text: Vec<Vec<String>> = grid(rows)
            .iter()
            .map(|line| {
                line.iter()
                    .map(|slot| match slot {
                        Slot::Cell(cell) => plain_blocks(&cell.blocks),
                        _ => String::new(),
                    })
                    .collect()
            })
            .collect();
        table_to_markdown(&text).trim_end().to_string()
    }

    fn inline(&mut self, inlines: &[Inline]) -> String {
        let mut out = String::new();
        let mut index = 0;
        while index < inlines.len() {
            match &inlines[index] {
                Inline::Text(_, style) => {
                    // WHY: mẩu liền nhau cùng định dạng được gộp trước khi bọc ký hiệu.
                    let mut run = String::new();
                    while let Some(Inline::Text(next, next_style)) = inlines.get(index) {
                        if next_style != style {
                            break;
                        }
                        run.push_str(next);
                        index += 1;
                    }
                    out.push_str(&self.wrap(&run, *style));
                }
                other => {
                    out.push_str(&self.single(other));
                    index += 1;
                }
            }
        }
        out
    }

    fn single(&mut self, inline: &Inline) -> String {
        match inline {
            Inline::Text(text, _) => escape_specials(text),
            Inline::Link(url, inner) => {
                let text = self.inline(inner);
                if url.is_empty() || text.trim().is_empty() {
                    text
                } else {
                    format!("[{text}]({})", link_destination(url))
                }
            }
            Inline::Image { alt, .. } => {
                self.dropped.images += 1;
                escape_specials(alt)
            }
            Inline::FootnoteRef(index) => format!("[^{}]", index + 1),
            Inline::Break => "\\\n".to_string(),
        }
    }

    /// Bọc ký hiệu quanh chữ. WHY: khoảng trắng đầu/cuối đặt ra ngoài ký hiệu, vì `** a**` không còn là
    /// chữ đậm trong Markdown.
    fn wrap(&mut self, text: &str, style: Style) -> String {
        let body = text.trim_matches(' ');
        if body.is_empty() {
            return escape_specials(text);
        }
        if style.underline || style.sup || style.sub {
            self.dropped.md_downgraded = true;
        }
        let start = &text[..text.len() - text.trim_start_matches(' ').len()];
        let end = &text[text.trim_end_matches(' ').len()..];
        let middle = if style.code {
            code_span(body)
        } else {
            escape_specials(body)
        };
        // WHY: khoảng trắng mép đi cùng vào `wrap_marks`, nó tự đưa ra ngoài ký hiệu. Cắt trước thì dấu câu
        // cuối (`**Note:** text`) bị đẩy ra sau ký hiệu thành `**Note**:`.
        let mut inner = format!("{start}{middle}{end}");
        for (on, mark) in [
            (style.strike, "~~"),
            (style.italic, "*"),
            (style.bold, "**"),
        ] {
            if on {
                inner = wrap_marks(&inner, mark);
            }
        }
        inner
    }
}

/// Chữ nhiều dòng nằm trên một dòng Markdown (ô bảng, mục danh sách): thoát từng dòng, nối `<br>`.
fn inline(text: &str) -> String {
    text.lines()
        .map(escape_line)
        .collect::<Vec<_>>()
        .join("<br>")
}

fn escape_line(line: &str) -> String {
    let body = line.trim_start();
    let lead = &line[..line.len() - body.len()];
    let trail_start = body.trim_end().len();
    let (body, trail) = body.split_at(trail_start);
    let mut out = String::with_capacity(line.len() + 4);
    // WHY: khoảng trắng đầu/cuối dòng ghi bằng mã ký tự: Markdown bỏ khoảng trắng thật ở hai đầu,
    // và dòng thụt ≥ 4 sẽ thành code block làm lộ dấu `\` thoát.
    let entities = |out: &mut String, spaces: &str| {
        for ch in spaces.chars() {
            out.push_str(&format!("&#{};", u32::from(ch)));
        }
    };
    entities(&mut out, lead);
    out.push_str(&escape_block_start(&escape_specials(body)));
    entities(&mut out, trail);
    out
}

/// Đích link an toàn: có khoảng trắng hay ngoặc thì bọc `<…>` để Markdown không cắt link giữa chừng.
pub(crate) fn link_destination(url: &str) -> String {
    if url.contains(|c: char| c.is_whitespace() || matches!(c, '(' | ')' | '<' | '>')) {
        format!("<{}>", url.replace('<', "%3C").replace('>', "%3E"))
    } else {
        url.to_string()
    }
}

/// Thoát ký tự có nghĩa ở bất kỳ đâu trên dòng (không xét đầu dòng).
pub(crate) fn escape_specials(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 4);
    for ch in text.chars() {
        if SPECIAL.contains(&ch) {
            out.push('\\');
        }
        out.push(ch);
    }
    out
}

/// Thoát phần đầu dòng của chữ đã qua `escape_specials`. WHY: `-`, `+`, `=` đầu dòng mở danh sách
/// hoặc gạch tiêu đề; `1.`/`1)` mở danh sách số.
pub(crate) fn escape_block_start(text: &str) -> String {
    if text.starts_with(['-', '+', '=']) {
        return format!("\\{text}");
    }
    let digits = text.len() - text.trim_start_matches(|c: char| c.is_ascii_digit()).len();
    if digits > 0 && text[digits..].starts_with(['.', ')']) {
        return format!("{}\\{}", &text[..digits], &text[digits..]);
    }
    text.to_string()
}

/// `escape_block_start` cho từng dòng. WHY: dòng sau ngắt dòng cứng bắt đầu bằng `- ` hay `===`
/// cũng đổi cấu trúc (thành danh sách, tiêu đề) như dòng đầu.
pub(crate) fn escape_lines_start(text: &str) -> String {
    text.split('\n')
        .map(escape_block_start)
        .collect::<Vec<_>>()
        .join("\n")
}

/// Bọc ký hiệu nhấn mạnh quanh Markdown inline. WHY: khoảng trắng đầu/cuối đặt ra ngoài, vì
/// `**Note: **` không còn là chữ đậm trong Markdown mà hiện nguyên dấu `*`.
pub(crate) fn wrap_marks(inner: &str, mark: &str) -> String {
    let body = inner.trim_matches(' ');
    if body.is_empty() {
        return inner.to_string();
    }
    let start = &inner[..inner.len() - inner.trim_start_matches(' ').len()];
    let end = &inner[inner.trim_end_matches(' ').len()..];
    // WHY: `**(a)**b` không thành chữ đậm (quy tắc "flanking" của CommonMark) khi dấu câu sát ký hiệu
    // và phía ngoài là chữ. Không có khoảng trắng ở mép thì đưa dấu câu ra ngoài ký hiệu.
    let (head, body) = if start.is_empty() {
        split_punctuation_start(body)
    } else {
        ("", body)
    };
    let (body, tail) = if end.is_empty() {
        split_punctuation_end(body)
    } else {
        (body, "")
    };
    if body.is_empty() {
        return inner.to_string();
    }
    format!("{start}{head}{mark}{body}{mark}{tail}{end}")
}

/// Dấu câu đưa được ra ngoài ký hiệu: không phải ký hiệu Markdown chưa thoát, không phải phần link/code.
fn movable(ch: char) -> bool {
    ch.is_ascii_punctuation() && !matches!(ch, '*' | '_' | '~' | '`' | '[' | ']' | '\\')
}

fn split_punctuation_start(body: &str) -> (&str, &str) {
    let bytes = body.as_bytes();
    let mut at = 0;
    loop {
        match bytes.get(at) {
            // `\X` là một ký tự đã thoát: tách cả cặp.
            Some(b'\\') if bytes.get(at + 1).is_some_and(|b| b.is_ascii_punctuation()) => at += 2,
            Some(&b) if movable(char::from(b)) => at += 1,
            _ => break,
        }
    }
    body.split_at(at)
}

fn split_punctuation_end(body: &str) -> (&str, &str) {
    // WHY: `)` cuối của link `[chữ](url)` là một phần của link, không được tách.
    if body.contains("](") || body.ends_with('`') {
        return (body, "");
    }
    let bytes = body.as_bytes();
    let mut at = bytes.len();
    while at > 0 {
        let b = bytes[at - 1];
        if at >= 2 && bytes[at - 2] == b'\\' && b.is_ascii_punctuation() {
            at -= 2;
        } else if movable(char::from(b)) {
            at -= 1;
        } else {
            break;
        }
    }
    body.split_at(at)
}

/// Độ dài chuỗi backtick liền nhau dài nhất trong `text`.
fn longest_backtick_run(text: &str) -> usize {
    text.split(|c| c != '`').map(str::len).max().unwrap_or(0)
}

/// Code inline an toàn với backtick bên trong; code rỗng ra chuỗi rỗng.
pub(crate) fn code_span(code: &str) -> String {
    if code.is_empty() {
        return String::new();
    }
    let fence = "`".repeat(longest_backtick_run(code) + 1);
    let pad = if code.starts_with('`') || code.ends_with('`') {
        " "
    } else {
        ""
    };
    format!("{fence}{pad}{code}{pad}{fence}")
}

/// Khối code có rào dài hơn mọi chuỗi backtick bên trong để nội dung không đóng rào sớm.
pub(crate) fn code_fence(code: &str) -> String {
    let fence = "`".repeat((longest_backtick_run(code) + 1).max(3));
    format!("{fence}\n{code}\n{fence}")
}

/// Cây tài liệu từ Markdown. Ký hiệu chuẩn được đổi thành định dạng; HTML trong Markdown được đọc như HTML.
pub fn read(markdown: &str, base: Option<&Path>) -> Document {
    let options =
        Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_FOOTNOTES;
    let mut reader = MdReader::new(base);
    for event in Parser::new_ext(markdown, options) {
        reader.event(event);
    }
    reader.finish()
}

/// Link hoặc ảnh đang mở: vị trí mẩu đầu tiên của nó trong `inlines`.
enum Open {
    Link(usize, String),
    Image(usize, String),
}

struct MdReader<'a> {
    base: Option<&'a Path>,
    images: Vec<Image>,
    /// Khối đang gom theo vùng: thân tài liệu, mục danh sách, trích dẫn, ô bảng. Luôn có ít nhất một.
    containers: Vec<Vec<Block>>,
    /// Danh sách đang mở: (số bắt đầu, các mục đã đóng).
    lists: Vec<(Option<u64>, Vec<Vec<Block>>)>,
    /// Bảng đang mở: các hàng đã đóng.
    tables: Vec<Vec<Vec<Cell>>>,
    in_head: bool,
    /// Chữ của đoạn, tiêu đề hoặc ô đang đọc.
    inlines: Vec<Inline>,
    heading: Option<u8>,
    /// Nội dung khối code đang đọc.
    code: Option<String>,
    style: Style,
    style_stack: Vec<Style>,
    open: Vec<Open>,
    /// Nhãn chú thích theo thứ tự tham chiếu đầu tiên; chỉ số = vị trí trong `footnotes`.
    labels: Vec<String>,
    definitions: HashMap<String, Vec<Block>>,
    defs_open: Vec<String>,
}

impl<'a> MdReader<'a> {
    fn new(base: Option<&'a Path>) -> Self {
        Self {
            base,
            images: Vec::new(),
            containers: vec![Vec::new()],
            lists: Vec::new(),
            tables: Vec::new(),
            in_head: false,
            inlines: Vec::new(),
            heading: None,
            code: None,
            style: Style::default(),
            style_stack: Vec::new(),
            open: Vec::new(),
            labels: Vec::new(),
            definitions: HashMap::new(),
            defs_open: Vec::new(),
        }
    }

    fn event(&mut self, event: Event<'_>) {
        match event {
            Event::Start(tag) => self.start(tag),
            Event::End(tag) => self.end(tag),
            Event::Text(text) => match self.code.as_mut() {
                Some(code) => code.push_str(&text),
                None => self
                    .inlines
                    .push(Inline::Text(text.to_string(), self.style)),
            },
            Event::Code(text) => self.inlines.push(Inline::Text(
                text.to_string(),
                Style {
                    code: true,
                    ..self.style
                },
            )),
            Event::SoftBreak => self.inlines.push(Inline::Text(" ".to_string(), self.style)),
            Event::HardBreak => self.inlines.push(Inline::Break),
            Event::InlineHtml(html) if is_br(&html) => self.inlines.push(Inline::Break),
            Event::InlineHtml(_) => {}
            Event::Html(html) => {
                self.flush();
                let blocks = read_html_blocks(&html, self.base, &mut self.images);
                self.top().extend(blocks);
            }
            Event::FootnoteReference(label) => {
                let index = match self
                    .labels
                    .iter()
                    .position(|known| known.as_str() == &*label)
                {
                    Some(index) => index,
                    None => {
                        self.labels.push(label.to_string());
                        self.labels.len() - 1
                    }
                };
                self.inlines.push(Inline::FootnoteRef(index));
            }
            Event::Rule => {
                self.flush();
                self.top().push(Block::Rule);
            }
            _ => {}
        }
    }

    fn start(&mut self, tag: Tag<'_>) {
        match tag {
            // WHY: không flush ở đây: chữ của FootnoteDefinition đứng trước đoạn đầu tiên của nó.
            Tag::Paragraph => {}
            Tag::Heading { level, .. } => {
                self.flush();
                self.heading = Some(level as u8);
            }
            Tag::BlockQuote(_) => {
                self.flush();
                self.containers.push(Vec::new());
            }
            Tag::CodeBlock(_) => {
                self.flush();
                self.code = Some(String::new());
            }
            Tag::List(start) => {
                self.flush();
                self.lists.push((start, Vec::new()));
            }
            Tag::Item => {
                self.flush();
                self.containers.push(Vec::new());
            }
            Tag::Table(_) => {
                self.flush();
                self.tables.push(Vec::new());
            }
            Tag::TableHead => {
                self.in_head = true;
                if let Some(rows) = self.tables.last_mut() {
                    rows.push(Vec::new());
                }
            }
            Tag::TableRow => {
                if let Some(rows) = self.tables.last_mut() {
                    rows.push(Vec::new());
                }
            }
            Tag::Emphasis => self.push_style(Style {
                italic: true,
                ..self.style
            }),
            Tag::Strong => self.push_style(Style {
                bold: true,
                ..self.style
            }),
            Tag::Strikethrough => self.push_style(Style {
                strike: true,
                ..self.style
            }),
            Tag::Link { dest_url, .. } => self
                .open
                .push(Open::Link(self.inlines.len(), dest_url.to_string())),
            Tag::Image { dest_url, .. } => self
                .open
                .push(Open::Image(self.inlines.len(), dest_url.to_string())),
            Tag::FootnoteDefinition(label) => {
                self.flush();
                self.containers.push(Vec::new());
                self.defs_open.push(label.to_string());
            }
            _ => {}
        }
    }

    fn end(&mut self, tag: TagEnd) {
        match tag {
            TagEnd::Paragraph => self.flush(),
            TagEnd::FootnoteDefinition => {
                self.flush();
                let blocks = self.containers.pop().unwrap_or_default();
                if let Some(label) = self.defs_open.pop() {
                    self.definitions.insert(label, blocks);
                }
            }
            TagEnd::Heading(_) => {
                let inlines = tidy(std::mem::take(&mut self.inlines));
                let level = self.heading.take().unwrap_or(1);
                if !inlines.is_empty() {
                    self.top().push(Block::Heading(level, inlines));
                }
            }
            TagEnd::BlockQuote(_) => {
                self.flush();
                let blocks = self.containers.pop().unwrap_or_default();
                if !blocks.is_empty() {
                    self.top().push(Block::Quote(blocks));
                }
            }
            TagEnd::CodeBlock => {
                if let Some(code) = self.code.take() {
                    self.top()
                        .push(Block::Code(code.trim_end_matches('\n').to_string()));
                }
            }
            TagEnd::List(_) => {
                self.flush();
                if let Some((start, items)) = self.lists.pop() {
                    self.top().push(Block::List { start, items });
                }
            }
            TagEnd::Item => {
                self.flush();
                let blocks = self.containers.pop().unwrap_or_default();
                if let Some((_, items)) = self.lists.last_mut() {
                    items.push(blocks);
                }
            }
            TagEnd::Table => {
                self.flush();
                if let Some(rows) = self.tables.pop() {
                    self.top().push(Block::Table(rows));
                }
            }
            TagEnd::TableHead => self.in_head = false,
            TagEnd::TableCell => {
                let inlines = tidy(std::mem::take(&mut self.inlines));
                let blocks = if inlines.is_empty() {
                    Vec::new()
                } else {
                    vec![Block::Paragraph(inlines)]
                };
                let header = self.in_head;
                if let Some(row) = self.tables.last_mut().and_then(|rows| rows.last_mut()) {
                    row.push(Cell {
                        blocks,
                        colspan: 1,
                        rowspan: 1,
                        header,
                    });
                }
            }
            TagEnd::Emphasis | TagEnd::Strong | TagEnd::Strikethrough => {
                self.style = self.style_stack.pop().unwrap_or_default();
            }
            TagEnd::Link => {
                if let Some(Open::Link(start, url)) = self.open.pop() {
                    let inner: Vec<Inline> = self.inlines.drain(start..).collect();
                    self.inlines.push(Inline::Link(url, inner));
                }
            }
            TagEnd::Image => {
                if let Some(Open::Image(start, src)) = self.open.pop() {
                    let inner: Vec<Inline> = self.inlines.drain(start..).collect();
                    let index = load_image_src(&mut self.images, &src, self.base);
                    self.inlines.push(Inline::Image {
                        index,
                        alt: plain_inlines(&inner),
                    });
                }
            }
            _ => {}
        }
    }

    fn push_style(&mut self, style: Style) {
        self.style_stack.push(self.style);
        self.style = style;
    }

    /// Đưa chữ đang gom thành một đoạn trong vùng hiện tại.
    fn flush(&mut self) {
        let inlines = tidy(std::mem::take(&mut self.inlines));
        if !inlines.is_empty() {
            self.top().push(Block::Paragraph(inlines));
        }
    }

    fn top(&mut self) -> &mut Vec<Block> {
        if self.containers.is_empty() {
            self.containers.push(Vec::new());
        }
        let last = self.containers.len() - 1;
        &mut self.containers[last]
    }

    fn finish(mut self) -> Document {
        self.flush();
        let footnotes: Vec<Vec<Block>> = {
            let mut definitions = self.definitions;
            self.labels
                .iter()
                .map(|label| definitions.remove(label).unwrap_or_default())
                .collect()
        };
        Document {
            blocks: self.containers.into_iter().next().unwrap_or_default(),
            images: self.images,
            footnotes,
            ..Document::default()
        }
    }
}

/// Mẫu Markdown ở dạng chuẩn mà bộ đọc HTML/DOCX sinh ra; dùng cho test vòng.
#[cfg(test)]
pub(crate) const SAMPLE: &str = "# Tiêu đề\n\n## Mục hai\n\nĐoạn có **đậm**, *nghiêng*, ~~gạch~~, \
[link](https://a.b) và `code`.\\\nDòng sau ngắt cứng.\n\n- một\n- hai\n    - con\n\n1. số một\n2. số hai\n\n\
| a | b |\n| --- | --- |\n| 1 | 2 |\n\n```\nlet x = 1;\n```\n\n> trích dẫn\n";

pub(crate) fn is_br(html: &str) -> bool {
    let tag: String = html.trim().to_ascii_lowercase().replace(' ', "");
    matches!(tag.as_str(), "<br>" | "<br/>")
}

/// Bảo đảm đang ở đầu dòng.
fn start_line(out: &mut String) {
    if !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
}

/// Kết một khối: bảo đảm có đúng một dòng trống phía sau.
fn end_block(out: &mut String) {
    // WHY: chỉ có dòng trống (từ `<br>` đầu file) thì đó chính là nội dung, không thêm dòng ngăn khối.
    if out.chars().all(|c| c == '\n') {
        return;
    }
    while !out.ends_with("\n\n") {
        out.push('\n');
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ops::doc::{Block, Cell, Document, Dropped, Inline, Style};

    fn text(s: &str) -> Inline {
        Inline::Text(s.to_string(), Style::default())
    }

    fn bold(s: &str) -> Inline {
        Inline::Text(
            s.to_string(),
            Style {
                bold: true,
                ..Style::default()
            },
        )
    }

    fn para(s: &str) -> Block {
        Block::Paragraph(vec![text(s)])
    }

    fn doc_of(blocks: Vec<Block>) -> Document {
        Document {
            blocks,
            ..Document::default()
        }
    }

    #[test]
    fn write_paragraph_heading_and_marks() {
        let doc = doc_of(vec![
            Block::Heading(1, vec![text("T")]),
            Block::Paragraph(vec![text("a "), bold("b"), text(" *c*")]),
        ]);
        assert_eq!(write(&doc).0, "# T\n\na **b** \\*c\\*\n");
    }

    #[test]
    fn write_nested_lists_use_four_spaces() {
        let doc = doc_of(vec![Block::List {
            start: Some(1),
            items: vec![
                vec![
                    para("một"),
                    Block::List {
                        start: None,
                        items: vec![vec![para("con")]],
                    },
                ],
                vec![para("hai")],
            ],
        }]);
        assert_eq!(write(&doc).0, "1. một\n    - con\n2. hai\n");
    }

    #[test]
    fn write_underline_and_merged_cells_downgrade() {
        let underlined = Block::Paragraph(vec![Inline::Text(
            "u".into(),
            Style {
                underline: true,
                ..Style::default()
            },
        )]);
        let merged = Cell {
            blocks: vec![para("x")],
            colspan: 2,
            rowspan: 1,
            header: false,
        };
        let plain = Cell {
            blocks: vec![para("y")],
            colspan: 1,
            rowspan: 1,
            header: false,
        };
        let doc = doc_of(vec![
            underlined,
            Block::Table(vec![vec![merged], vec![plain.clone(), plain]]),
        ]);
        let (md, dropped) = write(&doc);
        assert!(md.starts_with("u\n\n"), "{md}");
        assert!(dropped.md_downgraded);
    }

    #[test]
    fn write_image_keeps_alt_and_counts() {
        let doc = doc_of(vec![Block::Paragraph(vec![Inline::Image {
            index: None,
            alt: "logo".into(),
        }])]);
        assert_eq!(
            write(&doc),
            (
                "logo\n".to_string(),
                Dropped {
                    images: 1,
                    ..Dropped::default()
                }
            )
        );
    }

    #[test]
    fn write_footnotes_at_end() {
        let mut doc = doc_of(vec![Block::Paragraph(vec![
            text("a"),
            Inline::FootnoteRef(0),
        ])]);
        doc.footnotes = vec![vec![para("ghi chú")]];
        assert_eq!(write(&doc).0, "a[^1]\n\n[^1]: ghi chú\n");
    }

    #[test]
    fn headings_and_emphasis_lose_their_marks() {
        assert_eq!(
            markdown_to_plain("# Tiêu đề\n\nchữ **đậm** và *nghiêng*"),
            "Tiêu đề\n\nchữ đậm và nghiêng\n"
        );
    }

    #[test]
    fn links_keep_text_and_drop_url() {
        assert_eq!(markdown_to_plain("[Rust](https://rust-lang.org)"), "Rust\n");
        assert_eq!(markdown_to_plain("![logo](a.png)"), "logo\n");
    }

    #[test]
    fn code_block_keeps_content_without_fences() {
        assert_eq!(
            markdown_to_plain("```rust\nlet x = 1;\n```"),
            "let x = 1;\n"
        );
    }

    #[test]
    fn lists_become_plain_dashes_and_numbers() {
        assert_eq!(
            markdown_to_plain("- a\n- b\n\n1. c\n2. d"),
            "- a\n- b\n\n1. c\n2. d\n"
        );
    }

    #[test]
    fn table_cells_are_tab_separated() {
        assert_eq!(
            markdown_to_plain("| a | b |\n|---|---|\n| 1 | 2 |"),
            "a\tb\n1\t2\n"
        );
    }

    #[test]
    fn br_html_becomes_newline_and_other_html_is_kept() {
        assert_eq!(markdown_to_plain("a<br>b"), "a\nb\n");
        assert!(markdown_to_plain("<div>giữ</div>").contains("giữ"));
    }

    #[test]
    fn escaped_text_round_trips() {
        let t = "a_b *c* # d\n- e\n1. f\n[g](h) | i\n<j> &amp; ~k~ `l` \\m\n---\n===\n\nđoạn hai";
        assert_eq!(markdown_to_plain(&escape_plain(t)).trim_end(), t);
    }

    #[test]
    fn indented_text_round_trips_without_becoming_code() {
        let t = "Bài thơ:\n\n    câu một_x\n\tcâu hai\n  b";
        assert_eq!(markdown_to_plain(&escape_plain(t)).trim_end(), t);
    }

    #[test]
    fn footnotes_and_reference_definitions_keep_their_text() {
        let plain = markdown_to_plain("x[^1]\n\n[^1]: note");
        assert!(plain.contains("note"), "{plain:?}");
        assert!(plain.contains("[1]"), "{plain:?}");
    }

    #[test]
    fn list_continuation_paragraphs_and_code_keep_their_indent() {
        assert_eq!(
            markdown_to_plain("- a\n\n  para2\n- b"),
            "- a\n  para2\n- b\n"
        );
        assert_eq!(
            markdown_to_plain("1. a\n\n   ```\n   x\n   y\n   ```"),
            "1. a\n   x\n   y\n"
        );
    }

    #[test]
    fn br_in_table_cell_keeps_the_row_on_one_line() {
        assert_eq!(markdown_to_plain("| a<br>b |\n|---|\n| 1 |"), "a b\n1\n");
    }

    #[test]
    fn blank_lines_and_trailing_spaces_round_trip() {
        let t = "\nđầu\n\n\ngiữa  \ncuối";
        assert_eq!(markdown_to_plain(&escape_plain(t)).trim_end(), t);
    }

    #[test]
    fn crlf_line_endings_are_kept() {
        assert_eq!(escape_plain("a\r\nb\r\n"), "a\\\r\nb\r\n");
    }

    #[test]
    fn escape_plain_leaves_plain_words_alone() {
        assert_eq!(escape_plain("xin chào"), "xin chào");
    }

    #[test]
    fn empty_markdown_is_empty_text() {
        assert_eq!(markdown_to_plain(""), "");
    }

    fn row(cells: &[&str]) -> Vec<String> {
        cells.iter().map(|c| c.to_string()).collect()
    }

    fn cue(start_ms: u64, text: &str) -> Cue {
        Cue {
            start_ms,
            end_ms: start_ms + 1000,
            text: text.to_string(),
        }
    }

    #[test]
    fn table_has_header_separator_and_escaped_pipes() {
        assert_eq!(
            table_to_markdown(&[row(&["a", "b|c"]), row(&["1", "2"])]),
            "| a | b\\|c |\n| --- | --- |\n| 1 | 2 |\n"
        );
    }

    #[test]
    fn short_rows_are_padded_and_multiline_cells_use_br() {
        let md = table_to_markdown(&[row(&["a", "b"]), row(&["x"]), row(&["l1\nl2", "y"])]);
        let lines: Vec<&str> = md.lines().collect();
        assert_eq!(lines[2], "| x |  |");
        assert_eq!(lines[3], "| l1<br>l2 | y |");
    }

    #[test]
    fn empty_table_is_empty_string() {
        assert_eq!(table_to_markdown(&[]), "");
    }

    #[test]
    fn cues_become_bold_time_list() {
        assert_eq!(
            cues_to_markdown(&[cue(1000, "Xin chào")]),
            "- **00:00:01** Xin chào\n"
        );
        assert!(cues_to_markdown(&[cue(360_000_000, "x")]).contains("**100:00:00**"));
    }

    #[test]
    fn multiline_cue_uses_br_and_escapes_text() {
        assert_eq!(
            cues_to_markdown(&[cue(0, "a*\nb")]),
            "- **00:00:00** a\\*<br>b\n"
        );
    }

    #[test]
    fn no_cues_gives_empty_string() {
        assert_eq!(cues_to_markdown(&[]), "");
    }

    #[test]
    fn pages_get_numbered_headings() {
        assert_eq!(
            pages_to_markdown(&["một".to_string(), "hai_".to_string()]),
            "## Trang 1\n\nmột\n\n## Trang 2\n\nhai\\_\n"
        );
        assert!(pages_to_markdown(&[String::new()]).contains("## Trang 1"));
    }

    #[test]
    fn markdown_read_builds_list_with_two_paragraphs() {
        let doc = read("1. a\n\n   b\n2. c\n", None);
        assert_eq!(
            doc.blocks,
            [Block::List {
                start: Some(1),
                items: vec![vec![para("a"), para("b")], vec![para("c")]],
            }]
        );
    }
}
