use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::{Mutex, MutexGuard};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use morphix_core::{detect, FileKind};
use serde::Serialize;

use crate::history::{History, HistoryEntry};
use crate::jobs::JobRunner;
use crate::menu_items::{wheel_items, WheelItem};
use crate::settings::{self, Settings};

/// Các đường dẫn cố định nằm trong thư mục dữ liệu của app.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppPaths {
    pub data_dir: PathBuf,
    pub history: PathBuf,
    pub settings: PathBuf,
    pub log: PathBuf,
}

impl AppPaths {
    /// Tên file được cố định để người dùng luôn tìm đúng dữ liệu sau khi cập nhật app.
    pub fn from_data_dir(dir: &Path) -> Self {
        Self {
            data_dir: dir.to_path_buf(),
            history: dir.join("history.json"),
            settings: dir.join("settings.json"),
            log: dir.join("morphix.log"),
        }
    }
}

/// Yêu cầu menu đang được gom lại, chờ giao diện đọc.
#[derive(Debug, Default)]
pub struct PendingMenu {
    pub paths: Vec<PathBuf>,
    pub collecting: bool,
    pub revision: u64,
    /// Chỉ để đo thời gian mở cửa sổ, không gửi cho giao diện nên không serialize.
    pub first_request_at: Option<Instant>,
}

impl PendingMenu {
    /// Ảnh chụp yêu cầu đang giữ, dạng giao diện đọc được.
    pub fn request_state(&self) -> MenuRequestState {
        MenuRequestState {
            paths: self
                .paths
                .iter()
                .map(|path| path.to_string_lossy().into_owned())
                .collect(),
            collecting: self.collecting,
            revision: self.revision,
        }
    }
}

/// Yêu cầu menu đang giữ, gửi cho giao diện qua `pending_request` và sự kiện `menu-request`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MenuRequestState {
    pub paths: Vec<String>,
    /// `true` khi còn lô đang gom, giao diện chưa được chọn múi.
    pub collecting: bool,
    /// Tăng mỗi khi có lô mới, để giao diện bỏ kết quả bất đồng bộ của lô cũ.
    pub revision: u64,
}

#[derive(Debug)]
pub struct KhayWait {
    pub deadline: Instant,
}

#[derive(Debug, PartialEq, Eq)]
pub enum BatchRoute {
    Khay,
    Wheel,
}

pub fn route_for_batch(wait: &mut Option<KhayWait>, now: Instant) -> BatchRoute {
    match wait.take() {
        Some(wait) if now < wait.deadline => BatchRoute::Khay,
        _ => BatchRoute::Wheel,
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct KhaySelection {
    pub paths: Vec<String>,
    pub error: Option<String>,
    pub revision: u64,
}

impl KhaySelection {
    pub fn with_files(prev: &Self, paths: Vec<String>) -> Self {
        Self {
            paths,
            error: None,
            revision: prev.revision + 1,
        }
    }
    pub fn with_error(prev: &Self, message: String) -> Self {
        Self {
            paths: Vec::new(),
            error: Some(message),
            revision: prev.revision + 1,
        }
    }
    pub fn cleared(prev: &Self) -> Self {
        Self::with_files(prev, Vec::new())
    }
}

/// Trạng thái dùng chung của app Tauri, quản lý qua `tauri::State`.
pub struct AppState {
    pub paths: AppPaths,
    pub history: Mutex<History>,
    pub settings: Mutex<Settings>,
    pub jobs: JobRunner,
    pub pending: Mutex<PendingMenu>,
    pub khay_wait: Mutex<Option<KhayWait>>,
    pub khay_selection: Mutex<KhaySelection>,
    pub capturing: AtomicBool,
    /// Socket được khởi tạo sau khi Tauri có `AppHandle`, để callback mở được cửa sổ (Task 12).
    pub ipc_server: Mutex<Option<morphix_ipc::IpcServer>>,
    /// Đặt khi bắt đầu thoát, trước khi join luồng IPC, để callback không chạm cửa sổ nữa.
    pub exiting: AtomicBool,
}

impl AppState {
    /// Nạp lịch sử và cài đặt từ `paths`.
    ///
    /// Không tạo được thư mục dữ liệu thì chỉ log lỗi: app vẫn khởi động với giá trị mặc định,
    /// vì một thư mục hỏng không đáng để chặn cả ứng dụng.
    pub fn load(paths: AppPaths) -> Self {
        if let Err(error) = std::fs::create_dir_all(&paths.data_dir) {
            tracing::error!(
                path = %paths.data_dir.display(),
                error = %error,
                "không tạo được thư mục dữ liệu {}: {error}",
                paths.data_dir.display()
            );
        }
        let history = History::open(&paths.history);
        let settings = settings::load(&paths.settings);
        Self {
            history: Mutex::new(history),
            settings: Mutex::new(settings),
            jobs: JobRunner::new(),
            pending: Mutex::new(PendingMenu::default()),
            khay_wait: Mutex::new(None),
            khay_selection: Mutex::new(KhaySelection::default()),
            capturing: AtomicBool::new(false),
            ipc_server: Mutex::new(None),
            exiting: AtomicBool::new(false),
            paths,
        }
    }

    pub fn khay_wait_guard(&self) -> MutexGuard<'_, Option<KhayWait>> {
        self.khay_wait
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
    pub fn khay_selection_guard(&self) -> MutexGuard<'_, KhaySelection> {
        self.khay_selection
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Khoá yêu cầu menu đang giữ. Khoá bị poison vẫn dùng được vì chỉ là trạng thái tạm.
    pub fn pending_guard(&self) -> MutexGuard<'_, PendingMenu> {
        self.pending
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Khoá cài đặt đang dùng. Poison vẫn dùng được: giá trị đã ghi đầy đủ trước khi khoá được giải phóng.
    pub fn settings_guard(&self) -> MutexGuard<'_, Settings> {
        self.settings
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Khoá lịch sử. Poison vẫn dùng được: lịch sử chỉ là dữ liệu phụ, không để một panic cũ chặn việc ghi hay đọc.
    pub fn history_guard(&self) -> MutexGuard<'_, History> {
        self.history
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

/// Tóm tắt một file để hiển thị trong vòng tròn.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FileSummary {
    pub name: String,
    /// `image`, `pdf`, `text` hoặc `other`.
    pub kind: String,
}

/// Dữ liệu đủ để giao diện vẽ vòng tròn cho một lô file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WheelPayload {
    pub files: Vec<FileSummary>,
    pub items: Vec<WheelItem>,
    pub total_bytes: u64,
}

fn kind_label(kind: FileKind) -> &'static str {
    match kind {
        FileKind::Image(_) => "image",
        FileKind::Pdf => "pdf",
        // WHY: giao diện chỉ dùng kind làm mã định danh, không hiện chữ nên không thêm nhãn tiếng Việt.
        FileKind::Text(_) => "text",
        _ => "other",
    }
}

/// Đọc và nhận dạng từng file để dựng vòng tròn.
///
/// Mọi lỗi đều trả về chuỗi có tên file, để người dùng biết file nào gây lỗi thay vì một thông báo chung.
pub fn prepare_menu(paths: &[PathBuf]) -> Result<WheelPayload, String> {
    if paths.is_empty() {
        return Err("Chưa chọn file nào để xử lý.".to_string());
    }

    let mut files = Vec::with_capacity(paths.len());
    let mut kinds = Vec::with_capacity(paths.len());
    let mut total_bytes: u64 = 0;

    for path in paths {
        let kind = detect(path).map_err(|e| format!("Không đọc được {}: {e}", path.display()))?;
        let size = std::fs::metadata(path)
            .map_err(|e| format!("Không đọc được {}: {e}", path.display()))?
            .len();
        total_bytes += size;
        files.push(FileSummary {
            name: path
                .file_name()
                .unwrap_or(path.as_os_str())
                .to_string_lossy()
                .into_owned(),
            kind: kind_label(kind).to_string(),
        });
        kinds.push(kind);
    }

    Ok(WheelPayload {
        files,
        items: wheel_items(&kinds),
        total_bytes,
    })
}

/// Giây Unix hiện tại cho dấu thời gian lịch sử.
/// WHY 0 on failure: đồng hồ lùi về trước 1970 không được làm hỏng việc đã xong, chỉ lệch mốc thời gian.
pub fn unix_now() -> u64 {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(elapsed) => elapsed.as_secs(),
        Err(error) => {
            tracing::warn!(error = %error, "đồng hồ hệ thống lùi về trước năm 1970, ghi thời gian 0");
            0
        }
    }
}

/// Ghi một dòng lịch sử đã dựng sẵn. Lỗi ghi chỉ được log: kết quả việc đã chạy vẫn hiện ra cho người dùng.
pub fn push_history(state: &AppState, entry: HistoryEntry) {
    let mut history = state.history_guard();
    if let Err(error) = history.push(entry) {
        tracing::error!(
            path = %history.path().display(),
            error = %error,
            "không ghi được lịch sử: {error}"
        );
    }
}

#[cfg(test)]
mod selection_tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn route_for_batch_consumes_only_unexpired_wait() {
        let now = Instant::now();
        let mut wait = Some(KhayWait {
            deadline: now + Duration::from_secs(2),
        });
        assert_eq!(route_for_batch(&mut wait, now), BatchRoute::Khay);
        assert!(wait.is_none());
        assert_eq!(route_for_batch(&mut wait, now), BatchRoute::Wheel);
        wait = Some(KhayWait { deadline: now });
        assert_eq!(route_for_batch(&mut wait, now), BatchRoute::Wheel);
        assert!(wait.is_none());
    }

    #[test]
    fn khay_selection_revisions_replace_files_and_errors() {
        let initial = KhaySelection::default();
        let files = KhaySelection::with_files(&initial, vec!["ảnh.png".into()]);
        assert_eq!(files.revision, 1);
        assert_eq!(files.paths, ["ảnh.png"]);
        assert_eq!(files.error, None);
        let error = KhaySelection::with_error(&files, "lỗi".into());
        assert_eq!(error.revision, 2);
        assert!(error.paths.is_empty());
        assert_eq!(error.error.as_deref(), Some("lỗi"));
        let cleared = KhaySelection::cleared(&error);
        assert_eq!(cleared.revision, 3);
        assert!(cleared.paths.is_empty());
        assert!(cleared.error.is_none());
        let files = KhaySelection::with_files(&error, vec!["a.jpg".into()]);
        assert!(files.error.is_none());
    }
}
