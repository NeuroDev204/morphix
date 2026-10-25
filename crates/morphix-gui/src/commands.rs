//! Lệnh Tauri mỏng cho cửa sổ Menu nhanh: mỗi lệnh chỉ nối giao diện với lõi đã có sẵn.

use std::path::{Path, PathBuf};
use std::sync::Arc;
#[cfg(target_os = "linux")]
use std::{ffi::OsString, process::Command};

use morphix_core::job::require_ffmpeg;
use morphix_core::{Job, JobReport, MorphixError, Operation};
use morphix_ipc::{send_request, socket_name, MenuRequest};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::history::{format_size_vn, EntryStatus, History, HistoryEntry};
use crate::jobs::{describe_error, JobSnapshot, JobState};
use crate::menu_items::{operation_for, quality_for};
use crate::settings::{save, validate, Settings};
use crate::state::{
    prepare_menu, push_history, unix_now, AppState, MenuRequestState, WheelPayload,
};
use crate::windows;

pub const JOB_UPDATE_EVENT: &str = "job-update";
pub const JOB_FINISHED_EVENT: &str = "job-finished";

/// Chuyển danh sách đường dẫn từ giao diện thành `PathBuf`, từ chối mục rỗng hoặc chỉ có khoảng trắng.
/// WHY: một đường dẫn rỗng sẽ trỏ vào thư mục hiện tại và xử lý nhầm, nên phải chặn ngay ở biên.
pub fn parse_paths(raw: Vec<String>) -> Result<Vec<PathBuf>, String> {
    if raw.is_empty() {
        return Err("Chưa chọn file nào để xử lý.".to_string());
    }
    if raw.iter().any(|path| path.trim().is_empty()) {
        return Err("Có đường dẫn rỗng trong danh sách file.".to_string());
    }
    Ok(raw.into_iter().map(PathBuf::from).collect())
}

#[tauri::command]
pub fn menu_payload(paths: Vec<String>) -> Result<WheelPayload, String> {
    prepare_menu(&parse_paths(paths)?)
}

#[tauri::command]
pub fn run_action(
    id: String,
    paths: Vec<String>,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<u64, String> {
    let inputs = parse_paths(paths)?;
    // Dựng lại vòng để lấy nhãn đúng cho việc này và chặn thao tác không áp dụng cho loại file đã chọn.
    let label = prepare_menu(&inputs)?
        .items
        .into_iter()
        .find(|item| item.id == id)
        .map(|item| item.label)
        .ok_or_else(|| "Thao tác này không áp dụng cho các file đã chọn.".to_string())?;

    let settings = state.settings_guard().clone();
    let operation = operation_for(&id, &inputs, &settings)?;
    // WHY: thiếu ffmpeg thì báo ngay qua lỗi của lệnh (banner), không khởi động một job chắc chắn hỏng.
    if matches!(
        operation,
        Operation::ConvertMedia { .. } | Operation::CompressMedia
    ) {
        require_ffmpeg().map_err(|error| error.to_string())?;
    }
    let job = Job {
        inputs,
        operation,
        quality: quality_for(&settings),
        out_dir: settings.output_dir.clone(),
    };

    // WHY clone: the finished job is needed after the runner takes ownership, to describe it in history.
    let history_job = job.clone();
    let handle = app.clone();
    let on_update =
        Arc::new(move |snapshot: JobSnapshot| publish_update(&handle, snapshot, &history_job));
    state.jobs.start(job, label, on_update)
}

#[tauri::command]
pub fn cancel_job(id: u64, state: State<'_, AppState>) -> bool {
    state.jobs.cancel(id)
}

#[tauri::command]
pub fn active_job(state: State<'_, AppState>) -> Option<JobSnapshot> {
    state.jobs.active()
}

#[tauri::command]
pub fn pending_request(state: State<'_, AppState>) -> MenuRequestState {
    state.pending_guard().request_state()
}

/// Bỏ lô đang giữ khi người dùng chọn Bỏ qua.
/// WHY: kiểm tra cả danh sách lẫn revision, để không xoá nhầm lô mới vừa tới sau khi giao diện đọc trạng thái.
#[tauri::command]
pub fn discard_pending(
    paths: Vec<String>,
    revision: u64,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let mut pending = state.pending_guard();
    let current = pending.request_state();
    if current.paths != paths || current.revision != revision {
        return Err("Lô file đã thay đổi, không bỏ qua lô mới.".to_string());
    }
    pending.paths.clear();
    pending.collecting = false;
    pending.first_request_at = None;
    Ok(())
}

/// Các dòng lịch sử mới nhất, tối đa `limit`; phần thuần của `recent_history` để test không cần Tauri.
pub fn recent_from(history: &History, limit: usize) -> Vec<HistoryEntry> {
    history.recent(limit).to_vec()
}

/// Lệnh cho khay: trả về tối đa `limit` dòng mới nhất, mới nhất đứng đầu.
#[tauri::command]
pub fn recent_history(limit: usize, state: State<'_, AppState>) -> Vec<HistoryEntry> {
    recent_from(&state.history_guard(), limit)
}

pub fn output_directory_for_history(history: &History, path: &str) -> Result<PathBuf, String> {
    if !history
        .entries()
        .iter()
        .any(|entry| entry.status == EntryStatus::Done && entry.path == path)
    {
        return Err("Chỉ có thể mở thư mục của kết quả thành công trong lịch sử.".to_string());
    }
    let output = Path::new(path);
    if !output.is_absolute() || path.starts_with("file://") {
        return Err("Đường dẫn đầu ra không phải file cục bộ hợp lệ.".to_string());
    }
    match std::fs::symlink_metadata(output) {
        Ok(metadata) if metadata.file_type().is_file() => {}
        Ok(_) => return Err("Đường dẫn đầu ra không phải file thường.".to_string()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err("Không tìm thấy file đầu ra.".to_string())
        }
        Err(error) => return Err(format!("Không kiểm tra được file đầu ra: {error}")),
    }
    let parent = output
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .ok_or_else(|| "Không xác định được thư mục đầu ra.".to_string())?;
    if !parent.is_dir() {
        return Err("Thư mục đầu ra không còn tồn tại.".to_string());
    }
    Ok(parent.to_path_buf())
}

#[cfg(target_os = "linux")]
fn gio_open_args(directory: &Path) -> [OsString; 2] {
    [OsString::from("open"), directory.as_os_str().to_os_string()]
}

#[tauri::command]
pub fn open_output_directory(path: String, state: State<'_, AppState>) -> Result<(), String> {
    let directory = output_directory_for_history(&state.history_guard(), &path)?;
    #[cfg(target_os = "linux")]
    {
        let status = Command::new("gio")
            .args(gio_open_args(&directory))
            .status()
            .map_err(|error| format!("Không mở được trình quản lý file: {error}"))?;
        if status.success() {
            Ok(())
        } else {
            Err(format!("gio open kết thúc với mã {}.", status))
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = directory;
        Err("Mở thư mục đầu ra hiện chỉ hỗ trợ Ubuntu/Linux.".to_string())
    }
}

/// Xoá toàn bộ lịch sử, báo lỗi ghi file cho giao diện hiện lên thay vì chỉ nằm trong log.
#[tauri::command]
pub fn clear_history(state: State<'_, AppState>) -> Result<(), String> {
    state.history_guard().clear()
}

/// Cài đặt đang áp dụng, để trang Cài đặt điền form khi mở.
#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> Settings {
    state.settings_guard().clone()
}

/// Kiểm tra rồi ghi cài đặt ra `path`; phần thuần để test không cần Tauri.
/// WHY validate first: một giá trị sai không được ghi đè file cài đặt đang dùng được.
pub fn save_settings_to(path: &Path, settings: Settings) -> Result<(), String> {
    validate(&settings)?;
    save(path, &settings)
}

/// Lưu cài đặt từ trang Cài đặt. Ghi file xong mới đổi trạng thái đang dùng.
/// WHY lock across the write: hai lần lưu đồng thời không được để file và bộ nhớ lệch nhau.
#[tauri::command]
pub fn save_settings(settings: Settings, state: State<'_, AppState>) -> Result<(), String> {
    let mut current = state.settings_guard();
    save_settings_to(&state.paths.settings, settings.clone())?;
    *current = settings;
    Ok(())
}

/// Mở cửa sổ Cài đặt từ nút bánh răng trong Khay.
#[tauri::command]
pub fn open_settings(app: AppHandle) -> Result<(), String> {
    windows::open_settings(&app)
}

/// Yêu cầu thoát app từ giao diện khay.
#[tauri::command]
pub fn quit(app: AppHandle) -> Result<(), String> {
    request_quit(&app);
    Ok(())
}

/// Đường thoát duy nhất: `RunEvent::Exit` trong `lib.rs` mới dừng dịch vụ, nên nút Thoát và khay cùng đi qua đây.
pub fn request_quit(app: &AppHandle) {
    app.exit(0);
}

/// Gửi file từ giao diện qua cùng socket với Khay và CLI, để mọi nguồn đi chung luồng gom lô.
#[tauri::command]
pub fn submit_menu_request(paths: Vec<String>) -> Result<(), String> {
    let request = MenuRequest {
        paths: parse_paths(paths)?,
    };
    send_request(&socket_name(), &request)
        .map_err(|error| format!("Không gửi được yêu cầu tới Morphix đang chạy: {error}"))
}

/// Đẩy tiến trình ra giao diện. Khi việc kết thúc thì ghi lịch sử đúng một lần rồi mới báo `job-finished`.
/// WHY: ghi ở đây thay vì trong cửa sổ, để việc vẫn được ghi dù người dùng đã đóng vòng tròn.
fn publish_update(app: &AppHandle, snapshot: JobSnapshot, job: &Job) {
    if snapshot.state == JobState::Running {
        emit_or_log(app, JOB_UPDATE_EVENT, snapshot);
        return;
    }
    record_finished(app, &snapshot, job);
    emit_or_log(app, JOB_FINISHED_EVENT, snapshot);
}

fn record_finished(app: &AppHandle, snapshot: &JobSnapshot, job: &Job) {
    let state: State<'_, AppState> = app.state();
    let entry = match state.jobs.take_report(snapshot.id) {
        Some(report) => history_entry_for_report(job, &report),
        None => {
            tracing::error!(
                job_id = snapshot.id,
                "không lấy được báo cáo của việc đã kết thúc"
            );
            let first_input: &Path = job.inputs.first().map_or(Path::new(""), PathBuf::as_path);
            HistoryEntry::new(
                first_input,
                snapshot.label.clone(),
                "Không lấy được kết quả việc chạy",
                EntryStatus::Failed,
                unix_now(),
            )
        }
    };
    push_history(&state, entry);
}

/// Dựng dòng lịch sử cho một việc đã kết thúc. Không phụ thuộc Tauri nên kiểm thử được trực tiếp.
///
/// Trạng thái suy ra từ báo cáo: có mục bị huỷ thì `Cancelled` (giữ số mục đã xong), có lỗi thì `Failed`, còn lại `Done`.
/// WHY: lõi đánh dấu mục chưa làm bằng `Cancelled` đúng khi người dùng huỷ giữa chừng, nên không cần cờ riêng.
/// Dung lượng đầu ra đọc từ đĩa; lỗi đọc chỉ được log và tính 0 để không làm mất sự kiện hoàn tất.
pub fn history_entry_for_report(job: &Job, report: &JobReport) -> HistoryEntry {
    let cancelled_count: usize = report
        .items
        .iter()
        .filter(|item| matches!(item.result, Err(MorphixError::Cancelled)))
        .count();
    let succeeded_count: usize = report.succeeded();
    let failed_count: usize = report.failed() - cancelled_count;
    let status: EntryStatus = if cancelled_count > 0 {
        EntryStatus::Cancelled
    } else if failed_count > 0 {
        EntryStatus::Failed
    } else {
        EntryStatus::Done
    };

    let outputs: Vec<&Path> = report
        .items
        .iter()
        .filter_map(|item| item.result.as_ref().ok().map(PathBuf::as_path))
        .collect();
    let first_input: &Path = job.inputs.first().map_or(Path::new(""), PathBuf::as_path);
    // WHY: việc không có đầu ra nào (mọi mục lỗi) vẫn cần một đường dẫn để mở lại từ lịch sử, nên dùng đầu vào.
    let anchor: &Path = outputs.first().copied().unwrap_or(first_input);
    let anchor_name: String = anchor.file_name().map_or_else(
        || anchor.to_string_lossy().into_owned(),
        |n| n.to_string_lossy().into_owned(),
    );

    let input_bytes: u64 = job.inputs.iter().map(|path| file_size_or_log(path)).sum();
    let output_bytes: u64 = outputs.iter().map(|path| file_size_or_log(path)).sum();

    let mut counts: String = format!("{succeeded_count} thành công, {failed_count} lỗi");
    if cancelled_count > 0 {
        counts.push_str(&format!(", {cancelled_count} đã huỷ"));
    }
    let mut parts: Vec<String> = vec![
        status.label().to_string(),
        operation_name(&job.operation),
        counts,
        format!(
            "{} → {}",
            format_size_vn(input_bytes),
            format_size_vn(output_bytes)
        ),
    ];
    // WHY: lý do lỗi đầu tiên đi kèm để người dùng biết vì sao file không chuyển được; mục bị huỷ không phải lỗi.
    let first_error: Option<String> = report.items.iter().find_map(|item| {
        let input: &Path = item.inputs.first().map_or(Path::new(""), PathBuf::as_path);
        item.result
            .as_ref()
            .err()
            .filter(|error| !matches!(error, MorphixError::Cancelled))
            .map(|error| describe_error(error, input))
    });
    if let Some(reason) = first_error {
        parts.push(reason);
    }

    HistoryEntry::new(
        anchor,
        // WHY "file": lịch sử có cả video, PDF, văn bản, file nén; "ảnh" sai với mọi việc không phải ảnh.
        format!("{} file → {anchor_name}", job.inputs.len()),
        parts.join(" · "),
        status,
        unix_now(),
    )
}

/// Tên thao tác hiển thị trong lịch sử, khớp với nhãn ở danh mục hành động.
fn operation_name(operation: &Operation) -> String {
    match operation {
        Operation::ConvertImage { to } => format!("Đổi sang {}", to.extension().to_uppercase()),
        Operation::MergeImagesToPdf => "Gộp PDF".to_string(),
        Operation::CompressImage => "Nén ảnh".to_string(),
        Operation::StripMetadata => "Xoá metadata".to_string(),
        Operation::ResizeImage(_) => "Đổi kích thước".to_string(),
        Operation::ConvertText { to } => format!("Đổi sang {}", to.extension().to_uppercase()),
        Operation::ConvertMedia { to } => format!("Đổi sang {}", to.label()),
        Operation::CompressMedia => "Nén".to_string(),
        Operation::CutMedia { .. } => "Cắt đoạn".to_string(),
        Operation::ChangeSpeed { .. } => "Đổi tốc độ".to_string(),
        Operation::ExtractAudio { to } => format!("Tách âm thanh sang {}", to.label()),
        Operation::PdfToText => "Đổi sang TXT".to_string(),
        Operation::CreateArchive { format } => {
            format!("Nén thành {}", format.extension().to_uppercase())
        }
        Operation::ExtractArchive => "Giải nén".to_string(),
    }
}

/// Dung lượng một file, lỗi đọc chỉ log và tính 0 để một file thiếu không làm mất cả dòng lịch sử.
fn file_size_or_log(path: &Path) -> u64 {
    match std::fs::metadata(path) {
        Ok(metadata) => metadata.len(),
        Err(error) => {
            tracing::warn!(path = %path.display(), error = %error, "không đọc được dung lượng {}: {error}", path.display());
            0
        }
    }
}

/// Gửi sự kiện tới mọi cửa sổ. Lỗi gửi chỉ được log vì việc nền vẫn cần chạy tiếp.
fn emit_or_log<S: Serialize + Clone>(app: &AppHandle, event: &str, payload: S) {
    if let Err(error) = app.emit(event, payload) {
        tracing::warn!(event = event, error = %error, "không gửi được sự kiện {event}: {error}");
    }
}

#[tauri::command]
pub fn khay_selection(state: State<'_, AppState>) -> crate::state::KhaySelection {
    state.khay_selection_guard().clone()
}

#[tauri::command]
pub fn clear_khay_selection(state: State<'_, AppState>, app: AppHandle) -> Result<(), String> {
    let selection = {
        let mut selection = state.khay_selection_guard();
        *selection = crate::state::KhaySelection::cleared(&selection);
        selection.clone()
    };
    app.emit(windows::KHAY_SELECTION_EVENT, selection)
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod output_directory_tests {
    use super::output_directory_for_history;
    use crate::history::{EntryStatus, History, HistoryEntry};
    use std::path::Path;

    #[test]
    fn opens_only_existing_successful_output_paths_from_history() {
        let directory = tempfile::tempdir().unwrap();
        let output = directory.path().join("Ảnh; $(x).png");
        std::fs::write(&output, b"output").unwrap();
        let mut history = History::open(&directory.path().join("history.json"));
        history
            .push(HistoryEntry::new(
                &output,
                "convert",
                "ok",
                EntryStatus::Done,
                1,
            ))
            .unwrap();

        let path = output.to_string_lossy().into_owned();
        assert_eq!(
            output_directory_for_history(&history, &path).unwrap(),
            directory.path()
        );
        #[cfg(target_os = "linux")]
        assert_eq!(
            super::gio_open_args(Path::new(&path)),
            [std::ffi::OsString::from("open"), path.into()]
        );
    }

    #[test]
    fn rejects_unlisted_failed_missing_and_remote_outputs() {
        let directory = tempfile::tempdir().unwrap();
        let failed_output = directory.path().join("failed.png");
        std::fs::write(&failed_output, b"failed").unwrap();
        let mut history = History::open(&directory.path().join("history.json"));
        history
            .push(HistoryEntry::new(
                &failed_output,
                "convert",
                "failed",
                EntryStatus::Failed,
                1,
            ))
            .unwrap();
        assert!(output_directory_for_history(&history, &failed_output.to_string_lossy()).is_err());
        assert!(output_directory_for_history(&history, "file:///tmp/output.png").is_err());

        let missing = directory.path().join("missing.png");
        history
            .push(HistoryEntry::new(
                &missing,
                "convert",
                "ok",
                EntryStatus::Done,
                2,
            ))
            .unwrap();
        assert!(output_directory_for_history(&history, &missing.to_string_lossy()).is_err());
        assert!(output_directory_for_history(&history, "/tmp/unlisted.png").is_err());
    }
}
