use std::{
    io::{Cursor, Write},
    path::{Path, PathBuf},
};

use image::{
    codecs::{avif::AvifEncoder, jpeg::JpegEncoder},
    metadata::Orientation,
    DynamicImage, ImageDecoder, ImageError, ImageFormat, ImageReader, Rgb, RgbImage,
};
use img_parts::{jpeg::Jpeg, png::Png, webp::WebP, Bytes, DynImage, ImageICC};

use crate::{
    error::{MorphixError, Result},
    kind::ImgFormat,
    naming::{desired_output, write_output_like},
    quality::Quality,
};

/// Tốc độ mã hoá AVIF (1 chậm nhất, 10 nhanh nhất); 8 để vòng tròn không bắt người dùng đợi.
const AVIF_SPEED: u8 = 8;

/// Đọc ảnh theo nội dung và **áp dụng hướng EXIF vào điểm ảnh**, để ảnh chụp từ điện thoại
/// không bị nằm ngang sau khi mã hoá lại (các định dạng đầu ra không mang thẻ hướng).
pub fn load_oriented(path: &Path) -> Result<DynamicImage> {
    let io_error = |source| MorphixError::Io {
        path: path.to_path_buf(),
        source,
    };
    let reader = ImageReader::open(path)
        .map_err(io_error)?
        .with_guessed_format()
        .map_err(io_error)?;
    match reader.format() {
        None => {
            return Err(MorphixError::Unsupported {
                path: path.to_path_buf(),
            })
        }
        Some(ImageFormat::Avif) => return decode_avif_with_ffmpeg(path),
        Some(_) => {}
    }
    let classify = |error: ImageError| match error {
        ImageError::Unsupported(_) => MorphixError::Unsupported {
            path: path.to_path_buf(),
        },
        other => MorphixError::Corrupt {
            path: path.to_path_buf(),
            reason: other.to_string(),
        },
    };
    let mut decoder = reader.into_decoder().map_err(classify)?;
    // Không đọc được hướng thì coi như ảnh đã thẳng; không đáng làm hỏng cả việc đổi định dạng.
    let orientation = decoder.orientation().unwrap_or(Orientation::NoTransforms);
    let mut img = DynamicImage::from_decoder(decoder).map_err(classify)?;
    img.apply_orientation(orientation);
    Ok(img)
}

/// WHY: feature `avif` của `image` chỉ có bộ mã hoá; giải mã cần dav1d hệ thống. ffmpeg đã đi
/// kèm gói nên nhờ nó xuất PNG ra stdout. ffmpeg tự áp hướng xoay của AVIF (irot/imir).
fn decode_avif_with_ffmpeg(path: &Path) -> Result<DynamicImage> {
    let ffmpeg = crate::job::require_ffmpeg()?;
    let output = std::process::Command::new(&ffmpeg)
        .args(["-v", "error", "-i"])
        .arg(path)
        .args(["-frames:v", "1", "-f", "image2pipe", "-c:v", "png", "-"])
        .stdin(std::process::Stdio::null())
        .output()
        .map_err(|source| MorphixError::Io {
            path: ffmpeg.clone(),
            source,
        })?;
    if !output.status.success() {
        return Err(MorphixError::Corrupt {
            path: path.to_path_buf(),
            reason: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        });
    }
    image::load_from_memory_with_format(&output.stdout, ImageFormat::Png).map_err(|e| {
        MorphixError::Corrupt {
            path: path.to_path_buf(),
            reason: e.to_string(),
        }
    })
}

/// Dán ảnh có kênh trong suốt lên nền trắng. JPEG và PDF không có alpha, để mặc định thì vùng
/// trong suốt thường ra màu đen.
pub(crate) fn flatten_on_white(img: &DynamicImage) -> RgbImage {
    if !img.color().has_alpha() {
        return img.to_rgb8();
    }
    let rgba = img.to_rgba8();
    RgbImage::from_fn(rgba.width(), rgba.height(), |x, y| {
        let [r, g, b, a] = rgba.get_pixel(x, y).0;
        let blend =
            |c: u8| ((u32::from(c) * u32::from(a) + 255 * (255 - u32::from(a)) + 127) / 255) as u8;
        Rgb([blend(r), blend(g), blend(b)])
    })
}

/// Chuẩn hoá về 8 bit RGB hoặc RGBA: các bộ mã hoá GIF, TIFF, BMP, WebP, AVIF không nhận hết
/// mọi kiểu màu của `image` (16 bit, ảnh xám có alpha...).
fn to_eight_bit(img: &DynamicImage) -> DynamicImage {
    if img.color().has_alpha() {
        DynamicImage::ImageRgba8(img.to_rgba8())
    } else {
        DynamicImage::ImageRgb8(img.to_rgb8())
    }
}

pub fn encode_image(img: &DynamicImage, to: ImgFormat, quality: Quality) -> Result<Vec<u8>> {
    let encode_failure = |detail: String| {
        MorphixError::InvalidInput(format!("không mã hoá được {}: {detail}", to.label()))
    };
    let mut out = Vec::new();
    match to {
        ImgFormat::Jpeg => {
            let flat = flatten_on_white(img);
            JpegEncoder::new_with_quality(&mut out, quality.jpeg())
                .encode_image(&flat)
                .map_err(|e| encode_failure(e.to_string()))?;
        }
        ImgFormat::WebP => {
            let normalized = to_eight_bit(img);
            let encoder = webp::Encoder::from_image(&normalized)
                .map_err(|e| encode_failure(e.to_string()))?;
            out = encoder.encode(quality.webp()).to_vec();
        }
        ImgFormat::Avif => {
            let encoder = AvifEncoder::new_with_speed_quality(&mut out, AVIF_SPEED, quality.avif());
            to_eight_bit(img)
                .write_with_encoder(encoder)
                .map_err(|e| encode_failure(e.to_string()))?;
        }
        ImgFormat::Png => write_with_format(img, ImageFormat::Png, &mut out, &encode_failure)?,
        ImgFormat::Gif => write_with_format(
            &to_eight_bit(img),
            ImageFormat::Gif,
            &mut out,
            &encode_failure,
        )?,
        ImgFormat::Tiff => write_with_format(
            &to_eight_bit(img),
            ImageFormat::Tiff,
            &mut out,
            &encode_failure,
        )?,
        ImgFormat::Bmp => write_with_format(
            &to_eight_bit(img),
            ImageFormat::Bmp,
            &mut out,
            &encode_failure,
        )?,
    }
    Ok(out)
}

fn write_with_format(
    img: &DynamicImage,
    format: ImageFormat,
    out: &mut Vec<u8>,
    failure: &dyn Fn(String) -> MorphixError,
) -> Result<()> {
    img.write_to(&mut Cursor::new(out), format)
        .map_err(|e| failure(e.to_string()))
}

/// Đổi định dạng ảnh. Mã hoá lại nên mọi siêu dữ liệu bị bỏ (chỉ giữ hướng ảnh đã bake vào điểm ảnh);
/// muốn xoá metadata mà không mã hoá lại thì dùng `strip_metadata`.
pub fn convert_image(
    input: &Path,
    to: ImgFormat,
    quality: Quality,
    out_dir: Option<&Path>,
) -> Result<PathBuf> {
    let img = load_oriented(input)?;
    let bytes = with_source_icc(input, encode_image(&img, to, quality)?, to);
    let desired = desired_output(input, "", to.extension(), out_dir);
    write_output_like(&desired, input, |file| {
        file.write_all(&bytes).map_err(|source| MorphixError::Io {
            path: desired.clone(),
            source,
        })
    })
}

/// Chép hồ sơ màu ICC của file gốc sang ảnh vừa mã hoá. Mã hoá lại làm rơi ICC, nên ảnh Display P3
/// từ điện thoại sẽ nhạt đi nếu không chép lại. Điểm ảnh giữ nguyên, chỉ gắn lại nhãn màu.
/// Lỗi thì ghi log và dùng ảnh không có ICC: màu lệch nhẹ còn hơn mất cả việc.
pub(crate) fn with_source_icc(source: &Path, encoded: Vec<u8>, to: ImgFormat) -> Vec<u8> {
    match try_attach_icc(source, &encoded, to) {
        Ok(Some(with_icc)) => with_icc,
        Ok(None) => encoded,
        Err(reason) => {
            tracing::warn!(source = %source.display(), %reason, "không chép được hồ sơ màu ICC");
            encoded
        }
    }
}

fn try_attach_icc(
    source: &Path,
    encoded: &[u8],
    to: ImgFormat,
) -> std::result::Result<Option<Vec<u8>>, String> {
    let source_bytes = std::fs::read(source).map_err(|e| e.to_string())?;
    let Some(source_image) =
        DynImage::from_bytes(Bytes::from(source_bytes)).map_err(|e| e.to_string())?
    else {
        return Ok(None);
    };
    let Some(profile) = source_image.icc_profile() else {
        return Ok(None);
    };
    let bytes = Bytes::copy_from_slice(encoded);
    let mut out = Vec::with_capacity(encoded.len() + profile.len());
    match to {
        ImgFormat::Jpeg => {
            let mut image = Jpeg::from_bytes(bytes).map_err(|e| e.to_string())?;
            image.set_icc_profile(Some(profile));
            image
                .encoder()
                .write_to(&mut out)
                .map_err(|e| e.to_string())?;
        }
        ImgFormat::Png => {
            let mut image = Png::from_bytes(bytes).map_err(|e| e.to_string())?;
            image.set_icc_profile(Some(profile));
            image
                .encoder()
                .write_to(&mut out)
                .map_err(|e| e.to_string())?;
        }
        ImgFormat::WebP => {
            let mut image = WebP::from_bytes(bytes).map_err(|e| e.to_string())?;
            image.set_icc_profile(Some(profile));
            image
                .encoder()
                .write_to(&mut out)
                .map_err(|e| e.to_string())?;
        }
        _ => return Ok(None),
    }
    Ok(Some(out))
}

/// GIF có từ hai khung trở lên. Chỉ giải mã tối đa hai khung đầu để trả lời.
pub(crate) fn is_animated_gif(path: &Path) -> Result<bool> {
    use image::{codecs::gif::GifDecoder, AnimationDecoder};
    let file = std::fs::File::open(path).map_err(|source| MorphixError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let decoder =
        GifDecoder::new(std::io::BufReader::new(file)).map_err(|e| MorphixError::Corrupt {
            path: path.to_path_buf(),
            reason: e.to_string(),
        })?;
    Ok(decoder
        .into_frames()
        .take(2)
        .filter(|frame| frame.is_ok())
        .count()
        > 1)
}
