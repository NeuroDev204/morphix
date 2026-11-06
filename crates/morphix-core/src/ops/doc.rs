//! Cây tài liệu chung cho DOCX, HTML và Markdown. Mỗi định dạng đọc ra cây này rồi ghi từ cây; không
//! định dạng nào đi qua định dạng khác.

use std::path::Path;

use base64::{engine::general_purpose::STANDARD, Engine as _};

/// WHY: giới hạn một phần/ảnh đọc từ đĩa, chặn bom nén và file quá lớn đưa vào bộ nhớ.
pub(crate) const MAX_PART_BYTES: u64 = 256 << 20;

/// WHY: `colspan`/`rowspan` lấy từ đầu vào không tin cậy; kẹp để lưới không nổ bộ nhớ.
pub(crate) const MAX_SPAN: u32 = 1000;

/// WHY: số cột tối đa của một lưới; quá mức này là file hỏng, không phải bảng thật.
pub(crate) const MAX_GRID_WIDTH: usize = 1000;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Document {
    pub blocks: Vec<Block>,
    /// Chú thích cuối trang, chỉ số = thứ tự xuất hiện trong tài liệu.
    pub footnotes: Vec<Vec<Block>>,
    /// Ảnh nhúng; `Inline::Image { index }` trỏ vào đây.
    pub images: Vec<Image>,
    /// Những thứ bị bỏ khi đọc hoặc khi ghi, để báo cho người dùng.
    pub dropped: Dropped,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Image {
    pub bytes: Vec<u8>,
    /// Một trong: image/png, image/jpeg, image/gif, image/bmp, image/webp.
    pub mime: &'static str,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Dropped {
    pub images: usize,
    pub notes: usize,
    pub headers: bool,
    /// Đích Markdown không chứa được một định dạng (gạch chân, trên/dưới, ô gộp).
    pub md_downgraded: bool,
}

impl Dropped {
    pub fn add(&mut self, other: Dropped) {
        self.images += other.images;
        self.notes += other.notes;
        self.headers |= other.headers;
        self.md_downgraded |= other.md_downgraded;
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Block {
    Heading(u8, Vec<Inline>),
    Paragraph(Vec<Inline>),
    /// `start`: None = gạch đầu dòng; Some(n) = danh sách số bắt đầu từ n.
    List {
        start: Option<u64>,
        items: Vec<Vec<Block>>,
    },
    Table(Vec<Vec<Cell>>),
    Code(String),
    Quote(Vec<Block>),
    Rule,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Cell {
    pub blocks: Vec<Block>,
    pub colspan: u32,
    pub rowspan: u32,
    pub header: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Style {
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strike: bool,
    pub sup: bool,
    pub sub: bool,
    pub code: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Inline {
    Text(String, Style),
    Link(String, Vec<Inline>),
    /// `index` None = không lấy được ảnh; chỉ giữ chữ thay thế.
    Image {
        index: Option<usize>,
        alt: String,
    },
    FootnoteRef(usize),
    Break,
}

/// Một vị trí trong lưới đã dựng: ô gốc, phần bị ô khác gộp đè lên, hoặc trống để lưới đều cột.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Slot<'a> {
    Cell(&'a Cell),
    /// Bị `rowspan` của ô phía trên đè lên. `leftmost`: cột đầu của ô gốc (người ghi chỉ ghi một lần).
    Below {
        origin: &'a Cell,
        leftmost: bool,
    },
    /// Bị `colspan` của ô bên trái đè lên, cùng một hàng.
    Spanned,
    /// Hàng ngắn hơn lưới; không có ô nào ở vị trí này.
    Empty,
}

/// Dựng lưới: mọi hàng cùng độ rộng. Ô đặt vào cột trống đầu tiên, bỏ qua vị trí đã bị ô phía trên gộp.
pub fn grid(rows: &[Vec<Cell>]) -> Vec<Vec<Slot<'_>>> {
    let mut out: Vec<Vec<Option<Slot<'_>>>> = vec![Vec::new(); rows.len()];
    for (r, row) in rows.iter().enumerate() {
        let mut c = 0;
        for cell in row {
            while out[r].get(c).is_some_and(Option::is_some) {
                c += 1;
            }
            // WHY: giới hạn chiều rộng lưới để một hàng với colspan lớn không nhân ra hàng triệu ô trống.
            let span =
                (cell.colspan.clamp(1, MAX_SPAN) as usize).min(MAX_GRID_WIDTH.saturating_sub(c));
            if span == 0 {
                continue;
            }
            let height = (cell.rowspan.clamp(1, MAX_SPAN) as usize).min(rows.len() - r);
            for dr in 0..height {
                let line = &mut out[r + dr];
                if line.len() < c + span {
                    line.resize(c + span, None);
                }
                for dc in 0..span {
                    line[c + dc] = Some(match (dr, dc) {
                        (0, 0) => Slot::Cell(cell),
                        (0, _) => Slot::Spanned,
                        (_, 0) => Slot::Below {
                            origin: cell,
                            leftmost: true,
                        },
                        _ => Slot::Below {
                            origin: cell,
                            leftmost: false,
                        },
                    });
                }
            }
            c += span;
        }
    }
    let width = out.iter().map(Vec::len).max().unwrap_or(0);
    out.into_iter()
        .map(|line| {
            (0..width)
                .map(|c| line.get(c).copied().flatten().unwrap_or(Slot::Empty))
                .collect()
        })
        .collect()
}

/// Chữ thô của các mẩu: chữ link, chữ thay thế của ảnh; ngắt dòng thành `\n`; chú thích bỏ.
pub fn plain_inlines(inlines: &[Inline]) -> String {
    let mut out = String::new();
    for inline in inlines {
        match inline {
            Inline::Text(text, _) => out.push_str(text),
            Inline::Link(_, inner) => out.push_str(&plain_inlines(inner)),
            Inline::Image { alt, .. } => out.push_str(alt),
            Inline::FootnoteRef(_) => {}
            Inline::Break => out.push('\n'),
        }
    }
    out
}

/// Chữ thô của các khối, mỗi khối một dòng; khối rỗng bỏ.
pub fn plain_blocks(blocks: &[Block]) -> String {
    blocks
        .iter()
        .map(plain_block)
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

fn plain_block(block: &Block) -> String {
    match block {
        Block::Heading(_, inlines) | Block::Paragraph(inlines) => plain_inlines(inlines),
        Block::List { items, .. } => items
            .iter()
            .map(|item| plain_blocks(item))
            .filter(|text| !text.is_empty())
            .collect::<Vec<_>>()
            .join("\n"),
        Block::Table(rows) => rows
            .iter()
            .map(|row| {
                row.iter()
                    .map(|cell| plain_blocks(&cell.blocks))
                    .collect::<Vec<_>>()
                    .join("\t")
            })
            .collect::<Vec<_>>()
            .join("\n"),
        Block::Code(code) => code.clone(),
        Block::Quote(blocks) => plain_blocks(blocks),
        Block::Rule => String::new(),
    }
}

/// Ảnh đọc từ đĩa hoặc từ `data:` URI: đúng năm kiểu ảnh mà Word, HTML và trình duyệt đều xem được.
const KNOWN_IMAGES: [&str; 5] = [
    "image/png",
    "image/jpeg",
    "image/gif",
    "image/bmp",
    "image/webp",
];

/// Thêm ảnh nếu bytes đúng là một trong các kiểu đã biết; trả chỉ số trong `images`.
pub(crate) fn add_image(images: &mut Vec<Image>, bytes: Vec<u8>) -> Option<usize> {
    let mime = infer::get(&bytes)?.mime_type();
    let mime = *KNOWN_IMAGES.iter().find(|known| **known == mime)?;
    images.push(Image { bytes, mime });
    Some(images.len() - 1)
}

/// Nạp ảnh từ `src` của HTML hoặc Markdown: `data:` URI, hoặc file thường nằm trong thư mục `base`.
/// Không nạp URL từ mạng, đường dẫn tuyệt đối, hay file ra ngoài `base` (kể cả qua symlink hay `..`).
/// WHY: một trang lạ không được kéo file tuỳ ý trên máy vào tài liệu đầu ra.
pub(crate) fn load_image_src(
    images: &mut Vec<Image>,
    src: &str,
    base: Option<&Path>,
) -> Option<usize> {
    let bytes = match src.strip_prefix("data:") {
        Some(rest) => {
            let (_, payload) = rest.split_once(";base64,")?;
            // WHY: chặn data URI quá lớn trước khi giải mã; base64 nở ra 4/3 lần.
            if payload.len() as u64 > MAX_PART_BYTES / 3 * 4 + 4 {
                return None;
            }
            STANDARD.decode(payload).ok()?
        }
        None => {
            // WHY: file nhập bằng tên trơ (`ff.html`) cho thư mục cha rỗng; `canonicalize("")` lỗi nên dùng `.`.
            let base = match base? {
                dir if dir.as_os_str().is_empty() => Path::new("."),
                dir => dir,
            };
            if src.contains(':') || Path::new(src).is_absolute() || src.starts_with(['/', '\\']) {
                return None;
            }
            let path = base.join(src).canonicalize().ok()?;
            let root = base.canonicalize().ok()?;
            if !path.starts_with(&root) || !path.is_file() {
                return None;
            }
            if std::fs::metadata(&path).ok()?.len() > MAX_PART_BYTES {
                return None;
            }
            match std::fs::read(&path) {
                Ok(bytes) => bytes,
                Err(error) => {
                    tracing::warn!(%error, ?path, "không đọc được ảnh");
                    return None;
                }
            }
        }
    };
    add_image(images, bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cell(text: &str, colspan: u32, rowspan: u32) -> Cell {
        Cell {
            blocks: vec![Block::Paragraph(vec![Inline::Text(
                text.to_string(),
                Style::default(),
            )])],
            colspan,
            rowspan,
            header: false,
        }
    }

    fn label(slot: &Slot<'_>) -> &'static str {
        match slot {
            Slot::Cell(_) => "cell",
            Slot::Below { leftmost: true, .. } => "below-left",
            Slot::Below {
                leftmost: false, ..
            } => "below",
            Slot::Spanned => "span",
            Slot::Empty => "empty",
        }
    }

    #[test]
    fn grid_marks_spanned_and_below_slots() {
        let rows = vec![
            vec![cell("a", 2, 1), cell("b", 1, 2)],
            vec![cell("c", 1, 1)],
        ];
        let g = grid(&rows);
        let labels: Vec<Vec<&str>> = g
            .iter()
            .map(|line| line.iter().map(label).collect())
            .collect();
        assert_eq!(
            labels,
            [["cell", "span", "cell"], ["cell", "empty", "below-left"]]
        );
    }

    #[test]
    fn grid_pads_short_rows_with_empty_slots() {
        let rows = vec![
            vec![cell("a", 1, 1), cell("b", 1, 1)],
            vec![cell("c", 1, 1)],
        ];
        let g = grid(&rows);
        assert_eq!(g[1].len(), 2);
        assert_eq!(label(&g[1][1]), "empty");
    }

    #[test]
    fn grid_width_is_capped_for_hostile_spans() {
        let rows = vec![(0..2000)
            .map(|_| cell("a", MAX_SPAN, 1))
            .collect::<Vec<Cell>>()];
        let g = grid(&rows);
        assert!(g[0].len() <= MAX_GRID_WIDTH);
    }

    #[test]
    fn oversized_spans_are_clamped_to_the_grid() {
        let rows = vec![vec![cell("a", 99_999_999, 99_999_999)]];
        let g = grid(&rows);
        assert_eq!(g[0].len(), MAX_SPAN as usize);
    }

    #[test]
    fn plain_inlines_takes_link_text_and_alt() {
        let inlines = [
            Inline::Link("u".into(), vec![Inline::Text("x".into(), Style::default())]),
            Inline::Break,
            Inline::Image {
                index: None,
                alt: "y".into(),
            },
        ];
        assert_eq!(plain_inlines(&inlines), "x\ny");
    }

    #[test]
    fn dropped_add_sums_counts_and_ors_flags() {
        let mut total = Dropped {
            images: 1,
            ..Default::default()
        };
        total.add(Dropped {
            images: 2,
            headers: true,
            ..Default::default()
        });
        assert_eq!(
            total,
            Dropped {
                images: 3,
                headers: true,
                ..Default::default()
            }
        );
    }
}
