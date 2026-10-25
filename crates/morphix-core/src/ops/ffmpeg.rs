//! Chạy ffmpeg như tiến trình con riêng: argv không qua shell, đọc tiến trình từ stderr,
//! và huỷ được giữa chừng. Không link ffmpeg vào binary.

use std::ffi::OsString;
use std::io::{self, Read};
use std::path::Path;
use std::process::{Child, ChildStderr, Command, Stdio};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crate::cancel::CancelToken;
use crate::error::{MorphixError, Result};

/// WHY: đủ nhanh để nút Huỷ phản hồi gần như tức thì, đủ thưa để vòng chờ không đốt CPU.
const POLL_INTERVAL: Duration = Duration::from_millis(50);

/// Chạy `ffmpeg` với `args` và báo mốc thời gian đã xử lý qua `on_time_ms` (mili-giây).
/// Huỷ thì dừng tiến trình con rồi trả `Cancelled`. Hỏng thì trả `Corrupt` kèm dòng stderr cuối.
/// Hàm không tạo file: việc xoá file đầu ra dở dang thuộc về bên gọi.
pub fn run_ffmpeg(
    ffmpeg: &Path,
    args: &[OsString],
    cancel: &CancelToken,
    on_time_ms: &mut dyn FnMut(u64),
) -> Result<()> {
    // WHY: kiểm trước khi spawn để huỷ sớm không phải khởi động một tiến trình ffmpeg.
    cancel.check()?;
    let mut child: Child = Command::new(ffmpeg)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|source| io_error(ffmpeg, source))?;
    // WHY: đã yêu cầu `piped` ở trên, nên stderr luôn có mặt.
    let stderr: ChildStderr = child
        .stderr
        .take()
        .ok_or_else(|| io_error(ffmpeg, io::Error::other("không mở được stderr của ffmpeg")))?;

    let (tx, rx) = mpsc::channel::<String>();
    // WHY: đọc stderr trên luồng riêng. ffmpeg ghi tiến trình ra stderr; nếu đọc đồng bộ
    // thì khi bộ đệm pipe đầy, ffmpeg bị treo và vòng chờ không bao giờ thấy nó thoát.
    let reader: JoinHandle<io::Result<()>> = thread::spawn(move || read_segments(stderr, tx));

    let mut last_line: String = String::new();
    let status = loop {
        drain(&rx, on_time_ms, &mut last_line);
        if cancel.is_cancelled() {
            stop_child(&mut child, ffmpeg)?;
            finish_reader(reader, ffmpeg)?;
            return Err(MorphixError::Cancelled);
        }
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => thread::sleep(POLL_INTERVAL),
            Err(source) => {
                // WHY: lỗi try_wait không được nuốt; dừng con trước để không để lại tiến trình mồ côi.
                stop_child(&mut child, ffmpeg)?;
                return Err(io_error(ffmpeg, source));
            }
        }
    };
    finish_reader(reader, ffmpeg)?;
    drain(&rx, on_time_ms, &mut last_line);

    if status.success() {
        return Ok(());
    }
    let reason: String = if last_line.is_empty() {
        format!("ffmpeg thoát với mã {status}")
    } else {
        last_line
    };
    Err(MorphixError::Corrupt {
        path: ffmpeg.to_path_buf(),
        reason,
    })
}

/// Đọc `time=HH:MM:SS.xx` từ một dòng tiến trình của ffmpeg và trả về mili-giây.
/// Trả `None` nếu dòng không có `time=` hợp lệ (ví dụ `time=N/A`).
pub fn parse_time_ms(stderr_line: &str) -> Option<u64> {
    // WHY: khớp đúng từ bắt đầu bằng `time=`; `out_time=` không được tính là mốc thời gian.
    let value: &str = stderr_line
        .split_whitespace()
        .find_map(|word| word.strip_prefix("time="))?;
    let mut parts = value.split(':');
    let hours: u64 = parts.next()?.parse().ok()?;
    let minutes: u64 = parts.next()?.parse().ok()?;
    let sec_part: &str = parts.next()?;
    if parts.next().is_some() {
        return None;
    }
    let (secs_text, frac_text) = sec_part.split_once('.').unwrap_or((sec_part, ""));
    let secs: u64 = secs_text.parse().ok()?;
    // WHY: ffmpeg thường in 2 chữ số thập phân; đệm phải thành 3 chữ số để ra đúng mili-giây.
    let frac_head: String = frac_text.chars().take(3).collect();
    let millis: u64 = format!("{frac_head:0<3}").parse().ok()?;
    // WHY: phép nhân và cộng có kiểm tràn để một dòng lạ không làm panic khi debug.
    let total_secs = hours
        .checked_mul(3600)?
        .checked_add(minutes.checked_mul(60)?)?
        .checked_add(secs)?;
    total_secs.checked_mul(1000)?.checked_add(millis)
}

fn io_error(ffmpeg: &Path, source: io::Error) -> MorphixError {
    MorphixError::Io {
        path: ffmpeg.to_path_buf(),
        source,
    }
}

/// Dừng tiến trình con rồi thu hồi nó. Cả hai lỗi đều được trả lên, không nuốt.
fn stop_child(child: &mut Child, ffmpeg: &Path) -> Result<()> {
    child.kill().map_err(|source| io_error(ffmpeg, source))?;
    // WHY: phải wait sau kill để kernel thu hồi tiến trình, nếu không nó thành zombie.
    child
        .wait()
        .map(|_| ())
        .map_err(|source| io_error(ffmpeg, source))
}

/// Chờ luồng đọc kết thúc. Luồng panic hay lỗi đọc đều trở thành lỗi I/O của ffmpeg.
fn finish_reader(reader: JoinHandle<io::Result<()>>, ffmpeg: &Path) -> Result<()> {
    match reader.join() {
        Ok(Ok(())) => Ok(()),
        Ok(Err(source)) => Err(io_error(ffmpeg, source)),
        Err(_) => Err(io_error(
            ffmpeg,
            io::Error::other("luồng đọc stderr của ffmpeg bị panic"),
        )),
    }
}

/// Lấy hết dòng đang chờ trong kênh, cập nhật mốc thời gian và dòng cuối.
fn drain(rx: &Receiver<String>, on_time_ms: &mut dyn FnMut(u64), last_line: &mut String) {
    while let Ok(line) = rx.try_recv() {
        if let Some(ms) = parse_time_ms(&line) {
            on_time_ms(ms);
        }
        *last_line = line;
    }
}

/// Tách stderr thành các đoạn theo `\r` hoặc `\n`. ffmpeg dùng `\r` cho dòng tiến trình
/// nên chỉ tách `\n` sẽ làm mốc thời gian đến quá muộn.
fn read_segments(mut stderr: ChildStderr, tx: Sender<String>) -> io::Result<()> {
    let mut chunk = [0u8; 4096];
    let mut pending: Vec<u8> = Vec::new();
    loop {
        let n = stderr.read(&mut chunk)?;
        if n == 0 {
            break;
        }
        for &byte in &chunk[..n] {
            if byte == b'\r' || byte == b'\n' {
                send_segment(&tx, &mut pending)?;
            } else {
                pending.push(byte);
            }
        }
    }
    send_segment(&tx, &mut pending)
}

fn send_segment(tx: &Sender<String>, pending: &mut Vec<u8>) -> io::Result<()> {
    if pending.is_empty() {
        return Ok(());
    }
    let line: String = String::from_utf8_lossy(pending).into_owned();
    pending.clear();
    tx.send(line)
        .map_err(|_| io::Error::other("bên nhận stderr đã đóng"))
}
