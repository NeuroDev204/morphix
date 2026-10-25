use std::{
    ffi::OsString,
    io::{Cursor, Write},
    path::{Path, PathBuf},
    sync::mpsc::{self, RecvTimeoutError},
    time::Duration,
};

use image::{
    codecs::{avif::AvifEncoder, jpeg::JpegEncoder},
    metadata::Orientation,
    DynamicImage, ImageDecoder, ImageError, ImageFormat, ImageReader, Rgb, RgbImage,
};
use img_parts::{jpeg::Jpeg, png::Png, webp::WebP, Bytes, DynImage, ImageICC};

use crate::{
    cancel::CancelToken,
    error::{MorphixError, Result},
    kind::ImgFormat,
    naming::{desired_output, write_output_like},
    ops::ffmpeg::run_ffmpeg,
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

/// WHY: giải mã AVIF chạy ngoài `run_job` nên không có cờ huỷ; giới hạn thời gian để ffmpeg treo
/// không giữ luồng việc (và cả lúc thoát app) mãi mãi. Một khung ảnh thường giải mã dưới một giây.
const AVIF_DECODE_TIMEOUT: Duration = Duration::from_secs(60);

/// WHY: feature `avif` của `image` chỉ có bộ mã hoá; giải mã cần dav1d hệ thống. ffmpeg đã đi
/// kèm gói nên nhờ nó xuất PNG ra file tạm. ffmpeg tự áp hướng xoay của AVIF (irot/imir).
fn decode_avif_with_ffmpeg(path: &Path) -> Result<DynamicImage> {
    let ffmpeg = crate::job::require_ffmpeg()?;
    let work = tempfile::tempdir().map_err(|source| MorphixError::Io {
        path: std::env::temp_dir(),
        source,
    })?;
    let png: PathBuf = work.path().join("decoded.png");
    let mut args: Vec<OsString> = ["-hide_banner", "-nostdin", "-v", "error", "-i"]
        .into_iter()
        .map(OsString::from)
        .collect();
    args.push(path.into());
    args.extend(["-frames:v", "1", "-c:v", "png"].map(OsString::from));
    args.push(png.clone().into());

    // WHY: luồng canh giờ huỷ ffmpeg khi quá hạn; kết thúc sớm thì `done` bị drop và luồng thoát ngay.
    let timeout = CancelToken::new();
    let (done, finished) = mpsc::channel::<()>();
    let watchdog = {
        let timeout = timeout.clone();
        std::thread::spawn(move || {
            if finished.recv_timeout(AVIF_DECODE_TIMEOUT) == Err(RecvTimeoutError::Timeout) {
                timeout.cancel();
            }
        })
    };
    let ran = run_ffmpeg(&ffmpeg, &args, &timeout, &mut |_| {});
    drop(done);
    if watchdog.join().is_err() {
        tracing::error!("luồng canh giờ giải mã AVIF bị panic");
    }
    match ran {
        Ok(()) => {}
        Err(MorphixError::Cancelled) => {
            return Err(MorphixError::Corrupt {
                path: path.to_path_buf(),
                reason: format!(
                    "ffmpeg giải mã quá {} giây, đã dừng",
                    AVIF_DECODE_TIMEOUT.as_secs()
                ),
            })
        }
        Err(MorphixError::Corrupt { reason, .. }) => {
            return Err(MorphixError::Corrupt {
                path: path.to_path_buf(),
                reason,
            })
        }
        Err(other) => return Err(other),
    }
    image::open(&png).map_err(|e| MorphixError::Corrupt {
        path: path.to_path_buf(),
        reason: e.to_string(),
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
    // WHY: điểm ảnh đầu ra luôn là RGB (JPEG và WebP đã dàn phẳng); gắn hồ sơ CMYK hay xám vào ảnh RGB
    // làm trình xem hiện sai màu hoặc từ chối ảnh. Không phải hồ sơ RGB thì bỏ, màu chuẩn sRGB còn hơn.
    // ponytail: PNG xám cũng mất hồ sơ xám; giữ lại khi có người cần.
    if !is_rgb_profile(&profile) {
        return Ok(None);
    }
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

/// Hồ sơ ICC cho không gian màu RGB: trường "data colour space" ở byte 16..20 của header là `RGB `.
fn is_rgb_profile(profile: &[u8]) -> bool {
    profile.get(16..20) == Some(b"RGB ".as_slice())
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

#[cfg(test)]
mod tests {
    use super::*;

    fn profile_with_space(space: &[u8; 4]) -> Vec<u8> {
        let mut profile = vec![0u8; 128];
        profile[16..20].copy_from_slice(space);
        profile
    }

    #[test]
    fn only_rgb_profiles_are_copied_onto_rgb_output() {
        assert!(is_rgb_profile(&profile_with_space(b"RGB ")));
        assert!(!is_rgb_profile(&profile_with_space(b"CMYK")));
        assert!(!is_rgb_profile(&profile_with_space(b"GRAY")));
        assert!(!is_rgb_profile(&[0u8; 10]));
    }
}
