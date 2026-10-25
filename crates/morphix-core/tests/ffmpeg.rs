use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};

use morphix_core::error::MorphixError;
use morphix_core::ops::ffmpeg::{parse_time_ms, run_ffmpeg};
use morphix_core::CancelToken;
use tempfile::{tempdir, TempDir};

/// Ghi một script sh giả đóng vai ffmpeg. Test không bao giờ chạy ffmpeg thật.
#[cfg(unix)]
fn fake_ffmpeg(dir: &TempDir, body: &str) -> PathBuf {
    let path: PathBuf = dir.path().join("fake-ffmpeg.sh");
    std::fs::write(&path, format!("{body}\n")).expect("ghi script giả");
    path
}

/// Script được chạy qua `/bin/sh` thay vì exec trực tiếp. WHY: exec ngay một file vừa ghi
/// dễ gặp ETXTBSY khi test khác đang fork song song, làm test chập chờn.
#[cfg(unix)]
fn sh_args(script: &Path, rest: &[OsString]) -> Vec<OsString> {
    let mut args: Vec<OsString> = vec![script.as_os_str().to_owned()];
    args.extend_from_slice(rest);
    args
}

#[cfg(unix)]
fn sh() -> &'static Path {
    Path::new("/bin/sh")
}

#[test]
fn parse_time_ms_reads_progress_line() {
    assert_eq!(
        parse_time_ms("frame=  10 fps=0.0 time=00:01:02.50 bitrate="),
        Some(62500)
    );
}

#[test]
fn parse_time_ms_returns_none_without_time_field() {
    assert_eq!(
        parse_time_ms("Invalid data found when processing input"),
        None
    );
    assert_eq!(parse_time_ms("out_time=00:00:03.00"), None);
}

#[test]
fn parse_time_ms_returns_none_for_na_time() {
    assert_eq!(parse_time_ms("frame=0 time=N/A bitrate=N/A"), None);
}

#[test]
fn parse_time_ms_handles_large_hours_without_overflow() {
    assert_eq!(parse_time_ms("time=99:59:59.99"), Some(359_999_990));
}

#[cfg(unix)]
#[test]
fn run_ffmpeg_reports_progress_and_succeeds() {
    let dir = tempdir().expect("tạo thư mục tạm");
    let script = fake_ffmpeg(
        &dir,
        "echo 'frame=  1 time=00:00:02.50 bitrate=' >&2\nexit 0",
    );
    let mut times: Vec<u64> = Vec::new();
    run_ffmpeg(
        sh(),
        &sh_args(&script, &[]),
        &CancelToken::new(),
        &mut |ms: u64| times.push(ms),
    )
    .expect("ffmpeg giả thoát 0 phải thành công");
    assert_eq!(times, vec![2500]);
}

#[cfg(unix)]
#[test]
fn run_ffmpeg_reports_corrupt_input_with_last_stderr_line() {
    let dir = tempdir().expect("tạo thư mục tạm");
    let script = fake_ffmpeg(
        &dir,
        "echo 'first line' >&2\necho 'Invalid data found when processing input' >&2\nexit 1",
    );
    let result = run_ffmpeg(
        sh(),
        &sh_args(&script, &[]),
        &CancelToken::new(),
        &mut |_: u64| {},
    );
    match result {
        Err(MorphixError::Corrupt { reason, .. }) => {
            assert_eq!(reason, "Invalid data found when processing input");
        }
        other => panic!("kỳ vọng Corrupt, được {other:?}"),
    }
}

#[cfg(unix)]
#[test]
fn run_ffmpeg_cancel_mid_run_kills_and_reaps_child() {
    let script_dir = tempdir().expect("tạo thư mục tạm");
    let out_dir = tempdir().expect("tạo thư mục đầu ra");
    let pid_file: PathBuf = script_dir.path().join("pid");
    // WHY: `exec` thay shell bằng sleep, nên PID ghi ra chính là tiến trình con mà run_ffmpeg phải huỷ.
    let body = format!(
        "echo $$ > '{0}.part' && mv '{0}.part' '{0}'\nexec sleep 30",
        pid_file.display()
    );
    let script = fake_ffmpeg(&script_dir, &body);
    let out_path: PathBuf = out_dir.path().join("out.mp4");
    let rest: Vec<OsString> = vec!["-i".into(), "in.mp4".into(), out_path.into_os_string()];
    let args = sh_args(&script, &rest);

    let token = CancelToken::new();
    let canceller_token = token.clone();
    let watched: PathBuf = pid_file.clone();
    let canceller = thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(10);
        while Instant::now() < deadline {
            if watched.exists() {
                canceller_token.cancel();
                return;
            }
            thread::sleep(Duration::from_millis(10));
        }
    });

    let started = Instant::now();
    let result = run_ffmpeg(sh(), &args, &token, &mut |_: u64| {});
    canceller.join().expect("luồng huỷ không panic");

    assert!(
        matches!(result, Err(MorphixError::Cancelled)),
        "kỳ vọng Cancelled, được {result:?}"
    );
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "huỷ phải dừng ngay, không chờ sleep 30"
    );
    let pid: u32 = std::fs::read_to_string(&pid_file)
        .expect("đọc PID")
        .trim()
        .parse()
        .expect("PID là số");
    assert!(
        !Path::new(&format!("/proc/{pid}")).exists(),
        "tiến trình con {pid} vẫn còn: chưa bị huỷ hoặc chưa được thu hồi"
    );
    // WHY: run_ffmpeg không tạo file; test này khẳng định không có file nào rơi lại trong thư mục đầu ra.
    assert_eq!(
        std::fs::read_dir(out_dir.path())
            .expect("đọc thư mục đầu ra")
            .count(),
        0
    );
}

#[cfg(unix)]
#[test]
fn run_ffmpeg_cancelled_before_start_does_not_spawn() {
    let dir = tempdir().expect("tạo thư mục tạm");
    let marker: PathBuf = dir.path().join("started");
    let script = fake_ffmpeg(&dir, &format!("touch '{}'\nexit 0", marker.display()));
    let token = CancelToken::new();
    token.cancel();
    let result = run_ffmpeg(sh(), &sh_args(&script, &[]), &token, &mut |_: u64| {});
    assert!(
        matches!(result, Err(MorphixError::Cancelled)),
        "được {result:?}"
    );
    assert!(!marker.exists(), "ffmpeg không được khởi động khi đã huỷ");
}

#[cfg(unix)]
#[test]
fn run_ffmpeg_drains_one_megabyte_of_stderr() {
    let dir = tempdir().expect("tạo thư mục tạm");
    // WHY: 1 MB stderr vượt bộ đệm pipe; nếu đọc đồng bộ, child sẽ treo và test timeout.
    // `echo` đầu tiên kết thúc dòng dở dang do head cắt, để dòng time= đứng riêng.
    let script = fake_ffmpeg(
        &dir,
        "yes 'frame=  1 fps=0.0 q=0.0' | head -c 1048576 >&2\necho >&2\necho 'time=00:00:01.00' >&2\nexit 0",
    );
    let mut times: Vec<u64> = Vec::new();
    run_ffmpeg(
        sh(),
        &sh_args(&script, &[]),
        &CancelToken::new(),
        &mut |ms: u64| times.push(ms),
    )
    .expect("ffmpeg giả ghi 1 MB stderr phải hoàn tất");
    assert_eq!(times, vec![1000]);
}

#[cfg(unix)]
#[test]
fn run_ffmpeg_passes_args_as_argv_without_shell() {
    let dir = tempdir().expect("tạo thư mục tạm");
    let argv_file: PathBuf = dir.path().join("argv.txt");
    let script = fake_ffmpeg(
        &dir,
        &format!("printf '%s\\n' \"$@\" > '{}'\nexit 0", argv_file.display()),
    );
    let input = "/dir with space/it's a clip $HOME.mp4";
    let rest: Vec<OsString> = vec!["-i".into(), input.into(), "-y".into()];
    run_ffmpeg(
        sh(),
        &sh_args(&script, &rest),
        &CancelToken::new(),
        &mut |_: u64| {},
    )
    .expect("ffmpeg giả thoát 0 phải thành công");
    let got = std::fs::read_to_string(&argv_file).expect("đọc argv");
    assert_eq!(got.lines().collect::<Vec<&str>>(), vec!["-i", input, "-y"]);
}

#[cfg(not(unix))]
#[test]
#[ignore = "script giả là sh, chỉ chạy trên unix; Windows chờ bản thử thật"]
fn fake_ffmpeg_tests_need_unix() {}
