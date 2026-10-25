use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use serde_json::Value;

const DOCTOR_TIMEOUT: Duration = Duration::from_secs(30);

fn cli_binary_path() -> Result<PathBuf, String> {
    let current = std::env::current_exe()
        .map_err(|error| format!("Không xác định được vị trí Morphix: {error}"))?;
    Ok(current.with_file_name(format!("morphix{}", std::env::consts::EXE_SUFFIX)))
}

fn read_pipe<R: Read + Send + 'static>(mut pipe: R) -> JoinHandle<io::Result<Vec<u8>>> {
    thread::spawn(move || {
        let mut bytes = Vec::new();
        pipe.read_to_end(&mut bytes)?;
        Ok(bytes)
    })
}

fn finish_pipe(reader: JoinHandle<io::Result<Vec<u8>>>, name: &str) -> Result<Vec<u8>, String> {
    reader
        .join()
        .map_err(|_| format!("Luồng đọc {name} của doctor bị dừng bất thường."))?
        .map_err(|error| format!("Không đọc được {name} của doctor: {error}"))
}

fn stop_child(child: &mut Child) -> Result<(), String> {
    let kill_error = child.kill().err();
    let wait_result = child
        .wait()
        .map_err(|error| format!("Không thu dọn được tiến trình doctor: {error}"));
    match (kill_error, wait_result) {
        (None, result) => result.map(|_| ()),
        (Some(error), Ok(_)) => Err(format!("Không dừng được tiến trình doctor: {error}")),
        (Some(kill_error), Err(wait_error)) => Err(format!("{kill_error}; {wait_error}")),
    }
}

fn inspect_with_cli(cli: &Path, timeout: Duration) -> Result<Value, String> {
    let mut child = Command::new(cli)
        .args(["doctor", "--json"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("Không chạy được {}: {error}", cli.display()))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "Không đọc được stdout của doctor.".to_string())?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "Không đọc được stderr của doctor.".to_string())?;
    let stdout_reader = read_pipe(stdout);
    let stderr_reader = read_pipe(stderr);
    let started = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if started.elapsed() < timeout => thread::sleep(Duration::from_millis(25)),
            Ok(None) => {
                let cleanup = stop_child(&mut child);
                let stdout = finish_pipe(stdout_reader, "stdout")?;
                let stderr = finish_pipe(stderr_reader, "stderr")?;
                return Err(format!(
                    "doctor vượt quá {:.1} giây và đã bị dừng{}{}",
                    timeout.as_secs_f64(),
                    cleanup
                        .err()
                        .map_or_else(String::new, |error| format!(" ({error})")),
                    process_detail(&stdout, &stderr)
                ));
            }
            Err(error) => {
                let cleanup = stop_child(&mut child);
                let stdout = finish_pipe(stdout_reader, "stdout")?;
                let stderr = finish_pipe(stderr_reader, "stderr")?;
                return Err(format!(
                    "Không kiểm tra được tiến trình doctor: {error}{}{}",
                    cleanup
                        .err()
                        .map_or_else(String::new, |error| format!(" ({error})")),
                    process_detail(&stdout, &stderr)
                ));
            }
        }
    };
    let stdout = finish_pipe(stdout_reader, "stdout")?;
    let stderr = finish_pipe(stderr_reader, "stderr")?;
    let value: Value = serde_json::from_slice(&stdout).map_err(|error| {
        format!(
            "doctor trả về JSON không hợp lệ (mã {status}): {error}{}",
            process_detail(&[], &stderr)
        )
    })?;
    if !value
        .as_object()
        .is_some_and(|report| report.contains_key("integration"))
    {
        return Err(format!(
            "doctor thiếu báo cáo integration{}",
            process_detail(&[], &stderr)
        ));
    }
    Ok(value)
}

fn process_detail(stdout: &[u8], stderr: &[u8]) -> String {
    let stdout = String::from_utf8_lossy(stdout);
    let stderr = String::from_utf8_lossy(stderr);
    let detail = [stdout.trim(), stderr.trim()]
        .into_iter()
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>()
        .join("; ");
    if detail.is_empty() {
        String::new()
    } else {
        format!(": {detail}")
    }
}

#[tauri::command]
pub async fn integration_status() -> Result<Value, String> {
    let cli = cli_binary_path()?;
    tauri::async_runtime::spawn_blocking(move || inspect_with_cli(&cli, DOCTOR_TIMEOUT))
        .await
        .map_err(|error| format!("Không chờ được doctor hoàn tất: {error}"))?
}

#[cfg(all(test, unix))]
mod tests {
    use super::inspect_with_cli;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::Path;
    use std::time::{Duration, Instant};

    static PROCESS_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn executable(directory: &Path, name: &str, source: &str) -> std::path::PathBuf {
        let path = directory.join(name);
        fs::write(&path, source).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
        path
    }

    #[test]
    fn accepts_valid_doctor_json_even_when_doctor_reports_failure() {
        // WHY: `/tmp` on this overlay intermittently returns ETXTBSY when these fixtures execute concurrently.
        let _guard = PROCESS_TEST_LOCK.lock().unwrap();
        let directory = tempfile::tempdir().unwrap();
        let cli = executable(
            directory.path(),
            "morphix-valid",
            "#!/bin/sh\ntest \"$1 $2\" = 'doctor --json' || exit 4\nprintf '%s\\n' '{\"integration\":{\"overall\":\"failure\",\"components\":[]}}'\nexit 1\n",
        );
        let report = inspect_with_cli(&cli, Duration::from_secs(2)).unwrap();
        assert_eq!(report["integration"]["overall"], "failure");
    }

    #[test]
    fn rejects_malformed_doctor_json_and_missing_cli() {
        let _guard = PROCESS_TEST_LOCK.lock().unwrap();
        let directory = tempfile::tempdir().unwrap();
        let cli = executable(
            directory.path(),
            "morphix-malformed",
            "#!/bin/sh\necho broken\n",
        );
        let malformed = inspect_with_cli(&cli, Duration::from_secs(2)).unwrap_err();
        assert!(malformed.contains("JSON không hợp lệ"), "{malformed}");
        assert!(
            inspect_with_cli(&directory.path().join("missing"), Duration::from_secs(2))
                .unwrap_err()
                .contains("Không chạy được")
        );
    }

    #[test]
    fn timeout_kills_and_reaps_the_doctor_process() {
        let _guard = PROCESS_TEST_LOCK.lock().unwrap();
        let directory = tempfile::tempdir().unwrap();
        let cli = executable(
            directory.path(),
            "morphix-timeout",
            "#!/bin/sh\nexec sleep 5\n",
        );
        let started = Instant::now();
        let error = inspect_with_cli(&cli, Duration::from_millis(75)).unwrap_err();
        assert!(error.contains("vượt quá 0.1 giây"), "{error}");
        assert!(started.elapsed() < Duration::from_secs(2));
    }
}
