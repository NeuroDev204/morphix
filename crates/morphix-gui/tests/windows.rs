use morphix_gui::windows::{bottom_right_origin, close_hides_window, khay_size, WorkArea};

#[test]
fn bottom_right_origin_sits_in_the_corner_with_margin() {
    // WHY 1040: 1080 px monitor minus a 40 px taskbar that the work area excludes.
    let area = WorkArea {
        x: 0,
        y: 0,
        width: 1920,
        height: 1040,
    };
    assert_eq!(bottom_right_origin(area, 400, 520, 16), (1504, 504));
}

#[test]
fn bottom_right_origin_clamps_to_the_work_area_origin_when_window_is_too_big() {
    let area = WorkArea {
        x: 100,
        y: 50,
        width: 300,
        height: 200,
    };
    assert_eq!(bottom_right_origin(area, 400, 520, 16), (100, 50));
}

#[test]
fn close_hides_the_khay_and_menu_windows_instead_of_destroying_them() {
    assert!(close_hides_window("khay"));
    assert!(close_hides_window("menu"));
}

#[test]
fn close_hides_the_settings_window_too() {
    assert!(close_hides_window("settings"));
}

#[test]
fn close_does_not_intercept_unknown_windows() {
    assert!(!close_hides_window("main"));
}

#[test]
fn khay_size_falls_back_to_configured_size_before_first_show() {
    // WHY: cửa sổ chưa từng hiện trả kích thước 0/1 trên X11; dùng kích thước đó thì Khay nằm ngoài màn hình.
    assert_eq!(khay_size((1, 1), 1.0), (440, 700));
    assert_eq!(khay_size((0, 0), 2.0), (880, 1400));
    assert_eq!(khay_size((450, 710), 1.0), (450, 710));
}

#[test]
fn khay_fallback_size_matches_tauri_conf() {
    let conf: serde_json::Value = serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
    let khay = conf["app"]["windows"]
        .as_array()
        .unwrap()
        .iter()
        .find(|w| w["label"] == "khay")
        .unwrap();
    let configured = (
        khay["width"].as_f64().unwrap() as u32,
        khay["height"].as_f64().unwrap() as u32,
    );
    assert_eq!(khay_size((0, 0), 1.0), configured);
}
