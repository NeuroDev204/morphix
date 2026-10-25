use std::path::PathBuf;

use morphix_core::kind::{ArchiveFormat, MediaFormat};
use morphix_core::{actions_for, ActionKind, FileKind, ImgFormat, Operation, Quality, ResizeSpec};
use serde::{Deserialize, Serialize};

use crate::settings::Settings;

/// Mục hiển thị trên vòng tròn tác vụ nhanh (Quick Menu wheel).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WheelItem {
    /// Định danh duy nhất và ổn định của thao tác (ví dụ: `convert:<ext>`, `merge`, `compress`, `strip`, `resize`).
    pub id: String,
    /// Nhãn hiển thị tiếng Việt trên giao diện (lấy nguyên văn từ `Action.label`).
    pub label: String,
    /// `true` cho nhóm Công cụ (giữ Alt hoặc tab Công cụ), `false` cho nhóm Chuyển đổi thông thường.
    pub is_tool: bool,
    /// Đánh dấu thao tác gợi ý mặc định (mục đầu tiên của nhóm Chuyển đổi).
    pub is_suggested: bool,
}

/// Danh sách định danh công cụ đã được nối đầy đủ từ lõi xử lý đến giao diện.
///
/// Các công cụ bổ sung trong tương lai nếu chưa có giao diện sẽ bị lọc bỏ khỏi vòng tròn
/// để đảm bảo người dùng chỉ tương tác với các tính năng đã sẵn sàng.
pub const WIRED_TOOL_IDS: [&str; 4] = ["compress", "strip", "resize", "compress-media"];

fn action_to_id(kind: ActionKind) -> String {
    match kind {
        ActionKind::ConvertTo(format) => format!("convert:{}", format.extension()),
        ActionKind::MergeToPdf => "merge".to_string(),
        ActionKind::Compress => "compress".to_string(),
        ActionKind::StripMetadata => "strip".to_string(),
        ActionKind::Resize => "resize".to_string(),
        ActionKind::ConvertText(format) => format!("text:{}", format.extension()),
        ActionKind::ConvertMedia(format) => format!("media:{}", format.extension()),
        ActionKind::CompressMedia => "compress-media".to_string(),
        ActionKind::CutMedia => "cut".to_string(),
        ActionKind::ChangeSpeed => "speed".to_string(),
        ActionKind::ExtractAudio(format) => format!("extract:{}", format.extension()),
        ActionKind::PdfToText => "pdf-to-text".to_string(),
        ActionKind::CreateArchive(format) => format!("archive:{}", format.extension()),
        ActionKind::ExtractArchive => "extract".to_string(),
    }
}

/// Chuyển đổi danh sách định dạng file được chọn thành danh sách các múi tác vụ cho vòng tròn.
///
/// Lấy hành động từ `actions_for`, loại trừ các công cụ không nằm trong `WIRED_TOOL_IDS`,
/// và gán `is_suggested = true` duy nhất cho mục đầu tiên của nhóm Chuyển đổi.
pub fn wheel_items(kinds: &[FileKind]) -> Vec<WheelItem> {
    let actions = actions_for(kinds);
    let mut items = Vec::with_capacity(actions.len());
    let mut suggested_found = false;

    for action in actions {
        // WHY: giao diện chưa nối được việc văn bản (Task 4); hiện múi sẽ bấm ra lỗi "không hỗ trợ".
        // WHY: cắt, đổi tốc độ và tách âm thanh chỉ có ở CLI trong bản này (spec mục 2a), nên không hiện múi.
        if matches!(
            action.kind,
            ActionKind::ConvertText(_)
                | ActionKind::CutMedia
                | ActionKind::ChangeSpeed
                | ActionKind::ExtractAudio(_)
        ) {
            continue;
        }
        let id = action_to_id(action.kind);
        if action.is_tool && !WIRED_TOOL_IDS.contains(&id.as_str()) {
            continue;
        }

        let is_suggested = if !action.is_tool && !suggested_found {
            suggested_found = true;
            true
        } else {
            false
        };

        items.push(WheelItem {
            id,
            label: action.label,
            is_tool: action.is_tool,
            is_suggested,
        });
    }

    items
}

/// Ánh xạ định danh mục vòng tròn (`id`) sang `Operation` thực thi của `morphix-core`.
///
/// - `convert:<ext>` → `ConvertImage { to }`
/// - `merge` → `MergeImagesToPdf`
/// - `compress` → `CompressImage`
/// - `strip` → `StripMetadata`
/// - `resize` → `ResizeImage(ResizeSpec::Percent(settings.default_resize_percent))`
/// - `media:<đuôi>` → `ConvertMedia { to }`; `compress-media` → `CompressMedia`
/// - `pdf-to-text` → `PdfToText`
/// - `archive:<đuôi>` → `CreateArchive { format }`; `extract` → `ExtractArchive`
/// - Các mã khác hoặc định dạng không nhận dạng được trả về `Err`.
pub fn operation_for(
    id: &str,
    _inputs: &[PathBuf],
    settings: &Settings,
) -> Result<Operation, String> {
    if let Some(ext) = id.strip_prefix("convert:") {
        let to = ImgFormat::from_extension(ext)
            .ok_or_else(|| format!("Định dạng ảnh không được hỗ trợ: {ext}"))?;
        return Ok(Operation::ConvertImage { to });
    }
    if let Some(ext) = id.strip_prefix("archive:") {
        let format = match ext {
            "zip" => ArchiveFormat::Zip,
            "tar.gz" => ArchiveFormat::TarGz,
            "7z" => ArchiveFormat::SevenZ,
            _ => return Err(format!("Định dạng file nén không được hỗ trợ: {ext}")),
        };
        return Ok(Operation::CreateArchive { format });
    }
    if let Some(ext) = id.strip_prefix("media:") {
        let to = MediaFormat::from_extension(ext)
            .ok_or_else(|| format!("Định dạng âm thanh, video không được hỗ trợ: {ext}"))?;
        return Ok(Operation::ConvertMedia { to });
    }

    match id {
        "merge" => Ok(Operation::MergeImagesToPdf),
        "compress" => Ok(Operation::CompressImage),
        "compress-media" => Ok(Operation::CompressMedia),
        "strip" => Ok(Operation::StripMetadata),
        "pdf-to-text" => Ok(Operation::PdfToText),
        "extract" => Ok(Operation::ExtractArchive),
        "resize" => Ok(Operation::ResizeImage(ResizeSpec::Percent(
            settings.default_resize_percent,
        ))),
        _ => Err(format!("Thao tác không được hỗ trợ: {id}")),
    }
}

/// Chuyển đổi cấu hình chất lượng trong `Settings` sang kiểu `Quality` của `morphix-core`.
pub fn quality_for(settings: &Settings) -> Quality {
    settings.quality.into()
}
