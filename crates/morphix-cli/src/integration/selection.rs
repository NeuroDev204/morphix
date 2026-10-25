//! Phím tắt dùng chung cấu hình với người dùng: chỉ sửa đúng dòng/khoá của Morphix.
use super::{
    files::{publish_file, read_existing, Transaction, MANAGED_MARKER},
    linux::{failed_report, io_failure, report, LinuxBackend, XdgRoots},
    tools::{ToolOutcome, ToolRunner},
    ComponentId, ComponentStatus, IntegrationComponentReport,
};
use glib::variant::ToVariant;
use std::{ffi::OsStr, fs, path::Path};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Action {
    Install,
    Uninstall,
    Inspect,
}
pub const ACCEL_LINE: &str = "<Control><Shift>F7 Morphix";
const SHORTCUT_PATH: &str =
    "/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/morphix/";
const MEDIA_SCHEMA: &str = "org.gnome.settings-daemon.plugins.media-keys";
const CUSTOM_SCHEMA: &str = "org.gnome.settings-daemon.plugins.media-keys.custom-keybinding";
const SHORTCUT_NAME: &str = "Morphix: mở Khay";

fn is_capture_accel(accel: &str) -> bool {
    let normalized = accel
        .to_ascii_lowercase()
        .replace("<primary>", "<control>")
        .replace("<ctrl>", "<control>");
    matches!(
        normalized.as_str(),
        "<control><shift>f7" | "<shift><control>f7"
    )
}

pub(super) fn accelerator(
    roots: &XdgRoots,
    tools: &ToolRunner,
    action: Action,
) -> IntegrationComponentReport {
    let id = ComponentId::NautilusAccel;
    let path = roots.config_home.join("nautilus/scripts-accels");
    if action != Action::Uninstall {
        // WHY `nautilus --version` ("GNOME nautilus 46.4"): có trên mọi distro, dpkg-query chỉ có trên Debian/Ubuntu.
        let supported = match tools("nautilus", &[OsStr::new("--version")]) {
            ToolOutcome::Exited {
                code: Some(0),
                stdout,
                ..
            } => stdout
                .split_whitespace()
                .last()
                .and_then(|v| v.split('.').next()?.parse::<u32>().ok())
                .is_some_and(|v| v >= 46),
            _ => false,
        };
        if !supported {
            return report(
                id,
                ComponentStatus::NotApplicable,
                false,
                Some(&path),
                "cần Nautilus 46 trở lên",
            );
        }
    }
    let existing = match read_existing(&path) {
        Ok(file) => file,
        Err(error) => return io_failure(id, &path, &error),
    };
    let bytes = existing
        .as_ref()
        .map_or(&[][..], |file| file.bytes.as_slice());
    let text = match std::str::from_utf8(bytes) {
        Ok(text) => text,
        Err(error) => return failed_report(id, &error.to_string()),
    };
    let ours = text.lines().any(|line| line == ACCEL_LINE);
    if text.lines().any(|line| {
        line.split_whitespace().next().is_some_and(is_capture_accel) && line != ACCEL_LINE
    }) {
        return report(
            id,
            ComponentStatus::Conflict,
            false,
            Some(&path),
            "Ctrl+Shift+F7 đã thuộc script khác; giữ nguyên",
        );
    }
    let stale = text.lines().any(|line| {
        let mut parts = line.split_whitespace();
        parts.next().is_some() && parts.next() == Some("Morphix") && parts.next().is_none()
    });
    if action == Action::Inspect {
        return report(
            id,
            if ours {
                ComponentStatus::Installed
            } else if stale {
                ComponentStatus::Stale
            } else {
                ComponentStatus::Missing
            },
            false,
            Some(&path),
            if ours {
                "đã cài phím tắt Nautilus"
            } else {
                "chưa cài phím tắt Nautilus"
            },
        );
    }
    let marker = roots
        .config_home
        .join("nautilus/.morphix-created-scripts-accels");
    if action == Action::Install && existing.is_none() {
        match read_existing(&marker) {
            Ok(Some(file)) if file.bytes != format!("{MANAGED_MARKER}\n").as_bytes() => {
                return report(
                    id,
                    ComponentStatus::Conflict,
                    false,
                    Some(&marker),
                    "file đánh dấu không thuộc Morphix; giữ nguyên",
                );
            }
            Err(error) => return io_failure(id, &marker, &error),
            _ => {}
        }
    }
    let result = (|| -> std::io::Result<()> {
        let mut tx = Transaction::new();
        if action == Action::Install && !ours {
            // WHY đặt đầu file: gỡ lại đúng cả byte cuối khi file người dùng không có newline.
            let updated = format!("{ACCEL_LINE}\n{text}");
            publish_file(
                &path,
                updated.as_bytes(),
                existing.as_ref().map_or(0o644, |file| file.mode),
                &mut tx,
            )?;
            if existing.is_none() {
                publish_file(
                    &marker,
                    format!("{MANAGED_MARKER}\n").as_bytes(),
                    0o600,
                    &mut tx,
                )?;
            }
        } else if action == Action::Uninstall && ours {
            let updated: String = text
                .split_inclusive('\n')
                .filter(|line| line.trim_end_matches(['\r', '\n']) != ACCEL_LINE)
                .collect();
            publish_file(
                &path,
                updated.as_bytes(),
                existing.as_ref().map_or(0o644, |file| file.mode),
                &mut tx,
            )?;
            if updated.is_empty()
                && read_existing(&marker)?
                    .is_some_and(|file| file.bytes == format!("{MANAGED_MARKER}\n").as_bytes())
            {
                fs::remove_file(&path)?;
                fs::remove_file(&marker)?;
            }
        }
        tx.commit();
        Ok(())
    })();
    if let Err(error) = result {
        return io_failure(id, &path, &error);
    }
    match (action, ours) {
        (Action::Install, true) => report(
            id,
            ComponentStatus::AlreadyInstalled,
            false,
            Some(&path),
            "đã cài sẵn",
        ),
        (Action::Install, false) => report(
            id,
            ComponentStatus::Installed,
            true,
            Some(&path),
            "Chạy `nautilus -q` hoặc đăng xuất để Files nhận phím tắt",
        ),
        (Action::Uninstall, true) => report(
            id,
            ComponentStatus::Removed,
            true,
            Some(&path),
            "đã gỡ phím tắt Nautilus",
        ),
        _ => report(
            id,
            ComponentStatus::AlreadyAbsent,
            false,
            Some(&path),
            "chưa cài",
        ),
    }
}

fn setting(tools: &ToolRunner, args: &[&str]) -> Result<String, String> {
    let args: Vec<&OsStr> = args.iter().map(OsStr::new).collect();
    match tools("gsettings", &args) {
        ToolOutcome::Exited {
            code: Some(0),
            stdout,
            ..
        } => Ok(stdout.trim().into()),
        ToolOutcome::NotFound => Err("không có gsettings".into()),
        ToolOutcome::Failed(message) => Err(message),
        ToolOutcome::Exited {
            code,
            stdout,
            stderr,
        } => Err(format!("gsettings thoát {code:?}: {stdout} {stderr}")),
    }
}
fn string_setting(tools: &ToolRunner, schema: &str, key: &str) -> Result<String, String> {
    let value = setting(tools, &["get", schema, key])?;
    glib::Variant::parse(None, &value)
        .map_err(|e| e.to_string())?
        .get::<String>()
        .ok_or_else(|| format!("{key} không phải chuỗi GVariant"))
}
fn shortcut_schema(path: &str) -> String {
    format!("{CUSTOM_SCHEMA}:{path}")
}
fn command_for(morphix: &Path, selection: bool) -> Result<String, String> {
    let path = morphix
        .to_str()
        .ok_or_else(|| "đường dẫn morphix không phải UTF-8".to_string())?;
    if path.contains(['\n', '\r', '\0']) {
        return Err("đường dẫn morphix không hợp lệ".into());
    }
    let executable = if path
        .chars()
        .any(|c| c.is_whitespace() || "\\\"'`$".contains(c))
    {
        format!(
            "\"{}\"",
            path.replace('\\', "\\\\")
                .replace('"', "\\\"")
                .replace('`', "\\`")
                .replace('$', "\\$")
        )
    } else {
        path.into()
    };
    Ok(format!(
        "{executable} tray{}",
        if selection { " --selection" } else { "" }
    ))
}

pub(super) fn shortcut(
    backend: &LinuxBackend,
    action: Action,
    morphix: &Path,
) -> IntegrationComponentReport {
    let id = ComponentId::GnomeShortcut;
    if !backend.session.is_gnome() {
        return report(
            id,
            ComponentStatus::NotApplicable,
            false,
            None,
            "phím tắt này chỉ áp dụng trên GNOME",
        );
    }
    if !backend.session.has_session_bus {
        return report(
            id,
            ComponentStatus::Unknown,
            false,
            None,
            "thiếu phiên D-Bus",
        );
    }
    let tools = &*backend.tools;
    let result = shortcut_inner(tools, action, morphix);
    match result {
        Ok((status, changed, message)) => report(id, status, changed, None, message),
        Err(message) if message == "không có gsettings" => {
            report(id, ComponentStatus::NotApplicable, false, None, &message)
        }
        Err(message) => failed_report(id, &message),
    }
}

fn shortcut_inner(
    tools: &ToolRunner,
    action: Action,
    morphix: &Path,
) -> Result<(ComponentStatus, bool, &'static str), String> {
    let raw_list = setting(tools, &["get", MEDIA_SCHEMA, "custom-keybindings"])?;
    let mut paths = glib::Variant::parse(None, &raw_list)
        .map_err(|e| e.to_string())?
        .get::<Vec<String>>()
        .ok_or("custom-keybindings không phải danh sách chuỗi")?;
    let schema = shortcut_schema(SHORTCUT_PATH);
    let current = string_setting(tools, &schema, "command")?;
    let command = command_for(morphix, true)?;
    let legacy = command_for(morphix, false)?;
    let owned = current == command || current == legacy;
    if !current.is_empty() && !owned {
        return Ok((
            ComponentStatus::Conflict,
            false,
            "lệnh phím tắt Morphix đã thuộc chương trình khác; giữ nguyên",
        ));
    }
    let name = string_setting(tools, &schema, "name")?;
    let binding = string_setting(tools, &schema, "binding")?;
    let present = paths.iter().any(|path| path == SHORTCUT_PATH);
    if action != Action::Uninstall {
        for path in paths.iter().filter(|path| path.as_str() != SHORTCUT_PATH) {
            let other = string_setting(tools, &shortcut_schema(path), "binding")?;
            if other.eq_ignore_ascii_case("<Alt>c") || other.eq_ignore_ascii_case("<Mod1>c") {
                return Ok((
                    ComponentStatus::Conflict,
                    false,
                    "Alt+C đã thuộc phím tắt khác; giữ nguyên",
                ));
            }
        }
    }
    let complete = present && current == command && binding == "<Alt>c" && name == SHORTCUT_NAME;
    if action == Action::Inspect {
        return Ok((
            if complete {
                ComponentStatus::Installed
            } else if owned {
                ComponentStatus::Stale
            } else {
                ComponentStatus::Missing
            },
            false,
            if complete {
                "phím tắt Alt+C đang hoạt động"
            } else {
                "chạy install-integration để cài/cập nhật Alt+C"
            },
        ));
    }
    if action == Action::Install && complete {
        return Ok((ComponentStatus::AlreadyInstalled, false, "đã cài sẵn"));
    }
    if action == Action::Uninstall && !owned {
        return Ok((ComponentStatus::AlreadyAbsent, false, "chưa cài"));
    }
    let mut writes: Vec<(String, String, String, String)> = Vec::new();
    for (key, old, new) in [
        ("name", name, SHORTCUT_NAME.to_string()),
        ("command", current, command),
        ("binding", binding, "<Alt>c".to_string()),
    ] {
        writes.push((
            schema.clone(),
            key.into(),
            old.to_variant().print(true).to_string(),
            (if action == Action::Install {
                new
            } else {
                String::new()
            })
            .to_variant()
            .print(true)
            .to_string(),
        ));
    }
    // WHY: gsettings thay cả mảng; chỉ thêm/gỡ đường dẫn Morphix trên danh sách vừa đọc.
    if action == Action::Install && !present {
        paths.push(SHORTCUT_PATH.into());
    }
    if action == Action::Uninstall {
        paths.retain(|path| path != SHORTCUT_PATH);
    }
    writes.push((
        MEDIA_SCHEMA.into(),
        "custom-keybindings".into(),
        raw_list,
        paths.to_variant().print(true).to_string(),
    ));
    for (index, (schema, key, _, value)) in writes.iter().enumerate() {
        if let Err(error) = setting(tools, &["set", schema, key, value]) {
            for (schema, key, old, _) in writes[..=index].iter().rev() {
                if let Err(rollback) = setting(tools, &["set", schema, key, old]) {
                    tracing::error!(%rollback, "không khôi phục được phím tắt GNOME");
                }
            }
            return Err(error);
        }
    }
    Ok((
        if action == Action::Install {
            ComponentStatus::Installed
        } else {
            ComponentStatus::Removed
        },
        true,
        if action == Action::Install {
            "đã cài Alt+C"
        } else {
            "đã gỡ Alt+C"
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        cell::{Cell, RefCell},
        collections::HashMap,
        rc::Rc,
    };

    fn nautilus(program: &str, _: &[&OsStr]) -> ToolOutcome {
        // WHY `nautilus --version`: có trên mọi distro, còn dpkg-query chỉ có trên Debian/Ubuntu.
        if program == "nautilus" {
            ToolOutcome::Exited {
                code: Some(0),
                stdout: "GNOME nautilus 46.4\n".into(),
                stderr: String::new(),
            }
        } else {
            ToolOutcome::NotFound
        }
    }
    #[test]
    fn accel_installs_reinstalls_and_removes_only_our_line() {
        for original in [
            None,
            Some(""),
            Some("<Control>e Other"),
            Some("# user\n<Control>e Other\n"),
        ] {
            let dir = tempfile::tempdir().unwrap();
            let roots = XdgRoots {
                data_home: dir.path().join("data"),
                config_home: dir.path().join("config"),
            };
            let path = roots.config_home.join("nautilus/scripts-accels");
            if let Some(text) = original {
                fs::create_dir_all(path.parent().unwrap()).unwrap();
                fs::write(&path, text).unwrap();
            }
            assert_eq!(
                accelerator(&roots, &nautilus, Action::Inspect).status,
                ComponentStatus::Missing
            );
            assert_eq!(
                accelerator(&roots, &nautilus, Action::Install).status,
                ComponentStatus::Installed
            );
            assert_eq!(
                accelerator(&roots, &nautilus, Action::Install).status,
                ComponentStatus::AlreadyInstalled
            );
            assert_eq!(
                accelerator(&roots, &nautilus, Action::Inspect).status,
                ComponentStatus::Installed
            );
            assert_eq!(
                accelerator(&roots, &nautilus, Action::Uninstall).status,
                ComponentStatus::Removed
            );
            match original {
                Some(text) => assert_eq!(fs::read_to_string(&path).unwrap(), text),
                None => assert!(!path.exists()),
            }
        }
    }
    #[test]
    fn accel_key_used_by_other_script_is_conflict() {
        let dir = tempfile::tempdir().unwrap();
        let roots = XdgRoots {
            data_home: dir.path().into(),
            config_home: dir.path().into(),
        };
        let path = roots.config_home.join("nautilus/scripts-accels");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let original = "<Primary><Shift>F7 UserScript\n";
        fs::write(&path, original).unwrap();
        for action in [Action::Install, Action::Uninstall, Action::Inspect] {
            assert_eq!(
                accelerator(&roots, &nautilus, action).status,
                ComponentStatus::Conflict
            );
            assert_eq!(fs::read_to_string(&path).unwrap(), original);
        }
    }
    #[test]
    fn doctor_reports_stale_accel_without_modifying_user_line() {
        let dir = tempfile::tempdir().unwrap();
        let roots = XdgRoots {
            data_home: dir.path().into(),
            config_home: dir.path().into(),
        };
        let path = roots.config_home.join("nautilus/scripts-accels");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let original = "<Control>F8 Morphix\n";
        fs::write(&path, original).unwrap();
        assert_eq!(
            accelerator(&roots, &nautilus, Action::Inspect).status,
            ComponentStatus::Stale
        );
        assert_eq!(fs::read_to_string(path).unwrap(), original);
    }
    #[test]
    fn accel_never_overwrites_a_foreign_ownership_marker() {
        let dir = tempfile::tempdir().unwrap();
        let roots = XdgRoots {
            data_home: dir.path().into(),
            config_home: dir.path().into(),
        };
        let marker = roots
            .config_home
            .join("nautilus/.morphix-created-scripts-accels");
        fs::create_dir_all(marker.parent().unwrap()).unwrap();
        fs::write(&marker, "user data").unwrap();
        assert_eq!(
            accelerator(&roots, &nautilus, Action::Install).status,
            ComponentStatus::Conflict
        );
        assert_eq!(fs::read_to_string(marker).unwrap(), "user data");
        assert!(!roots.config_home.join("nautilus/scripts-accels").exists());
    }
    #[test]
    fn accel_older_nautilus_is_not_applicable() {
        let dir = tempfile::tempdir().unwrap();
        let roots = XdgRoots {
            data_home: dir.path().into(),
            config_home: dir.path().into(),
        };
        let old = |_: &str, _: &[&OsStr]| ToolOutcome::Exited {
            code: Some(0),
            stdout: "1:42.6-0ubuntu1".into(),
            stderr: String::new(),
        };
        assert_eq!(
            accelerator(&roots, &old, Action::Install).status,
            ComponentStatus::NotApplicable
        );
        assert!(!roots.config_home.join("nautilus").exists());
    }

    type Settings = HashMap<(String, String), String>;
    struct FakeSettings {
        values: RefCell<Settings>,
        writes: RefCell<Vec<Vec<String>>>,
        fail_at: Cell<Option<usize>>,
    }
    impl FakeSettings {
        fn new() -> Rc<Self> {
            let fake = Rc::new(Self {
                values: RefCell::new(HashMap::new()),
                writes: RefCell::new(vec![]),
                fail_at: Cell::new(None),
            });
            fake.set(MEDIA_SCHEMA, "custom-keybindings", "@as []");
            for key in ["name", "command", "binding"] {
                fake.set(&shortcut_schema(SHORTCUT_PATH), key, "''");
            }
            fake
        }
        fn set(&self, schema: &str, key: &str, value: &str) {
            self.values
                .borrow_mut()
                .insert((schema.into(), key.into()), value.into());
        }
        fn run(&self, program: &str, args: &[&OsStr]) -> ToolOutcome {
            assert_eq!(program, "gsettings");
            let args: Vec<String> = args
                .iter()
                .map(|arg| arg.to_str().unwrap().into())
                .collect();
            let result = match args[0].as_str() {
                "get" => self
                    .values
                    .borrow()
                    .get(&(args[1].clone(), args[2].clone()))
                    .cloned()
                    .unwrap_or_else(|| "''".into()),
                "set" => {
                    self.writes.borrow_mut().push(args.clone());
                    if self.fail_at.get() == Some(self.writes.borrow().len()) {
                        return ToolOutcome::Exited {
                            code: Some(1),
                            stdout: String::new(),
                            stderr: "test write failed".into(),
                        };
                    }
                    self.set(&args[1], &args[2], &args[3]);
                    String::new()
                }
                _ => panic!("unexpected args {args:?}"),
            };
            ToolOutcome::Exited {
                code: Some(0),
                stdout: result,
                stderr: String::new(),
            }
        }
        fn paths(&self) -> Vec<String> {
            let values = self.values.borrow();
            glib::Variant::parse(
                None,
                &values[&(MEDIA_SCHEMA.into(), "custom-keybindings".into())],
            )
            .unwrap()
            .get()
            .unwrap()
        }
    }
    fn with_tools(fake: &Rc<FakeSettings>) -> impl Fn(&str, &[&OsStr]) -> ToolOutcome + 'static {
        let fake = fake.clone();
        move |program, args| fake.run(program, args)
    }
    fn other_shortcut(fake: &FakeSettings, binding: &str) {
        fake.set(MEDIA_SCHEMA, "custom-keybindings", "['/user/']");
        fake.set(
            &shortcut_schema("/user/"),
            "binding",
            &binding.to_variant().print(true),
        );
    }
    #[test]
    fn shortcut_installs_keeps_other_bindings_and_uninstalls_only_ours() {
        let fake = FakeSettings::new();
        other_shortcut(&fake, "<Super>x");
        let tools = with_tools(&fake);
        let path = Path::new("/usr/bin/morphix");
        assert_eq!(
            shortcut_inner(&tools, Action::Install, path).unwrap().0,
            ComponentStatus::Installed
        );
        assert_eq!(fake.paths(), ["/user/", SHORTCUT_PATH]);
        assert_eq!(
            shortcut_inner(&tools, Action::Install, path).unwrap().0,
            ComponentStatus::AlreadyInstalled
        );
        assert_eq!(
            shortcut_inner(&tools, Action::Inspect, path).unwrap().0,
            ComponentStatus::Installed
        );
        assert_eq!(
            shortcut_inner(&tools, Action::Uninstall, path).unwrap().0,
            ComponentStatus::Removed
        );
        assert_eq!(fake.paths(), ["/user/"]);
        assert_eq!(
            string_setting(&tools, &shortcut_schema("/user/"), "binding").unwrap(),
            "<Super>x"
        );
    }
    #[test]
    fn shortcut_replaces_old_manual_command() {
        let fake = FakeSettings::new();
        fake.set(
            &shortcut_schema(SHORTCUT_PATH),
            "command",
            "'/usr/bin/morphix tray'",
        );
        assert_eq!(
            shortcut_inner(
                &with_tools(&fake),
                Action::Install,
                Path::new("/usr/bin/morphix")
            )
            .unwrap()
            .0,
            ComponentStatus::Installed
        );
        assert_eq!(
            string_setting(
                &with_tools(&fake),
                &shortcut_schema(SHORTCUT_PATH),
                "command"
            )
            .unwrap(),
            "/usr/bin/morphix tray --selection"
        );
    }
    #[test]
    fn shortcut_conflicts_write_nothing_and_cannot_remove_foreign_command() {
        for foreign_command in [false, true] {
            let fake = FakeSettings::new();
            if foreign_command {
                fake.set(&shortcut_schema(SHORTCUT_PATH), "command", "'other-app'");
            } else {
                other_shortcut(&fake, "<Alt>c");
            }
            let before = fake.values.borrow().clone();
            let action = if foreign_command {
                Action::Uninstall
            } else {
                Action::Install
            };
            assert_eq!(
                shortcut_inner(&with_tools(&fake), action, Path::new("/usr/bin/morphix"))
                    .unwrap()
                    .0,
                ComponentStatus::Conflict
            );
            assert!(fake.writes.borrow().is_empty());
            assert_eq!(*fake.values.borrow(), before);
        }
    }
    #[test]
    fn shortcut_rolls_back_when_a_write_fails() {
        let fake = FakeSettings::new();
        let before = fake.values.borrow().clone();
        fake.fail_at.set(Some(3));
        assert!(shortcut_inner(
            &with_tools(&fake),
            Action::Install,
            Path::new("/usr/bin/morphix")
        )
        .is_err());
        assert_eq!(*fake.values.borrow(), before);
    }
    #[test]
    fn shortcut_missing_gsettings_is_not_applicable() {
        let roots = XdgRoots {
            data_home: "/unused".into(),
            config_home: "/unused".into(),
        };
        let backend = LinuxBackend::new(
            roots,
            super::super::linux::SessionEnv {
                current_desktop: Some("GNOME".into()),
                has_session_bus: true,
                ..Default::default()
            },
            Box::new(|_, _| ToolOutcome::NotFound),
        );
        assert_eq!(
            shortcut(&backend, Action::Install, Path::new("/usr/bin/morphix")).status,
            ComponentStatus::NotApplicable
        );
    }
    #[test]
    fn shortcut_and_accelerator_are_not_applicable_on_wayland() {
        use super::super::{BinaryPaths, IntegrationBackend};
        let calls: Rc<std::cell::RefCell<Vec<String>>> = Rc::default();
        let log = calls.clone();
        let backend = LinuxBackend::new(
            XdgRoots {
                data_home: "/unused".into(),
                config_home: "/unused".into(),
            },
            super::super::linux::SessionEnv {
                current_desktop: Some("GNOME".into()),
                has_session_bus: true,
                session_type: Some("wayland".into()),
                ..Default::default()
            },
            Box::new(move |program, _| {
                log.borrow_mut().push(program.to_string());
                ToolOutcome::NotFound
            }),
        );
        let binaries = BinaryPaths {
            morphix: "/usr/bin/morphix".into(),
            gui: "/usr/bin/morphix-gui".into(),
        };
        for id in [ComponentId::GnomeShortcut, ComponentId::NautilusAccel] {
            let report = backend.install(id, &binaries);
            assert_eq!(report.status, ComponentStatus::NotApplicable, "{id:?}");
            assert!(
                report.message.contains("chỉ có trên X11"),
                "{}",
                report.message
            );
        }
        assert!(calls.borrow().is_empty(), "{:?}", calls.borrow());
    }
    #[test]
    fn shortcut_quotes_binary_paths_as_one_argument() {
        let path = Path::new("/tmp/Apps \"v1\" & it's/morphix");
        let command = command_for(path, true).unwrap();
        assert_eq!(
            glib::shell_parse_argv(&command).unwrap(),
            [
                path.as_os_str(),
                OsStr::new("tray"),
                OsStr::new("--selection")
            ]
        );
    }
}
