use interprocess::local_socket::{prelude::*, Stream};
#[cfg(unix)]
use interprocess::local_socket::GenericFilePath;
#[cfg(not(unix))]
use interprocess::local_socket::GenericNamespaced;
use morphix_ipc::{decode, encode, Batcher, IpcServer, MenuRequest};
use std::io::{self, ErrorKind, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

#[test]
fn round_trip_keeps_unicode_quotes_spaces_and_newlines_in_names() {
    let req = MenuRequest {
        paths: vec![
            PathBuf::from("/home/a/Ảnh chụp 1.jpg"),
            PathBuf::from("/home/a/it's \"quoted\".png"),
            PathBuf::from("/home/a/ghi\nchú.jpg"),
        ],
    };
    let line = encode(&req);
    assert!(line.ends_with('\n'));
    assert_eq!(line.matches('\n').count(), 1, "xuống dòng trong tên phải được thoát");
    assert_eq!(decode(line.trim_end()).unwrap(), req);
}

#[test]
fn decode_rejects_garbage_with_a_reason() {
    assert!(!decode("").unwrap_err().is_empty());
    assert!(decode("{ broken").is_err());
}

#[cfg(unix)]
#[test]
fn non_utf8_path_is_lossy_only_at_the_ipc_boundary() {
    use std::os::unix::ffi::OsStringExt;
    let path = PathBuf::from(std::ffi::OsString::from_vec(vec![b'/', b'a', 0xff]));
    let decoded = decode(encode(&MenuRequest { paths: vec![path] }).trim_end()).unwrap();
    assert_eq!(decoded.paths, vec![PathBuf::from("/a\u{fffd}")]);
}

#[test]
fn batcher_merges_requests_inside_the_window_and_dedups() {
    let t0 = Instant::now();
    let mut b = Batcher::new(Duration::from_millis(300));
    b.push(vec![PathBuf::from("a.jpg"), PathBuf::from("b.jpg")], t0);
    b.push(
        vec![PathBuf::from("b.jpg"), PathBuf::from("c.jpg")],
        t0 + Duration::from_millis(100),
    );
    assert!(b.take_ready(t0 + Duration::from_millis(250)).is_none());
    assert_eq!(
        b.take_ready(t0 + Duration::from_millis(350)).unwrap(),
        vec![PathBuf::from("a.jpg"), PathBuf::from("b.jpg"), PathBuf::from("c.jpg")]
    );
}

#[test]
fn batcher_keeps_forty_files_from_two_fast_clicks() {
    let t0 = Instant::now();
    let mut b = Batcher::new(Duration::from_millis(300));
    let first: Vec<_> = (0..20).map(|i| PathBuf::from(format!("{i}.jpg"))).collect();
    let second: Vec<_> = (20..40).map(|i| PathBuf::from(format!("{i}.jpg"))).collect();
    b.push(first, t0);
    b.push(second, t0 + Duration::from_millis(50));
    assert_eq!(b.take_ready(t0 + Duration::from_secs(1)).unwrap().len(), 40);
}

#[test]
fn server_and_client_talk_over_a_real_local_socket() {
    let name = format!("morphix-test-{}", std::process::id());
    let got = Arc::new(Mutex::new(Vec::<(bool, Vec<PathBuf>)>::new()));
    let pending_sink = got.clone();
    let final_sink = got.clone();
    let server = morphix_ipc::IpcServer::start(
        &name,
        move |paths| pending_sink.lock().unwrap().push((false, paths)),
        move |paths| final_sink.lock().unwrap().push((true, paths)),
    ).unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let sent = morphix_ipc::send_request(&name, &MenuRequest { paths: vec![PathBuf::from("x.png")] });
        if sent.is_ok() {
            break;
        }
        assert!(Instant::now() < deadline, "server không lên");
        std::thread::sleep(Duration::from_millis(50));
    }
    while !got.lock().unwrap().iter().any(|(is_final, _)| !is_final) {
        assert!(Instant::now() < deadline + Duration::from_secs(3), "lô không tới");
        std::thread::sleep(Duration::from_millis(20));
    }
    let pending = got.lock().unwrap()[0].1.clone();
    assert_eq!(pending, vec![PathBuf::from("x.png")]);
    while !got.lock().unwrap().iter().any(|(is_final, _)| *is_final) {
        assert!(Instant::now() < deadline + Duration::from_secs(3), "lô chưa được chốt");
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(got.lock().unwrap().last().unwrap().1, pending);
    server.shutdown().unwrap();
}

#[test]
fn shutdown_releases_the_socket_name_for_a_new_server() {
    let name = format!("morphix-shutdown-test-{}", std::process::id());
    let first = morphix_ipc::IpcServer::start(&name, |_| {}, |_| {}).unwrap();
    first.shutdown().unwrap();
    let second = morphix_ipc::IpcServer::start(&name, |_| {}, |_| {}).unwrap();
    second.shutdown().unwrap();
}

/// Raw client socket, so tests can send bytes that `send_request` would never produce.
fn connect_raw(name: &str) -> io::Result<Stream> {
    #[cfg(unix)]
    let socket_name = name.to_fs_name::<GenericFilePath>()?;
    #[cfg(not(unix))]
    let socket_name = name.to_ns_name::<GenericNamespaced>()?;
    Stream::connect(socket_name)
}

fn menu_request(paths: &[&str]) -> MenuRequest {
    MenuRequest {
        paths: paths.iter().map(PathBuf::from).collect(),
    }
}

fn wait_until(deadline: Instant, condition: impl Fn() -> bool) -> bool {
    while Instant::now() < deadline {
        if condition() {
            return true;
        }
        thread::sleep(Duration::from_millis(10));
    }
    condition()
}

fn start_pending_recorder(name: &str) -> (IpcServer, Arc<Mutex<Vec<Vec<PathBuf>>>>) {
    let received = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&received);
    let server = IpcServer::start(
        name,
        move |paths| sink.lock().unwrap().push(paths),
        |_| {},
    )
    .unwrap();
    (server, received)
}

/// Mirrors `CLIENT_READ_DEADLINE` in the crate, which is private to the library.
const SERVER_READ_DEADLINE: Duration = Duration::from_secs(2);

#[test]
fn oversized_line_without_newline_is_dropped_quickly_and_server_keeps_serving() {
    let name = format!("morphix-oversize-test-{}", std::process::id());
    let (server, received) = start_pending_recorder(&name);
    // Bigger than the 1 MiB cap, so the server must give up before the client finishes writing.
    let total_bytes = 4 * 1024 * 1024;
    let chunk = vec![b'a'; 64 * 1024];
    let mut oversized = connect_raw(&name).unwrap();
    let started = Instant::now();
    let mut written = 0usize;
    while written < total_bytes {
        match oversized.write_all(&chunk) {
            Ok(()) => written += chunk.len(),
            // The server closing its side after the cap is the expected outcome, not a test failure.
            Err(error)
                if error.kind() == ErrorKind::BrokenPipe
                    || error.kind() == ErrorKind::ConnectionReset =>
            {
                break
            }
            Err(error) => panic!("unexpected write error: {error}"),
        }
    }
    assert!(written < total_bytes, "server must close the oversized connection");

    let valid = menu_request(&["ok.png"]);
    morphix_ipc::send_request(&name, &valid).unwrap();
    let answered = wait_until(Instant::now() + Duration::from_secs(5), || {
        received.lock().unwrap().contains(&valid.paths)
    });
    // Without the cap the server would wait out the full read deadline, so anything near it means the cap failed.
    assert!(answered, "valid request must be served after the rejection");
    assert!(started.elapsed() < SERVER_READ_DEADLINE, "rejection took {:?}", started.elapsed());
    server.shutdown().unwrap();
}

#[test]
fn slow_trickling_client_does_not_hold_the_server_past_the_deadline() {
    let name = format!("morphix-trickle-test-{}", std::process::id());
    let (server, received) = start_pending_recorder(&name);
    // Connected first, so the server accepts this client before the valid request.
    let mut trickler = connect_raw(&name).unwrap();
    // One byte every 100 ms keeps each individual read well inside any per-read timeout,
    // so only a total deadline can cut this client off.
    let trickle = thread::spawn(move || {
        for _ in 0..100 {
            // A failed write means the server dropped us at the deadline, which is what we want.
            if trickler.write_all(b"{").is_err() {
                break;
            }
            thread::sleep(Duration::from_millis(100));
        }
    });
    let valid = menu_request(&["late.png"]);
    let started = Instant::now();
    morphix_ipc::send_request(&name, &valid).unwrap();
    let answered = wait_until(Instant::now() + Duration::from_secs(10), || {
        received.lock().unwrap().contains(&valid.paths)
    });
    let elapsed = started.elapsed();
    assert!(answered, "valid request must be served once the trickler is cut off");
    // The deadline is measured from accept, so the valid request waits roughly one deadline; the slack covers poll and CI load.
    assert!(
        elapsed < SERVER_READ_DEADLINE + Duration::from_secs(1),
        "valid request waited {elapsed:?}"
    );
    trickle.join().unwrap();
    server.shutdown().unwrap();
}

#[test]
fn shutdown_flushes_the_batch_still_inside_its_window() {
    let name = format!("morphix-drain-test-{}", std::process::id());
    let pending: Arc<Mutex<Vec<Vec<PathBuf>>>> = Arc::new(Mutex::new(Vec::new()));
    let pending_sink = Arc::clone(&pending);
    let batches: Arc<Mutex<Vec<Vec<PathBuf>>>> = Arc::new(Mutex::new(Vec::new()));
    let batch_sink = Arc::clone(&batches);
    let server = IpcServer::start(
        &name,
        move |paths| pending_sink.lock().unwrap().push(paths),
        move |paths| batch_sink.lock().unwrap().push(paths),
    )
    .unwrap();
    let request = menu_request(&["a.png", "b.png"]);
    morphix_ipc::send_request(&name, &request).unwrap();
    // on_pending fires only after the path is in the batcher, so this is the point where shutdown can race with it.
    assert!(wait_until(Instant::now() + Duration::from_secs(5), || {
        !pending.lock().unwrap().is_empty()
    }));
    server.shutdown().unwrap();
    assert_eq!(*batches.lock().unwrap(), vec![request.paths]);
}
