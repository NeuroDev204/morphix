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
        ["VTT", "TXT"]
    );
    assert_eq!(
        text_action_labels(&text_kinds(&[TextFormat::Vtt])),
        ["SRT", "TXT"]
    );
}

#[test]
fn text_and_table_actions_follow_their_group() {
    assert_eq!(text_action_labels(&text_kinds(&[TextFormat::Txt])), ["MD"]);
    assert_eq!(
        text_action_labels(&text_kinds(&[TextFormat::Markdown])),
        ["TXT"]
    );
    assert_eq!(text_action_labels(&text_kinds(&[TextFormat::Csv])), ["TSV"]);
    assert_eq!(text_action_labels(&text_kinds(&[TextFormat::Tsv])), ["CSV"]);
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
