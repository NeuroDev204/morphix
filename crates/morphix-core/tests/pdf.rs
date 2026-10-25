mod common;

use common::{make_jpeg, make_png, make_png_rgba};
use lopdf::{Document, Object};
use morphix_core::{images_to_pdf, CancelToken, MorphixError};
use tempfile::tempdir;

const A4_LONG: f64 = 841.890;
const A4_SHORT: f64 = 595.276;

/// Kích thước (rộng, cao) của MediaBox ở trang `page` (đếm từ 1).
fn media_box(doc: &Document, page: u32) -> (f64, f64) {
    let id = doc.get_pages()[&page];
    let numbers: Vec<f64> = doc
        .get_dictionary(id)
        .unwrap()
        .get(b"MediaBox")
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .map(|n| f64::from(n.as_float().unwrap()))
        .collect();
    (numbers[2] - numbers[0], numbers[3] - numbers[1])
}

fn assert_box(actual: (f64, f64), expected: (f64, f64)) {
    assert!(
        (actual.0 - expected.0).abs() < 0.01 && (actual.1 - expected.1).abs() < 0.01,
        "{actual:?} != {expected:?}"
    );
}

/// Ba byte RGB đầu tiên của ảnh duy nhất trong tài liệu (sau khi giải nén).
fn first_pixel_rgb(doc: &Document) -> [u8; 3] {
    for object in doc.objects.values() {
        if let Object::Stream(stream) = object {
            let is_image = stream
                .dict
                .get(b"Subtype")
                .ok()
                .and_then(|s| s.as_name().ok())
                == Some(b"Image".as_slice());
            if is_image {
                let data = stream
                    .decompressed_content()
                    .unwrap_or_else(|_| stream.content.clone());
                return [data[0], data[1], data[2]];
            }
        }
    }
    panic!("không có ảnh nào trong PDF");
}

#[test]
fn three_images_make_one_pdf_with_three_pages_and_a4_orientation() {
    let d = tempdir().unwrap();
    let files = vec![
        make_jpeg(d.path(), "IMG_1.jpg", 100, 200, None), // đứng
        make_png(d.path(), "IMG_2.png", 200, 100),        // ngang
        make_png_rgba(d.path(), "IMG_3.png", 50, 50),
    ];
    let out = images_to_pdf(&files, None, &CancelToken::new()).unwrap();
    assert_eq!(out, d.path().join("IMG_1-gop.pdf"));
    let doc = Document::load(&out).unwrap();
    assert_eq!(doc.get_pages().len(), 3);
    assert_box(media_box(&doc, 1), (A4_SHORT, A4_LONG));
    assert_box(media_box(&doc, 2), (A4_LONG, A4_SHORT));
}

#[test]
fn pages_follow_natural_filename_order() {
    let d = tempdir().unwrap();
    let ten = make_png(d.path(), "IMG_10.png", 200, 100); // ngang
    let two = make_jpeg(d.path(), "IMG_2.jpg", 100, 200, None); // đứng
    let out = images_to_pdf(&[ten, two], None, &CancelToken::new()).unwrap();
    assert_eq!(out.file_name().unwrap(), "IMG_2-gop.pdf");
    assert_box(
        media_box(&Document::load(&out).unwrap(), 1),
        (A4_SHORT, A4_LONG),
    );
}

#[test]
fn single_image_is_named_without_suffix() {
    let d = tempdir().unwrap();
    let out = images_to_pdf(
        &[make_png(d.path(), "scan.png", 10, 10)],
        None,
        &CancelToken::new(),
    )
    .unwrap();
    assert_eq!(out, d.path().join("scan.pdf"));
}

#[test]
fn plain_jpeg_is_embedded_without_recompression() {
    let d = tempdir().unwrap();
    let src = make_jpeg(d.path(), "a.jpg", 64, 64, None);
    let out = images_to_pdf(std::slice::from_ref(&src), None, &CancelToken::new()).unwrap();
    let jpeg = std::fs::read(&src).unwrap();
    let pdf = std::fs::read(&out).unwrap();
    assert!(pdf.windows(jpeg.len()).any(|w| w == jpeg.as_slice()));
}

#[test]
fn exif_rotated_jpeg_gets_portrait_page() {
    let d = tempdir().unwrap();
    let src = make_jpeg(d.path(), "p.jpg", 40, 20, Some(6)); // lưu ngang, hiển thị đứng
    let out = images_to_pdf(&[src], None, &CancelToken::new()).unwrap();
    assert_box(
        media_box(&Document::load(&out).unwrap(), 1),
        (A4_SHORT, A4_LONG),
    );
}

#[test]
fn transparent_png_is_flattened_to_white() {
    let d = tempdir().unwrap();
    let out = images_to_pdf(
        &[make_png_rgba(d.path(), "t.png", 4, 4)],
        None,
        &CancelToken::new(),
    )
    .unwrap();
    assert_eq!(
        first_pixel_rgb(&Document::load(&out).unwrap()),
        [255, 255, 255]
    );
}

#[test]
fn bad_input_or_cancel_leaves_no_output() {
    let d = tempdir().unwrap();
    let good = make_png(d.path(), "ok.png", 8, 8);
    let bad = d.path().join("bad.jpg");
    std::fs::write(&bad, b"nope").unwrap();
    let error = images_to_pdf(&[good.clone(), bad], None, &CancelToken::new()).unwrap_err();
    assert!(error.to_string().contains("bad.jpg"), "{error}");

    let token = CancelToken::new();
    token.cancel();
    assert!(matches!(
        images_to_pdf(&[good], None, &token),
        Err(MorphixError::Cancelled)
    ));

    let any_pdf = std::fs::read_dir(d.path())
        .unwrap()
        .any(|f| f.unwrap().path().extension().is_some_and(|e| e == "pdf"));
    assert!(!any_pdf, "không được để lại file PDF");
}
