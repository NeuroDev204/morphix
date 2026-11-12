#[cfg(unix)]
use std::path::PathBuf;

use assert_cmd::cargo::cargo_bin_cmd;
use morphix_cli::uri::file_uris_to_paths;
use predicates::str::contains;

#[cfg(windows)]
#[test]
fn menu_on_windows_points_to_explorer_and_exits_2() {
    let d = tempfile::tempdir().unwrap();
    let file = d.path().join("a.png");
    std::fs::write(&file, b"x").unwrap();
    cargo_bin_cmd!("morphix")
        .env(
            "MORPHIX_IPC_NAME",
            format!("morphix-cli-test-{}-windows", std::process::id()),
        )
        .env("MORPHIX_DATA_DIR", d.path().join("data"))
        .arg("menu")
        .arg(&file)
        .assert()
        .code(2)
        .stderr(contains("Morphix trong Explorer"));
}

#[cfg(unix)]
#[test]
fn menu_without_a_running_gui_reports_a_reason_and_exits_1() {
    let d = tempfile::tempdir().unwrap();
    let file = d.path().join("a.jpg");
    std::fs::write(&file, b"x").unwrap();
    cargo_bin_cmd!("morphix")
        .env("MORPHIX_GUI_BIN", "/không/tồn/tại/morphix-gui")
        .env(
            "MORPHIX_IPC_NAME",
            format!("morphix-cli-test-{}-missing", std::process::id()),
        )
        .env("MORPHIX_DATA_DIR", d.path().join("data"))
        .arg("menu")
        .arg(&file)
        .assert()
        .code(1)
        .stderr(contains("không khởi động được"));
}

#[cfg(unix)]
#[test]
fn menu_accepts_a_newline_separated_list_when_no_positional_paths_are_given() {
    let d = tempfile::tempdir().unwrap();
    let list = format!(
        "{}\n{}",
        d.path().join("a.jpg").display(),
        d.path().join("b.jpg").display()
    );
    cargo_bin_cmd!("morphix")
        .env("MORPHIX_GUI_BIN", "/không/tồn/tại")
        .env(
            "MORPHIX_IPC_NAME",
            format!("morphix-cli-test-{}-newline", std::process::id()),
        )
        .env("MORPHIX_DATA_DIR", d.path().join("data"))
        .args(["menu", "--newline-list", &list])
        .assert()
        .code(1)
        .stderr(contains("không khởi động được"));
}

#[cfg(unix)]
#[test]
fn menu_forwards_positional_and_newline_paths_to_the_running_server() {
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};

    let data = tempfile::tempdir().unwrap();
    let name = format!("morphix-cli-success-{}", std::process::id());
    let received = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&received);
    let server = morphix_ipc::IpcServer::start(
        &name,
        move |_paths| {},
        move |batch| sink.lock().unwrap().push(batch),
    )
    .unwrap();
    let expected = ["/tmp/ghi\nchú.jpg", "/tmp/Ảnh 2.png", "/tmp/a 3.jpg"];
    let list = format!("{}\n{}\n", expected[1], expected[2]);
    cargo_bin_cmd!("morphix")
        .env("MORPHIX_IPC_NAME", &name)
        .env("MORPHIX_DATA_DIR", data.path())
        .arg("menu")
        .arg(expected[0])
        .arg("--newline-list")
        .arg(list)
        .assert()
        .success();

    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(batch) = received.lock().unwrap().first() {
            assert_eq!(batch.len(), 3);
            assert_eq!(batch[0].to_string_lossy(), expected[0]);
            assert_eq!(batch[1].to_string_lossy(), expected[1]);
            assert_eq!(batch[2].to_string_lossy(), expected[2]);
            break;
        }
        assert!(Instant::now() < deadline, "server không nhận được yêu cầu");
        std::thread::sleep(Duration::from_millis(20));
    }
    server.shutdown().unwrap();
}

// ---- --uris: URI file: từ Dolphin (%U) ----

// WHY unix: URI `file:///tmp/...` chỉ có nghĩa với đường dẫn kiểu Unix; `--uris` chỉ do trình quản lý file Linux dùng.
#[cfg(unix)]
#[test]
fn file_uris_decode_spaces_hash_plus_and_unicode_into_pathbufs() {
    let uris = [
        "file:///tmp/a%20b.png".to_string(),
        "file:///tmp/%23tag.png".to_string(),
        "file:///tmp/c+d.png".to_string(),
        "file:///tmp/%E1%BA%A2nh%202.png".to_string(),
        "file:///tmp/Ảnh 3.png".to_string(),
    ];
    assert_eq!(
        file_uris_to_paths(&uris).unwrap(),
        vec![
            PathBuf::from("/tmp/a b.png"),
            PathBuf::from("/tmp/#tag.png"),
            PathBuf::from("/tmp/c+d.png"),
            PathBuf::from("/tmp/Ảnh 2.png"),
            PathBuf::from("/tmp/Ảnh 3.png"),
        ]
    );
}

// WHY unix: URI `file:///tmp/...` chỉ có nghĩa với đường dẫn kiểu Unix; `--uris` chỉ do trình quản lý file Linux dùng.
#[cfg(unix)]
#[test]
fn remote_malformed_and_non_local_uris_are_rejected_with_the_offending_uri() {
    for bad in [
        "https://example.com/a.png",
        "smb://server/share/a.png",
        "file://server/share/a.png",
        "/tmp/a.png",
        "not a uri",
    ] {
        let error = file_uris_to_paths(&[bad.to_string()]).unwrap_err();
        assert!(error.contains(bad), "{error}");
    }
}

#[test]
fn empty_uri_list_is_rejected() {
    assert!(file_uris_to_paths(&[]).is_err());
}

#[cfg(unix)]
#[test]
fn menu_uris_are_converted_before_reaching_the_running_server() {
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};

    let data = tempfile::tempdir().unwrap();
    let name = format!("morphix-cli-uris-{}", std::process::id());
    let received = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&received);
    let server = morphix_ipc::IpcServer::start(
        &name,
        move |_paths| {},
        move |batch| sink.lock().unwrap().push(batch),
    )
    .unwrap();
    cargo_bin_cmd!("morphix")
        .env("MORPHIX_IPC_NAME", &name)
        .env("MORPHIX_DATA_DIR", data.path())
        .args([
            "menu",
            "--uris",
            "file:///tmp/a%20b.png",
            "file:///tmp/%23x.png",
        ])
        .assert()
        .success();

    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(batch) = received.lock().unwrap().first() {
            assert_eq!(
                batch,
                &vec![PathBuf::from("/tmp/a b.png"), PathBuf::from("/tmp/#x.png")]
            );
            break;
        }
        assert!(Instant::now() < deadline, "server không nhận được yêu cầu");
        std::thread::sleep(Duration::from_millis(20));
    }
    server.shutdown().unwrap();
}

#[cfg(unix)]
#[test]
fn menu_uris_reject_remote_schemes_and_mixing_with_other_inputs_with_exit_2() {
    let d = tempfile::tempdir().unwrap();
    cargo_bin_cmd!("morphix")
        .env("MORPHIX_DATA_DIR", d.path())
        .args(["menu", "--uris", "https://example.com/a.png"])
        .assert()
        .code(2)
        .stderr(contains("file:"));
    for args in [
        vec!["menu", "--uris", "file:///tmp/a.png", "/tmp/b.png"],
        vec![
            "menu",
            "--uris",
            "file:///tmp/a.png",
            "--newline-list",
            "/tmp/b.png",
        ],
        vec!["menu", "--uris"],
    ] {
        cargo_bin_cmd!("morphix")
            .env("MORPHIX_DATA_DIR", d.path())
            .args(&args)
            .assert()
            .code(2);
    }
}

#[cfg(unix)]
#[test]
fn menu_with_a_non_utf8_file_name_explains_it_instead_of_starting_the_gui() {
    use std::os::unix::ffi::OsStrExt;
    let d = tempfile::tempdir().unwrap();
    let file = d.path().join(std::ffi::OsStr::from_bytes(b"\xffanh.jpg"));
    cargo_bin_cmd!("morphix")
        .env("MORPHIX_GUI_BIN", "/không/tồn/tại/morphix-gui")
        .env(
            "MORPHIX_IPC_NAME",
            format!("morphix-cli-test-{}-non-utf8", std::process::id()),
        )
        .env("MORPHIX_DATA_DIR", d.path().join("data"))
        .arg("menu")
        .arg(&file)
        .assert()
        .code(2)
        .stderr(contains("UTF-8"));
}

// ---- --list và --action: việc chạy thẳng từ menu chuột phải ----

fn listing(args: &[&str], dir: &std::path::Path) -> serde_json::Value {
    let output = cargo_bin_cmd!("morphix")
        .env("MORPHIX_DATA_DIR", dir.join("data"))
        .args(["--json", "menu", "--list", "--"])
        .args(args)
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    let text = String::from_utf8(output.stdout).unwrap();
    assert_eq!(text.lines().count(), 1, "phải đúng một dòng JSON: {text:?}");
    serde_json::from_str(&text).unwrap()
}

#[test]
fn list_json_for_one_png_contains_convert_webp() {
    let d = tempfile::tempdir().unwrap();
    let png = d.path().join("a.png");
    image::RgbImage::from_pixel(8, 8, image::Rgb([10, 120, 200]))
        .save(&png)
        .unwrap();
    let json = listing(&[png.to_str().unwrap()], d.path());
    assert_eq!(json["too_many"], false);
    let items = json["items"].as_array().unwrap();
    let webp = items.iter().find(|item| item["id"] == "convert:webp").unwrap();
    assert_eq!(webp["label"], "Đổi sang WebP");
    assert_eq!(webp["is_tool"], false);
    assert!(items.iter().any(|item| item["is_tool"] == true));
}

#[test]
fn list_json_for_101_files_reports_too_many() {
    let d = tempfile::tempdir().unwrap();
    let names: Vec<String> = (0..101).map(|n| format!("/không/có/{n}.png")).collect();
    let args: Vec<&str> = names.iter().map(String::as_str).collect();
    let json = listing(&args, d.path());
    assert_eq!(json["too_many"], true);
    assert!(json["items"].as_array().unwrap().is_empty());
}

#[test]
fn list_json_for_an_unrecognised_file_is_empty_not_an_error() {
    let d = tempfile::tempdir().unwrap();
    let rar = d.path().join("a.rar");
    std::fs::write(&rar, b"Rar!\x1A\x07\x00\xCF\x90").unwrap();
    let json = listing(&[rar.to_str().unwrap()], d.path());
    assert_eq!(json["too_many"], false);
    assert!(json["items"].as_array().unwrap().is_empty());
}

#[test]
fn list_and_action_conflict_and_exit_2() {
    cargo_bin_cmd!("morphix")
        .args(["menu", "--list", "--action", "convert:webp", "--", "/tmp/a.png"])
        .assert()
        .code(2);
}

fn serve_requests(
    name: &str,
) -> (
    morphix_ipc::IpcServer,
    std::sync::Arc<std::sync::Mutex<Vec<morphix_ipc::MenuRequest>>>,
) {
    let received = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let sink = std::sync::Arc::clone(&received);
    let server = morphix_ipc::IpcServer::start_requests(
        name,
        |_request| {},
        move |request| sink.lock().unwrap().push(request),
    )
    .unwrap();
    (server, received)
}

fn wait_for_request(
    received: &std::sync::Arc<std::sync::Mutex<Vec<morphix_ipc::MenuRequest>>>,
) -> morphix_ipc::MenuRequest {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        if let Some(request) = received.lock().unwrap().first() {
            return request.clone();
        }
        assert!(
            std::time::Instant::now() < deadline,
            "server không nhận được yêu cầu"
        );
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}

#[test]
fn action_request_carries_the_action_id_and_the_paths() {
    let data = tempfile::tempdir().unwrap();
    let name = format!("morphix-cli-action-{}", std::process::id());
    let (server, received) = serve_requests(&name);
    // WHY đường dẫn tuyệt đối thật: `/tmp/...` không tuyệt đối trên Windows nên bị đổi khi CLI chuẩn hoá.
    let files = [data.path().join("Ảnh 1.png"), data.path().join("b'c.png")];
    cargo_bin_cmd!("morphix")
        .env("MORPHIX_IPC_NAME", &name)
        .env("MORPHIX_DATA_DIR", data.path())
        .args(["menu", "--action", "convert:webp", "--"])
        .args(&files)
        .assert()
        .success();
    let request = wait_for_request(&received);
    assert_eq!(request.action.as_deref(), Some("convert:webp"));
    assert_eq!(request.paths, files);
    server.shutdown().unwrap();
}

#[cfg(unix)]
#[test]
fn action_request_starts_the_gui_when_nothing_is_listening() {
    use std::os::unix::fs::PermissionsExt;
    let data = tempfile::tempdir().unwrap();
    let marker = data.path().join("gui-started");
    let fake_gui = data.path().join("fake-gui");
    std::fs::write(&fake_gui, format!("#!/bin/sh\ntouch '{}'\n", marker.display())).unwrap();
    std::fs::set_permissions(&fake_gui, std::fs::Permissions::from_mode(0o755)).unwrap();
    let name = format!("morphix-cli-action-start-{}", std::process::id());

    // WHY trễ 600 ms: CLI phải gửi hụt lần đầu (chưa có server) rồi mới khởi động GUI và thử lại;
    // server dựng sau đó đóng vai GUI vừa mở xong socket.
    let late_name = name.clone();
    let late_server = std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(600));
        serve_requests(&late_name)
    });
    cargo_bin_cmd!("morphix")
        .env("MORPHIX_GUI_BIN", &fake_gui)
        .env("MORPHIX_IPC_NAME", &name)
        .env("MORPHIX_DATA_DIR", data.path().join("data"))
        .args(["menu", "--action", "compress", "--", "/tmp/a.png"])
        .assert()
        .success();
    let (server, received) = late_server.join().unwrap();
    assert!(marker.exists(), "CLI phải khởi động GUI khi chưa có ai nghe");
    assert_eq!(wait_for_request(&received).action.as_deref(), Some("compress"));
    server.shutdown().unwrap();
}

#[cfg(unix)]
#[test]
fn action_that_cannot_reach_the_gui_is_reported_with_a_desktop_notification() {
    use std::os::unix::fs::PermissionsExt;
    let d = tempfile::tempdir().unwrap();
    let bin = d.path().join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    let record = d.path().join("notify.txt");
    let notify = bin.join("notify-send");
    std::fs::write(
        &notify,
        format!("#!/bin/sh\nprintf '%s\\n' \"$@\" > '{}'\n", record.display()),
    )
    .unwrap();
    std::fs::set_permissions(&notify, std::fs::Permissions::from_mode(0o755)).unwrap();
    let path = std::env::join_paths(
        std::iter::once(bin).chain(std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default())),
    )
    .unwrap();
    cargo_bin_cmd!("morphix")
        .env("PATH", path)
        .env("MORPHIX_GUI_BIN", "/không/tồn/tại/morphix-gui")
        .env("MORPHIX_IPC_NAME", format!("morphix-cli-test-{}-notify", std::process::id()))
        .env("MORPHIX_DATA_DIR", d.path().join("data"))
        .args(["menu", "--action", "compress", "--", "/tmp/a.png"])
        .assert()
        .code(1)
        .stderr(contains("không khởi động được"));
    // WHY thông báo: bấm từ menu chuột phải không có terminal nào để đọc stderr.
    let text = std::fs::read_to_string(&record).expect("phải gọi notify-send");
    assert!(text.contains("Morphix") && text.contains("không khởi động được"), "{text}");
}

// ---- quit: yêu cầu GUI thoát êm khi gỡ hoặc nâng cấp cài đặt ----

#[test]
fn quit_sends_the_quit_request_to_a_running_gui() {
    let data = tempfile::tempdir().unwrap();
    let name = format!("morphix-cli-quit-{}", std::process::id());
    let (server, received) = serve_requests(&name);
    cargo_bin_cmd!("morphix")
        .env("MORPHIX_IPC_NAME", &name)
        .env("MORPHIX_DATA_DIR", data.path())
        .arg("quit")
        .assert()
        .success();
    let request = wait_for_request(&received);
    assert!(request.is_quit(), "{request:?}");
    assert!(request.paths.is_empty());
    server.shutdown().unwrap();
}

#[test]
fn quit_without_a_running_gui_succeeds_and_says_so() {
    let data = tempfile::tempdir().unwrap();
    cargo_bin_cmd!("morphix")
        .env("MORPHIX_IPC_NAME", format!("morphix-cli-quit-none-{}", std::process::id()))
        .env("MORPHIX_DATA_DIR", data.path())
        .arg("quit")
        .assert()
        .success()
        .stdout(contains("không đang chạy"));
}
