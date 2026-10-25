// WHY source check: Builder của Tauri không cho đọc danh sách plugin đã đăng ký lúc chạy, nên
// test đọc thẳng src/lib.rs. Thiếu dòng này thì file picker báo "plugin dialog not found" mà
// không có lỗi biên dịch nào.
#[test]
fn dialog_plugin_is_registered_in_run() {
    let source: &str = include_str!("../src/lib.rs");
    assert!(
        source.contains(".plugin(tauri_plugin_dialog::init())"),
        "run() phải đăng ký tauri_plugin_dialog::init() để khay và settings mở được hộp chọn file"
    );
}
