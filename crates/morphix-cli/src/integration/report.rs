use std::path::PathBuf;

use serde::{Deserialize, Serialize, Serializer};

/// Tên thành phần là hợp đồng JSON ổn định; đổi tên là phá các script đọc `--json`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComponentId {
    WindowsContextMenu,
    NautilusScript,
    NautilusAccel,
    GnomeShortcut,
    DolphinServiceMenu,
    Autostart,
    TraySupport,
    /// Chỉ doctor kiểm tra: file `.desktop` đã cài có qua `desktop-file-validate` không.
    DesktopValidation,
    NautilusExtension,
    DesktopContextMenu,
    ApplicationLauncher,
    NemoAction,
    CajaScript,
    ThunarAction,
}

impl ComponentId {
    pub fn as_str(self) -> &'static str {
        match self {
            ComponentId::WindowsContextMenu => "windows_context_menu",
            ComponentId::NautilusScript => "nautilus_script",
            ComponentId::NautilusAccel => "nautilus_accel",
            ComponentId::GnomeShortcut => "gnome_shortcut",
            ComponentId::DolphinServiceMenu => "dolphin_service_menu",
            ComponentId::Autostart => "autostart",
            ComponentId::TraySupport => "tray_support",
            ComponentId::DesktopValidation => "desktop_validation",
            ComponentId::NautilusExtension => "nautilus_extension",
            ComponentId::DesktopContextMenu => "desktop_context_menu",
            ComponentId::ApplicationLauncher => "application_launcher",
            ComponentId::NemoAction => "nemo_action",
            ComponentId::CajaScript => "caja_script",
            ComponentId::ThunarAction => "thunar_action",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComponentStatus {
    Installed,
    AlreadyInstalled,
    Removed,
    AlreadyAbsent,
    Missing,
    Stale,
    Conflict,
    Unsupported,
    Unknown,
    NotApplicable,
    Failed,
}

impl ComponentStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            ComponentStatus::Installed => "installed",
            ComponentStatus::AlreadyInstalled => "already_installed",
            ComponentStatus::Removed => "removed",
            ComponentStatus::AlreadyAbsent => "already_absent",
            ComponentStatus::Missing => "missing",
            ComponentStatus::Stale => "stale",
            ComponentStatus::Conflict => "conflict",
            ComponentStatus::Unsupported => "unsupported",
            ComponentStatus::Unknown => "unknown",
            ComponentStatus::NotApplicable => "not_applicable",
            ComponentStatus::Failed => "failed",
        }
    }

    /// `unsupported` và `not_applicable` không phải lỗi: thành phần đó không có trên nền tảng này.
    pub fn is_applicable(self) -> bool {
        !matches!(
            self,
            ComponentStatus::Unsupported | ComponentStatus::NotApplicable
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OverallStatus {
    Success,
    Partial,
    Failure,
    DryRun,
    /// Không thành phần nào áp dụng được để kiểm tra; không được báo là khoẻ mạnh.
    Inconclusive,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IntegrationComponentReport {
    pub id: ComponentId,
    pub status: ComponentStatus,
    pub changed: bool,
    #[serde(serialize_with = "serialize_path_lossy")]
    pub path: Option<PathBuf>,
    pub message: String,
}

/// WHY lossy: JSON chỉ có chuỗi UTF-8, mà đường dẫn không bắt buộc là UTF-8; báo cáo vẫn phải ra JSON hợp lệ.
fn serialize_path_lossy<S: Serializer>(
    path: &Option<PathBuf>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    path.as_ref()
        .map(|p| p.to_string_lossy().into_owned())
        .serialize(serializer)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IntegrationReport {
    pub platform: String,
    pub overall: OverallStatus,
    pub components: Vec<IntegrationComponentReport>,
}

/// Tổng hợp cài/gỡ: không có mục áp dụng thì không thể tiếp tục; có mục lỗi lẫn mục thành công thì một phần.
/// `is_failure` khác nhau giữa cài và gỡ: `conflict` khi cài là lỗi, khi gỡ là mục không thuộc Morphix đã
/// nằm ngoài phạm vi gỡ nên không tính là lỗi.
fn summarize_changes(
    components: &[IntegrationComponentReport],
    is_failure: fn(ComponentStatus) -> bool,
) -> OverallStatus {
    let applicable: Vec<_> = components
        .iter()
        .filter(|c| c.status.is_applicable())
        .collect();
    if applicable.is_empty() {
        return OverallStatus::Failure;
    }
    let failures = applicable.iter().filter(|c| is_failure(c.status)).count();
    let successes = applicable.len() - failures;
    match (failures, successes) {
        (0, _) => OverallStatus::Success,
        (_, 0) => OverallStatus::Failure,
        _ => OverallStatus::Partial,
    }
}

pub fn summarize_install(components: &[IntegrationComponentReport]) -> OverallStatus {
    summarize_changes(components, |status| {
        matches!(status, ComponentStatus::Failed | ComponentStatus::Conflict)
    })
}

pub fn summarize_uninstall(components: &[IntegrationComponentReport]) -> OverallStatus {
    summarize_changes(components, |status| status == ComponentStatus::Failed)
}

/// Kiểm tra chỉ đọc: `overall` chỉ báo có lỗi hay không; từng mục vẫn mô tả trạng thái chi tiết.
pub fn summarize_inspection(components: &[IntegrationComponentReport]) -> OverallStatus {
    let applicable: Vec<_> = components
        .iter()
        .filter(|c| c.status.is_applicable())
        .collect();
    if applicable.is_empty() {
        return OverallStatus::Inconclusive;
    }
    let bad = applicable
        .iter()
        .filter(|c| is_error_status(c.status))
        .count();
    match (bad, applicable.len()) {
        (0, _) => OverallStatus::Success,
        (bad, total) if bad == total => OverallStatus::Failure,
        _ => OverallStatus::Partial,
    }
}

/// `stale`/`conflict`/`failed` là lỗi cần người dùng xử lý; `missing` chỉ là chưa cài, không phải lỗi.
fn is_error_status(status: ComponentStatus) -> bool {
    matches!(
        status,
        ComponentStatus::Failed | ComponentStatus::Conflict | ComponentStatus::Stale
    )
}

/// Mã thoát install/uninstall theo spec: 0 thành công hoặc dry-run, 1 không thể tiếp tục, 3 một phần.
pub fn exit_code_for(overall: OverallStatus) -> u8 {
    match overall {
        OverallStatus::Success | OverallStatus::DryRun => 0,
        OverallStatus::Partial | OverallStatus::Inconclusive => 3,
        OverallStatus::Failure => 1,
    }
}

/// Mã thoát phần integration của doctor: lỗi thắng mục không kết luận được, nên `1` trước `3`.
/// WHY không áp dụng mục nào thì `3`: exit 0 sẽ bị script đọc là khoẻ trong khi chưa kiểm được gì.
pub fn doctor_exit_code(components: &[IntegrationComponentReport]) -> u8 {
    if !components.iter().any(|c| c.status.is_applicable()) {
        return 3;
    }
    let mut inconclusive = false;
    for component in components.iter().filter(|c| c.status.is_applicable()) {
        if is_error_status(component.status) {
            return 1;
        }
        if component.status == ComponentStatus::Unknown {
            inconclusive = true;
        }
    }
    if inconclusive {
        3
    } else {
        0
    }
}
