use morphix_gui::settings::{load, save, validate, QualityChoice, Settings};

#[test]
fn missing_file_gives_defaults() {
    let d = tempfile::tempdir().unwrap();
    assert_eq!(load(&d.path().join("settings.json")), Settings::default());
}

#[test]
fn corrupt_file_gives_defaults_and_is_left_untouched() {
    let d = tempfile::tempdir().unwrap();
    let path = d.path().join("settings.json");
    std::fs::write(&path, b"{ not json").unwrap();
    assert_eq!(load(&path), Settings::default());
    assert_eq!(std::fs::read(&path).unwrap(), b"{ not json");
}

#[test]
fn save_then_load_round_trips_unicode_paths() {
    let d = tempfile::tempdir().unwrap();
    let path = d.path().join("settings.json");
    let s = Settings {
        quality: QualityChoice::High,
        output_dir: Some(d.path().join("Ảnh xuất ra")),
        default_resize_percent: 25,
    };
    save(&path, &s).unwrap();
    assert_eq!(load(&path), s);
}

#[test]
fn validate_rejects_bad_percent() {
    let s_zero = Settings {
        default_resize_percent: 0,
        ..Settings::default()
    };
    assert!(validate(&s_zero).is_err());

    let s_over = Settings {
        default_resize_percent: 101,
        ..Settings::default()
    };
    assert!(validate(&s_over).is_err());
}

#[test]
fn validate_creates_missing_output_dir() {
    let d = tempfile::tempdir().unwrap();
    let s = Settings {
        output_dir: Some(d.path().join("moi/thu-muc")),
        ..Settings::default()
    };
    validate(&s).unwrap();
    assert!(d.path().join("moi/thu-muc").is_dir());
}

#[test]
fn quality_choice_converts_to_core_quality() {
    assert_eq!(
        morphix_core::Quality::from(QualityChoice::Small),
        morphix_core::Quality::Small
    );
    assert_eq!(
        morphix_core::Quality::from(QualityChoice::Balanced),
        morphix_core::Quality::Balanced
    );
    assert_eq!(
        morphix_core::Quality::from(QualityChoice::High),
        morphix_core::Quality::High
    );
}

#[test]
fn save_overwrites_existing_file_cleanly() {
    let d = tempfile::tempdir().unwrap();
    let path = d.path().join("settings.json");
    let initial = Settings {
        quality: QualityChoice::Small,
        output_dir: None,
        default_resize_percent: 30,
    };
    save(&path, &initial).unwrap();
    assert_eq!(load(&path), initial);

    let updated = Settings {
        quality: QualityChoice::Balanced,
        output_dir: Some(d.path().join("updated")),
        default_resize_percent: 80,
    };
    save(&path, &updated).unwrap();
    assert_eq!(load(&path), updated);
}

#[test]
fn json_serialization_lowercase_quality() {
    let s = Settings {
        quality: QualityChoice::Balanced,
        output_dir: None,
        default_resize_percent: 50,
    };
    let json = serde_json::to_string(&s).unwrap();
    assert!(json.contains(r#""quality":"balanced""#));
}

