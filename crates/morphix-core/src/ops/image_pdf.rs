use std::{
    io::Write,
    path::{Path, PathBuf},
};

use flate2::{write::ZlibEncoder, Compression};
use image::{metadata::Orientation, ExtendedColorType, ImageDecoder, ImageFormat, ImageReader};
use pdf_writer::{Content, Filter, Finish, Name, Pdf, Rect, Ref};

use crate::{
    cancel::CancelToken,
    error::{MorphixError, Result},
    naming::{desired_output, natural_sort, write_output_like},
    ops::image_io::{flatten_on_white, load_oriented},
};

/// A4 theo điểm PDF (1/72 inch).
const A4_SHORT_PT: f32 = 595.276;
const A4_LONG_PT: f32 = 841.890;

/// Hậu tố của PDF ghép từ nhiều ảnh.
pub const SUFFIX_MERGED_PDF: &str = "-gop";

struct PageImage {
    width: u32,
    height: u32,
    data: Vec<u8>,
    filter: Filter,
    is_gray: bool,
}

/// JPEG thường (hướng bình thường, 1 hoặc 3 kênh) nhúng thẳng vào PDF để không nén lại và mất chất
/// lượng; mọi ảnh khác đi qua giải mã, áp hướng, dán nền trắng rồi nén zlib.
fn prepare_page_image(path: &Path) -> Result<PageImage> {
    if let Some(passthrough) = try_jpeg_passthrough(path)? {
        return Ok(passthrough);
    }
    let img = load_oriented(path)?;
    let rgb = flatten_on_white(&img);
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    let io_error = |source| MorphixError::Io {
        path: path.to_path_buf(),
        source,
    };
    encoder.write_all(rgb.as_raw()).map_err(io_error)?;
    Ok(PageImage {
        width: rgb.width(),
        height: rgb.height(),
        data: encoder.finish().map_err(io_error)?,
        filter: Filter::FlateDecode,
        is_gray: false,
    })
}

fn try_jpeg_passthrough(path: &Path) -> Result<Option<PageImage>> {
    let io_error = |source| MorphixError::Io {
        path: path.to_path_buf(),
        source,
    };
    let reader = ImageReader::open(path)
        .map_err(io_error)?
        .with_guessed_format()
        .map_err(io_error)?;
    if reader.format() != Some(ImageFormat::Jpeg) {
        return Ok(None);
    }
    // Không giải mã được ở đây thì để `load_oriented` báo lỗi có đường dẫn và nguyên nhân.
    let Ok(mut decoder) = reader.into_decoder() else {
        return Ok(None);
    };
    let is_upright =
        decoder.orientation().unwrap_or(Orientation::NoTransforms) == Orientation::NoTransforms;
    let is_gray = match decoder.original_color_type() {
        ExtendedColorType::L8 => true,
        ExtendedColorType::Rgb8 => false,
        _ => return Ok(None), // CMYK và các kiểu khác: nhúng thẳng sẽ sai màu
    };
    if !is_upright {
        return Ok(None);
    }
    let (width, height) = decoder.dimensions();
    let data = std::fs::read(path).map_err(io_error)?;
    Ok(Some(PageImage {
        width,
        height,
        data,
        filter: Filter::DctDecode,
        is_gray,
    }))
}

/// Tên PDF mong muốn cho danh sách ảnh **đã sắp xếp** (không rỗng): một ảnh thì `<stem>.pdf`, nhiều
/// ảnh thì `<stem ảnh đầu>-gop.pdf`. Dùng chung cho việc chạy thật và `--dry-run`.
pub(crate) fn pdf_destination(sorted: &[PathBuf], out_dir: Option<&Path>) -> PathBuf {
    let suffix = if sorted.len() > 1 {
        SUFFIX_MERGED_PDF
    } else {
        ""
    };
    desired_output(&sorted[0], suffix, "pdf", out_dir)
}

/// Gộp ảnh thành **một** PDF khổ A4, mỗi ảnh một trang, xếp theo tên file (sắp xếp tự nhiên).
/// Trang tự chọn hướng theo ảnh; ảnh co vừa trang, giữ tỉ lệ, đặt giữa, không cắt, không lề.
pub fn images_to_pdf(
    inputs: &[PathBuf],
    out_dir: Option<&Path>,
    cancel: &CancelToken,
) -> Result<PathBuf> {
    if inputs.is_empty() {
        return Err(MorphixError::InvalidInput(
            "không có ảnh nào để gộp".to_string(),
        ));
    }
    let mut sorted = inputs.to_vec();
    natural_sort(&mut sorted);

    let mut pdf = Pdf::new();
    let catalog_id = Ref::new(1);
    let page_tree_id = Ref::new(2);
    let mut page_ids = Vec::with_capacity(sorted.len());

    for (index, path) in sorted.iter().enumerate() {
        cancel.check()?;
        let image = prepare_page_image(path)?;
        let base = 3 + 3 * i32::try_from(index).unwrap_or(i32::MAX / 4);
        let (page_id, content_id, image_id) =
            (Ref::new(base), Ref::new(base + 1), Ref::new(base + 2));
        page_ids.push(page_id);

        let (image_w, image_h) = (image.width as f32, image.height as f32);
        let (page_w, page_h) = if image_w > image_h {
            (A4_LONG_PT, A4_SHORT_PT)
        } else {
            (A4_SHORT_PT, A4_LONG_PT)
        };
        let scale = (page_w / image_w).min(page_h / image_h);
        let (draw_w, draw_h) = (image_w * scale, image_h * scale);

        let mut page = pdf.page(page_id);
        page.media_box(Rect::new(0.0, 0.0, page_w, page_h));
        page.parent(page_tree_id);
        page.contents(content_id);
        page.resources().x_objects().pair(Name(b"Im0"), image_id);
        page.finish();

        let mut xobject = pdf.image_xobject(image_id, &image.data);
        xobject.filter(image.filter);
        xobject.width(i32::try_from(image.width).unwrap_or(i32::MAX));
        xobject.height(i32::try_from(image.height).unwrap_or(i32::MAX));
        if image.is_gray {
            xobject.color_space().device_gray();
        } else {
            xobject.color_space().device_rgb();
        }
        xobject.bits_per_component(8);
        xobject.finish();

        let mut content = Content::new();
        content.save_state();
        content.transform([
            draw_w,
            0.0,
            0.0,
            draw_h,
            (page_w - draw_w) / 2.0,
            (page_h - draw_h) / 2.0,
        ]);
        content.x_object(Name(b"Im0"));
        content.restore_state();
        pdf.stream(content_id, &content.finish());
    }

    pdf.catalog(catalog_id).pages(page_tree_id);
    pdf.pages(page_tree_id)
        .kids(page_ids.iter().copied())
        .count(i32::try_from(page_ids.len()).unwrap_or(i32::MAX));
    let bytes = pdf.finish();

    let desired = pdf_destination(&sorted, out_dir);
    write_output_like(&desired, &sorted[0], |file| {
        file.write_all(&bytes).map_err(|source| MorphixError::Io {
            path: desired.clone(),
            source,
        })
    })
}
