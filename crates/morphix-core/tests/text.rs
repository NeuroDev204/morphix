use std::path::PathBuf;

use morphix_core::catalog::{actions_for, ActionKind};
use morphix_core::error::MorphixError;
use morphix_core::job::{run_job, Job, Operation};
use morphix_core::kind::{FileKind, TextFormat};
use morphix_core::ops::text::{
    read_cues, read_table, strip_timestamps, write_cues, write_table, Cue,
};
use morphix_core::{CancelToken, Quality};
use tempfile::{tempdir, TempDir};

const UTF8_BOM: &[u8] = &[0xEF, 0xBB, 0xBF];

fn write_bytes(dir: &TempDir, name: &str, bytes: &[u8]) -> PathBuf {
    let path: PathBuf = dir.path().join(name);
    std::fs::write(&path, bytes).expect("ghi file test");
    path
}

fn cue(start_ms: u64, end_ms: u64, text: &str) -> Cue {
    Cue {
        start_ms,
        end_ms,
        text: text.to_string(),
    }
}

fn rows(data: &[&[&str]]) -> Vec<Vec<String>> {
    data.iter()
        .map(|row| row.iter().map(|cell| cell.to_string()).collect())
        .collect()
}

fn written_srt_cues(cues: &[Cue], format: TextFormat) -> Vec<u8> {
    let mut buf: Vec<u8> = Vec::new();
    write_cues(cues, format, &mut buf).expect("ghi phụ đề");
    buf
}

#[test]
fn srt_with_bom_and_multiline_cue_roundtrips() {
    let dir: TempDir = tempdir().expect("thư mục tạm");
    let input: &[u8] = "\u{feff}1\n00:00:01,000 --> 00:00:03,500\nDòng một\nDòng hai\n\n2\n00:00:04,000 --> 00:00:05,000\nCâu hai"
        .as_bytes();
    let path: PathBuf = write_bytes(&dir, "a.srt", input);

    let cues: Vec<Cue> = read_cues(&path).expect("đọc SRT");
    assert_eq!(
        cues,
        vec![
            cue(1000, 3500, "Dòng một\nDòng hai"),
            cue(4000, 5000, "Câu hai"),
        ]
    );

    let back: Vec<u8> = written_srt_cues(&cues, TextFormat::Srt);
    let back_path: PathBuf = write_bytes(&dir, "b.srt", &back);
    assert_eq!(read_cues(&back_path).expect("đọc lại SRT"), cues);
}

#[test]
fn srt_to_vtt_keeps_times_and_uses_dot() {
    let dir: TempDir = tempdir().expect("thư mục tạm");
    let path: PathBuf = write_bytes(
        &dir,
        "a.srt",
        "1\n00:00:01,000 --> 00:00:03,500\nXin chào\n".as_bytes(),
    );
    let cues: Vec<Cue> = read_cues(&path).expect("đọc SRT");

    let mut buf: Vec<u8> = Vec::new();
    write_cues(&cues, TextFormat::Vtt, &mut buf).expect("ghi VTT");
    let vtt: String = String::from_utf8(buf).expect("VTT là UTF-8");

    assert!(vtt.starts_with("WEBVTT\n\n"), "thiếu header: {vtt}");
    assert!(
        vtt.contains("00:00:01.000 --> 00:00:03.500"),
        "mốc sai: {vtt}"
    );
    assert!(!vtt.contains(','), "còn dấu phẩy trong mốc: {vtt}");
}

#[test]
fn vtt_skips_header_and_note_blocks() {
    let dir: TempDir = tempdir().expect("thư mục tạm");
    let path: PathBuf = write_bytes(
        &dir,
        "a.vtt",
        "WEBVTT\n\nNOTE ghi chú\nkhông phải phụ đề\n\n00:01.000 --> 00:02.000\nChào\n".as_bytes(),
    );
    assert_eq!(
        read_cues(&path).expect("đọc VTT"),
        vec![cue(1000, 2000, "Chào")]
    );
}

#[test]
fn strip_timestamps_keeps_only_text_lines() {
    let cues: Vec<Cue> = vec![
        cue(1000, 3500, "Dòng một\nDòng hai"),
        cue(4000, 5000, "Câu hai"),
    ];
    assert_eq!(strip_timestamps(&cues), "Dòng một\nDòng hai\nCâu hai\n");
}

#[test]
fn csv_with_vietnamese_quotes_and_newline_cell_roundtrips() {
    let dir: TempDir = tempdir().expect("thư mục tạm");
    let input: &[u8] =
        "\u{feff}ten,ghi chú\n\"Nguyễn, Văn A\",\"ghi chú \"\"có ngoặc\"\"\"\n\"dòng\nmới\",x\n"
            .as_bytes();
    let path: PathBuf = write_bytes(&dir, "a.csv", input);

    let table: Vec<Vec<String>> = read_table(&path, TextFormat::Csv).expect("đọc CSV");
    let expected: Vec<Vec<String>> = rows(&[
        &["ten", "ghi chú"],
        &["Nguyễn, Văn A", "ghi chú \"có ngoặc\""],
        &["dòng\nmới", "x"],
    ]);
    assert_eq!(table, expected);

    let mut buf: Vec<u8> = Vec::new();
    write_table(&table, TextFormat::Csv, &mut buf).expect("ghi CSV");
    let back_path: PathBuf = write_bytes(&dir, "b.csv", &buf);
    assert_eq!(
        read_table(&back_path, TextFormat::Csv).expect("đọc lại CSV"),
        expected
    );
}

#[test]
fn csv_with_semicolon_line_reports_dau_phay() {
    let dir: TempDir = tempdir().expect("thư mục tạm");
    let path: PathBuf = write_bytes(&dir, "a.csv", "a;b\n".as_bytes());

    match read_table(&path, TextFormat::Csv) {
        Err(MorphixError::Corrupt { reason, .. }) => {
            assert!(reason.contains("dấu phẩy"), "lý do không nói rõ: {reason}");
        }
        other => panic!("kỳ vọng Corrupt, được {other:?}"),
    }
}

#[test]
fn tsv_cell_with_tab_is_invalid_input_naming_row() {
    let table: Vec<Vec<String>> = rows(&[&["a", "b"], &["c\td", "e"]]);
    let mut buf: Vec<u8> = Vec::new();
    match write_table(&table, TextFormat::Tsv, &mut buf) {
        Err(MorphixError::InvalidInput(msg)) => {
            assert!(msg.contains('2'), "thiếu số dòng: {msg}");
        }
        other => panic!("kỳ vọng InvalidInput, được {other:?}"),
    }
}

#[test]
fn csv_output_starts_with_utf8_bom() {
    let table: Vec<Vec<String>> = rows(&[&["a", "b"]]);
    let mut buf: Vec<u8> = Vec::new();
    write_table(&table, TextFormat::Csv, &mut buf).expect("ghi CSV");
    assert_eq!(&buf[..3], &[0xEF, 0xBB, 0xBF]);
}

#[test]
fn non_utf8_input_is_corrupt_with_vietnamese_reason() {
    let dir: TempDir = tempdir().expect("thư mục tạm");
    // WHY: 0xE1 là "á" trong Latin-1 nhưng không hợp lệ trong UTF-8; không được đoán mã hoá.
    let path: PathBuf = write_bytes(
        &dir,
        "a.srt",
        b"1\n00:00:01,000 --> 00:00:02,000\nNgu\xE1\n",
    );

    match read_cues(&path) {
        Err(MorphixError::Corrupt { reason, .. }) => {
            assert!(reason.contains("UTF-8"), "lý do không nói rõ: {reason}");
        }
        other => panic!("kỳ vọng Corrupt, được {other:?}"),
    }
    match read_table(&path, TextFormat::Csv) {
        Err(MorphixError::Corrupt { .. }) => {}
        other => panic!("kỳ vọng Corrupt, được {other:?}"),
    }
}

#[test]
fn tsv_cell_keeps_literal_double_quotes() {
    let dir: TempDir = tempdir().expect("thư mục tạm");
    let path: PathBuf = write_bytes(&dir, "a.tsv", "\"Nguyễn\"\t1\n".as_bytes());
    assert_eq!(
        read_table(&path, TextFormat::Tsv).expect("đọc TSV"),
        rows(&[&["\"Nguyễn\"", "1"]])
    );
}

#[test]
fn tsv_row_with_one_empty_cell_is_kept() {
    let dir: TempDir = tempdir().expect("thư mục tạm");
    let path: PathBuf = write_bytes(&dir, "a.tsv", "a\n\nb\n".as_bytes());
    assert_eq!(
        read_table(&path, TextFormat::Tsv).expect("đọc TSV"),
        rows(&[&["a"], &[""], &["b"]])
    );
}

#[test]
fn csv_one_column_with_quoted_semicolon_reads() {
    let dir: TempDir = tempdir().expect("thư mục tạm");
    let path: PathBuf = write_bytes(&dir, "a.csv", "\"Chương 1; phần 2\"\n".as_bytes());
    assert_eq!(
        read_table(&path, TextFormat::Csv).expect("đọc CSV"),
        rows(&[&["Chương 1; phần 2"]])
    );
}

#[test]
fn csv_comma_file_with_unquoted_semicolon_cell_reads() {
    let dir: TempDir = tempdir().expect("thư mục tạm");
    let path: PathBuf = write_bytes(
        &dir,
        "a.csv",
        "ten,ghi chú\n\"Chương 1; phần 2\"\n".as_bytes(),
    );
    assert_eq!(
        read_table(&path, TextFormat::Csv).expect("đọc CSV"),
        rows(&[&["ten", "ghi chú"], &["Chương 1; phần 2"]])
    );
}

#[test]
fn csv_ragged_rows_write_back_unchanged() {
    let dir: TempDir = tempdir().expect("thư mục tạm");
    let path: PathBuf = write_bytes(&dir, "a.csv", "a,b\nc\n".as_bytes());
    let table: Vec<Vec<String>> = read_table(&path, TextFormat::Csv).expect("đọc CSV");

    let mut buf: Vec<u8> = Vec::new();
    write_table(&table, TextFormat::Csv, &mut buf).expect("ghi CSV");
    assert_eq!(buf, [UTF8_BOM, "a,b\nc\n".as_bytes()].concat());
}

#[test]
fn srt_cue_with_empty_text_roundtrips() {
    let dir: TempDir = tempdir().expect("thư mục tạm");
    let path: PathBuf = write_bytes(
        &dir,
        "a.srt",
        "1\n00:00:01,000 --> 00:00:02,000\n\n2\n00:00:03,000 --> 00:00:04,000\nHi\n".as_bytes(),
    );
    let cues: Vec<Cue> = read_cues(&path).expect("đọc SRT");
    assert_eq!(cues, vec![cue(1000, 2000, ""), cue(3000, 4000, "Hi")]);

    let back: Vec<u8> = written_srt_cues(&cues, TextFormat::Srt);
    let back_path: PathBuf = write_bytes(&dir, "b.srt", &back);
    assert_eq!(read_cues(&back_path).expect("đọc lại SRT"), cues);
}

#[test]
fn vtt_note_prefix_is_not_a_note_block() {
    let dir: TempDir = tempdir().expect("thư mục tạm");
    let path: PathBuf = write_bytes(
        &dir,
        "a.vtt",
        "WEBVTT\n\nNOTE-1\n00:01.000 --> 00:02.000\nHi\n".as_bytes(),
    );
    assert_eq!(
        read_cues(&path).expect("đọc VTT"),
        vec![cue(1000, 2000, "Hi")]
    );
}

#[test]
fn timestamp_overflow_is_corrupt_not_panic() {
    let dir: TempDir = tempdir().expect("thư mục tạm");
    let path: PathBuf = write_bytes(
        &dir,
        "a.srt",
        "1\n00:00:20000000000000000,000 --> 00:00:20000000000000001,000\nHi\n".as_bytes(),
    );
    match read_cues(&path) {
        Err(MorphixError::Corrupt { reason, .. }) => {
            assert!(reason.contains("quá lớn"), "lý do không nói rõ: {reason}");
        }
        other => panic!("kỳ vọng Corrupt, được {other:?}"),
    }
}

// ---- Job và catalog: chuyển văn bản qua run_job ----

fn text_job(inputs: Vec<PathBuf>, to: TextFormat, out_dir: Option<PathBuf>) -> Job {
    Job {
        inputs,
        operation: Operation::ConvertText { to },
        quality: Quality::Balanced,
        out_dir,
    }
}

fn text_kinds(formats: &[TextFormat]) -> Vec<FileKind> {
    formats.iter().map(|f| FileKind::Text(*f)).collect()
}

fn text_action_labels(kinds: &[FileKind]) -> Vec<String> {
    actions_for(kinds)
        .into_iter()
        .filter(|a| matches!(a.kind, ActionKind::ConvertText(_)))
        .map(|a| a.label)
        .collect()
}

fn dir_names(dir: &TempDir) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir.path())
        .expect("đọc thư mục test")
        .map(|e| {
            e.expect("đọc mục test")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    names.sort();
    names
}

#[test]
fn subtitle_actions_offer_other_subtitle_and_txt_not_own_format() {
    assert_eq!(
        text_action_labels(&text_kinds(&[TextFormat::Srt])),
        ["VTT", "TXT", "MD"]
    );
    assert_eq!(
        text_action_labels(&text_kinds(&[TextFormat::Vtt])),
        ["SRT", "TXT", "MD"]
    );
}

#[test]
fn text_and_table_actions_follow_their_group() {
    assert_eq!(text_action_labels(&text_kinds(&[TextFormat::Txt])), ["MD"]);
    assert_eq!(
        text_action_labels(&text_kinds(&[TextFormat::Markdown])),
        ["TXT", "HTML", "DOCX"]
    );
    assert_eq!(
        text_action_labels(&text_kinds(&[TextFormat::Csv])),
        ["TSV", "MD"]
    );
    assert_eq!(
        text_action_labels(&text_kinds(&[TextFormat::Tsv])),
        ["CSV", "MD"]
    );
}

#[test]
fn mixed_text_groups_offer_no_text_action() {
    assert!(text_action_labels(&text_kinds(&[TextFormat::Srt, TextFormat::Csv])).is_empty());
    assert!(text_action_labels(&text_kinds(&[TextFormat::Txt, TextFormat::Srt])).is_empty());
    assert!(text_action_labels(&text_kinds(&[TextFormat::Txt, TextFormat::Csv])).is_empty());
}

#[test]
fn srt_to_txt_writes_next_to_source_and_keeps_source() {
    let d = tempdir().unwrap();
    let source = write_bytes(
        &d,
        "a.srt",
        &written_srt_cues(&[cue(1000, 2500, "Xin chào\nlần hai")], TextFormat::Srt),
    );
    let before = std::fs::read(&source).unwrap();
    let report = run_job(
        &text_job(vec![source.clone()], TextFormat::Txt, None),
        &CancelToken::new(),
        &|_| {},
    );
    let out = report.items[0]
        .result
        .as_ref()
        .expect("chuyển SRT sang TXT");
    assert_eq!(out, &d.path().join("a.txt"));
    assert_eq!(
        std::fs::read(out).unwrap(),
        "Xin chào\nlần hai\n".as_bytes()
    );
    assert_eq!(std::fs::read(&source).unwrap(), before);
}

#[test]
fn srt_to_vtt_job_keeps_times_with_dot_separator() {
    let d = tempdir().unwrap();
    let source = write_bytes(
        &d,
        "a.srt",
        &written_srt_cues(&[cue(1000, 2500, "hi")], TextFormat::Srt),
    );
    let report = run_job(
        &text_job(vec![source], TextFormat::Vtt, None),
        &CancelToken::new(),
        &|_| {},
    );
    let out = report.items[0]
        .result
        .as_ref()
        .expect("chuyển SRT sang VTT");
    assert_eq!(
        std::fs::read_to_string(out).unwrap(),
        "WEBVTT\n\n00:00:01.000 --> 00:00:02.500\nhi\n\n"
    );
}

#[test]
fn output_name_avoids_existing_file_like_other_jobs() {
    let d = tempdir().unwrap();
    let source = write_bytes(
        &d,
        "a.srt",
        &written_srt_cues(&[cue(0, 1000, "x")], TextFormat::Srt),
    );
    write_bytes(&d, "a.txt", b"do nguoi dung");
    let report = run_job(
        &text_job(vec![source], TextFormat::Txt, None),
        &CancelToken::new(),
        &|_| {},
    );
    let out = report.items[0]
        .result
        .as_ref()
        .expect("chuyển SRT sang TXT");
    assert_eq!(out, &d.path().join("a (1).txt"));
    assert_eq!(
        std::fs::read(d.path().join("a.txt")).unwrap(),
        b"do nguoi dung"
    );
}

#[test]
fn disallowed_text_conversions_fail_and_write_nothing() {
    let d = tempdir().unwrap();
    let txt = write_bytes(&d, "a.txt", "chỉ có chữ\n".as_bytes());
    let srt = write_bytes(
        &d,
        "b.srt",
        &written_srt_cues(&[cue(0, 1000, "x")], TextFormat::Srt),
    );
    let before = dir_names(&d);
    let report = run_job(
        &text_job(vec![txt], TextFormat::Srt, None),
        &CancelToken::new(),
        &|_| {},
    );
    assert!(matches!(
        report.items[0].result,
        Err(MorphixError::InvalidInput(_))
    ));
    let report = run_job(
        &text_job(vec![srt], TextFormat::Csv, None),
        &CancelToken::new(),
        &|_| {},
    );
    assert!(matches!(
        report.items[0].result,
        Err(MorphixError::InvalidInput(_))
    ));
    assert_eq!(dir_names(&d), before);
}

#[test]
fn csv_to_tsv_converts_plain_table() {
    let d = tempdir().unwrap();
    let source = write_bytes(&d, "a.csv", "tên,giá\nphở,\"1,5\"\n".as_bytes());
    let report = run_job(
        &text_job(vec![source], TextFormat::Tsv, None),
        &CancelToken::new(),
        &|_| {},
    );
    let out = report.items[0]
        .result
        .as_ref()
        .expect("chuyển CSV sang TSV");
    assert_eq!(out, &d.path().join("a.tsv"));
    assert_eq!(
        std::fs::read_to_string(out).unwrap(),
        "tên\tgiá\nphở\t1,5\n"
    );
}

#[test]
fn csv_to_tsv_with_tab_in_cell_fails_with_row_and_leaves_no_file() {
    let d = tempdir().unwrap();
    let source = write_bytes(&d, "a.csv", "a,b\n\"x\ty\",z\n".as_bytes());
    let report = run_job(
        &text_job(vec![source], TextFormat::Tsv, None),
        &CancelToken::new(),
        &|_| {},
    );
    match &report.items[0].result {
        Err(MorphixError::InvalidInput(msg)) => assert!(msg.contains("dòng 2"), "{msg}"),
        other => panic!("cần InvalidInput nêu dòng 2, được {other:?}"),
    }
    assert_eq!(dir_names(&d), ["a.csv"]);
}

#[test]
fn vtt_style_and_region_blocks_are_skipped_but_a_cue_named_style_is_kept() {
    let dir: TempDir = tempdir().expect("thư mục tạm");
    let path: PathBuf = write_bytes(
        &dir,
        "a.vtt",
        "WEBVTT\n\nSTYLE\n::cue { color: yellow }\n\nREGION\nid:fred width:40%\n\n\
         STYLE\n00:01.000 --> 00:02.000\nHi\n"
            .as_bytes(),
    );
    assert_eq!(
        read_cues(&path).expect("đọc VTT"),
        vec![cue(1000, 2000, "Hi")]
    );
}

// ---- Markdown: bỏ ký hiệu khi ra, thêm ký hiệu khi vào ----

/// Chuyển một file vừa ghi sang `to` qua `run_job` và trả nội dung file ra.
fn convert(name: &str, content: &str, to: TextFormat) -> String {
    let d = tempdir().unwrap();
    let source = write_bytes(&d, name, content.as_bytes());
    let report = run_job(
        &text_job(vec![source], to, None),
        &CancelToken::new(),
        &|_| {},
    );
    let out = report.items[0].result.as_ref().expect("chuyển văn bản");
    std::fs::read_to_string(out).unwrap()
}

#[test]
fn markdown_to_txt_strips_syntax() {
    assert_eq!(convert("a.md", "# T\n\n**b**", TextFormat::Txt), "T\n\nb\n");
}

#[test]
fn txt_to_markdown_escapes_special_characters() {
    assert_eq!(
        convert("a.txt", "a_b *c*\n", TextFormat::Markdown),
        "a\\_b \\*c\\*\n"
    );
}

#[test]
fn markdown_to_markdown_and_txt_to_txt_stay_verbatim() {
    assert_eq!(
        convert("a.md", "# x *y*\n", TextFormat::Markdown),
        "# x *y*\n"
    );
    assert_eq!(convert("a.txt", "# x *y*\n", TextFormat::Txt), "# x *y*\n");
}

#[test]
fn csv_to_markdown_makes_a_table() {
    assert_eq!(
        convert("a.csv", "a,b\n1,2\n", TextFormat::Markdown),
        "| a | b |\n| --- | --- |\n| 1 | 2 |\n"
    );
}

#[test]
fn tsv_to_markdown_makes_a_table() {
    assert_eq!(
        convert("a.tsv", "a\tb\n1\t2\n", TextFormat::Markdown),
        "| a | b |\n| --- | --- |\n| 1 | 2 |\n"
    );
}

#[test]
fn srt_to_markdown_lists_start_times() {
    let srt = String::from_utf8(written_srt_cues(
        &[cue(1000, 2000, "Xin chào")],
        TextFormat::Srt,
    ))
    .unwrap();
    assert_eq!(
        convert("a.srt", &srt, TextFormat::Markdown),
        "- **00:00:01** Xin chào\n"
    );
}

#[test]
fn vtt_to_markdown_lists_start_times() {
    assert_eq!(
        convert(
            "a.vtt",
            "WEBVTT\n\n01:05.000 --> 01:06.000\nHi\n",
            TextFormat::Markdown
        ),
        "- **00:01:05** Hi\n"
    );
}

#[test]
fn txt_to_csv_and_markdown_to_srt_are_still_rejected() {
    let d = tempdir().unwrap();
    for (name, to) in [("a.txt", TextFormat::Csv), ("b.md", TextFormat::Srt)] {
        let source = write_bytes(&d, name, b"x\n");
        let report = run_job(
            &text_job(vec![source], to, None),
            &CancelToken::new(),
            &|_| {},
        );
        match &report.items[0].result {
            Err(MorphixError::InvalidInput(msg)) => {
                // WHY: chỉ đích phụ đề mới có lý do "mốc thời gian"; TXT→CSV là khác nhóm.
                assert_eq!(
                    msg.contains("mốc thời gian"),
                    to == TextFormat::Srt,
                    "{msg}"
                );
            }
            other => panic!("{name}: cần InvalidInput, được {other:?}"),
        }
    }
}

// ---- DOCX/HTML: Markdown làm trung gian ----

/// Chạy chuyển văn bản trên `source` và trả report của mục duy nhất.
fn run_one(source: PathBuf, to: TextFormat) -> morphix_core::job::ItemReport {
    run_job(
        &text_job(vec![source], to, None),
        &CancelToken::new(),
        &|_| {},
    )
    .items
    .remove(0)
}

fn docx_markdown(path: &std::path::Path) -> String {
    morphix_core::ops::docx::docx_to_markdown(path)
        .expect("đọc DOCX vừa ghi")
        .markdown
}

const MD_SAMPLE: &str = "# Tiêu đề\n\nĐoạn có **đậm** và [link](https://a.b).\n\n- một\n- hai\n";

#[test]
fn md_to_html_and_back() {
    let d = tempdir().unwrap();
    let md = write_bytes(&d, "a.md", MD_SAMPLE.as_bytes());
    let html = run_one(md, TextFormat::Html).result.expect("MD sang HTML");
    assert!(std::fs::read_to_string(&html)
        .unwrap()
        .contains("<h1>Tiêu đề</h1>"));
    let back = d.path().join("b.html");
    std::fs::rename(&html, &back).unwrap();
    let md2 = run_one(back, TextFormat::Markdown)
        .result
        .expect("HTML sang MD");
    assert_eq!(std::fs::read_to_string(md2).unwrap(), MD_SAMPLE);
}

#[test]
fn md_to_docx_and_back() {
    let d = tempdir().unwrap();
    let md = write_bytes(&d, "a.md", MD_SAMPLE.as_bytes());
    let docx = run_one(md, TextFormat::Docx).result.expect("MD sang DOCX");
    assert_eq!(docx, d.path().join("a.docx"));
    assert_eq!(docx_markdown(&docx), MD_SAMPLE);
}

#[test]
fn docx_to_html_keeps_heading_and_bold() {
    let d = tempdir().unwrap();
    let md = write_bytes(&d, "a.md", MD_SAMPLE.as_bytes());
    let docx = run_one(md, TextFormat::Docx).result.unwrap();
    let html = run_one(docx, TextFormat::Html)
        .result
        .expect("DOCX sang HTML");
    let page = std::fs::read_to_string(html).unwrap();
    assert!(page.contains("<h1>Tiêu đề</h1>"), "{page}");
    assert!(page.contains("<strong>đậm</strong>"), "{page}");
}

#[test]
fn html_to_docx_keeps_list() {
    let d = tempdir().unwrap();
    let html = write_bytes(&d, "a.html", "<ul><li>một</li><li>hai</li></ul>".as_bytes());
    let docx = run_one(html, TextFormat::Docx)
        .result
        .expect("HTML sang DOCX");
    assert_eq!(docx_markdown(&docx), "- một\n- hai\n");
}

#[test]
fn docx_to_txt_has_no_markdown_marks() {
    let d = tempdir().unwrap();
    let md = write_bytes(&d, "a.md", MD_SAMPLE.as_bytes());
    let docx = run_one(md, TextFormat::Docx).result.unwrap();
    let txt = run_one(docx, TextFormat::Txt)
        .result
        .expect("DOCX sang TXT");
    assert_eq!(
        std::fs::read_to_string(txt).unwrap(),
        "Tiêu đề\n\nĐoạn có đậm và link (https://a.b).\n\n- một\n- hai\n"
    );
}

#[test]
fn html_to_txt_has_no_markdown_marks() {
    let d = tempdir().unwrap();
    let html = write_bytes(&d, "a.html", "<h1>A</h1><p><b>b</b> c*</p>".as_bytes());
    let txt = run_one(html, TextFormat::Txt)
        .result
        .expect("HTML sang TXT");
    assert_eq!(std::fs::read_to_string(txt).unwrap(), "A\n\nb c*\n");
}

#[test]
fn html_with_image_warns_about_dropped_images() {
    let d = tempdir().unwrap();
    let html = write_bytes(
        &d,
        "a.html",
        "<p><img src=\"x.png\" alt=\"logo\"></p>".as_bytes(),
    );
    let report = run_one(html, TextFormat::Markdown);
    assert!(report.result.is_ok());
    assert_eq!(report.warnings, ["a.html: bỏ 1 ảnh (chỉ giữ chữ thay thế)"]);
}

#[test]
fn txt_to_docx_is_rejected() {
    let d = tempdir().unwrap();
    let txt = write_bytes(&d, "a.txt", b"x\n");
    assert!(matches!(
        run_one(txt, TextFormat::Docx).result,
        Err(MorphixError::InvalidInput(_))
    ));
    assert_eq!(dir_names(&d), ["a.txt"]);
}

#[test]
fn word_and_html_offer_the_other_document_formats() {
    assert_eq!(
        text_action_labels(&text_kinds(&[TextFormat::Docx])),
        ["MD", "HTML", "TXT"]
    );
    assert_eq!(
        text_action_labels(&text_kinds(&[TextFormat::Html])),
        ["MD", "DOCX", "TXT"]
    );
    assert_eq!(
        text_action_labels(&text_kinds(&[TextFormat::Markdown])),
        ["TXT", "HTML", "DOCX"]
    );
}

#[test]
fn txt_and_md_together_still_offer_txt_and_md() {
    assert_eq!(
        text_action_labels(&text_kinds(&[TextFormat::Txt, TextFormat::Markdown])),
        ["TXT", "MD"]
    );
}

#[test]
fn docx_and_html_together_offer_their_common_targets() {
    assert_eq!(
        text_action_labels(&text_kinds(&[TextFormat::Docx, TextFormat::Html])),
        ["MD", "TXT"]
    );
    assert!(text_action_labels(&text_kinds(&[TextFormat::Docx, TextFormat::Markdown])).is_empty());
}

#[test]
fn md_with_image_to_docx_warns_about_dropped_images() {
    let d = tempdir().unwrap();
    let md = write_bytes(&d, "a.md", "![logo](x.png)\n".as_bytes());
    let report = run_one(md, TextFormat::Docx);
    assert!(report.result.is_ok());
    assert_eq!(report.warnings, ["a.md: bỏ 1 ảnh (chỉ giữ chữ thay thế)"]);
}

#[test]
fn password_protected_docx_reports_the_password_reason() {
    let d = tempdir().unwrap();
    let mut ole = vec![0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1];
    ole.resize(4096, 0);
    let docx = write_bytes(&d, "khoa.docx", &ole);
    match run_one(docx, TextFormat::Markdown).result {
        Err(MorphixError::Corrupt { reason, .. }) => {
            assert!(reason.contains("mật khẩu"), "{reason}")
        }
        other => panic!("cần Corrupt nêu mật khẩu, được {other:?}"),
    }
}

#[test]
fn docx_with_footnotes_and_header_warns_what_was_left_out() {
    use std::io::Write as _;
    let d = tempdir().unwrap();
    let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    writer
        .start_file(
            "word/document.xml",
            zip::write::SimpleFileOptions::default(),
        )
        .unwrap();
    writer
        .write_all(br#"<w:document xmlns:w="w" xmlns:r="r"><w:body><w:p><w:r><w:t>a</w:t></w:r><w:r><w:footnoteReference w:id="1"/></w:r></w:p><w:sectPr><w:headerReference r:id="rId9"/></w:sectPr></w:body></w:document>"#)
        .unwrap();
    let bytes = writer.finish().unwrap().into_inner();
    let docx = write_bytes(&d, "a.docx", &bytes);
    let report = run_one(docx, TextFormat::Markdown);
    assert!(report.result.is_ok());
    assert_eq!(
        report.warnings,
        ["a.docx: bỏ 1 chú thích/ghi chú và đầu/chân trang (chỉ giữ nội dung chính)"]
    );
}

#[test]
fn docx_to_html_writes_html_directly() {
    let d = tempdir().unwrap();
    let md = write_bytes(&d, "a.md", MD_SAMPLE.as_bytes());
    let docx = run_one(md, TextFormat::Docx).result.unwrap();
    let html = run_one(docx, TextFormat::Html)
        .result
        .expect("DOCX sang HTML");
    let page = std::fs::read_to_string(html).unwrap();
    assert!(page.contains("<h1>Tiêu đề</h1>"), "{page}");
    assert!(page.contains("<strong>đậm</strong>"), "{page}");
    assert!(page.contains("<a href=\"https://a.b\">link</a>"), "{page}");
    assert!(!page.contains("**"), "{page}");
}

#[test]
fn docx_and_html_to_txt_keep_link_urls() {
    let d = tempdir().unwrap();
    let md = write_bytes(&d, "a.md", MD_SAMPLE.as_bytes());
    let docx = run_one(md, TextFormat::Docx).result.unwrap();
    let txt = run_one(docx, TextFormat::Txt)
        .result
        .expect("DOCX sang TXT");
    assert_eq!(
        std::fs::read_to_string(txt).unwrap(),
        "Tiêu đề\n\nĐoạn có đậm và link (https://a.b).\n\n- một\n- hai\n"
    );
    let html = write_bytes(
        &d,
        "b.html",
        "<p><a href=\"https://x.y\">x</a> và <a href=\"https://x.y\">https://x.y</a></p>"
            .as_bytes(),
    );
    let txt = run_one(html, TextFormat::Txt)
        .result
        .expect("HTML sang TXT");
    assert_eq!(
        std::fs::read_to_string(txt).unwrap(),
        "x (https://x.y) và https://x.y\n"
    );
}

// ---- Ảnh: đọc từ HTML/MD/DOCX, nhúng khi ghi HTML/DOCX ----

fn png_bytes() -> Vec<u8> {
    let mut out = Vec::new();
    image::RgbaImage::from_pixel(2, 2, image::Rgba([255, 0, 0, 255]))
        .write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png)
        .unwrap();
    out
}

fn zip_names(path: &std::path::Path) -> Vec<String> {
    let archive = zip::ZipArchive::new(std::fs::File::open(path).unwrap()).unwrap();
    archive
        .file_names()
        .filter_map(Result::ok)
        .map(|name| name.to_string())
        .collect()
}

fn zip_entry(path: &std::path::Path, name: &str) -> String {
    use std::io::Read as _;
    let mut archive = zip::ZipArchive::new(std::fs::File::open(path).unwrap()).unwrap();
    let mut out = String::new();
    archive
        .by_name(name)
        .unwrap()
        .read_to_string(&mut out)
        .unwrap();
    out
}

fn has_media(path: &std::path::Path) -> bool {
    zip_names(path).iter().any(|n| n.starts_with("word/media/"))
}

#[test]
fn html_with_relative_image_to_docx_embeds_it() {
    let d = tempdir().unwrap();
    write_bytes(&d, "x.png", &png_bytes());
    let html = write_bytes(
        &d,
        "a.html",
        r#"<p><img src="x.png" alt="l"></p>"#.as_bytes(),
    );
    let report = run_one(html, TextFormat::Docx);
    let docx = report.result.expect("HTML sang DOCX");
    assert!(report.warnings.is_empty(), "{:?}", report.warnings);
    assert!(zip_names(&docx).contains(&"word/media/image1.png".to_string()));
    assert!(zip_entry(&docx, "word/_rels/document.xml.rels").contains("/relationships/image"));
}

#[test]
fn docx_image_to_html_becomes_data_uri() {
    let d = tempdir().unwrap();
    write_bytes(&d, "x.png", &png_bytes());
    let html = write_bytes(
        &d,
        "a.html",
        r#"<p><img src="x.png" alt="l"></p>"#.as_bytes(),
    );
    let docx = run_one(html, TextFormat::Docx).result.unwrap();
    let back = run_one(docx, TextFormat::Html)
        .result
        .expect("DOCX sang HTML");
    let page = std::fs::read_to_string(back).unwrap();
    assert!(page.contains(r#"src="data:image/png;base64,"#), "{page}");
}

#[test]
fn data_uri_image_round_trips_to_docx() {
    use base64::Engine as _;
    let d = tempdir().unwrap();
    let b64 = base64::engine::general_purpose::STANDARD.encode(png_bytes());
    let html = write_bytes(
        &d,
        "a.html",
        format!(r#"<p><img src="data:image/png;base64,{b64}" alt="l"></p>"#).as_bytes(),
    );
    let report = run_one(html, TextFormat::Docx);
    let docx = report.result.expect("HTML sang DOCX");
    assert!(report.warnings.is_empty(), "{:?}", report.warnings);
    assert!(zip_names(&docx).contains(&"word/media/image1.png".to_string()));
}

#[test]
fn relative_image_outside_the_folder_is_not_read() {
    let d = tempdir().unwrap();
    write_bytes(&d, "x.png", &png_bytes());
    let sub = d.path().join("sub");
    std::fs::create_dir(&sub).unwrap();
    for src in ["../x.png", "http://h/x.png", "file:///x.png", "/x.png"] {
        let html = sub.join("a.html");
        std::fs::write(&html, format!(r#"<p><img src="{src}" alt="l"></p>"#)).unwrap();
        let report = run_one(html, TextFormat::Docx);
        let docx = report.result.expect("HTML sang DOCX");
        assert_eq!(
            report.warnings,
            ["a.html: bỏ 1 ảnh (chỉ giữ chữ thay thế)"],
            "{src}"
        );
        assert!(!has_media(&docx), "{src}");
    }
}

#[test]
fn broken_image_is_dropped_with_warning() {
    let d = tempdir().unwrap();
    write_bytes(&d, "x.png", b"not png");
    let html = write_bytes(
        &d,
        "a.html",
        r#"<p><img src="x.png" alt="l"></p>"#.as_bytes(),
    );
    let report = run_one(html, TextFormat::Docx);
    assert!(report.result.is_ok());
    assert_eq!(report.warnings, ["a.html: bỏ 1 ảnh (chỉ giữ chữ thay thế)"]);
}

#[test]
fn image_inside_table_cell_is_embedded() {
    let d = tempdir().unwrap();
    write_bytes(&d, "x.png", &png_bytes());
    let html = write_bytes(
        &d,
        "a.html",
        r#"<table><tr><td><img src="x.png" alt="l"></td></tr></table>"#.as_bytes(),
    );
    let docx = run_one(html, TextFormat::Docx).result.unwrap();
    assert!(zip_names(&docx).contains(&"word/media/image1.png".to_string()));
}

#[cfg(unix)]
#[test]
fn symlinked_image_pointing_outside_is_not_read() {
    let d = tempdir().unwrap();
    let outside = tempdir().unwrap();
    write_bytes(&outside, "secret.png", &png_bytes());
    std::os::unix::fs::symlink(outside.path().join("secret.png"), d.path().join("link.png"))
        .unwrap();
    let html = write_bytes(&d, "a.html", r#"<p><img src="link.png"></p>"#.as_bytes());
    let report = run_one(html, TextFormat::Docx);
    let docx = report.result.unwrap();
    assert!(!has_media(&docx));
    assert_eq!(report.warnings.len(), 1);
}

// ---- Chú thích cuối trang ----

fn docx_with_parts(dir: &TempDir, name: &str, parts: &[(&str, &str)]) -> PathBuf {
    use std::io::Write as _;
    let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for (part, body) in parts {
        writer
            .start_file(*part, zip::write::SimpleFileOptions::default())
            .unwrap();
        writer.write_all(body.as_bytes()).unwrap();
    }
    write_bytes(dir, name, &writer.finish().unwrap().into_inner())
}

#[test]
fn footnote_read_from_footnotes_part_reaches_markdown() {
    let d = tempdir().unwrap();
    let docx = docx_with_parts(
        &d,
        "a.docx",
        &[
            (
                "word/document.xml",
                r#"<w:document xmlns:w="w" xmlns:r="r"><w:body><w:p><w:r><w:t>a</w:t></w:r><w:r><w:footnoteReference w:id="1"/></w:r></w:p></w:body></w:document>"#,
            ),
            (
                "word/footnotes.xml",
                r#"<w:footnotes xmlns:w="w"><w:footnote w:type="separator" w:id="-1"><w:p/></w:footnote><w:footnote w:id="1"><w:p><w:r><w:t>ghi chú</w:t></w:r></w:p></w:footnote></w:footnotes>"#,
            ),
        ],
    );
    let report = run_one(docx, TextFormat::Markdown);
    let md = report.result.expect("DOCX sang MD");
    assert!(report.warnings.is_empty(), "{:?}", report.warnings);
    assert_eq!(
        std::fs::read_to_string(md).unwrap(),
        "a[^1]\n\n[^1]: ghi chú\n"
    );
}

#[test]
fn markdown_footnote_becomes_word_footnote() {
    let d = tempdir().unwrap();
    let md = write_bytes(&d, "a.md", "a[^x]\n\n[^x]: n\n".as_bytes());
    let docx = run_one(md, TextFormat::Docx).result.expect("MD sang DOCX");
    assert!(zip_names(&docx).contains(&"word/footnotes.xml".to_string()));
    assert!(zip_entry(&docx, "word/footnotes.xml").contains(">n</w:t>"));
    assert!(zip_entry(&docx, "word/document.xml").contains(r#"<w:footnoteReference w:id="1"/>"#));
}

#[test]
fn footnotes_survive_to_html_and_go_to_the_end_of_txt() {
    let d = tempdir().unwrap();
    let md = write_bytes(&d, "a.md", "a[^x]\n\n[^x]: ghi chú\n".as_bytes());
    let docx = run_one(md, TextFormat::Docx).result.unwrap();
    let html = run_one(docx.clone(), TextFormat::Html)
        .result
        .expect("DOCX sang HTML");
    let page = std::fs::read_to_string(html).unwrap();
    assert!(page.contains(r#"<li id="fn1">"#), "{page}");
    assert!(page.contains("ghi chú"), "{page}");
    let txt = run_one(docx, TextFormat::Txt)
        .result
        .expect("DOCX sang TXT");
    assert_eq!(
        std::fs::read_to_string(txt).unwrap(),
        "a[1]\n\n[1] ghi chú\n"
    );
}

// ---- Ô gộp và chữ gạch chân/chỉ số ----

#[test]
fn merged_cells_survive_html_to_docx_and_back() {
    let d = tempdir().unwrap();
    let html = write_bytes(
        &d,
        "a.html",
        r#"<table><tr><td colspan="2">a</td><td rowspan="2">b</td></tr><tr><td>c</td><td>d</td></tr></table>"#.as_bytes(),
    );
    let docx = run_one(html, TextFormat::Docx).result.unwrap();
    let back = run_one(docx, TextFormat::Html)
        .result
        .expect("DOCX sang HTML");
    let page = std::fs::read_to_string(back).unwrap();
    assert!(page.contains(r#"colspan="2""#), "{page}");
    assert!(page.contains(r#"rowspan="2""#), "{page}");
}

#[test]
fn merged_cells_line_up_in_txt() {
    let d = tempdir().unwrap();
    let html = write_bytes(
        &d,
        "a.html",
        r#"<table><tr><td colspan="2">a</td><td rowspan="2">b</td></tr><tr><td>c</td><td>d</td></tr></table>"#.as_bytes(),
    );
    let txt = run_one(html, TextFormat::Txt)
        .result
        .expect("HTML sang TXT");
    assert_eq!(std::fs::read_to_string(txt).unwrap(), "a\t\tb\nc\td\t\n");
}

#[test]
fn markdown_target_warns_about_downgraded_formatting() {
    let d = tempdir().unwrap();
    let html = write_bytes(&d, "a.html", "<p><u>u</u></p>".as_bytes());
    let report = run_one(html, TextFormat::Markdown);
    assert!(report.result.is_ok());
    assert_eq!(
        report.warnings,
        ["a.html: bỏ định dạng MD không có (gạch chân, trên/dưới, ô gộp) (chỉ giữ nội dung chính)"]
    );
}

#[test]
fn underline_and_superscript_survive_docx_and_html() {
    let d = tempdir().unwrap();
    let html = write_bytes(
        &d,
        "a.html",
        "<p><u>u</u> x<sup>2</sup> H<sub>2</sub>O</p>".as_bytes(),
    );
    let docx = run_one(html, TextFormat::Docx).result.unwrap();
    let back = run_one(docx, TextFormat::Html)
        .result
        .expect("DOCX sang HTML");
    let page = std::fs::read_to_string(back).unwrap();
    assert!(page.contains("<u>u</u>"), "{page}");
    assert!(page.contains("<sup>2</sup>"), "{page}");
    assert!(page.contains("<sub>2</sub>"), "{page}");
}

#[test]
fn footnote_link_gets_its_own_relationship_part() {
    let d = tempdir().unwrap();
    let md = write_bytes(
        &d,
        "a.md",
        "a[^1]\n\n[^1]: see [b](https://b.c)\n".as_bytes(),
    );
    let docx = run_one(md, TextFormat::Docx).result.unwrap();
    assert!(zip_entry(&docx, "word/_rels/footnotes.xml.rels").contains("https://b.c"));
}

#[test]
fn table_in_a_cell_is_followed_by_a_paragraph() {
    let d = tempdir().unwrap();
    let html = write_bytes(
        &d,
        "a.html",
        "<table><tr><td><table><tr><td>x</td></tr></table></td></tr></table>".as_bytes(),
    );
    let docx = run_one(html, TextFormat::Docx).result.unwrap();
    assert!(zip_entry(&docx, "word/document.xml").contains("</w:tbl><w:p/></w:tc>"));
}
