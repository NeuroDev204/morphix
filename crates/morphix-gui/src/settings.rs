use std::io::Write;
use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};

/// Lựa chọn chất lượng xuất hình ảnh ở giao diện người dùng.
///
/// Ánh xạ 1-1 sang `morphix_core::Quality` nhưng được đặt tên và serialize dưới dạng chuỗi thường
/// (`lowercase`) để tương thích với JSON cài đặt và giao diện người dùng.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum QualityChoice {
    Small,
    #[default]
    Balanced,
    High,
}

impl From<QualityChoice> for morphix_core::Quality {
    fn from(choice: QualityChoice) -> Self {
        match choice {
            QualityChoice::Small => morphix_core::Quality::Small,
            QualityChoice::Balanced => morphix_core::Quality::Balanced,
            QualityChoice::High => morphix_core::Quality::High,
        }
    }
}

impl From<morphix_core::Quality> for QualityChoice {
    fn from(quality: morphix_core::Quality) -> Self {
        match quality {
            morphix_core::Quality::Small => QualityChoice::Small,
            morphix_core::Quality::Balanced => QualityChoice::Balanced,
            morphix_core::Quality::High => QualityChoice::High,
        }
    }
}

/// Cấu hình lưu trữ của Morphix GUI.
///
/// Các trường được gán giá trị mặc định khi deserializing thiếu trường
/// để đảm bảo khả năng tương thích xuôi khi bổ sung cấu hình trong tương lai.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Chất lượng mặc định khi nén/chuyển đổi.
    pub quality: QualityChoice,
    /// Thư mục xuất ảnh cố định (nếu `None`, ghi đè hoặc lưu cạnh file gốc tuỳ tác vụ).
    pub output_dir: Option<PathBuf>,
    /// Tỉ lệ phần trăm thu nhỏ mặc định (từ 1 đến 100).
    pub default_resize_percent: u8,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            quality: QualityChoice::Balanced,
            output_dir: None,
            default_resize_percent: 50,
        }
    }
}

/// Đọc cấu hình từ đường dẫn `path`.
///
/// Nguyên tắc an toàn:
/// - Nếu file chưa tồn tại: trả về `Settings::default()` mà không báo lỗi (lần đầu khởi chạy).
/// - Nếu file bị hỏng hoặc JSON sai cú pháp: ghi log cảnh báo bằng `tracing::warn!`,
///   giữ nguyên file hỏng (không tự ý xoá hay ghi đè để người dùng có thể cứu dữ liệu),
///   và trả về `Settings::default()`.
pub fn load(path: &Path) -> Settings {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Settings::default();
        }
        Err(e) => {
            tracing::warn!(
                path = %path.display(),
                error = %e,
                "Không thể đọc file cài đặt tại {}: {e}",
                path.display()
            );
            return Settings::default();
        }
    };

    match serde_json::from_str::<Settings>(&content) {
        Ok(settings) => settings,
        Err(e) => {
            tracing::warn!(
                path = %path.display(),
                error = %e,
                "File cài đặt tại {} bị hỏng hoặc sai định dạng: {e}",
                path.display()
            );
            Settings::default()
        }
    }
}

/// Lưu cấu hình ra file `path` một cách an toàn (atomic save).
///
/// Ghi vào file tạm trong cùng thư mục cha rồi đổi tên (rename) đè lên `path`.
/// Thao tác này tránh trường hợp ứng dụng bị tắt đột ngột khiến file cài đặt bị hỏng nửa chừng.
pub fn save(path: &Path, settings: &Settings) -> Result<(), String> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    if !parent.as_os_str().is_empty() {
        std::fs::create_dir_all(parent).map_err(|e| {
            format!(
                "Không thể tạo thư mục chứa file cài đặt tại {}: {e}",
                parent.display()
            )
        })?;
    }

    let json_bytes = serde_json::to_vec_pretty(settings)
        .map_err(|e| format!("Không thể tuần tự hoá cài đặt: {e}"))?;

    let mut temp_file = tempfile::NamedTempFile::new_in(parent)
        .map_err(|e| format!("Không thể tạo file tạm trong {}: {e}", parent.display()))?;

    temp_file
        .write_all(&json_bytes)
        .map_err(|e| format!("Không thể ghi dữ liệu cài đặt vào file tạm: {e}"))?;

    temp_file
        .flush()
        .map_err(|e| format!("Không thể đẩy dữ liệu cài đặt ra đĩa: {e}"))?;

    temp_file
        .persist(path)
        .map_err(|e| format!("Không thể lưu file cài đặt tại {}: {e}", path.display()))?;

    Ok(())
}

/// Kiểm tra tính hợp lệ của cấu hình trước khi áp dụng hoặc lưu.
///
/// - `default_resize_percent` bắt buộc trong khoảng `1..=100`.
/// - `output_dir` nếu có giá trị thì phải tạo được (hoặc đã tồn tại) trên đĩa.
pub fn validate(settings: &Settings) -> Result<(), String> {
    if !(1..=100).contains(&settings.default_resize_percent) {
        return Err(format!(
            "Phần trăm thu nhỏ mặc định phải trong khoảng 1..=100, nhận được {}",
            settings.default_resize_percent
        ));
    }

    if let Some(ref dir) = settings.output_dir {
        std::fs::create_dir_all(dir).map_err(|e| {
            format!(
                "Không thể tạo thư mục đầu ra tại {}: {e}",
                dir.display()
            )
        })?;
    }

    Ok(())
}
