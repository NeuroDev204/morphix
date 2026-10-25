//! Gỡ bản Morphix cài trong home bằng lệnh curl (`<prefix>/bin`, `<prefix>/lib/morphix`, ...).

use std::{
    fs, io,
    path::{Path, PathBuf},
};

/// File do `install.sh` ghi trong `<prefix>/lib/morphix/`, nội dung là phiên bản. WHY: chỉ xoá cả thư mục
/// khi chắc nó do bản cài của Morphix tạo ra, giống ownership marker của các file tích hợp.
pub const INSTALL_MARKER: &str = ".morphix-install";

/// `prefix` là bản cài bằng lệnh curl khi có marker. WHY bắt buộc cả khi thiếu `lib/morphix`: chạy từ thư mục lạ
/// không được xoá `bin/morphix` của người khác.
pub fn is_user_install(prefix: &Path) -> bool {
    prefix.join("lib/morphix").join(INSTALL_MARKER).is_file()
}

/// Xoá binary, thư viện đi kèm, icon và tài liệu của bản cài trong `prefix`; `bin/morphix` xoá sau cùng.
/// Mục không tồn tại thì bỏ qua. Trả danh sách đã xoá.
pub fn remove_user_install(prefix: &Path) -> Result<Vec<PathBuf>, String> {
    let lib = prefix.join("lib/morphix");
    if !is_user_install(prefix) {
        return Err(format!(
            "{} không có {INSTALL_MARKER}; đây không phải bản cài Morphix bằng lệnh curl, nên không xoá gì",
            lib.display()
        ));
    }
    let targets = [
        prefix.join("bin/morphix-gui"),
        lib,
        prefix.join("share/icons/hicolor/128x128/apps/morphix.png"),
        prefix.join("share/doc/morphix"),
        prefix.join("bin/morphix"),
    ];
    let mut removed = Vec::new();
    for target in targets {
        let result = if target.is_dir() {
            fs::remove_dir_all(&target)
        } else {
            fs::remove_file(&target)
        };
        match result {
            Ok(()) => removed.push(target),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(format!("không xoá được {}: {error}", target.display())),
        }
    }
    Ok(removed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn touch(path: &Path) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, b"x").unwrap();
    }

    #[test]
    fn removes_owned_files_and_keeps_others() {
        let prefix = tempfile::tempdir().unwrap();
        let p = prefix.path();
        for rel in [
            "bin/morphix",
            "bin/morphix-gui",
            "bin/other-tool",
            "lib/morphix/.morphix-install",
            "lib/morphix/ffmpeg",
            "share/icons/hicolor/128x128/apps/morphix.png",
            "share/doc/morphix/LICENSE",
        ] {
            touch(&p.join(rel));
        }
        let removed = remove_user_install(p).unwrap();
        assert!(removed.contains(&p.join("bin/morphix")));
        assert!(p.join("bin/other-tool").is_file());
        for rel in [
            "bin/morphix",
            "bin/morphix-gui",
            "lib/morphix",
            "share/icons/hicolor/128x128/apps/morphix.png",
            "share/doc/morphix",
        ] {
            assert!(!p.join(rel).exists(), "{rel}");
        }
    }

    #[test]
    fn refuses_when_prefix_is_not_a_morphix_install() {
        let prefix = tempfile::tempdir().unwrap();
        touch(&prefix.path().join("bin/morphix"));
        assert!(remove_user_install(prefix.path()).is_err());
        assert!(prefix.path().join("bin/morphix").is_file());
    }

    #[test]
    fn refuses_to_delete_lib_dir_without_install_marker() {
        let prefix = tempfile::tempdir().unwrap();
        let p = prefix.path();
        touch(&p.join("bin/morphix"));
        touch(&p.join("lib/morphix/ffmpeg"));
        let error = remove_user_install(p).unwrap_err();
        assert!(error.contains(INSTALL_MARKER), "{error}");
        assert!(p.join("bin/morphix").is_file());
        assert!(p.join("lib/morphix/ffmpeg").is_file());
    }
}
