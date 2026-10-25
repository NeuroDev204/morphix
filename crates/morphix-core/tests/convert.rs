mod common;

use common::{make_jpeg, make_png_rgba};
use morphix_core::{convert_image, ImgFormat, MorphixError, Quality};
use tempfile::tempdir;

#[test]
fn jpg_to_png_keeps_dimensions() {
    let d = tempdir().unwrap();
    let src = make_jpeg(d.path(), "a.jpg", 64, 32, None);
    let out = convert_image(&src, ImgFormat::Png, Quality::Balanced, None).unwrap();
    assert_eq!(out, d.path().join("a.png"));
    assert_eq!(image::image_dimensions(&out).unwrap(), (64, 32));
}

#[test]
fn transparent_png_to_jpg_is_white_not_black() {
    let d = tempdir().unwrap();
    let src = make_png_rgba(d.path(), "t.png", 8, 8);
    let out = convert_image(&src, ImgFormat::Jpeg, Quality::High, None).unwrap();
    let px = image::open(&out).unwrap().to_rgb8().get_pixel(4, 4).0;
    assert!(px.iter().all(|c| *c > 240), "pixel was {px:?}");
}

#[test]
fn exif_orientation_is_applied_to_pixels() {
    let d = tempdir().unwrap();
    let src = make_jpeg(d.path(), "p.jpg", 40, 20, Some(6)); // lưu ngang, hiển thị đứng
    let out = convert_image(&src, ImgFormat::Png, Quality::Balanced, None).unwrap();
    assert_eq!(image::image_dimensions(&out).unwrap(), (20, 40));
}

#[test]
fn webp_and_avif_have_correct_container_signature() {
    let d = tempdir().unwrap();
    let src = make_jpeg(d.path(), "a.jpg", 32, 32, None);
    let w =
        std::fs::read(convert_image(&src, ImgFormat::WebP, Quality::Small, None).unwrap()).unwrap();
    assert_eq!((&w[0..4], &w[8..12]), (&b"RIFF"[..], &b"WEBP"[..]));
    let a =
        std::fs::read(convert_image(&src, ImgFormat::Avif, Quality::Small, None).unwrap()).unwrap();
    assert_eq!(&a[4..8], b"ftyp");
}

#[test]
fn avif_source_converts_back() {
    if morphix_core::find_ffmpeg().is_none() {
        eprintln!("bỏ qua: máy test không có ffmpeg");
        return;
    }
    let d = tempdir().unwrap();
    let src = make_jpeg(d.path(), "a.jpg", 32, 24, None);
    let avif = convert_image(&src, ImgFormat::Avif, Quality::Small, None).unwrap();
    let png = convert_image(&avif, ImgFormat::Png, Quality::Balanced, None).unwrap();
    assert_eq!(image::image_dimensions(&png).unwrap(), (32, 24));
}

#[test]
fn unicode_name_with_spaces_and_existing_output_are_safe() {
    let d = tempdir().unwrap();
    let src = make_jpeg(d.path(), "Ảnh chụp màn hình 1.jpg", 16, 16, None);
    std::fs::write(d.path().join("Ảnh chụp màn hình 1.png"), b"keep me").unwrap();
    let out = convert_image(&src, ImgFormat::Png, Quality::Balanced, None).unwrap();
    assert_eq!(out, d.path().join("Ảnh chụp màn hình 1 (1).png"));
    assert_eq!(
        std::fs::read(d.path().join("Ảnh chụp màn hình 1.png")).unwrap(),
        b"keep me"
    );
}

#[test]
fn corrupt_inputs_return_errors_not_panics() {
    let d = tempdir().unwrap();
    let empty = d.path().join("zero.jpg");
    std::fs::write(&empty, b"").unwrap();
    let text = d.path().join("note.jpg");
    std::fs::write(&text, b"just text").unwrap();
    for p in [&empty, &text] {
        assert!(matches!(
            convert_image(p, ImgFormat::Png, Quality::Balanced, None),
            Err(MorphixError::Corrupt { .. }) | Err(MorphixError::Unsupported { .. })
        ));
    }
    assert_eq!(
        std::fs::read_dir(d.path()).unwrap().count(),
        2,
        "không được tạo file đầu ra"
    );
}
