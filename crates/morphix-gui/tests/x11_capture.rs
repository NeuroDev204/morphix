#![cfg(target_os = "linux")]

use morphix_gui::capture::{x11::X11Session, KeyInjector};

#[test]
#[ignore = "cần phiên X11 đang chạy; chỉ đọc cửa sổ/phím, không bấm phím hay sửa clipboard"]
fn reads_real_x11_session_without_modifying_it() {
    let mut session = X11Session::connect().unwrap();
    println!(
        "active target: {:?}",
        session.active_window_target().unwrap()
    );
    println!("modifiers held: {}", session.modifiers_held().unwrap());
    println!("clipboard owner: {}", session.selection_owner().unwrap());
}

#[test]
#[ignore = "cần GTK trên phiên X11; chỉ đọc các định dạng, không ghi clipboard"]
fn reads_gtk_clipboard_targets_and_connects_owner_change_signal() {
    use morphix_gui::capture::clipboard::{ClipboardAccess, GtkClipboard};
    gtk::init().unwrap();
    let mut clipboard = GtkClipboard::new().unwrap();
    println!(
        "clipboard target count: {}",
        clipboard.targets().unwrap().len()
    );
    let _changes = clipboard.owner_changes();
}
