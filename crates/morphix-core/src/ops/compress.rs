use std::{
    io::Write,
    path::{Path, PathBuf},
};

use image::{
    codecs::png::{CompressionType, FilterType as PngFilter, PngEncoder},
    imageops::FilterType,
    metadata::Orientation,
    DynamicImage, ImageDecoder, ImageReader,
};

use crate::{
    error::{MorphixError, Result},
    kind::{detect, FileKind, ImgFormat},
    naming::{desired_output, extension_of, write_output_like},
    ops::image_io::{encode_image, is_animated_gif, load_oriented, with_source_icc},
    quality::Quality,
};

/// Hậu tố của ảnh đã nén.
pub const SUFFIX_COMPRESSED: &str = "-nen";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResizeSpec {
    /// Phần trăm kích thước gốc, từ 1 đến 100 (không phóng to).
    Percent(u8),
    /// Cạnh dài nhất tối đa, tính bằng điểm ảnh.
    MaxEdge(u32),
}

fn image_format_of(input: &Path) -> Result<ImgFormat> {
    match detect(input)? {
        FileKind::Image(format) => Ok(format),
        _ => Err(MorphixError::Unsupported {
            path: input.to_path_buf(),
        }),
    }
}

fn write_bytes(template: &Path, desired: &Path, bytes: &[u8]) -> Result<PathBuf> {
    write_output_like(desired, template, |file| {
        file.write_all(bytes).map_err(|source| MorphixError::Io {
            path: desired.to_path_buf(),
            source,
        })
    })
}

/// PNG không có thanh chất lượng, nên "nén" nghĩa là mã hoá lại với mức nén cao nhất.
fn encode_png_smallest(img: &DynamicImage) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    let encoder =
        PngEncoder::new_with_quality(&mut out, CompressionType::Best, PngFilter::Adaptive);
    img.write_with_encoder(encoder)
        .map_err(|e| MorphixError::InvalidInput(format!("không mã hoá được PNG: {e}")))?;
    Ok(out)
}

/// Nén ảnh, giữ nguyên định dạng. Mã hoá vào bộ nhớ trước rồi so kích thước: không nhỏ hơn bản gốc
/// thì không tạo file nào (ảnh đã tối ưu thì "nén" chỉ làm to thêm).
pub fn compress_image(input: &Path, quality: Quality, out_dir: Option<&Path>) -> Result<PathBuf> {
    let format = image_format_of(input)?;
    if !format.supports_metadata_tools() {
        return Err(MorphixError::Unsupported {
            path: input.to_path_buf(),
        });
    }
    let original_size = std::fs::metadata(input)
        .map_err(|source| MorphixError::Io {
            path: input.to_path_buf(),
            source,
        })?
        .len();
    let img = load_oriented(input)?;
    let encoded = if format == ImgFormat::Png {
        encode_png_smallest(&img)?
    } else {
        encode_image(&img, format, quality)?
    };
    let bytes = with_source_icc(input, encoded, format);
    if bytes.len() as u64 >= original_size {
        return Err(MorphixError::NotSmaller {
            path: input.to_path_buf(),
        });
    }
    let extension = extension_of(input, format.extension());
    write_bytes(
        input,
        &desired_output(input, SUFFIX_COMPRESSED, &extension, out_dir),
        &bytes,
    )
}

fn scaled(edge: u32, percent: u8) -> u32 {
    ((u64::from(edge) * u64::from(percent) + 50) / 100).max(1) as u32
}

/// Kích thước đích cho `spec`, hoặc lỗi nếu yêu cầu sai hay không làm ảnh nhỏ đi. Hàm thuần, dùng
/// chung cho việc chạy thật và việc đoán tên đầu ra của `--dry-run`.
pub(crate) fn target_dimensions(width: u32, height: u32, spec: ResizeSpec) -> Result<(u32, u32)> {
    let dimensions = match spec {
        ResizeSpec::Percent(percent) if percent == 0 || percent > 100 => {
            return Err(MorphixError::InvalidInput(
                "phần trăm phải từ 1 đến 100".to_string(),
            ))
        }
        ResizeSpec::Percent(percent) => (scaled(width, percent), scaled(height, percent)),
        ResizeSpec::MaxEdge(0) => {
            return Err(MorphixError::InvalidInput(
                "cạnh dài nhất phải lớn hơn 0".to_string(),
            ))
        }
        ResizeSpec::MaxEdge(max_edge) => {
            let longest = width.max(height);
            let scale = |edge: u32| {
                ((u64::from(edge) * u64::from(max_edge) + u64::from(longest) / 2)
                    / u64::from(longest))
                .max(1) as u32
            };
            if max_edge >= longest {
                (width, height)
            } else {
                (scale(width), scale(height))
            }
        }
    };
    if dimensions == (width, height) {
        return Err(MorphixError::InvalidInput(format!(
            "ảnh {width}x{height} đã không lớn hơn kích thước yêu cầu; Morphix không phóng to ảnh"
        )));
    }
    Ok(dimensions)
}

/// Kích thước ảnh khi hiển thị (đã tính hướng EXIF) mà không giải mã cả ảnh.
pub(crate) fn display_dimensions(path: &Path) -> Result<(u32, u32)> {
    let io_error = |source| MorphixError::Io {
        path: path.to_path_buf(),
        source,
    };
    let reader = ImageReader::open(path)
        .map_err(io_error)?
        .with_guessed_format()
        .map_err(io_error)?;
    let mut decoder = reader.into_decoder().map_err(|e| MorphixError::Corrupt {
        path: path.to_path_buf(),
        reason: e.to_string(),
    })?;
    let (width, height) = decoder.dimensions();
    let orientation = decoder.orientation().unwrap_or(Orientation::NoTransforms);
    let swaps = matches!(
        orientation,
        Orientation::Rotate90
            | Orientation::Rotate270
            | Orientation::Rotate90FlipH
            | Orientation::Rotate270FlipH
    );
    Ok(if swaps {
        (height, width)
    } else {
        (width, height)
    })
}

/// Hậu tố tên file của ảnh đã đổi kích thước: kích thước mới, ví dụ `-50x100`.
pub(crate) fn resize_suffix(width: u32, height: u32) -> String {
    format!("-{width}x{height}")
}

/// Đổi kích thước, giữ nguyên định dạng và tỉ lệ. Không bao giờ phóng to: yêu cầu không làm ảnh
/// nhỏ đi thì trả `InvalidInput` nói rõ không phóng to, thay vì tạo một bản sao vô nghĩa.
pub fn resize_image(
    input: &Path,
    spec: ResizeSpec,
    quality: Quality,
    out_dir: Option<&Path>,
) -> Result<PathBuf> {
    let format = image_format_of(input)?;
    if format == ImgFormat::Gif && is_animated_gif(input)? {
        return Err(MorphixError::InvalidInput(format!(
            "{} là GIF động; đổi kích thước sẽ làm mất chuyển động nên chưa hỗ trợ",
            input.display()
        )));
    }
    let img = load_oriented(input)?;
    let (new_width, new_height) = target_dimensions(img.width(), img.height(), spec)?;
    let resized = img.resize_exact(new_width, new_height, FilterType::Lanczos3);
    let bytes = with_source_icc(input, encode_image(&resized, format, quality)?, format);
    let extension = extension_of(input, format.extension());
    let suffix = resize_suffix(new_width, new_height);
    write_bytes(
        input,
        &desired_output(input, &suffix, &extension, out_dir),
        &bytes,
    )
}
