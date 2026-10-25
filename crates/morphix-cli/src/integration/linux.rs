//! Tích hợp Linux theo từng người dùng: script Nautilus (submenu Scripts), service menu Dolphin và autostart XDG.
//! Mọi đường dẫn đi từ XDG roots được inject, nên test không chạm `~/.local/share` hay `~/.config` thật.

use std::{
    env,
    ffi::OsStr,
    fs, io,
    path::{Path, PathBuf},
};

use super::{
    files::{
        classify, publish_file, read_existing, remove_file_if_owned, ExistingFile, FileOwnership,
        Transaction, MANAGED_MARKER,
    },
    tools::{system_tool, ToolOutcome, ToolRunner},
    BinaryPaths, ComponentId, ComponentStatus, IntegrationBackend, IntegrationComponentReport,
};

const NAUTILUS_SCRIPT_NAME: &str = "Morphix";
const DOLPHIN_MENU_NAME: &str = "morphix.desktop";
const AUTOSTART_NAME: &str = "morphix.desktop";
const NAUTILUS_EXTENSION_PATH: &str = "/usr/share/nautilus-python/extensions/morphix_menu.py";
const APPLICATION_LAUNCHER_PATH: &str = "/usr/share/applications/morphix.desktop";
const NAUTILUS_EXTENSION_TEMPLATE: &str =
    include_str!("../../../../integration/nautilus/morphix_menu.py");
/// Dòng duy nhất trong template được thay bằng đường dẫn morphix của bản cài per-user.
const NAUTILUS_EXECUTABLE_LINE: &str = "EXECUTABLE: str = \"/usr/bin/morphix\"";

/// Bản `.deb` cài binary dưới `/usr` và tự quản extension, launcher, menu DING; bản curl cài trong home
/// nên các mục đó là file per-user do lệnh cài tích hợp ghi.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallMode {
    Package,
    User,
}

pub fn install_mode(morphix: &Path) -> InstallMode {
    if morphix.starts_with("/usr/") {
        InstallMode::Package
    } else {
        InstallMode::User
    }
}

/// UUID của extension AppIndicator. Ubuntu đóng gói bản riêng nên cả hai UUID đều được coi là AppIndicator.
const APPINDICATOR_UUIDS: [&str; 2] = [
    "appindicatorsupport@rgcjonas.gmail.com",
    "ubuntu-appindicators@ubuntu.com",
];

/// Phiên đồ hoạ cần để biết AppIndicator có áp dụng và kiểm được không. Đọc từ môi trường một lần khi khởi tạo backend.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SessionEnv {
    pub current_desktop: Option<String>,
    pub has_session_bus: bool,
    /// `XDG_SESSION_TYPE`: Alt+C cần X11 để giả lập phím và đọc lựa chọn.
    pub session_type: Option<String>,
    /// `PATH` lúc khởi tạo; dùng để biết trình quản lý file nào có trên máy.
    pub path: Option<std::ffi::OsString>,
}

impl SessionEnv {
    pub fn from_env() -> Self {
        SessionEnv {
            current_desktop: std::env::var_os("XDG_CURRENT_DESKTOP")
                .map(|value| value.to_string_lossy().into_owned()),
            has_session_bus: std::env::var_os("DBUS_SESSION_BUS_ADDRESS")
                .is_some_and(|value| !value.is_empty()),
            session_type: std::env::var_os("XDG_SESSION_TYPE")
                .map(|value| value.to_string_lossy().into_owned()),
            // WHY biến riêng: test chỉ định trình quản lý file "có trên máy" mà không phải đổi PATH thật
            // (PATH thật còn cần cho gsettings, dpkg-query...). Cùng kiểu với `MORPHIX_GUI_BIN`.
            path: std::env::var_os("MORPHIX_FILE_MANAGER_PATH")
                .or_else(|| std::env::var_os("PATH")),
        }
    }

    /// WHY `None` là X11: phiên không đặt `XDG_SESSION_TYPE` giữ hành vi cũ thay vì mất Alt+C.
    pub(super) fn is_x11(&self) -> bool {
        self.session_type
            .as_deref()
            .is_none_or(|kind| kind.eq_ignore_ascii_case("x11"))
    }

    /// Có file `name` trong một thư mục của `PATH`.
    pub fn has_program(&self, name: &str) -> bool {
        self.path
            .as_ref()
            .is_some_and(|path| std::env::split_paths(path).any(|dir| dir.join(name).is_file()))
    }

    /// `XDG_CURRENT_DESKTOP` là danh sách ngăn bằng `:`; Ubuntu đặt `ubuntu:GNOME`.
    pub(super) fn is_gnome(&self) -> bool {
        self.current_desktop.as_deref().is_some_and(|desktop| {
            desktop
                .split(':')
                .any(|part| part.eq_ignore_ascii_case("GNOME"))
        })
    }
}

/// Marker đặt trong thư mục do lần cài tạo ra. WHY: chỉ thư mục có marker và không còn gì khác mới được gỡ,
/// nên thư mục có sẵn (kể cả rỗng) của người dùng không bao giờ bị xoá.
const CREATED_DIR_MARKER: &str = ".morphix-created";

/// Nautilus chỉ liệt kê script có bit thực thi.
const EXECUTABLE_MODE: u32 = 0o755;
const DESKTOP_ENTRY_MODE: u32 = 0o644;
/// WHY 0755: KDE từng yêu cầu bit thực thi cho service menu. Chưa đo trên Dolphin thật (Chưa kiểm).
const DOLPHIN_MENU_MODE: u32 = 0o755;

/// Thư mục dữ liệu và cấu hình per-user theo XDG Base Directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XdgRoots {
    pub data_home: PathBuf,
    pub config_home: PathBuf,
}

/// Biến XDG rỗng hoặc tương đối bị bỏ qua (đúng spec XDG) và rơi về `$HOME`. WHY không dùng HOME trực tiếp:
/// XDG có thể đặt khác HOME, còn HOME có thể không tồn tại trong môi trường service.
pub fn resolve_xdg_roots(
    xdg_data_home: Option<&OsStr>,
    xdg_config_home: Option<&OsStr>,
    home: Option<&OsStr>,
) -> Result<XdgRoots, String> {
    Ok(XdgRoots {
        data_home: resolve_base_dir(xdg_data_home, home, ".local/share", "XDG_DATA_HOME")?,
        config_home: resolve_base_dir(xdg_config_home, home, ".config", "XDG_CONFIG_HOME")?,
    })
}

fn resolve_base_dir(
    xdg_value: Option<&OsStr>,
    home: Option<&OsStr>,
    home_relative: &str,
    variable: &str,
) -> Result<PathBuf, String> {
    if let Some(absolute) = xdg_value.map(Path::new).filter(|path| path.is_absolute()) {
        return Ok(absolute.to_path_buf());
    }
    let home = home
        .map(Path::new)
        .filter(|path| path.is_absolute())
        .ok_or_else(|| {
            format!("không xác định được thư mục người dùng: đặt {variable} hoặc HOME")
        })?;
    Ok(home.join(home_relative))
}

/// `roots` là `Err` khi không xác định được thư mục người dùng; mỗi thành phần Linux báo lỗi đó thay vì panic.
/// `tools` được inject để test kiểm kết luận của doctor mà không phụ thuộc GNOME hay validator trên máy chạy test.
pub struct LinuxBackend {
    roots: Result<XdgRoots, String>,
    pub(super) session: SessionEnv,
    pub(super) tools: Box<ToolRunner>,
}

impl LinuxBackend {
    pub fn new(roots: XdgRoots, session: SessionEnv, tools: Box<ToolRunner>) -> Self {
        Self::from_resolution(Ok(roots), session, tools)
    }

    pub fn from_env() -> Self {
        Self::from_resolution(
            resolve_xdg_roots(
                env::var_os("XDG_DATA_HOME").as_deref(),
                env::var_os("XDG_CONFIG_HOME").as_deref(),
                env::var_os("HOME").as_deref(),
            ),
            SessionEnv::from_env(),
            Box::new(system_tool),
        )
    }

    pub fn from_resolution(
        roots: Result<XdgRoots, String>,
        session: SessionEnv,
        tools: Box<ToolRunner>,
    ) -> Self {
        LinuxBackend {
            roots,
            session,
            tools,
        }
    }
}

/// File Morphix cần công bố: vị trí, nội dung đích và quyền (đặt trước khi rename để không lộ file thiếu quyền).
struct DesiredFile {
    path: PathBuf,
    bytes: Vec<u8>,
    mode: u32,
}

/// Vị trí file của thành phần Linux; `None` cho thành phần không thuộc Linux.
fn location(roots: &XdgRoots, id: ComponentId) -> Option<PathBuf> {
    match id {
        ComponentId::NautilusScript => Some(
            roots
                .data_home
                .join("nautilus")
                .join("scripts")
                .join(NAUTILUS_SCRIPT_NAME),
        ),
        ComponentId::DolphinServiceMenu => Some(
            roots
                .data_home
                .join("kio")
                .join("servicemenus")
                .join(DOLPHIN_MENU_NAME),
        ),
        ComponentId::Autostart => Some(roots.config_home.join("autostart").join(AUTOSTART_NAME)),
        ComponentId::NautilusExtension => Some(
            roots
                .data_home
                .join("nautilus-python")
                .join("extensions")
                .join("morphix_menu.py"),
        ),
        ComponentId::ApplicationLauncher => {
            Some(roots.data_home.join("applications").join("morphix.desktop"))
        }
        ComponentId::NemoAction => Some(
            roots
                .data_home
                .join("nemo")
                .join("actions")
                .join("morphix.nemo_action"),
        ),
        ComponentId::CajaScript => Some(
            roots
                .config_home
                .join("caja")
                .join("scripts")
                .join(NAUTILUS_SCRIPT_NAME),
        ),
        ComponentId::WindowsContextMenu
        | ComponentId::TraySupport
        | ComponentId::NautilusAccel
        | ComponentId::GnomeShortcut
        | ComponentId::DesktopValidation
        | ComponentId::DesktopContextMenu
        | ComponentId::ThunarAction => None,
    }
}

/// Xoá bytecode mà nautilus-python sinh cho extension (`__pycache__/morphix_menu.*.pyc`) và thư mục cache nếu rỗng.
/// WHY: để lại thì thư mục do Morphix tạo không bao giờ rỗng nên không gỡ được. Chỉ đụng file mang tên extension.
fn remove_extension_bytecode(extension: &Path) -> io::Result<()> {
    let Some(cache) = extension.parent().map(|dir| dir.join("__pycache__")) else {
        return Ok(());
    };
    let entries = match std::fs::read_dir(&cache) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    for entry in entries {
        let entry = entry?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with("morphix_menu.") && name.ends_with(".pyc") {
            std::fs::remove_file(entry.path())?;
        }
    }
    match std::fs::remove_dir(&cache) {
        Ok(()) => Ok(()),
        // WHY: còn bytecode của extension khác thì giữ thư mục.
        Err(error) if error.kind() == io::ErrorKind::DirectoryNotEmpty => Ok(()),
        Err(error) => Err(error),
    }
}

/// `uca.xml` dùng chung với action của người dùng; module `thunar` chỉ sửa khối của Morphix.
fn thunar_path(roots: &XdgRoots) -> PathBuf {
    roots.config_home.join("Thunar").join("uca.xml")
}

fn desired_file(
    roots: &XdgRoots,
    id: ComponentId,
    binaries: &BinaryPaths,
) -> Option<Result<DesiredFile, String>> {
    let path = location(roots, id)?;
    let content = match id {
        ComponentId::NautilusScript => {
            nautilus_script_body(&binaries.morphix).map(|body| (body, EXECUTABLE_MODE))
        }
        ComponentId::DolphinServiceMenu => {
            dolphin_service_menu_body(&binaries.morphix).map(|body| (body, DOLPHIN_MENU_MODE))
        }
        ComponentId::Autostart => {
            autostart_entry_body(&binaries.gui).map(|body| (body, DESKTOP_ENTRY_MODE))
        }
        ComponentId::NautilusExtension => {
            nautilus_extension_body(&binaries.morphix).map(|body| (body, DESKTOP_ENTRY_MODE))
        }
        ComponentId::ApplicationLauncher => {
            application_launcher_body(&binaries.morphix).map(|body| (body, DESKTOP_ENTRY_MODE))
        }
        ComponentId::NemoAction => {
            nemo_action_body(&binaries.morphix).map(|body| (body, DESKTOP_ENTRY_MODE))
        }
        // WHY dùng lại script Nautilus: Caja truyền file đã chọn qua argv giống hệt Nautilus.
        ComponentId::CajaScript => {
            nautilus_script_body(&binaries.morphix).map(|body| (body, EXECUTABLE_MODE))
        }
        ComponentId::WindowsContextMenu
        | ComponentId::TraySupport
        | ComponentId::NautilusAccel
        | ComponentId::GnomeShortcut
        | ComponentId::DesktopValidation
        | ComponentId::DesktopContextMenu
        | ComponentId::ThunarAction => return None,
    };
    Some(content.map(|(body, mode)| DesiredFile {
        path,
        bytes: body.into_bytes(),
        mode,
    }))
}

fn utf8(path: &Path) -> Result<&str, String> {
    path.to_str()
        .ok_or_else(|| format!("đường dẫn không phải UTF-8: {}", path.display()))
}

/// Script Nautilus. WHY `"$@"` và không biến đổi: đường dẫn file đi nguyên argv tới CLI, không qua shell parse.
/// Chỉ đường dẫn của chính morphix được nội suy, và đã POSIX-quote.
pub fn nautilus_script_body(morphix: &Path) -> Result<String, String> {
    let exec_line = format!("exec {} menu -- \"$@\"", posix_single_quote(utf8(morphix)?));
    Ok([
        "#!/bin/sh".to_string(),
        format!("# {MANAGED_MARKER}"),
        "if [ \"$#\" -eq 0 ]; then".to_string(),
        "    printf '%s\\n' 'Morphix: không có file cục bộ nào được chọn (thư mục từ xa không truyền đường dẫn).' >&2".to_string(),
        "    exit 1".to_string(),
        "fi".to_string(),
        exec_line,
    ]
    .join("\n")
        + "\n")
}

/// Service menu Dolphin. `%F` là đường dẫn cục bộ của lựa chọn (không phải URI): Dolphin trên KDE 6 đưa
/// đường dẫn thường cho `%U` ở một số máy, khiến `menu --uris` từ chối và menu im lặng.
pub fn dolphin_service_menu_body(morphix: &Path) -> Result<String, String> {
    let exec = escape_desktop_value(&format!(
        "{} menu -- %F",
        desktop_exec_argument(utf8(morphix)?)
    ));
    Ok([
        format!("# {MANAGED_MARKER}"),
        "[Desktop Entry]".to_string(),
        "Type=Service".to_string(),
        // WHY Name ở nhóm chính: desktop-file-validate báo lỗi nếu thiếu, dù menu hiển thị lấy tên từ nhóm action.
        "Name=Morphix".to_string(),
        // WHY mọi file: Morphix xử lý cả âm thanh, video, văn bản, PDF và file nén, không chỉ ảnh.
        "MimeType=all/allfiles;".to_string(),
        "Actions=Morphix;".to_string(),
        // WHY ServiceTypes (không phải X-KDE-ServiceTypes): khóa theo KDE docs để Dolphin nhận diện service menu chuột phải.
        "ServiceTypes=KonqPopupMenu/Plugin".to_string(),
        String::new(),
        "[Desktop Action Morphix]".to_string(),
        "Name=Morphix".to_string(),
        format!("Exec={exec}"),
    ]
    .join("\n")
        + "\n")
}

/// Extension nautilus-python per-user: cùng mã với bản trong gói `.deb`, chỉ đổi đường dẫn morphix.
/// WHY JSON: chuỗi JSON là literal Python hợp lệ, nên đường dẫn có nháy hay ký tự lạ vẫn đúng.
pub fn nautilus_extension_body(morphix: &Path) -> Result<String, String> {
    if NAUTILUS_EXTENSION_TEMPLATE
        .matches(NAUTILUS_EXECUTABLE_LINE)
        .count()
        != 1
    {
        return Err("template extension Nautilus không có đúng một dòng EXECUTABLE".to_string());
    }
    let literal = serde_json::to_string(utf8(morphix)?).map_err(|error| error.to_string())?;
    Ok(format!(
        "# {MANAGED_MARKER}\n{}",
        NAUTILUS_EXTENSION_TEMPLATE.replace(
            NAUTILUS_EXECUTABLE_LINE,
            &format!("EXECUTABLE: str = {literal}")
        )
    ))
}

/// Mục Morphix trong danh sách ứng dụng cho bản cài trong home; giống `packaging/morphix.desktop`.
pub fn application_launcher_body(morphix: &Path) -> Result<String, String> {
    let morphix = utf8(morphix)?;
    Ok([
        format!("# {MANAGED_MARKER}"),
        "[Desktop Entry]".to_string(),
        "Name=Morphix".to_string(),
        "Comment=Convert files on your computer".to_string(),
        format!(
            "Exec={}",
            escape_desktop_value(&format!("{} tray", desktop_exec_argument(morphix)))
        ),
        format!("TryExec={}", escape_desktop_value(morphix)),
        "Icon=morphix".to_string(),
        "Terminal=false".to_string(),
        "Type=Application".to_string(),
        "Categories=Utility;".to_string(),
        "StartupNotify=true".to_string(),
    ]
    .join("\n")
        + "\n")
}

/// Nemo action (Cinnamon). `%U` truyền URI `file:` của lựa chọn, như service menu Dolphin.
pub fn nemo_action_body(morphix: &Path) -> Result<String, String> {
    let exec = escape_desktop_value(&format!(
        "{} menu --uris %U",
        desktop_exec_argument(utf8(morphix)?)
    ));
    Ok([
        format!("# {MANAGED_MARKER}"),
        "[Nemo Action]".to_string(),
        "Name=Morphix".to_string(),
        "Comment=Mở Morphix cho các file đã chọn".to_string(),
        format!("Exec={exec}"),
        "Icon-Name=morphix".to_string(),
        "Selection=notnone".to_string(),
        "Extensions=nodirs;".to_string(),
    ]
    .join("\n")
        + "\n")
}

/// Autostart cho GUI. `morphix-gui` không nhận đối số khi đăng nhập nên Exec chỉ có đường dẫn.
pub fn autostart_entry_body(gui: &Path) -> Result<String, String> {
    let gui = utf8(gui)?;
    let exec = escape_desktop_value(&desktop_exec_argument(gui));
    let try_exec = escape_desktop_value(gui);
    Ok([
        format!("# {MANAGED_MARKER}"),
        "[Desktop Entry]".to_string(),
        "Type=Application".to_string(),
        "Name=Morphix".to_string(),
        format!("Exec={exec}"),
        format!("TryExec={try_exec}"),
        "Terminal=false".to_string(),
    ]
    .join("\n")
        + "\n")
}

/// Bọc chuỗi trong nháy đơn POSIX; nháy đơn trong chuỗi thành `'\''`, nên shell không diễn giải ký tự nào.
pub fn posix_single_quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', "'\\''"))
}

/// Escape giá trị Desktop Entry: `\` và xuống dòng/tab/CR phải viết thành chuỗi escape, nếu không file bị hỏng.
pub fn escape_desktop_value(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('\n', "\\n")
        .replace('\t', "\\t")
        .replace('\r', "\\r")
}

/// Đối số của khoá Exec: có ký tự đặc biệt thì bọc nháy kép và escape `"`, `` ` ``, `$`, `\`. `%` luôn nhân đôi
/// vì `%` mở field code. WHY không dùng shell: Exec có ngữ pháp riêng của Desktop Entry.
pub fn desktop_exec_argument(arg: &str) -> String {
    const NEEDS_QUOTES: &str = " \t\n\"'\\><~|&;$*?#()`";
    let needs_quotes = arg.is_empty() || arg.contains(|ch: char| NEEDS_QUOTES.contains(ch));
    let mut quoted = String::with_capacity(arg.len() + 2);
    if needs_quotes {
        quoted.push('"');
    }
    for ch in arg.chars() {
        if needs_quotes && matches!(ch, '"' | '`' | '$' | '\\') {
            quoted.push('\\');
        }
        if ch == '%' {
            quoted.push_str("%%");
        } else {
            quoted.push(ch);
        }
    }
    if needs_quotes {
        quoted.push('"');
    }
    quoted
}

pub(super) fn report(
    id: ComponentId,
    status: ComponentStatus,
    changed: bool,
    path: Option<&Path>,
    message: &str,
) -> IntegrationComponentReport {
    IntegrationComponentReport {
        id,
        status,
        changed,
        path: path.map(Path::to_path_buf),
        message: message.to_string(),
    }
}

/// Lỗi I/O được đưa vào message; `log_failure` trong mod ghi lại nên không log lần hai ở đây.
pub(super) fn io_failure(
    id: ComponentId,
    path: &Path,
    error: &io::Error,
) -> IntegrationComponentReport {
    report(
        id,
        ComponentStatus::Failed,
        false,
        Some(path),
        &format!("không ghi/đọc được {}: {error}", path.display()),
    )
}

pub(super) fn failed_report(id: ComponentId, message: &str) -> IntegrationComponentReport {
    report(id, ComponentStatus::Failed, false, None, message)
}

fn outside_linux(id: ComponentId) -> IntegrationComponentReport {
    match id {
        ComponentId::WindowsContextMenu => report(
            id,
            ComponentStatus::NotApplicable,
            false,
            None,
            "menu chuột phải Windows không áp dụng trên Linux",
        ),
        ComponentId::NautilusExtension
        | ComponentId::DesktopContextMenu
        | ComponentId::ApplicationLauncher => report(
            id,
            ComponentStatus::Unsupported,
            false,
            None,
            "gói tích hợp Ubuntu chưa áp dụng trên bản Linux này",
        ),
        _ => report(
            id,
            ComponentStatus::Unsupported,
            false,
            None,
            "tích hợp này chưa có trên Linux",
        ),
    }
}

/// `true` khi file hiện có đúng nội dung và quyền đích. WHY kiểm quyền: chmod -x làm script không chạy được
/// dù nội dung đúng, nên không được báo `already_installed`.
fn is_same(current: &ExistingFile, file: &DesiredFile) -> bool {
    current.bytes == file.bytes && current.mode & 0o777 == file.mode
}

fn install_file(id: ComponentId, file: &DesiredFile) -> IntegrationComponentReport {
    let existing = match read_existing(&file.path) {
        Ok(existing) => existing,
        Err(error) => return io_failure(id, &file.path, &error),
    };
    match classify(existing.as_ref()) {
        FileOwnership::Foreign => {
            return report(
                id,
                ComponentStatus::Conflict,
                false,
                Some(&file.path),
                "đã có file không thuộc Morphix tại đường dẫn này; giữ nguyên",
            )
        }
        FileOwnership::Owned
            if existing
                .as_ref()
                .is_some_and(|current| is_same(current, file)) =>
        {
            return report(
                id,
                ComponentStatus::AlreadyInstalled,
                false,
                Some(&file.path),
                "đã cài sẵn",
            )
        }
        FileOwnership::Owned | FileOwnership::Absent => {}
    }
    let mut tx = Transaction::new();
    match write_with_created_dirs(file, &mut tx) {
        Ok(()) => {
            tx.commit();
            report(
                id,
                ComponentStatus::Installed,
                true,
                Some(&file.path),
                "đã cài",
            )
        }
        // WHY không commit: `tx` đảo ngược các bước của lần cài này khi rời phạm vi.
        Err(error) => io_failure(id, &file.path, &error),
    }
}

fn write_with_created_dirs(file: &DesiredFile, tx: &mut Transaction) -> io::Result<()> {
    let install_dir = file.path.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "đường dẫn không có thư mục cha",
        )
    })?;
    // WHY tính trước khi ghi: sau khi ghi không còn biết thư mục nào do lần cài này tạo ra.
    let created_dirs = missing_dirs(install_dir);
    publish_file(&file.path, &file.bytes, file.mode, tx)?;
    let marker_bytes = format!("{MANAGED_MARKER}\n").into_bytes();
    for dir in created_dirs {
        publish_file(
            &dir.join(CREATED_DIR_MARKER),
            &marker_bytes,
            DESKTOP_ENTRY_MODE,
            tx,
        )?;
    }
    Ok(())
}

/// `dir` và tổ tiên chưa tồn tại, từ dưới lên.
fn missing_dirs(dir: &Path) -> Vec<PathBuf> {
    let mut missing = Vec::new();
    let mut current = Some(dir);
    while let Some(path) = current.filter(|path| !path.exists()) {
        missing.push(path.to_path_buf());
        current = path.parent();
    }
    missing
}

fn uninstall_file(id: ComponentId, path: &Path) -> IntegrationComponentReport {
    let mut tx = Transaction::new();
    let ownership = match remove_file_if_owned(path, &mut tx) {
        Ok(ownership) => ownership,
        Err(error) => return io_failure(id, path, &error),
    };
    if ownership == FileOwnership::Foreign {
        return report(
            id,
            ComponentStatus::Conflict,
            false,
            Some(path),
            "file không thuộc Morphix; giữ nguyên",
        );
    }
    let cleanup = match path.parent() {
        Some(dir) => remove_created_dirs(dir),
        None => Ok(()),
    };
    // WHY lỗi dọn thư mục làm rollback file: `tx` còn chưa commit nên file được khôi phục.
    if let Err(error) = cleanup {
        return io_failure(id, path, &error);
    }
    tx.commit();
    match ownership {
        FileOwnership::Owned => report(id, ComponentStatus::Removed, true, Some(path), "đã gỡ"),
        _ => report(
            id,
            ComponentStatus::AlreadyAbsent,
            false,
            Some(path),
            "chưa cài",
        ),
    }
}

/// Gỡ thư mục do Morphix tạo, từ `start` lên trên, dừng ở thư mục đầu tiên không còn đúng marker và rỗng.
/// WHY dừng: thư mục có file khác hoặc không có marker là của người dùng hoặc của lần cài khác.
fn remove_created_dirs(start: &Path) -> io::Result<()> {
    let mut current = Some(start);
    while let Some(dir) = current {
        if !holds_only_created_marker(dir)? {
            break;
        }
        fs::remove_file(dir.join(CREATED_DIR_MARKER))?;
        fs::remove_dir(dir)?;
        current = dir.parent();
    }
    Ok(())
}

fn holds_only_created_marker(dir: &Path) -> io::Result<bool> {
    let marker = dir.join(CREATED_DIR_MARKER);
    let is_marker =
        read_existing(&marker)?.is_some_and(|file| classify(Some(&file)) == FileOwnership::Owned);
    if !is_marker {
        return Ok(false);
    }
    Ok(fs::read_dir(dir)?.count() == 1)
}

fn inspect_file(id: ComponentId, file: &DesiredFile) -> IntegrationComponentReport {
    let existing = match read_existing(&file.path) {
        Ok(existing) => existing,
        Err(error) => return io_failure(id, &file.path, &error),
    };
    match classify(existing.as_ref()) {
        FileOwnership::Absent => report(
            id,
            ComponentStatus::Missing,
            false,
            Some(&file.path),
            "chưa cài",
        ),
        FileOwnership::Foreign => report(
            id,
            ComponentStatus::Conflict,
            false,
            Some(&file.path),
            "file không thuộc Morphix",
        ),
        FileOwnership::Owned => {
            if existing
                .as_ref()
                .is_some_and(|current| is_same(current, file))
            {
                report(
                    id,
                    ComponentStatus::Installed,
                    false,
                    Some(&file.path),
                    "đang hoạt động",
                )
            } else if existing
                .as_ref()
                .is_some_and(|current| current.bytes == file.bytes)
            {
                // WHY tách: nội dung đúng nhưng quyền sai (ví dụ mất bit thực thi) thì cài lại sửa được ngay, không phải do di chuyển binary.
                report(
                    id,
                    ComponentStatus::Stale,
                    false,
                    Some(&file.path),
                    &format!(
                        "quyền file không đúng (cần {:o}); chạy lại install-integration",
                        file.mode
                    ),
                )
            } else {
                // WHY stale: nội dung chứa đường dẫn morphix, nên khác nghĩa là binary đã bị di chuyển hoặc cập nhật.
                report(
                    id,
                    ComponentStatus::Stale,
                    false,
                    Some(&file.path),
                    "morphix đã được di chuyển hoặc cập nhật; chạy lại install-integration",
                )
            }
        }
    }
}

/// Chương trình (tên lệnh, tên hiển thị) mà thành phần phục vụ; chỉ cài khi chương trình có trên máy.
fn file_manager_of(id: ComponentId) -> Option<(&'static str, &'static str)> {
    match id {
        ComponentId::NautilusScript
        | ComponentId::NautilusAccel
        | ComponentId::NautilusExtension => Some(("nautilus", "Nautilus")),
        ComponentId::DolphinServiceMenu => Some(("dolphin", "Dolphin")),
        ComponentId::NemoAction => Some(("nemo", "Nemo")),
        ComponentId::ThunarAction => Some(("thunar", "Thunar")),
        ComponentId::CajaScript => Some(("caja", "Caja")),
        _ => None,
    }
}

/// WHY bỏ qua khi không biết PATH: không đủ dữ liệu để kết luận thiếu, nên giữ hành vi cài như trước.
fn absent_file_manager(session: &SessionEnv, id: ComponentId) -> Option<&'static str> {
    let (program, label) = file_manager_of(id)?;
    (session.path.is_some() && !session.has_program(program)).then_some(label)
}

fn absent_report(id: ComponentId, label: &str) -> IntegrationComponentReport {
    report(
        id,
        ComponentStatus::NotApplicable,
        false,
        None,
        &format!("không có {label} trên máy"),
    )
}

fn is_package_owned(id: ComponentId) -> bool {
    matches!(
        id,
        ComponentId::NautilusExtension
            | ComponentId::ApplicationLauncher
            | ComponentId::DesktopContextMenu
    )
}

fn desktop_menu_not_applicable() -> IntegrationComponentReport {
    report(
        ComponentId::DesktopContextMenu,
        ComponentStatus::NotApplicable,
        false,
        None,
        "menu trên desktop chỉ có trong gói .deb cho Ubuntu",
    )
}

/// Alt+C (phím tắt GNOME và accel Nautilus đi kèm) chỉ chạy trên X11; trên Wayland báo rõ thay vì cài thứ không dùng được.
fn alt_c_unavailable(session: &SessionEnv, id: ComponentId) -> Option<IntegrationComponentReport> {
    let alt_c = matches!(id, ComponentId::GnomeShortcut | ComponentId::NautilusAccel);
    (alt_c && !session.is_x11()).then(|| {
        report(
            id,
            ComponentStatus::NotApplicable,
            false,
            None,
            "Alt+C chỉ có trên X11; trên Wayland hãy dùng menu chuột phải",
        )
    })
}

impl IntegrationBackend for LinuxBackend {
    fn platform(&self) -> &'static str {
        "linux"
    }

    fn install(&self, id: ComponentId, binaries: &BinaryPaths) -> IntegrationComponentReport {
        if let Some(skipped) = alt_c_unavailable(&self.session, id) {
            return skipped;
        }
        if is_package_owned(id) && install_mode(&binaries.morphix) == InstallMode::Package {
            // WHY: gói `.deb` đã cài các mục này cho cả máy; ghi thêm bản per-user sẽ hiện hai mục Morphix.
            return self.inspect(id, binaries);
        }
        if id == ComponentId::DesktopContextMenu {
            return desktop_menu_not_applicable();
        }
        if let Some(label) = absent_file_manager(&self.session, id) {
            return absent_report(id, label);
        }
        let roots = match &self.roots {
            Ok(roots) => roots,
            Err(message) => return failed_report(id, message),
        };
        #[cfg(target_os = "linux")]
        if id == ComponentId::NautilusAccel {
            return super::selection::accelerator(
                roots,
                &*self.tools,
                super::selection::Action::Install,
            );
        }
        #[cfg(target_os = "linux")]
        if id == ComponentId::GnomeShortcut {
            return super::selection::shortcut(
                self,
                super::selection::Action::Install,
                &binaries.morphix,
            );
        }
        if id == ComponentId::ThunarAction {
            return super::thunar::install(&thunar_path(roots), &binaries.morphix);
        }
        match desired_file(roots, id, binaries) {
            None => outside_linux(id),
            Some(Err(message)) => failed_report(id, &message),
            Some(Ok(file)) => install_file(id, &file),
        }
    }

    fn uninstall(&self, id: ComponentId) -> IntegrationComponentReport {
        let roots = match &self.roots {
            Ok(roots) => roots,
            Err(message) => return failed_report(id, message),
        };
        #[cfg(target_os = "linux")]
        if id == ComponentId::NautilusAccel {
            return super::selection::accelerator(
                roots,
                &*self.tools,
                super::selection::Action::Uninstall,
            );
        }
        #[cfg(target_os = "linux")]
        if id == ComponentId::GnomeShortcut {
            return match BinaryPaths::resolve() {
                Ok(binaries) => super::selection::shortcut(
                    self,
                    super::selection::Action::Uninstall,
                    &binaries.morphix,
                ),
                Err(message) => failed_report(id, &message),
            };
        }
        if id == ComponentId::ThunarAction {
            return super::thunar::uninstall(&thunar_path(roots));
        }
        if id == ComponentId::NautilusExtension {
            if let Some(path) = location(roots, id) {
                if let Err(error) = remove_extension_bytecode(&path) {
                    return io_failure(id, &path, &error);
                }
            }
        }
        match location(roots, id) {
            None => outside_linux(id),
            Some(path) => uninstall_file(id, &path),
        }
    }

    fn inspect(&self, id: ComponentId, binaries: &BinaryPaths) -> IntegrationComponentReport {
        let inspected = self.inspect_component(id, binaries);
        // WHY: trình quản lý file không có trên máy thì "chưa cài" là đúng, không phải thiếu sót cần sửa.
        match absent_file_manager(&self.session, id) {
            Some(label) if inspected.status == ComponentStatus::Missing => absent_report(id, label),
            _ => inspected,
        }
    }
}

impl LinuxBackend {
    fn inspect_component(
        &self,
        id: ComponentId,
        binaries: &BinaryPaths,
    ) -> IntegrationComponentReport {
        // WHY trước roots: tray phụ thuộc phiên GNOME, không phụ thuộc HOME; thiếu HOME không được làm nó thành failed.
        if id == ComponentId::TraySupport {
            return tray_report(&self.session, &*self.tools);
        }
        if let Some(skipped) = alt_c_unavailable(&self.session, id) {
            return skipped;
        }
        let package = install_mode(&binaries.morphix) == InstallMode::Package;
        if id == ComponentId::DesktopContextMenu && !package {
            return desktop_menu_not_applicable();
        }
        if id == ComponentId::DesktopContextMenu {
            return super::desktop::inspect_desktop(
                Path::new(super::desktop::SYSTEM_DING_EXTENSION_PATH),
                &*self.tools,
            );
        }
        if id == ComponentId::NautilusExtension && package {
            return super::desktop::inspect_package_file(
                id,
                Path::new(NAUTILUS_EXTENSION_PATH),
                &*self.tools,
            );
        }
        if id == ComponentId::ApplicationLauncher && package {
            return super::desktop::inspect_package_file(
                id,
                Path::new(APPLICATION_LAUNCHER_PATH),
                &*self.tools,
            );
        }
        let roots = match &self.roots {
            Ok(roots) => roots,
            Err(message) => return failed_report(id, message),
        };
        if id == ComponentId::DesktopValidation {
            return desktop_validation_report(roots, &*self.tools);
        }
        #[cfg(target_os = "linux")]
        if id == ComponentId::NautilusAccel {
            return super::selection::accelerator(
                roots,
                &*self.tools,
                super::selection::Action::Inspect,
            );
        }
        #[cfg(target_os = "linux")]
        if id == ComponentId::GnomeShortcut {
            return super::selection::shortcut(
                self,
                super::selection::Action::Inspect,
                &binaries.morphix,
            );
        }
        if id == ComponentId::ThunarAction {
            return super::thunar::inspect(&thunar_path(roots), &binaries.morphix);
        }
        match desired_file(roots, id, binaries) {
            None => outside_linux(id),
            Some(Err(message)) => failed_report(id, &message),
            Some(Ok(file)) if id == ComponentId::NautilusExtension => {
                // WHY: file extension có mà thiếu binding Python của Nautilus thì Nautilus không nạp nó; báo đã cài là sai.
                if nautilus_python_present(&NAUTILUS_EXTENSION_DIRS) {
                    inspect_file(id, &file)
                } else {
                    report(
                        id,
                        ComponentStatus::Missing,
                        false,
                        Some(&file.path),
                        "thiếu binding Python của Nautilus nên Files không nạp menu Morphix; cài gói nautilus-python (Fedora, Arch, openSUSE) hoặc python3-nautilus (Debian, Ubuntu) rồi mở lại Files",
                    )
                }
            }
            Some(Ok(file)) => inspect_file(id, &file),
        }
    }
}

/// Thư mục chứa `libnautilus-python.so` trên các bản phân phối phổ biến (Fedora/Arch dùng lib64/lib, Debian/Ubuntu dùng arch triplet).
const NAUTILUS_EXTENSION_DIRS: [&str; 4] = [
    "/usr/lib64/nautilus/extensions-4",
    "/usr/lib/nautilus/extensions-4",
    "/usr/lib/x86_64-linux-gnu/nautilus/extensions-4",
    "/usr/lib/nautilus/extensions-3.0",
];

/// Có binding `libnautilus-python.so` trong một trong các thư mục `dirs`.
pub fn nautilus_python_present(dirs: &[&str]) -> bool {
    dirs.iter()
        .any(|dir| Path::new(dir).join("libnautilus-python.so").is_file())
}

/// AppIndicator là extension GNOME nên chỉ kiểm trên phiên GNOME. Thiếu dữ liệu thì `unknown`, không `missing`:
/// không có D-Bus hay không đọc được danh sách không có nghĩa là extension chưa bật.
fn tray_report(session: &SessionEnv, tools: &ToolRunner) -> IntegrationComponentReport {
    let id = ComponentId::TraySupport;
    if !session.is_gnome() {
        return report(
            id,
            ComponentStatus::NotApplicable,
            false,
            None,
            "phiên hiện tại không phải GNOME; AppIndicator không áp dụng",
        );
    }
    if !session.has_session_bus {
        return report(
            id,
            ComponentStatus::Unknown,
            false,
            None,
            "thiếu phiên D-Bus; không kiểm được AppIndicator",
        );
    }
    match tools(
        "gnome-extensions",
        &[OsStr::new("list"), OsStr::new("--enabled")],
    ) {
        ToolOutcome::NotFound => report(
            id,
            ComponentStatus::Unknown,
            false,
            None,
            "không có gnome-extensions; không kiểm được AppIndicator",
        ),
        ToolOutcome::Failed(message) => report(id, ComponentStatus::Unknown, false, None, &message),
        ToolOutcome::Exited {
            code: Some(0),
            stdout,
            ..
        } => {
            if lists_appindicator(&stdout) {
                report(
                    id,
                    ComponentStatus::Installed,
                    false,
                    None,
                    "AppIndicator đang bật; icon khay hiện được",
                )
            } else {
                report(
                    id,
                    ComponentStatus::Missing,
                    false,
                    None,
                    "AppIndicator chưa bật: cài gói gnome-shell-extension-appindicator, chạy `gnome-extensions enable appindicatorsupport@rgcjonas.gmail.com` rồi đăng nhập lại; tạm thời dùng `morphix tray`",
                )
            }
        }
        ToolOutcome::Exited {
            code,
            stdout,
            stderr,
        } => report(
            id,
            ComponentStatus::Unknown,
            false,
            None,
            &format!(
                "gnome-extensions thoát {} : {}",
                exit_code_text(code),
                output_detail(&stdout, &stderr)
            ),
        ),
    }
}

/// WHY so khớp đúng dòng: `gnome-extensions list` in một UUID mỗi dòng, so khớp chuỗi con sẽ nhận nhầm UUID dài hơn.
fn lists_appindicator(listing: &str) -> bool {
    listing
        .lines()
        .map(str::trim)
        .any(|line| APPINDICATOR_UUIDS.contains(&line))
}

/// Kiểm mọi file `.desktop` đã cài bằng `desktop-file-validate` nếu có, rồi lấy mức xấu nhất. WHY không dừng ở file đầu:
/// Dolphin luôn bị validator từ chối, nên dừng sớm sẽ che mất việc kiểm file autostart.
fn desktop_validation_report(roots: &XdgRoots, tools: &ToolRunner) -> IntegrationComponentReport {
    let id = ComponentId::DesktopValidation;
    let mut reports: Vec<IntegrationComponentReport> = Vec::new();
    let entries = [ComponentId::DolphinServiceMenu, ComponentId::Autostart];
    for path in entries.iter().filter_map(|&entry| location(roots, entry)) {
        match read_existing(&path) {
            Ok(None) => continue,
            Ok(Some(_)) => reports.push(validate_desktop_file(id, &path, tools)),
            Err(error) => reports.push(io_failure(id, &path, &error)),
        }
    }
    reports
        .into_iter()
        .max_by_key(|file_report| validation_severity(file_report.status))
        .unwrap_or_else(|| {
            report(
                id,
                ComponentStatus::Missing,
                false,
                None,
                "chưa có file .desktop nào để kiểm tra",
            )
        })
}

fn validate_desktop_file(
    id: ComponentId,
    path: &Path,
    tools: &ToolRunner,
) -> IntegrationComponentReport {
    match tools("desktop-file-validate", &[path.as_os_str()]) {
        ToolOutcome::NotFound => report(
            id,
            ComponentStatus::NotApplicable,
            false,
            None,
            "không có desktop-file-validate; bỏ qua kiểm tra cú pháp .desktop",
        ),
        ToolOutcome::Failed(message) => {
            report(id, ComponentStatus::Unknown, false, Some(path), &message)
        }
        ToolOutcome::Exited { code: Some(0), .. } => report(
            id,
            ComponentStatus::Installed,
            false,
            None,
            "file .desktop hợp lệ theo desktop-file-validate",
        ),
        ToolOutcome::Exited {
            code,
            stdout,
            stderr,
        } => match validator_failure_status(&format!("{stdout}\n{stderr}")) {
            ComponentStatus::Installed => report(
                id,
                ComponentStatus::Installed,
                false,
                None,
                "file .desktop hợp lệ; desktop-file-validate chỉ báo giới hạn đã biết với service menu KDE",
            ),
            status => report(
                id,
                status,
                false,
                Some(path),
                &format!(
                    "desktop-file-validate thoát {} : {}",
                    exit_code_text(code),
                    output_detail(&stdout, &stderr)
                ),
            ),
        },
    }
}

/// WHY thứ tự này: một file lỗi thật phải thắng file hợp lệ hoặc chưa kiểm được.
fn validation_severity(status: ComponentStatus) -> u8 {
    match status {
        ComponentStatus::Failed => 3,
        ComponentStatus::Unknown => 2,
        ComponentStatus::NotApplicable => 1,
        _ => 0,
    }
}

/// Mức trạng thái khi validator báo lỗi. WHY `installed` khi mọi dòng là giới hạn đã biết của Task 19:
/// đã đối chiếu KDE docs nên đó không phải lỗi, báo `unknown` chỉ làm người dùng lo vô cớ. Lỗi khác (cú pháp, thiếu Name, khoá sai với Type=Application) là hỏng thật nên `failed`; không có dòng nào thì không giải thích được, cũng `failed`.
fn validator_failure_status(output: &str) -> ComponentStatus {
    let lines: Vec<&str> = output
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect();
    if !lines.is_empty() && lines.iter().all(|line| is_known_service_restriction(line)) {
        ComponentStatus::Installed
    } else {
        ComponentStatus::Failed
    }
}

/// WHY so khớp nguyên văn đuôi thông báo của desktop-file-validate 0.27: chỉ MimeType/Actions trong Type=Service, theo ruling Task 19.
fn is_known_service_restriction(line: &str) -> bool {
    // WHY: desktop-file-validate 0.28 còn báo `all/allfiles` (MIME riêng của KDE cho service menu) là không hợp lệ.
    if line.ends_with(
        "contains value \"all/allfiles\" which is an invalid MIME type: \"all\" is an unregistered media type",
    ) {
        return true;
    }
    ["MimeType", "Actions"].iter().any(|key| {
        line.ends_with(&format!(
            "error: key \"{key}\" is present in group \"Desktop Entry\", but the type is \"Service\" while this key is only valid for type \"Application\""
        ))
    })
}

/// WHY cả hai luồng: desktop-file-validate in lỗi ra stdout, còn gnome-extensions dùng stderr; chỉ lấy một luồng sẽ làm mất lý do.
fn output_detail(stdout: &str, stderr: &str) -> String {
    [stdout.trim(), stderr.trim()]
        .into_iter()
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>()
        .join(" | ")
}

fn exit_code_text(code: Option<i32>) -> String {
    code.map_or_else(|| "không có mã".to_string(), |code| code.to_string())
}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, fs, rc::Rc};

    use tempfile::tempdir;

    use super::*;

    const DOLPHIN_RESTRICTION_MIME: &str = "/x/morphix.desktop: error: key \"MimeType\" is present in group \"Desktop Entry\", but the type is \"Service\" while this key is only valid for type \"Application\"";
    const DOLPHIN_RESTRICTION_ACTIONS: &str = "/x/morphix.desktop: error: key \"Actions\" is present in group \"Desktop Entry\", but the type is \"Service\" while this key is only valid for type \"Application\"";

    #[test]
    fn dolphin_service_restriction_only_counts_as_installed() {
        let output = format!("{DOLPHIN_RESTRICTION_MIME}\n{DOLPHIN_RESTRICTION_ACTIONS}\n");
        assert_eq!(
            validator_failure_status(&output),
            ComponentStatus::Installed
        );
    }

    #[test]
    fn kde_allfiles_mime_warning_of_validator_0_28_counts_as_installed() {
        let output = format!(
            "/x/morphix.desktop: error: (will be fatal in the future): value \"all/allfiles;\" for key \"MimeType\" in group \"Desktop Entry\" contains value \"all/allfiles\" which is an invalid MIME type: \"all\" is an unregistered media type\n{DOLPHIN_RESTRICTION_MIME}\n{DOLPHIN_RESTRICTION_ACTIONS}\n"
        );
        assert_eq!(
            validator_failure_status(&output),
            ComponentStatus::Installed
        );
    }

    #[test]
    fn autostart_missing_name_is_failed() {
        let output = "/x/morphix.desktop: error: required key \"Name\" in group \"Desktop Entry\" is not present\n";
        assert_eq!(validator_failure_status(output), ComponentStatus::Failed);
    }

    #[test]
    fn dolphin_restriction_mixed_with_a_syntax_error_is_failed() {
        let output = format!(
            "{DOLPHIN_RESTRICTION_MIME}\n/x/morphix.desktop: error: file contains line \"junk\", which is not a comment, a group or an entry\n"
        );
        assert_eq!(validator_failure_status(&output), ComponentStatus::Failed);
    }

    #[test]
    fn validator_failure_without_any_message_is_failed() {
        assert_eq!(validator_failure_status("  \n"), ComponentStatus::Failed);
    }

    #[test]
    fn every_desktop_file_is_validated_even_after_an_invalid_one() {
        let sandbox = tempdir().unwrap();
        let roots = XdgRoots {
            data_home: sandbox.path().join("data"),
            config_home: sandbox.path().join("config"),
        };
        let dolphin = location(&roots, ComponentId::DolphinServiceMenu).unwrap();
        let autostart = location(&roots, ComponentId::Autostart).unwrap();
        for path in [&dolphin, &autostart] {
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, "[Desktop Entry]\n").unwrap();
        }
        // WHY Rc: ToolRunner là `dyn Fn` 'static nên closure phải sở hữu trạng thái ghi lại.
        let checked: Rc<RefCell<Vec<PathBuf>>> = Rc::new(RefCell::new(Vec::new()));
        let recorder = Rc::clone(&checked);
        let invalid_path = dolphin.clone();
        let tools = move |_program: &str, args: &[&OsStr]| -> ToolOutcome {
            recorder.borrow_mut().extend(args.iter().map(PathBuf::from));
            if args.iter().any(|arg| *arg == invalid_path.as_os_str()) {
                ToolOutcome::Exited {
                    code: Some(1),
                    stdout: "/x/file.desktop: error: file contains line \"junk\"\n".to_string(),
                    stderr: String::new(),
                }
            } else {
                ToolOutcome::Exited {
                    code: Some(0),
                    stdout: String::new(),
                    stderr: String::new(),
                }
            }
        };
        let result = desktop_validation_report(&roots, &tools);
        assert_eq!(
            checked.borrow().as_slice(),
            &[dolphin.clone(), autostart.clone()]
        );
        assert_eq!(result.status, ComponentStatus::Failed);
        assert!(result.message.contains("junk"), "{}", result.message);
    }
}

#[cfg(test)]
mod nautilus_python_tests {
    use super::nautilus_python_present;

    #[test]
    fn binding_is_found_only_where_nautilus_loads_it() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().to_str().unwrap();
        assert!(!nautilus_python_present(&[path]));
        std::fs::write(dir.path().join("libnautilus-python.so"), b"").unwrap();
        assert!(nautilus_python_present(&["/nonexistent", path]));
    }
}
