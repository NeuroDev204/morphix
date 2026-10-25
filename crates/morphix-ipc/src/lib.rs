//! Giao thức yêu cầu menu giữa CLI và GUI, cùng socket cục bộ và bộ gom lô.

use std::collections::HashSet;
use std::fs;
use std::io::{self, BufRead, BufReader, ErrorKind, Write};
#[cfg(unix)]
use std::path::Path;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

#[cfg(not(windows))]
use directories::ProjectDirs;
#[cfg(unix)]
use interprocess::local_socket::GenericFilePath;
#[cfg(not(unix))]
use interprocess::local_socket::GenericNamespaced;
use interprocess::local_socket::{
    prelude::*, Listener, ListenerNonblockingMode, ListenerOptions, Name, Stream,
};
use serde::{Deserialize, Serialize};

/// Cửa sổ gom lô: đủ dài để gộp hai lần bấm nhanh, đủ ngắn để người dùng không thấy trễ.
const BATCH_WINDOW: Duration = Duration::from_millis(300);
/// Chu kỳ kiểm tra accept và cửa sổ gom lô. Listener không chặn nên `shutdown` luôn thoát được.
const POLL_INTERVAL: Duration = Duration::from_millis(20);
/// Tổng thời gian tối đa để đọc một yêu cầu. Đây là hạn chót chung, không phải timeout từng lần đọc:
/// client nhỏ giọt byte sẽ không reset được đồng hồ và giữ luồng server mãi.
const CLIENT_READ_DEADLINE: Duration = Duration::from_secs(2);
/// Giới hạn độ dài một dòng yêu cầu. WHY: luồng server duy nhất và bộ nhớ không được phình vô hạn
/// khi client gửi một dòng khổng lồ không có xuống dòng; 1 MiB chứa hàng nghìn đường dẫn.
const MAX_REQUEST_LINE_BYTES: usize = 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MenuRequest {
    pub paths: Vec<PathBuf>,
}

/// DTO trên dây: `PathBuf` không luôn là UTF-8 nên chỉ chuỗi mới đi qua serde.
#[derive(Serialize, Deserialize)]
struct WireRequest {
    paths: Vec<String>,
}

pub fn encode(request: &MenuRequest) -> String {
    let wire = WireRequest {
        paths: request
            .paths
            .iter()
            .map(|path| path.to_string_lossy().into_owned())
            .collect(),
    };
    // serde_json escapes newlines inside strings, so the payload always stays on one line.
    let mut line = serde_json::to_string(&wire).expect("WireRequest of strings always serializes");
    line.push('\n');
    line
}

pub fn decode(line: &str) -> Result<MenuRequest, String> {
    let wire: WireRequest = serde_json::from_str(line)
        .map_err(|error| format!("yêu cầu menu không hợp lệ: {error}"))?;
    Ok(MenuRequest {
        paths: wire.paths.into_iter().map(PathBuf::from).collect(),
    })
}

/// Tên socket duy nhất theo người dùng. `MORPHIX_IPC_NAME` ghi đè để tiến trình test không đụng nhau.
pub fn socket_name() -> String {
    if let Some(name) = std::env::var_os("MORPHIX_IPC_NAME") {
        return name.to_string_lossy().into_owned();
    }
    let user = current_user();
    #[cfg(windows)]
    {
        format!("morphix-{user}")
    }
    #[cfg(not(windows))]
    {
        socket_directory()
            .join(format!("morphix-{user}.sock"))
            .to_string_lossy()
            .into_owned()
    }
}

fn current_user() -> String {
    std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_else(|_| "default".to_string())
}

#[cfg(not(windows))]
fn socket_directory() -> PathBuf {
    // XDG_RUNTIME_DIR is already per-user and cleared on logout, so it is the preferred home.
    if let Some(dir) = std::env::var_os("XDG_RUNTIME_DIR") {
        return PathBuf::from(dir);
    }
    if let Some(dir) = std::env::var_os("MORPHIX_DATA_DIR") {
        return PathBuf::from(dir);
    }
    match ProjectDirs::from("", "", "Morphix") {
        Some(dirs) => dirs.data_local_dir().to_path_buf(),
        None => {
            let fallback = std::env::temp_dir();
            tracing::warn!(path = %fallback.display(), "không xác định được thư mục socket, dùng thư mục tạm");
            fallback
        }
    }
}

fn local_socket_name(name: &str) -> io::Result<Name<'_>> {
    #[cfg(unix)]
    {
        name.to_fs_name::<GenericFilePath>()
    }
    #[cfg(not(unix))]
    {
        name.to_ns_name::<GenericNamespaced>()
    }
}

/// Gom các đường dẫn đến trong cửa sổ `window`, bỏ trùng nhưng giữ thứ tự đến.
pub struct Batcher {
    window: Duration,
    pending: Vec<PathBuf>,
    pending_set: HashSet<PathBuf>,
    window_opened_at: Option<Instant>,
}

impl Batcher {
    pub fn new(window: Duration) -> Self {
        Self {
            window,
            pending: Vec::new(),
            pending_set: HashSet::new(),
            window_opened_at: None,
        }
    }

    /// Trả snapshot của toàn bộ lô hiện tại để GUI hiện trạng thái "đang nhận file" ngay lập tức.
    pub fn push(&mut self, paths: Vec<PathBuf>, now: Instant) -> Vec<PathBuf> {
        // The window is anchored to the first push so a steady trickle cannot postpone the batch forever.
        if self.window_opened_at.is_none() && !paths.is_empty() {
            self.window_opened_at = Some(now);
        }
        for path in paths {
            if self.pending_set.insert(path.clone()) {
                self.pending.push(path);
            }
        }
        self.pending.clone()
    }

    pub fn take_ready(&mut self, now: Instant) -> Option<Vec<PathBuf>> {
        let opened_at = self.window_opened_at?;
        if now.duration_since(opened_at) < self.window {
            return None;
        }
        self.flush()
    }

    /// Lấy lô đang chờ ngay cả khi cửa sổ chưa đóng, dùng lúc dừng server để không rơi file.
    pub fn drain(&mut self) -> Option<Vec<PathBuf>> {
        self.flush()
    }

    fn flush(&mut self) -> Option<Vec<PathBuf>> {
        self.window_opened_at.take()?;
        self.pending_set.clear();
        Some(std::mem::take(&mut self.pending))
    }
}

pub fn send_request(name: &str, request: &MenuRequest) -> io::Result<()> {
    let mut stream = Stream::connect(local_socket_name(name)?)?;
    stream.write_all(encode(request).as_bytes())?;
    stream.flush()
}

pub struct IpcServer {
    stop_requested: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl IpcServer {
    pub fn start(
        name: &str,
        on_pending: impl Fn(Vec<PathBuf>) + Send + Sync + 'static,
        on_batch: impl Fn(Vec<PathBuf>) + Send + Sync + 'static,
    ) -> io::Result<Self> {
        // Binding happens on the caller's thread so a busy or unwritable name is reported to the caller.
        let listener = bind_listener(name)?;
        let stop_requested = Arc::new(AtomicBool::new(false));
        let worker_stop = Arc::clone(&stop_requested);
        let worker = thread::Builder::new()
            .name("morphix-ipc".to_string())
            .spawn(move || {
                run_accept_loop(
                    listener,
                    &worker_stop,
                    Batcher::new(BATCH_WINDOW),
                    &on_pending,
                    &on_batch,
                )
            })?;
        Ok(Self {
            stop_requested,
            worker: Some(worker),
        })
    }

    pub fn shutdown(mut self) -> io::Result<()> {
        self.stop_requested.store(true, Ordering::SeqCst);
        match self.worker.take() {
            Some(handle) => handle
                .join()
                .map_err(|_| io::Error::other("luồng IPC bị panic khi dừng")),
            None => Ok(()),
        }
    }
}

impl Drop for IpcServer {
    // Safety net only: a dropped server stops its loop, but the thread is not joined here.
    fn drop(&mut self) {
        self.stop_requested.store(true, Ordering::SeqCst);
    }
}

fn create_listener(name: &str) -> io::Result<Listener> {
    // Non-blocking accept lets the loop poll the stop flag instead of blocking forever in accept().
    ListenerOptions::new()
        .name(local_socket_name(name)?)
        .nonblocking(ListenerNonblockingMode::Accept)
        .create_sync()
}

fn bind_listener(name: &str) -> io::Result<Listener> {
    #[cfg(unix)]
    if let Some(parent) = Path::new(name).parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)?;
        }
    }
    let error = match create_listener(name) {
        Ok(listener) => return Ok(listener),
        Err(error) => error,
    };
    // A live server keeps the name. Only a socket file nobody answers on is stale, and only Unix
    // leaves such files behind: named pipes vanish with their process.
    if error.kind() != ErrorKind::AddrInUse || !cfg!(unix) || send_probe_connects(name) {
        return Err(error);
    }
    // A crash skips Drop, so interprocess's name reclamation never ran and the file lingers.
    tracing::warn!(
        path = name,
        "xoá socket IPC cũ do tiến trình trước bị dừng đột ngột"
    );
    fs::remove_file(name)?;
    create_listener(name)
}

fn send_probe_connects(name: &str) -> bool {
    match local_socket_name(name).and_then(Stream::connect) {
        Ok(_) => true,
        Err(probe_error) => {
            tracing::debug!(error = %probe_error, "không có listener đang sống trên socket IPC");
            false
        }
    }
}

fn run_accept_loop(
    listener: Listener,
    stop_requested: &AtomicBool,
    mut batcher: Batcher,
    on_pending: &impl Fn(Vec<PathBuf>),
    on_batch: &impl Fn(Vec<PathBuf>),
) {
    while !stop_requested.load(Ordering::SeqCst) {
        match listener.accept() {
            Ok(stream) => handle_connection(stream, &mut batcher, on_pending),
            Err(error) if error.kind() == ErrorKind::WouldBlock => {}
            Err(error) => tracing::warn!(error = %error, "accept socket IPC thất bại"),
        }
        if let Some(batch) = batcher.take_ready(Instant::now()) {
            on_batch(batch);
        }
        thread::sleep(POLL_INTERVAL);
    }
    // WHY: a batch still inside its window at shutdown would otherwise vanish without a trace.
    if let Some(batch) = batcher.drain() {
        on_batch(batch);
    }
    // Listener drops here, which reclaims the socket name for the next server.
}

/// Đọc một dòng yêu cầu trong tổng thời gian `deadline` và tối đa `MAX_REQUEST_LINE_BYTES` byte.
/// Trả về byte trước `\n` (hoặc toàn bộ dữ liệu nếu client đóng kết nối trước khi gửi `\n`).
fn read_request_line(stream: Stream, deadline: Instant) -> io::Result<Vec<u8>> {
    let mut reader = BufReader::new(stream);
    let mut line: Vec<u8> = Vec::new();
    loop {
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .filter(|left| !left.is_zero())
            .ok_or_else(|| {
                io::Error::new(
                    ErrorKind::TimedOut,
                    "client IPC vượt quá hạn chót đọc yêu cầu",
                )
            })?;
        // Re-arming the per-read timeout with the time left turns it into a total deadline.
        reader.get_ref().set_recv_timeout(Some(remaining))?;
        // Interrupted reads are retried, as `read_line` would, so a signal does not drop a valid request.
        let chunk = match reader.fill_buf() {
            Ok(chunk) => chunk,
            Err(error) if error.kind() == ErrorKind::Interrupted => continue,
            Err(error) => return Err(error),
        };
        if chunk.is_empty() {
            return Ok(line);
        }
        let (take, found_newline) = match chunk.iter().position(|&byte| byte == b'\n') {
            Some(index) => (index, true),
            None => (chunk.len(), false),
        };
        if line.len() + take > MAX_REQUEST_LINE_BYTES {
            return Err(io::Error::new(
                ErrorKind::InvalidData,
                "yêu cầu IPC vượt quá giới hạn độ dài",
            ));
        }
        line.extend_from_slice(&chunk[..take]);
        let consumed = if found_newline { take + 1 } else { take };
        reader.consume(consumed);
        if found_newline {
            return Ok(line);
        }
    }
}

fn handle_connection(stream: Stream, batcher: &mut Batcher, on_pending: &impl Fn(Vec<PathBuf>)) {
    let deadline = Instant::now() + CLIENT_READ_DEADLINE;
    let line_bytes = match read_request_line(stream, deadline) {
        Ok(bytes) => bytes,
        Err(error) => {
            tracing::warn!(error = %error, "đọc yêu cầu từ client IPC thất bại");
            return;
        }
    };
    let line = match String::from_utf8(line_bytes) {
        Ok(text) => text,
        Err(error) => {
            tracing::warn!(error = %error, "yêu cầu IPC không phải UTF-8");
            return;
        }
    };
    match decode(&line) {
        Ok(request) => on_pending(batcher.push(request.paths, Instant::now())),
        Err(reason) => tracing::warn!(reason = %reason, "bỏ qua yêu cầu IPC sai định dạng"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn socket_name_honours_env_override() {
        std::env::set_var("MORPHIX_IPC_NAME", "morphix-override-test");
        assert_eq!(socket_name(), "morphix-override-test");
        std::env::remove_var("MORPHIX_IPC_NAME");
    }
}
