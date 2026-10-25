pub mod clipboard;
pub mod x11;

use std::{fmt, time::Duration};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    Files,
    Desktop,
    Other,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Ctrl,
    Shift,
    C,
    F7,
}
#[derive(Debug, PartialEq, Eq)]
pub enum CaptureError {
    Unsupported(String),
    ModifiersStuck,
    KeysFailed(String),
    NoFiles,
    ClipboardUnsafe(String),
    Timeout(String),
    Other(String),
}
impl fmt::Display for CaptureError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ModifiersStuck => f.write_str("Hãy thả Alt, Ctrl, Shift và Super rồi thử lại."),
            Self::NoFiles => f.write_str("Không nhận được file nào. Hãy chọn file rồi thử lại."),
            Self::ClipboardUnsafe(reason) => {
                write!(f, "Không thể mượn clipboard an toàn: {reason}")
            }
            Self::KeysFailed(reason) => write!(f, "Không bấm được phím: {reason}"),
            Self::Unsupported(reason) | Self::Timeout(reason) | Self::Other(reason) => {
                f.write_str(reason)
            }
        }
    }
}
impl std::error::Error for CaptureError {}

pub trait KeyInjector {
    fn modifiers_held(&mut self) -> Result<bool, CaptureError>;
    fn key_event(&mut self, key: Key, pressed: bool) -> Result<(), CaptureError>;
}

pub fn classify_window(classes: &[String], title: &str) -> Target {
    if classes
        .iter()
        .any(|c| c.eq_ignore_ascii_case("org.gnome.Nautilus"))
    {
        Target::Files
    } else if classes.iter().any(|c| c.eq_ignore_ascii_case("gjs"))
        && title.starts_with("Desktop Icons")
    {
        Target::Desktop
    } else {
        Target::Other
    }
}

pub fn wait_modifiers_released(
    injector: &mut dyn KeyInjector,
    sleep: &mut dyn FnMut(Duration),
    max: Duration,
) -> Result<(), CaptureError> {
    let interval = Duration::from_millis(20);
    let mut elapsed = Duration::ZERO;
    while injector.modifiers_held()? {
        if elapsed >= max {
            return Err(CaptureError::ModifiersStuck);
        }
        let step = interval.min(max - elapsed);
        sleep(step);
        elapsed += step;
    }
    Ok(())
}

/// Chờ tới khi `done` đúng hoặc hết `max`, kiểm mỗi 20ms. Trả `done` lần cuối.
fn poll_until(
    max: Duration,
    sleep: &mut dyn FnMut(Duration),
    done: &mut dyn FnMut() -> bool,
) -> bool {
    let interval = Duration::from_millis(20);
    let mut elapsed = Duration::ZERO;
    while !done() {
        if elapsed >= max {
            return false;
        }
        let step = interval.min(max - elapsed);
        sleep(step);
        elapsed += step;
    }
    true
}

pub fn press_with_gaps(
    injector: &mut dyn KeyInjector,
    chord: &[Key],
    sleep: &mut dyn FnMut(Duration),
) -> Result<(), CaptureError> {
    let mut held = Vec::new();
    let result = (|| {
        for key in chord {
            injector.key_event(*key, true)?;
            held.push(*key);
            sleep(Duration::from_millis(30));
        }
        Ok(())
    })();
    let mut release_result = Ok(());
    // WHY: lỗi giữa tổ hợp vẫn phải thả mọi phím đã nhấn, tránh kẹt Ctrl/Shift.
    for key in held.into_iter().rev() {
        if let Err(error) = injector.key_event(key, false) {
            tracing::error!(%error, "không thả được phím");
            release_result = Err(error);
        }
        sleep(Duration::from_millis(30));
    }
    result.and(release_result)
}

/// Lệnh hỏi phiên bản Nautilus theo từng trình quản lý gói, như `morphix doctor`.
/// WHY không chạy `nautilus --version`: lệnh đó khởi động GTK/Tracker và ghi vào thư mục dữ liệu.
const NAUTILUS_VERSION_QUERIES: &[(&str, &[&str])] = &[
    ("dpkg-query", &["-W", "-f=${Version}", "nautilus"]),
    ("rpm", &["-q", "--qf", "%{VERSION}", "nautilus"]),
    ("pacman", &["-Q", "nautilus"]),
];

/// Phiên bản chính của Nautilus từ trình quản lý gói đầu tiên trả lời được; không có thì `None`.
fn nautilus_version_major() -> Option<u32> {
    NAUTILUS_VERSION_QUERIES.iter().find_map(|(program, args)| {
        match std::process::Command::new(program).args(*args).output() {
            Ok(output) if output.status.success() => {
                nautilus_major(&String::from_utf8_lossy(&output.stdout))
            }
            Ok(output) => {
                tracing::debug!(program, status = ?output.status, "trình quản lý gói không biết gói nautilus");
                None
            }
            Err(error) => {
                tracing::debug!(program, %error, "không có trình quản lý gói này");
                None
            }
        }
    })
}

/// Số phiên bản chính: `1:46.4-0ubuntu1` (dpkg), `46.4` (rpm), `nautilus 46.4-1` (pacman) đều cho 46.
fn nautilus_major(output: &str) -> Option<u32> {
    output
        .split_whitespace()
        .last()?
        .rsplit(':')
        .next()?
        .split('.')
        .next()?
        .parse()
        .ok()
}

#[derive(Debug, PartialEq, Eq)]
pub enum Plan {
    WaitForNautilusScript,
    ClipboardCopy,
    EmptyKhay,
}
pub fn plan_for(target: Target, nautilus_major: Option<u32>) -> Result<Plan, CaptureError> {
    match target {
        Target::Files if nautilus_major.is_some_and(|v| v >= 46) => Ok(Plan::WaitForNautilusScript),
        Target::Files => Err(CaptureError::Unsupported(
            "Chưa hỗ trợ trên phiên bản Nautilus này (cần 46 trở lên).".into(),
        )),
        Target::Desktop => Ok(Plan::ClipboardCopy),
        Target::Other => Ok(Plan::EmptyKhay),
    }
}

pub fn run(app: &tauri::AppHandle) {
    use std::sync::atomic::Ordering;
    use tauri::Manager;
    let state = app.state::<crate::state::AppState>();
    if state.capturing.swap(true, Ordering::SeqCst) {
        return;
    }
    let handle = app.clone();
    std::thread::spawn(move || {
        // WHY catch_unwind: panic giữa chừng (GTK, x11rb) mà không trả cờ thì mọi Alt+C sau bị bỏ qua
        // im lặng tới khi khởi động lại, và wait còn sót sẽ kéo lô file kế tiếp vào Khay.
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| capture(&handle)))
            .unwrap_or_else(|_| {
                Err(CaptureError::Other(
                    "lỗi nội bộ khi lấy file đang chọn, hãy thử lại".into(),
                ))
            });
        let state = handle.state::<crate::state::AppState>();
        state.khay_wait_guard().take();
        state.capturing.store(false, Ordering::SeqCst);
        if let Err(error) = result {
            if let Err(reason) = crate::windows::open_khay_with_error(&handle, error.to_string()) {
                tracing::error!(%reason, "không hiện được lỗi lấy file");
            }
        }
    });
}

fn capture(app: &tauri::AppHandle) -> Result<(), CaptureError> {
    use crate::{
        state::{AppState, KhayWait},
        windows,
    };
    use std::time::Instant;
    use tauri::Manager;
    if std::env::var("XDG_SESSION_TYPE").is_ok_and(|value| value == "wayland") {
        return Err(CaptureError::Unsupported(
            "Lấy file đang chọn hiện chỉ hỗ trợ X11.".into(),
        ));
    }
    let mut session = x11::X11Session::connect()?;
    wait_modifiers_released(
        &mut session,
        &mut std::thread::sleep,
        Duration::from_secs(3),
    )?;
    let target = session.active_window_target()?;
    let major = if target == Target::Files {
        nautilus_version_major()
    } else {
        None
    };
    ensure_target(&mut session, target)?;
    match plan_for(target, major)? {
        Plan::WaitForNautilusScript => {
            let state = app.state::<AppState>();
            let deadline = Instant::now() + Duration::from_secs(2);
            *state.khay_wait_guard() = Some(KhayWait { deadline });
            if let Err(error) = press_with_gaps(
                &mut session,
                &[Key::Ctrl, Key::Shift, Key::F7],
                &mut std::thread::sleep,
            ) {
                state.khay_wait_guard().take();
                return Err(error);
            }
            // WHY: lô tới thì route_for_batch đã lấy wait; thoát ngay để Alt+C kế tiếp không bị bỏ qua.
            poll_until(
                deadline.saturating_duration_since(Instant::now()),
                &mut std::thread::sleep,
                &mut || state.khay_wait_guard().is_none(),
            );
            let timed_out = state.khay_wait_guard().take().is_some();
            if timed_out {
                return Err(CaptureError::NoFiles);
            }
        }
        Plan::ClipboardCopy => {
            let handle = app.clone();
            let (sender, receiver) = std::sync::mpsc::sync_channel(1);
            app.run_on_main_thread(move || {
                let result = (|| {
                    let mut clipboard = clipboard::GtkClipboard::new()?;
                    clipboard::capture_selection_via_clipboard(
                        &mut clipboard,
                        &mut || {
                            ensure_target(&mut session, Target::Desktop)?;
                            press_with_gaps(
                                &mut session,
                                &[Key::Ctrl, Key::C],
                                &mut std::thread::sleep,
                            )
                        },
                        &mut clipboard::gtk_sleep,
                        Duration::from_secs(1),
                    )
                })();
                let displayed = match result {
                    Ok(paths) => windows::open_khay_with_files(&handle, paths),
                    Err(error) => windows::open_khay_with_error(&handle, error.to_string()),
                };
                if let Err(error) = sender.send(displayed) {
                    tracing::error!(%error, "luồng lấy file đã dừng");
                }
            })
            .map_err(|e| CaptureError::Other(e.to_string()))?;
            receiver
                .recv()
                .map_err(|e| CaptureError::Other(e.to_string()))?
                .map_err(CaptureError::Other)?;
        }
        Plan::EmptyKhay => {
            windows::open_khay_with_files(app, Vec::new()).map_err(CaptureError::Other)?
        }
    }
    Ok(())
}

fn ensure_target(session: &mut x11::X11Session, expected: Target) -> Result<(), CaptureError> {
    if session.active_window_target()? != expected || session.modifiers_held()? {
        return Err(CaptureError::Other(
            "Cửa sổ hoặc phím đang giữ đã thay đổi. Hãy thả phím rồi thử lại.".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;

    #[test]
    fn nautilus_major_reads_dpkg_rpm_and_pacman_versions() {
        assert_eq!(nautilus_major("1:46.4-0ubuntu1"), Some(46));
        assert_eq!(nautilus_major("47.1"), Some(47));
        assert_eq!(nautilus_major("nautilus 46.2-1\n"), Some(46));
        assert_eq!(nautilus_major(""), None);
    }

    struct FakeKeys {
        modifiers: VecDeque<bool>,
        events: Vec<(Key, bool)>,
        fail_key: Option<Key>,
    }
    impl KeyInjector for FakeKeys {
        fn modifiers_held(&mut self) -> Result<bool, CaptureError> {
            Ok(self.modifiers.pop_front().unwrap_or(true))
        }
        fn key_event(&mut self, key: Key, pressed: bool) -> Result<(), CaptureError> {
            if self.fail_key == Some(key) && pressed {
                return Err(CaptureError::KeysFailed("test".into()));
            }
            self.events.push((key, pressed));
            Ok(())
        }
    }
    #[test]
    fn classifies_nautilus_gjs_desktop_and_morphix() {
        assert_eq!(
            classify_window(&["org.gnome.Nautilus".into()], "Files"),
            Target::Files
        );
        assert_eq!(
            classify_window(&["gjs".into(), "Gjs".into()], "Desktop Icons 1"),
            Target::Desktop
        );
        for (class, title) in [
            ("Gjs", "Other"),
            ("morphix-gui", "Khay"),
            ("firefox", "Firefox"),
        ] {
            assert_eq!(classify_window(&[class.into()], title), Target::Other);
        }
    }
    #[test]
    fn wait_modifiers_releases_or_times_out_without_real_sleep() {
        let mut keys = FakeKeys {
            modifiers: [true, true, false].into(),
            events: vec![],
            fail_key: None,
        };
        let mut waited = Duration::ZERO;
        assert!(
            wait_modifiers_released(&mut keys, &mut |d| waited += d, Duration::from_secs(3))
                .is_ok()
        );
        assert_eq!(waited, Duration::from_millis(40));
        waited = Duration::ZERO;
        assert_eq!(
            wait_modifiers_released(&mut keys, &mut |d| waited += d, Duration::from_secs(3)),
            Err(CaptureError::ModifiersStuck)
        );
        assert_eq!(waited, Duration::from_secs(3));
        assert!(keys.events.is_empty());
    }
    #[test]
    fn poll_until_stops_early_or_at_max() {
        let mut slept = Duration::ZERO;
        let mut checks = 0;
        assert!(poll_until(
            Duration::from_secs(2),
            &mut |d| slept += d,
            &mut || {
                checks += 1;
                checks > 3
            }
        ));
        assert_eq!(slept, Duration::from_millis(60));
        slept = Duration::ZERO;
        assert!(!poll_until(
            Duration::from_millis(50),
            &mut |d| slept += d,
            &mut || false
        ));
        assert_eq!(slept, Duration::from_millis(50));
    }

    #[test]
    fn press_with_gaps_orders_and_pauses_and_releases_on_error() {
        let mut keys = FakeKeys {
            modifiers: [].into(),
            events: vec![],
            fail_key: None,
        };
        let mut pauses = vec![];
        press_with_gaps(&mut keys, &[Key::Ctrl, Key::Shift, Key::F7], &mut |d| {
            pauses.push(d)
        })
        .unwrap();
        assert_eq!(
            keys.events,
            [
                (Key::Ctrl, true),
                (Key::Shift, true),
                (Key::F7, true),
                (Key::F7, false),
                (Key::Shift, false),
                (Key::Ctrl, false)
            ]
        );
        assert_eq!(pauses, vec![Duration::from_millis(30); 6]);
        keys.events.clear();
        keys.fail_key = Some(Key::F7);
        assert!(
            press_with_gaps(&mut keys, &[Key::Ctrl, Key::Shift, Key::F7], &mut |_| {}).is_err()
        );
        assert_eq!(
            keys.events,
            [
                (Key::Ctrl, true),
                (Key::Shift, true),
                (Key::Shift, false),
                (Key::Ctrl, false)
            ]
        );
    }
    #[test]
    fn plan_files_desktop_and_other() {
        assert_eq!(
            plan_for(Target::Files, Some(46)).unwrap(),
            Plan::WaitForNautilusScript
        );
        assert!(matches!(
            plan_for(Target::Files, Some(42)),
            Err(CaptureError::Unsupported(_))
        ));
        assert!(plan_for(Target::Files, None).is_err());
        assert_eq!(
            plan_for(Target::Desktop, None).unwrap(),
            Plan::ClipboardCopy
        );
        assert_eq!(plan_for(Target::Other, None).unwrap(), Plan::EmptyKhay);
    }
}
