use morphix_gui::history::{EntryStatus, HistoryEntry};
use morphix_gui::state::{prepare_menu, push_history, AppPaths, AppState};
use std::path::PathBuf;

#[test]
fn prepare_menu_summarises_images_and_reports_total_size() {
    let d = tempfile::tempdir().unwrap();
    let a = d.path().join("Ảnh 1.png");
    image::RgbImage::from_pixel(10, 10, image::Rgb([9, 9, 9]))
        .save(&a)
        .unwrap();
    let payload = prepare_menu(&[a]).unwrap();
    assert_eq!(payload.files[0].name, "Ảnh 1.png");
    assert_eq!(payload.files[0].kind, "image");
    assert!(payload.total_bytes > 0);
    assert_eq!(payload.items[0].label, "PDF");
}

#[test]
fn prepare_menu_names_the_missing_file() {
    let err = prepare_menu(&[PathBuf::from("/không/có/file.jpg")]).unwrap_err();
    assert!(err.contains("file.jpg"), "{err}");
}

#[test]
fn prepare_menu_rejects_empty_request() {
    assert!(prepare_menu(&[]).is_err());
}

#[test]
fn app_paths_are_fixed_names_under_data_dir() {
    let p = AppPaths::from_data_dir(std::path::Path::new("/x"));
    assert_eq!(p.history.file_name().unwrap(), "history.json");
    assert_eq!(p.settings.file_name().unwrap(), "settings.json");
    assert_eq!(p.log.file_name().unwrap(), "morphix.log");
}

#[test]
fn push_history_stores_the_given_path_in_history() {
    let d = tempfile::tempdir().unwrap();
    let paths = AppPaths::from_data_dir(d.path());
    let state = AppState::load(paths);
    let out = d.path().join("Ảnh 1.webp");
    push_history(
        &state,
        HistoryEntry::new(
            &out,
            "Đổi sang WEBP",
            "Hoàn tất 1 file",
            EntryStatus::Done,
            1,
        ),
    );
    let history = state.history.lock().unwrap();
    assert_eq!(history.entries()[0].path, out.to_string_lossy());
}

#[test]
fn prepare_menu_names_a_subtitle_as_text_not_other() {
    let d = tempfile::tempdir().unwrap();
    let a = d.path().join("sub.srt");
    std::fs::write(&a, "1\n00:00:01,000 --> 00:00:02,000\nXin chào\n\n").unwrap();
    let payload = prepare_menu(&[a]).unwrap();
    assert_eq!(payload.files[0].kind, "text");
}
