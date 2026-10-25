use std::path::{Path, PathBuf};

use lopdf::{Document, EncryptionState, EncryptionVersion, Object, Permissions};
use morphix_core::ops::pdf_text::pdf_to_text;
use morphix_core::{run_job, CancelToken, Job, MorphixError, Operation, Quality};
use pdf_writer::{Content, Finish, Name, Pdf, Rect, Ref, Str};
use tempfile::tempdir;

/// Ba ký tự Việt không nằm trong bảng WinAnsi của font chuẩn. Các mã 0x80–0x83 được gán
/// sang chữ thật qua ToUnicode, cách mà các trình tạo PDF dùng để chữ vẫn chọn và sao chép được.
const CMAP: &str = "/CIDInit /ProcSet findresource begin 12 dict begin begincmap \
1 begincodespacerange <00> <FF> endcodespacerange \
4 beginbfchar <80> <1EC5> <81> <0103> <82> <01B0> <83> <01A1> endbfchar \
endcmap CMapName currentdict /CMap defineresource pop end end";

/// "Nguyễn Văn A" và "Chương 1" viết bằng font Helvetica, chữ Việt lấy mã 0x80–0x83.
const PAGE_TEXT: &[u8] = b"Nguy\x80n V\x81n A";
const PAGE_TEXT_2: &[u8] = b"Ch\x82\x83ng 1";

/// Dựng PDF; mỗi phần tử là một trang. `Some` là trang có chữ, `None` là trang trống.
fn build_pdf(pages: &[Option<Vec<u8>>]) -> Vec<u8> {
    let mut pdf = Pdf::new();
    let catalog_id = Ref::new(1);
    let pages_id = Ref::new(2);
    let font_id = Ref::new(3);
    let cmap_id = Ref::new(4);
    let page_id = |i: usize| Ref::new(10 + 2 * i as i32);
    let content_id = |i: usize| Ref::new(11 + 2 * i as i32);

    pdf.catalog(catalog_id).pages(pages_id);
    pdf.pages(pages_id)
        .kids((0..pages.len()).map(page_id))
        .count(pages.len() as i32);
    pdf.type1_font(font_id)
        .base_font(Name(b"Helvetica"))
        .encoding_predefined(Name(b"WinAnsiEncoding"))
        .to_unicode(cmap_id);
    pdf.stream(cmap_id, CMAP.as_bytes());

    for (i, page) in pages.iter().enumerate() {
        let mut dict = pdf.page(page_id(i));
        dict.parent(pages_id)
            .media_box(Rect::new(0.0, 0.0, 612.0, 792.0));
        dict.resources().fonts().pair(Name(b"F1"), font_id);
        if let Some(bytes) = page {
            dict.contents(content_id(i));
            dict.finish();
            let mut content = Content::new();
            content
                .begin_text()
                .set_font(Name(b"F1"), 12.0)
                .next_line(72.0, 700.0)
                .show(Str(bytes))
                .end_text();
            pdf.stream(content_id(i), &content.finish());
        } else {
            dict.finish();
        }
    }
    pdf.finish()
}

/// PDF hai trang: trang một có chữ, trang hai là `second`.
fn two_page_pdf(second: Option<Vec<u8>>) -> Vec<u8> {
    build_pdf(&[Some(PAGE_TEXT.to_vec()), second])
}

fn write_file(dir: &Path, name: &str, bytes: &[u8]) -> PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, bytes).unwrap();
    path
}

fn corrupt_reason(error: MorphixError) -> String {
    match error {
        MorphixError::Corrupt { reason, .. } => reason,
        other => panic!("cần lỗi Corrupt, nhận được {other:?}"),
    }
}

#[test]
fn vietnamese_text_layer_comes_out_exact() {
    let dir = tempdir().unwrap();
    let pages = [Some(PAGE_TEXT.to_vec()), Some(PAGE_TEXT_2.to_vec())];
    let path = write_file(dir.path(), "vi.pdf", &build_pdf(&pages));

    let mut out = Vec::new();
    pdf_to_text(&path, &mut out).unwrap();

    let text = String::from_utf8(out).unwrap();
    assert!(text.contains("Nguyễn Văn A"), "chữ đọc ra: {text:?}");
    assert!(text.contains("Chương 1"), "chữ đọc ra: {text:?}");
}

#[test]
fn single_empty_page_reports_page_one_and_writes_nothing() {
    let dir = tempdir().unwrap();
    let path = write_file(dir.path(), "scan.pdf", &build_pdf(&[None]));

    let mut out = Vec::new();
    let reason = corrupt_reason(pdf_to_text(&path, &mut out).unwrap_err());

    assert!(
        reason.contains("trang 1 không có chữ có thể chọn, cần OCR, chưa hỗ trợ"),
        "lý do: {reason}"
    );
    assert!(out.is_empty());
}

#[test]
fn empty_later_page_blocks_the_whole_output() {
    // WHY: trang một đọc được nhưng trang hai thì không. Nếu ghi trang một trước, file
    // đầu ra sẽ cụt; bài kiểm này bảo đảm không byte nào được ghi.
    let dir = tempdir().unwrap();
    let path = write_file(dir.path(), "half.pdf", &two_page_pdf(None));

    let mut out = Vec::new();
    let reason = corrupt_reason(pdf_to_text(&path, &mut out).unwrap_err());

    assert!(
        reason.contains("trang 2 không có chữ có thể chọn, cần OCR, chưa hỗ trợ"),
        "lý do: {reason}"
    );
    assert!(out.is_empty());
}

#[test]
fn bytes_that_are_not_a_pdf_are_reported_as_corrupt() {
    let dir = tempdir().unwrap();
    let path = write_file(dir.path(), "fake.pdf", b"day khong phai PDF");

    let mut out = Vec::new();
    let reason = corrupt_reason(pdf_to_text(&path, &mut out).unwrap_err());

    assert!(
        reason.contains("không đọc được cấu trúc PDF"),
        "lý do: {reason}"
    );
    assert!(out.is_empty());
}

#[test]
fn password_protected_pdf_names_the_password_cause() {
    let dir = tempdir().unwrap();
    let mut document = Document::load_mem(&two_page_pdf(None)).unwrap();
    // WHY: lopdf không tạo khoá mã hoá khi thiếu /ID trong trailer; file thật luôn có /ID.
    let id = Object::string_literal(b"morphix-test-id".to_vec());
    document.trailer.set("ID", vec![id.clone(), id]);
    let state = EncryptionState::try_from(EncryptionVersion::V1 {
        document: &document,
        owner_password: "owner",
        user_password: "user",
        permissions: Permissions::all(),
    })
    .unwrap();
    document.encrypt(&state).unwrap();
    let mut bytes = Vec::new();
    document.save_to(&mut bytes).unwrap();
    let path = write_file(dir.path(), "locked.pdf", &bytes);

    let mut out = Vec::new();
    let reason = corrupt_reason(pdf_to_text(&path, &mut out).unwrap_err());

    assert!(reason.contains("mật khẩu"), "lý do: {reason}");
    assert!(out.is_empty());
}

#[test]
fn missing_file_is_an_io_error() {
    let dir = tempdir().unwrap();
    let mut out = Vec::new();

    let error = pdf_to_text(&dir.path().join("khong-co.pdf"), &mut out).unwrap_err();

    assert!(matches!(error, MorphixError::Io { .. }), "{error:?}");
    assert!(out.is_empty());
}

fn pdf_job(input: &Path) -> Job {
    Job {
        inputs: vec![input.to_path_buf()],
        operation: Operation::PdfToText,
        quality: Quality::Balanced,
        out_dir: None,
    }
}

#[test]
fn pdf_job_writes_txt_next_to_input_and_keeps_source() {
    let dir = tempdir().unwrap();
    let bytes = build_pdf(&[Some(PAGE_TEXT.to_vec()), Some(PAGE_TEXT_2.to_vec())]);
    let input = write_file(dir.path(), "sach.pdf", &bytes);

    let report = run_job(&pdf_job(&input), &CancelToken::new(), &|_| {});

    let output = report.items[0].result.as_ref().unwrap().clone();
    assert_eq!(output, dir.path().join("sach.txt"));
    let text = std::fs::read_to_string(&output).unwrap();
    assert!(text.contains("Nguyễn Văn A"), "chữ đọc ra: {text:?}");
    assert!(text.contains("Chương 1"), "chữ đọc ra: {text:?}");
    assert_eq!(std::fs::read(&input).unwrap(), bytes);
}

#[test]
fn pdf_job_page_without_text_is_corrupt_with_real_path_and_no_output() {
    let dir = tempdir().unwrap();
    let input = write_file(dir.path(), "scan.pdf", &build_pdf(&[None]));

    let report = run_job(&pdf_job(&input), &CancelToken::new(), &|_| {});

    match report.items[0].result.as_ref().unwrap_err() {
        MorphixError::Corrupt { path, reason } => {
            assert_eq!(path, &input);
            assert!(reason.contains("trang 1"), "lý do: {reason}");
        }
        other => panic!("cần Corrupt, nhận được {other:?}"),
    }
    // WHY: chỉ còn file gốc; file tạm của lần ghi phải bị xoá, không để file TXT cụt.
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    assert!(!dir.path().join("scan.txt").exists());
}

#[test]
fn pdf_job_never_overwrites_an_existing_txt() {
    let dir = tempdir().unwrap();
    let input = write_file(
        dir.path(),
        "sach.pdf",
        &build_pdf(&[Some(PAGE_TEXT.to_vec())]),
    );
    let existing = write_file(dir.path(), "sach.txt", "ghi chú cũ".as_bytes());

    let report = run_job(&pdf_job(&input), &CancelToken::new(), &|_| {});

    let output = report.items[0].result.as_ref().unwrap().clone();
    assert_eq!(output, dir.path().join("sach (1).txt"));
    assert_eq!(std::fs::read(&existing).unwrap(), "ghi chú cũ".as_bytes());
}
