//! Gọi công cụ bên ngoài (gnome-extensions, desktop-file-validate) bằng argv, có thời hạn, không qua shell.

use std::{
    ffi::OsStr,
    io::{self, Read},
    process::{Child, Command, ExitStatus, Stdio},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use tracing::error;

/// WHY 5 giây: doctor không được treo theo một D-Bus chậm; quá thời gian thì báo `unknown` thay vì chờ mãi.
const TOOL_TIMEOUT: Duration = Duration::from_secs(5);
const POLL_INTERVAL: Duration = Duration::from_millis(20);

/// Phân biệt "không cài công cụ" với "công cụ chạy nhưng lỗi": hai trường hợp này dẫn tới kết luận khác nhau.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolOutcome {
    NotFound,
    Failed(String),
    Exited {
        code: Option<i32>,
        stdout: String,
        stderr: String,
    },
}

/// Hàm chạy công cụ được inject: bản thật gọi tiến trình, test trả kết quả giả.
pub type ToolRunner = dyn Fn(&str, &[&OsStr]) -> ToolOutcome;

/// Chạy `program` với `args` trực tiếp, không shell. Quá thời hạn thì huỷ tiến trình và báo `Failed`.
pub fn system_tool(program: &str, args: &[&OsStr]) -> ToolOutcome {
    let mut command = Command::new(program);
    command.args(args);
    run_command(command, TOOL_TIMEOUT)
}

/// WHY nhận `Command` và timeout: test cần timeout ngắn và tiến trình tự chọn, mà không đổi đường chạy thật.
fn run_command(mut command: Command, timeout: Duration) -> ToolOutcome {
    let program = command.get_program().to_string_lossy().into_owned();
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return ToolOutcome::NotFound,
        Err(error) => return ToolOutcome::Failed(format!("không chạy được {program}: {error}")),
    };
    // WHY đọc song song ngay từ đầu: pipe chỉ giữ ~64 KiB; nếu không có người đọc, tiến trình bị chặn khi ghi và không bao giờ thoát.
    let stdout_reader = spawn_reader(child.stdout.take());
    let stderr_reader = spawn_reader(child.stderr.take());
    // WHY một deadline cho cả hai giai đoạn: EOF chỉ tới khi mọi bản sao đầu ghi của pipe đã đóng, kể cả của tiến trình cháu.
    let deadline = Instant::now() + timeout;
    let status = match wait_for_exit(&mut child, deadline) {
        Ok(status) => status,
        Err(message) => return ToolOutcome::Failed(format!("{program}: {message}")),
    };
    while !(stdout_reader.is_finished() && stderr_reader.is_finished()) {
        // ponytail: chỉ bỏ luồng đọc, không huỷ tiến trình cháu; huỷ cả nhóm tiến trình cần libc hoặc unsafe, thêm khi cần.
        if Instant::now() >= deadline {
            return ToolOutcome::Failed(format!(
                "{program}: quá thời hạn khi đọc đầu ra (tiến trình con còn giữ pipe)"
            ));
        }
        thread::sleep(POLL_INTERVAL);
    }
    match (join_reader(stdout_reader), join_reader(stderr_reader)) {
        (Ok(stdout), Ok(stderr)) => ToolOutcome::Exited {
            code: status.code(),
            stdout,
            stderr,
        },
        (Err(error), _) | (_, Err(error)) => {
            ToolOutcome::Failed(format!("{program}: không đọc được đầu ra: {error}"))
        }
    }
}

fn spawn_reader<R: Read + Send + 'static>(pipe: Option<R>) -> JoinHandle<io::Result<String>> {
    thread::spawn(move || -> io::Result<String> {
        let mut bytes = Vec::new();
        if let Some(mut pipe) = pipe {
            pipe.read_to_end(&mut bytes)?;
        }
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    })
}

/// WHY không bỏ panic của luồng đọc: biến nó thành lỗi I/O để báo `failed`/`unknown` thay vì làm sập doctor.
fn join_reader(reader: JoinHandle<io::Result<String>>) -> io::Result<String> {
    reader
        .join()
        .unwrap_or_else(|_| Err(io::Error::other("luồng đọc đầu ra bị panic")))
}

fn wait_for_exit(child: &mut Child, deadline: Instant) -> Result<ExitStatus, String> {
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Ok(status),
            Ok(None) if Instant::now() >= deadline => {
                kill_and_reap(child);
                return Err("quá thời hạn".to_string());
            }
            Ok(None) => thread::sleep(POLL_INTERVAL),
            Err(error) => {
                kill_and_reap(child);
                return Err(format!("không đợi được tiến trình: {error}"));
            }
        }
    }
}

/// WHY log: kill/wait lỗi vẫn phải để lại dấu vết, vì tiến trình có thể còn chạy nền.
fn kill_and_reap(child: &mut Child) {
    if let Err(error) = child.kill() {
        error!(%error, "không huỷ được công cụ");
    }
    if let Err(error) = child.wait() {
        error!(%error, "không thu được tiến trình công cụ đã huỷ");
    }
}

#[cfg(test)]
mod tests {
    // WHY unix: mọi test trong module dùng `head`/`sleep` của unix, nên import chỉ có nghĩa trên unix.
    #[cfg(unix)]
    use super::*;

    /// WHY biến môi trường: test tự gọi lại chính binary test để tạo tiến trình cháu; biến này phân biệt chế độ đó.
    #[cfg(unix)]
    const GRANDCHILD_ENV: &str = "MORPHIX_TOOLS_TEST_GRANDCHILD";

    #[cfg(unix)]
    #[test]
    fn large_output_from_a_fast_child_is_not_reported_as_a_timeout() {
        let outcome = system_tool(
            "head",
            &[
                OsStr::new("-c"),
                OsStr::new("131072"),
                OsStr::new("/dev/zero"),
            ],
        );
        match outcome {
            ToolOutcome::Exited {
                code: Some(0),
                stdout,
                ..
            } => assert_eq!(stdout.len(), 131_072),
            other => panic!("đầu ra lớn bị coi là lỗi: {other:?}"),
        }
    }

    // WHY allow: tiến trình cháu phải sống sau khi helper thoát để giữ pipe; init sẽ thu dọn nó.
    #[allow(clippy::zombie_processes)]
    #[cfg(unix)]
    #[test]
    fn grandchild_holding_stdout_cannot_extend_the_deadline() {
        if std::env::var_os(GRANDCHILD_ENV).is_some() {
            // WHY: tiến trình này thoát ngay, nhưng `sleep` thừa kế stdout nên pipe vẫn mở sau khi nó thoát.
            Command::new("sleep").arg("5").spawn().unwrap();
            return;
        }
        let test_exe = std::env::current_exe().unwrap();
        let mut command = Command::new(test_exe);
        command
            .args([
                "--exact",
                "integration::tools::tests::grandchild_holding_stdout_cannot_extend_the_deadline",
                "--test-threads=1",
            ])
            .env(GRANDCHILD_ENV, "1");
        let started = Instant::now();
        let outcome = run_command(command, Duration::from_secs(1));
        let elapsed = started.elapsed();
        assert!(
            matches!(outcome, ToolOutcome::Failed(_)),
            "kết quả không phải timeout: {outcome:?}"
        );
        assert!(
            elapsed < Duration::from_secs(2),
            "chờ quá lâu vì tiến trình cháu: {elapsed:?}"
        );
    }
}
