//! Thao tác file dùng chung cho mọi backend: ghi nguyên tử, ownership bằng marker, rollback.

use std::{
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
};

use tempfile::NamedTempFile;

/// Marker nằm trong mọi file Morphix tạo. WHY: đây là bằng chứng ownership duy nhất để gỡ không xoá
/// file của người khác có cùng tên.
pub const MANAGED_MARKER: &str = "Managed by Morphix";

/// Nội dung và quyền của một file đang tồn tại. Quyền được giữ để rollback khôi phục đúng.
pub struct ExistingFile {
    pub bytes: Vec<u8>,
    pub mode: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileOwnership {
    Absent,
    Owned,
    Foreign,
}

/// Đọc file hiện có; `None` khi chưa có. Lỗi đọc được trả lên để không đoán ownership khi không đọc được.
pub fn read_existing(path: &Path) -> io::Result<Option<ExistingFile>> {
    match fs::read(path) {
        Ok(bytes) => Ok(Some(ExistingFile {
            bytes,
            mode: file_mode(&fs::metadata(path)?),
        })),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

/// Marker phải là nguyên một dòng trong vài dòng đầu (sau shebang nếu có). WHY: file của người dùng chỉ
/// nhắc cụm từ trong câu không được coi là của Morphix, nếu không gỡ sẽ xoá nhầm file đó.
const MARKER_SEARCH_LINES: usize = 2;

fn has_marker_line(bytes: &[u8]) -> bool {
    String::from_utf8_lossy(bytes)
        .lines()
        .take(MARKER_SEARCH_LINES)
        .any(is_marker_line)
}

/// WHY `#` được chấp nhận: script shell và Desktop Entry coi `#` là comment, nên dòng marker trần sẽ bị
/// thực thi thành lệnh. Vẫn yêu cầu nguyên dòng marker, nên câu chứa cụm từ không khớp.
fn is_marker_line(line: &str) -> bool {
    line.strip_prefix('#').map_or(line, str::trim_start) == MANAGED_MARKER
}

pub fn classify(existing: Option<&ExistingFile>) -> FileOwnership {
    match existing {
        None => FileOwnership::Absent,
        Some(file) if has_marker_line(&file.bytes) => FileOwnership::Owned,
        Some(_) => FileOwnership::Foreign,
    }
}

/// Các bước đã ghi trong một lần cài; `commit` xác nhận, nếu không thì `Drop` đảo ngược theo thứ tự ngược.
#[derive(Default)]
pub struct Transaction {
    undo: Vec<Undo>,
}

enum Undo {
    RemoveFile(PathBuf),
    RestoreFile(PathBuf, ExistingFile),
    RemoveDirIfEmpty(PathBuf),
    /// Thao tác đảo ngược của backend không phải file (ví dụ xoá key registry vừa tạo).
    Custom(Box<dyn FnOnce() -> io::Result<()>>),
}

impl Transaction {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn commit(mut self) {
        self.undo.clear();
    }

    /// Ghi nhận bước đã làm để rollback; chỉ gọi sau khi thao tác đã thành công.
    fn record(&mut self, step: Undo) {
        self.undo.push(step);
    }

    /// Đăng ký đảo ngược tuỳ ý cho thao tác không phải file; chỉ gọi sau khi thao tác đã thành công.
    pub fn defer(&mut self, undo: impl FnOnce() -> io::Result<()> + 'static) {
        self.record(Undo::Custom(Box::new(undo)));
    }

    fn rollback(&mut self) {
        while let Some(step) = self.undo.pop() {
            // WHY log thay vì bỏ qua: rollback thất bại để lại file dở dang, người dùng cần biết đường dẫn.
            if let Err(error) = step.apply() {
                tracing::error!(%error, "rollback không hoàn tất");
            }
        }
    }
}

impl Drop for Transaction {
    fn drop(&mut self) {
        self.rollback();
    }
}

impl Undo {
    fn apply(self) -> io::Result<()> {
        match self {
            Undo::RemoveFile(path) => fs::remove_file(path),
            Undo::RestoreFile(path, previous) => {
                let mut scratch = Transaction::new();
                publish_file(&path, &previous.bytes, previous.mode, &mut scratch)?;
                scratch.commit();
                Ok(())
            }
            Undo::RemoveDirIfEmpty(dir) => remove_dir_if_empty(&dir),
            Undo::Custom(undo) => undo(),
        }
    }
}

/// Ghi `bytes` vào `path` qua file tạm cùng thư mục rồi rename. WHY cùng thư mục: rename chỉ nguyên tử
/// khi cùng filesystem. Quyền được đặt trước khi rename để script/menu không bao giờ xuất hiện thiếu quyền chạy.
pub fn publish_file(path: &Path, bytes: &[u8], mode: u32, tx: &mut Transaction) -> io::Result<()> {
    let parent = path.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "đường dẫn không có thư mục cha",
        )
    })?;
    let created_dirs = create_missing_dirs(parent)?;
    let previous = read_existing(path)?;

    let mut temp = NamedTempFile::new_in(parent)?;
    temp.write_all(bytes)?;
    temp.as_file().sync_all()?;
    set_mode(temp.path(), mode)?;
    temp.persist(path).map_err(|error| error.error)?;

    // Ghi nhận sau persist: nếu bước này lỗi thì không có gì để rollback.
    for dir in created_dirs {
        tx.record(Undo::RemoveDirIfEmpty(dir));
    }
    match previous {
        Some(old) => tx.record(Undo::RestoreFile(path.to_path_buf(), old)),
        None => tx.record(Undo::RemoveFile(path.to_path_buf())),
    }
    Ok(())
}

/// Tạo các thư mục còn thiếu, trả về danh sách đã tạo theo thứ tự từ trên xuống để rollback xoá từ dưới lên.
fn create_missing_dirs(dir: &Path) -> io::Result<Vec<PathBuf>> {
    let mut missing = Vec::new();
    let mut current = dir;
    while !current.exists() {
        missing.push(current.to_path_buf());
        match current.parent() {
            Some(parent) => current = parent,
            None => break,
        }
    }
    fs::create_dir_all(dir)?;
    missing.reverse();
    Ok(missing)
}

/// Xoá file chỉ khi nó mang marker Morphix. Trả về ownership đã thấy; file của người khác được giữ nguyên.
pub fn remove_file_if_owned(path: &Path, tx: &mut Transaction) -> io::Result<FileOwnership> {
    let existing = read_existing(path)?;
    let ownership = classify(existing.as_ref());
    if ownership != FileOwnership::Owned {
        return Ok(ownership);
    }
    fs::remove_file(path)?;
    if let Some(old) = existing {
        tx.record(Undo::RestoreFile(path.to_path_buf(), old));
    }
    Ok(FileOwnership::Owned)
}

/// Xoá thư mục chỉ khi rỗng. WHY: thư mục có thể do người dùng hoặc ứng dụng khác dùng chung.
pub fn remove_dir_if_empty(dir: &Path) -> io::Result<()> {
    match fs::read_dir(dir) {
        Ok(mut entries) => {
            if entries.next().is_none() {
                fs::remove_dir(dir)
            } else {
                Ok(())
            }
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

#[cfg(unix)]
fn file_mode(metadata: &fs::Metadata) -> u32 {
    use std::os::unix::fs::PermissionsExt;
    metadata.permissions().mode() & 0o7777
}

#[cfg(not(unix))]
fn file_mode(_metadata: &fs::Metadata) -> u32 {
    // WHY 0: Windows không có bit executable kiểu POSIX, nên không có quyền nào để khôi phục.
    0
}

#[cfg(unix)]
fn set_mode(path: &Path, mode: u32) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(mode))
}

#[cfg(not(unix))]
fn set_mode(_path: &Path, _mode: u32) -> io::Result<()> {
    // WHY no-op: quyền thực thi không áp dụng trên Windows; Task 18 dựa vào registry, không dựa vào mode.
    Ok(())
}
