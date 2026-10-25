// WHY unix: các helper và test dùng fs/thread/Instant chỉ có trong nhánh unix.
#[cfg(unix)]
use std::{
    fs,
    path::Path,
    thread,
    time::{Duration, Instant},
};

use assert_cmd::cargo::cargo_bin_cmd;
use predicates::str::contains;

/// Thư mục có khoảng trắng, dấu nháy kép và `&` để chứng minh path đi nguyên vẹn, không qua shell.
#[cfg(unix)]
const AWKWARD_DIR_NAME: &str = "Apps \"v1\" & it's";

/// Script giả ghi `$0` và argv vào file trong `MORPHIX_TEST_RECORD` rồi ngủ lâu; nếu CLI chờ child thì test quá thời gian.
/// WHY env: đường dẫn ghi có dấu nháy đơn nên không nhúng vào script shell được.
#[cfg(unix)]
fn fake_gui(dir: &Path) -> std::path::PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let gui = dir.join("morphix-gui");
    // WHY một printf: ghi cả argv trong một lần, để người đọc không thấy file dở dang.
    let script =
        "#!/bin/sh\nprintf '%s\\n' \"$0\" \"$#\" \"$@\" > \"$MORPHIX_TEST_RECORD\"\nsleep 15\n";
    fs::write(&gui, script).unwrap();
    fs::set_permissions(&gui, fs::Permissions::from_mode(0o755)).unwrap();
    gui
}

#[cfg(unix)]
fn wait_for_record(record: &Path) -> String {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Ok(text) = fs::read_to_string(record) {
            if text.ends_with('\n') {
                return text;
            }
        }
        assert!(Instant::now() < deadline, "GUI giả không ghi argv");
        thread::sleep(Duration::from_millis(20));
    }
}

#[cfg(unix)]
#[test]
fn tray_spawns_gui_with_exactly_show_khay_and_an_absolute_path_with_spaces() {
    let d = tempfile::tempdir().unwrap();
    let dir = d.path().join(AWKWARD_DIR_NAME);
    fs::create_dir_all(&dir).unwrap();
    let gui = fake_gui(&dir);
    let record = dir.join("argv.txt");
    cargo_bin_cmd!("morphix")
        .env("MORPHIX_GUI_BIN", &gui)
        .env("MORPHIX_TEST_RECORD", &record)
        .arg("tray")
        .assert()
        .success();
    // WHY chờ: CLI không đợi GUI, nên file argv có thể chưa được ghi khi `tray` vừa thoát.
    let recorded = wait_for_record(&record);
    let lines: Vec<&str> = recorded.lines().collect();
    assert_eq!(
        lines,
        [gui.to_str().unwrap(), "1", "--show-khay"],
        "{recorded:?}"
    );
    assert!(Path::new(lines[0]).is_absolute());
}

#[cfg(unix)]
#[test]
fn tray_returns_without_waiting_for_the_gui_process() {
    let d = tempfile::tempdir().unwrap();
    let gui = fake_gui(d.path());
    let started = Instant::now();
    cargo_bin_cmd!("morphix")
        .env("MORPHIX_GUI_BIN", &gui)
        .env("MORPHIX_TEST_RECORD", d.path().join("argv.txt"))
        .arg("tray")
        .assert()
        .success();
    assert!(
        started.elapsed().as_secs() < 10,
        "tray đã chờ GUI: {:?}",
        started.elapsed()
    );
}

#[test]
fn tray_without_a_gui_binary_prints_a_reason_and_exits_1() {
    cargo_bin_cmd!("morphix")
        .env("MORPHIX_GUI_BIN", "/không/tồn/tại/morphix-gui")
        .arg("tray")
        .assert()
        .code(1)
        .stderr(contains("không tìm thấy morphix-gui"));
}

#[cfg(unix)]
#[test]
fn tray_selection_flag_spawns_gui_with_capture_flag() {
    let dir = tempfile::tempdir().unwrap();
    let gui = fake_gui(dir.path());
    let record = dir.path().join("argv.txt");
    cargo_bin_cmd!("morphix")
        .env("MORPHIX_GUI_BIN", &gui)
        .env("MORPHIX_TEST_RECORD", &record)
        .args(["tray", "--selection"])
        .assert()
        .success();
    assert_eq!(
        wait_for_record(&record).lines().collect::<Vec<_>>(),
        [gui.to_str().unwrap(), "1", "--capture-selection"]
    );
}

#[cfg(unix)]
#[test]
fn tray_unsupported_selection_opens_khay_with_fixed_error_state() {
    let dir = tempfile::tempdir().unwrap();
    let gui = fake_gui(dir.path());
    let record = dir.path().join("argv.txt");
    cargo_bin_cmd!("morphix")
        .env("MORPHIX_GUI_BIN", &gui)
        .env("MORPHIX_TEST_RECORD", &record)
        .args(["tray", "--unsupported-selection"])
        .assert()
        .success();
    assert_eq!(
        wait_for_record(&record).lines().collect::<Vec<_>>(),
        [
            gui.to_str().unwrap(),
            "2",
            "--show-khay",
            "--unsupported-selection"
        ]
    );
}
