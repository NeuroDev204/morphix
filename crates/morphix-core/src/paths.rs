use std::ffi::OsString;
use std::path::{Path, PathBuf};

use directories::ProjectDirs;

/// Thư mục dữ liệu của app (log, lịch sử, cài đặt). Biến `MORPHIX_DATA_DIR` ghi đè để test
/// và người dùng nâng cao tách dữ liệu khỏi thư mục hệ thống.
pub fn data_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("MORPHIX_DATA_DIR") {
        return PathBuf::from(dir);
    }
    match ProjectDirs::from("", "", "Morphix") {
        Some(dirs) => dirs.data_local_dir().to_path_buf(),
        None => {
            // Môi trường lạ (không có HOME): lùi về thư mục tạm thay vì làm app không chạy được.
            let fallback = std::env::temp_dir().join("morphix");
            tracing::warn!(path = %fallback.display(), "không xác định được thư mục dữ liệu, dùng thư mục tạm");
            fallback
        }
    }
}

/// Thư mục lib đi kèm bản đóng gói Linux, tính từ binary đang chạy: `/usr/bin/morphix` tìm ở
/// `/usr/lib/morphix`. Nơi khác (Windows, chạy từ cargo) trả `None` để hành vi không đổi.
#[cfg(target_os = "linux")]
fn bundled_lib_dir() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    std::fs::canonicalize(exe.parent()?.join("..").join("lib").join("morphix")).ok()
}

#[cfg(not(target_os = "linux"))]
fn bundled_lib_dir() -> Option<PathBuf> {
    None
}

/// Đường dẫn `ffmpeg` để chạy video và âm thanh (đợt 2). Thứ tự: bản đi kèm, rồi
/// `MORPHIX_FFMPEG`, rồi PATH.
/// WHY: bản đi kèm đứng trước để máy có ffmpeg khác trong PATH vẫn chạy đúng bản đã kiểm
/// (cấu hình GPL có libx264). `MORPHIX_FFMPEG` chỉ để test trỏ tới ffmpeg giả.
pub fn find_ffmpeg() -> Option<PathBuf> {
    pick_ffmpeg(
        bundled_lib_dir().map(|dir| dir.join("ffmpeg")),
        std::env::var_os("MORPHIX_FFMPEG"),
        || which::which("ffmpeg").ok(),
    )
}

fn pick_ffmpeg(
    bundled: Option<PathBuf>,
    env_override: Option<OsString>,
    on_path: impl FnOnce() -> Option<PathBuf>,
) -> Option<PathBuf> {
    if let Some(path) = bundled.filter(|path| path.is_file()) {
        return Some(path);
    }
    if let Some(path) = env_override {
        return Some(PathBuf::from(path));
    }
    on_path()
}

/// Thư mục chứa `libpdfium.so` để nạp PDFium. `None` nghĩa là hỏi thư viện hệ thống.
/// Thứ tự giống `find_ffmpeg`: bản đi kèm, rồi `MORPHIX_PDFIUM_DIR`.
pub fn find_pdfium_dir() -> Option<PathBuf> {
    pick_pdfium_dir(
        bundled_lib_dir().map(|dir| dir.join("pdfium")),
        std::env::var_os("MORPHIX_PDFIUM_DIR"),
    )
}

fn pick_pdfium_dir(bundled: Option<PathBuf>, env_override: Option<OsString>) -> Option<PathBuf> {
    bundled
        .filter(|dir| dir.join("libpdfium.so").is_file())
        .or_else(|| env_override.map(PathBuf::from))
}

/// Thử tạo rồi xoá một file tạm trong thư mục dữ liệu để biết app có ghi log, lịch sử được không.
pub fn check_dir_writable(dir: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    tempfile::NamedTempFile::new_in(dir).map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn data_dir_honours_env_override() {
        std::env::set_var("MORPHIX_DATA_DIR", "/tmp/morphix-test-dir");
        assert_eq!(
            data_dir(),
            std::path::PathBuf::from("/tmp/morphix-test-dir")
        );
    }

    #[test]
    fn pick_ffmpeg_prefers_bundled_file_over_env_and_path() {
        let dir: tempfile::TempDir = tempfile::tempdir().expect("tạo thư mục tạm");
        let bundled: PathBuf = dir.path().join("ffmpeg");
        std::fs::write(&bundled, b"").expect("ghi file giả");
        let picked: Option<PathBuf> =
            pick_ffmpeg(Some(bundled.clone()), Some("/env/ffmpeg".into()), || {
                Some(PathBuf::from("/path/ffmpeg"))
            });
        assert_eq!(picked, Some(bundled));
    }

    #[test]
    fn pick_ffmpeg_skips_bundled_path_that_is_missing() {
        let picked: Option<PathBuf> = pick_ffmpeg(
            Some(PathBuf::from("/khong/co/ffmpeg")),
            Some("/env/ffmpeg".into()),
            || None,
        );
        assert_eq!(picked, Some(PathBuf::from("/env/ffmpeg")));
    }

    #[test]
    fn pick_ffmpeg_uses_env_before_path() {
        let picked: Option<PathBuf> = pick_ffmpeg(None, Some("/env/ffmpeg".into()), || {
            Some(PathBuf::from("/path/ffmpeg"))
        });
        assert_eq!(picked, Some(PathBuf::from("/env/ffmpeg")));
    }

    #[test]
    fn pick_ffmpeg_falls_back_to_path() {
        let picked: Option<PathBuf> =
            pick_ffmpeg(None, None, || Some(PathBuf::from("/path/ffmpeg")));
        assert_eq!(picked, Some(PathBuf::from("/path/ffmpeg")));
    }

    #[test]
    fn pick_pdfium_dir_prefers_bundled_library_over_env() {
        let dir: tempfile::TempDir = tempfile::tempdir().expect("tạo thư mục tạm");
        std::fs::write(dir.path().join("libpdfium.so"), b"").expect("ghi file giả");
        let picked: Option<PathBuf> =
            pick_pdfium_dir(Some(dir.path().to_path_buf()), Some("/env/pdfium".into()));
        assert_eq!(picked, Some(dir.path().to_path_buf()));
    }

    #[test]
    fn pick_pdfium_dir_skips_bundled_dir_without_library() {
        let dir: tempfile::TempDir = tempfile::tempdir().expect("tạo thư mục tạm");
        let picked: Option<PathBuf> =
            pick_pdfium_dir(Some(dir.path().to_path_buf()), Some("/env/pdfium".into()));
        assert_eq!(picked, Some(PathBuf::from("/env/pdfium")));
    }
}
