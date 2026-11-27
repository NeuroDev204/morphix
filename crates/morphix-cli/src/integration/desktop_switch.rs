//! Đổi giữa DING gốc và bản chép Morphix (`ding_fork`) ở mỗi lần đăng nhập, và lùi về DING gốc khi bản chép hỏng.
//! WHY ở lần đăng nhập sau chứ không lúc cài: GNOME chỉ thấy thư mục extension mới sau khi đăng nhập lại; tắt DING
//! gốc trước lúc đó sẽ làm desktop trống tới khi đăng nhập.

use std::ffi::OsStr;
use std::path::PathBuf;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use super::ding_fork::{build_fork, fingerprint, find_source, read_marker, DING_UUID, FORK_UUID};
use super::linux::SessionEnv;
use super::tools::{ToolOutcome, ToolRunner};

/// Tên D-Bus mà tiến trình `ding.js` giữ khi đang vẽ desktop. WHY kiểm nó chứ không chỉ trạng thái extension:
/// `ding.js` chạy tiến trình riêng, JS chèn lỗi thì extension vẫn báo bật nhưng desktop trống.
const DING_BUS_NAME: &str = "com.rastersoft.ding";
const HEALTH_ATTEMPTS: u32 = 10;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Fallback {
    pub fingerprint: String,
    pub morphix_version: String,
    pub reason: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct DesktopState {
    pub switched: bool,
    pub ding_was_enabled: bool,
    pub fallback: Option<Fallback>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckOutcome {
    NotApplicable(String),
    Switched,
    Rebuilt,
    FellBack(String),
    Nothing,
}

pub struct DesktopCtx {
    pub data_home: PathBuf,
    pub data_dirs: Vec<PathBuf>,
    pub session: SessionEnv,
    pub morphix: PathBuf,
    pub version: String,
}

impl DesktopCtx {
    /// `session.system_data_dirs` `None` là mặc định XDG (`/usr/local/share:/usr/share`).
    pub fn new(data_home: PathBuf, session: &SessionEnv, morphix: PathBuf) -> Self {
        DesktopCtx {
            data_home,
            data_dirs: session
                .system_data_dirs
                .clone()
                .unwrap_or_else(|| vec![PathBuf::from("/usr/local/share"), PathBuf::from("/usr/share")]),
            session: session.clone(),
            morphix,
            version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }
    pub fn extensions_dir(&self) -> PathBuf {
        self.data_home.join("gnome-shell/extensions")
    }
    pub fn fork_dir(&self) -> PathBuf {
        self.extensions_dir().join(FORK_UUID)
    }
    pub fn state_path(&self) -> PathBuf {
        self.data_home.join("morphix/desktop-state.json")
    }
}

/// File hỏng hoặc thiếu → trạng thái mới (chưa đổi lần nào), không lỗi.
pub fn load_state(ctx: &DesktopCtx) -> DesktopState {
    std::fs::read_to_string(ctx.state_path())
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

pub fn save_state(ctx: &DesktopCtx, state: &DesktopState) -> Result<(), String> {
    let path = ctx.state_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| format!("không tạo được {}: {error}", parent.display()))?;
    }
    let json = serde_json::to_string_pretty(state).map_err(|error| error.to_string())?;
    std::fs::write(&path, json).map_err(|error| format!("không ghi được {}: {error}", path.display()))
}

struct Exts {
    known: Vec<String>,
    enabled: Vec<String>,
}

impl Exts {
    fn knows(&self, uuid: &str) -> bool { self.known.iter().any(|u| u == uuid) }
    fn is_enabled(&self, uuid: &str) -> bool { self.enabled.iter().any(|u| u == uuid) }
}

fn run(tools: &ToolRunner, program: &str, args: &[&str]) -> ToolOutcome {
    let args: Vec<&OsStr> = args.iter().map(OsStr::new).collect();
    tools(program, &args)
}

/// `None` khi không có `gnome-extensions` hoặc GNOME Shell không trả lời (không biết gì thì không đổi gì).
fn exts(tools: &ToolRunner) -> Option<Exts> {
    let lines = |args: &[&str]| match run(tools, "gnome-extensions", args) {
        ToolOutcome::Exited { code: Some(0), stdout, .. } => Some(stdout.lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect()),
        _ => None,
    };
    Some(Exts { known: lines(&["list"])?, enabled: lines(&["list", "--enabled"])? })
}

/// `Some(bật?)`; `None` khi không hỏi được GNOME Shell.
pub(super) fn is_enabled(tools: &ToolRunner, uuid: &str) -> Option<bool> {
    exts(tools).map(|e| e.is_enabled(uuid))
}

fn set_enabled(tools: &ToolRunner, uuid: &str, on: bool) -> Result<(), String> {
    let verb = if on { "enable" } else { "disable" };
    match run(tools, "gnome-extensions", &[verb, uuid]) {
        ToolOutcome::Exited { code: Some(0), .. } => Ok(()),
        other => Err(format!("gnome-extensions {verb} {uuid} lỗi: {other:?}")),
    }
}

fn ding_running(tools: &ToolRunner) -> bool {
    let args = ["call", "--session", "--dest", "org.freedesktop.DBus", "--object-path", "/org/freedesktop/DBus", "--method", "org.freedesktop.DBus.NameHasOwner", DING_BUS_NAME];
    matches!(run(tools, "gdbus", &args), ToolOutcome::Exited { code: Some(0), stdout, .. } if stdout.contains("true"))
}

/// Đợi bản chép chạy thật: được bật **và** giữ tên D-Bus của DING.
fn verify_fork(tools: &ToolRunner, wait: &dyn Fn(Duration)) -> Result<(), String> {
    for attempt in 0..HEALTH_ATTEMPTS {
        let enabled = exts(tools).is_some_and(|e| e.is_enabled(FORK_UUID));
        if enabled && ding_running(tools) {
            return Ok(());
        }
        if attempt + 1 < HEALTH_ATTEMPTS {
            wait(Duration::from_secs(1));
        }
    }
    Err(format!("bản chép không chạy sau {HEALTH_ATTEMPTS} giây (không thấy tên D-Bus {DING_BUS_NAME})"))
}

/// Tắt bản chép, bật lại DING gốc nếu Morphix đã tắt nó, ghi lý do để không thử lại vô ích.
fn fall_back(ctx: &DesktopCtx, tools: &ToolRunner, mut state: DesktopState, fingerprint: String, reason: String) -> CheckOutcome {
    if exts(tools).is_some_and(|e| e.is_enabled(FORK_UUID)) {
        if let Err(error) = set_enabled(tools, FORK_UUID, false) {
            tracing::error!(%error, "không tắt được bản chép DING của Morphix");
        }
    }
    let ding_back = state.switched && state.ding_was_enabled;
    if ding_back {
        if let Err(error) = set_enabled(tools, DING_UUID, true) {
            tracing::error!(%error, "không bật lại được DING gốc");
        }
    }
    tracing::warn!(%reason, "menu desktop của Morphix lùi về DING gốc");
    state.switched = false;
    state.fallback = Some(Fallback { fingerprint, morphix_version: ctx.version.clone(), reason: reason.clone() });
    if let Err(error) = save_state(ctx, &state) {
        tracing::error!(%error, "không ghi được trạng thái desktop");
    }
    if ding_back {
        run(tools, "notify-send", &["Morphix", "Menu Morphix trên desktop tạm tắt vì lỗi; icon desktop đã trở lại như cũ."]);
    }
    CheckOutcome::FellBack(reason)
}

pub fn desktop_check(ctx: &DesktopCtx, tools: &ToolRunner, wait: &dyn Fn(Duration)) -> CheckOutcome {
    if !ctx.session.is_gnome() || !ctx.session.has_session_bus {
        return CheckOutcome::NotApplicable("không phải phiên GNOME".into());
    }
    let Some(listing) = exts(tools) else {
        return CheckOutcome::NotApplicable("không hỏi được GNOME Shell (gnome-extensions)".into());
    };
    let fork_dir = ctx.fork_dir();
    let Some(marker) = read_marker(&fork_dir) else { return CheckOutcome::Nothing };
    let mut state = load_state(ctx);

    // WHY không bao giờ để hai bản cùng bật: hai tiến trình vẽ desktop, trùng tên D-Bus.
    if listing.is_enabled(FORK_UUID) && listing.is_enabled(DING_UUID) {
        if let Err(error) = set_enabled(tools, FORK_UUID, false) {
            tracing::error!(%error, "không tắt được bản chép DING của Morphix");
        }
        return CheckOutcome::Nothing;
    }

    let Some(source) = find_source(&ctx.data_home, &ctx.data_dirs) else {
        if listing.is_enabled(FORK_UUID) {
            return fall_back(ctx, tools, state, marker.fingerprint, "không còn thấy DING gốc để chép".into());
        }
        return CheckOutcome::Nothing;
    };
    let current = match fingerprint(&source) {
        Ok(value) => value,
        Err(reason) => return fall_back(ctx, tools, state, String::new(), reason),
    };
    if state.fallback.as_ref().is_some_and(|f| f.fingerprint == current && f.morphix_version == ctx.version) {
        return CheckOutcome::Nothing;
    }

    let mut rebuilt = false;
    if marker.fingerprint != current || marker.morphix_version != ctx.version {
        if let Err(reason) = build_fork(&source, &ctx.extensions_dir(), &ctx.morphix, &ctx.version) {
            return fall_back(ctx, tools, state, current, reason);
        }
        rebuilt = true;
    }

    if !state.switched {
        // DING gốc đang tắt: người dùng không dùng desktop icon, không bật hộ. Bản chép chưa được GNOME biết: chờ đăng nhập lại.
        if !listing.is_enabled(DING_UUID) || !listing.knows(FORK_UUID) {
            return if rebuilt { CheckOutcome::Rebuilt } else { CheckOutcome::Nothing };
        }
        state = DesktopState { switched: true, ding_was_enabled: true, fallback: None };
        if let Err(error) = save_state(ctx, &state) {
            return CheckOutcome::NotApplicable(error);
        }
        if let Err(reason) = set_enabled(tools, DING_UUID, false).and_then(|()| set_enabled(tools, FORK_UUID, true)) {
            return fall_back(ctx, tools, state, current, reason);
        }
        return match verify_fork(tools, wait) {
            Ok(()) => CheckOutcome::Switched,
            Err(reason) => fall_back(ctx, tools, state, current, reason),
        };
    }

    if !listing.is_enabled(FORK_UUID) {
        return if rebuilt { CheckOutcome::Rebuilt } else { CheckOutcome::Nothing };
    }
    match verify_fork(tools, wait) {
        Ok(()) if rebuilt => CheckOutcome::Rebuilt,
        Ok(()) => CheckOutcome::Nothing,
        Err(reason) => fall_back(ctx, tools, state, current, reason),
    }
}

/// Gỡ: tắt bản chép; bật lại DING gốc nếu Morphix đã tắt nó. Không xoá file (việc của lệnh gỡ).
pub fn restore(ctx: &DesktopCtx, tools: &ToolRunner) -> Result<(), String> {
    let state = load_state(ctx);
    let Some(listing) = exts(tools) else { return Ok(()) };
    if listing.is_enabled(FORK_UUID) {
        set_enabled(tools, FORK_UUID, false)?;
    }
    if state.switched && state.ding_was_enabled {
        set_enabled(tools, DING_UUID, true)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::ding_fork::fixtures::{fake_ding, MENU_26};
    use super::*;
    use std::cell::RefCell;
    use std::collections::BTreeSet;
    use std::rc::Rc;

    #[derive(Default)]
    struct Fake {
        known: RefCell<BTreeSet<String>>,
        enabled: RefCell<BTreeSet<String>>,
        log: RefCell<Vec<String>>,
        bus_owned: RefCell<bool>,
        no_gnome_extensions: RefCell<bool>,
    }

    impl Fake {
        fn run(&self, program: &str, args: &[&OsStr]) -> ToolOutcome {
            let args: Vec<String> = args.iter().map(|a| a.to_string_lossy().into_owned()).collect();
            let ok = |stdout: String| ToolOutcome::Exited { code: Some(0), stdout, stderr: String::new() };
            match (program, args.iter().map(String::as_str).collect::<Vec<_>>().as_slice()) {
                ("gnome-extensions", _) if *self.no_gnome_extensions.borrow() => ToolOutcome::NotFound,
                ("gnome-extensions", ["list"]) => ok(self.known.borrow().iter().map(|u| format!("{u}\n")).collect()),
                ("gnome-extensions", ["list", "--enabled"]) => ok(self.enabled.borrow().iter().map(|u| format!("{u}\n")).collect()),
                ("gnome-extensions", [verb @ ("enable" | "disable"), uuid]) => {
                    self.log.borrow_mut().push(format!("{verb} {uuid}"));
                    if *verb == "enable" { self.enabled.borrow_mut().insert((*uuid).into()); } else { self.enabled.borrow_mut().remove(*uuid); }
                    ok(String::new())
                }
                ("gdbus", _) => ok(if *self.bus_owned.borrow() { "(true,)\n".into() } else { "(false,)\n".into() }),
                ("notify-send", _) => { self.log.borrow_mut().push("notify".into()); ok(String::new()) }
                other => panic!("lệnh không mong đợi: {other:?}"),
            }
        }
        fn count(&self, entry: &str) -> usize { self.log.borrow().iter().filter(|l| *l == entry).count() }
        fn position(&self, entry: &str) -> usize { self.log.borrow().iter().position(|l| l == entry).unwrap_or_else(|| panic!("thiếu {entry}")) }
    }

    struct Rig { dir: tempfile::TempDir, ctx: DesktopCtx, fake: Rc<Fake> }

    impl Rig {
        fn tools(&self) -> impl Fn(&str, &[&OsStr]) -> ToolOutcome {
            let fake = Rc::clone(&self.fake);
            move |program, args| fake.run(program, args)
        }
        fn check(&self) -> CheckOutcome {
            let tools = self.tools();
            desktop_check(&self.ctx, &tools, &|_| {})
        }
        fn fork_exists(&self) -> bool { self.ctx.fork_dir().join("metadata.json").is_file() }
    }

    fn gnome() -> SessionEnv {
        SessionEnv { current_desktop: Some("ubuntu:GNOME".into()), has_session_bus: true, ..SessionEnv::default() }
    }

    /// Bản chép đã dựng sẵn; `ding_enabled`: DING gốc đang bật; `fork_known`: GNOME đã thấy bản chép (đã đăng nhập lại).
    fn rig(ding_enabled: bool, fork_known: bool) -> Rig {
        let dir = tempfile::tempdir().unwrap();
        let system = dir.path().join("sys");
        fake_ding(&system, MENU_26);
        let ctx = DesktopCtx {
            data_home: dir.path().join("home"),
            data_dirs: vec![system],
            session: gnome(),
            morphix: PathBuf::from("/x/morphix"),
            version: "0.1.7".into(),
        };
        let source = find_source(&ctx.data_home, &ctx.data_dirs).unwrap();
        build_fork(&source, &ctx.extensions_dir(), &ctx.morphix, &ctx.version).unwrap();
        let fake = Rc::new(Fake::default());
        fake.known.borrow_mut().insert(DING_UUID.into());
        if ding_enabled { fake.enabled.borrow_mut().insert(DING_UUID.into()); }
        if fork_known { fake.known.borrow_mut().insert(FORK_UUID.into()); }
        *fake.bus_owned.borrow_mut() = true;
        Rig { dir, ctx, fake }
    }

    #[test]
    fn not_gnome_is_not_applicable() {
        let mut r = rig(true, true);
        r.ctx.session.current_desktop = Some("KDE".into());
        assert!(matches!(r.check(), CheckOutcome::NotApplicable(_)));
        assert!(r.fake.log.borrow().is_empty());
        let mut r = rig(true, true);
        r.ctx.session.has_session_bus = false;
        assert!(matches!(r.check(), CheckOutcome::NotApplicable(_)));
        let r = rig(true, true);
        *r.fake.no_gnome_extensions.borrow_mut() = true;
        assert!(matches!(r.check(), CheckOutcome::NotApplicable(_)));
    }

    #[test]
    fn first_check_switches_when_ding_enabled() {
        let r = rig(true, true);
        assert_eq!(r.check(), CheckOutcome::Switched);
        assert!(r.fake.position("disable ding@rastersoft.com") < r.fake.position("enable morphix-desktop@morphix"));
        let state = load_state(&r.ctx);
        assert!(state.switched && state.ding_was_enabled && state.fallback.is_none());
        assert_eq!(r.check(), CheckOutcome::Nothing);
    }

    #[test]
    fn first_check_respects_ding_disabled_by_user() {
        let r = rig(false, true);
        assert_eq!(r.check(), CheckOutcome::Nothing);
        assert!(r.fake.log.borrow().is_empty());
        assert!(!load_state(&r.ctx).switched);
    }

    #[test]
    fn unknown_fork_before_relogin_does_nothing() {
        let r = rig(true, false);
        assert_eq!(r.check(), CheckOutcome::Nothing);
        assert!(r.fake.log.borrow().is_empty());
    }

    #[test]
    fn missing_fork_does_nothing() {
        let r = rig(true, true);
        std::fs::remove_dir_all(r.ctx.fork_dir()).unwrap();
        assert_eq!(r.check(), CheckOutcome::Nothing);
    }

    #[test]
    fn dead_fork_falls_back_and_records_reason() {
        let r = rig(true, true);
        *r.fake.bus_owned.borrow_mut() = false;
        let outcome = r.check();
        assert!(matches!(outcome, CheckOutcome::FellBack(_)), "{outcome:?}");
        assert!(r.fake.position("disable morphix-desktop@morphix") < r.fake.position("enable ding@rastersoft.com"));
        assert_eq!(r.fake.count("notify"), 1);
        let state = load_state(&r.ctx);
        assert!(!state.switched);
        let fallback = state.fallback.unwrap();
        assert_eq!(fallback.morphix_version, "0.1.7");
        assert!(!fallback.reason.is_empty());
        assert!(r.fake.enabled.borrow().contains(DING_UUID) && !r.fake.enabled.borrow().contains(FORK_UUID));
    }

    #[test]
    fn fallback_not_retried_until_source_or_version_changes() {
        let r = rig(true, true);
        *r.fake.bus_owned.borrow_mut() = false;
        r.check();
        r.fake.log.borrow_mut().clear();
        *r.fake.bus_owned.borrow_mut() = true;
        assert_eq!(r.check(), CheckOutcome::Nothing);
        assert!(r.fake.log.borrow().is_empty());
    }

    #[test]
    fn fallback_retried_after_version_change() {
        let mut r = rig(true, true);
        *r.fake.bus_owned.borrow_mut() = false;
        r.check();
        *r.fake.bus_owned.borrow_mut() = true;
        r.ctx.version = "0.1.8".into();
        assert_eq!(r.check(), CheckOutcome::Switched);
        assert_eq!(read_marker(&r.ctx.fork_dir()).unwrap().morphix_version, "0.1.8");
        assert!(load_state(&r.ctx).fallback.is_none());
    }

    #[test]
    fn rebuilds_when_the_source_changes_and_keeps_running_fork() {
        let r = rig(true, true);
        assert_eq!(r.check(), CheckOutcome::Switched);
        let ext = r.ctx.data_dirs[0].join("gnome-shell/extensions/ding@rastersoft.com/metadata.json");
        let text = std::fs::read_to_string(&ext).unwrap();
        std::fs::write(&ext, text.replace("\"47\"", "\"47\", \"48\"")).unwrap();
        r.fake.log.borrow_mut().clear();
        assert_eq!(r.check(), CheckOutcome::Rebuilt);
        assert!(r.fake.log.borrow().is_empty());
    }

    #[test]
    fn user_opt_out_is_respected() {
        let r = rig(true, true);
        r.check();
        r.fake.enabled.borrow_mut().remove(FORK_UUID);
        r.fake.enabled.borrow_mut().remove(DING_UUID);
        r.fake.log.borrow_mut().clear();
        assert_eq!(r.check(), CheckOutcome::Nothing);
        assert!(r.fake.log.borrow().is_empty());
    }

    #[test]
    fn both_enabled_yields_to_ding() {
        let r = rig(true, true);
        r.check();
        r.fake.enabled.borrow_mut().insert(DING_UUID.into());
        r.fake.log.borrow_mut().clear();
        r.check();
        assert_eq!(r.fake.log.borrow().as_slice(), ["disable morphix-desktop@morphix"]);
    }

    #[test]
    fn source_missing_disables_the_fork_and_restores_ding() {
        let r = rig(true, true);
        r.check();
        std::fs::remove_dir_all(&r.ctx.data_dirs[0]).unwrap();
        assert!(matches!(r.check(), CheckOutcome::FellBack(_)));
        assert!(r.fake.enabled.borrow().contains(DING_UUID));
    }

    #[test]
    fn corrupt_state_file_is_treated_as_fresh() {
        let r = rig(true, true);
        std::fs::create_dir_all(r.ctx.state_path().parent().unwrap()).unwrap();
        std::fs::write(r.ctx.state_path(), "{not json").unwrap();
        assert_eq!(r.check(), CheckOutcome::Switched);
    }

    #[test]
    fn restore_reenables_ding_only_if_it_was_enabled() {
        let r = rig(true, true);
        r.check();
        r.fake.log.borrow_mut().clear();
        let tools = r.tools();
        restore(&r.ctx, &tools).unwrap();
        assert!(r.fake.position("disable morphix-desktop@morphix") < r.fake.position("enable ding@rastersoft.com"));

        let r = rig(false, true);
        let tools = r.tools();
        restore(&r.ctx, &tools).unwrap();
        assert_eq!(r.fake.count("enable ding@rastersoft.com"), 0);
        let _ = (&r.dir, r.fork_exists());
    }
}
