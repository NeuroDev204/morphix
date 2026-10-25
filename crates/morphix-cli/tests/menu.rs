use std::path::PathBuf;

use assert_cmd::cargo::cargo_bin_cmd;
use morphix_cli::uri::file_uris_to_paths;
use predicates::str::contains;

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
