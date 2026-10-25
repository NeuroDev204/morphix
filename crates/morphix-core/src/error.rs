use std::path::PathBuf;

/// Mọi lỗi của lõi. Thông báo nói được nguyên nhân bằng tiếng Việt để CLI và giao diện
/// hiện thẳng cho người dùng mà không cần dịch lại.
#[derive(Debug, thiserror::Error)]
pub enum MorphixError {
    #[error("không đọc hoặc ghi được {}: {source}", path.display())]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("{} không phải loại file được hỗ trợ cho việc này", path.display())]
    Unsupported { path: PathBuf },
    #[error("{} bị hỏng: {reason}", path.display())]
    Corrupt { path: PathBuf, reason: String },
    #[error("không nén thêm được {}", path.display())]
    NotSmaller { path: PathBuf },
    #[error("{0}")]
    InvalidInput(String),
    #[error("đã huỷ")]
    Cancelled,
}

pub type Result<T> = std::result::Result<T, MorphixError>;
