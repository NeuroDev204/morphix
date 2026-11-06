//! HTML ↔ cây tài liệu. Bộ đọc dựng `Document`; Markdown chỉ là một đích ghi từ cây đó.

use dom_query::{Document as HtmlDocument, NodeRef};
use pulldown_cmark::{html::push_html, Options, Parser};

use std::path::Path;

use base64::{engine::general_purpose::STANDARD, Engine as _};

use crate::ops::{
    doc::{
        load_image_src, plain_inlines, Block, Cell, Document, Dropped, Image, Inline, Style,
        MAX_SPAN,
    },
    markdown,
};

/// WHY: duyệt DOM bằng đệ quy; HTML lồng hàng nghìn cấp sẽ tràn stack và làm sập cả ứng dụng.
/// Quá độ sâu này thì lấy chữ thô của nhánh còn lại, không mất chữ.
const MAX_DEPTH: usize = 200;

/// Markdown từ HTML, kèm số ảnh không mang theo được (chỉ giữ chữ thay thế).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HtmlToMarkdown {
    pub markdown: String,
    pub images_dropped: usize,
}

/// Trang HTML đầy đủ (UTF-8, có tiêu đề) từ Markdown.
pub fn markdown_to_html(markdown: &str, title: &str) -> String {
    let options =
        Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_FOOTNOTES;
    let mut body = String::new();
    push_html(&mut body, Parser::new_ext(markdown, options));
    format!(
        "<!doctype html>\n<html><head><meta charset=\"utf-8\"><title>{}</title></head><body>\n{body}</body></html>\n",
        escape_html(title)
    )
}

/// Markdown dạng chuẩn từ HTML, đi qua cây tài liệu.
pub fn html_to_markdown(html: &str) -> HtmlToMarkdown {
    let (markdown, dropped) = markdown::write(&read(html, None));
    HtmlToMarkdown {
        markdown,
        images_dropped: dropped.images,
    }
}

/// Cây tài liệu từ HTML. Thẻ lạ giữ chữ bên trong; `script`, `style`, `head` bị bỏ.
pub fn read(html: &str, base: Option<&Path>) -> Document {
    let mut images = Vec::new();
    let blocks = read_blocks(html, base, &mut images);
    Document {
        blocks,
        images,
        ..Document::default()
    }
}

/// Khối của HTML; ảnh đọc được được thêm vào `images`, để HTML nằm trong Markdown dùng chung danh sách ảnh.
pub(crate) fn read_blocks(html: &str, base: Option<&Path>, images: &mut Vec<Image>) -> Vec<Block> {
    let document = HtmlDocument::from(html);
    let mut walker = Walker {
        depth: 0,
        base,
        images: std::mem::take(images),
    };
    let blocks = match document.body() {
        Some(body) => walker.blocks_of(&body),
        None => walker.blocks_of(&document.root()),
    };
    *images = walker.images;
    blocks
}

/// Thẻ bị bỏ cả chữ bên trong: không phải nội dung người đọc thấy.
const SKIPPED: &[&str] = &["head", "title", "script", "style", "noscript", "template"];

/// Thẻ mở khối mới; mọi thẻ khác là inline.
const BLOCKS: &[&str] = &[
    "p",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "ul",
    "ol",
    "table",
    "pre",
    "blockquote",
    "hr",
    "div",
    "section",
    "article",
    "main",
    "header",
    "footer",
    "nav",
    "aside",
    "figure",
    "dl",
    "form",
    "fieldset",
    "address",
    "body",
    "html",
    "li",
    "dt",
    "dd",
    "figcaption",
    "caption",
    "details",
    "summary",
];

struct Walker<'a> {
    depth: usize,
    base: Option<&'a Path>,
    images: Vec<Image>,
}

impl Walker<'_> {
    /// Các khối của con `node`; chữ rời giữa các khối gom thành đoạn.
    fn blocks_of(&mut self, node: &NodeRef) -> Vec<Block> {
        if self.depth > MAX_DEPTH {
            return paragraph(vec![Inline::Text(collapse(&node.text()), Style::default())]);
        }
        self.depth += 1;
        let blocks = self.blocks_of_children(node);
        self.depth -= 1;
        blocks
    }

    fn blocks_of_children(&mut self, node: &NodeRef) -> Vec<Block> {
        let mut blocks = Vec::new();
        let mut run: Vec<Inline> = Vec::new();
        for child in node.children() {
            let name = tag(&child);
            if name
                .as_deref()
                .is_some_and(|n| BLOCKS.contains(&n) || SKIPPED.contains(&n))
            {
                blocks.extend(paragraph(std::mem::take(&mut run)));
                blocks.extend(self.block(&child, name.as_deref().unwrap_or_default()));
            } else {
                run.extend(self.inline(&child, Style::default()));
            }
        }
        blocks.extend(paragraph(run));
        blocks
    }

    fn block(&mut self, node: &NodeRef, name: &str) -> Vec<Block> {
        match name {
            _ if SKIPPED.contains(&name) => Vec::new(),
            "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                let inlines = tidy(self.inline_children(node, Style::default()));
                let level = name.as_bytes()[1] - b'0';
                if inlines.is_empty() {
                    Vec::new()
                } else {
                    vec![Block::Heading(level, inlines)]
                }
            }
            "p" => paragraph(self.inline_children(node, Style::default())),
            "ul" | "ol" => vec![self.list(node)],
            "table" => {
                let mut before = Vec::new();
                let mut rows = Vec::new();
                self.collect_rows(node, &mut rows, &mut before);
                if !rows.is_empty() {
                    before.push(Block::Table(rows));
                }
                before
            }
            "pre" => vec![Block::Code(
                node.text().to_string().trim_end_matches('\n').to_string(),
            )],
            "blockquote" => {
                let inner = self.blocks_of(node);
                if inner.is_empty() {
                    Vec::new()
                } else {
                    vec![Block::Quote(inner)]
                }
            }
            "hr" => vec![Block::Rule],
            _ => self.blocks_of(node),
        }
    }

    /// Danh sách; danh sách con nằm trong mục chứa nó.
    fn list(&mut self, node: &NodeRef) -> Block {
        if self.depth > MAX_DEPTH {
            return Block::Paragraph(vec![Inline::Text(collapse(&node.text()), Style::default())]);
        }
        let start = (tag(node).as_deref() == Some("ol")).then(|| {
            node.attr("start")
                .and_then(|s| s.trim().parse::<u64>().ok())
                .unwrap_or(1)
        });
        self.depth += 1;
        let mut items: Vec<Vec<Block>> = Vec::new();
        for child in node.children() {
            match tag(&child).as_deref() {
                Some("li") => items.push(self.item(&child)),
                // WHY: HTML sai chuẩn hay đặt danh sách con thẳng trong `ul`; không để mất mục.
                Some("ul" | "ol") => {
                    let nested = self.list(&child);
                    match items.last_mut() {
                        Some(item) => item.push(nested),
                        None => items.push(vec![nested]),
                    }
                }
                _ => {
                    let inlines = tidy(self.inline(&child, Style::default()));
                    if !inlines.is_empty() {
                        items.push(vec![Block::Paragraph(inlines)]);
                    }
                }
            }
        }
        self.depth -= 1;
        Block::List { start, items }
    }

    /// Một mục: chữ của mục đứng trước, danh sách con đứng sau (đúng thứ tự Markdown).
    fn item(&mut self, node: &NodeRef) -> Vec<Block> {
        let mut inlines: Vec<Inline> = Vec::new();
        let mut nested = Vec::new();
        for child in node.children() {
            match tag(&child).as_deref() {
                Some("ul" | "ol") if self.depth <= MAX_DEPTH => nested.push(self.list(&child)),
                Some("p") => {
                    inlines.push(Inline::Text(" ".to_string(), Style::default()));
                    inlines.extend(self.inline_children(&child, Style::default()));
                }
                _ => inlines.extend(self.inline(&child, Style::default())),
            }
        }
        let mut blocks = paragraph(inlines);
        blocks.extend(nested);
        blocks
    }

    /// Mẩu inline của một nút, mang theo định dạng của các thẻ bao ngoài.
    fn inline(&mut self, node: &NodeRef, style: Style) -> Vec<Inline> {
        if node.is_text() || self.depth > MAX_DEPTH {
            return vec![Inline::Text(collapse(&node.text()), style)];
        }
        let Some(name) = tag(node) else {
            return Vec::new(); // comment, doctype
        };
        self.depth += 1;
        let inlines = self.inline_element(node, &name, style);
        self.depth -= 1;
        inlines
    }

    fn inline_children(&mut self, node: &NodeRef, style: Style) -> Vec<Inline> {
        let mut out = Vec::new();
        for child in node.children() {
            out.extend(self.inline(&child, style));
        }
        out
    }

    fn inline_element(&mut self, node: &NodeRef, name: &str, style: Style) -> Vec<Inline> {
        match name {
            _ if SKIPPED.contains(&name) => Vec::new(),
            "br" => vec![Inline::Break],
            "strong" | "b" => self.inline_children(
                node,
                Style {
                    bold: true,
                    ..style
                },
            ),
            "em" | "i" => self.inline_children(
                node,
                Style {
                    italic: true,
                    ..style
                },
            ),
            "del" | "s" | "strike" => self.inline_children(
                node,
                Style {
                    strike: true,
                    ..style
                },
            ),
            "u" | "ins" => self.inline_children(
                node,
                Style {
                    underline: true,
                    ..style
                },
            ),
            "sup" => self.inline_children(node, Style { sup: true, ..style }),
            "sub" => self.inline_children(node, Style { sub: true, ..style }),
            "code" | "kbd" | "samp" => vec![Inline::Text(
                collapse(&node.text()),
                Style {
                    code: true,
                    ..style
                },
            )],
            // WHY: thẻ khối nằm trong thẻ inline (div trong a) vẫn là ranh giới chữ.
            _ if BLOCKS.contains(&name) => {
                let mut out = vec![Inline::Text(" ".to_string(), style)];
                out.extend(self.inline_children(node, style));
                out.push(Inline::Text(" ".to_string(), style));
                out
            }
            "a" => {
                let inner = self.inline_children(node, style);
                let plain = plain_inlines(&inner);
                match node.attr("href") {
                    Some(href) if !plain.trim().is_empty() => {
                        // WHY: khoảng trắng mép của link đứng ngoài `[…]`, nếu không hai chữ cạnh link dính vào nhau.
                        let mut out = Vec::new();
                        if plain.starts_with(' ') {
                            out.push(Inline::Text(" ".to_string(), style));
                        }
                        out.push(Inline::Link(href.to_string(), tidy(inner)));
                        if plain.ends_with(' ') {
                            out.push(Inline::Text(" ".to_string(), style));
                        }
                        out
                    }
                    _ => inner,
                }
            }
            "img" => vec![Inline::Image {
                index: node
                    .attr("src")
                    .and_then(|src| load_image_src(&mut self.images, &src, self.base)),
                alt: node
                    .attr("alt")
                    .map(|alt| collapse(&alt))
                    .unwrap_or_default(),
            }],
            _ => self.inline_children(node, style),
        }
    }

    /// Các hàng của bảng theo thứ tự (cả `thead`, `tbody`, `tfoot`); `caption` thành đoạn đứng trước bảng.
    fn collect_rows(&mut self, node: &NodeRef, rows: &mut Vec<Vec<Cell>>, before: &mut Vec<Block>) {
        for child in node.children() {
            match tag(&child).as_deref() {
                Some("tr") => {
                    let cells = child
                        .children()
                        .iter()
                        .filter(|cell| matches!(tag(cell).as_deref(), Some("td" | "th")))
                        .map(|cell| self.cell(cell))
                        .collect();
                    rows.push(cells);
                }
                Some("thead" | "tbody" | "tfoot") if self.depth <= MAX_DEPTH => {
                    self.depth += 1;
                    self.collect_rows(&child, rows, before);
                    self.depth -= 1;
                }
                Some("caption") => before.extend(paragraph(vec![Inline::Text(
                    collapse(&child.text()),
                    Style::default(),
                )])),
                _ => {}
            }
        }
    }

    fn cell(&mut self, node: &NodeRef) -> Cell {
        Cell {
            blocks: self.blocks_of(node),
            colspan: span_attr(node, "colspan"),
            rowspan: span_attr(node, "rowspan"),
            header: tag(node).as_deref() == Some("th"),
        }
    }
}

// `colspan`/`rowspan` của ô, kẹp để một số lớn không làm nổ lưới.
fn span_attr(node: &NodeRef, name: &str) -> u32 {
    node.attr(name)
        .and_then(|value| value.trim().parse::<u32>().ok())
        .unwrap_or(1)
        .clamp(1, MAX_SPAN)
}

fn tag(node: &NodeRef) -> Option<String> {
    node.node_name().map(|name| name.to_ascii_lowercase())
}

/// Gộp mọi chuỗi khoảng trắng ASCII thành một dấu cách. WHY: không đụng `\u{a0}` vì đó là chữ
/// người viết cố ý đặt.
fn collapse(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut in_space = false;
    for ch in text.chars() {
        if ch.is_ascii_whitespace() {
            if !in_space {
                out.push(' ');
            }
            in_space = true;
        } else {
            out.push(ch);
            in_space = false;
        }
    }
    out
}

/// Đoạn từ các mẩu inline; rỗng thì không tạo khối.
fn paragraph(inlines: Vec<Inline>) -> Vec<Block> {
    let inlines = tidy(inlines);
    if inlines.is_empty() {
        Vec::new()
    } else {
        vec![Block::Paragraph(inlines)]
    }
}

/// Bỏ khoảng trắng thừa giữa các mẩu, khoảng trắng và ngắt dòng ở hai đầu, và khoảng trắng quanh `<br>`.
/// WHY: HTML không giữ khoảng trắng thừa giữa các thẻ, nhưng Markdown và DOCX sinh ra từ đây thì không.
pub(crate) fn tidy(inlines: Vec<Inline>) -> Vec<Inline> {
    let mut out: Vec<Inline> = Vec::new();
    for inline in inlines {
        match inline {
            Inline::Text(text, style) => {
                let at_edge = matches!(out.last(), None | Some(Inline::Break))
                    || matches!(out.last(), Some(Inline::Text(prev, _)) if prev.ends_with(' '));
                let text = if at_edge {
                    text.trim_start_matches(' ').to_string()
                } else {
                    text
                };
                if !text.is_empty() {
                    out.push(Inline::Text(text, style));
                }
            }
            Inline::Break => {
                if out.is_empty() {
                    continue;
                }
                if let Some(Inline::Text(prev, _)) = out.last_mut() {
                    let end = prev.trim_end_matches(' ').len();
                    prev.truncate(end);
                }
                out.push(Inline::Break);
            }
            other => out.push(other),
        }
    }
    loop {
        match out.last_mut() {
            Some(Inline::Break) => {
                out.pop();
            }
            Some(Inline::Text(text, _)) => {
                let end = text.trim_end_matches(' ').len();
                text.truncate(end);
                if text.is_empty() {
                    out.pop();
                } else {
                    break;
                }
            }
            _ => break,
        }
    }
    out.retain(|inline| !matches!(inline, Inline::Text(text, _) if text.is_empty()));
    out
}

fn escape_html(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Trang HTML đầy đủ từ cây tài liệu. Ảnh chưa nhúng được giữ chữ thay thế và được đếm để cảnh báo.
pub fn write(doc: &Document, title: &str) -> (String, Dropped) {
    let mut writer = HtmlWriter {
        dropped: doc.dropped,
        images: &doc.images,
    };
    let body: String = doc
        .blocks
        .iter()
        .map(|block| format!("{}\n", writer.block(block)))
        .collect();
    // WHY: chú thích cuối trang gom thành một mục ở cuối body; tham chiếu trong thân trỏ tới `fn{n}`.
    let notes: String = if doc.footnotes.is_empty() {
        String::new()
    } else {
        let items: String = doc
            .footnotes
            .iter()
            .enumerate()
            .map(|(index, note)| format!("<li id=\"fn{}\">{}</li>", index + 1, writer.blocks(note)))
            .collect();
        format!("<section class=\"footnotes\"><ol>{items}</ol></section>\n")
    };
    let page = format!(
        "<!doctype html>\n<html><head><meta charset=\"utf-8\"><title>{}</title><style>table{{border-collapse:collapse}}td,th{{border:1px solid #999;padding:4px}}</style></head><body>\n{body}{notes}</body></html>\n",
        escape_html(title)
    );
    (page, writer.dropped)
}

struct HtmlWriter<'a> {
    dropped: Dropped,
    images: &'a [Image],
}

impl HtmlWriter<'_> {
    fn block(&mut self, block: &Block) -> String {
        match block {
            Block::Heading(level, inlines) => {
                format!("<h{level}>{}</h{level}>", self.inline(inlines))
            }
            Block::Paragraph(inlines) => format!("<p>{}</p>", self.inline(inlines)),
            Block::List { start, items } => {
                let tag = if start.is_some() { "ol" } else { "ul" };
                let start_attr = match start {
                    Some(n) if *n != 1 => format!(" start=\"{n}\""),
                    _ => String::new(),
                };
                let items: String = items
                    .iter()
                    .map(|item| format!("<li>{}</li>", self.blocks(item)))
                    .collect();
                format!("<{tag}{start_attr}>{items}</{tag}>")
            }
            Block::Table(rows) => {
                let mut out = String::from("<table>");
                for row in rows {
                    out.push_str("<tr>");
                    for cell in row {
                        let tag = if cell.header { "th" } else { "td" };
                        let mut attrs = String::new();
                        if cell.colspan > 1 {
                            attrs.push_str(&format!(" colspan=\"{}\"", cell.colspan));
                        }
                        if cell.rowspan > 1 {
                            attrs.push_str(&format!(" rowspan=\"{}\"", cell.rowspan));
                        }
                        out.push_str(&format!(
                            "<{tag}{attrs}>{}</{tag}>",
                            self.blocks(&cell.blocks)
                        ));
                    }
                    out.push_str("</tr>");
                }
                out.push_str("</table>");
                out
            }
            Block::Code(code) => format!("<pre><code>{}</code></pre>", escape_html(code)),
            Block::Quote(blocks) => format!("<blockquote>{}</blockquote>", self.blocks(blocks)),
            Block::Rule => "<hr>".to_string(),
        }
    }

    fn blocks(&mut self, blocks: &[Block]) -> String {
        blocks
            .iter()
            .map(|block| self.block(block))
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn inline(&mut self, inlines: &[Inline]) -> String {
        let mut out = String::new();
        for inline in inlines {
            match inline {
                Inline::Text(text, style) => out.push_str(&styled(text, *style)),
                Inline::Link(url, inner) => out.push_str(&format!(
                    "<a href=\"{}\">{}</a>",
                    escape_html(url),
                    self.inline(inner)
                )),
                // WHY: ảnh chưa nhúng vào trang (xem Task 6); giữ chữ thay thế và đếm để cảnh báo.
                Inline::Image { index, alt } => {
                    match index.and_then(|index| self.images.get(index)) {
                        Some(image) => out.push_str(&format!(
                            "<img src=\"data:{};base64,{}\" alt=\"{}\">",
                            image.mime,
                            STANDARD.encode(&image.bytes),
                            escape_html(alt)
                        )),
                        // WHY: ảnh không nạp được: giữ chữ thay thế và đếm để cảnh báo.
                        None => {
                            self.dropped.images += 1;
                            out.push_str(&escape_html(alt));
                        }
                    }
                }
                Inline::FootnoteRef(index) => out.push_str(&format!(
                    "<sup><a href=\"#fn{n}\" id=\"ref{n}\">{n}</a></sup>",
                    n = index + 1
                )),
                Inline::Break => out.push_str("<br>"),
            }
        }
        out
    }
}

/// Chữ đã thoát, bọc thẻ theo định dạng. Thứ tự bọc không quan trọng với trình duyệt.
fn styled(text: &str, style: Style) -> String {
    let mut out = escape_html(text);
    for (on, open, close) in [
        (style.code, "<code>", "</code>"),
        (style.bold, "<strong>", "</strong>"),
        (style.italic, "<em>", "</em>"),
        (style.underline, "<u>", "</u>"),
        (style.strike, "<s>", "</s>"),
        (style.sup, "<sup>", "</sup>"),
        (style.sub, "<sub>", "</sub>"),
    ] {
        if on {
            out = format!("{open}{out}{close}");
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ops::markdown::SAMPLE;

    #[test]
    fn html_page_has_charset_title_and_formatting() {
        let html = markdown_to_html("# A\n\n**b**", "t<x>");
        assert!(html.contains("<meta charset=\"utf-8\">"), "{html}");
        assert!(html.contains("<title>t&lt;x&gt;</title>"), "{html}");
        assert!(html.contains("<h1>A</h1>"), "{html}");
        assert!(html.contains("<strong>b</strong>"), "{html}");
    }

    #[test]
    fn markdown_round_trips_through_html() {
        assert_eq!(
            html_to_markdown(&markdown_to_html(SAMPLE, "x")).markdown,
            SAMPLE
        );
    }

    #[test]
    fn scripts_styles_and_head_are_dropped() {
        let md = html_to_markdown(
            "<head><title>T</title></head><script>x()</script><style>p{}</style><p>giữ</p>",
        );
        assert_eq!(md.markdown, "giữ\n");
    }

    #[test]
    fn unknown_tags_keep_their_text() {
        let md = html_to_markdown("<div>a <span>b</span></div><section>c</section>").markdown;
        assert!(md.contains("a b"), "{md:?}");
        assert!(md.contains('c'), "{md:?}");
    }

    #[test]
    fn text_with_markdown_characters_is_escaped() {
        assert_eq!(
            html_to_markdown("<p>a*b_c #1</p>").markdown,
            "a\\*b\\_c \\#1\n"
        );
    }

    #[test]
    fn entities_and_whitespace_collapse() {
        assert_eq!(
            html_to_markdown("<p>  x&nbsp;&amp;\n  y </p>").markdown,
            "x\u{a0}\\& y\n"
        );
    }

    #[test]
    fn ordered_list_keeps_start() {
        assert_eq!(
            html_to_markdown("<ol start=\"3\"><li>a</li><li>b</li></ol>").markdown,
            "3. a\n4. b\n"
        );
    }

    #[test]
    fn table_without_thead_uses_first_row_as_header() {
        assert_eq!(
            html_to_markdown(
                "<table><tr><td>a</td><td>b</td></tr><tr><td>1</td><td>2</td></tr></table>"
            )
            .markdown,
            "| a | b |\n| --- | --- |\n| 1 | 2 |\n"
        );
    }

    #[test]
    fn images_keep_alt_and_are_counted() {
        let md = html_to_markdown("<p><img src=\"a.png\" alt=\"logo\"> x</p><img src=\"b.png\">");
        assert!(md.markdown.contains("logo"), "{:?}", md.markdown);
        assert_eq!(md.images_dropped, 2);
    }

    #[test]
    fn sublist_under_numbered_item_stays_nested() {
        let md = html_to_markdown("<ol><li>a<ul><li>b</li></ul></li></ol>").markdown;
        assert_eq!(md, "1. a\n    - b\n");
        assert!(markdown_to_html(&md, "x").contains("<li>a\n<ul>"), "{md:?}");
    }

    #[test]
    fn list_children_that_are_not_li_keep_their_text() {
        let md = html_to_markdown("<ul><li>a</li><ul><li>b</li></ul><b>c</b></ul>").markdown;
        for word in ["a", "b", "c"] {
            assert!(md.contains(word), "{word}: {md:?}");
        }
    }

    #[test]
    fn cells_definition_lists_and_blocks_in_links_keep_word_boundaries() {
        let table = |cell: &str| {
            html_to_markdown(&format!(
                "<table><tr><td>h</td></tr><tr><td>{cell}</td></tr></table>"
            ))
            .markdown
        };
        assert!(table("a<br>b").contains("| a<br>b |"));
        assert!(table("<p>a</p><p>b</p>").contains("| a<br>b |"));
        let dl = html_to_markdown("<dl><dt>Term</dt><dd>Def</dd></dl>").markdown;
        assert_eq!(dl, "Term\n\nDef\n");
        let link = html_to_markdown("<a href=\"u\"><div>Title</div><div>Desc</div></a>").markdown;
        assert!(link.contains("Title Desc"), "{link:?}");
        let caption =
            html_to_markdown("<table><caption>Bảng 1</caption><tr><td>a</td></tr></table>")
                .markdown;
        assert!(caption.contains("Bảng 1"), "{caption:?}");
    }

    #[test]
    fn emphasis_with_edge_spaces_keeps_marks_tight() {
        assert_eq!(
            html_to_markdown("<p><b>Note: </b>text <i> x</i>y</p>").markdown,
            "**Note:** text *x*y\n"
        );
    }

    #[test]
    fn deep_nesting_does_not_overflow_the_stack() {
        let html = format!("{}x{}", "<div>".repeat(5000), "</div>".repeat(5000));
        assert!(html_to_markdown(&html).markdown.contains('x'));
    }

    #[test]
    fn lines_after_a_break_are_escaped_at_their_start() {
        assert_eq!(
            html_to_markdown("<p>a<br>- b<br>===</p>").markdown,
            "a\\\n\\- b\\\n\\===\n"
        );
    }

    #[test]
    fn code_containing_backticks_keeps_its_fence() {
        let md =
            html_to_markdown("<pre>a\n```\nb</pre><p><code>a`b</code> <code></code>z</p>").markdown;
        let html = markdown_to_html(&md, "x");
        assert!(html.contains("<pre><code>a\n```\nb"), "{md:?}");
        assert!(html.contains("<code>a`b</code>"), "{md:?}");
        assert!(!md.contains("``z"), "{md:?}");
    }

    #[test]
    fn link_with_spaces_or_parens_is_wrapped() {
        let md = html_to_markdown("<a href=\"http://x/a b\">t</a>").markdown;
        assert_eq!(md, "[t](<http://x/a b>)\n");
        assert!(markdown_to_html(&md, "x").contains("href=\"http://x/a%20b\""));
    }

    #[test]
    fn emphasis_ending_in_punctuation_next_to_a_letter_still_renders() {
        let md = html_to_markdown("<p><b>(a)</b>b</p>").markdown;
        assert_eq!(md, "(**a**)b\n");
        assert!(markdown_to_html(&md, "x").contains("<strong>a</strong>"));
    }

    #[test]
    fn list_item_starting_with_number_is_escaped() {
        assert_eq!(
            html_to_markdown("<ul><li>1. x</li></ul>").markdown,
            "- 1\\. x\n"
        );
    }
}

#[cfg(test)]
mod tree_tests {
    use super::*;
    use crate::ops::doc::{Block, Inline, Style};

    #[test]
    fn read_builds_tree_not_markdown() {
        let doc = read("<p>a <b>b</b></p><ul><li>x</li></ul>", None);
        let bold = Style {
            bold: true,
            ..Style::default()
        };
        assert_eq!(
            doc.blocks,
            [
                Block::Paragraph(vec![
                    Inline::Text("a ".into(), Style::default()),
                    Inline::Text("b".into(), bold),
                ]),
                Block::List {
                    start: None,
                    items: vec![vec![Block::Paragraph(vec![Inline::Text(
                        "x".into(),
                        Style::default()
                    )])]],
                },
            ]
        );
    }
}

#[cfg(test)]
mod span_tests {
    use super::*;
    use crate::ops::doc::MAX_SPAN;

    #[test]
    fn oversized_spans_are_clamped() {
        let doc = read(
            r#"<table><tr><td colspan="99999999" rowspan="0">a</td></tr></table>"#,
            None,
        );
        let Block::Table(rows) = &doc.blocks[0] else {
            panic!("cần bảng, được {:?}", doc.blocks);
        };
        assert_eq!(rows[0][0].colspan, MAX_SPAN);
        assert_eq!(rows[0][0].rowspan, 1);
    }
}
