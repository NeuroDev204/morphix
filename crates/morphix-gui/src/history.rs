use std::io::Write;
use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};

/// Số lượng bản ghi lịch sử tối đa được lưu trữ.
pub const MAX_ENTRIES: usize = 50;

/// Trạng thái hoàn thành của một tác vụ xử lý ảnh.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EntryStatus {
    Done,
    Failed,
    Cancelled,
}

impl EntryStatus {
    /// Nhãn tiếng Việt mở đầu chi tiết của dòng lịch sử, để người dùng thấy kết quả không cần đọc số liệu.
    pub fn label(self) -> &'static str {
        match self {
            Self::Done => "Hoàn tất",
            Self::Failed => "Có lỗi",
            Self::Cancelled => "Đã huỷ",
        }
    }
}

const KILOBYTE: u64 = 1024;
const MEGABYTE: u64 = 1024 * KILOBYTE;

/// Định dạng dung lượng kiểu Việt Nam: đơn vị nhị phân, dấu phẩy thập phân, khớp với `formatSize` của giao diện.
///
/// WHY: tự định dạng thay vì dựa vào locale của hệ điều hành, để chuỗi lịch sử luôn giống nhau trên mọi máy.
pub fn format_size_vn(bytes: u64) -> String {
    // WHY: dưới 1 KB làm tròn lên KB sẽ in sai "0 KB" cho file nhỏ.
    if bytes < KILOBYTE {
        return format!("{bytes} B");
    }
    if bytes < MEGABYTE {
        let kilobytes: u64 = (bytes as f64 / KILOBYTE as f64).round() as u64;
        return format!("{kilobytes} KB");
    }
    let megabytes: f64 = bytes as f64 / MEGABYTE as f64;
    // WHY: một chữ số thập phân chỉ khi dưới 10 MB, từ 10 MB trở lên số nguyên là đủ chính xác.
    let text: String = if megabytes < 10.0 {
        format!("{megabytes:.1}").replace('.', ",")
    } else {
        format!("{}", megabytes.round() as u64)
    };
    format!("{text} MB")
}

/// Một bản ghi trong lịch sử xử lý ảnh.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistoryEntry {
    /// Đường dẫn đầu ra đầu tiên hoặc đường dẫn đầu vào nếu không có đầu ra.
    /// Chuỗi được chuyển đổi lossily để bảo đảm tính tương thích UTF-8 trên mọi nền tảng.
    pub path: String,
    /// Tiêu đề ngắn gọn mô tả tác vụ (ví dụ: tên file hoặc tóm tắt thao tác).
    pub title: String,
    /// Chi tiết kết quả (ví dụ: tỉ lệ nén, thông báo hoàn tất, hoặc lỗi).
    pub detail: String,
    /// Trạng thái thành công hay thất bại.
    pub status: EntryStatus,
    /// Dấu thời gian Unix (giây) khi tác vụ hoàn thành.
    pub finished_at_unix: u64,
}

impl HistoryEntry {
    /// Khởi tạo một bản ghi lịch sử mới, tự động chuyển đổi `Path` sang UTF-8 lossily.
    pub fn new(
        path: impl AsRef<Path>,
        title: impl Into<String>,
        detail: impl Into<String>,
        status: EntryStatus,
        finished_at_unix: u64,
    ) -> Self {
        Self {
            path: path.as_ref().to_string_lossy().into_owned(),
            title: title.into(),
            detail: detail.into(),
            status,
            finished_at_unix,
        }
    }
}

/// Quản lý lịch sử các tác vụ xử lý ảnh gần nhất, lưu trữ dưới dạng JSON trên đĩa.
///
/// Lịch sử được giới hạn tối đa `MAX_ENTRIES` (50) bản ghi mới nhất.
/// Các thao tác ghi đĩa được thực hiện nguyên tử (atomic save) thông qua file tạm để tránh hỏng dữ liệu.
#[derive(Debug)]
pub struct History {
    path: PathBuf,
    entries: Vec<HistoryEntry>,
}

impl History {
    /// Mở hoặc khởi tạo lịch sử từ đường dẫn `path`.
    ///
    /// - Nếu file chưa tồn tại: bắt đầu với danh sách rỗng mà không báo lỗi.
    /// - Nếu file bị hỏng hoặc sai cú pháp JSON: ghi cảnh báo qua `tracing::warn!`,
    ///   giữ nguyên file hỏng trên đĩa (không tự động xoá để người dùng có thể cứu dữ liệu),
    ///   và trả về danh sách rỗng trong bộ nhớ. File chỉ bị ghi đè khi có thao tác ghi mới (`push` hoặc `clear`).
    pub fn open(path: &Path) -> Self {
        let entries = match std::fs::read_to_string(path) {
            Ok(content) => match serde_json::from_str::<Vec<HistoryEntry>>(&content) {
                Ok(mut list) => {
                    list.truncate(MAX_ENTRIES);
                    list
                }
                Err(err) => {
                    tracing::warn!(
                        path = %path.display(),
                        error = %err,
                        "File lịch sử tại {} bị hỏng hoặc sai định dạng: {err}",
                        path.display()
                    );
                    Vec::new()
                }
            },
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Vec::new(),
            Err(err) => {
                tracing::warn!(
                    path = %path.display(),
                    error = %err,
                    "Không thể đọc file lịch sử tại {}: {err}",
                    path.display()
                );
                Vec::new()
            }
        };

        Self {
            path: path.to_path_buf(),
            entries,
        }
    }

    /// Thêm một bản ghi mới lên đầu danh sách, cắt bớt nếu vượt quá `MAX_ENTRIES` (50),
    /// và lưu nguyên tử xuống đĩa.
    pub fn push(&mut self, entry: HistoryEntry) -> Result<(), String> {
        self.entries.insert(0, entry);
        self.entries.truncate(MAX_ENTRIES);
        self.save()
    }

    /// Lấy danh sách tối đa `n` bản ghi mới nhất.
    pub fn recent(&self, n: usize) -> &[HistoryEntry] {
        let count = n.min(self.entries.len());
        &self.entries[..count]
    }

    /// Toàn bộ danh sách bản ghi hiện có trong bộ nhớ.
    pub fn entries(&self) -> &[HistoryEntry] {
        &self.entries
    }

    /// Kiểm tra xem danh sách lịch sử có rỗng hay không.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Số lượng bản ghi hiện có.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Đường dẫn file lịch sử trên đĩa.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Xoá toàn bộ lịch sử trong bộ nhớ và cập nhật file rỗng trên đĩa.
    pub fn clear(&mut self) -> Result<(), String> {
        self.entries.clear();
        self.save()
    }

    /// Ghi danh sách bản ghi hiện tại ra đĩa một cách nguyên tử (atomic save).
    fn save(&self) -> Result<(), String> {
        let parent = match self.path.parent() {
            Some(p) if !p.as_os_str().is_empty() => p,
            _ => Path::new("."),
        };

        if parent != Path::new(".") {
            std::fs::create_dir_all(parent).map_err(|e| {
                format!(
                    "Không thể tạo thư mục chứa file lịch sử tại {}: {e}",
                    parent.display()
                )
            })?;
        }

        let json_bytes = serde_json::to_vec_pretty(&self.entries)
            .map_err(|e| format!("Không thể tuần tự hoá lịch sử: {e}"))?;

        let mut temp_file = tempfile::NamedTempFile::new_in(parent)
            .map_err(|e| format!("Không thể tạo file tạm trong {}: {e}", parent.display()))?;

        temp_file
            .write_all(&json_bytes)
            .map_err(|e| format!("Không thể ghi dữ liệu lịch sử vào file tạm: {e}"))?;

        temp_file
            .flush()
            .map_err(|e| format!("Không thể đẩy dữ liệu lịch sử ra đĩa: {e}"))?;

        temp_file
            .persist(&self.path)
            .map_err(|e| format!("Không thể lưu file lịch sử tại {}: {e}", self.path.display()))?;

        Ok(())
    }
}
