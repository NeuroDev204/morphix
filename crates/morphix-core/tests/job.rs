mod common;

use std::cell::RefCell;

use common::{make_jpeg, make_png};
use morphix_core::{
    planned_outputs, run_job, CancelToken, ImgFormat, Job, MorphixError, Operation, Progress,
    Quality, ResizeSpec,
};
use tempfile::tempdir;

fn job(inputs: Vec<std::path::PathBuf>, operation: Operation) -> Job {
    Job {
        inputs,
        operation,
        quality: Quality::Balanced,
        out_dir: None,
    }
}

#[test]
fn merging_a_plain_jpeg_does_not_warn_about_the_pdf_frame() {
    let d = tempdir().unwrap();
    let jpeg = make_jpeg(d.path(), "a.jpg", 64, 64, None);
    let report = run_job(
        &job(vec![jpeg], Operation::MergeImagesToPdf),
        &CancelToken::new(),
        &|_| {},
    );
    assert!(report.items[0].result.is_ok());
    assert!(report.items[0].warnings.is_empty());
}

#[test]
fn one_bad_file_does_not_stop_the_batch() {
    let d = tempdir().unwrap();
    let good1 = make_png(d.path(), "a.png", 8, 8);
    let bad = d.path().join("zero.png");
    std::fs::write(&bad, b"").unwrap();
    let good2 = make_jpeg(d.path(), "b.jpg", 8, 8, None);
    let batch = job(
        vec![good1, bad, good2],
        Operation::ConvertImage {
            to: ImgFormat::WebP,
        },
    );
    let seen = RefCell::new(Vec::new());
    let report = run_job(&batch, &CancelToken::new(), &|p| seen.borrow_mut().push(p));
    assert_eq!((report.succeeded(), report.failed()), (2, 1));
    assert!(report.items[1]
        .result
        .as_ref()
        .unwrap_err()
        .to_string()
        .contains("zero.png"));
    assert_eq!(
        seen.borrow().last().copied(),
        Some(Progress { done: 3, total: 3 })
    );
}

#[test]
fn merge_is_a_single_item_and_cancel_marks_everything_cancelled() {
    let d = tempdir().unwrap();
    let files = vec![
        make_png(d.path(), "1.png", 8, 8),
        make_png(d.path(), "2.png", 8, 8),
    ];
    let merge = job(files, Operation::MergeImagesToPdf);
    assert_eq!(run_job(&merge, &CancelToken::new(), &|_| {}).items.len(), 1);
    let token = CancelToken::new();
    token.cancel();
    let report = run_job(&merge, &token, &|_| {});
    assert!(matches!(
        report.items[0].result,
        Err(MorphixError::Cancelled)
    ));
}

#[test]
fn planned_outputs_match_real_names_when_no_collision() {
    let d = tempdir().unwrap();
    let a = make_jpeg(d.path(), "a.jpg", 8, 8, None);
    let strip = job(vec![a.clone()], Operation::StripMetadata);
    let plan = planned_outputs(&strip);
    let real = run_job(&strip, &CancelToken::new(), &|_| {})
        .items
        .remove(0)
        .result
        .unwrap();
    assert_eq!(plan, vec![(vec![a], real)]);
}

#[test]
fn planned_pdf_to_markdown_is_named_md_next_to_the_pdf() {
    let d = tempdir().unwrap();
    let pdf = d.path().join("a.pdf");
    let plan = planned_outputs(&job(vec![pdf.clone()], Operation::PdfToMarkdown));
    assert_eq!(plan, vec![(vec![pdf], d.path().join("a.md"))]);
}

#[test]
fn planned_resize_name_matches_real_name_even_for_rotated_photos() {
    let d = tempdir().unwrap();
    let rotated = make_jpeg(d.path(), "r.jpg", 40, 20, Some(6)); // hiển thị 20x40
    let resize = job(
        vec![rotated],
        Operation::ResizeImage(ResizeSpec::Percent(50)),
    );
    let plan = planned_outputs(&resize);
    let real = run_job(&resize, &CancelToken::new(), &|_| {})
        .items
        .remove(0)
        .result
        .unwrap();
    assert_eq!(plan[0].1, real);
    assert_eq!(real.file_name().unwrap(), "r-10x20.jpg");
}

#[cfg(unix)]
#[test]
fn outputs_inherit_the_source_file_permissions() {
    use std::os::unix::fs::PermissionsExt;
    let d = tempdir().unwrap();
    let src = make_jpeg(d.path(), "a.jpg", 100, 200, None);
    std::fs::set_permissions(&src, std::fs::Permissions::from_mode(0o644)).unwrap();
    let operations = [
        Operation::ConvertImage { to: ImgFormat::Png },
        Operation::StripMetadata,
        Operation::ResizeImage(ResizeSpec::Percent(50)),
        Operation::MergeImagesToPdf,
    ];
    for operation in operations {
        let report = run_job(
            &job(vec![src.clone()], operation.clone()),
            &CancelToken::new(),
            &|_| {},
        );
        let out = report.items[0].result.as_ref().unwrap();
        let mode = std::fs::metadata(out).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o644, "{operation:?} -> {out:?}");
    }
}

#[test]
fn item_count_counts_merge_and_archive_as_one_item() {
    use morphix_core::{item_count, kind::ArchiveFormat};
    let inputs = vec!["a.jpg".into(), "b.jpg".into(), "c.jpg".into()];
    assert_eq!(
        item_count(&job(inputs.clone(), Operation::MergeImagesToPdf)),
        1
    );
    let archive = Operation::CreateArchive {
        format: ArchiveFormat::Zip,
    };
    assert_eq!(item_count(&job(inputs.clone(), archive)), 1);
    assert_eq!(item_count(&job(inputs, Operation::CompressImage)), 3);
}

#[test]
fn dry_run_media_names_use_the_written_lowercase_extension() {
    let d = tempdir().unwrap();
    // WHY: chỉ cần header `ftyp` để nhận diện MP4 theo nội dung; dry-run không chạy ffmpeg.
    let clip = d.path().join("clip.MP4");
    std::fs::write(&clip, b"\0\0\0\x18ftypisom\0\0\x02\0isomiso2avc1mp41").unwrap();
    let plan = planned_outputs(&job(
        vec![clip],
        Operation::CutMedia {
            start_ms: 0,
            end_ms: 1000,
        },
    ));
    assert_eq!(plan[0].1, d.path().join("clip-cat.mp4"));
}

#[test]
fn converting_an_animated_gif_warns_that_only_the_first_frame_is_kept() {
    use image::{codecs::gif::GifEncoder, Frame, RgbaImage};
    let d = tempdir().unwrap();
    let path = d.path().join("anim.gif");
    let mut encoder = GifEncoder::new(std::fs::File::create(&path).unwrap());
    for shade in [0u8, 255] {
        let frame = RgbaImage::from_pixel(8, 8, image::Rgba([shade, shade, shade, 255]));
        encoder.encode_frame(Frame::new(frame)).unwrap();
    }
    drop(encoder);
    let report = run_job(
        &job(
            vec![path],
            Operation::ConvertImage {
                to: ImgFormat::WebP,
            },
        ),
        &CancelToken::new(),
        &|_| {},
    );
    assert!(report.items[0].result.is_ok());
    assert!(
        report.items[0]
            .warnings
            .iter()
            .any(|w| w.contains("khung đầu")),
        "{:?}",
        report.items[0].warnings
    );
}
