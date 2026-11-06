//! Word (DOCX) ↔ cây tài liệu. Bộ đọc dựng `Document`; bộ ghi sinh WordprocessingML từ Markdown.

use std::{
    collections::{BTreeMap, HashMap},
    io::{Read, Seek, Write},
    path::Path,
};

use quick_xml::{
    escape::resolve_xml_entity,
    events::{BytesStart, Event},
    Reader, XmlVersion,
};
use zip::ZipArchive;

use crate::{
    error::{MorphixError, Result},
    ops::{
        doc::{
            add_image, grid, plain_inlines, Block, Cell, Document, Dropped, Image, Inline, Slot,
            Style, MAX_PART_BYTES, MAX_SPAN,
        },
        html::tidy,
    },
};

/// Dấu hiệu file OLE: file Word có mật khẩu (DOCX bị mã hoá) hoặc định dạng `.doc` cũ.
const OLE_MAGIC: &[u8] = &[0xD0, 0xCF, 0x11, 0xE0];

/// Markdown từ DOCX, kèm số ảnh không mang theo được (chỉ giữ chữ thay thế).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocxToMarkdown {
    pub markdown: String,
    pub images_dropped: usize,
    /// Chú thích cuối trang, ghi chú cuối bài và bình luận không mang theo được.
    pub notes_dropped: usize,
    /// Tài liệu có đầu/chân trang (không mang theo được).
    pub headers_dropped: bool,
}

/// Đọc file Word thành cây tài liệu. Đọc zip thẳng từ file, không nạp cả file vào RAM.
pub fn read(path: &Path) -> Result<Document> {
    let file = std::fs::File::open(path).map_err(|source| MorphixError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    read_from(path, file)
}

/// Markdown dạng chuẩn từ file Word, đi qua cây tài liệu.
pub fn docx_to_markdown(path: &Path) -> Result<DocxToMarkdown> {
    read(path).map(to_markdown)
}

#[cfg(test)]
pub(crate) fn docx_bytes_to_markdown(path: &Path, bytes: &[u8]) -> Result<DocxToMarkdown> {
    read_from(path, std::io::Cursor::new(bytes)).map(to_markdown)
}

fn to_markdown(doc: Document) -> DocxToMarkdown {
    let (markdown, dropped) = crate::ops::markdown::write(&doc);
    DocxToMarkdown {
        markdown,
        images_dropped: dropped.images,
        notes_dropped: dropped.notes,
        headers_dropped: dropped.headers,
    }
}

pub(crate) fn read_from<R: Read + Seek>(path: &Path, mut reader: R) -> Result<Document> {
    let io_error = |source| MorphixError::Io {
        path: path.to_path_buf(),
        source,
    };
    let mut magic = Vec::with_capacity(OLE_MAGIC.len());
    (&mut reader)
        .take(OLE_MAGIC.len() as u64)
        .read_to_end(&mut magic)
        .map_err(io_error)?;
    if magic == OLE_MAGIC {
        return Err(corrupt(
            path,
            "file Word có mật khẩu hoặc định dạng .doc cũ, chưa hỗ trợ",
        ));
    }
    reader.rewind().map_err(io_error)?;
    let mut archive = ZipArchive::new(reader)
        .map_err(|e| corrupt(path, format!("không mở được file Word: {e}")))?;
    let document = read_part(path, &mut archive, "word/document.xml", MAX_PART_BYTES)?
        .ok_or_else(|| corrupt(path, "thiếu word/document.xml, không phải file Word hợp lệ"))?;
    let rels = read_part(
        path,
        &mut archive,
        "word/_rels/document.xml.rels",
        MAX_PART_BYTES,
    )?;
    let numbering = read_part(path, &mut archive, "word/numbering.xml", MAX_PART_BYTES)?;
    let styles = read_part(path, &mut archive, "word/styles.xml", MAX_PART_BYTES)?;
    let footnotes = read_part(path, &mut archive, "word/footnotes.xml", MAX_PART_BYTES)?;
    let endnotes = read_part(path, &mut archive, "word/endnotes.xml", MAX_PART_BYTES)?;
    let context = Context {
        links: rels
            .map(|xml| parse_rels(path, &xml))
            .transpose()?
            .unwrap_or_default(),
        ordered: numbering
            .map(|xml| parse_numbering(path, &xml))
            .transpose()?
            .unwrap_or_default(),
        styles: styles
            .map(|xml| parse_styles(path, &xml))
            .transpose()?
            .unwrap_or_default(),
    };
    let mut load_media = |rel: &str| -> Option<Vec<u8>> {
        let target = context.links.get(rel)?;
        let name = match target.strip_prefix('/') {
            Some(absolute) => absolute.to_string(),
            None => format!("word/{target}"),
        };
        match read_bytes(path, &mut archive, &name, MAX_PART_BYTES) {
            Ok(bytes) => bytes,
            Err(error) => {
                tracing::warn!(%error, "không đọc được ảnh trong file Word");
                None
            }
        }
    };
    let mut images: Vec<Image> = Vec::new();
    let mut notes = NoteMap::new();
    for xml in [footnotes, endnotes].into_iter().flatten() {
        let (_, parsed) = convert_document(
            path,
            &xml,
            &context,
            &mut load_media,
            &NoteMap::new(),
            &mut images,
        )?;
        notes.extend(parsed);
    }
    let (mut document, _) = convert_document(
        path,
        &document,
        &context,
        &mut load_media,
        &notes,
        &mut images,
    )?;
    document.images = images;
    Ok(document)
}

/// Một mục trong zip dạng bytes; `None` khi không có. Vượt `limit` byte là lỗi.
fn read_bytes<R: Read + Seek>(
    path: &Path,
    archive: &mut ZipArchive<R>,
    name: &str,
    limit: u64,
) -> Result<Option<Vec<u8>>> {
    let Some(index) = archive.index_for_name(name) else {
        return Ok(None);
    };
    let entry = archive
        .by_index(index)
        .map_err(|e| corrupt(path, format!("không đọc được {name}: {e}")))?;
    let mut bytes = Vec::new();
    entry
        .take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| corrupt(path, format!("không đọc được {name}: {e}")))?;
    if bytes.len() as u64 > limit {
        return Err(corrupt(path, format!("phần {name} quá lớn")));
    }
    Ok(Some(bytes))
}

/// Một mục trong zip dưới dạng UTF-8; `None` khi không có.
pub(crate) fn read_part<R: Read + Seek>(
    path: &Path,
    archive: &mut ZipArchive<R>,
    name: &str,
    limit: u64,
) -> Result<Option<String>> {
    read_bytes(path, archive, name, limit)?
        .map(|bytes| {
            String::from_utf8(bytes).map_err(|_| corrupt(path, format!("{name} không phải UTF-8")))
        })
        .transpose()
}

// Chú thích cuối trang đã đọc: (là endnote?, id) → các khối của chú thích.
type NoteMap = HashMap<(bool, String), Vec<Block>>;

struct Context {
    /// rId → đích của link.
    links: HashMap<String, String>,
    /// (numId, ilvl) → danh sách số (true) hay gạch đầu dòng (false).
    ordered: HashMap<(String, usize), bool>,
    /// styleId → tên style viết thường (id có thể đã dịch, tên thì không).
    styles: HashMap<String, String>,
}

impl Context {
    fn style_name(&self, id: &str) -> String {
        self.styles
            .get(id)
            .cloned()
            .unwrap_or_else(|| id.to_ascii_lowercase())
    }
}

/// Giá trị thuộc tính theo tên cục bộ (bỏ tiền tố `w:`, `r:`), đã giải mã thực thể.
fn attr(e: &BytesStart, local: &str) -> Option<String> {
    e.attributes()
        .flatten()
        .find(|a| a.key.local_name().as_ref() == local)
        .and_then(|a| {
            a.normalized_value(XmlVersion::Implicit1_0)
                .ok()
                .map(|v| v.into_owned())
        })
}

/// Bật/tắt kiểu `w:b`: không có `w:val` là bật; `0`/`false`/`off` là tắt.
fn toggle_on(e: &BytesStart) -> bool {
    !matches!(attr(e, "val").as_deref(), Some("0" | "false" | "off"))
}

fn xml_error(path: &Path, part: &str, error: impl std::fmt::Display) -> MorphixError {
    corrupt(path, format!("{part} hỏng: {error}"))
}

fn parse_rels(path: &Path, xml: &str) -> Result<HashMap<String, String>> {
    let mut reader = Reader::from_str(xml);
    let mut links = HashMap::new();
    loop {
        match reader
            .read_event()
            .map_err(|e| xml_error(path, "document.xml.rels", e))?
        {
            Event::Start(e) | Event::Empty(e) if e.local_name().as_ref() == "Relationship" => {
                if let (Some(id), Some(target)) = (attr(&e, "Id"), attr(&e, "Target")) {
                    links.insert(id, target);
                }
            }
            Event::Eof => return Ok(links),
            _ => {}
        }
    }
}

fn parse_numbering(path: &Path, xml: &str) -> Result<HashMap<(String, usize), bool>> {
    let mut reader = Reader::from_str(xml);
    // abstractNumId → (ilvl → có số)
    let mut abstracts: HashMap<String, HashMap<usize, bool>> = HashMap::new();
    let mut nums: Vec<(String, String)> = Vec::new();
    let mut abstract_id: Option<String> = None;
    let mut level: usize = 0;
    let mut num_id: Option<String> = None;
    loop {
        match reader
            .read_event()
            .map_err(|e| xml_error(path, "numbering.xml", e))?
        {
            Event::Start(e) | Event::Empty(e) => match e.local_name().as_ref() {
                "abstractNum" => abstract_id = attr(&e, "abstractNumId"),
                "lvl" => level = attr(&e, "ilvl").and_then(|v| v.parse().ok()).unwrap_or(0),
                "numFmt" => {
                    if let Some(id) = &abstract_id {
                        let ordered = attr(&e, "val").is_some_and(|f| f != "bullet" && f != "none");
                        abstracts
                            .entry(id.clone())
                            .or_default()
                            .insert(level, ordered);
                    }
                }
                "num" => num_id = attr(&e, "numId"),
                "abstractNumId" => {
                    if let (Some(num), Some(target)) = (&num_id, attr(&e, "val")) {
                        nums.push((num.clone(), target));
                    }
                }
                _ => {}
            },
            Event::End(e) if e.local_name().as_ref() == "abstractNum" => abstract_id = None,
            Event::Eof => break,
            _ => {}
        }
    }
    let mut ordered = HashMap::new();
    for (num, target) in nums {
        for (level, is_ordered) in abstracts.get(&target).into_iter().flatten() {
            ordered.insert((num.clone(), *level), *is_ordered);
        }
    }
    Ok(ordered)
}

fn parse_styles(path: &Path, xml: &str) -> Result<HashMap<String, String>> {
    let mut reader = Reader::from_str(xml);
    let mut styles = HashMap::new();
    let mut current: Option<String> = None;
    loop {
        match reader
            .read_event()
            .map_err(|e| xml_error(path, "styles.xml", e))?
        {
            Event::Start(e) | Event::Empty(e) => match e.local_name().as_ref() {
                "style" => current = attr(&e, "styleId"),
                "name" => {
                    if let (Some(id), Some(name)) = (&current, attr(&e, "val")) {
                        styles.insert(id.clone(), name.to_ascii_lowercase());
                    }
                }
                _ => {}
            },
            Event::Eof => return Ok(styles),
            _ => {}
        }
    }
}

/// Kiểu khối của một đoạn theo style và đánh số.
enum Kind {
    Paragraph,
    Heading(usize),
    Quote,
    Code,
    Item { num: String, level: usize },
}

fn heading_level(name: &str) -> Option<usize> {
    let compact: String = name.chars().filter(|c| !c.is_whitespace()).collect();
    let level: usize = compact.strip_prefix("heading")?.parse().ok()?;
    (1..=6).contains(&level).then_some(level)
}

/// Đoạn đang đọc.
#[derive(Default)]
struct Paragraph {
    style: Option<String>,
    num: Option<String>,
    level: usize,
    inlines: Vec<Inline>,
}

/// Danh sách đang mở ở một cấp; mục của cấp sâu hơn nằm trong mục cuối của cấp nông hơn.
struct ListFrame {
    num: String,
    ordered: bool,
    items: Vec<Vec<Block>>,
}

/// Các khối đang gom của một vùng: thân tài liệu, hoặc một ô bảng.
#[derive(Default)]
struct Builder {
    done: Vec<Block>,
    lists: Vec<ListFrame>,
    code: Vec<String>,
}

impl Builder {
    fn flush_code(&mut self) {
        if !self.code.is_empty() {
            self.done.push(Block::Code(self.code.join("\n")));
            self.code.clear();
        }
    }

    fn close_list(&mut self) {
        let Some(frame) = self.lists.pop() else {
            return;
        };
        let block = Block::List {
            start: frame.ordered.then_some(1),
            items: frame.items,
        };
        // WHY: danh sách con đặt vào mục cuối của danh sách cha; không có mục thì để ngang cấp để không mất chữ.
        match self
            .lists
            .last_mut()
            .and_then(|parent| parent.items.last_mut())
        {
            Some(item) => item.push(block),
            None => self.done.push(block),
        }
    }

    fn flush_lists(&mut self) {
        while !self.lists.is_empty() {
            self.close_list();
        }
    }

    fn flush_all(&mut self) {
        self.flush_code();
        self.flush_lists();
    }

    fn finish(mut self) -> Vec<Block> {
        self.flush_all();
        self.done
    }

    fn push(&mut self, kind: Kind, inlines: Vec<Inline>, context: &Context) {
        let kind = match kind {
            Kind::Item { num, level } => {
                self.push_item(num, level, inlines, context);
                return;
            }
            other => other,
        };
        if !matches!(kind, Kind::Code) {
            self.flush_code();
        }
        self.flush_lists();
        match kind {
            Kind::Code => self.code.push(plain_inlines(&inlines)),
            _ if inlines.is_empty() => {}
            Kind::Heading(level) => self.done.push(Block::Heading(level as u8, inlines)),
            Kind::Quote => self
                .done
                .push(Block::Quote(vec![Block::Paragraph(inlines)])),
            Kind::Paragraph => self.done.push(Block::Paragraph(inlines)),
            Kind::Item { .. } => {}
        }
    }

    fn push_item(&mut self, num: String, level: usize, inlines: Vec<Inline>, context: &Context) {
        if inlines.is_empty() {
            self.flush_code();
            return;
        }
        self.flush_code();
        while self.lists.len() > level + 1 {
            self.close_list();
        }
        // WHY: đổi numId ở cùng cấp là danh sách khác, đánh số lại từ đầu.
        if self.lists.len() == level + 1 && self.lists[level].num != num {
            self.close_list();
        }
        while self.lists.len() <= level {
            let ordered = context
                .ordered
                .get(&(num.clone(), self.lists.len()))
                .copied()
                .unwrap_or(false);
            self.lists.push(ListFrame {
                num: num.clone(),
                ordered,
                items: Vec::new(),
            });
        }
        self.lists[level]
            .items
            .push(vec![Block::Paragraph(inlines)]);
    }
}

fn kind_of(paragraph: &Paragraph, context: &Context) -> Kind {
    if let Some(num) = &paragraph.num {
        return Kind::Item {
            num: num.clone(),
            level: paragraph.level,
        };
    }
    let Some(style) = &paragraph.style else {
        return Kind::Paragraph;
    };
    let name = context.style_name(style);
    if let Some(level) = heading_level(&name) {
        return Kind::Heading(level);
    }
    match name.as_str() {
        "quote" | "intense quote" => Kind::Quote,
        "code" | "html preformatted" => Kind::Code,
        _ => Kind::Paragraph,
    }
}

/// Đẩy chữ vào đoạn đang đọc; mọi kiểu chữ đi qua đây.
fn push_text(paragraph: &mut Paragraph, text: &str, style: Style) {
    paragraph
        .inlines
        .push(Inline::Text(text.to_string(), style));
}

/// Bảng đang đọc: các hàng đã đóng, cột kế tiếp của hàng hiện tại, và ô gộp dọc còn mở theo cột.
#[derive(Default)]
struct OpenTable {
    rows: Vec<Vec<Cell>>,
    cursor: usize,
    /// Cột → (hàng, chỉ số ô) của ô `vMerge restart` đang chờ các ô `continue` bên dưới.
    merges: HashMap<usize, (usize, usize)>,
}

/// Kiểu gộp dọc của một ô (`w:vMerge`).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Merge {
    None,
    Restart,
    Continue,
}

/// Thuộc tính `w:tcPr` của ô đang đọc.
#[derive(Clone, Copy)]
struct CellProps {
    colspan: u32,
    merge: Merge,
}

impl Default for CellProps {
    fn default() -> Self {
        Self {
            colspan: 1,
            merge: Merge::None,
        }
    }
}

impl OpenTable {
    /// Đặt một ô vào hàng hiện tại. WHY: ô `vMerge continue` không thành ô mới mà tăng `rowspan` của ô
    /// trên; ô có chữ thì giữ như ô thường để không mất chữ.
    fn place(&mut self, blocks: Vec<Block>, props: CellProps) {
        let column = self.cursor;
        self.cursor += props.colspan as usize;
        if self.rows.is_empty() {
            self.rows.push(Vec::new());
        }
        let row_index = self.rows.len() - 1;
        if props.merge == Merge::Continue && blocks.is_empty() {
            if let Some(&(row, index)) = self.merges.get(&column) {
                if let Some(cell) = self
                    .rows
                    .get_mut(row)
                    .and_then(|cells| cells.get_mut(index))
                {
                    cell.rowspan += 1;
                    return;
                }
            }
        }
        let row = &mut self.rows[row_index];
        match props.merge {
            Merge::Restart => {
                self.merges.insert(column, (row_index, row.len()));
            }
            _ => {
                self.merges.remove(&column);
            }
        }
        row.push(Cell {
            blocks,
            colspan: props.colspan,
            rowspan: 1,
            header: false,
        });
    }
}

/// Gắn ảnh của `a:blip` (DOCX) hoặc `v:imagedata` (VML) vào mẩu ảnh đặt chỗ của đoạn đang đọc.
fn attach_image(
    paragraph: &mut Paragraph,
    e: &BytesStart,
    load_media: &mut dyn FnMut(&str) -> Option<Vec<u8>>,
    images: &mut Vec<Image>,
) {
    let index = attr(e, "embed")
        .or_else(|| attr(e, "id"))
        .and_then(|rel| load_media(&rel))
        .and_then(|bytes| add_image(images, bytes));
    if let Some(Inline::Image { index: slot, .. }) = paragraph.inlines.last_mut() {
        *slot = index;
    }
}

/// Duyệt `document.xml` và dựng cây tài liệu.
fn convert_document(
    path: &Path,
    xml: &str,
    context: &Context,
    load_media: &mut dyn FnMut(&str) -> Option<Vec<u8>>,
    known: &NoteMap,
    images: &mut Vec<Image>,
) -> Result<(Document, NoteMap)> {
    let mut reader = Reader::from_str(xml);
    let mut footnotes: Vec<Vec<Block>> = Vec::new();
    let mut collected: NoteMap = NoteMap::new();
    // Chú thích đang mở: khóa (endnote?, id), hoặc None với separator (không lưu).
    let mut notes_open: Vec<Option<(bool, String)>> = Vec::new();
    // builders[0] là thân tài liệu; mỗi ô bảng đang mở có một builder riêng ở trên cùng.
    let mut builders: Vec<Builder> = vec![Builder::default()];
    // Mỗi bảng đang mở: các hàng, mỗi hàng các ô đã đóng.
    let mut tables: Vec<OpenTable> = Vec::new();
    let mut cell_props = CellProps::default();
    let mut paragraph = Paragraph::default();
    let mut outer: Vec<Paragraph> = Vec::new();
    let mut style = Style::default();
    let mut in_run_props = false;
    let mut in_text = false;
    let mut in_para_props = false;
    // Link đang mở: (vị trí mẩu đầu tiên của link, đích).
    let mut link: Option<(usize, Option<String>)> = None;
    let mut notes_dropped = 0usize;
    let mut headers_dropped = false;

    loop {
        let event = reader
            .read_event()
            .map_err(|e| xml_error(path, "word/document.xml", e))?;
        // WHY: Word ghi textbox hai lần (mc:Choice và bản dự phòng mc:Fallback); bỏ bản dự phòng
        // để chữ không lặp. `*PrChange` là định dạng CŨ của thay đổi đang theo dõi, không phải hiện tại.
        if let Event::Start(e) = &event {
            let name = e.local_name();
            if name.as_ref() == "Fallback" || name.as_ref().ends_with("PrChange") {
                reader
                    .read_to_end(e.name())
                    .map_err(|e| xml_error(path, "word/document.xml", e))?;
                continue;
            }
        }
        match event {
            Event::Start(e) => match e.local_name().as_ref() {
                // WHY: đoạn trong textbox nằm lồng trong đoạn ngoài; cất đoạn ngoài để không mất chữ.
                "p" => outer.push(std::mem::take(&mut paragraph)),
                "r" => style = Style::default(),
                "rPr" => in_run_props = true,
                "pPr" => in_para_props = true,
                "footnote" | "endnote" => {
                    let key = attr(&e, "type").is_none().then(|| {
                        (
                            e.local_name().as_ref() == "endnote",
                            attr(&e, "id").unwrap_or_default(),
                        )
                    });
                    notes_open.push(key);
                    builders.push(Builder::default());
                }
                "t" => in_text = true,
                "hyperlink" => {
                    let target = attr(&e, "id").and_then(|id| context.links.get(&id).cloned());
                    link = Some((paragraph.inlines.len(), target));
                }
                "tbl" => tables.push(OpenTable::default()),
                "tr" => {
                    if let Some(table) = tables.last_mut() {
                        table.rows.push(Vec::new());
                        table.cursor = 0;
                    }
                }
                "tc" => {
                    builders.push(Builder::default());
                    cell_props = CellProps::default();
                }
                // WHY: ảnh đặt chỗ; chữ thay thế (docPr) lấy ở sự kiện sau, và bộ ghi đếm ảnh không nhúng được.
                "drawing" | "pict" => paragraph.inlines.push(Inline::Image {
                    index: None,
                    alt: String::new(),
                }),
                "blip" | "imagedata" => attach_image(&mut paragraph, &e, load_media, images),
                _ => properties(&e, &mut paragraph, &mut style, in_run_props),
            },
            Event::Empty(e) => match e.local_name().as_ref() {
                "br" | "cr" => paragraph.inlines.push(Inline::Break),
                "footnoteReference" | "endnoteReference" => {
                    let key = (
                        e.local_name().as_ref() == "endnoteReference",
                        attr(&e, "id").unwrap_or_default(),
                    );
                    match known.get(&key) {
                        Some(blocks) => {
                            footnotes.push(blocks.clone());
                            paragraph
                                .inlines
                                .push(Inline::FootnoteRef(footnotes.len() - 1));
                        }
                        None => notes_dropped += 1,
                    }
                }
                "commentReference" => notes_dropped += 1,
                "blip" | "imagedata" => attach_image(&mut paragraph, &e, load_media, images),
                "gridSpan" => {
                    cell_props.colspan = attr(&e, "val")
                        .and_then(|value| value.parse::<u32>().ok())
                        .unwrap_or(1)
                        .clamp(1, MAX_SPAN);
                }
                "vMerge" => {
                    cell_props.merge = match attr(&e, "val").as_deref() {
                        Some("restart") => Merge::Restart,
                        _ => Merge::Continue,
                    };
                }
                "headerReference" | "footerReference" => headers_dropped = true,
                "noBreakHyphen" => push_text(&mut paragraph, "\u{2011}", style),
                "softHyphen" => push_text(&mut paragraph, "\u{ad}", style),
                // WHY: ký hiệu chèn từ font (Symbol, Wingdings) chỉ có mã ký tự; giữ mã đó còn hơn mất chữ.
                "sym" => {
                    if let Some(ch) = attr(&e, "char")
                        .and_then(|hex| u32::from_str_radix(&hex, 16).ok())
                        .and_then(char::from_u32)
                    {
                        push_text(&mut paragraph, &ch.to_string(), style);
                    }
                }
                // WHY: `w:tab` trong `w:pPr/w:tabs` là điểm dừng tab, không phải ký tự tab trong chữ.
                "tab" if !in_run_props && !in_para_props => push_text(&mut paragraph, "\t", style),
                "docPr" => {
                    if let (Some(alt), Some(Inline::Image { alt: slot, .. })) = (
                        attr(&e, "descr")
                            .filter(|a| !a.is_empty())
                            .or_else(|| attr(&e, "title")),
                        paragraph.inlines.last_mut(),
                    ) {
                        *slot = alt;
                    }
                }
                _ => properties(&e, &mut paragraph, &mut style, in_run_props),
            },
            Event::Text(e) if in_text => {
                let text = e.xml10_content().into_owned();
                push_text(&mut paragraph, &text, style);
            }
            Event::GeneralRef(e) if in_text => {
                let text = match e.resolve_char_ref() {
                    Ok(Some(ch)) => ch.to_string(),
                    _ => {
                        let name = e.xml10_content();
                        resolve_xml_entity(&name)
                            .map(str::to_string)
                            .unwrap_or_else(|| format!("&{name};"))
                    }
                };
                push_text(&mut paragraph, &text, style);
            }
            Event::End(e) => match e.local_name().as_ref() {
                "t" => in_text = false,
                "rPr" => in_run_props = false,
                "pPr" => in_para_props = false,
                "hyperlink" => {
                    if let Some((start, target)) = link.take() {
                        let inner: Vec<Inline> = paragraph.inlines.drain(start..).collect();
                        match target {
                            Some(url) if !plain_inlines(&inner).trim().is_empty() => {
                                paragraph.inlines.push(Inline::Link(url, inner));
                            }
                            _ => paragraph.inlines.extend(inner),
                        }
                    }
                }
                "p" => {
                    let kind = kind_of(&paragraph, context);
                    let inlines = tidy(std::mem::take(&mut paragraph.inlines));
                    if let Some(builder) = builders.last_mut() {
                        builder.push(kind, inlines, context);
                    }
                    paragraph = outer.pop().unwrap_or_default();
                }
                "footnote" | "endnote" => {
                    let key = notes_open.pop().flatten();
                    let blocks = if builders.len() > 1 {
                        builders.pop().unwrap_or_default().finish()
                    } else {
                        Vec::new()
                    };
                    if let Some(key) = key {
                        collected.insert(key, blocks);
                    }
                }
                "tc" => {
                    // WHY: `builders.len() > 1` giữ thân tài liệu khi XML thiếu thẻ mở ô.
                    if builders.len() > 1 {
                        let blocks = builders.pop().unwrap_or_default().finish();
                        if let Some(table) = tables.last_mut() {
                            table.place(blocks, cell_props);
                        }
                    }
                }
                "tbl" => {
                    if let Some(OpenTable { rows, .. }) = tables.pop() {
                        if let Some(builder) = builders.last_mut() {
                            builder.flush_all();
                            if !rows.is_empty() {
                                builder.done.push(Block::Table(rows));
                            }
                        }
                    }
                }
                _ => {}
            },
            Event::Eof => break,
            _ => {}
        }
    }
    // WHY: XML hỏng thiếu thẻ đóng thì phần chữ đang mở vẫn được giữ, không lặng lẽ mất.
    while let Some(OpenTable { rows, .. }) = tables.pop() {
        if let Some(builder) = builders.last_mut() {
            builder.flush_all();
            builder.done.push(Block::Table(rows));
        }
    }
    while builders.len() > 1 {
        let blocks = builders.pop().unwrap_or_default().finish();
        if let Some(builder) = builders.last_mut() {
            builder.done.extend(blocks);
        }
    }
    let body = builders.pop().unwrap_or_default();
    let document = Document {
        blocks: body.finish(),
        footnotes,
        images: Vec::new(),
        dropped: Dropped {
            notes: notes_dropped,
            headers: headers_dropped,
            ..Dropped::default()
        },
    };
    Ok((document, collected))
}

/// Thuộc tính đoạn (`pStyle`, `ilvl`, `numId`) và thuộc tính run (`b`, `i`, `strike`, `u`, `vertAlign`, `rStyle`).
fn properties(e: &BytesStart, paragraph: &mut Paragraph, style: &mut Style, in_run_props: bool) {
    match e.local_name().as_ref() {
        "pStyle" => paragraph.style = attr(e, "val"),
        "ilvl" => paragraph.level = attr(e, "val").and_then(|v| v.parse().ok()).unwrap_or(0),
        "numId" => {
            // WHY: numId 0 nghĩa là bỏ đánh số cho đoạn này.
            paragraph.num = attr(e, "val").filter(|v| v != "0");
        }
        "b" if in_run_props => style.bold = toggle_on(e),
        "i" if in_run_props => style.italic = toggle_on(e),
        "strike" | "dstrike" if in_run_props => style.strike = toggle_on(e),
        // WHY: `w:u` không có `w:val="none"` là gạch chân (Word ghi `single` khi người dùng bật).
        "u" if in_run_props => style.underline = attr(e, "val").as_deref() != Some("none"),
        "vertAlign" if in_run_props => {
            let value = attr(e, "val");
            style.sup = value.as_deref() == Some("superscript");
            style.sub = value.as_deref() == Some("subscript");
        }
        "rStyle" if in_run_props => {
            style.code = attr(e, "val").is_some_and(|v| {
                let v = v.to_ascii_lowercase();
                v.contains("code") || v.contains("verbatim")
            });
        }
        _ => {}
    }
}

const W_NS: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
const R_NS: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const REL_TYPE: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const WP_NS: &str = "http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing";

/// 1 pixel = 9525 EMU ở 96 DPI.
const EMU_PER_PIXEL: u64 = 9525;
/// Rộng tối đa 6 inch (5486400 EMU): vừa khổ trang Word mặc định.
const MAX_WIDTH_EMU: u64 = 5_486_400;

/// Ghi cây tài liệu ra một DOCX tối giản (mở được bằng Word và LibreOffice). Trả số thứ không mang theo
/// được (ảnh không nhúng được, chữ thay thế) để người gọi cảnh báo.
pub fn write(doc: &Document, out: &mut (impl Write + Seek)) -> Result<Dropped> {
    let mut writer = BodyWriter {
        images: &doc.images,
        dropped: doc.dropped,
        ..BodyWriter::default()
    };
    let ctx = Ctx::default();
    for block in &doc.blocks {
        writer.block(block, &ctx);
    }
    // WHY: chú thích viết sau thân; mỗi chú thích thành một mục trong footnotes.xml, đánh số theo thứ tự.
    let note_ctx = Ctx {
        style: Some("FootnoteText"),
        ..Ctx::default()
    };
    for note in &doc.footnotes {
        let saved = std::mem::take(&mut writer.body);
        for block in note {
            writer.block(block, &note_ctx);
        }
        writer
            .notes
            .push(std::mem::replace(&mut writer.body, saved));
    }
    writer.finish(out)
}

/// Ghi Markdown ra DOCX. Trả số ảnh không mang theo được để người gọi cảnh báo.
pub fn markdown_to_docx(markdown: &str, out: &mut (impl Write + Seek)) -> Result<usize> {
    Ok(write(&crate::ops::markdown::read(markdown, None), out)?.images)
}

/// Ngữ cảnh của một khối: style đoạn mặc định, độ sâu danh sách và mức thụt của đoạn tiếp nối.
#[derive(Clone, Copy, Default)]
struct Ctx {
    style: Option<&'static str>,
    depth: usize,
    indent: usize,
}

/// Ảnh đã chuẩn bị để nhúng: đuôi file, bytes và kích thước theo EMU.
struct Media {
    ext: &'static str,
    bytes: Vec<u8>,
    cx: u64,
    cy: u64,
}

#[derive(Default)]
struct BodyWriter<'a> {
    images: &'a [Image],
    body: String,
    /// Danh sách đã dùng: (có số, cấp, số bắt đầu); mỗi danh sách một numId để đánh số lại từ đầu.
    nums: Vec<(bool, usize, u64)>,
    links: Vec<String>,
    /// Ảnh đã nhúng, theo chỉ số trong `images`; mỗi ảnh chỉ có một phần trong gói.
    media: BTreeMap<usize, Media>,
    /// Số mẩu ảnh đã ghi, dùng làm `docPr id` duy nhất.
    pictures: usize,
    /// Nội dung (đoạn XML) của từng chú thích, theo thứ tự đánh số.
    notes: Vec<String>,
    dropped: Dropped,
}

impl BodyWriter<'_> {
    fn block(&mut self, block: &Block, ctx: &Ctx) {
        match block {
            Block::Heading(level, inlines) => {
                let style = format!("Heading{level}");
                let runs = self.runs(inlines);
                self.paragraph(Some(&style), None, 0, &runs);
            }
            Block::Paragraph(inlines) => {
                let runs = self.runs(inlines);
                self.paragraph(ctx.style, None, ctx.indent, &runs);
            }
            Block::List { start, items } => self.list(*start, items, ctx),
            Block::Table(rows) => self.table(rows, ctx),
            Block::Code(code) => {
                for line in code.split('\n') {
                    let runs = text_run(line, Style::default());
                    self.paragraph(Some("Code"), None, ctx.indent, &runs);
                }
            }
            Block::Quote(blocks) => {
                let inner = Ctx {
                    style: Some("Quote"),
                    ..*ctx
                };
                for block in blocks {
                    self.block(block, &inner);
                }
            }
            // WHY: đoạn có viền dưới là đường kẻ ngang; đoạn trống thường sẽ bị bỏ qua.
            Block::Rule => self.body.push_str(
                r#"<w:p><w:pPr><w:pBdr><w:bottom w:val="single" w:sz="6" w:space="1" w:color="auto"/></w:pBdr></w:pPr></w:p>"#,
            ),
        }
    }

    fn list(&mut self, start: Option<u64>, items: &[Vec<Block>], ctx: &Ctx) {
        let level = word_level(ctx.depth);
        self.nums.push((start.is_some(), level, start.unwrap_or(1)));
        let num = u32::try_from(self.nums.len()).unwrap_or(u32::MAX);
        let item_ctx = Ctx {
            style: None,
            depth: ctx.depth + 1,
            indent: ctx.depth + 1,
        };
        for item in items {
            for (index, block) in item.iter().enumerate() {
                match block {
                    // WHY: chỉ đoạn đầu của mục mang số/dấu; đoạn sau thụt để vẫn thuộc mục.
                    Block::Paragraph(inlines) if index == 0 => {
                        let runs = self.runs(inlines);
                        self.paragraph(None, Some((num, level)), 0, &runs);
                    }
                    other => self.block(other, &item_ctx),
                }
            }
        }
    }

    /// Bảng theo lưới: ô gộp ngang là `gridSpan`, ô gộp dọc là `vMerge` (ô gốc `restart`, ô bên dưới `continue`).
    fn table(&mut self, rows: &[Vec<Cell>], ctx: &Ctx) {
        let slots = grid(rows);
        let width = slots.iter().map(Vec::len).max().unwrap_or(0);
        self.body.push_str(
            r#"<w:tbl><w:tblPr><w:tblW w:w="0" w:type="auto"/><w:tblBorders><w:top w:val="single" w:sz="4"/><w:left w:val="single" w:sz="4"/><w:bottom w:val="single" w:sz="4"/><w:right w:val="single" w:sz="4"/><w:insideH w:val="single" w:sz="4"/><w:insideV w:val="single" w:sz="4"/></w:tblBorders></w:tblPr><w:tblGrid>"#,
        );
        for _ in 0..width {
            self.body.push_str("<w:gridCol/>");
        }
        self.body.push_str("</w:tblGrid>");
        let cell_ctx = Ctx {
            style: None,
            depth: ctx.depth,
            indent: ctx.indent,
        };
        for line in &slots {
            self.body.push_str("<w:tr>");
            for slot in line {
                match slot {
                    Slot::Cell(cell) => {
                        let vmerge = (cell.rowspan > 1).then_some(VMerge::Restart);
                        self.body.push_str("<w:tc>");
                        self.body.push_str(&tc_props(cell.colspan, vmerge));
                        // WHY: ô Word phải có ít nhất một đoạn, kể cả ô rỗng.
                        if cell.blocks.is_empty() {
                            self.body.push_str("<w:p/>");
                        }
                        for block in &cell.blocks {
                            self.block(block, &cell_ctx);
                        }
                        // WHY: Word cần ô kết thúc bằng đoạn; bảng lồng hoặc khối không ra đoạn thì thêm đoạn trống.
                        if !self.body.ends_with("</w:p>") && !self.body.ends_with("<w:p/>") {
                            self.body.push_str("<w:p/>");
                        }
                        self.body.push_str("</w:tc>");
                    }
                    // WHY: phần bị gộp dọc chỉ ghi một ô `continue` ở cột trái nhất, rộng bằng ô gốc.
                    Slot::Below {
                        origin,
                        leftmost: true,
                    } => {
                        self.body.push_str("<w:tc>");
                        self.body
                            .push_str(&tc_props(origin.colspan, Some(VMerge::Continue)));
                        self.body.push_str("<w:p/></w:tc>");
                    }
                    Slot::Below { .. } | Slot::Spanned => {}
                    Slot::Empty => self.body.push_str("<w:tc><w:p/></w:tc>"),
                }
            }
            self.body.push_str("</w:tr>");
        }
        self.body.push_str("</w:tbl>");
    }

    fn paragraph(
        &mut self,
        style: Option<&str>,
        numbering: Option<(u32, usize)>,
        indent: usize,
        runs: &str,
    ) {
        if runs.is_empty() && style.is_none() && numbering.is_none() && indent == 0 {
            // WHY: đoạn trống không có gì để hiện: bỏ, như bộ ghi Markdown cũ.
            return;
        }
        let mut props = String::new();
        if let Some(style) = style {
            props.push_str(&format!(r#"<w:pStyle w:val="{style}"/>"#));
        }
        if let Some((num, level)) = numbering {
            props.push_str(&format!(
                r#"<w:numPr><w:ilvl w:val="{level}"/><w:numId w:val="{num}"/></w:numPr>"#
            ));
        } else if indent > 0 {
            props.push_str(&format!(r#"<w:ind w:left="{}"/>"#, 720 * indent));
        }
        self.body
            .push_str(&format!("<w:p><w:pPr>{props}</w:pPr>{runs}</w:p>"));
    }

    fn runs(&mut self, inlines: &[Inline]) -> String {
        let mut out = String::new();
        for inline in inlines {
            match inline {
                Inline::Text(text, style) => out.push_str(&text_run(text, *style)),
                Inline::Link(url, inner) => {
                    self.links.push(url.clone());
                    let id = format!("rIdL{}", self.links.len());
                    let runs = self.runs(inner);
                    out.push_str(&format!(r#"<w:hyperlink r:id="{id}">{runs}</w:hyperlink>"#));
                }
                Inline::Image { index, alt } => {
                    let picture = index.and_then(|index| self.picture(index, alt));
                    match picture {
                        Some(xml) => out.push_str(&xml),
                        // WHY: ảnh không nhúng được: giữ chữ thay thế và đếm để cảnh báo.
                        None => {
                            self.dropped.images += 1;
                            out.push_str(&text_run(alt, Style::default()));
                        }
                    }
                }
                Inline::FootnoteRef(index) => out.push_str(&footnote_reference(*index)),
                Inline::Break => out.push_str("<w:r><w:br/></w:r>"),
            }
        }
        out
    }

    /// Run chứa ảnh inline; `None` khi ảnh không đọc được hoặc không đổi được sang định dạng Word hiểu.
    fn picture(&mut self, index: usize, alt: &str) -> Option<String> {
        if !self.media.contains_key(&index) {
            let media = prepare_media(self.images.get(index)?)?;
            self.media.insert(index, media);
        }
        let media = self.media.get(&index)?;
        let (cx, cy) = (media.cx, media.cy);
        let rel = format!("rIdI{}", index + 1);
        self.pictures += 1;
        let id = self.pictures;
        Some(format!(
            r#"<w:r><w:drawing><wp:inline><wp:extent cx="{cx}" cy="{cy}"/><wp:docPr id="{id}" name="Ảnh {id}" descr="{}"/><a:graphic xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"><a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/picture"><pic:pic xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture"><pic:nvPicPr><pic:cNvPr id="{id}" name="image{id}"/><pic:cNvPicPr/></pic:nvPicPr><pic:blipFill><a:blip r:embed="{rel}"/><a:stretch><a:fillRect/></a:stretch></pic:blipFill><pic:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="{cx}" cy="{cy}"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></pic:spPr></pic:pic></a:graphicData></a:graphic></wp:inline></w:drawing></w:r>"#,
            xml_escape(alt)
        ))
    }

    fn finish(self, out: &mut (impl Write + Seek)) -> Result<Dropped> {
        let document = format!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document xmlns:w="{W_NS}" xmlns:r="{R_NS}" xmlns:wp="{WP_NS}"><w:body>{}<w:sectPr/></w:body></w:document>"#,
            self.body
        );
        // WHY: liên kết và ảnh dùng chung cho thân và chú thích. Chú thích là part riêng, nên quan hệ của
        // chúng phải được khai báo lại trong footnotes.xml.rels, nếu không Word báo file hỏng.
        let mut links_rels = String::new();
        for (index, url) in self.links.iter().enumerate() {
            links_rels.push_str(&format!(
                r#"<Relationship Id="rIdL{}" Type="{REL_TYPE}/hyperlink" Target="{}" TargetMode="External"/>"#,
                index + 1,
                xml_escape(url)
            ));
        }
        let mut image_rels = String::new();
        let mut content_types = CONTENT_TYPES.to_string();
        let mut extensions: Vec<&str> = Vec::new();
        for (index, media) in &self.media {
            image_rels.push_str(&format!(
                r#"<Relationship Id="rIdI{}" Type="{REL_TYPE}/image" Target="media/image{}.{}"/>"#,
                index + 1,
                index + 1,
                media.ext
            ));
            if !extensions.contains(&media.ext) {
                extensions.push(media.ext);
                content_types = content_types.replace(
                    "</Types>",
                    &format!(
                        r#"<Default Extension="{}" ContentType="{}"/></Types>"#,
                        media.ext,
                        mime_of_ext(media.ext)
                    ),
                );
            }
        }
        let has_notes = !self.notes.is_empty();
        let notes_rel = if has_notes {
            content_types = content_types.replace(
                "</Types>",
                r#"<Override PartName="/word/footnotes.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.footnotes+xml"/></Types>"#,
            );
            format!(
                r#"<Relationship Id="rIdF" Type="{REL_TYPE}/footnotes" Target="footnotes.xml"/>"#
            )
        } else {
            String::new()
        };
        let rels = format!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdS" Type="{REL_TYPE}/styles" Target="styles.xml"/><Relationship Id="rIdN" Type="{REL_TYPE}/numbering" Target="numbering.xml"/>{notes_rel}{links_rels}{image_rels}</Relationships>"#
        );
        let mut parts: Vec<(String, Vec<u8>)> = vec![
            (
                "[Content_Types].xml".to_string(),
                content_types.into_bytes(),
            ),
            ("_rels/.rels".to_string(), ROOT_RELS.as_bytes().to_vec()),
            ("word/document.xml".to_string(), document.into_bytes()),
            (
                "word/_rels/document.xml.rels".to_string(),
                rels.into_bytes(),
            ),
            ("word/styles.xml".to_string(), styles_xml().into_bytes()),
            (
                "word/numbering.xml".to_string(),
                numbering_xml(&self.nums).into_bytes(),
            ),
        ];
        if has_notes {
            parts.push((
                "word/footnotes.xml".to_string(),
                footnotes_xml(&self.notes).into_bytes(),
            ));
            let notes_rels = format!(
                r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">{links_rels}{image_rels}</Relationships>"#
            );
            parts.push((
                "word/_rels/footnotes.xml.rels".to_string(),
                notes_rels.into_bytes(),
            ));
        }
        for (index, media) in self.media {
            parts.push((
                format!("word/media/image{}.{}", index + 1, media.ext),
                media.bytes,
            ));
        }
        let mut zip = zip::ZipWriter::new(out);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        for (name, content) in parts {
            zip.start_file(name, options).map_err(zip_error)?;
            zip.write_all(&content).map_err(io_sink_error)?;
        }
        zip.finish().map_err(zip_error)?;
        Ok(self.dropped)
    }
}

/// Kiểu gộp dọc ghi vào `w:vMerge` của một ô.
#[derive(Clone, Copy)]
enum VMerge {
    Restart,
    Continue,
}

/// `w:tcPr` của ô: `gridSpan` khi gộp ngang, `vMerge` khi gộp dọc; không có gì thì chuỗi rỗng.
fn tc_props(colspan: u32, vmerge: Option<VMerge>) -> String {
    let mut props = String::new();
    if colspan > 1 {
        props.push_str(&format!(r#"<w:gridSpan w:val="{colspan}"/>"#));
    }
    match vmerge {
        Some(VMerge::Restart) => props.push_str(r#"<w:vMerge w:val="restart"/>"#),
        Some(VMerge::Continue) => props.push_str("<w:vMerge/>"),
        None => {}
    }
    if props.is_empty() {
        String::new()
    } else {
        format!("<w:tcPr>{props}</w:tcPr>")
    }
}

/// Tham chiếu chú thích trong thân: số nằm trên chữ, nội dung ở footnotes.xml.
fn footnote_reference(index: usize) -> String {
    format!(
        r#"<w:r><w:rPr><w:rStyle w:val="FootnoteReference"/></w:rPr><w:footnoteReference w:id="{}"/></w:r>"#,
        index + 1
    )
}

/// footnotes.xml: hai mục phân cách bắt buộc, rồi mỗi chú thích có dấu số ở đầu.
fn footnotes_xml(notes: &[String]) -> String {
    let mut xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:footnotes xmlns:w="{W_NS}" xmlns:r="{R_NS}"><w:footnote w:type="separator" w:id="-1"><w:p><w:r><w:separator/></w:r></w:p></w:footnote><w:footnote w:type="continuationSeparator" w:id="0"><w:p><w:r><w:continuationSeparator/></w:r></w:p></w:footnote>"#
    );
    for (index, body) in notes.iter().enumerate() {
        xml.push_str(&format!(
            r#"<w:footnote w:id="{}">{}</w:footnote>"#,
            index + 1,
            with_footnote_mark(body)
        ));
    }
    xml.push_str("</w:footnotes>");
    xml
}

/// Dấu số của chú thích đặt trong đoạn đầu tiên, sau thuộc tính đoạn để giữ cấu trúc XML.
fn with_footnote_mark(body: &str) -> String {
    let mark = r#"<w:r><w:rPr><w:rStyle w:val="FootnoteReference"/></w:rPr><w:footnoteRef/></w:r><w:r><w:t xml:space="preserve"> </w:t></w:r>"#;
    if body.starts_with("<w:p><w:pPr>") {
        if let Some(end) = body.find("</w:pPr>") {
            let cut = end + "</w:pPr>".len();
            return format!("{}{mark}{}", &body[..cut], &body[cut..]);
        }
    }
    format!("<w:p>{mark}</w:p>{body}")
}

/// Đổi ảnh sang dạng Word đọc được và đo kích thước. WHY: Word cũ không đọc WebP nên đổi sang PNG.
fn prepare_media(image: &Image) -> Option<Media> {
    let (ext, bytes) = if image.mime == "image/webp" {
        let decoded = image::load_from_memory(&image.bytes).ok()?;
        let mut png = Vec::new();
        decoded
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .ok()?;
        ("png", png)
    } else {
        let ext = match image.mime {
            "image/png" => "png",
            "image/jpeg" => "jpg",
            "image/gif" => "gif",
            _ => "bmp",
        };
        (ext, image.bytes.clone())
    };
    let (width, height) = image::ImageReader::new(std::io::Cursor::new(&bytes))
        .with_guessed_format()
        .ok()?
        .into_dimensions()
        .ok()?;
    let (mut cx, mut cy) = (
        u64::from(width) * EMU_PER_PIXEL,
        u64::from(height) * EMU_PER_PIXEL,
    );
    if cx > MAX_WIDTH_EMU {
        // WHY: u128 vì cy × 5486400 vượt u64 với ảnh rất cao; kết quả luôn nhỏ hơn cy.
        cy = (u128::from(cy) * u128::from(MAX_WIDTH_EMU) / u128::from(cx)) as u64;
        cx = MAX_WIDTH_EMU;
    }
    Some(Media { ext, bytes, cx, cy })
}

fn mime_of_ext(ext: &str) -> &'static str {
    match ext {
        "png" => "image/png",
        "jpg" => "image/jpeg",
        "gif" => "image/gif",
        _ => "image/bmp",
    }
}

/// Một đoạn chữ (run) với định dạng; chuỗi rỗng không tạo run.
fn text_run(text: &str, style: Style) -> String {
    if text.is_empty() {
        return String::new();
    }
    let mut props = String::new();
    if style.code {
        props.push_str(r#"<w:rStyle w:val="CodeChar"/>"#);
    }
    if style.bold {
        props.push_str("<w:b/>");
    }
    if style.italic {
        props.push_str("<w:i/>");
    }
    if style.strike {
        props.push_str("<w:strike/>");
    }
    if style.underline {
        props.push_str(r#"<w:u w:val="single"/>"#);
    }
    if style.sup {
        props.push_str(r#"<w:vertAlign w:val="superscript"/>"#);
    } else if style.sub {
        props.push_str(r#"<w:vertAlign w:val="subscript"/>"#);
    }
    format!(
        r#"<w:r><w:rPr>{props}</w:rPr><w:t xml:space="preserve">{}</w:t></w:r>"#,
        xml_escape(text)
    )
}

const CONTENT_TYPES: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml"/><Override PartName="/word/numbering.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.numbering+xml"/></Types>"#;

const ROOT_RELS: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#;

fn styles_xml() -> String {
    let mut styles = String::from(
        r#"<w:style w:type="paragraph" w:default="1" w:styleId="Normal"><w:name w:val="Normal"/></w:style>"#,
    );
    for level in 1..=6 {
        let size = 40 - 4 * level; // nửa point: 36pt… giảm dần
        styles.push_str(&format!(
            r#"<w:style w:type="paragraph" w:styleId="Heading{level}"><w:name w:val="heading {level}"/><w:basedOn w:val="Normal"/><w:next w:val="Normal"/><w:pPr><w:keepNext/><w:spacing w:before="240" w:after="120"/><w:outlineLvl w:val="{}"/></w:pPr><w:rPr><w:b/><w:sz w:val="{size}"/></w:rPr></w:style>"#,
            level - 1
        ));
    }
    styles.push_str(r#"<w:style w:type="paragraph" w:styleId="Quote"><w:name w:val="Quote"/><w:basedOn w:val="Normal"/><w:pPr><w:ind w:left="720"/></w:pPr><w:rPr><w:i/></w:rPr></w:style>"#);
    styles.push_str(r#"<w:style w:type="paragraph" w:styleId="Code"><w:name w:val="Code"/><w:basedOn w:val="Normal"/><w:pPr><w:spacing w:after="0"/></w:pPr><w:rPr><w:rFonts w:ascii="Consolas" w:hAnsi="Consolas"/></w:rPr></w:style>"#);
    styles.push_str(r#"<w:style w:type="paragraph" w:styleId="FootnoteText"><w:name w:val="footnote text"/><w:basedOn w:val="Normal"/><w:pPr><w:spacing w:after="0"/></w:pPr><w:rPr><w:sz w:val="18"/></w:rPr></w:style>"#);
    styles.push_str(r#"<w:style w:type="character" w:styleId="FootnoteReference"><w:name w:val="footnote reference"/><w:rPr><w:vertAlign w:val="superscript"/></w:rPr></w:style>"#);
    styles.push_str(r#"<w:style w:type="character" w:styleId="CodeChar"><w:name w:val="Code Char"/><w:rPr><w:rFonts w:ascii="Consolas" w:hAnsi="Consolas"/></w:rPr></w:style>"#);
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:styles xmlns:w="{W_NS}">{styles}</w:styles>"#
    )
}

/// Hai định nghĩa (0 = số, 1 = gạch đầu dòng) chín cấp; mỗi danh sách một `w:num` riêng.
fn numbering_xml(nums: &[(bool, usize, u64)]) -> String {
    let mut xml = String::new();
    for (id, ordered) in [(0, true), (1, false)] {
        xml.push_str(&format!(r#"<w:abstractNum w:abstractNumId="{id}">"#));
        for level in 0..9 {
            let (format, text) = if ordered {
                ("decimal", format!("%{}.", level + 1))
            } else {
                ("bullet", "•".to_string())
            };
            xml.push_str(&format!(
                r#"<w:lvl w:ilvl="{level}"><w:start w:val="1"/><w:numFmt w:val="{format}"/><w:lvlText w:val="{text}"/><w:pPr><w:ind w:left="{}" w:hanging="360"/></w:pPr></w:lvl>"#,
                720 * (level + 1)
            ));
        }
        xml.push_str("</w:abstractNum>");
    }
    for (index, (ordered, level, start)) in nums.iter().enumerate() {
        xml.push_str(&format!(
            r#"<w:num w:numId="{}"><w:abstractNumId w:val="{}"/><w:lvlOverride w:ilvl="{level}"><w:startOverride w:val="{start}"/></w:lvlOverride></w:num>"#,
            index + 1,
            if *ordered { 0 } else { 1 }
        ));
    }
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:numbering xmlns:w="{W_NS}">{xml}</w:numbering>"#
    )
}

/// WHY: `numbering.xml` chỉ định nghĩa 9 cấp (0–8); cấp sâu hơn làm Word báo file hỏng.
fn word_level(depth: usize) -> usize {
    depth.min(8)
}

/// Thoát XML và bỏ ký tự điều khiển XML 1.0 cấm. WHY: một ký tự như `\u{c}` (hay có trong chữ lấy
/// từ PDF) làm Word từ chối mở cả file.
fn xml_escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\t' | '\n' | '\r' => out.push(ch),
            '\u{0}'..='\u{1f}' | '\u{fffe}' | '\u{ffff}' => {}
            _ => out.push(ch),
        }
    }
    out
}

/// WHY: ghi vào `out` không gắn với đường dẫn; người gọi (`write_output`) gắn đường dẫn đích.
fn io_sink_error(source: std::io::Error) -> MorphixError {
    MorphixError::Io {
        path: std::path::PathBuf::new(),
        source,
    }
}

fn zip_error(error: zip::result::ZipError) -> MorphixError {
    io_sink_error(std::io::Error::other(error))
}

fn corrupt(path: &Path, reason: impl Into<String>) -> MorphixError {
    MorphixError::Corrupt {
        path: path.to_path_buf(),
        reason: reason.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Cursor, Write};

    const W: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
    const R: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";

    /// Zip DOCX trong bộ nhớ: `word/document.xml` bọc `body`, cộng các mục `extra`.
    fn build_docx(body: &str, extra: &[(&str, &str)]) -> Vec<u8> {
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let options = zip::write::SimpleFileOptions::default();
        writer.start_file("[Content_Types].xml", options).unwrap();
        writer.write_all(b"<Types/>").unwrap();
        writer.start_file("word/document.xml", options).unwrap();
        write!(
            writer,
            r#"<?xml version="1.0" encoding="UTF-8"?><w:document xmlns:w="{W}" xmlns:r="{R}" xmlns:wp="wp"><w:body>{body}</w:body></w:document>"#
        )
        .unwrap();
        for (name, content) in extra {
            writer.start_file(*name, options).unwrap();
            writer.write_all(content.as_bytes()).unwrap();
        }
        writer.finish().unwrap().into_inner()
    }

    fn md(body: &str, extra: &[(&str, &str)]) -> DocxToMarkdown {
        docx_bytes_to_markdown(Path::new("a.docx"), &build_docx(body, extra)).unwrap()
    }

    fn para(text: &str) -> String {
        format!("<w:p><w:r><w:t>{text}</w:t></w:r></w:p>")
    }

    fn corrupt_reason(result: Result<DocxToMarkdown>) -> String {
        match result {
            Err(MorphixError::Corrupt { reason, .. }) => reason,
            other => panic!("cần Corrupt, được {other:?}"),
        }
    }

    #[test]
    fn headings_follow_style_names_even_when_ids_are_translated() {
        let styles = format!(
            r#"<w:styles xmlns:w="{W}"><w:style w:type="paragraph" w:styleId="Kop1"><w:name w:val="heading 1"/></w:style></w:styles>"#
        );
        let body =
            r#"<w:p><w:pPr><w:pStyle w:val="Kop1"/></w:pPr><w:r><w:t>Chào</w:t></w:r></w:p>"#;
        assert_eq!(
            md(body, &[("word/styles.xml", &styles)]).markdown,
            "# Chào\n"
        );
    }

    #[test]
    fn run_formatting_respects_val_false() {
        let body = r#"<w:p><w:r><w:rPr><w:b/></w:rPr><w:t>a</w:t></w:r><w:r><w:t xml:space="preserve"> </w:t></w:r><w:r><w:rPr><w:b w:val="0"/></w:rPr><w:t>b</w:t></w:r><w:r><w:t xml:space="preserve"> </w:t></w:r><w:r><w:rPr><w:i/></w:rPr><w:t>c</w:t></w:r><w:r><w:t xml:space="preserve"> </w:t></w:r><w:r><w:rPr><w:strike/></w:rPr><w:t>d</w:t></w:r></w:p>"#;
        assert_eq!(md(body, &[]).markdown, "**a** b *c* ~~d~~\n");
    }

    #[test]
    fn hyperlink_target_comes_from_rels() {
        let rels = r#"<Relationships><Relationship Id="rId5" Type="hyperlink" Target="https://a.b" TargetMode="External"/></Relationships>"#;
        let body = r#"<w:p><w:hyperlink r:id="rId5"><w:r><w:t>chữ</w:t></w:r></w:hyperlink></w:p>"#;
        assert_eq!(
            md(body, &[("word/_rels/document.xml.rels", rels)]).markdown,
            "[chữ](https://a.b)\n"
        );
    }

    #[test]
    fn numbered_and_bullet_lists_nest_by_ilvl() {
        let numbering = format!(
            r#"<w:numbering xmlns:w="{W}"><w:abstractNum w:abstractNumId="0"><w:lvl w:ilvl="0"><w:numFmt w:val="decimal"/></w:lvl></w:abstractNum><w:abstractNum w:abstractNumId="1"><w:lvl w:ilvl="0"><w:numFmt w:val="bullet"/></w:lvl><w:lvl w:ilvl="1"><w:numFmt w:val="bullet"/></w:lvl></w:abstractNum><w:num w:numId="1"><w:abstractNumId w:val="0"/></w:num><w:num w:numId="2"><w:abstractNumId w:val="1"/></w:num></w:numbering>"#
        );
        let item = |num: u32, level: u32, text: &str| {
            format!(
                r#"<w:p><w:pPr><w:numPr><w:ilvl w:val="{level}"/><w:numId w:val="{num}"/></w:numPr></w:pPr><w:r><w:t>{text}</w:t></w:r></w:p>"#
            )
        };
        let body = format!("{}{}{}", item(2, 0, "a"), item(2, 1, "b"), item(1, 0, "c"));
        assert_eq!(
            md(&body, &[("word/numbering.xml", &numbering)]).markdown,
            "- a\n    - b\n\n1. c\n"
        );
    }

    #[test]
    fn table_becomes_markdown_table() {
        let cell = |t: &str| format!("<w:tc>{}</w:tc>", para(t));
        let body = format!(
            "<w:tbl><w:tr>{}{}</w:tr><w:tr>{}{}</w:tr></w:tbl>",
            cell("a"),
            cell("b"),
            cell("1"),
            cell("2")
        );
        assert_eq!(
            md(&body, &[]).markdown,
            "| a | b |\n| --- | --- |\n| 1 | 2 |\n"
        );
    }

    #[test]
    fn text_with_markdown_characters_is_escaped() {
        assert_eq!(md(&para("a*b &amp; #c"), &[]).markdown, "a\\*b \\& \\#c\n");
    }

    #[test]
    fn breaks_tabs_and_images() {
        let body = r#"<w:p><w:r><w:t>a</w:t><w:br/><w:t>b</w:t><w:tab/><w:t>c</w:t></w:r><w:r><w:drawing><wp:inline><wp:docPr id="1" name="Ảnh 1" descr="logo"/></wp:inline></w:drawing></w:r></w:p>"#;
        let out = md(body, &[]);
        assert_eq!(out.markdown, "a\\\nb\tclogo\n");
        assert_eq!(out.images_dropped, 1);
    }

    #[test]
    fn missing_document_xml_is_corrupt() {
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        writer
            .start_file("x.txt", zip::write::SimpleFileOptions::default())
            .unwrap();
        let bytes = writer.finish().unwrap().into_inner();
        let reason = corrupt_reason(docx_bytes_to_markdown(Path::new("a.docx"), &bytes));
        assert!(reason.contains("word/document.xml"), "{reason}");
    }

    #[test]
    fn not_a_zip_is_corrupt() {
        corrupt_reason(docx_bytes_to_markdown(Path::new("a.docx"), b"nope"));
    }

    #[test]
    fn password_protected_word_file_is_corrupt_with_reason() {
        let ole = [0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1, 0, 0];
        let reason = corrupt_reason(docx_bytes_to_markdown(Path::new("a.docx"), &ole));
        assert!(reason.contains("mật khẩu"), "{reason}");
    }

    #[test]
    fn oversized_part_is_corrupt() {
        let bytes = build_docx(&para("dài hơn bốn byte"), &[]);
        let mut archive = zip::ZipArchive::new(Cursor::new(bytes.as_slice())).unwrap();
        let reason = match read_part(Path::new("a.docx"), &mut archive, "word/document.xml", 4) {
            Err(MorphixError::Corrupt { reason, .. }) => reason,
            other => panic!("cần Corrupt, được {other:?}"),
        };
        assert!(reason.contains("quá lớn"), "{reason}");
    }

    #[test]
    fn text_box_inside_a_paragraph_loses_no_text() {
        let body = r#"<w:p><w:r><w:t>trước</w:t></w:r><w:r><w:drawing><w:txbxContent><w:p><w:r><w:t>trong hộp</w:t></w:r></w:p></w:txbxContent></w:drawing></w:r><w:r><w:t>sau</w:t></w:r></w:p>"#;
        let out = md(body, &[]).markdown;
        for word in ["trước", "trong hộp", "sau"] {
            assert!(out.contains(word), "{word}: {out:?}");
        }
    }

    fn written(markdown: &str) -> Vec<u8> {
        let mut out = Cursor::new(Vec::new());
        markdown_to_docx(markdown, &mut out).unwrap();
        out.into_inner()
    }

    #[test]
    fn markdown_round_trips_through_docx() {
        let bytes = written(crate::ops::markdown::SAMPLE);
        assert_eq!(
            docx_bytes_to_markdown(Path::new("a.docx"), &bytes)
                .unwrap()
                .markdown,
            crate::ops::markdown::SAMPLE
        );
    }

    #[test]
    fn docx_has_required_parts() {
        let bytes = written("x");
        let archive = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
        for part in [
            "[Content_Types].xml",
            "_rels/.rels",
            "word/document.xml",
            "word/_rels/document.xml.rels",
            "word/styles.xml",
            "word/numbering.xml",
        ] {
            assert!(archive.index_for_name(part).is_some(), "{part}");
        }
    }

    #[test]
    fn xml_special_characters_are_escaped() {
        let bytes = written("a < b & \"c\"");
        let md = docx_bytes_to_markdown(Path::new("a.docx"), &bytes)
            .unwrap()
            .markdown;
        assert_eq!(
            crate::ops::markdown::markdown_to_plain(&md),
            "a < b & \"c\"\n"
        );
    }

    fn document_xml(bytes: &[u8]) -> String {
        let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
        read_part(
            Path::new("a.docx"),
            &mut archive,
            "word/document.xml",
            1 << 30,
        )
        .unwrap()
        .unwrap()
    }

    #[test]
    fn control_characters_are_dropped_so_word_can_open_the_file() {
        let bytes = written("form\u{c}feed\u{1}x");
        assert!(!document_xml(&bytes).contains(['\u{c}', '\u{1}']));
        assert_eq!(
            docx_bytes_to_markdown(Path::new("a.docx"), &bytes)
                .unwrap()
                .markdown,
            "formfeedx\n"
        );
    }

    #[test]
    fn very_deep_lists_stay_within_word_levels() {
        let md: String = (0..12)
            .map(|level| format!("{}- l{level}\n", "    ".repeat(level)))
            .collect();
        let xml = document_xml(&written(&md));
        assert!(!xml.contains(r#"w:ilvl w:val="9""#), "{xml}");
    }

    #[test]
    fn alternate_content_text_box_is_read_once() {
        let body = r#"<w:p><w:r><mc:AlternateContent xmlns:mc="mc"><mc:Choice Requires="wps"><w:drawing><w:txbxContent><w:p><w:r><w:t>boxed</w:t></w:r></w:p></w:txbxContent></w:drawing></mc:Choice><mc:Fallback><w:pict><w:txbxContent><w:p><w:r><w:t>boxed</w:t></w:r></w:p></w:txbxContent></w:pict></mc:Fallback></mc:AlternateContent></w:r></w:p>"#;
        let out = md(body, &[]);
        assert_eq!(out.markdown, "boxed\n");
        assert_eq!(out.images_dropped, 1);
    }

    #[test]
    fn line_after_break_starting_with_dash_is_escaped() {
        let body = r#"<w:p><w:r><w:t>a</w:t><w:br/><w:t>- b</w:t></w:r></w:p>"#;
        assert_eq!(md(body, &[]).markdown, "a\\\n\\- b\n");
    }

    #[test]
    fn code_with_backticks_keeps_its_fence_and_span() {
        let styles = format!(
            r#"<w:styles xmlns:w="{W}"><w:style w:styleId="Code"><w:name w:val="Code"/></w:style></w:styles>"#
        );
        let body = r#"<w:p><w:pPr><w:pStyle w:val="Code"/></w:pPr><w:r><w:t>```</w:t></w:r></w:p><w:p><w:r><w:rPr><w:rStyle w:val="CodeChar"/></w:rPr><w:t>a`b</w:t></w:r></w:p>"#;
        let out = md(body, &[("word/styles.xml", &styles)]).markdown;
        assert_eq!(out, "````\n```\n````\n\n``a`b``\n");
    }

    #[test]
    fn non_breaking_hyphen_and_symbols_are_kept() {
        let body = r#"<w:p><w:r><w:t>co</w:t><w:noBreakHyphen/><w:t>op</w:t><w:sym w:font="Symbol" w:char="03A9"/></w:r></w:p>"#;
        assert_eq!(md(body, &[]).markdown, "co\u{2011}op\u{3a9}\n");
    }

    #[test]
    fn link_in_table_cell_is_not_escaped_twice() {
        let rels =
            r#"<Relationships><Relationship Id="rId5" Target="https://a.b"/></Relationships>"#;
        let body = r#"<w:tbl><w:tr><w:tc><w:p><w:r><w:t>h</w:t></w:r></w:p></w:tc></w:tr><w:tr><w:tc><w:p><w:hyperlink r:id="rId5"><w:r><w:t>a*b</w:t></w:r></w:hyperlink></w:p></w:tc></w:tr></w:tbl>"#;
        let out = md(body, &[("word/_rels/document.xml.rels", rels)]).markdown;
        assert!(out.contains("| a\\*b |"), "{out:?}");
    }

    #[test]
    fn markdown_images_are_counted_when_writing_docx() {
        let mut out = Cursor::new(Vec::new());
        let dropped = markdown_to_docx("![logo](x.png) và ![](y.png)", &mut out).unwrap();
        assert_eq!(dropped, 2);
    }

    #[test]
    fn tracked_formatting_changes_use_the_new_formatting() {
        let body = r#"<w:p><w:pPr><w:pPrChange w:id="1"><w:pPr><w:pStyle w:val="Heading1"/></w:pPr></w:pPrChange></w:pPr><w:r><w:rPr><w:rPrChange w:id="2"><w:rPr><w:b/></w:rPr></w:rPrChange></w:rPr><w:t>now normal</w:t></w:r></w:p>"#;
        assert_eq!(md(body, &[]).markdown, "now normal\n");
    }

    #[test]
    fn paragraph_tab_stops_are_not_tab_characters() {
        let cell = |p: &str| format!("<w:tc>{p}</w:tc>");
        let tabbed = r#"<w:p><w:pPr><w:tabs><w:tab w:val="left" w:pos="720"/></w:tabs></w:pPr><w:r><w:t>a</w:t></w:r></w:p>"#;
        let body = format!(
            "<w:tbl><w:tr>{}</w:tr><w:tr>{}</w:tr></w:tbl>",
            cell(&para("h")),
            cell(tabbed)
        );
        assert!(md(&body, &[]).markdown.contains("| a |"));
    }

    #[test]
    fn link_destination_with_spaces_is_wrapped() {
        let rels =
            r#"<Relationships><Relationship Id="rId5" Target="http://x/a b (1)"/></Relationships>"#;
        let body = r#"<w:p><w:hyperlink r:id="rId5"><w:r><w:t>t</w:t></w:r></w:hyperlink></w:p>"#;
        assert_eq!(
            md(body, &[("word/_rels/document.xml.rels", rels)]).markdown,
            "[t](<http://x/a b (1)>)\n"
        );
    }

    #[test]
    fn emphasis_ending_in_punctuation_next_to_a_letter_still_renders() {
        let body =
            r#"<w:p><w:r><w:rPr><w:b/></w:rPr><w:t>(a)</w:t></w:r><w:r><w:t>b</w:t></w:r></w:p>"#;
        assert_eq!(md(body, &[]).markdown, "(**a**)b\n");
    }

    #[test]
    fn raw_html_in_markdown_does_not_leak_tags_into_word() {
        let bytes = written("a <b>raw</b> c\n\n<div>giữ</div>");
        let plain = crate::ops::markdown::markdown_to_plain(
            &docx_bytes_to_markdown(Path::new("a.docx"), &bytes)
                .unwrap()
                .markdown,
        );
        assert!(plain.contains("a raw c"), "{plain:?}");
        assert!(plain.contains("giữ"), "{plain:?}");
        assert!(!plain.contains('<'), "{plain:?}");
    }

    #[test]
    fn notes_comments_and_headers_are_counted() {
        let body = r#"<w:p><w:r><w:t>a</w:t></w:r><w:r><w:footnoteReference w:id="1"/></w:r><w:r><w:commentReference w:id="0"/></w:r></w:p><w:sectPr><w:headerReference w:type="default" r:id="rId9"/></w:sectPr>"#;
        let out = md(body, &[]);
        assert_eq!(out.markdown, "a\n");
        assert_eq!(out.notes_dropped, 2);
        assert!(out.headers_dropped);
    }

    #[test]
    fn empty_paragraphs_are_skipped() {
        let body = format!("<w:p/>{}<w:p></w:p>{}", para("a"), para("b"));
        assert_eq!(md(&body, &[]).markdown, "a\n\nb\n");
    }

    #[test]
    fn read_builds_nested_list_blocks() {
        use crate::ops::doc::{Block, Inline, Style};
        let numbering = format!(
            r#"<w:numbering xmlns:w="{W}"><w:abstractNum w:abstractNumId="0"><w:lvl w:ilvl="0"><w:numFmt w:val="decimal"/></w:lvl></w:abstractNum><w:abstractNum w:abstractNumId="1"><w:lvl w:ilvl="0"><w:numFmt w:val="bullet"/></w:lvl><w:lvl w:ilvl="1"><w:numFmt w:val="bullet"/></w:lvl></w:abstractNum><w:num w:numId="1"><w:abstractNumId w:val="0"/></w:num><w:num w:numId="2"><w:abstractNumId w:val="1"/></w:num></w:numbering>"#
        );
        let item = |num: u32, level: u32, text: &str| {
            format!(
                r#"<w:p><w:pPr><w:numPr><w:ilvl w:val="{level}"/><w:numId w:val="{num}"/></w:numPr></w:pPr><w:r><w:t>{text}</w:t></w:r></w:p>"#
            )
        };
        let body = format!("{}{}{}", item(2, 0, "a"), item(2, 1, "b"), item(1, 0, "c"));
        let bytes = build_docx(&body, &[("word/numbering.xml", &numbering)]);
        let doc = read_from(Path::new("a.docx"), Cursor::new(bytes)).unwrap();
        let text = |s: &str| Inline::Text(s.to_string(), Style::default());
        assert_eq!(
            doc.blocks,
            [
                Block::List {
                    start: None,
                    items: vec![vec![
                        Block::Paragraph(vec![text("a")]),
                        Block::List {
                            start: None,
                            items: vec![vec![Block::Paragraph(vec![text("b")])]],
                        },
                    ]],
                },
                Block::List {
                    start: Some(1),
                    items: vec![vec![Block::Paragraph(vec![text("c")])]],
                },
            ]
        );
    }

    #[test]
    fn list_continuation_paragraph_is_indented_not_numbered() {
        use crate::ops::doc::{Block, Document, Inline, Style};
        let text = |s: &str| Inline::Text(s.to_string(), Style::default());
        let doc = Document {
            blocks: vec![Block::List {
                start: Some(1),
                items: vec![vec![
                    Block::Paragraph(vec![text("a")]),
                    Block::Paragraph(vec![text("b")]),
                ]],
            }],
            ..Document::default()
        };
        let mut out = Cursor::new(Vec::new());
        write(&doc, &mut out).unwrap();
        let xml = document_xml(&out.into_inner());
        assert_eq!(xml.matches("<w:numPr>").count(), 1, "{xml}");
        assert!(xml.contains(r#"<w:ind w:left="720"/>"#), "{xml}");
    }
}

#[cfg(test)]
mod span_tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn vmerge_continue_without_restart_keeps_its_text() {
        let body = r#"<w:tbl><w:tr><w:tc><w:tcPr><w:vMerge/></w:tcPr><w:p><w:r><w:t>x</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>y</w:t></w:r></w:p></w:tc></w:tr></w:tbl>"#;
        let bytes = {
            let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
            writer
                .start_file(
                    "word/document.xml",
                    zip::write::SimpleFileOptions::default(),
                )
                .unwrap();
            std::io::Write::write_all(
                &mut writer,
                format!(r#"<w:document xmlns:w="w"><w:body>{body}</w:body></w:document>"#)
                    .as_bytes(),
            )
            .unwrap();
            writer.finish().unwrap().into_inner()
        };
        let markdown = docx_bytes_to_markdown(Path::new("a.docx"), &bytes)
            .unwrap()
            .markdown;
        assert!(
            markdown.contains('x') && markdown.contains('y'),
            "{markdown}"
        );
    }
}
