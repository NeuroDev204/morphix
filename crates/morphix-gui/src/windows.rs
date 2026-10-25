//! Điều khiển cửa sổ Menu nhanh: lưu lô file, định vị theo con trỏ rồi báo giao diện vẽ lại.

use std::path::PathBuf;
use std::time::Instant;

use tauri::{AppHandle, Emitter, LogicalPosition, Manager, PhysicalPosition, WebviewWindow};

use crate::state::AppState;

pub const MENU_WINDOW_LABEL: &str = "menu";
pub const MENU_REQUEST_EVENT: &str = "menu-request";
pub const KHAY_WINDOW_LABEL: &str = "khay";
pub const SETTINGS_WINDOW_LABEL: &str = "settings";
/// Khoảng cách logic từ mép vùng làm việc tới khay, để cửa sổ không dính sát viền màn hình.
const KHAY_MARGIN_LOGICAL_PX: f64 = 16.0;
/// Kích thước Khay khai báo trong `tauri.conf.json` (điểm ảnh logic).
const KHAY_LOGICAL_SIZE: (f64, f64) = (440.0, 700.0);

/// Kích thước thật của Khay, hoặc kích thước khai báo (nhân tỉ lệ) khi cửa sổ chưa hiện lần nào.
/// WHY: trên X11, cửa sổ chưa từng hiện trả kích thước 0/1; tính góc dưới phải bằng số đó thì Khay nằm ngoài
/// màn hình, và chỉ trình quản lý cửa sổ nào tự kéo cửa sổ vào lại (như GNOME) mới che được lỗi.
pub fn khay_size(actual: (u32, u32), scale: f64) -> (u32, u32) {
    if actual.0 > 1 && actual.1 > 1 {
        return actual;
    }
    (
        (KHAY_LOGICAL_SIZE.0 * scale).round() as u32,
        (KHAY_LOGICAL_SIZE.1 * scale).round() as u32,
    )
}

/// Vùng làm việc của màn hình theo pixel vật lý: đã trừ thanh tác vụ mà hệ điều hành báo cáo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorkArea {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

/// Toạ độ góc trên-trái để cửa sổ nằm ở góc dưới phải của vùng làm việc, cách mép `margin` pixel.
///
/// Hàm thuần không phụ thuộc Tauri. Khi cửa sổ lớn hơn vùng làm việc thì kẹp về góc trên-trái
/// của vùng để cửa sổ không bị đẩy ra ngoài màn hình.
pub fn bottom_right_origin(
    area: WorkArea,
    window_width: u32,
    window_height: u32,
    margin: u32,
) -> (i32, i32) {
    // WHY i64: cộng trừ kích thước vùng và cửa sổ có thể âm tạm thời, và i32 có thể tràn với toạ độ màn hình lớn.
    let left = i64::from(area.x);
    let top = i64::from(area.y);
    let right_edge = left + i64::from(area.width) - i64::from(window_width) - i64::from(margin);
    let bottom_edge = top + i64::from(area.height) - i64::from(window_height) - i64::from(margin);
    let x = right_edge.max(left);
    let y = bottom_edge.max(top);
    (x as i32, y as i32)
}

/// Cửa sổ nào khi người dùng yêu cầu đóng thì ẩn thay vì huỷ.
///
/// WHY: hai cửa sổ này không có khung nên Alt+F4 hoặc lệnh đóng của WM là đường đóng duy nhất;
/// nếu để cửa sổ bị huỷ thì `open_khay`/`open_menu` lỗi cho tới khi khởi động lại app.
pub fn close_hides_window(label: &str) -> bool {
    matches!(
        label,
        MENU_WINDOW_LABEL | KHAY_WINDOW_LABEL | SETTINGS_WINDOW_LABEL
    )
}

fn khay_window(app: &AppHandle) -> Result<WebviewWindow, String> {
    app.get_webview_window(KHAY_WINDOW_LABEL)
        .ok_or_else(|| "Không tìm thấy cửa sổ Khay.".to_string())
}

/// Đặt app ở góc dưới phải; nếu không đọc/đặt được vị trí thì căn giữa, rồi luôn thử hiện cửa sổ.
pub fn open_khay(app: &AppHandle) -> Result<(), String> {
    let window = khay_window(app)?;
    let position = window
        .primary_monitor()
        .map_err(|error| format!("Không đọc được màn hình chính: {error}"))
        .and_then(|monitor| monitor.ok_or_else(|| "Không tìm thấy màn hình chính.".to_string()))
        .and_then(|monitor| {
            let area = monitor.work_area();
            let size = window
                .outer_size()
                .map_err(|error| format!("Không đọc được kích thước cửa sổ Khay: {error}"))?;
            let (width, height) = khay_size((size.width, size.height), monitor.scale_factor());
            // WHY scaled: lề 16 px là kích thước logic, trên màn hình HiDPI phải nhân theo tỉ lệ để có cùng khoảng cách.
            let margin = (KHAY_MARGIN_LOGICAL_PX * monitor.scale_factor()).round() as u32;
            let (x, y) = bottom_right_origin(
                WorkArea {
                    x: area.position.x,
                    y: area.position.y,
                    width: area.size.width,
                    height: area.size.height,
                },
                width,
                height,
                margin,
            );
            Ok(PhysicalPosition::new(x, y))
        });
    match position {
        Ok(position) => {
            if let Err(error) = window.set_position(position) {
                tracing::warn!(error = %error, "compositor từ chối đặt vị trí Khay; chuyển về giữa màn hình: {error}");
                center_khay(&window);
            }
        }
        Err(reason) => {
            tracing::warn!(%reason, "không đặt được vị trí Khay; chuyển về giữa màn hình: {reason}");
            center_khay(&window);
        }
    }
    window
        .show()
        .map_err(|error| format!("Không hiện được cửa sổ Khay: {error}"))?;
    if let Err(error) = window.set_focus() {
        // WHY: không lấy được focus thì khay vẫn dùng được bằng chuột.
        tracing::warn!(error = %error, "không đưa được cửa sổ Khay lên trước: {error}");
    }
    Ok(())
}

fn center_khay(window: &WebviewWindow) {
    if let Err(error) = window.center() {
        tracing::warn!(error = %error, "không căn giữa được cửa sổ Khay: {error}");
    }
}

/// Ẩn cửa sổ Khay. Khay đang ẩn thì hàm không làm gì thêm ngoài việc ẩn lại.
pub fn hide_khay(app: &AppHandle) -> Result<(), String> {
    khay_window(app)?
        .hide()
        .map_err(|error| format!("Không ẩn được cửa sổ Khay: {error}"))
}

/// Hiện cửa sổ Cài đặt và đưa lên trước. Cửa sổ bị ẩn khi đóng nên mở lại chỉ cần hiện, không tạo mới.
pub fn open_settings(app: &AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window(SETTINGS_WINDOW_LABEL)
        .ok_or_else(|| "Không tìm thấy cửa sổ Cài đặt.".to_string())?;
    if let Err(error) = window.center() {
        // WHY: không căn giữa được thì cửa sổ vẫn hiện ở vị trí cũ và dùng được.
        tracing::warn!(error = %error, "không căn giữa được cửa sổ Cài đặt: {error}");
    }
    // WHY: GNOME chặn set_focus kéo cửa sổ lên trước (chống cướp focus), còn Khay luôn ở trên;
    // không đặt luôn-ở-trên thì Cài đặt hiện nhưng nằm khuất sau cửa sổ khác.
    if let Err(error) = window.set_always_on_top(true) {
        tracing::warn!(error = %error, "không đặt được cửa sổ Cài đặt luôn ở trên: {error}");
    }
    window
        .show()
        .map_err(|error| format!("Không hiện được cửa sổ Cài đặt: {error}"))?;
    if let Err(error) = window.set_focus() {
        // WHY: không lấy được focus thì cửa sổ vẫn hiện và dùng được bằng chuột.
        tracing::warn!(error = %error, "không đưa được cửa sổ Cài đặt lên trước: {error}");
    }
    Ok(())
}

/// Bấm trái khay: đang hiện thì ẩn, đang ẩn thì mở lại đúng vị trí góc dưới phải.
pub fn toggle_khay(app: &AppHandle) -> Result<(), String> {
    let visible = khay_window(app)?
        .is_visible()
        .map_err(|error| format!("Không đọc được trạng thái cửa sổ Khay: {error}"))?;
    if visible {
        hide_khay(app)
    } else {
        open_khay(app)
    }
}

/// Đưa lô file mới vào cửa sổ Menu nhanh và hiện cửa sổ nếu đang ẩn.
///
/// Cửa sổ đã hiện thì chỉ cập nhật lô, không định vị lại và không tạo cửa sổ thứ hai.
/// Giao diện vẫn đọc `pending_request` khi tải nên không mất lô nếu sự kiện đến trước khi trang sẵn sàng.
pub fn open_menu(app: &AppHandle, paths: Vec<PathBuf>, collecting: bool) -> Result<(), String> {
    let window = app
        .get_webview_window(MENU_WINDOW_LABEL)
        .ok_or_else(|| "Không tìm thấy cửa sổ Menu nhanh.".to_string())?;
    let was_visible = window
        .is_visible()
        .map_err(|error| format!("Không đọc được trạng thái cửa sổ Menu nhanh: {error}"))?;

    let state = app.state::<AppState>();
    let request = {
        let mut pending = state.pending_guard();
        pending.paths = paths;
        pending.collecting = collecting;
        pending.revision += 1;
        if !was_visible {
            pending.first_request_at = Some(Instant::now());
        }
        pending.request_state()
    };

    if !was_visible {
        place_near_cursor(&window);
        window
            .show()
            .map_err(|error| format!("Không hiện được cửa sổ Menu nhanh: {error}"))?;
        // WHY take: mốc chỉ đo một lần cho mỗi lần mở; lần mở sau phải bắt đầu mốc mới.
        let first_request: Option<Instant> = state.pending_guard().first_request_at.take();
        if let Some(started) = first_request {
            let elapsed_ms: u64 = started.elapsed().as_millis() as u64;
            tracing::info!("menu shown in {elapsed_ms} ms");
        }
        if let Err(error) = window.set_focus() {
            // WHY: không lấy được focus thì cửa sổ vẫn dùng được bằng chuột; chỉ mất cơ chế tự ẩn khi blur.
            tracing::warn!(error = %error, "không đưa được cửa sổ Menu nhanh lên trước: {error}");
        }
    }

    app.emit(MENU_REQUEST_EVENT, request)
        .map_err(|error| format!("Không gửi được lô file tới Menu nhanh: {error}"))
}

/// Đặt tâm vòng tròn ở con trỏ, kẹp vào vùng làm việc của màn hình chứa con trỏ.
///
/// Không lấy được con trỏ (Wayland không cho đọc) hoặc compositor từ chối đặt vị trí thì
/// chỉ log và để cửa sổ ở giữa màn hình; không panic vì vị trí không ảnh hưởng đến việc xử lý file.
fn place_near_cursor(window: &WebviewWindow) {
    match cursor_anchor(window) {
        Ok(position) => {
            if let Err(error) = window.set_position(position) {
                tracing::warn!(error = %error, "compositor từ chối đặt vị trí Menu nhanh: {error}");
            }
        }
        Err(reason) => {
            tracing::warn!(reason = %reason, "không đặt được Menu nhanh theo con trỏ, chuyển về giữa màn hình: {reason}");
            if let Err(error) = window.center() {
                tracing::warn!(error = %error, "không căn giữa được cửa sổ Menu nhanh: {error}");
            }
        }
    }
}

/// Tính vị trí logic để tâm cửa sổ trùng con trỏ, đã kẹp trong vùng làm việc của màn hình.
fn cursor_anchor(window: &WebviewWindow) -> Result<LogicalPosition<f64>, String> {
    let cursor = window
        .cursor_position()
        .map_err(|error| format!("không lấy được vị trí con trỏ: {error}"))?;
    let monitor = window
        .monitor_from_point(cursor.x, cursor.y)
        .map_err(|error| format!("không tìm được màn hình: {error}"))?
        .ok_or_else(|| "con trỏ không nằm trên màn hình nào".to_string())?;
    let size = window
        .outer_size()
        .map_err(|error| format!("không đọc được kích thước cửa sổ: {error}"))?;

    // WHY physical: con trỏ và vùng làm việc đều là pixel vật lý, nên kẹp trước rồi mới đổi sang logic.
    let area = monitor.work_area();
    let width = f64::from(size.width);
    let height = f64::from(size.height);
    let min_x = f64::from(area.position.x);
    let min_y = f64::from(area.position.y);
    // max(min) tránh panic của clamp khi cửa sổ lớn hơn vùng làm việc.
    let max_x = (min_x + f64::from(area.size.width) - width).max(min_x);
    let max_y = (min_y + f64::from(area.size.height) - height).max(min_y);
    let x = (cursor.x - width / 2.0).clamp(min_x, max_x);
    let y = (cursor.y - height / 2.0).clamp(min_y, max_y);

    let scale = monitor.scale_factor();
    Ok(LogicalPosition::new(x / scale, y / scale))
}

pub const KHAY_SELECTION_EVENT: &str = "khay-selection";

pub fn open_khay_with_files(app: &AppHandle, paths: Vec<PathBuf>) -> Result<(), String> {
    set_khay_selection(
        app,
        paths
            .into_iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect(),
        None,
    )
}

pub fn open_khay_with_error(app: &AppHandle, message: String) -> Result<(), String> {
    tracing::error!(%message, "không lấy được file đang chọn");
    set_khay_selection(app, Vec::new(), Some(message))
}

fn set_khay_selection(
    app: &AppHandle,
    paths: Vec<String>,
    error: Option<String>,
) -> Result<(), String> {
    let state = app.state::<AppState>();
    // WHY: Khay đọc lại trạng thái lúc tải nên không mất file khi sự kiện tới trước giao diện.
    let selection = {
        let mut selection = state.khay_selection_guard();
        *selection = match error {
            Some(message) => crate::state::KhaySelection::with_error(&selection, message),
            None => crate::state::KhaySelection::with_files(&selection, paths),
        };
        selection.clone()
    };
    let emitted = app
        .emit(KHAY_SELECTION_EVENT, selection)
        .map_err(|error| error.to_string());
    open_khay(app)?;
    emitted
}
