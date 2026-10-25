use morphix_gui::tray::tray_items;

#[test]
fn tray_menu_lists_show_hide_settings_quit_in_order() {
    let items = tray_items();
    let ids: Vec<&str> = items.iter().map(|item| item.id).collect();
    let labels: Vec<&str> = items.iter().map(|item| item.label).collect();
    assert_eq!(ids, ["show", "hide", "settings", "quit"]);
    assert_eq!(labels, ["Hiện", "Ẩn", "Cài đặt", "Thoát"]);
}

#[test]
fn settings_is_enabled_now_that_the_settings_window_exists() {
    let settings = tray_items()
        .into_iter()
        .find(|item| item.id == "settings")
        .unwrap();
    assert!(settings.enabled);
}
