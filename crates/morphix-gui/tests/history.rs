use morphix_gui::history::{format_size_vn, EntryStatus, History, HistoryEntry, MAX_ENTRIES};

fn entry(n: usize) -> HistoryEntry {
    HistoryEntry {
        path: format!("/tmp/file-{n}.jpg"),
        title: format!("việc {n}"),
        detail: "Đã xong".into(),
        status: EntryStatus::Done,
        finished_at_unix: n as u64,
    }
}

#[test]
fn keeps_only_the_newest_fifty() {
    let d = tempfile::tempdir().unwrap();
    let mut h = History::open(&d.path().join("history.json"));
    for n in 0..(MAX_ENTRIES + 5) {
        h.push(entry(n)).unwrap();
    }
    assert_eq!(h.recent(usize::MAX).len(), MAX_ENTRIES);
    assert_eq!(h.recent(1)[0].title, format!("việc {}", MAX_ENTRIES + 4));
}

#[test]
fn survives_reopen_with_unicode_titles() {
    let d = tempfile::tempdir().unwrap();
    let path = d.path().join("history.json");
    History::open(&path).push(entry(1)).unwrap();
    assert_eq!(History::open(&path).recent(10)[0].title, "việc 1");
}

#[test]
fn corrupt_file_starts_empty_and_is_not_deleted_until_first_write() {
    let d = tempfile::tempdir().unwrap();
    let path = d.path().join("history.json");
    std::fs::write(&path, b"[[[").unwrap();
    let mut h = History::open(&path);
    assert!(h.recent(10).is_empty());
    assert_eq!(std::fs::read(&path).unwrap(), b"[[[");
    h.push(entry(1)).unwrap();
    assert_eq!(History::open(&path).recent(10).len(), 1);
}

#[test]
fn clear_empties_the_file_on_disk() {
    let d = tempfile::tempdir().unwrap();
    let path = d.path().join("history.json");
    let mut h = History::open(&path);
    h.push(entry(1)).unwrap();
    h.clear().unwrap();
    assert!(History::open(&path).recent(10).is_empty());
}

#[test]
fn entry_status_serde_lowercase() {
    let done_json = serde_json::to_string(&EntryStatus::Done).unwrap();
    let failed_json = serde_json::to_string(&EntryStatus::Failed).unwrap();
    assert_eq!(done_json, "\"done\"");
    assert_eq!(failed_json, "\"failed\"");

    let de_done: EntryStatus = serde_json::from_str("\"done\"").unwrap();
    let de_failed: EntryStatus = serde_json::from_str("\"failed\"").unwrap();
    assert_eq!(de_done, EntryStatus::Done);
    assert_eq!(de_failed, EntryStatus::Failed);
}

#[test]
fn entry_constructor_converts_path_lossily() {
    let p = std::path::Path::new("/tmp/ảnh đẹp.png");
    let e = HistoryEntry::new(p, "Tiêu đề", "Chi tiết", EntryStatus::Done, 12345);
    assert_eq!(e.path, "/tmp/ảnh đẹp.png");
    assert_eq!(e.title, "Tiêu đề");
    assert_eq!(e.detail, "Chi tiết");
    assert_eq!(e.status, EntryStatus::Done);
    assert_eq!(e.finished_at_unix, 12345);
}

#[test]
fn creates_nested_directories_if_parent_missing() {
    let d = tempfile::tempdir().unwrap();
    let path = d.path().join("nested").join("sub").join("history.json");
    let mut h = History::open(&path);
    assert!(h.is_empty());
    h.push(entry(1)).unwrap();
    assert_eq!(h.len(), 1);
    assert_eq!(History::open(&path).len(), 1);
}

#[test]
fn handles_relative_path_without_parent_directory() {
    let d = tempfile::tempdir().unwrap();
    // Use current_dir scope safely inside tempdir
    let original = std::env::current_dir().unwrap();
    std::env::set_current_dir(d.path()).unwrap();

    let relative_path = std::path::Path::new("history_relative.json");
    let mut h = History::open(relative_path);
    h.push(entry(99)).unwrap();

    assert_eq!(History::open(relative_path).recent(1)[0].title, "việc 99");
    std::env::set_current_dir(original).unwrap();
}


#[test]
fn format_size_vn_uses_one_decimal_below_ten_megabytes() {
    assert_eq!(format_size_vn(8_808_038), "8,4 MB");
}

#[test]
fn format_size_vn_uses_whole_megabytes_from_ten_up() {
    assert_eq!(format_size_vn(412 * 1024 * 1024), "412 MB");
}

#[test]
fn format_size_vn_uses_kilobytes_below_one_megabyte() {
    assert_eq!(format_size_vn(512 * 1024), "512 KB");
}

#[test]
fn format_size_vn_keeps_small_files_out_of_zero_kilobytes() {
    assert_eq!(format_size_vn(999), "999 B");
}
