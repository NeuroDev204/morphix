use std::{
    io::Write,
    path::{Path, PathBuf},
};

use image::{ImageDecoder, ImageReader};
use img_parts::{
    jpeg::{markers, Jpeg},
    png::Png,
    webp::WebP,
    Bytes, ImageEXIF,
};

use crate::{
    error::{MorphixError, Result},
    kind::{detect, FileKind, ImgFormat},
    naming::{desired_output, extension_of, write_output_like},
};

/// Hậu tố của ảnh đã xoá metadata.
pub const SUFFIX_STRIPPED: &str = "-sach";

/// Hướng ảnh bình thường theo EXIF.
const ORIENTATION_NORMAL: u16 = 1;

/// Payload EXIF (TIFF big-endian) chỉ chứa thẻ Orientation, không có tiền tố `Exif\0\0` (đúng dạng
/// `img-parts::set_exif` nhận). Dùng để xoá hết metadata mà ảnh chụp xoay vẫn hiện thẳng đứng.
pub fn exif_with_orientation(exif_orientation: u16) -> Vec<u8> {
    let mut tiff = Vec::with_capacity(26);
    tiff.extend_from_slice(b"MM\0*\0\0\0\x08"); // big-endian, IFD đầu nằm ở byte 8
    tiff.extend_from_slice(&1u16.to_be_bytes()); // một mục
    tiff.extend_from_slice(&0x0112u16.to_be_bytes()); // thẻ Orientation
    tiff.extend_from_slice(&3u16.to_be_bytes()); // kiểu SHORT
    tiff.extend_from_slice(&1u32.to_be_bytes()); // một giá trị
    tiff.extend_from_slice(&exif_orientation.to_be_bytes());
    tiff.extend_from_slice(&[0, 0]); // đệm cho đủ 4 byte của trường giá trị
    tiff.extend_from_slice(&0u32.to_be_bytes()); // không có IFD kế tiếp
    tiff
}

/// Hướng EXIF của ảnh gốc (1 đến 8). Không đọc được thì coi như thẳng và ghi log, vì xoá metadata
/// vẫn nên chạy tiếp thay vì từ chối cả ảnh.
fn read_exif_orientation(path: &Path) -> u16 {
    let attempt = ImageReader::open(path)
        .and_then(ImageReader::with_guessed_format)
        .map_err(|e| e.to_string())
        .and_then(|reader| reader.into_decoder().map_err(|e| e.to_string()))
        .and_then(|mut decoder| decoder.orientation().map_err(|e| e.to_string()));
    match attempt {
        Ok(orientation) => u16::from(orientation.to_exif()),
        Err(reason) => {
            tracing::warn!(path = %path.display(), %reason, "không đọc được hướng ảnh, coi như thẳng");
            ORIENTATION_NORMAL
        }
    }
}

/// Đoạn JPEG còn giữ khi xoá metadata: mọi đoạn không phải APPn, cộng JFIF (APP0), hồ sơ màu ICC (APP2)
/// và Adobe (APP14). WHY: thiếu đoạn Adobe thì ảnh CMYK/YCCK giải mã sai màu.
fn keep_jpeg_segment(segment: &img_parts::jpeg::JpegSegment) -> bool {
    let contents: &[u8] = segment.contents();
    match segment.marker() {
        markers::APP0 => contents.starts_with(b"JFIF\0"),
        markers::APP2 => contents.starts_with(b"ICC_PROFILE\0"),
        markers::APP14 => contents.starts_with(b"Adobe"),
        marker => !(markers::APP1..=markers::APP15).contains(&marker),
    }
}

/// Xoá metadata (EXIF kể cả GPS, XMP, IPTC, chú thích, văn bản PNG) **mà không mã hoá lại ảnh**, nên
/// điểm ảnh giữ nguyên. Giữ lại hồ sơ màu ICC để màu không đổi, và giữ hướng xoay của ảnh chụp.
pub fn strip_metadata(input: &Path, out_dir: Option<&Path>) -> Result<PathBuf> {
    let format = match detect(input)? {
        FileKind::Image(format) if format.supports_metadata_tools() => format,
        _ => {
            return Err(MorphixError::Unsupported {
                path: input.to_path_buf(),
            })
        }
    };
    let corrupt = |reason: String| MorphixError::Corrupt {
        path: input.to_path_buf(),
        reason,
    };
    let bytes = Bytes::from(std::fs::read(input).map_err(|source| MorphixError::Io {
        path: input.to_path_buf(),
        source,
    })?);
    let orientation = read_exif_orientation(input);
    let keep_orientation = |exif: &mut dyn ImageEXIF| {
        if orientation != ORIENTATION_NORMAL {
            exif.set_exif(Some(Bytes::from(exif_with_orientation(orientation))));
        }
    };

    let mut stripped = Vec::new();
    match format {
        ImgFormat::Jpeg => {
            let mut jpeg = Jpeg::from_bytes(bytes).map_err(|e| corrupt(e.to_string()))?;
            // WHY: mọi APPn và COM đều có thể chứa metadata hay ảnh nhúng (EXIF/XMP ở APP1, MPF ở APP2,
            // IPTC ở APP13...). Chỉ giữ những đoạn cần để giải mã đúng màu.
            jpeg.segments_mut().retain(keep_jpeg_segment);
            jpeg.remove_segments_by_marker(markers::COM);
            keep_orientation(&mut jpeg);
            jpeg.encoder()
                .write_to(&mut stripped)
                .map_err(|e| corrupt(e.to_string()))?;
        }
        ImgFormat::Png => {
            let mut png = Png::from_bytes(bytes).map_err(|e| corrupt(e.to_string()))?;
            for kind in [*b"tEXt", *b"zTXt", *b"iTXt", *b"eXIf"] {
                png.remove_chunks_by_type(kind);
            }
            keep_orientation(&mut png);
            png.encoder()
                .write_to(&mut stripped)
                .map_err(|e| corrupt(e.to_string()))?;
        }
        ImgFormat::WebP => {
            let mut webp = WebP::from_bytes(bytes).map_err(|e| corrupt(e.to_string()))?;
            webp.set_exif(None);
            webp.remove_chunks_by_id(*b"XMP ");
            keep_orientation(&mut webp);
            webp.encoder()
                .write_to(&mut stripped)
                .map_err(|e| corrupt(e.to_string()))?;
        }
        _ => unreachable!("supports_metadata_tools chỉ cho Jpeg, Png, WebP"),
    }

    // Giữ đuôi gốc của người dùng (`.jpeg` vẫn là `.jpeg`).
    let extension = extension_of(input, format.extension());
    let desired = desired_output(input, SUFFIX_STRIPPED, &extension, out_dir);
    write_output_like(&desired, input, |file| {
        file.write_all(&stripped)
            .map_err(|source| MorphixError::Io {
                path: desired.clone(),
                source,
            })
    })
}
