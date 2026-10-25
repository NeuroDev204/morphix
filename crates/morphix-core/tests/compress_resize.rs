mod common;

use std::path::{Path, PathBuf};

use common::{make_jpeg, make_png};
use image::{codecs::jpeg::JpegEncoder, Rgb, RgbImage};
use img_parts::{jpeg::Jpeg, png::Png, Bytes, ImageICC};
use morphix_core::{
    compress_image, convert_image, resize_image, ImgFormat, MorphixError, Quality, ResizeSpec,
};
use tempfile::tempdir;

/// JPEG từ ảnh nhiễu giả ngẫu nhiên (xác định, không cần crate `rand`): nén được rất nhiều khi hạ chất lượng.
fn noisy_jpeg(dir: &Path, name: &str, w: u32, h: u32, quality: u8) -> PathBuf {
    let mut state: u32 = 0x1234_5678;
    let mut next = || {
        state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (state >> 24) as u8
    };
    let img = RgbImage::from_fn(w, h, |_, _| Rgb([next(), next(), next()]));
    let mut buf = Vec::new();
    JpegEncoder::new_with_quality(&mut buf, quality)
        .encode_image(&img)
        .unwrap();
    let path = dir.join(name);
    std::fs::write(&path, buf).unwrap();
    path
}

fn size(path: &Path) -> u64 {
    std::fs::metadata(path).unwrap().len()
}

#[test]
fn compress_shrinks_a_high_quality_jpeg_and_keeps_format() {
    let d = tempdir().unwrap();
    let src = noisy_jpeg(d.path(), "big.jpg", 800, 600, 98);
    let out = compress_image(&src, Quality::Small, None).unwrap();
    assert_eq!(out, d.path().join("big-nen.jpg"));
    assert!(size(&out) < size(&src));
}

#[test]
fn compress_that_cannot_shrink_creates_nothing() {
    let d = tempdir().unwrap();
    let src = noisy_jpeg(d.path(), "small.jpg", 8, 8, 20);
    assert!(matches!(
        compress_image(&src, Quality::High, None),
        Err(MorphixError::NotSmaller { .. })
    ));
    assert_eq!(std::fs::read_dir(d.path()).unwrap().count(), 1);
}

#[test]
fn resize_percent_and_max_edge_set_expected_dimensions_and_names() {
    let d = tempdir().unwrap();
    let src = make_jpeg(d.path(), "a.jpg", 100, 200, None);
    let half = resize_image(&src, ResizeSpec::Percent(50), Quality::Balanced, None).unwrap();
    assert_eq!(half, d.path().join("a-50x100.jpg"));
    assert_eq!(image::image_dimensions(&half).unwrap(), (50, 100));
    let wide = make_png(d.path(), "w.png", 200, 100);
    let capped = resize_image(&wide, ResizeSpec::MaxEdge(80), Quality::Balanced, None).unwrap();
    assert_eq!(image::image_dimensions(&capped).unwrap(), (80, 40));
}

#[test]
fn resize_never_upscales_and_rejects_bad_specs() {
    let d = tempdir().unwrap();
    let src = make_png(d.path(), "a.png", 200, 100);
    // WHY: "không nén thêm được" sai nghĩa với đổi kích thước; người dùng cần biết là không phóng to.
    match resize_image(&src, ResizeSpec::MaxEdge(500), Quality::Balanced, None) {
        Err(MorphixError::InvalidInput(message)) => {
            assert!(message.contains("không phóng to"), "{message}")
        }
        other => panic!("cần InvalidInput báo không phóng to, được {other:?}"),
    }
    assert!(matches!(
        resize_image(&src, ResizeSpec::Percent(0), Quality::Balanced, None),
        Err(MorphixError::InvalidInput(_))
    ));
    assert!(matches!(
        resize_image(&src, ResizeSpec::Percent(150), Quality::Balanced, None),
        Err(MorphixError::InvalidInput(_))
    ));
    assert!(matches!(
        resize_image(&src, ResizeSpec::MaxEdge(0), Quality::Balanced, None),
        Err(MorphixError::InvalidInput(_))
    ));
}

#[test]
fn recompressed_outputs_keep_the_source_icc_profile() {
    let d = tempdir().unwrap();
    let src = noisy_jpeg(d.path(), "p3.jpg", 400, 300, 98);
    let mut profile = vec![7u8; 128];
    // WHY: chỉ hồ sơ RGB được chép sang ảnh đầu ra (luôn RGB), nên header giả phải khai không gian RGB.
    profile[16..20].copy_from_slice(b"RGB ");
    let mut jpeg = Jpeg::from_bytes(Bytes::from(std::fs::read(&src).unwrap())).unwrap();
    jpeg.set_icc_profile(Some(Bytes::from(profile.clone())));
    let mut bytes = Vec::new();
    jpeg.encoder().write_to(&mut bytes).unwrap();
    std::fs::write(&src, bytes).unwrap();

    let compressed = compress_image(&src, Quality::Small, None).unwrap();
    let resized = resize_image(&src, ResizeSpec::Percent(50), Quality::Balanced, None).unwrap();
    for out in [compressed, resized] {
        let icc = Jpeg::from_bytes(Bytes::from(std::fs::read(&out).unwrap()))
            .unwrap()
            .icc_profile();
        assert_eq!(icc.map(|b| b.to_vec()), Some(profile.clone()), "{out:?}");
    }
    let as_png = convert_image(&src, ImgFormat::Png, Quality::Balanced, None).unwrap();
    let icc = Png::from_bytes(Bytes::from(std::fs::read(&as_png).unwrap()))
        .unwrap()
        .icc_profile();
    assert_eq!(icc.map(|b| b.to_vec()), Some(profile));
}

fn animated_gif(dir: &Path, name: &str) -> PathBuf {
    use image::{codecs::gif::GifEncoder, Frame, RgbaImage};
    let path = dir.join(name);
    let mut encoder = GifEncoder::new(std::fs::File::create(&path).unwrap());
    let frames = [[255, 0, 0, 255], [0, 0, 255, 255]]
        .map(|rgba| Frame::new(RgbaImage::from_pixel(20, 20, image::Rgba(rgba))));
    encoder.encode_frames(frames).unwrap();
    path
}

#[test]
fn animated_gif_is_not_silently_flattened_by_resize() {
    let d = tempdir().unwrap();
    let anim = animated_gif(d.path(), "anim.gif");
    assert!(matches!(
        resize_image(&anim, ResizeSpec::Percent(50), Quality::Balanced, None),
        Err(MorphixError::InvalidInput(_))
    ));
    assert_eq!(std::fs::read_dir(d.path()).unwrap().count(), 1);
    // GIF một khung vẫn đổi kích thước bình thường.
    let still = convert_image(
        &common::make_png(d.path(), "s.png", 40, 40),
        ImgFormat::Gif,
        Quality::Balanced,
        None,
    )
    .unwrap();
    assert!(resize_image(&still, ResizeSpec::Percent(50), Quality::Balanced, None).is_ok());
}
