use std::io::Write;
use std::path::Path;
use std::sync::OnceLock;

use pdfium_render::prelude::{Pdfium, PdfiumError, PdfiumInternalError};

use crate::error::{MorphixError, Result};
use crate::ops::markdown::pages_to_markdown;
use crate::paths::find_pdfium_dir;

/// WHY: PDFium chỉ nạp một lần cho mỗi tiến trình. Bind lần hai trả lỗi, nên giữ một bản
/// dùng chung và lưu cả lỗi nạp để mọi lần gọi đều thấy cùng một nguyên nhân.
static PDFIUM: OnceLock<std::result::Result<Pdfium, String>> = OnceLock::new();

fn bind() -> std::result::Result<Pdfium, PdfiumError> {
    // WHY: `MORPHIX_PDFIUM_DIR` (cargo đặt sẵn lúc phát triển, xem `.cargo/config.toml`) được
    // đọc trong `find_pdfium_dir`, cùng thứ tự với bản đi kèm.
    let bindings = match find_pdfium_dir() {
        Some(dir) => Pdfium::bind_to_library(Pdfium::pdfium_platform_library_name_at_path(&dir))?,
        None => Pdfium::bind_to_system_library()?,
    };
    Ok(Pdfium::new(bindings))
}

fn pdfium() -> Result<&'static Pdfium> {
    PDFIUM
        .get_or_init(|| bind().map_err(|error| error.to_string()))
        .as_ref()
        .map_err(|reason| {
            MorphixError::InvalidInput(format!("không tải được thư viện PDFium: {reason}"))
        })
}

fn load_error(path: &Path, error: PdfiumError) -> MorphixError {
    let reason = match error {
        PdfiumError::IoError(source) => {
            return MorphixError::Io {
                path: path.to_path_buf(),
                source,
            }
        }
        PdfiumError::PdfiumLibraryInternalError(PdfiumInternalError::PasswordError) => {
            "file được bảo vệ bằng mật khẩu, Morphix chưa đọc được file có mật khẩu".to_string()
        }
        PdfiumError::PdfiumLibraryInternalError(PdfiumInternalError::SecurityError) => {
            "file bị chặn bởi cài đặt bảo mật của PDF".to_string()
        }
        PdfiumError::PdfiumLibraryInternalError(PdfiumInternalError::FormatError) => {
            "không đọc được cấu trúc PDF, file có thể bị hỏng".to_string()
        }
        other => format!("PDFium không mở được file: {other:?}"),
    };
    MorphixError::Corrupt {
        path: path.to_path_buf(),
        reason,
    }
}

/// Ghi lớp văn bản có sẵn của mọi trang PDF ra `out`. Không OCR.
pub fn pdf_to_text(path: &Path, out: &mut impl Write) -> Result<()> {
    let mut text = String::new();
    for page in pdf_pages_text(path)? {
        text.push_str(&page);
        text.push('\n');
    }
    write_all(path, out, &text)
}

/// Lớp văn bản ra Markdown: mỗi trang một mục `## Trang N`, không đoán cấu trúc trong trang.
pub fn pdf_to_markdown(path: &Path, out: &mut impl Write) -> Result<()> {
    write_all(path, out, &pages_to_markdown(&pdf_pages_text(path)?))
}

fn write_all(path: &Path, out: &mut impl Write, text: &str) -> Result<()> {
    out.write_all(text.as_bytes())
        .map_err(|source| MorphixError::Io {
            path: path.to_path_buf(),
            source,
        })
}

/// Chữ của từng trang. WHY: đọc hết mọi trang trước khi ghi byte nào. Nếu trang N không có chữ thì
/// cả file báo lỗi và `out` không nhận gì, để người gọi không bao giờ có file đầu ra cụt.
pub fn pdf_pages_text(path: &Path) -> Result<Vec<String>> {
    let pdfium = pdfium()?;
    let document = pdfium
        .load_pdf_from_file(path, None)
        .map_err(|error| load_error(path, error))?;

    let mut pages = Vec::new();
    for (index, page) in document.pages().iter().enumerate() {
        let number = index + 1;
        let text_layer = page.text().map_err(|error| MorphixError::Corrupt {
            path: path.to_path_buf(),
            reason: format!("không đọc được chữ của trang {number}: {error:?}"),
        })?;
        let text = text_layer.all();
        if text.trim().is_empty() {
            return Err(MorphixError::Corrupt {
                path: path.to_path_buf(),
                reason: format!("trang {number} không có chữ có thể chọn, cần OCR, chưa hỗ trợ"),
            });
        }
        pages.push(text);
    }
    Ok(pages)
}
