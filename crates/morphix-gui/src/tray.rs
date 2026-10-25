//! Khay hệ thống: biểu tượng, bấm trái để mở hoặc ẩn Khay, menu phải Hiện / Ẩn / Cài đặt / Thoát.

use tauri::menu::{IsMenuItem, Menu, MenuEvent, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Wry};

use crate::commands::request_quit;
use crate::windows::{hide_khay, open_khay, open_settings, toggle_khay};

/// Tên hiện trong tooltip khay. Nguồn là `appName` trong `ui/src/strings.ts`.
pub const TRAY_TOOLTIP: &str = "Morphix";

/// Một mục trong menu khay. Chữ lấy từ `ui/src/strings.ts` (`trayShow`, `trayHide`, `traySettings`, `trayQuit`).
/// WHY copy: menu khay được dựng từ Rust trước khi có cửa sổ nào, nên không đọc được file TypeScript; giữ hai bảng khớp nhau bằng tay.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrayItem {
    pub id: &'static str,
    pub label: &'static str,
    pub enabled: bool,
}

/// Thứ tự và trạng thái các mục của menu khay; hàm thuần để test không cần Tauri.
pub fn tray_items() -> Vec<TrayItem> {
    vec![
        TrayItem {
            id: "show",
            label: "Hiện",
            enabled: true,
        },
        TrayItem {
            id: "hide",
            label: "Ẩn",
            enabled: true,
        },
        TrayItem {
            id: "settings",
            label: "Cài đặt",
            enabled: true,
        },
        TrayItem {
            id: "quit",
            label: "Thoát",
            enabled: true,
        },
    ]
}

/// Dựng biểu tượng khay và menu của nó. Lỗi trả về để `setup` quyết định cách xử lý.
pub fn build_tray(app: &AppHandle) -> tauri::Result<()> {
    let menu_items = tray_items()
        .into_iter()
        .map(|item| MenuItem::with_id(app, item.id, item.label, item.enabled, None::<&str>))
        .collect::<tauri::Result<Vec<MenuItem<Wry>>>>()?;
    let refs: Vec<&dyn IsMenuItem<Wry>> = menu_items
        .iter()
        .map(|item| item as &dyn IsMenuItem<Wry>)
        .collect();
    let menu = Menu::with_items(app, &refs)?;

    TrayIconBuilder::new()
        .icon(tauri::include_image!("./icons/32x32.png"))
        .tooltip(TRAY_TOOLTIP)
        // WHY false: bấm trái là mở hoặc ẩn Khay; menu chỉ hiện khi bấm phải.
        .show_menu_on_left_click(false)
        .menu(&menu)
        .on_menu_event(handle_menu_event)
        .on_tray_icon_event(handle_tray_event)
        .build(app)?;
    Ok(())
}

fn handle_menu_event(app: &AppHandle, event: MenuEvent) {
    let id: &str = event.id().as_ref();
    let result = match id {
        "show" => open_khay(app),
        "hide" => hide_khay(app),
        "settings" => open_settings(app),
        // WHY shared: Thoát đi cùng đường với nút trong Khay, nên RunEvent::Exit là nơi dừng dịch vụ duy nhất.
        "quit" => {
            request_quit(app);
            Ok(())
        }
        other => Err(format!("mục khay không có hành động: {other}")),
    };
    if let Err(reason) = result {
        tracing::error!(%reason, "lỗi khi chạy mục khay: {reason}");
    }
}

fn handle_tray_event(tray: &TrayIcon, event: TrayIconEvent) {
    // WHY Up: một cú bấm phát ra cả Down và Up; mở ở Up để không bật rồi tắt Khay trong cùng một cú bấm.
    if let TrayIconEvent::Click {
        button: MouseButton::Left,
        button_state: MouseButtonState::Up,
        ..
    } = event
    {
        if let Err(reason) = toggle_khay(tray.app_handle()) {
            tracing::error!(%reason, "không mở hoặc ẩn được Khay từ khay hệ thống: {reason}");
        }
    }
}
