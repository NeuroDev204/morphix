use morphix_gui::commands::parse_paths;
use morphix_gui::history::EntryStatus;

#[test]
fn parse_paths_rejects_empty_and_blank_entries() {
    assert!(parse_paths(vec![]).is_err());
    assert!(parse_paths(vec!["  ".into()]).is_err());
}

#[test]
fn parse_paths_keeps_unicode_and_order() {
    let got = parse_paths(vec!["/a/Ảnh 2.jpg".into(), "/a/Ảnh 10.jpg".into()]).unwrap();
    assert_eq!(got[0].to_string_lossy(), "/a/Ảnh 2.jpg");
    assert_eq!(got[1].to_string_lossy(), "/a/Ảnh 10.jpg");
}

#[test]
fn recent_history_returns_newest_first_and_respects_limit() {
    use morphix_gui::history::{EntryStatus, History, HistoryEntry};
    let d = tempfile::tempdir().unwrap();
    let mut h = History::open(&d.path().join("history.json"));
    for n in 0..6u64 {
        h.push(HistoryEntry {
            path: format!("/tmp/file-{n}.jpg"),
            title: format!("việc {n}"),
            detail: String::new(),
            status: EntryStatus::Done,
            finished_at_unix: n,
        })
        .unwrap();
    }
    let got = morphix_gui::commands::recent_from(&h, 4);
    assert_eq!(got.len(), 4);
    assert_eq!(got[0].title, "việc 5");
}

#[test]
fn save_settings_rejects_invalid_values_before_writing() {
    let d = tempfile::tempdir().unwrap();
    let path = d.path().join("settings.json");
    let bad = morphix_gui::settings::Settings {
        default_resize_percent: 0,
        ..Default::default()
    };
    assert!(morphix_gui::commands::save_settings_to(&path, bad).is_err());
    assert!(!path.exists());
}

#[test]
fn save_settings_writes_valid_values() {
    let d = tempfile::tempdir().unwrap();
    let path = d.path().join("settings.json");
    let good = morphix_gui::settings::Settings {
        default_resize_percent: 30,
        ..Default::default()
    };
    morphix_gui::commands::save_settings_to(&path, good.clone()).unwrap();
    assert_eq!(morphix_gui::settings::load(&path), good);
}

fn convert_job(inputs: Vec<std::path::PathBuf>) -> morphix_core::Job {
    morphix_core::Job {
        inputs,
        operation: morphix_core::Operation::ConvertImage {
            to: morphix_core::ImgFormat::Jpeg,
        },
        quality: morphix_core::Quality::Balanced,
        out_dir: None,
    }
}

fn sized_file(dir: &std::path::Path, name: &str, bytes: usize) -> std::path::PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, vec![0u8; bytes]).unwrap();
    path
}

#[test]
fn history_entry_for_report_success_names_first_output_and_sums_bytes() {
    let d = tempfile::tempdir().unwrap();
    let inputs = vec![
        sized_file(d.path(), "a.png", 1000),
        sized_file(d.path(), "b.png", 2000),
    ];
    let outputs = [
        sized_file(d.path(), "a.jpg", 500),
        sized_file(d.path(), "b.jpg", 700),
    ];
    let report = morphix_core::JobReport {
        items: vec![
            morphix_core::ItemReport {
                inputs: vec![inputs[0].clone()],
                result: Ok(outputs[0].clone()),
            },
            morphix_core::ItemReport {
                inputs: vec![inputs[1].clone()],
                result: Ok(outputs[1].clone()),
            },
        ],
    };
    let entry = morphix_gui::commands::history_entry_for_report(&convert_job(inputs), &report);
    assert_eq!(entry.status, EntryStatus::Done);
    assert_eq!(entry.title, "2 ảnh → a.jpg");
    assert_eq!(entry.path, outputs[0].to_string_lossy());
    assert_eq!(
        entry.detail,
        "Hoàn tất · Đổi sang JPG · 2 thành công, 0 lỗi · 3 KB → 1 KB"
    );
}

#[test]
fn history_entry_for_report_mixed_is_failed_and_counts_only_successes_in_output() {
    let d = tempfile::tempdir().unwrap();
    let inputs = vec![
        sized_file(d.path(), "a.png", 1000),
        sized_file(d.path(), "b.png", 1000),
        sized_file(d.path(), "c.png", 1000),
    ];
    let out_a = sized_file(d.path(), "a.jpg", 300);
    let out_c = sized_file(d.path(), "c.jpg", 300);
    let report = morphix_core::JobReport {
        items: vec![
            morphix_core::ItemReport {
                inputs: vec![inputs[0].clone()],
                result: Ok(out_a.clone()),
            },
            morphix_core::ItemReport {
                inputs: vec![inputs[1].clone()],
                result: Err(morphix_core::MorphixError::InvalidInput("hỏng".into())),
            },
            morphix_core::ItemReport {
                inputs: vec![inputs[2].clone()],
                result: Ok(out_c),
            },
        ],
    };
    let entry = morphix_gui::commands::history_entry_for_report(&convert_job(inputs), &report);
    assert_eq!(entry.status, EntryStatus::Failed);
    assert_eq!(entry.title, "3 ảnh → a.jpg");
    assert_eq!(entry.path, out_a.to_string_lossy());
    assert!(
        entry
            .detail
            .starts_with("Có lỗi · Đổi sang JPG · 2 thành công, 1 lỗi"),
        "{}",
        entry.detail
    );
    assert!(entry.detail.contains("· 3 KB → 600 B"), "{}", entry.detail);
}

#[test]
fn history_entry_for_report_all_failed_uses_input_name_and_reports_no_output() {
    let d = tempfile::tempdir().unwrap();
    let inputs = vec![
        sized_file(d.path(), "b.png", 2000),
        sized_file(d.path(), "c.png", 2000),
    ];
    let report = morphix_core::JobReport {
        items: inputs
            .iter()
            .map(|input| morphix_core::ItemReport {
                inputs: vec![input.clone()],
                result: Err(morphix_core::MorphixError::InvalidInput("hỏng".into())),
            })
            .collect(),
    };
    let entry =
        morphix_gui::commands::history_entry_for_report(&convert_job(inputs.clone()), &report);
    assert_eq!(entry.status, EntryStatus::Failed);
    assert_eq!(entry.title, "2 ảnh → b.png");
    assert_eq!(entry.path, inputs[0].to_string_lossy());
    assert!(
        entry.detail.contains("0 thành công, 2 lỗi"),
        "{}",
        entry.detail
    );
    assert!(entry.detail.contains("· 4 KB → 0 B"), "{}", entry.detail);
}

#[test]
fn history_entry_for_report_cancelled_keeps_partial_counts() {
    let d = tempfile::tempdir().unwrap();
    let inputs = vec![
        sized_file(d.path(), "a.png", 1000),
        sized_file(d.path(), "b.png", 1000),
        sized_file(d.path(), "c.png", 1000),
    ];
    let out_a = sized_file(d.path(), "a.jpg", 400);
    let report = morphix_core::JobReport {
        items: vec![
            morphix_core::ItemReport {
                inputs: vec![inputs[0].clone()],
                result: Ok(out_a),
            },
            morphix_core::ItemReport {
                inputs: vec![inputs[1].clone()],
                result: Err(morphix_core::MorphixError::Cancelled),
            },
            morphix_core::ItemReport {
                inputs: vec![inputs[2].clone()],
                result: Err(morphix_core::MorphixError::Cancelled),
            },
        ],
    };
    let entry = morphix_gui::commands::history_entry_for_report(&convert_job(inputs), &report);
    assert_eq!(entry.status, EntryStatus::Cancelled);
    assert!(
        entry
            .detail
            .starts_with("Đã huỷ · Đổi sang JPG · 1 thành công, 0 lỗi, 2 đã huỷ"),
        "{}",
        entry.detail
    );
}
