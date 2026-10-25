use std::{
    ffi::OsString,
    fs::{File, OpenOptions},
    io::ErrorKind,
    path::{Path, PathBuf},
};

use tempfile::NamedTempFile;

use crate::error::{MorphixError, Result};

/// Giới hạn số lần thử ` (n)`; tránh vòng lặp vô hạn khi thư mục đầy file trùng tên.
const MAX_COLLISION_SUFFIX: u32 = 9999;

/// Tên đầu ra mong muốn: cùng thư mục với file gốc (hoặc `out_dir`), `stem` + hậu tố + đuôi mới.
/// Không đụng đến đĩa; việc xử lý trùng tên nằm ở `write_output`.
pub fn desired_output(
    input: &Path,
    suffix: &str,
    extension: &str,
    out_dir: Option<&Path>,
) -> PathBuf {
    let mut name: OsString = input.file_stem().unwrap_or_default().to_os_string();
    name.push(suffix);
    name.push(".");
    name.push(extension);
    let dir = out_dir
        .or_else(|| input.parent())
        .unwrap_or_else(|| Path::new(""));
    dir.join(name)
}

/// Đuôi của file gốc để giữ nguyên (`.jpeg` vẫn là `.jpeg`); file không có đuôi thì dùng `fallback`.
pub fn extension_of(input: &Path, fallback: &str) -> String {
    input
        .extension()
        .and_then(|e| e.to_str())
        .map_or_else(|| fallback.to_string(), str::to_string)
}

/// Ghi một file mới mà không bao giờ ghi đè file có sẵn: viết vào file tạm cùng thư mục rồi đổi tên
/// kiểu "không đè". Đụng tên thì thử `stem (1).ext`, `stem (2).ext`... Nếu `write` lỗi, file tạm
/// bị xoá (Drop), không để lại file dở.
pub fn write_output<F>(desired: &Path, write: F) -> Result<PathBuf>
where
    F: FnOnce(&mut File) -> Result<()>,
{
    let dir = match desired.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent,
        _ => Path::new("."),
    };
    let io_error = |source| MorphixError::Io {
        path: desired.to_path_buf(),
        source,
    };

    std::fs::create_dir_all(dir).map_err(io_error)?;
    let mut temp = NamedTempFile::new_in(dir).map_err(io_error)?;
    write(temp.as_file_mut())?;

    let (stem, extension) = split_for_collision(desired);
    for attempt in 0..=MAX_COLLISION_SUFFIX {
        let candidate = if attempt == 0 {
            desired.to_path_buf()
        } else {
            let name = match &extension {
                Some(ext) => format!("{stem} ({attempt}).{ext}"),
                None => format!("{stem} ({attempt})"),
            };
            dir.join(name)
        };
        match temp.persist_noclobber(&candidate) {
            Ok(_) => return Ok(candidate),
            // PersistError trả lại file tạm, nên thử số kế tiếp mà không phải ghi lại từ đầu.
            Err(failure) if failure.error.kind() == ErrorKind::AlreadyExists => temp = failure.file,
            Err(failure) => return Err(io_error(failure.error)),
        }
    }
    Err(MorphixError::InvalidInput(format!(
        "quá nhiều file trùng tên với {}",
        desired.display()
    )))
}

/// Tên gốc và đuôi để đánh số ` (n)`. `.tar.gz` là một đuôi kép: nếu tách theo `Path::extension`
/// thì `x-nen.tar.gz` đụng tên sẽ thành `x-nen.tar (1).gz`.
fn split_for_collision(desired: &Path) -> (String, Option<String>) {
    let name: String = desired
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    if let Some(stem) = name.strip_suffix(".tar.gz") {
        return (stem.to_string(), Some("tar.gz".to_string()));
    }
    let stem: String = desired
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    let extension: Option<String> = desired
        .extension()
        .map(|e| e.to_string_lossy().into_owned());
    (stem, extension)
}

/// Như `write_output`, nhưng `write` nhận đường dẫn file tạm (cho ffmpeg chạy từ dòng lệnh). File tạm
/// nằm trong thư mục tạm riêng cùng thư mục đích và **chưa tồn tại** khi `write` bắt đầu, vì ffmpeg với
/// `-n` từ chối ghi đè file có sẵn. Huỷ hoặc lỗi thì xoá cả thư mục tạm, nên không còn file dở.
pub fn write_output_path<F>(desired: &Path, write: F) -> Result<PathBuf>
where
    F: FnOnce(&Path) -> Result<()>,
{
    let dir = match desired.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent,
        _ => Path::new("."),
    };
    let io_error = |source| MorphixError::Io {
        path: desired.to_path_buf(),
        source,
    };

    std::fs::create_dir_all(dir).map_err(io_error)?;
    let work = tempfile::Builder::new()
        .prefix(".morphix-")
        .tempdir_in(dir)
        .map_err(io_error)?;
    let encoded: PathBuf = match desired.extension() {
        Some(ext) => work
            .path()
            .join(format!("encoded.{}", ext.to_string_lossy())),
        None => work.path().join("encoded"),
    };
    write(&encoded)?;

    let stem = desired
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    let extension = desired
        .extension()
        .map(|e| e.to_string_lossy().into_owned());
    for attempt in 0..=MAX_COLLISION_SUFFIX {
        let candidate: PathBuf = if attempt == 0 {
            desired.to_path_buf()
        } else {
            let name = match &extension {
                Some(ext) => format!("{stem} ({attempt}).{ext}"),
                None => format!("{stem} ({attempt})"),
            };
            dir.join(name)
        };
        // WHY: tạo file giữ chỗ với create_new là cách không ghi đè trên mọi hệ điều hành (kể cả
        // FAT, nơi không có hard link). File giữ chỗ là của mình nên đổi tên đè lên nó là an toàn.
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate)
        {
            Ok(placeholder) => {
                // WHY: Windows không đổi tên được file đang mở, nên đóng trước khi đổi tên.
                drop(placeholder);
                if let Err(source) = std::fs::rename(&encoded, &candidate) {
                    // Lỗi đổi tên là lỗi chính; file giữ chỗ còn sót thì ghi log để người dùng biết.
                    if let Err(cleanup) = std::fs::remove_file(&candidate) {
                        tracing::warn!(path = %candidate.display(), %cleanup, "không xoá được file giữ chỗ");
                    }
                    return Err(io_error(source));
                }
                return Ok(candidate);
            }
            Err(source) if source.kind() == ErrorKind::AlreadyExists => continue,
            Err(source) => return Err(io_error(source)),
        }
    }
    Err(MorphixError::InvalidInput(format!(
        "quá nhiều file trùng tên với {}",
        desired.display()
    )))
}

/// Như `write_output`, nhưng file mới mang quyền truy cập của file gốc `template`. `NamedTempFile`
/// tạo file với quyền 0600 trên Unix, khiến kết quả nằm cạnh file gốc mà người khác (máy chủ web,
/// dịch vụ đồng bộ) không đọc được.
pub fn write_output_like<F>(desired: &Path, template: &Path, write: F) -> Result<PathBuf>
where
    F: FnOnce(&mut File) -> Result<()>,
{
    let created = write_output(desired, write)?;
    mirror_permissions(template, &created);
    Ok(created)
}

#[cfg(unix)]
fn mirror_permissions(template: &Path, created: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let result = std::fs::metadata(template).and_then(|meta| {
        let mode = meta.permissions().mode() & 0o777;
        std::fs::set_permissions(created, std::fs::Permissions::from_mode(mode))
    });
    // File đã tạo xong và đúng nội dung; chỉ quyền chặt hơn mong muốn, nên ghi log chứ không làm hỏng việc.
    if let Err(error) = result {
        tracing::warn!(created = %created.display(), %error, "không chép được quyền truy cập từ file gốc");
    }
}

#[cfg(not(unix))]
fn mirror_permissions(_template: &Path, _created: &Path) {}

/// Sắp theo tên file, không phân biệt hoa thường, số được so theo giá trị (`IMG_2` trước `IMG_10`).
pub fn natural_sort(paths: &mut [PathBuf]) {
    let name = |p: &PathBuf| {
        p.file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned()
    };
    paths.sort_by(|a, b| natord::compare_ignore_case(&name(a), &name(b)));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::MorphixError;
    use std::io::Write;
    use std::path::{Path, PathBuf};

    fn io_err(path: &Path, source: std::io::Error) -> MorphixError {
        MorphixError::Io {
            path: path.to_path_buf(),
            source,
        }
    }

    #[test]
    fn desired_output_keeps_unicode_stem_and_adds_suffix() {
        let p = desired_output(Path::new("/x/Ảnh chụp màn hình 1.png"), "-nen", "png", None);
        assert_eq!(p, PathBuf::from("/x/Ảnh chụp màn hình 1-nen.png"));
    }

    #[test]
    fn write_output_never_overwrites_existing_file() {
        let dir = tempfile::tempdir().unwrap();
        let wanted = dir.path().join("a.pdf");
        std::fs::write(&wanted, b"original").unwrap();
        let made = write_output(&wanted, |f| {
            f.write_all(b"new").map_err(|e| io_err(&wanted, e))
        })
        .unwrap();
        assert_eq!(made, dir.path().join("a (1).pdf"));
        assert_eq!(std::fs::read(&wanted).unwrap(), b"original");
        let again = write_output(&wanted, |f| {
            f.write_all(b"x").map_err(|e| io_err(&wanted, e))
        })
        .unwrap();
        assert_eq!(again, dir.path().join("a (2).pdf"));
    }

    #[test]
    fn failed_write_leaves_no_temp_file() {
        let dir = tempfile::tempdir().unwrap();
        let r = write_output(&dir.path().join("a.pdf"), |_| Err(MorphixError::Cancelled));
        assert!(matches!(r, Err(MorphixError::Cancelled)));
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
    }

    #[test]
    fn natural_sort_orders_numbers_by_value() {
        let mut v: Vec<PathBuf> = ["IMG_10.jpg", "img_2.jpg", "IMG_1.jpg"]
            .iter()
            .map(PathBuf::from)
            .collect();
        natural_sort(&mut v);
        assert_eq!(
            v,
            vec![
                PathBuf::from("IMG_1.jpg"),
                PathBuf::from("img_2.jpg"),
                PathBuf::from("IMG_10.jpg")
            ]
        );
    }
}
