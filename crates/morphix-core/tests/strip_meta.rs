mod common;

use std::path::{Path, PathBuf};

use common::{exif_with_orientation, make_jpeg, make_png};
use img_parts::{
    jpeg::{markers, Jpeg, JpegSegment},
    png::{Png, PngChunk},
    Bytes, ImageEXIF, ImageICC,
};
use morphix_core::{convert_image, strip_metadata, ImgFormat, MorphixError, Quality};
use tempfile::tempdir;

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.windows(needle.len()).any(|w| w == needle)
}

/// JPEG có đủ loại metadata chứa chuỗi "SECRET": EXIF (kèm hướng ảnh), XMP, IPTC, COM, và một ICC giả.
fn jpeg_with_secrets(dir: &Path, name: &str, orientation: Option<u16>) -> PathBuf {
    let path = make_jpeg(dir, name, 32, 16, None);
    let mut jpeg = Jpeg::from_bytes(Bytes::from(std::fs::read(&path).unwrap())).unwrap();
    let mut exif = exif_with_orientation(orientation.unwrap_or(1));
    exif.extend_from_slice(b"SECRET-EXIF"); // dữ liệu thừa sau IFD, parser bỏ qua nhưng quét byte thì thấy
    jpeg.set_exif(Some(Bytes::from(exif)));
    let xmp = b"http://ns.adobe.com/xap/1.0/\0<x:xmpmeta>SECRET-XMP</x:xmpmeta>".to_vec();
    jpeg.segments_mut().insert(
        1,
        JpegSegment::new_with_contents(markers::APP1, Bytes::from(xmp)),
    );
    let iptc = b"Photoshop 3.0\0SECRET-IPTC".to_vec();
    jpeg.segments_mut().insert(
        1,
        JpegSegment::new_with_contents(markers::APP13, Bytes::from(iptc)),
    );
    jpeg.segments_mut().insert(
        1,
        JpegSegment::new_with_contents(markers::COM, Bytes::from_static(b"SECRET-COM")),
    );
    jpeg.set_icc_profile(Some(Bytes::from(vec![7u8; 128])));
    let mut out = Vec::new();
    jpeg.encoder().write_to(&mut out).unwrap();
    std::fs::write(&path, out).unwrap();
    path
}

fn png_with_text_chunk(dir: &Path, name: &str, secret: &str) -> PathBuf {
    let path = make_png(dir, name, 8, 8);
    let mut png = Png::from_bytes(Bytes::from(std::fs::read(&path).unwrap())).unwrap();
    let text = format!("Comment\0{secret}").into_bytes();
    png.chunks_mut()
        .insert(1, PngChunk::new(*b"tEXt", Bytes::from(text)));
    let mut out = Vec::new();
    png.encoder().write_to(&mut out).unwrap();
    std::fs::write(&path, out).unwrap();
    path
}

#[test]
fn jpeg_loses_exif_and_xmp_text_but_keeps_pixels_and_icc() {
    let d = tempdir().unwrap();
    let src = jpeg_with_secrets(d.path(), "a.jpg", None);
    let out = strip_metadata(&src, None).unwrap();
    assert_eq!(out, d.path().join("a-sach.jpg"));
    let bytes = std::fs::read(&out).unwrap();
    assert!(!contains(&bytes, b"SECRET"), "metadata còn sót");
    assert!(Jpeg::from_bytes(Bytes::from(bytes))
        .unwrap()
        .icc_profile()
        .is_some());
    // Không mã hoá lại: điểm ảnh giải mã ra phải y hệt bản gốc.
    assert_eq!(
        image::open(&out).unwrap().to_rgb8(),
        image::open(&src).unwrap().to_rgb8()
    );
}

#[test]
fn rotated_photo_stays_upright_after_strip() {
    let d = tempdir().unwrap();
    let src = jpeg_with_secrets(d.path(), "p.jpg", Some(6));
    let out = strip_metadata(&src, None).unwrap();
    let bytes = std::fs::read(&out).unwrap();
    let reader = exif::Reader::new()
        .read_from_container(&mut std::io::Cursor::new(&bytes))
        .unwrap();
    let orientation = reader
        .get_field(exif::Tag::Orientation, exif::In::PRIMARY)
        .unwrap();
    assert_eq!(orientation.value.get_uint(0), Some(6));
    assert!(!contains(&bytes, b"SECRET"));
}

#[test]
fn png_text_chunks_are_removed() {
    let d = tempdir().unwrap();
    let src = png_with_text_chunk(d.path(), "a.png", "SECRET-PNG");
    let out = strip_metadata(&src, None).unwrap();
    assert!(!contains(&std::fs::read(&out).unwrap(), b"SECRET"));
    assert_eq!(
        image::open(&out).unwrap().to_rgba8(),
        image::open(&src).unwrap().to_rgba8()
    );
}

#[test]
fn unsupported_formats_and_non_images_error_cleanly() {
    let d = tempdir().unwrap();
    let gif = convert_image(
        &make_png(d.path(), "x.png", 4, 4),
        ImgFormat::Gif,
        Quality::Balanced,
        None,
    )
    .unwrap();
    assert!(matches!(
        strip_metadata(&gif, None),
        Err(MorphixError::Unsupported { .. })
    ));
    let text = d.path().join("t.jpg");
    std::fs::write(&text, b"text").unwrap();
    assert!(strip_metadata(&text, None).is_err());
}

#[test]
fn rotated_png_keeps_only_its_orientation_after_strip() {
    let d = tempdir().unwrap();
    let path = make_png(d.path(), "r.png", 60, 30);
    let mut png = Png::from_bytes(Bytes::from(std::fs::read(&path).unwrap())).unwrap();
    let mut exif = exif_with_orientation(6);
    exif.extend_from_slice(b"SECRET-EXIF");
    png.set_exif(Some(Bytes::from(exif)));
    let mut bytes = Vec::new();
    png.encoder().write_to(&mut bytes).unwrap();
    std::fs::write(&path, bytes).unwrap();

    let out = strip_metadata(&path, None).unwrap();
    let stripped = std::fs::read(&out).unwrap();
    assert!(!contains(&stripped, b"SECRET"));
    let kept = Png::from_bytes(Bytes::from(stripped))
        .unwrap()
        .exif()
        .expect("chunk eXIf giữ hướng ảnh phải còn");
    let reader = exif::Reader::new().read_raw(kept.to_vec()).unwrap();
    let orientation = reader
        .get_field(exif::Tag::Orientation, exif::In::PRIMARY)
        .unwrap();
    assert_eq!(orientation.value.get_uint(0), Some(6));
}
