use morphix_core::{ImgFormat, Job, MorphixError, Operation, Quality};
use morphix_gui::jobs::JobRunner;
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant};

fn png(dir: &std::path::Path, name: &str) -> std::path::PathBuf {
    let p = dir.join(name);
    image::RgbImage::from_pixel(8, 8, image::Rgb([1, 2, 3])).save(&p).unwrap();
    p
}

fn job(inputs: Vec<std::path::PathBuf>, quality: Quality) -> Job {
    Job {
        inputs,
        operation: Operation::ConvertImage { to: ImgFormat::Jpeg },
        quality,
        out_dir: None,
    }
}

fn wait_for_report(runner: &JobRunner, id: u64) -> morphix_core::JobReport {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        if let Some(report) = runner.take_report(id) {
            return report;
        }
        assert!(Instant::now() < deadline, "việc không kết thúc");
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn second_job_is_refused_while_one_is_running() {
    let d = tempfile::tempdir().unwrap();
    let runner = JobRunner::new();
    let inputs: Vec<_> = (0..30).map(|i| png(d.path(), &format!("f{i}.png"))).collect();
    let (reached_first_item, reached_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let release_rx = Mutex::new(release_rx);
    let callback = Arc::new(move |snapshot: morphix_gui::jobs::JobSnapshot| {
        if snapshot.done == 1 {
            reached_first_item.send(()).unwrap();
            release_rx.lock().unwrap().recv().unwrap();
        }
    });
    let id = runner.start(job(inputs, Quality::High), "Đổi sang JPG".into(), callback).unwrap();
    reached_rx.recv_timeout(Duration::from_secs(20)).unwrap();
    let second = runner.start(job(vec![png(d.path(), "b.png")], Quality::Balanced), "Nén ảnh".into(), Arc::new(|_| {}));
    assert!(second.is_err(), "việc thứ hai phải bị từ chối khi việc đầu còn chạy");
    release_tx.send(()).unwrap();
    wait_for_report(&runner, id);
}

#[test]
fn updates_reach_the_callback_and_finish_with_counts() {
    let d = tempfile::tempdir().unwrap();
    let runner = JobRunner::new();
    let seen = Arc::new(Mutex::new(Vec::new()));
    let sink = seen.clone();
    let inputs = vec![png(d.path(), "a.png"), png(d.path(), "b.png")];
    let id = runner
        .start(job(inputs, Quality::Balanced), "Đổi sang JPG".into(), Arc::new(move |s| sink.lock().unwrap().push(s)))
        .unwrap();
    let report = wait_for_report(&runner, id);
    assert_eq!((report.succeeded(), report.failed()), (2, 0));
    let updates = seen.lock().unwrap();
    assert!(updates.iter().any(|s| s.done == 2 && s.total == 2));
}

#[test]
fn cancel_marks_pending_items_cancelled() {
    let d = tempfile::tempdir().unwrap();
    let runner = JobRunner::new();
    let inputs: Vec<_> = (0..30).map(|i| png(d.path(), &format!("g{i}.png"))).collect();
    let (reached_first_item, reached_rx) = std::sync::mpsc::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let release_rx = Mutex::new(release_rx);
    let callback = Arc::new(move |snapshot: morphix_gui::jobs::JobSnapshot| {
        if snapshot.done == 1 {
            reached_first_item.send(()).unwrap();
            release_rx.lock().unwrap().recv().unwrap();
        }
    });
    let id = runner.start(job(inputs, Quality::High), "Đổi sang JPG".into(), callback).unwrap();
    reached_rx.recv_timeout(Duration::from_secs(20)).unwrap();
    assert!(runner.cancel(id));
    release_tx.send(()).unwrap();
    let report = wait_for_report(&runner, id);
    assert!(report.items.iter().any(|i| matches!(&i.result, Err(MorphixError::Cancelled))));
}

#[test]
fn describe_error_names_the_file_when_the_core_message_does_not() {
    use morphix_gui::jobs::describe_error;
    let e = MorphixError::InvalidInput("phần trăm phải từ 1 đến 100".into());
    let text = describe_error(&e, std::path::Path::new("/a/Ảnh.jpg"));
    assert!(text.contains("Ảnh.jpg"), "{text}");
    let corrupt = MorphixError::Corrupt {
        path: "/a/x.jpg".into(),
        reason: "EOF".into(),
    };
    assert!(describe_error(&corrupt, std::path::Path::new("/a/x.jpg")).contains("bị hỏng"));
}

#[test]
fn shutdown_cancels_and_joins_the_active_worker() {
    let d = tempfile::tempdir().unwrap();
    let runner = JobRunner::new();
    let inputs: Vec<_> = (0..30).map(|i| png(d.path(), &format!("s{i}.png"))).collect();
    let (reached_first_item, reached_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let release_rx = Mutex::new(release_rx);
    let callback = Arc::new(move |snapshot: morphix_gui::jobs::JobSnapshot| {
        if snapshot.done == 1 {
            reached_first_item.send(()).unwrap();
            release_rx.lock().unwrap().recv().unwrap();
        }
    });
    runner.start(job(inputs, Quality::High), "Đổi sang JPG".into(), callback).unwrap();
    reached_rx.recv_timeout(Duration::from_secs(20)).unwrap();
    runner.cancel(runner.active().unwrap().id);
    release_tx.send(()).unwrap();
    runner.shutdown().unwrap();
    assert!(runner.active().is_none());
}


#[test]
fn untaken_report_refuses_new_job_until_taken() {
    let d = tempfile::tempdir().unwrap();
    let runner = JobRunner::new();
    let inputs = vec![png(d.path(), "u.png")];
    let id = runner
        .start(job(inputs, Quality::Balanced), "Đổi sang JPG".into(), Arc::new(|_| {}))
        .unwrap();
    let report = wait_for_report(&runner, id);
    assert_eq!(report.succeeded(), 1);

    // Bắt đầu việc thứ hai ngay bây giờ phải thành công vì báo cáo đã được lấy
    let inputs2 = vec![png(d.path(), "u2.png")];
    let id2 = runner
        .start(job(inputs2, Quality::Balanced), "Đổi sang JPG".into(), Arc::new(|_| {}))
        .unwrap();

    // Đợi việc 2 xong nhưng KHÔNG lấy báo cáo
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        if runner.active().is_none() {
            break;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(10));
    }

    // Báo cáo việc 2 vẫn còn trong runner (chưa take_report) -> việc thứ 3 phải bị từ chối!
    let inputs3 = vec![png(d.path(), "u3.png")];
    let third = runner.start(job(inputs3, Quality::Balanced), "Đổi sang JPG".into(), Arc::new(|_| {}));
    assert!(third.is_err(), "việc thứ ba phải bị từ chối vì báo cáo việc hai chưa được lấy");

    // Lấy báo cáo việc 2
    let rep2 = runner.take_report(id2);
    assert!(rep2.is_some());
    // Lấy lần nữa phải trả về None
    assert!(runner.take_report(id2).is_none());

    // Thử lại việc 3, bây giờ phải thành công
    let inputs3 = vec![png(d.path(), "u3.png")];
    let id3 = runner
        .start(job(inputs3, Quality::Balanced), "Đổi sang JPG".into(), Arc::new(|_| {}))
        .unwrap();
    wait_for_report(&runner, id3);
}

#[test]
fn cancel_unknown_id_returns_false() {
    let runner = JobRunner::new();
    assert!(!runner.cancel(9999));
}

#[test]
fn describe_error_unsupported_and_not_smaller() {
    use morphix_gui::jobs::describe_error;
    let unsupp = MorphixError::Unsupported {
        path: "/b/file.xyz".into(),
    };
    let text_unsupp = describe_error(&unsupp, std::path::Path::new("/b/file.xyz"));
    assert!(text_unsupp.contains("không phải loại file được hỗ trợ"));

    let not_smaller = MorphixError::NotSmaller {
        path: "/b/photo.jpg".into(),
    };
    let text_ns = describe_error(&not_smaller, std::path::Path::new("/b/photo.jpg"));
    assert!(text_ns.contains("không nén thêm được"));
}


#[test]
fn shutdown_captures_panic_and_recovers_state() {
    let d = tempfile::tempdir().unwrap();
    let runner = JobRunner::new();
    let inputs = vec![png(d.path(), "p.png")];
    let callback = Arc::new(|_| {
        panic!("cố ý panic trong callback");
    });
    runner.start(job(inputs, Quality::Balanced), "Panic job".into(), callback).unwrap();
    // Chờ luồng worker chạy và gặp panic
    std::thread::sleep(Duration::from_millis(50));
    let shutdown_res = runner.shutdown();
    assert!(shutdown_res.is_err());
    let err_msg = shutdown_res.unwrap_err();
    assert!(err_msg.contains("panic"), "{err_msg}");
    // Sau khi shutdown, runner ở trạng thái sạch, có thể chạy việc mới
    assert!(runner.active().is_none());
    let inputs2 = vec![png(d.path(), "p2.png")];
    let second = runner.start(job(inputs2, Quality::Balanced), "Việc bình thường".into(), Arc::new(|_| {}));
    assert!(second.is_ok());
    wait_for_report(&runner, second.unwrap());
}

#[test]
fn cancelled_running_job_ends_as_cancelled_with_one_final_callback() {
    use morphix_gui::jobs::JobState;
    let d = tempfile::tempdir().unwrap();
    let runner = JobRunner::new();
    let inputs: Vec<_> = (0..30).map(|i| png(d.path(), &format!("h{i}.png"))).collect();
    let (reached_tx, reached_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let release_rx = Mutex::new(release_rx);
    let finals = Arc::new(Mutex::new(Vec::new()));
    let sink = finals.clone();
    let callback = Arc::new(move |snapshot: morphix_gui::jobs::JobSnapshot| {
        if snapshot.state != JobState::Running {
            sink.lock().unwrap().push(snapshot);
        } else if snapshot.done == 1 {
            reached_tx.send(()).unwrap();
            release_rx.lock().unwrap().recv().unwrap();
        }
    });
    let id = runner
        .start(job(inputs, Quality::High), "Đổi sang JPG".into(), callback)
        .unwrap();
    reached_rx.recv_timeout(Duration::from_secs(20)).unwrap();
    assert!(runner.cancel(id));
    release_tx.send(()).unwrap();
    let report = wait_for_report(&runner, id);
    assert!(report
        .items
        .iter()
        .any(|i| matches!(&i.result, Err(MorphixError::Cancelled))));

    // WHY poll: the final callback fires after the report is stored, so it may land a moment later.
    let deadline = Instant::now() + Duration::from_secs(20);
    while finals.lock().unwrap().is_empty() {
        assert!(Instant::now() < deadline, "không có callback kết thúc");
        std::thread::sleep(Duration::from_millis(10));
    }
    std::thread::sleep(Duration::from_millis(100));
    let finals = finals.lock().unwrap();
    assert_eq!(
        finals.len(),
        1,
        "callback kết thúc phải chỉ gọi đúng một lần"
    );
    assert_eq!(finals[0].state, JobState::Cancelled);
    assert_eq!(finals[0].id, id);
}
