use std::{
    collections::BTreeMap,
    ffi::OsStr,
    fs,
    path::{Path, PathBuf},
};
// WHY unix: chỉ các test chạy script giả cần Command, và các test đó đều gate unix.
#[cfg(unix)]
use std::process::Command;

use assert_cmd::{assert::Assert, cargo::cargo_bin_cmd};
use morphix_cli::integration::windows;
use morphix_cli::integration::{
    self,
    fake::FakeBackend,
    files::{publish_file, remove_dir_if_empty, Transaction, MANAGED_MARKER},
    linux::{
        autostart_entry_body, desktop_exec_argument, dolphin_service_menu_body,
        escape_desktop_value, install_mode, nautilus_script_body, nemo_action_body,
        resolve_xdg_roots, InstallMode, LinuxBackend, SessionEnv, XdgRoots,
    },
    report::doctor_exit_code,
    tools::{ToolOutcome, ToolRunner},
    BinaryPaths, ComponentId, ComponentStatus, IntegrationBackend, IntegrationComponentReport,
    IntegrationOptions, IntegrationReport, OverallStatus, UnsupportedBackend,
};
use serde_json::{json, Value};
use tempfile::{tempdir, TempDir};

/// Thư mục gốc tạm cho một lần test: binary nằm trong đường dẫn có khoảng trắng, dấu nháy, `&`
/// để kiểm đường dẫn được ghi nguyên văn (quoting thuộc backend, không thuộc lớp này).
struct Fixture {
    _dir: TempDir,
    state: PathBuf,
    binaries: BinaryPaths,
}

impl Fixture {
    fn new() -> Self {
        let dir = tempdir().unwrap();
        let apps = dir.path().join("Apps \"v1\" & it's/Morphix");
        fs::create_dir_all(&apps).unwrap();
        let binaries = binaries_in(&apps);
        Fixture {
            state: dir.path().join("state"),
            _dir: dir,
            binaries,
        }
    }

    fn fake(&self) -> FakeBackend {
        FakeBackend::new(self.state.clone())
    }

    fn options(&self, dry_run: bool, install_autostart: bool) -> IntegrationOptions {
        IntegrationOptions {
            dry_run,
            install_autostart,
        }
    }
}

fn binaries_in(dir: &Path) -> BinaryPaths {
    let morphix = dir.join("morphix");
    let gui = dir.join("morphix-gui");
    fs::write(&morphix, b"bin").unwrap();
    fs::write(&gui, b"bin").unwrap();
    BinaryPaths { morphix, gui }
}

const INSTALLABLE: [ComponentId; 11] = [
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

fn status_of(report: &IntegrationReport, id: ComponentId) -> ComponentStatus {
    report
        .components
        .iter()
        .find(|component| component.id == id)
        .unwrap_or_else(|| panic!("thiếu thành phần {id:?}"))
        .status
}

/// Toàn bộ file dưới `dir` theo đường dẫn; thư mục chưa tồn tại coi như rỗng.
fn snapshot(dir: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    let mut files = BTreeMap::new();
    let Ok(entries) = fs::read_dir(dir) else {
        return files;
    };
    for entry in entries {
        let path = entry.expect("đọc mục thư mục").path();
        if path.is_dir() {
            files.extend(snapshot(&path));
        } else {
            files.insert(path.clone(), fs::read(&path).expect("đọc file"));
        }
    }
    files
}

#[cfg(unix)]
fn mode_of(path: &Path) -> u32 {
    use std::os::unix::fs::PermissionsExt;
    fs::metadata(path).unwrap().permissions().mode() & 0o777
}

#[cfg(unix)]
fn set_mode(path: &Path, mode: u32) {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(mode)).unwrap();
}

fn stdout_json(assert: &Assert) -> Value {
    serde_json::from_slice(&assert.get_output().stdout)
        .expect("stdout phải là đúng một JSON object")
}

fn component(id: ComponentId, status: ComponentStatus) -> IntegrationComponentReport {
    IntegrationComponentReport {
        id,
        status,
        changed: false,
        path: None,
        message: String::new(),
    }
}

// ---- Report schema ----

#[test]
fn report_json_uses_the_stable_field_names_and_values() {
    let report = IntegrationReport {
        platform: "linux".into(),
        overall: OverallStatus::DryRun,
        components: vec![IntegrationComponentReport {
            id: ComponentId::WindowsContextMenu,
            status: ComponentStatus::AlreadyInstalled,
            changed: false,
            path: Some(PathBuf::from("/x/y")),
            message: "đã có".into(),
        }],
    };
    assert_eq!(
        serde_json::to_value(&report).unwrap(),
        json!({
            "platform": "linux",
            "overall": "dry_run",
            "components": [{
                "id": "windows_context_menu",
                "status": "already_installed",
                "changed": false,
                "path": "/x/y",
                "message": "đã có",
            }],
        })
    );
}

#[test]
fn every_component_and_status_name_matches_the_contract() {
    let ids: Vec<Value> = [
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
    ]
    .iter()
    .map(|id| serde_json::to_value(id).unwrap())
    .collect();
    assert_eq!(
        ids,
        vec![
            "windows_context_menu",
            "nautilus_script",
            "nautilus_accel",
            "gnome_shortcut",
            "dolphin_service_menu",
            "autostart",
            "tray_support",
            "desktop_validation",
            "nautilus_extension",
            "desktop_context_menu",
            "application_launcher",
            "nemo_action",
            "caja_script",
            "thunar_action"
        ]
    );
    let statuses: Vec<Value> = [
        ComponentStatus::Installed,
        ComponentStatus::AlreadyInstalled,
        ComponentStatus::Removed,
        ComponentStatus::AlreadyAbsent,
        ComponentStatus::Missing,
        ComponentStatus::Stale,
        ComponentStatus::Conflict,
        ComponentStatus::Unsupported,
        ComponentStatus::Unknown,
        ComponentStatus::NotApplicable,
        ComponentStatus::Failed,
    ]
    .iter()
    .map(|status| serde_json::to_value(status).unwrap())
    .collect();
    assert_eq!(
        statuses,
        vec![
            "installed",
            "already_installed",
            "removed",
            "already_absent",
            "missing",
            "stale",
            "conflict",
            "unsupported",
            "unknown",
            "not_applicable",
            "failed"
        ]
    );
    let overall: Vec<Value> = [
        OverallStatus::Success,
        OverallStatus::Partial,
        OverallStatus::Failure,
        OverallStatus::DryRun,
        OverallStatus::Inconclusive,
    ]
    .iter()
    .map(|status| serde_json::to_value(status).unwrap())
    .collect();
    assert_eq!(
        overall,
        vec!["success", "partial", "failure", "dry_run", "inconclusive"]
    );
}

#[test]
fn default_options_install_autostart() {
    let options = IntegrationOptions::default();
    assert!(!options.dry_run);
    assert!(options.install_autostart);
}

// ---- Install, uninstall, inspect with the fake adapter ----

#[test]
fn install_writes_every_installable_component_and_exits_0() {
    let f = Fixture::new();
    let report = integration::install(&f.fake(), &f.binaries, &f.options(false, true));
    assert_eq!(report.overall, OverallStatus::Success);
    for id in INSTALLABLE {
        assert_eq!(status_of(&report, id), ComponentStatus::Installed);
    }
    assert_eq!(integration::exit_code_for(report.overall), 0);
}

#[test]
fn installing_twice_is_idempotent() {
    let f = Fixture::new();
    let backend = f.fake();
    integration::install(&backend, &f.binaries, &f.options(false, true));
    let before = snapshot(&f.state);
    let second = integration::install(&backend, &f.binaries, &f.options(false, true));
    assert_eq!(second.overall, OverallStatus::Success);
    for component in &second.components {
        assert_eq!(component.status, ComponentStatus::AlreadyInstalled);
        assert!(!component.changed);
    }
    assert_eq!(snapshot(&f.state), before);
}

#[test]
fn uninstall_removes_owned_entries_and_second_run_is_already_absent() {
    let f = Fixture::new();
    let backend = f.fake();
    integration::install(&backend, &f.binaries, &f.options(false, true));
    let first = integration::uninstall(&backend, &f.binaries, &f.options(false, true));
    assert_eq!(first.overall, OverallStatus::Success);
    for id in INSTALLABLE {
        assert_eq!(status_of(&first, id), ComponentStatus::Removed);
    }
    assert!(snapshot(&f.state).is_empty());

    let second = integration::uninstall(&backend, &f.binaries, &f.options(false, true));
    assert_eq!(second.overall, OverallStatus::Success);
    for id in INSTALLABLE {
        assert_eq!(status_of(&second, id), ComponentStatus::AlreadyAbsent);
    }
    assert_eq!(integration::exit_code_for(second.overall), 0);
}

#[test]
fn user_integration_lifecycle_excludes_only_the_desktop_menu() {
    // WHY: bản cài trong home tự ghi extension Nautilus và launcher; chỉ menu DING là riêng của gói .deb.
    let f = Fixture::new();
    let install = integration::install(&f.fake(), &f.binaries, &f.options(false, true));
    let uninstall = integration::uninstall(&f.fake(), &f.binaries, &f.options(false, true));
    for report in [install, uninstall] {
        let ids: Vec<ComponentId> = report.components.iter().map(|c| c.id).collect();
        assert!(!ids.contains(&ComponentId::DesktopContextMenu));
        assert!(ids.contains(&ComponentId::NautilusExtension));
        assert!(ids.contains(&ComponentId::ApplicationLauncher));
    }
}

#[test]
fn dry_run_install_writes_nothing() {
    let f = Fixture::new();
    let report = integration::install(&f.fake(), &f.binaries, &f.options(true, true));
    assert_eq!(report.overall, OverallStatus::DryRun);
    assert!(report.components.iter().all(|c| !c.changed));
    assert!(snapshot(&f.state).is_empty());
    assert_eq!(integration::exit_code_for(report.overall), 0);
}

#[test]
fn dry_run_uninstall_keeps_installed_entries() {
    let f = Fixture::new();
    let backend = f.fake();
    integration::install(&backend, &f.binaries, &f.options(false, true));
    let before = snapshot(&f.state);
    let report = integration::uninstall(&backend, &f.binaries, &f.options(true, true));
    assert_eq!(report.overall, OverallStatus::DryRun);
    assert_eq!(snapshot(&f.state), before);
}

#[test]
fn no_autostart_leaves_out_only_the_autostart_component() {
    let f = Fixture::new();
    let backend = f.fake();
    let report = integration::install(&backend, &f.binaries, &f.options(false, false));
    let ids: Vec<ComponentId> = report.components.iter().map(|c| c.id).collect();
    assert_eq!(
        ids,
        vec![
            ComponentId::WindowsContextMenu,
            ComponentId::NautilusScript,
            ComponentId::NautilusAccel,
            ComponentId::GnomeShortcut,
            ComponentId::DolphinServiceMenu,
            ComponentId::NautilusExtension,
            ComponentId::ApplicationLauncher,
            ComponentId::NemoAction,
            ComponentId::CajaScript,
            ComponentId::ThunarAction,
        ]
    );
    assert!(!backend.entry_path(ComponentId::Autostart).exists());
}

#[test]
fn foreign_file_at_target_is_reported_as_conflict_and_kept_byte_for_byte() {
    let f = Fixture::new();
    let backend = f.fake();
    let foreign = backend.entry_path(ComponentId::NautilusScript);
    fs::create_dir_all(foreign.parent().unwrap()).unwrap();
    fs::write(&foreign, b"script cua nguoi dung\n").unwrap();
    #[cfg(unix)]
    set_mode(&foreign, 0o640);

    let report = integration::install(&backend, &f.binaries, &f.options(false, true));
    assert_eq!(
        status_of(&report, ComponentId::NautilusScript),
        ComponentStatus::Conflict
    );
    assert_eq!(fs::read(&foreign).unwrap(), b"script cua nguoi dung\n");
    #[cfg(unix)]
    assert_eq!(mode_of(&foreign), 0o640);
    assert_eq!(report.overall, OverallStatus::Partial);
    assert_eq!(integration::exit_code_for(report.overall), 3);
}

#[test]
fn uninstall_never_removes_foreign_files() {
    let f = Fixture::new();
    let backend = f.fake();
    integration::install(&backend, &f.binaries, &f.options(false, true));
    let foreign = backend.entry_path(ComponentId::NautilusScript);
    fs::write(&foreign, b"khong phai cua Morphix\n").unwrap();

    let report = integration::uninstall(&backend, &f.binaries, &f.options(false, true));
    assert_eq!(
        status_of(&report, ComponentId::NautilusScript),
        ComponentStatus::Conflict
    );
    assert_eq!(fs::read(&foreign).unwrap(), b"khong phai cua Morphix\n");
    assert_eq!(report.overall, OverallStatus::Success);
    assert!(!backend.entry_path(ComponentId::Autostart).exists());
}

#[test]
fn failure_after_first_file_rolls_back_the_new_entries_and_directory() {
    let f = Fixture::new();
    let backend = f
        .fake()
        .failing_after_first_file(ComponentId::NautilusScript);
    let report = integration::install(&backend, &f.binaries, &f.options(false, true));
    assert_eq!(
        status_of(&report, ComponentId::NautilusScript),
        ComponentStatus::Failed
    );
    assert!(!backend
        .entry_path(ComponentId::NautilusScript)
        .parent()
        .unwrap()
        .exists());
    assert_eq!(
        status_of(&report, ComponentId::Autostart),
        ComponentStatus::Installed
    );
    assert_eq!(report.overall, OverallStatus::Partial);
    assert_eq!(integration::exit_code_for(report.overall), 3);
}

#[test]
fn every_component_failing_is_a_total_failure_that_exits_1() {
    let f = Fixture::new();
    let mut backend = f.fake();
    for id in INSTALLABLE {
        backend = backend.failing_after_first_file(id);
    }
    let report = integration::install(&backend, &f.binaries, &f.options(false, true));
    assert_eq!(report.overall, OverallStatus::Failure);
    assert!(snapshot(&f.state).is_empty());
    assert_eq!(integration::exit_code_for(report.overall), 1);
}

#[test]
fn moved_binary_is_reported_stale_and_reinstall_repairs_it() {
    let f = Fixture::new();
    let backend = f.fake();
    integration::install(&backend, &f.binaries, &f.options(false, true));

    let moved_dir = tempdir().unwrap();
    let moved = binaries_in(moved_dir.path());
    let inspection = integration::inspect(&backend, &moved);
    for id in INSTALLABLE {
        assert_eq!(status_of(&inspection, id), ComponentStatus::Stale);
    }

    let repaired = integration::install(&backend, &moved, &f.options(false, true));
    for id in INSTALLABLE {
        assert_eq!(status_of(&repaired, id), ComponentStatus::Installed);
    }
}

#[test]
fn binary_path_with_spaces_quotes_and_ampersand_is_recorded_verbatim() {
    let f = Fixture::new();
    let backend = f.fake();
    integration::install(&backend, &f.binaries, &f.options(false, true));
    let text = fs::read_to_string(backend.entry_path(ComponentId::NautilusScript)).unwrap();
    assert!(
        text.contains(&format!("target={}", f.binaries.morphix.display())),
        "{text}"
    );
    let inspection = integration::inspect(&backend, &f.binaries);
    assert_eq!(
        status_of(&inspection, ComponentId::NautilusScript),
        ComponentStatus::Installed
    );
}

#[test]
fn missing_morphix_binary_fails_every_install_component() {
    let f = Fixture::new();
    fs::remove_file(&f.binaries.morphix).unwrap();
    let report = integration::install(&f.fake(), &f.binaries, &f.options(false, true));
    for id in INSTALLABLE {
        assert_eq!(status_of(&report, id), ComponentStatus::Failed);
    }
    assert_eq!(report.overall, OverallStatus::Failure);
    assert_eq!(integration::exit_code_for(report.overall), 1);
}

#[test]
fn missing_gui_binary_fails_only_autostart() {
    let f = Fixture::new();
    fs::remove_file(&f.binaries.gui).unwrap();
    let report = integration::install(&f.fake(), &f.binaries, &f.options(false, true));
    assert_eq!(
        status_of(&report, ComponentId::Autostart),
        ComponentStatus::Failed
    );
    assert_eq!(
        status_of(&report, ComponentId::NautilusScript),
        ComponentStatus::Installed
    );
    assert_eq!(integration::exit_code_for(report.overall), 3);
}

#[test]
fn unsupported_backend_cannot_continue_and_exits_1() {
    let f = Fixture::new();
    let report = integration::install(&UnsupportedBackend, &f.binaries, &f.options(false, true));
    for id in INSTALLABLE {
        assert_eq!(status_of(&report, id), ComponentStatus::Unsupported);
    }
    assert_eq!(report.overall, OverallStatus::Failure);
    assert_eq!(integration::exit_code_for(report.overall), 1);
    let doctor = integration::inspect(&UnsupportedBackend, &f.binaries);
    assert_eq!(doctor.overall, OverallStatus::Inconclusive);
    assert_eq!(integration::doctor_exit_code(&doctor.components), 3);
}

// ---- Exit code and doctor mapping ----

#[test]
fn exit_codes_map_overall_status_to_the_spec_values() {
    assert_eq!(integration::exit_code_for(OverallStatus::Success), 0);
    assert_eq!(integration::exit_code_for(OverallStatus::DryRun), 0);
    assert_eq!(integration::exit_code_for(OverallStatus::Partial), 3);
    assert_eq!(integration::exit_code_for(OverallStatus::Failure), 1);
    assert_eq!(integration::exit_code_for(OverallStatus::Inconclusive), 3);
}

#[test]
fn doctor_exit_code_prefers_errors_over_inconclusive_checks() {
    use ComponentId::*;
    use ComponentStatus as S;
    let case = |statuses: &[S]| -> u8 {
        let components: Vec<_> = statuses
            .iter()
            .zip([
                WindowsContextMenu,
                NautilusScript,
                DolphinServiceMenu,
                Autostart,
                TraySupport,
            ])
            .map(|(status, id)| component(id, *status))
            .collect();
        integration::doctor_exit_code(&components)
    };
    assert_eq!(case(&[S::Installed, S::Missing]), 0);
    // Không có mục nào kiểm được thì không được coi là khoẻ mạnh.
    assert_eq!(case(&[S::Unsupported, S::NotApplicable]), 3);
    assert_eq!(case(&[S::Unknown, S::Installed]), 3);
    assert_eq!(case(&[S::Installed, S::Stale]), 1);
    assert_eq!(case(&[S::Conflict, S::Unknown]), 1);
    assert_eq!(case(&[S::Failed, S::Unknown]), 1);
}

#[test]
fn foreign_file_quoting_the_marker_inside_a_sentence_is_kept() {
    let f = Fixture::new();
    let backend = f.fake();
    let foreign = backend.entry_path(ComponentId::WindowsContextMenu);
    fs::create_dir_all(foreign.parent().unwrap()).unwrap();
    let content = format!("Ghi chú của tôi: {MANAGED_MARKER} là phần mềm tôi đã gỡ.\n");
    fs::write(&foreign, &content).unwrap();

    let report = integration::uninstall(&backend, &f.binaries, &f.options(false, true));
    assert_eq!(
        status_of(&report, ComponentId::WindowsContextMenu),
        ComponentStatus::Conflict
    );
    assert_eq!(fs::read_to_string(&foreign).unwrap(), content);
}

#[cfg(unix)]
#[test]
fn non_utf8_path_serializes_to_valid_json_without_panicking() {
    use std::{ffi::OsString, os::unix::ffi::OsStringExt};
    let path = PathBuf::from(OsString::from_vec(b"/opt/\xff/morphix".to_vec()));
    let report = IntegrationReport {
        platform: "linux".into(),
        overall: OverallStatus::Failure,
        components: vec![IntegrationComponentReport {
            id: ComponentId::Autostart,
            status: ComponentStatus::Failed,
            changed: false,
            path: Some(path),
            message: "không tìm thấy".into(),
        }],
    };
    let json = serde_json::to_string(&report).unwrap();
    let value: Value = serde_json::from_str(&json).unwrap();
    assert_eq!(value["components"][0]["path"], "/opt/\u{FFFD}/morphix");
}

// ---- Generic file helpers ----

#[test]
fn publish_file_creates_missing_directories_with_mode_and_no_temp_leftovers() {
    let dir = tempdir().unwrap();
    let target = dir.path().join("sub dir/script");
    let mut tx = Transaction::new();
    publish_file(&target, b"#!/bin/sh\n", 0o755, &mut tx).unwrap();
    tx.commit();
    assert_eq!(fs::read(&target).unwrap(), b"#!/bin/sh\n");
    #[cfg(unix)]
    assert_eq!(mode_of(&target), 0o755);
    let names: Vec<_> = fs::read_dir(target.parent().unwrap())
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert_eq!(names, vec!["script"]);
}

#[test]
fn dropping_an_uncommitted_transaction_restores_replaced_bytes_and_mode() {
    let dir = tempdir().unwrap();
    let target = dir.path().join("entry");
    fs::write(&target, format!("{MANAGED_MARKER}\nv1\n")).unwrap();
    #[cfg(unix)]
    set_mode(&target, 0o600);
    {
        let mut tx = Transaction::new();
        publish_file(&target, b"v2\n", 0o755, &mut tx).unwrap();
        assert_eq!(fs::read(&target).unwrap(), b"v2\n");
    }
    assert_eq!(
        fs::read_to_string(&target).unwrap(),
        format!("{MANAGED_MARKER}\nv1\n")
    );
    #[cfg(unix)]
    assert_eq!(mode_of(&target), 0o600);
}

#[test]
fn dropping_an_uncommitted_transaction_removes_new_files_and_directories() {
    let dir = tempdir().unwrap();
    let nested = dir.path().join("a/b");
    {
        let mut tx = Transaction::new();
        publish_file(&nested.join("entry"), b"x", 0o644, &mut tx).unwrap();
    }
    assert!(!dir.path().join("a").exists());
}

#[test]
fn remove_dir_if_empty_keeps_directories_that_still_hold_files() {
    let dir = tempdir().unwrap();
    let kept = dir.path().join("kept");
    fs::create_dir(&kept).unwrap();
    fs::write(kept.join("user-file"), b"x").unwrap();
    remove_dir_if_empty(&kept).unwrap();
    assert!(kept.join("user-file").exists());

    let empty = dir.path().join("empty");
    fs::create_dir(&empty).unwrap();
    remove_dir_if_empty(&empty).unwrap();
    assert!(!empty.exists());
}

// ---- CLI binary: schema, exit codes, usage errors, doctor ----

/// Mã thoát khi cài trên Linux: backend thật thành công. Chỉ dùng cho test Linux vì Windows ghi HKCU thật,
/// mà sandbox của CLI không đổi được HKCU.
#[cfg(target_os = "linux")]
const INSTALL_EXIT_CODE: i32 = 0;
/// Mã thoát doctor khi chưa cài gì trên Linux: `missing` không phải lỗi.
#[cfg(target_os = "linux")]
const DOCTOR_EMPTY_EXIT_CODE: i32 = 0;

/// Thư mục chỉ chứa các trình quản lý file `present` (file giả). WHY: kết quả tích hợp phụ thuộc trình
/// quản lý file có trên máy; máy chạy test không được làm đổi kết quả.
fn file_manager_path(sandbox: &Path, present: &[&str]) -> std::path::PathBuf {
    let fake = sandbox.join("file-managers");
    fs::create_dir_all(&fake).unwrap();
    for name in present {
        let program = fake.join(name);
        // WHY in phiên bản: accel Nautilus đọc `nautilus --version`; máy chạy test (CI) có thể không có Nautilus.
        fs::write(&program, "#!/bin/sh\necho 'GNOME nautilus 46.4'\n").unwrap();
        fs::set_permissions(
            &program,
            std::os::unix::fs::PermissionsExt::from_mode(0o755),
        )
        .unwrap();
    }
    fake
}

/// Chạy CLI với HOME và XDG nằm trong `sandbox`, để test không đọc/ghi `~/.local/share` hay `~/.config` thật.
fn sandboxed_morphix(sandbox: &Path) -> assert_cmd::Command {
    let gui = sandbox.join("morphix-gui");
    fs::write(&gui, b"bin").unwrap();
    let mut command = cargo_bin_cmd!("morphix");
    let file_managers = file_manager_path(sandbox, &["nautilus", "dolphin"]);
    let path = std::env::join_paths(std::iter::once(file_managers.clone()).chain(
        std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()),
    ))
    .unwrap();
    command
        .env("MORPHIX_FILE_MANAGER_PATH", &file_managers)
        .env("PATH", path);
    // WHY: sesion GNOME của máy chạy test không được rò vào kết quả doctor, nên tray luôn không áp dụng.
    command
        .env("HOME", sandbox.join("home"))
        .env("XDG_DATA_HOME", sandbox.join("xdg-data"))
        .env("XDG_CONFIG_HOME", sandbox.join("xdg-config"))
        .env("MORPHIX_GUI_BIN", gui)
        .env_remove("XDG_CURRENT_DESKTOP")
        .env_remove("DBUS_SESSION_BUS_ADDRESS");
    command
}

#[cfg(not(any(target_os = "linux", target_os = "windows")))]
#[test]
fn install_integration_json_reports_components_and_exits_1_on_unsupported_platform() {
    let data = tempdir().unwrap();
    let assert = cargo_bin_cmd!("morphix")
        .env("MORPHIX_DATA_DIR", data.path())
        .args(["install-integration", "--json"])
        .assert()
        .code(1);
    let json = stdout_json(&assert);
    assert_eq!(json["overall"], "failure");
    let components = json["components"].as_array().unwrap();
    assert_eq!(components.len(), 6);
    assert!(components.iter().all(|c| c["status"] == "unsupported"));
}

#[cfg(target_os = "linux")]
#[test]
fn install_integration_json_installs_linux_components_into_sandboxed_roots() {
    let sandbox = tempdir().unwrap();
    let assert = sandboxed_morphix(sandbox.path())
        .env("MORPHIX_DATA_DIR", sandbox.path().join("data"))
        .args(["install-integration", "--json"])
        .assert()
        .code(0);
    let json = stdout_json(&assert);
    assert_eq!(json["overall"], "success");
    let statuses: Vec<Value> = json["components"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["status"].clone())
        .collect();
    // WHY thứ tự cố định: đây là thứ tự báo cáo trong INSTALLABLE_COMPONENTS, script đọc JSON dựa vào đó.
    assert_eq!(
        statuses,
        [
            "not_applicable",
            "installed",
            "installed",
            "not_applicable",
            "installed",
            "installed",
            "installed",
            "installed",
            "not_applicable",
            "not_applicable",
            "not_applicable"
        ]
    );
    assert!(sandbox
        .path()
        .join("xdg-data/nautilus/scripts/Morphix")
        .is_file());
}

#[cfg(target_os = "linux")]
#[test]
fn no_autostart_flag_drops_autostart_in_the_cli_output() {
    let sandbox = tempdir().unwrap();
    let assert = sandboxed_morphix(sandbox.path())
        .env("MORPHIX_DATA_DIR", sandbox.path().join("data"))
        .args(["install-integration", "--no-autostart", "--json"])
        .assert()
        .code(INSTALL_EXIT_CODE);
    let ids: Vec<Value> = stdout_json(&assert)["components"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["id"].clone())
        .collect();
    assert!(!ids.contains(&json!("autostart")), "{ids:?}");
    assert_eq!(ids.len(), 10);
}

#[cfg(target_os = "linux")]
#[test]
fn dry_run_and_uninstall_json_parse_as_one_object() {
    let sandbox = tempdir().unwrap();
    for args in [
        vec!["install-integration", "--dry-run", "--json"],
        vec!["uninstall-integration", "--json"],
    ] {
        let assert = sandboxed_morphix(sandbox.path())
            .env("MORPHIX_DATA_DIR", sandbox.path().join("data"))
            .args(&args)
            .assert()
            .code(INSTALL_EXIT_CODE);
        assert_eq!(stdout_json(&assert)["platform"], std::env::consts::OS);
    }
}

#[test]
fn usage_errors_exit_2_and_never_print_success() {
    for args in [
        vec!["install-integration", "--bogus"],
        vec!["install-integration", "--desktop"],
        vec!["uninstall-integration", "--no-autostart"],
        vec!["doctor", "extra"],
    ] {
        cargo_bin_cmd!("morphix")
            .args(&args)
            .assert()
            .code(2)
            .stdout("");
    }
}

#[cfg(target_os = "linux")]
#[test]
fn doctor_json_keeps_ffmpeg_and_data_dir_and_adds_integration_status() {
    let sandbox = tempdir().unwrap();
    let data = sandbox.path().join("data");
    let assert = sandboxed_morphix(sandbox.path())
        .env("MORPHIX_DATA_DIR", &data)
        .env("PATH", "")
        .args(["doctor", "--json"])
        .assert()
        .code(DOCTOR_EMPTY_EXIT_CODE);
    let json = stdout_json(&assert);
    assert!(json["ffmpeg"].is_null(), "{json}");
    assert_eq!(json["data_dir_writable"], true);
    assert!(json["data_dir"].as_str().is_some());
    assert!(json["log"].as_str().unwrap().ends_with("morphix.log"));
    assert_eq!(json["integration"]["overall"], "success");
    let components = json["integration"]["components"].as_array().unwrap();
    assert_eq!(components.len(), 14);
    assert!(!data.join("morphix.log").exists());
}

#[cfg(target_os = "linux")]
#[test]
fn dry_run_writes_nothing_to_the_data_dir() {
    let sandbox = tempdir().unwrap();
    let data = sandbox.path().join("data");
    fs::create_dir_all(&data).unwrap();
    sandboxed_morphix(sandbox.path())
        .env("MORPHIX_DATA_DIR", &data)
        .args(["install-integration", "--dry-run", "--json"])
        .assert()
        .code(INSTALL_EXIT_CODE);
    assert_eq!(fs::read_dir(&data).unwrap().count(), 0);
    assert!(
        !sandbox.path().join("xdg-data").exists(),
        "dry-run không được ghi XDG"
    );
}

#[test]
fn doctor_json_reports_data_dir_failure_with_exit_1() {
    let sandbox = tempdir().unwrap();
    let blocked = sandbox.path().join("plain-file");
    fs::write(&blocked, b"x").unwrap();
    let assert = sandboxed_morphix(sandbox.path())
        .env("MORPHIX_DATA_DIR", &blocked)
        .args(["doctor", "--json"])
        .assert()
        .code(1);
    assert_eq!(stdout_json(&assert)["data_dir_writable"], false);
}

// ---- Linux: XDG roots, Nautilus script, Dolphin service menu, autostart ----

/// Đường dẫn có khoảng trắng, dấu nháy kép, `&` và dấu nháy đơn: mọi escape phải đúng với bộ ký tự này.
const AWKWARD_DIR: &str = r#"/Apps "v1" & it's/Morphix/"#;

fn linux_at(root: &Path) -> LinuxBackend {
    linux_with(root, no_session(), scripted_tools(Vec::new()))
}

fn linux_with(root: &Path, session: SessionEnv, tools: Box<ToolRunner>) -> LinuxBackend {
    LinuxBackend::new(
        XdgRoots {
            data_home: root.join("data"),
            config_home: root.join("config"),
        },
        session,
        tools,
    )
}

fn nautilus_path(root: &Path) -> PathBuf {
    root.join("data/nautilus/scripts/Morphix")
}

#[cfg(unix)]
fn dolphin_path(root: &Path) -> PathBuf {
    root.join("data/kio/servicemenus/morphix.desktop")
}

#[cfg(unix)]
fn autostart_path(root: &Path) -> PathBuf {
    root.join("config/autostart/morphix.desktop")
}

#[test]
fn xdg_roots_default_under_home_when_variables_are_unset() {
    let roots = resolve_xdg_roots(None, None, Some(OsStr::new("/home/u"))).unwrap();
    assert_eq!(
        roots,
        XdgRoots {
            data_home: "/home/u/.local/share".into(),
            config_home: "/home/u/.config".into(),
        }
    );
}

#[test]
fn xdg_variables_override_the_defaults_when_absolute() {
    let roots = resolve_xdg_roots(
        Some(OsStr::new("/x/data")),
        Some(OsStr::new("/x/conf")),
        Some(OsStr::new("/home/u")),
    )
    .unwrap();
    assert_eq!(
        roots,
        XdgRoots {
            data_home: "/x/data".into(),
            config_home: "/x/conf".into(),
        }
    );
}

#[test]
fn relative_or_empty_xdg_values_are_ignored_in_favour_of_home() {
    let roots = resolve_xdg_roots(
        Some(OsStr::new("relative/data")),
        Some(OsStr::new("")),
        Some(OsStr::new("/home/u")),
    )
    .unwrap();
    assert_eq!(roots.data_home, PathBuf::from("/home/u/.local/share"));
    assert_eq!(roots.config_home, PathBuf::from("/home/u/.config"));
}

#[test]
fn missing_home_without_overrides_is_an_error_not_a_panic() {
    assert!(resolve_xdg_roots(None, None, None).is_err());
    // WHY lỗi: XDG_DATA_HOME đặt không cứu được autostart, vì config vẫn cần HOME.
    assert!(resolve_xdg_roots(Some(OsStr::new("/d")), None, None).is_err());
}

#[test]
fn nautilus_script_execs_the_absolute_binary_with_posix_quoting_and_no_input_interpolation() {
    let morphix = PathBuf::from(format!("{AWKWARD_DIR}morphix"));
    let body = nautilus_script_body(&morphix).unwrap();
    assert!(
        body.starts_with("#!/bin/sh\n# Managed by Morphix\n"),
        "{body}"
    );
    assert!(
        body.contains("\nexec '/Apps \"v1\" & it'\\''s/Morphix/morphix' menu -- \"$@\"\n"),
        "{body}"
    );
    assert!(!body.contains("eval"), "{body}");
    assert!(!body.contains("sh -c"), "{body}");
}

#[cfg(unix)]
#[test]
fn installed_nautilus_script_forwards_selected_paths_as_untouched_argv() {
    let f = tempdir().unwrap();
    let fake_dir = f.path().join("Apps \"v1\" & it's");
    fs::create_dir_all(&fake_dir).unwrap();
    let fake = fake_dir.join("morphix");
    let argv_out = f.path().join("argv.bin");
    fs::write(
        &fake,
        "#!/bin/sh\nprintf '%s\\0' \"$@\" > \"$MORPHIX_ARGV_OUT\"\n",
    )
    .unwrap();
    set_mode(&fake, 0o755);
    let binaries = BinaryPaths {
        morphix: fake.clone(),
        gui: fake.clone(),
    };
    let root = f.path().join("root");
    let backend = linux_at(&root);

    let report = backend.install(ComponentId::NautilusScript, &binaries);
    assert_eq!(
        report.status,
        ComponentStatus::Installed,
        "{}",
        report.message
    );
    let script = nautilus_path(&root);
    assert_eq!(mode_of(&script), 0o755);

    let output = Command::new(&script)
        .env("MORPHIX_ARGV_OUT", &argv_out)
        .args(["a b.png", "it's.jpg", "-leading.png", "x\nLF.jpg"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let argv: Vec<String> = fs::read(&argv_out)
        .unwrap()
        .split(|byte| *byte == 0)
        .filter(|part| !part.is_empty())
        .map(|part| String::from_utf8_lossy(part).into_owned())
        .collect();
    assert_eq!(
        argv,
        [
            "menu",
            "--",
            "a b.png",
            "it's.jpg",
            "-leading.png",
            "x\nLF.jpg"
        ]
    );

    // WHY kiểm lựa chọn rỗng: nếu không có file cục bộ, script phải báo lỗi chứ không gửi yêu cầu rỗng.
    fs::remove_file(&argv_out).unwrap();
    let empty = Command::new(&script)
        .env("MORPHIX_ARGV_OUT", &argv_out)
        .output()
        .unwrap();
    assert_eq!(empty.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&empty.stderr).contains("không có file cục bộ"));
    assert!(
        !argv_out.exists(),
        "morphix không được gọi khi không có file"
    );
}

#[test]
fn dolphin_service_menu_declares_konq_popup_service_type_exactly_once() {
    let body = dolphin_service_menu_body(Path::new("/opt/morphix/morphix")).unwrap();
    let service_types: Vec<&str> = body
        .lines()
        .filter_map(|line| line.strip_prefix("ServiceTypes="))
        .collect();
    assert_eq!(service_types, vec!["KonqPopupMenu/Plugin"]);
}

#[test]
fn dolphin_service_menu_passes_uri_placeholder_with_desktop_entry_escaping() {
    let morphix = PathBuf::from(format!("{AWKWARD_DIR}morphix"));
    let expected = concat!(
        "# Managed by Morphix\n",
        "[Desktop Entry]\n",
        "Type=Service\n",
        "Name=Morphix\n",
        "MimeType=all/allfiles;\n",
        "Actions=Morphix;\n",
        "ServiceTypes=KonqPopupMenu/Plugin\n",
        "\n",
        "[Desktop Action Morphix]\n",
        "Name=Morphix\n",
        "Exec=\"/Apps \\\\\"v1\\\\\" & it's/Morphix/morphix\" menu -- %F\n",
    );
    assert_eq!(dolphin_service_menu_body(&morphix).unwrap(), expected);
}

#[test]
fn autostart_entry_has_required_keys_and_escaped_exec_for_the_gui() {
    let gui = PathBuf::from(format!("{AWKWARD_DIR}morphix-gui"));
    let expected = concat!(
        "# Managed by Morphix\n",
        "[Desktop Entry]\n",
        "Type=Application\n",
        "Name=Morphix\n",
        "Exec=\"/Apps \\\\\"v1\\\\\" & it's/Morphix/morphix-gui\"\n",
        "TryExec=/Apps \"v1\" & it's/Morphix/morphix-gui\n",
        "Terminal=false\n",
    );
    assert_eq!(autostart_entry_body(&gui).unwrap(), expected);
}

#[test]
fn desktop_entry_values_escape_backslash_newline_tab_and_return() {
    assert_eq!(escape_desktop_value("a\\b\nc\td\re"), "a\\\\b\\nc\\td\\re");
}

#[test]
fn desktop_exec_arguments_quote_reserved_characters_and_double_percent() {
    assert_eq!(
        desktop_exec_argument("/usr/bin/morphix"),
        "/usr/bin/morphix"
    );
    assert_eq!(desktop_exec_argument("a b"), "\"a b\"");
    assert_eq!(desktop_exec_argument("50%"), "50%%");
    assert_eq!(desktop_exec_argument("x\"$`\\y"), "\"x\\\"\\$\\`\\\\y\"");
}

#[cfg(unix)]
#[test]
fn linux_file_modes_are_0755_for_scripts_and_menus_and_0644_for_autostart() {
    let f = tempdir().unwrap();
    let root = f.path().join("root");
    let binaries = binaries_in(f.path());
    let backend = linux_at(&root);
    for id in [
        ComponentId::NautilusScript,
        ComponentId::DolphinServiceMenu,
        ComponentId::Autostart,
    ] {
        assert_eq!(
            backend.install(id, &binaries).status,
            ComponentStatus::Installed
        );
    }
    assert_eq!(mode_of(&nautilus_path(&root)), 0o755);
    assert_eq!(mode_of(&dolphin_path(&root)), 0o755);
    assert_eq!(mode_of(&autostart_path(&root)), 0o644);
}

#[test]
fn installing_and_uninstalling_linux_components_twice_is_idempotent_and_leaves_no_directories() {
    let f = tempdir().unwrap();
    let root = f.path().join("root");
    let binaries = binaries_in(f.path());
    let backend = linux_at(&root);
    for id in [
        ComponentId::NautilusScript,
        ComponentId::DolphinServiceMenu,
        ComponentId::Autostart,
    ] {
        assert_eq!(
            backend.install(id, &binaries).status,
            ComponentStatus::Installed
        );
        assert_eq!(
            backend.install(id, &binaries).status,
            ComponentStatus::AlreadyInstalled
        );
        assert_eq!(backend.uninstall(id).status, ComponentStatus::Removed);
        assert_eq!(backend.uninstall(id).status, ComponentStatus::AlreadyAbsent);
    }
    // WHY: mọi thư mục do lần cài tạo ra đều có marker nên gỡ xong không để lại gì, kể cả `root`.
    assert!(!root.exists());
}

#[test]
fn moved_binary_is_stale_until_reinstalled_on_linux() {
    let f = tempdir().unwrap();
    let root = f.path().join("root");
    let backend = linux_at(&root);
    fs::create_dir_all(f.path().join("first")).unwrap();
    fs::create_dir_all(f.path().join("second")).unwrap();
    let first = binaries_in(&f.path().join("first"));
    let second = binaries_in(&f.path().join("second"));
    assert_eq!(
        backend.install(ComponentId::NautilusScript, &first).status,
        ComponentStatus::Installed
    );
    assert_eq!(
        backend.inspect(ComponentId::NautilusScript, &second).status,
        ComponentStatus::Stale
    );
    assert_eq!(
        backend.install(ComponentId::NautilusScript, &second).status,
        ComponentStatus::Installed
    );
    assert_eq!(
        backend.inspect(ComponentId::NautilusScript, &second).status,
        ComponentStatus::Installed
    );
}

#[test]
fn pre_existing_empty_scripts_directory_survives_uninstall() {
    let f = tempdir().unwrap();
    let root = f.path().join("root");
    let scripts = root.join("data/nautilus/scripts");
    fs::create_dir_all(&scripts).unwrap();
    let binaries = binaries_in(f.path());
    let backend = linux_at(&root);

    assert_eq!(
        backend
            .install(ComponentId::NautilusScript, &binaries)
            .status,
        ComponentStatus::Installed
    );
    assert_eq!(
        backend.uninstall(ComponentId::NautilusScript).status,
        ComponentStatus::Removed
    );
    assert!(
        scripts.is_dir(),
        "thư mục có sẵn không do Morphix tạo phải được giữ"
    );
    assert_eq!(fs::read_dir(&scripts).unwrap().count(), 0);
}

#[test]
fn app_created_empty_scripts_directory_is_removed_together_with_its_created_parents() {
    let f = tempdir().unwrap();
    let root = f.path().join("root");
    let binaries = binaries_in(f.path());
    let backend = linux_at(&root);

    assert_eq!(
        backend
            .install(ComponentId::NautilusScript, &binaries)
            .status,
        ComponentStatus::Installed
    );
    assert_eq!(
        backend.uninstall(ComponentId::NautilusScript).status,
        ComponentStatus::Removed
    );
    assert!(!root.join("data/nautilus").exists());
    assert!(!root.exists());
}

#[test]
fn foreign_script_at_the_target_is_kept_and_nothing_else_is_written() {
    let f = tempdir().unwrap();
    let root = f.path().join("root");
    let path = nautilus_path(&root);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, "echo người dùng\n".as_bytes()).unwrap();
    let binaries = binaries_in(f.path());
    let backend = linux_at(&root);

    let report = backend.install(ComponentId::NautilusScript, &binaries);
    assert_eq!(report.status, ComponentStatus::Conflict);
    assert_eq!(fs::read(&path).unwrap(), "echo người dùng\n".as_bytes());
    assert_eq!(
        snapshot(&root).len(),
        1,
        "conflict không được tạo file marker hay file tạm"
    );
    assert_eq!(
        backend.uninstall(ComponentId::NautilusScript).status,
        ComponentStatus::Conflict
    );
    assert!(path.is_file());
}

#[test]
fn linux_backend_reports_windows_verb_as_not_applicable_and_tray_outside_gnome_as_not_applicable() {
    let f = tempdir().unwrap();
    let backend = linux_at(&f.path().join("root"));
    let binaries = binaries_in(f.path());
    assert_eq!(
        backend
            .install(ComponentId::WindowsContextMenu, &binaries)
            .status,
        ComponentStatus::NotApplicable
    );
    assert_eq!(
        backend.inspect(ComponentId::TraySupport, &binaries).status,
        ComponentStatus::NotApplicable
    );
}

#[test]
fn linux_roots_without_home_report_each_linux_component_as_failed() {
    let backend = LinuxBackend::from_resolution(
        Err("không xác định được thư mục người dùng".into()),
        no_session(),
        scripted_tools(Vec::new()),
    );
    let f = tempdir().unwrap();
    let binaries = binaries_in(f.path());
    let report = integration::install(
        &backend,
        &binaries,
        &IntegrationOptions {
            dry_run: false,
            install_autostart: true,
        },
    );
    assert_eq!(report.overall, OverallStatus::Failure);
    assert_eq!(
        status_of(&report, ComponentId::NautilusScript),
        ComponentStatus::Failed
    );
}

// ---- Doctor: trạng thái tích hợp chỉ đọc, không đoán khi thiếu dữ liệu ----

const APPINDICATOR_ENABLED: &str =
    "x11gestures@joseexposito.github.io\nubuntu-appindicators@ubuntu.com\n";
const APPINDICATOR_ABSENT: &str = "x11gestures@joseexposito.github.io\n";

#[cfg(target_os = "linux")]
const DING_SOURCE_MANIFEST: &str = include_str!("../../../integration/ding/source.json");

fn gnome_session() -> SessionEnv {
    SessionEnv {
        current_desktop: Some("ubuntu:GNOME".to_string()),
        has_session_bus: true,
        ..Default::default()
    }
}

fn no_session() -> SessionEnv {
    SessionEnv {
        current_desktop: None,
        has_session_bus: false,
        ..Default::default()
    }
}

fn exited(code: i32, stdout: &str, stderr: &str) -> ToolOutcome {
    ToolOutcome::Exited {
        code: Some(code),
        stdout: stdout.to_string(),
        stderr: stderr.to_string(),
    }
}

/// Công cụ giả theo tên chương trình; chương trình không có trong bảng coi như chưa cài.
fn scripted_tools(answers: Vec<(&'static str, ToolOutcome)>) -> Box<ToolRunner> {
    Box::new(move |program: &str, _args: &[&OsStr]| {
        answers
            .iter()
            .find(|(name, _)| *name == program)
            .map_or(ToolOutcome::NotFound, |(_, outcome)| outcome.clone())
    })
}

fn healthy_tools(listing: &'static str) -> Box<ToolRunner> {
    scripted_tools(vec![
        ("gnome-extensions", exited(0, listing, "")),
        ("desktop-file-validate", exited(0, "", "")),
    ])
}

#[cfg(target_os = "linux")]
fn ding_test_tree(root: &Path) -> PathBuf {
    let extension_root = root.join("ding@rastersoft.com");
    let manifest: Value = serde_json::from_str(DING_SOURCE_MANIFEST).unwrap();
    for relative_path in manifest["patched_files"].as_object().unwrap().keys() {
        let file = extension_root.join(relative_path);
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(&file, b"test patched file").unwrap();
        fs::write(
            PathBuf::from(format!("{}.morphix-original", file.display())),
            b"test original file",
        )
        .unwrap();
    }
    extension_root
}

#[cfg(target_os = "linux")]
fn ding_tools(
    extension_root: &Path,
    package_version: Option<&str>,
    diversion_owner: Option<&str>,
    diversion_suffix: Option<&str>,
    hash_mismatch: Option<&Path>,
) -> Box<ToolRunner> {
    let manifest: Value = serde_json::from_str(DING_SOURCE_MANIFEST).unwrap();
    let source_hashes = manifest["source_files"].as_object().unwrap().clone();
    let patched_hashes = manifest["patched_files"].as_object().unwrap().clone();
    let extension_root = extension_root.to_path_buf();
    let package_version = package_version.map(str::to_string);
    let diversion_owner = diversion_owner.map(str::to_string);
    let diversion_suffix = diversion_suffix.map(str::to_string);
    let hash_mismatch = hash_mismatch.map(Path::to_path_buf);

    Box::new(move |program: &str, raw_args: &[&OsStr]| {
        let args: Vec<String> = raw_args
            .iter()
            .map(|argument| argument.to_string_lossy().into_owned())
            .collect();
        if program == "dpkg-query" && args.first().is_some_and(|argument| argument == "-W") {
            return match &package_version {
                Some(version) => exited(0, version, ""),
                None => exited(1, "", "package is not installed"),
            };
        }
        if program == "dpkg-divert"
            && args
                .first()
                .is_some_and(|argument| argument == "--listpackage")
        {
            return match &diversion_owner {
                Some(owner) => exited(0, owner, ""),
                None => exited(0, "", ""),
            };
        }
        if program == "dpkg-divert"
            && args
                .first()
                .is_some_and(|argument| argument == "--truename")
        {
            let target = PathBuf::from(args.get(1).expect("--truename phải có file"));
            return match &diversion_suffix {
                Some(suffix) => exited(0, &format!("{}{suffix}", target.display()), ""),
                None => exited(0, &target.to_string_lossy(), ""),
            };
        }
        if program == "sha256sum" {
            let file = PathBuf::from(args.first().expect("sha256sum phải có file"));
            if hash_mismatch.as_deref() == Some(file.as_path()) {
                return exited(0, &format!("{}  {}", "0".repeat(64), file.display()), "");
            }
            let Ok(relative_path) = file.strip_prefix(&extension_root) else {
                return exited(1, "", "file không thuộc extension root");
            };
            let relative = relative_path.to_string_lossy();
            let (hashes, key) = match relative.strip_suffix(".morphix-original") {
                Some(source_path) => (&source_hashes, source_path),
                None if diversion_suffix.is_none() => (&source_hashes, relative.as_ref()),
                None => (&patched_hashes, relative.as_ref()),
            };
            return match hashes.get(key) {
                Some(hash) => exited(
                    0,
                    &format!("{}  {}", hash.as_str().unwrap(), file.display()),
                    "",
                ),
                None => exited(1, "", "không có hash cho file"),
            };
        }
        ToolOutcome::NotFound
    })
}

#[cfg(target_os = "linux")]
#[test]
fn desktop_context_menu_is_installed_only_when_version_diversions_and_hashes_match() {
    let sandbox = tempdir().unwrap();
    let extension_root = ding_test_tree(sandbox.path());
    let report = morphix_cli::integration::desktop::inspect_desktop(
        &extension_root,
        &*ding_tools(
            &extension_root,
            Some("46+really47.0.9-1ubuntu5"),
            Some("morphix"),
            Some(".morphix-original"),
            None,
        ),
    );

    assert_eq!(report.id, ComponentId::DesktopContextMenu);
    assert_eq!(report.status, ComponentStatus::Installed);
}

#[cfg(target_os = "linux")]
#[test]
fn desktop_context_menu_reports_missing_or_stale_ding_package() {
    let sandbox = tempdir().unwrap();
    let extension_root = ding_test_tree(sandbox.path());
    let missing = morphix_cli::integration::desktop::inspect_desktop(
        &extension_root,
        &*ding_tools(&extension_root, None, None, None, None),
    );
    assert_eq!(missing.status, ComponentStatus::Missing);

    let stale = morphix_cli::integration::desktop::inspect_desktop(
        &extension_root,
        &*ding_tools(
            &extension_root,
            Some("46+really47.0.9-1ubuntu4"),
            Some("morphix"),
            Some(".morphix-original"),
            None,
        ),
    );
    assert_eq!(stale.status, ComponentStatus::Stale);
}

#[cfg(target_os = "linux")]
#[test]
fn desktop_context_menu_reports_package_manager_failures() {
    let sandbox = tempdir().unwrap();
    let extension_root = ding_test_tree(sandbox.path());
    let tools: Box<ToolRunner> = Box::new(|program: &str, _args: &[&OsStr]| {
        if program == "dpkg-query" {
            ToolOutcome::Failed("dpkg database is unreadable".to_string())
        } else {
            ToolOutcome::NotFound
        }
    });
    let report = morphix_cli::integration::desktop::inspect_desktop(&extension_root, &*tools);

    assert_eq!(report.status, ComponentStatus::Failed);
    assert!(report.message.contains("dpkg database is unreadable"));
}

#[cfg(target_os = "linux")]
#[test]
fn desktop_context_menu_rejects_a_symlinked_ding_app_directory() {
    let sandbox = tempdir().unwrap();
    let extension_root = ding_test_tree(sandbox.path());
    let app_path = extension_root.join("app");
    let real_app_path = extension_root.join("app.real");
    fs::rename(&app_path, &real_app_path).unwrap();
    std::os::unix::fs::symlink(&real_app_path, &app_path).unwrap();
    let report = morphix_cli::integration::desktop::inspect_desktop(
        &extension_root,
        &*ding_tools(
            &extension_root,
            Some("46+really47.0.9-1ubuntu5"),
            Some("morphix"),
            Some(".morphix-original"),
            None,
        ),
    );

    assert_eq!(report.status, ComponentStatus::Conflict);
}

#[cfg(target_os = "linux")]
#[test]
fn desktop_context_menu_reports_missing_and_foreign_diversions() {
    let sandbox = tempdir().unwrap();
    let extension_root = ding_test_tree(sandbox.path());
    let no_diversion = morphix_cli::integration::desktop::inspect_desktop(
        &extension_root,
        &*ding_tools(
            &extension_root,
            Some("46+really47.0.9-1ubuntu5"),
            None,
            None,
            None,
        ),
    );
    assert_eq!(no_diversion.status, ComponentStatus::Missing);

    let changed_file = extension_root.join("app/fileItemMenu.js");
    let untracked_patch = morphix_cli::integration::desktop::inspect_desktop(
        &extension_root,
        &*ding_tools(
            &extension_root,
            Some("46+really47.0.9-1ubuntu5"),
            None,
            None,
            Some(&changed_file),
        ),
    );
    assert_eq!(untracked_patch.status, ComponentStatus::Conflict);

    let foreign_diversion = morphix_cli::integration::desktop::inspect_desktop(
        &extension_root,
        &*ding_tools(
            &extension_root,
            Some("46+really47.0.9-1ubuntu5"),
            Some("another-package"),
            Some(".original"),
            None,
        ),
    );
    assert_eq!(foreign_diversion.status, ComponentStatus::Conflict);
}

#[cfg(target_os = "linux")]
#[test]
fn desktop_context_menu_reports_diversion_path_and_file_hash_conflicts() {
    let sandbox = tempdir().unwrap();
    let extension_root = ding_test_tree(sandbox.path());
    let wrong_target = morphix_cli::integration::desktop::inspect_desktop(
        &extension_root,
        &*ding_tools(
            &extension_root,
            Some("46+really47.0.9-1ubuntu5"),
            Some("morphix"),
            Some(".wrong-original"),
            None,
        ),
    );
    assert_eq!(wrong_target.status, ComponentStatus::Conflict);

    let changed_patch = extension_root.join("app/fileItemMenu.js");
    let changed_patch_report = morphix_cli::integration::desktop::inspect_desktop(
        &extension_root,
        &*ding_tools(
            &extension_root,
            Some("46+really47.0.9-1ubuntu5"),
            Some("morphix"),
            Some(".morphix-original"),
            Some(&changed_patch),
        ),
    );
    assert_eq!(changed_patch_report.status, ComponentStatus::Conflict);

    let changed_source = PathBuf::from(format!(
        "{}.morphix-original",
        extension_root.join("app/fileItemMenu.js").display()
    ));
    let changed_source_report = morphix_cli::integration::desktop::inspect_desktop(
        &extension_root,
        &*ding_tools(
            &extension_root,
            Some("46+really47.0.9-1ubuntu5"),
            Some("morphix"),
            Some(".morphix-original"),
            Some(&changed_source),
        ),
    );
    assert_eq!(changed_source_report.status, ComponentStatus::Conflict);
}

#[cfg(target_os = "linux")]
#[test]
fn package_owned_files_report_installation_and_unmanaged_conflicts() {
    use morphix_cli::integration::desktop::inspect_package_file;

    let sandbox = tempdir().unwrap();
    let path = sandbox.path().join("morphix.desktop");
    fs::write(&path, b"[Desktop Entry]\n").unwrap();
    let owned_path = path.clone();
    let owned_tools: Box<ToolRunner> = Box::new(move |program: &str, args: &[&OsStr]| {
        if program == "dpkg-query"
            && args
                .first()
                .is_some_and(|argument| *argument == OsStr::new("-S"))
        {
            return exited(0, &format!("morphix: {}", owned_path.display()), "");
        }
        ToolOutcome::NotFound
    });
    assert_eq!(
        inspect_package_file(ComponentId::ApplicationLauncher, &path, &*owned_tools).status,
        ComponentStatus::Installed
    );

    let foreign_path = path.clone();
    let foreign_tools: Box<ToolRunner> = Box::new(move |program: &str, args: &[&OsStr]| {
        if program == "dpkg-query"
            && args
                .first()
                .is_some_and(|argument| *argument == OsStr::new("-S"))
        {
            return exited(0, &format!("other-package: {}", foreign_path.display()), "");
        }
        ToolOutcome::NotFound
    });
    assert_eq!(
        inspect_package_file(ComponentId::ApplicationLauncher, &path, &*foreign_tools).status,
        ComponentStatus::Conflict
    );

    let missing = sandbox.path().join("missing.desktop");
    let missing_tools: Box<ToolRunner> = Box::new(|program: &str, _args: &[&OsStr]| {
        if program == "dpkg-query" {
            exited(1, "", "no package owns this file")
        } else {
            ToolOutcome::NotFound
        }
    });
    assert_eq!(
        inspect_package_file(ComponentId::ApplicationLauncher, &missing, &*missing_tools).status,
        ComponentStatus::Missing
    );
}

#[cfg(target_os = "linux")]
#[test]
fn doctor_reports_installed_entries_valid_desktop_files_and_appindicator_as_installed() {
    let f = tempdir().unwrap();
    let binaries = binaries_in(f.path());
    let backend = linux_with(
        &f.path().join("root"),
        gnome_session(),
        healthy_tools(APPINDICATOR_ENABLED),
    );
    integration::install(
        &backend,
        &binaries,
        &IntegrationOptions {
            dry_run: false,
            install_autostart: true,
        },
    );
    let report = integration::inspect(&backend, &binaries);
    for id in [
        ComponentId::NautilusScript,
        ComponentId::DolphinServiceMenu,
        ComponentId::Autostart,
        ComponentId::DesktopValidation,
        ComponentId::TraySupport,
    ] {
        assert_eq!(status_of(&report, id), ComponentStatus::Installed, "{id:?}");
    }
    assert_eq!(doctor_exit_code(&report.components), 0);
}

#[test]
fn doctor_reports_missing_entries_and_disabled_appindicator_as_missing() {
    let f = tempdir().unwrap();
    let binaries = binaries_in(f.path());
    let backend = linux_with(
        &f.path().join("root"),
        gnome_session(),
        healthy_tools(APPINDICATOR_ABSENT),
    );
    let report = integration::inspect(&backend, &binaries);
    for id in [
        ComponentId::NautilusScript,
        ComponentId::DolphinServiceMenu,
        ComponentId::Autostart,
        ComponentId::DesktopValidation,
        ComponentId::TraySupport,
    ] {
        assert_eq!(status_of(&report, id), ComponentStatus::Missing, "{id:?}");
    }
    // WHY kiểm nội dung: người dùng GNOME Fedora/Debian cần biết gói nào để bật icon khay.
    let tray = report
        .components
        .iter()
        .find(|c| c.id == ComponentId::TraySupport)
        .unwrap();
    assert!(
        tray.message.contains("gnome-shell-extension-appindicator"),
        "{}",
        tray.message
    );
    assert_eq!(doctor_exit_code(&report.components), 0);
}

// WHY unix: test này đổi exec bit bằng set_mode, mà chỉ unix có quyền file kiểu đó.
#[cfg(unix)]
#[test]
fn doctor_reports_stale_when_a_managed_script_lost_its_exec_bit() {
    let f = tempdir().unwrap();
    let binaries = binaries_in(f.path());
    let root = f.path().join("root");
    let backend = linux_with(&root, gnome_session(), healthy_tools(APPINDICATOR_ENABLED));
    integration::install(
        &backend,
        &binaries,
        &IntegrationOptions {
            dry_run: false,
            install_autostart: true,
        },
    );
    set_mode(&nautilus_path(&root), 0o644);
    let report = integration::inspect(&backend, &binaries);
    let nautilus = report
        .components
        .iter()
        .find(|c| c.id == ComponentId::NautilusScript)
        .unwrap();
    assert_eq!(nautilus.status, ComponentStatus::Stale);
    assert!(nautilus.message.contains("quyền"), "{}", nautilus.message);
    assert_eq!(doctor_exit_code(&report.components), 1);
}

#[test]
fn doctor_reports_conflict_for_a_foreign_file_and_leaves_its_bytes_alone() {
    let f = tempdir().unwrap();
    let binaries = binaries_in(f.path());
    let root = f.path().join("root");
    let foreign = b"#!/bin/sh\necho nguoi dung\n";
    fs::create_dir_all(nautilus_path(&root).parent().unwrap()).unwrap();
    fs::write(nautilus_path(&root), foreign).unwrap();
    let backend = linux_with(&root, gnome_session(), healthy_tools(APPINDICATOR_ENABLED));
    let report = integration::inspect(&backend, &binaries);
    assert_eq!(
        status_of(&report, ComponentId::NautilusScript),
        ComponentStatus::Conflict
    );
    assert_eq!(fs::read(nautilus_path(&root)).unwrap(), foreign);
    assert_eq!(doctor_exit_code(&report.components), 1);
}

#[test]
fn doctor_reports_unsupported_everywhere_on_a_backend_without_linux_support() {
    let f = tempdir().unwrap();
    let binaries = binaries_in(f.path());
    let report = integration::inspect(&UnsupportedBackend, &binaries);
    assert!(report
        .components
        .iter()
        .all(|c| c.status == ComponentStatus::Unsupported));
    assert_eq!(report.overall, OverallStatus::Inconclusive);
    assert_eq!(doctor_exit_code(&report.components), 3);
}

#[test]
fn doctor_reports_unknown_for_appindicator_when_gnome_data_is_insufficient() {
    let f = tempdir().unwrap();
    let binaries = binaries_in(f.path());
    let root = f.path().join("root");
    let cases = [
        (gnome_session(), scripted_tools(Vec::new())),
        (
            SessionEnv {
                current_desktop: Some("GNOME".to_string()),
                has_session_bus: false,
                ..Default::default()
            },
            healthy_tools(APPINDICATOR_ENABLED),
        ),
        (
            gnome_session(),
            scripted_tools(vec![("gnome-extensions", exited(1, "", "boom"))]),
        ),
        (
            gnome_session(),
            scripted_tools(vec![(
                "gnome-extensions",
                ToolOutcome::Failed("quá 5 giây".to_string()),
            )]),
        ),
    ];
    for (session, tools) in cases {
        let backend = linux_with(&root, session, tools);
        let report = integration::inspect(&backend, &binaries);
        let tray = report
            .components
            .iter()
            .find(|c| c.id == ComponentId::TraySupport)
            .unwrap();
        assert_eq!(tray.status, ComponentStatus::Unknown, "{}", tray.message);
        assert_eq!(doctor_exit_code(&report.components), 3);
    }
}

#[test]
fn doctor_reports_tray_as_not_applicable_outside_gnome_and_does_not_call_gnome_tools() {
    let f = tempdir().unwrap();
    let binaries = binaries_in(f.path());
    let session = SessionEnv {
        current_desktop: Some("KDE".to_string()),
        has_session_bus: true,
        ..Default::default()
    };
    let backend = linux_with(&f.path().join("root"), session, scripted_tools(Vec::new()));
    let report = integration::inspect(&backend, &binaries);
    assert_eq!(
        status_of(&report, ComponentId::TraySupport),
        ComponentStatus::NotApplicable
    );
}

#[test]
fn missing_desktop_file_validator_is_not_applicable_and_validator_errors_follow_the_matcher() {
    let f = tempdir().unwrap();
    let binaries = binaries_in(f.path());
    let root = f.path().join("root");
    integration::install(
        &linux_with(&root, no_session(), scripted_tools(Vec::new())),
        &binaries,
        &IntegrationOptions {
            dry_run: false,
            install_autostart: true,
        },
    );
    let without = linux_with(&root, no_session(), scripted_tools(Vec::new()));
    assert_eq!(
        status_of(
            &integration::inspect(&without, &binaries),
            ComponentId::DesktopValidation
        ),
        ComponentStatus::NotApplicable
    );

    // WHY installed: chỉ lỗi khoá KDE MimeType/Actions với Type=Service (ruling Task 19) là giới hạn đã biết của validator, không phải lỗi.
    let known_restriction = "/x/morphix.desktop: error: key \"MimeType\" is present in group \"Desktop Entry\", but the type is \"Service\" while this key is only valid for type \"Application\"";
    let restricted = linux_with(
        &root,
        no_session(),
        // WHY stdout: desktop-file-validate in lỗi ra stdout, đúng như máy thật.
        scripted_tools(vec![(
            "desktop-file-validate",
            exited(1, known_restriction, ""),
        )]),
    );
    let report = integration::inspect(&restricted, &binaries);
    let validation = report
        .components
        .iter()
        .find(|c| c.id == ComponentId::DesktopValidation)
        .unwrap();
    assert_eq!(validation.status, ComponentStatus::Installed);
    assert!(
        validation.message.contains("giới hạn đã biết"),
        "{}",
        validation.message
    );
    // WHY: chính giới hạn đã biết không được kéo doctor thoát khác 0 (lý do của commit c09fff6).
    assert_eq!(doctor_exit_code(std::slice::from_ref(validation)), 0);

    // WHY failed: lỗi cú pháp không thuộc giới hạn đã biết, nên doctor phải thoát 1.
    let broken = linux_with(
        &root,
        no_session(),
        scripted_tools(vec![(
            "desktop-file-validate",
            exited(
                1,
                "/x/morphix.desktop: error: file contains line \"junk\"",
                "",
            ),
        )]),
    );
    let report = integration::inspect(&broken, &binaries);
    let validation = report
        .components
        .iter()
        .find(|c| c.id == ComponentId::DesktopValidation)
        .unwrap();
    assert_eq!(validation.status, ComponentStatus::Failed);
    assert!(
        validation.message.contains("junk"),
        "{}",
        validation.message
    );
    assert_eq!(doctor_exit_code(&report.components), 1);
}

#[cfg(target_os = "linux")]
#[test]
fn doctor_json_lists_every_integration_status_and_never_changes_files() {
    let sandbox = tempdir().unwrap();
    let data = sandbox.path().join("data");
    fs::create_dir_all(&data).unwrap();
    sandboxed_morphix(sandbox.path())
        .env("MORPHIX_DATA_DIR", &data)
        .args(["install-integration"])
        .assert()
        .code(INSTALL_EXIT_CODE);
    let before = snapshot(sandbox.path());
    let assert = sandboxed_morphix(sandbox.path())
        .env("MORPHIX_DATA_DIR", &data)
        .args(["doctor", "--json"])
        .assert();
    let json = stdout_json(&assert);
    let components = json["integration"]["components"].as_array().unwrap();
    assert_eq!(components.len(), 14, "{json}");
    for id in [
        "nautilus_extension",
        "desktop_context_menu",
        "application_launcher",
    ] {
        assert!(
            components.iter().any(|component| component["id"] == id),
            "{json}"
        );
    }
    assert_eq!(snapshot(sandbox.path()), before, "doctor không được ghi gì");
}

// ---- Windows: phần thuần (command, quote, sở hữu) chạy được trên mọi nền tảng ----

/// Chuỗi đường dẫn dưới dạng bên trong dấu nháy kép của command: mỗi `"` thành `\"`.
fn quoted_in_command(path: &Path) -> String {
    path.display().to_string().replace('"', "\\\"")
}

#[test]
fn quote_windows_arg_keeps_spaces_unicode_and_ampersand_inside_one_quoted_argument() {
    assert_eq!(
        windows::quote_windows_arg(r"C:\Program Files\Morphix\morphix.exe"),
        r#""C:\Program Files\Morphix\morphix.exe""#
    );
    assert_eq!(
        windows::quote_windows_arg(r"C:\Ứng dụng\R&D\morphix.exe"),
        r#""C:\Ứng dụng\R&D\morphix.exe""#
    );
}

#[test]
fn quote_windows_arg_escapes_embedded_quotes_and_doubles_backslashes_before_them() {
    assert_eq!(windows::quote_windows_arg(r#"a"b"#), r#""a\"b""#);
    assert_eq!(windows::quote_windows_arg(r#"a\"b"#), r#""a\\\"b""#);
}

#[test]
fn quote_windows_arg_doubles_trailing_backslashes_so_the_closing_quote_stays_literal() {
    assert_eq!(windows::quote_windows_arg(r"C:\dir\"), r#""C:\dir\\""#);
}

#[test]
fn menu_command_quotes_the_absolute_path_and_passes_only_the_explorer_placeholder() {
    let morphix = std::env::temp_dir().join("Morphix").join("morphix.exe");
    let expected = format!("\"{}\" menu \"%1\"", morphix.display());
    assert_eq!(windows::menu_command(&morphix).unwrap(), expected);
}

#[test]
fn menu_command_keeps_spaces_unicode_quotes_and_ampersand_of_the_install_path() {
    let morphix = std::env::temp_dir()
        .join("Apps \"v1\" & Ứng dụng")
        .join("morphix.exe");
    let expected = format!("\"{}\" menu \"%1\"", quoted_in_command(&morphix));
    assert_eq!(windows::menu_command(&morphix).unwrap(), expected);
}

#[test]
fn menu_command_rejects_a_relative_executable_path() {
    assert!(windows::menu_command(Path::new("morphix.exe")).is_err());
}

#[test]
fn menu_entry_is_player_model_with_the_vietnamese_name_and_the_menu_command() {
    let morphix = std::env::temp_dir().join("Morphix").join("morphix.exe");
    let entry = windows::menu_entry(&morphix).unwrap();
    assert_eq!(entry.display_name, windows::MENU_DISPLAY_NAME);
    assert_eq!(entry.multi_select_model, "Player");
    assert_eq!(entry.command, windows::menu_command(&morphix).unwrap());
}

#[test]
fn autostart_command_quotes_only_the_gui_path_and_takes_no_arguments() {
    let gui = std::env::temp_dir()
        .join("Morphix App")
        .join("morphix-gui.exe");
    let expected = format!("\"{}\"", gui.display());
    assert_eq!(windows::autostart_command(&gui).unwrap(), expected);
}

#[cfg(target_os = "linux")]
#[test]
fn default_backend_is_the_linux_backend_on_linux() {
    assert_eq!(integration::default_backend().platform(), "linux");
}

#[cfg(target_os = "windows")]
#[test]
fn default_backend_is_the_windows_backend_on_windows() {
    assert_eq!(integration::default_backend().platform(), "windows");
}

// WHY ignored thay vì bỏ hẳn: `cargo test` ghi rõ suite registry chưa chạy trên Linux, không lặng lẽ thành PASS.
#[cfg(not(target_os = "windows"))]
#[ignore = "PENDING: registry HKCU suite chỉ chạy trên Windows runner; không phải PASS trên Linux"]
#[test]
fn windows_registry_suite_is_pending_on_this_platform() {}

#[test]
fn dropping_an_uncommitted_transaction_runs_deferred_undo_steps_in_reverse() {
    use std::{cell::RefCell, rc::Rc};
    let order = Rc::new(RefCell::new(Vec::new()));
    {
        let mut tx = Transaction::new();
        let first = Rc::clone(&order);
        tx.defer(move || {
            first.borrow_mut().push("first");
            Ok(())
        });
        let second = Rc::clone(&order);
        tx.defer(move || {
            second.borrow_mut().push("second");
            Ok(())
        });
    }
    assert_eq!(*order.borrow(), vec!["second", "first"]);
}

// ---- Windows registry: chỉ biên dịch và chạy trên Windows runner; Linux không chạy suite này ----

#[cfg(target_os = "windows")]
mod windows_registry {
    use std::time::SystemTime;

    use morphix_cli::integration::windows::{
        menu_command, AUTOSTART_OWNER_VALUE, COMMAND_SUBKEY, LEGACY_VERB_KEY_PATH,
        MENU_DISPLAY_NAME, MULTI_SELECT_MODEL, OWNER_KEY_PATH, RUN_KEY_PATH, RUN_VALUE_NAME,
        VERB_KEY_PATH, VERB_OWNER_VALUE,
    };
    use morphix_cli::integration::{
        windows::WindowsBackend, BinaryPaths, ComponentId, ComponentStatus, IntegrationBackend,
    };
    use winreg::{enums::HKEY_CURRENT_USER, RegKey};

    const SHELL_PARENT: &str = r"Software\Classes\*\shell";

    /// Nhánh HKCU dành riêng cho một test. Drop xoá nhánh này kể cả khi assert lỗi (unwind).
    struct RegistrySandbox {
        path: String,
    }

    impl RegistrySandbox {
        fn new() -> Self {
            let nanos = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or_default();
            let path = format!(r"Software\MorphixTest\{}-{nanos}", std::process::id());
            RegKey::predef(HKEY_CURRENT_USER)
                .create_subkey(&path)
                .expect("tạo nhánh test HKCU");
            RegistrySandbox { path }
        }

        /// Handle ghi được tới gốc nhánh test; backend dùng handle này thay cho HKCU.
        fn root(&self) -> RegKey {
            RegKey::predef(HKEY_CURRENT_USER)
                .open_subkey_with_flags(&self.path, winreg::enums::KEY_ALL_ACCESS)
                .expect("mở nhánh test")
        }

        fn backend(&self) -> WindowsBackend {
            WindowsBackend::new(self.root())
        }

        fn read(&self, key: &str, name: &str) -> Option<String> {
            let handle = self.root().open_subkey(key).ok()?;
            handle.get_value::<String, _>(name).ok()
        }

        fn key_exists(&self, key: &str) -> bool {
            self.root().open_subkey(key).is_ok()
        }
    }

    impl Drop for RegistrySandbox {
        fn drop(&mut self) {
            // WHY ghi lỗi thay vì bỏ qua: nhánh test còn sót trên máy thật phải được người chạy biết để dọn tay.
            if let Err(error) = RegKey::predef(HKEY_CURRENT_USER).delete_subkey_all(&self.path) {
                eprintln!("không xoá được nhánh test {}: {error}", self.path);
            }
        }
    }

    fn binaries(dir_name: &str) -> BinaryPaths {
        let dir = std::env::temp_dir().join(dir_name);
        BinaryPaths {
            morphix: dir.join("morphix.exe"),
            gui: dir.join("morphix-gui.exe"),
        }
    }

    #[test]
    fn install_and_uninstall_remove_the_owned_image_only_verb_of_older_versions() {
        let sandbox = RegistrySandbox::new();
        let backend = sandbox.backend();
        let paths = binaries("Morphix Legacy");
        let command = menu_command(&paths.morphix).unwrap();
        let root = sandbox.root();
        let (legacy, _) = root
            .create_subkey(LEGACY_VERB_KEY_PATH)
            .expect("tạo verb cũ");
        legacy
            .set_value(VERB_OWNER_VALUE, &command)
            .expect("ghi marker cũ");
        legacy
            .create_subkey(COMMAND_SUBKEY)
            .expect("tạo command cũ")
            .0
            .set_value("", &command)
            .expect("ghi command cũ");

        backend.install(ComponentId::WindowsContextMenu, &paths);
        assert!(sandbox.key_exists(VERB_KEY_PATH));
        assert!(
            !sandbox.key_exists(LEGACY_VERB_KEY_PATH),
            "verb chỉ-ảnh cũ phải bị dọn"
        );
        backend.uninstall(ComponentId::WindowsContextMenu);
        assert!(!sandbox.key_exists(VERB_KEY_PATH));
    }

    #[test]
    fn install_writes_player_verb_with_menu_command_and_quoted_run_value() {
        let sandbox = RegistrySandbox::new();
        let backend = sandbox.backend();
        let paths = binaries("Morphix App");
        let verb = VERB_KEY_PATH;
        assert_eq!(
            backend
                .install(ComponentId::WindowsContextMenu, &paths)
                .status,
            ComponentStatus::Installed
        );
        assert_eq!(
            backend.install(ComponentId::Autostart, &paths).status,
            ComponentStatus::Installed
        );
        assert_eq!(
            sandbox.read(verb, "MUIVerb").as_deref(),
            Some(MENU_DISPLAY_NAME)
        );
        assert_eq!(
            sandbox.read(verb, "MultiSelectModel").as_deref(),
            Some(MULTI_SELECT_MODEL)
        );
        let expected_command = menu_command(&paths.morphix).unwrap();
        assert_eq!(
            sandbox
                .read(&format!(r"{verb}\{COMMAND_SUBKEY}"), "")
                .as_deref(),
            Some(expected_command.as_str())
        );
        assert_eq!(
            sandbox.read(verb, VERB_OWNER_VALUE).as_deref(),
            Some(expected_command.as_str())
        );
        let expected_run = format!("\"{}\"", paths.gui.display());
        assert_eq!(
            sandbox.read(RUN_KEY_PATH, RUN_VALUE_NAME).as_deref(),
            Some(expected_run.as_str())
        );
        assert_eq!(
            sandbox
                .read(OWNER_KEY_PATH, AUTOSTART_OWNER_VALUE)
                .as_deref(),
            Some(expected_run.as_str())
        );
    }

    #[test]
    fn rewritten_run_value_is_conflict_and_survives_uninstall() {
        let sandbox = RegistrySandbox::new();
        let backend = sandbox.backend();
        let paths = binaries("Morphix App");
        let rewritten = "\"C:\\Other\\other.exe\"";
        assert_eq!(
            backend.install(ComponentId::Autostart, &paths).status,
            ComponentStatus::Installed
        );
        sandbox
            .root()
            .create_subkey(RUN_KEY_PATH)
            .expect("mở Run")
            .0
            .set_value(RUN_VALUE_NAME, &rewritten.to_owned())
            .expect("ghi đè Run");

        assert_eq!(
            backend.uninstall(ComponentId::Autostart).status,
            ComponentStatus::Conflict
        );
        assert_eq!(
            backend.install(ComponentId::Autostart, &paths).status,
            ComponentStatus::Conflict
        );
        assert_eq!(
            sandbox.read(RUN_KEY_PATH, RUN_VALUE_NAME).as_deref(),
            Some(rewritten)
        );
    }

    #[test]
    fn rewritten_verb_command_is_conflict_and_survives_uninstall() {
        let sandbox = RegistrySandbox::new();
        let backend = sandbox.backend();
        let paths = binaries("Morphix App");
        let command_path = format!(r"{VERB_KEY_PATH}\{COMMAND_SUBKEY}");
        let rewritten = "\"C:\\Other\\other.exe\" \"%1\"";
        assert_eq!(
            backend
                .install(ComponentId::WindowsContextMenu, &paths)
                .status,
            ComponentStatus::Installed
        );
        sandbox
            .root()
            .create_subkey(&command_path)
            .expect("mở command của verb")
            .0
            .set_value("", &rewritten.to_owned())
            .expect("ghi đè command");

        assert_eq!(
            backend.uninstall(ComponentId::WindowsContextMenu).status,
            ComponentStatus::Conflict
        );
        assert!(
            sandbox.key_exists(VERB_KEY_PATH),
            "không được xoá verb đã bị ghi đè"
        );
        assert_eq!(sandbox.read(&command_path, "").as_deref(), Some(rewritten));
    }

    #[test]
    fn install_and_uninstall_are_idempotent_and_second_uninstall_is_already_absent() {
        let sandbox = RegistrySandbox::new();
        let backend = sandbox.backend();
        let paths = binaries("Morphix App");
        for id in [ComponentId::WindowsContextMenu, ComponentId::Autostart] {
            assert_eq!(
                backend.install(id, &paths).status,
                ComponentStatus::Installed
            );
            assert_eq!(
                backend.install(id, &paths).status,
                ComponentStatus::AlreadyInstalled
            );
            assert_eq!(backend.uninstall(id).status, ComponentStatus::Removed);
            assert_eq!(backend.uninstall(id).status, ComponentStatus::AlreadyAbsent);
        }
    }

    #[test]
    fn uninstall_removes_only_morphix_entries_and_keeps_parents_and_other_values() {
        let sandbox = RegistrySandbox::new();
        let backend = sandbox.backend();
        let paths = binaries("Morphix App");
        let other_run = r"Software\Microsoft\Windows\CurrentVersion\Run";
        let other_class = r"Software\Classes\.other";
        let root = sandbox.root();
        root.create_subkey(other_class)
            .expect("tạo key ngoài Morphix");
        root.create_subkey(other_run)
            .expect("mở Run")
            .0
            .set_value("OtherApp", &"\"C:\\Other\\other.exe\"".to_owned())
            .expect("ghi Run khác");
        backend.install(ComponentId::WindowsContextMenu, &paths);
        backend.install(ComponentId::Autostart, &paths);

        backend.uninstall(ComponentId::WindowsContextMenu);
        backend.uninstall(ComponentId::Autostart);

        assert!(!sandbox.key_exists(VERB_KEY_PATH));
        assert!(
            sandbox.key_exists(SHELL_PARENT),
            "không được xoá parent shell"
        );
        assert!(
            sandbox.key_exists(other_class),
            "không được xoá key của chương trình khác"
        );
        assert_eq!(sandbox.read(other_run, RUN_VALUE_NAME), None);
        assert!(sandbox.read(other_run, "OtherApp").is_some());
        assert_eq!(sandbox.read(RUN_KEY_PATH, RUN_VALUE_NAME), None);
    }

    #[test]
    fn foreign_verb_and_run_value_are_reported_as_conflict_and_left_byte_for_byte() {
        let sandbox = RegistrySandbox::new();
        let backend = sandbox.backend();
        let paths = binaries("Morphix App");
        let foreign_command = "\"C:\\Other\\other.exe\" \"%1\"";
        let root = sandbox.root();
        root.create_subkey(format!(r"{VERB_KEY_PATH}\{COMMAND_SUBKEY}"))
            .expect("tạo verb của chương trình khác")
            .0
            .set_value("", &foreign_command.to_owned())
            .expect("ghi verb khác");
        root.create_subkey(RUN_KEY_PATH)
            .expect("mở Run")
            .0
            .set_value(RUN_VALUE_NAME, &foreign_command.to_owned())
            .expect("ghi Run khác");

        assert_eq!(
            backend
                .install(ComponentId::WindowsContextMenu, &paths)
                .status,
            ComponentStatus::Conflict
        );
        assert_eq!(
            backend.install(ComponentId::Autostart, &paths).status,
            ComponentStatus::Conflict
        );
        assert_eq!(
            backend.uninstall(ComponentId::WindowsContextMenu).status,
            ComponentStatus::Conflict
        );
        assert_eq!(
            backend.uninstall(ComponentId::Autostart).status,
            ComponentStatus::Conflict
        );
        assert_eq!(
            sandbox
                .read(&format!(r"{VERB_KEY_PATH}\{COMMAND_SUBKEY}"), "")
                .as_deref(),
            Some(foreign_command)
        );
        assert_eq!(
            sandbox.read(RUN_KEY_PATH, RUN_VALUE_NAME).as_deref(),
            Some(foreign_command)
        );
        // WHY kiểm key: marker autostart nằm ở OWNER_KEY_PATH; xung đột không được tạo key đó.
        assert!(
            !sandbox.key_exists(OWNER_KEY_PATH),
            "xung đột không được ghi marker autostart"
        );
    }

    #[test]
    fn moved_binary_is_stale_until_reinstalled_and_paths_with_spaces_quotes_and_ampersand_round_trip(
    ) {
        let sandbox = RegistrySandbox::new();
        let backend = sandbox.backend();
        let awkward = BinaryPaths {
            morphix: std::env::temp_dir()
                .join("Apps \"v1\" & Ứng dụng")
                .join("morphix.exe"),
            gui: std::env::temp_dir()
                .join("Apps \"v1\" & Ứng dụng")
                .join("morphix-gui.exe"),
        };
        backend.install(ComponentId::WindowsContextMenu, &awkward);
        backend.install(ComponentId::Autostart, &awkward);
        assert_eq!(
            backend
                .inspect(ComponentId::WindowsContextMenu, &awkward)
                .status,
            ComponentStatus::Installed
        );
        let moved = binaries("Morphix App");
        assert_eq!(
            backend
                .inspect(ComponentId::WindowsContextMenu, &moved)
                .status,
            ComponentStatus::Stale
        );
        assert_eq!(
            backend.inspect(ComponentId::Autostart, &moved).status,
            ComponentStatus::Stale
        );
        assert_eq!(
            backend
                .install(ComponentId::WindowsContextMenu, &moved)
                .status,
            ComponentStatus::Installed
        );
    }
}

#[test]
fn user_mode_installs_nautilus_extension_and_launcher() {
    let f = tempdir().unwrap();
    let binaries = binaries_in(f.path());
    let root = f.path().join("root");
    let backend = linux_with(&root, gnome_session(), scripted_tools(Vec::new()));
    let report = integration::install(&backend, &binaries, &IntegrationOptions::default());
    for id in [
        ComponentId::NautilusExtension,
        ComponentId::ApplicationLauncher,
    ] {
        assert_eq!(status_of(&report, id), ComponentStatus::Installed, "{id:?}");
    }
    let extension =
        fs::read_to_string(root.join("data/nautilus-python/extensions/morphix_menu.py")).unwrap();
    let morphix = binaries.morphix.to_str().unwrap();
    assert!(extension.contains(&format!("EXECUTABLE: str = \"{morphix}\"")));
    assert!(!extension.contains("/usr/bin/morphix"));
    assert!(extension.starts_with(&format!("# {MANAGED_MARKER}\n")));
    let launcher = fs::read_to_string(root.join("data/applications/morphix.desktop")).unwrap();
    assert!(
        launcher.contains(&format!("Exec={morphix} tray")),
        "{launcher}"
    );
    assert!(
        launcher.contains(&format!("TryExec={morphix}")),
        "{launcher}"
    );
}

#[test]
fn user_mode_reports_desktop_menu_not_applicable() {
    let f = tempdir().unwrap();
    let binaries = binaries_in(f.path());
    let backend = linux_with(
        &f.path().join("root"),
        gnome_session(),
        scripted_tools(Vec::new()),
    );
    let report = backend.inspect(ComponentId::DesktopContextMenu, &binaries);
    assert_eq!(report.status, ComponentStatus::NotApplicable);
    assert!(report.message.contains(".deb"), "{}", report.message);
}

#[test]
fn uninstall_keeps_unowned_launcher() {
    let f = tempdir().unwrap();
    let binaries = binaries_in(f.path());
    let root = f.path().join("root");
    let launcher = root.join("data/applications/morphix.desktop");
    fs::create_dir_all(launcher.parent().unwrap()).unwrap();
    fs::write(&launcher, "[Desktop Entry]\nName=Của người dùng\n").unwrap();
    let backend = linux_with(&root, gnome_session(), scripted_tools(Vec::new()));
    integration::uninstall(&backend, &binaries, &IntegrationOptions::default());
    assert_eq!(
        fs::read_to_string(&launcher).unwrap(),
        "[Desktop Entry]\nName=Của người dùng\n"
    );
}

#[test]
fn install_mode_detects_package_paths() {
    assert_eq!(
        install_mode(Path::new("/usr/bin/morphix")),
        InstallMode::Package
    );
    assert_eq!(
        install_mode(Path::new("/home/u/.local/bin/morphix")),
        InstallMode::User
    );
}

#[test]
fn only_present_file_managers_get_integration() {
    let f = tempdir().unwrap();
    let binaries = binaries_in(f.path());
    let bin = f.path().join("path-bin");
    fs::create_dir_all(&bin).unwrap();
    fs::write(bin.join("nemo"), b"").unwrap();
    let root = f.path().join("root");
    let session = SessionEnv {
        path: Some(bin.into_os_string()),
        ..gnome_session()
    };
    let backend = linux_with(&root, session, scripted_tools(Vec::new()));
    let report = integration::install(&backend, &binaries, &IntegrationOptions::default());
    assert_eq!(
        status_of(&report, ComponentId::NemoAction),
        ComponentStatus::Installed
    );
    assert!(root.join("data/nemo/actions/morphix.nemo_action").is_file());
    for id in [
        ComponentId::DolphinServiceMenu,
        ComponentId::CajaScript,
        ComponentId::NautilusScript,
    ] {
        assert_eq!(
            status_of(&report, id),
            ComponentStatus::NotApplicable,
            "{id:?}"
        );
    }
    assert!(!root.join("data/kio/servicemenus/morphix.desktop").exists());
    assert!(!root.join("config/caja/scripts/Morphix").exists());
    assert!(!root.join("data/nautilus/scripts/Morphix").exists());
}

#[test]
fn nemo_and_thunar_bodies_quote_paths_with_spaces() {
    let path = Path::new("/tmp/a b/Tài liệu/morphix");
    let body = nemo_action_body(path).unwrap();
    let exec = format!(
        "Exec={} menu --uris %U",
        escape_desktop_value(&desktop_exec_argument(path.to_str().unwrap()))
    );
    assert!(body.lines().any(|line| line == exec), "{body}");
    assert!(exec.contains("\"/tmp/a b/Tài liệu/morphix\""), "{exec}");
    assert!(body.starts_with(&format!("# {MANAGED_MARKER}\n[Nemo Action]\n")));
}

#[test]
fn thunar_action_lifecycle_keeps_user_actions() {
    let f = tempdir().unwrap();
    let binaries = binaries_in(f.path());
    let bin = f.path().join("path-bin");
    fs::create_dir_all(&bin).unwrap();
    fs::write(bin.join("thunar"), b"").unwrap();
    let root = f.path().join("root");
    let uca = root.join("config/Thunar/uca.xml");
    fs::create_dir_all(uca.parent().unwrap()).unwrap();
    let user = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<actions>\n<action>\n\t<unique-id>user-1</unique-id>\n</action>\n</actions>\n";
    fs::write(&uca, user).unwrap();
    let session = SessionEnv {
        path: Some(bin.into_os_string()),
        ..gnome_session()
    };
    let backend = linux_with(&root, session, scripted_tools(Vec::new()));
    let installed = integration::install(&backend, &binaries, &IntegrationOptions::default());
    assert_eq!(
        status_of(&installed, ComponentId::ThunarAction),
        ComponentStatus::Installed
    );
    assert_eq!(
        backend.inspect(ComponentId::ThunarAction, &binaries).status,
        ComponentStatus::Installed
    );
    assert!(fs::read_to_string(&uca).unwrap().contains("morphix-menu"));
    integration::uninstall(&backend, &binaries, &IntegrationOptions::default());
    assert_eq!(fs::read_to_string(&uca).unwrap(), user);
}

#[cfg(target_os = "linux")]
#[test]
fn uninstall_outside_a_curl_install_changes_nothing() {
    // WHY: binary test nằm trong target/, không có marker cài đặt; lệnh phải dừng trước khi gỡ bất cứ gì.
    let sandbox = tempdir().unwrap();
    let nautilus = sandbox.path().join("xdg-data/nautilus/scripts/Morphix");
    fs::create_dir_all(nautilus.parent().unwrap()).unwrap();
    fs::write(&nautilus, format!("#!/bin/sh\n# {MANAGED_MARKER}\n")).unwrap();
    sandboxed_morphix(sandbox.path())
        .env("MORPHIX_DATA_DIR", sandbox.path().join("data"))
        .arg("uninstall")
        .assert()
        .code(1)
        .stderr(predicates::str::contains(".morphix-install"));
    assert!(nautilus.is_file());
}

#[cfg(target_os = "linux")]
#[test]
fn uninstall_refuses_dry_run_and_json_instead_of_deleting() {
    let sandbox = tempdir().unwrap();
    for flag in ["--dry-run", "--json"] {
        sandboxed_morphix(sandbox.path())
            .env("MORPHIX_DATA_DIR", sandbox.path().join("data"))
            .args([flag, "uninstall"])
            .assert()
            .code(2)
            .stderr(predicates::str::contains(flag));
    }
}

#[test]
fn uninstall_removes_python_bytecode_of_the_nautilus_extension() {
    // WHY: nautilus-python ghi __pycache__ cạnh extension; để lại thì thư mục do Morphix tạo không xoá được.
    let f = tempdir().unwrap();
    let binaries = binaries_in(f.path());
    let root = f.path().join("root");
    let backend = linux_with(&root, gnome_session(), scripted_tools(Vec::new()));
    integration::install(&backend, &binaries, &IntegrationOptions::default());
    let cache = root.join("data/nautilus-python/extensions/__pycache__");
    fs::create_dir_all(&cache).unwrap();
    fs::write(cache.join("morphix_menu.cpython-314.pyc"), b"pyc").unwrap();
    integration::uninstall(&backend, &binaries, &IntegrationOptions::default());
    assert!(!root.join("data/nautilus-python").exists());
}

#[test]
fn bare_marker_line_in_a_user_file_is_not_ownership() {
    let f = Fixture::new();
    let backend = f.fake();
    let foreign = backend.entry_path(ComponentId::WindowsContextMenu);
    fs::create_dir_all(foreign.parent().unwrap()).unwrap();
    // WHY: chỉ `# Managed by Morphix` hoặc file đánh dấu chứa đúng marker mới là của Morphix.
    let content = format!("{MANAGED_MARKER}\nghi chú riêng của tôi\n");
    fs::write(&foreign, &content).unwrap();

    let report = integration::uninstall(&backend, &f.binaries, &f.options(false, true));
    assert_eq!(
        status_of(&report, ComponentId::WindowsContextMenu),
        ComponentStatus::Conflict
    );
    assert_eq!(fs::read_to_string(&foreign).unwrap(), content);
}

#[test]
fn failed_publish_removes_directories_it_created() {
    let dir = tempdir().unwrap();
    // WHY tên quá dài: tạo thư mục thành công rồi bước đọc file đích lỗi, đúng khe hở cần kiểm.
    let target = dir.path().join("new/sub").join("x".repeat(300));
    {
        let mut tx = Transaction::new();
        assert!(publish_file(&target, b"x", 0o644, &mut tx).is_err());
    }
    assert!(
        !dir.path().join("new").exists(),
        "thư mục tạo dở phải bị xoá"
    );
}

#[test]
fn nautilus_accelerator_reads_the_version_from_rpm_or_pacman_too() {
    let f = tempdir().unwrap();
    let binaries = binaries_in(f.path());
    let options = IntegrationOptions {
        dry_run: false,
        install_autostart: true,
    };
    let accel_status = |root: &Path, tool: &'static str, version: &'static str| {
        let tools = scripted_tools(vec![(tool, exited(0, version, ""))]);
        let report =
            integration::install(&linux_with(root, no_session(), tools), &binaries, &options);
        status_of(&report, ComponentId::NautilusAccel)
    };
    // WHY: Fedora/Arch không có dpkg-query; phiên bản phải đọc được qua rpm hoặc pacman.
    assert_eq!(
        accel_status(&f.path().join("old"), "rpm", "45.2"),
        ComponentStatus::NotApplicable
    );
    assert_ne!(
        accel_status(&f.path().join("fedora"), "rpm", "47.1"),
        ComponentStatus::NotApplicable
    );
    assert_ne!(
        accel_status(&f.path().join("arch"), "pacman", "nautilus 47.1-1\n"),
        ComponentStatus::NotApplicable
    );
}
