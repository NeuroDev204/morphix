//! Quản lý tích hợp theo người dùng và kiểm tra read-only các mục do package hệ thống cài đặt.
//! Các lệnh cài/gỡ theo người dùng không sửa tài nguyên desktop thuộc package.

pub mod desktop;
pub mod fake;
pub mod files;
pub mod linux;
pub mod report;
#[cfg(target_os = "linux")]
mod selection;
pub mod thunar;
pub mod tools;
pub mod windows;

use std::path::PathBuf;

use tracing::error;

pub use report::{
    doctor_exit_code, exit_code_for, summarize_inspection, summarize_install, summarize_uninstall,
    ComponentId, ComponentStatus, IntegrationComponentReport, IntegrationReport, OverallStatus,
};

/// Autostart bật mặc định; `--no-autostart` chỉ có ở lệnh cài.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IntegrationOptions {
    pub dry_run: bool,
    pub install_autostart: bool,
}

impl Default for IntegrationOptions {
    fn default() -> Self {
        IntegrationOptions {
            dry_run: false,
            install_autostart: true,
        }
    }
}

/// Đường dẫn tuyệt đối của hai executable mà integration trỏ tới. Tách khỏi `current_exe()` để test
/// inject được đường dẫn có khoảng trắng và kiểm tra stale khi binary bị di chuyển.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BinaryPaths {
    pub morphix: PathBuf,
    pub gui: PathBuf,
}

impl BinaryPaths {
    pub fn resolve() -> Result<Self, String> {
        let morphix = std::env::current_exe()
            .map_err(|error| format!("không xác định được vị trí morphix: {error}"))?;
        Ok(BinaryPaths {
            morphix,
            gui: gui_binary_path()?,
        })
    }
}

/// `morphix-gui` nằm cạnh `morphix`; `MORPHIX_GUI_BIN` ghi đè để test trỏ tới đường dẫn giả.
pub fn gui_binary_path() -> Result<PathBuf, String> {
    if let Some(path) = std::env::var_os("MORPHIX_GUI_BIN") {
        return Ok(PathBuf::from(path));
    }
    let current = std::env::current_exe()
        .map_err(|error| format!("không xác định được vị trí morphix: {error}"))?;
    Ok(current.with_file_name(format!("morphix-gui{}", std::env::consts::EXE_SUFFIX)))
}

/// Mỗi nền tảng cung cấp một backend. Hàm trả report, không trả `Result`: lỗi từng thành phần nằm trong
/// report để cài một phần vẫn báo đúng từng mục.
pub trait IntegrationBackend {
    fn platform(&self) -> &'static str;
    fn install(&self, id: ComponentId, binaries: &BinaryPaths) -> IntegrationComponentReport;
    fn uninstall(&self, id: ComponentId) -> IntegrationComponentReport;
    fn inspect(&self, id: ComponentId, binaries: &BinaryPaths) -> IntegrationComponentReport;
}

/// Backend theo nền tảng. Linux đọc XDG và trạng thái package; Windows ghi HKCU của người dùng hiện tại.
#[cfg(target_os = "linux")]
pub fn default_backend() -> Box<dyn IntegrationBackend> {
    Box::new(linux::LinuxBackend::from_env())
}

#[cfg(target_os = "windows")]
pub fn default_backend() -> Box<dyn IntegrationBackend> {
    Box::new(windows::WindowsBackend::from_current_user())
}

#[cfg(not(any(target_os = "linux", target_os = "windows")))]
pub fn default_backend() -> Box<dyn IntegrationBackend> {
    Box::new(UnsupportedBackend)
}

/// Không làm gì và báo `unsupported` cho mọi thành phần; để CLI chạy đúng trên nền tảng chưa hỗ trợ.
pub struct UnsupportedBackend;

const ALL_COMPONENTS: [ComponentId; 14] = [
    ComponentId::WindowsContextMenu,
    ComponentId::NautilusScript,
    ComponentId::NautilusAccel,
    ComponentId::GnomeShortcut,
    ComponentId::DolphinServiceMenu,
    ComponentId::Autostart,
    ComponentId::TraySupport,
    ComponentId::DesktopValidation,
    ComponentId::NautilusExtension,
    ComponentId::DesktopContextMenu,
    ComponentId::ApplicationLauncher,
    ComponentId::NemoAction,
    ComponentId::CajaScript,
    ComponentId::ThunarAction,
];

const INSTALLABLE_COMPONENTS: [ComponentId; 11] = [
    ComponentId::WindowsContextMenu,
    ComponentId::NautilusScript,
    ComponentId::NautilusAccel,
    ComponentId::GnomeShortcut,
    ComponentId::DolphinServiceMenu,
    ComponentId::Autostart,
    ComponentId::NautilusExtension,
    ComponentId::ApplicationLauncher,
    ComponentId::NemoAction,
    ComponentId::CajaScript,
    ComponentId::ThunarAction,
];

impl IntegrationBackend for UnsupportedBackend {
    fn platform(&self) -> &'static str {
        std::env::consts::OS
    }

    fn install(&self, id: ComponentId, _binaries: &BinaryPaths) -> IntegrationComponentReport {
        unsupported_report(id)
    }

    fn uninstall(&self, id: ComponentId) -> IntegrationComponentReport {
        unsupported_report(id)
    }

    fn inspect(&self, id: ComponentId, _binaries: &BinaryPaths) -> IntegrationComponentReport {
        unsupported_report(id)
    }
}

fn unsupported_report(id: ComponentId) -> IntegrationComponentReport {
    IntegrationComponentReport {
        id,
        status: ComponentStatus::Unsupported,
        changed: false,
        path: None,
        message: "tích hợp này chưa có trên nền tảng hiện tại".to_string(),
    }
}

/// Cài các thành phần. `--dry-run` chỉ kiểm trạng thái hiện tại, không ghi gì.
/// `--no-autostart` bỏ hẳn thành phần autostart khỏi report, không đánh dấu là bỏ qua.
pub fn install(
    backend: &dyn IntegrationBackend,
    binaries: &BinaryPaths,
    options: &IntegrationOptions,
) -> IntegrationReport {
    let components: Vec<IntegrationComponentReport> = INSTALLABLE_COMPONENTS
        .iter()
        .filter(|id| options.install_autostart || **id != ComponentId::Autostart)
        .map(|&id| {
            if options.dry_run {
                backend.inspect(id, binaries)
            } else if let Some(message) = missing_binary_message(binaries, id) {
                failed_report(id, binaries, message)
            } else {
                backend.install(id, binaries)
            }
        })
        .map(log_failure)
        .collect();
    let overall = overall_for_changes(options, &components, summarize_install);
    IntegrationReport {
        platform: backend.platform().to_string(),
        overall,
        components,
    }
}

/// Gỡ mọi thành phần Morphix có thể tạo; `--no-autostart` không áp dụng ở đây.
pub fn uninstall(
    backend: &dyn IntegrationBackend,
    binaries: &BinaryPaths,
    options: &IntegrationOptions,
) -> IntegrationReport {
    let components: Vec<IntegrationComponentReport> = INSTALLABLE_COMPONENTS
        .iter()
        .map(|&id| {
            if options.dry_run {
                backend.inspect(id, binaries)
            } else {
                backend.uninstall(id)
            }
        })
        .map(log_failure)
        .collect();
    let overall = overall_for_changes(options, &components, summarize_uninstall);
    IntegrationReport {
        platform: backend.platform().to_string(),
        overall,
        components,
    }
}

/// Chỉ đọc trạng thái mọi thành phần, kể cả tray; dùng cho `morphix doctor`.
pub fn inspect(backend: &dyn IntegrationBackend, binaries: &BinaryPaths) -> IntegrationReport {
    let components: Vec<IntegrationComponentReport> = ALL_COMPONENTS
        .iter()
        .map(|&id| backend.inspect(id, binaries))
        .map(log_failure)
        .collect();
    IntegrationReport {
        platform: backend.platform().to_string(),
        overall: summarize_inspection(&components),
        components,
    }
}

/// Dry-run không ghi gì nhưng vẫn phải báo lỗi "không thể tiếp tục" nếu không có gì áp dụng được.
fn overall_for_changes(
    options: &IntegrationOptions,
    components: &[IntegrationComponentReport],
    summarize: fn(&[IntegrationComponentReport]) -> OverallStatus,
) -> OverallStatus {
    let summary = summarize(components);
    if options.dry_run && summary != OverallStatus::Failure {
        OverallStatus::DryRun
    } else {
        summary
    }
}

/// WHY kiểm trước khi cài: menu hoặc autostart trỏ tới file không tồn tại thì người dùng sẽ thấy lỗi lúc
/// bấm, nên báo lỗi ngay ở lúc cài.
fn missing_binary_message(binaries: &BinaryPaths, id: ComponentId) -> Option<String> {
    if !binaries.morphix.is_file() {
        return Some(format!(
            "không tìm thấy morphix tại {}",
            binaries.morphix.display()
        ));
    }
    if id == ComponentId::Autostart && !binaries.gui.is_file() {
        return Some(format!(
            "không tìm thấy morphix-gui tại {}",
            binaries.gui.display()
        ));
    }
    None
}

fn failed_report(
    id: ComponentId,
    binaries: &BinaryPaths,
    message: String,
) -> IntegrationComponentReport {
    IntegrationComponentReport {
        id,
        status: ComponentStatus::Failed,
        changed: false,
        path: Some(binaries.morphix.clone()),
        message,
    }
}

/// Ghi log mọi thành phần lỗi; báo lỗi không được chỉ nằm trong JSON mà mất khỏi nhật ký.
fn log_failure(report: IntegrationComponentReport) -> IntegrationComponentReport {
    if report.status == ComponentStatus::Failed {
        error!(component = report.id.as_str(), message = %report.message, "tích hợp thất bại");
    }
    report
}
