// Mỗi file test dùng một phần các hàm này.
#![allow(dead_code)]

use std::path::{Path, PathBuf};

use image::{codecs::jpeg::JpegEncoder, Rgb, RgbImage, Rgba, RgbaImage};
use img_parts::{jpeg::Jpeg, Bytes, ImageEXIF};

pub use morphix_core::exif_with_orientation;

pub fn make_jpeg(dir: &Path, name: &str, w: u32, h: u32, exif_orientation: Option<u16>) -> PathBuf {
    let img = RgbImage::from_fn(w, h, |x, y| {
        Rgb([(x * 7 % 256) as u8, (y * 11 % 256) as u8, 128])
    });
    let mut buf = Vec::new();
    JpegEncoder::new_with_quality(&mut buf, 90)
        .encode_image(&img)
        .unwrap();
    if let Some(orientation) = exif_orientation {
        let mut jpeg = Jpeg::from_bytes(Bytes::from(buf)).unwrap();
        jpeg.set_exif(Some(Bytes::from(exif_with_orientation(orientation))));
        buf = Vec::new();
        jpeg.encoder().write_to(&mut buf).unwrap();
    }
    let path = dir.join(name);
    std::fs::write(&path, buf).unwrap();
    path
}

pub fn make_png(dir: &Path, name: &str, w: u32, h: u32) -> PathBuf {
    let path = dir.join(name);
    RgbImage::from_pixel(w, h, Rgb([10, 120, 200]))
        .save(&path)
        .unwrap();
    path
}

/// PNG có gradient để so điểm ảnh sau khi convert. Alpha (nếu bật) nằm trong 100..=255: libwebp lossless
/// bỏ RGB của điểm ảnh trong suốt hoàn toàn, nên ảnh thử không được có điểm nào như vậy.
pub fn make_png_pattern(dir: &Path, name: &str, w: u32, h: u32, with_alpha: bool) -> PathBuf {
    let path = dir.join(name);
    let rgb = |x: u32, y: u32| {
        [
            (x * 7 % 256) as u8,
            (y * 11 % 256) as u8,
            ((x + y) * 3 % 256) as u8,
        ]
    };
    if with_alpha {
        RgbaImage::from_fn(w, h, |x, y| {
            let [r, g, b] = rgb(x, y);
            Rgba([r, g, b, 100 + ((x + y) % 156) as u8])
        })
        .save(&path)
        .unwrap();
    } else {
        RgbImage::from_fn(w, h, |x, y| Rgb(rgb(x, y)))
            .save(&path)
            .unwrap();
    }
    path
}

/// PNG mà mọi điểm ảnh trong suốt hoàn toàn: dán nền trắng đúng thì ra trắng, sai thì ra đen.
pub fn make_png_rgba(dir: &Path, name: &str, w: u32, h: u32) -> PathBuf {
    let path = dir.join(name);
    RgbaImage::from_pixel(w, h, Rgba([255, 0, 0, 0]))
        .save(&path)
        .unwrap();
    path
}
