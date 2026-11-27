//! Bản chép DING mang UUID riêng (`morphix-desktop@morphix`) trong thư mục người dùng, có thêm menu Morphix.
//! WHY không chép cùng UUID: Ubuntu khai DING là extension của phiên (`modes/ubuntu.json`) nên GNOME bỏ qua bản trong
//! thư mục người dùng ("not loading ... as part of session mode").

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const FORK_UUID: &str = "morphix-desktop@morphix";
pub const DING_UUID: &str = "ding@rastersoft.com";
const FORK_NAME: &str = "Desktop Icons (Morphix)";
const MARKER_FILE: &str = "morphix-source.json";
const MENU_JS: &str = include_str!("../../../../integration/ding/morphixMenu.js");
const WATCH_JS: &str = include_str!("../../../../integration/ding/morphixWatch.js");
/// Các file mà bản chép sửa; vân tay lấy từ chính chúng để biết khi nào Ubuntu cập nhật DING.
const FINGERPRINT_FILES: [&str; 4] = ["metadata.json", "extension.js", "app/fileItemMenu.js", "app/desktopManager.js"];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ForkMarker {
    pub source: PathBuf,
    pub fingerprint: String,
    pub morphix_version: String,
}

/// Thư mục DING nguồn đầu tiên có `metadata.json`: `data_home` trước, rồi `data_dirs`. Không bao giờ là bản chép (khác tên thư mục).
pub fn find_source(data_home: &Path, data_dirs: &[PathBuf]) -> Option<PathBuf> {
    std::iter::once(data_home)
        .chain(data_dirs.iter().map(PathBuf::as_path))
        .map(|dir| dir.join("gnome-shell/extensions").join(DING_UUID))
        .find(|path| path.join("metadata.json").is_file())
}

/// SHA-256 (hex) của tên + nội dung các file mà bản chép sửa.
pub fn fingerprint(source: &Path) -> Result<String, String> {
    let mut hasher = Sha256::new();
    for relative in FINGERPRINT_FILES {
        let path = source.join(relative);
        let bytes = fs::read(&path).map_err(|error| format!("không đọc được {}: {error}", path.display()))?;
        hasher.update(relative.as_bytes());
        hasher.update((bytes.len() as u64).to_le_bytes());
        hasher.update(&bytes);
    }
    Ok(hasher.finalize().iter().map(|byte| format!("{byte:02x}")).collect())
}

/// Chèn `line` ngay sau dòng chứa mốc đầu tiên xuất hiện đúng một lần. WHY đúng một lần: mốc trùng nghĩa là
/// không biết chèn chỗ nào; thiếu nghĩa là DING bản này khác bản đã kiểm.
pub fn inject(file: &str, anchors: &[&str], line: &str) -> Result<String, String> {
    for anchor in anchors {
        if file.matches(anchor).count() != 1 {
            continue;
        }
        let at = file.find(anchor).unwrap_or(0);
        let line_end = file[at..].find('\n').map_or(file.len(), |offset| at + offset);
        let mut out = String::with_capacity(file.len() + line.len() + 1);
        out.push_str(&file[..line_end]);
        out.push('\n');
        out.push_str(line);
        out.push_str(&file[line_end..]);
        return Ok(out);
    }
    Err(format!("DING bản này chưa hỗ trợ: không thấy mốc {anchors:?} đúng một lần"))
}

fn copy_tree(from: &Path, to: &Path) -> Result<(), String> {
    fs::create_dir_all(to).map_err(|error| format!("không tạo được {}: {error}", to.display()))?;
    let entries = fs::read_dir(from).map_err(|error| format!("không đọc được {}: {error}", from.display()))?;
    for entry in entries {
        let entry = entry.map_err(|error| error.to_string())?;
        let (source, target) = (entry.path(), to.join(entry.file_name()));
        if source.is_dir() {
            copy_tree(&source, &target)?;
        } else {
            fs::copy(&source, &target).map_err(|error| format!("không chép được {}: {error}", source.display()))?;
        }
    }
    Ok(())
}

fn patch_file(root: &Path, relative: &str, edit: impl FnOnce(&str) -> Result<String, String>) -> Result<(), String> {
    let path = root.join(relative);
    let text = fs::read_to_string(&path).map_err(|error| format!("không đọc được {}: {error}", path.display()))?;
    fs::write(&path, edit(&text)?).map_err(|error| format!("không ghi được {}: {error}", path.display()))
}

fn assemble(source: &Path, tmp: &Path, morphix: &Path, version: &str) -> Result<(), String> {
    let binary = morphix.to_str().ok_or("đường dẫn morphix không phải UTF-8")?;
    copy_tree(source, tmp)?;
    patch_file(tmp, "metadata.json", |text| {
        let mut meta: serde_json::Value = serde_json::from_str(text).map_err(|error| format!("metadata.json hỏng: {error}"))?;
        meta["uuid"] = FORK_UUID.into();
        meta["name"] = FORK_NAME.into();
        serde_json::to_string_pretty(&meta).map_err(|error| error.to_string())
    })?;
    patch_file(tmp, "app/fileItemMenu.js", |text| {
        inject(
            text,
            &[
                "this._menu = DesktopIconsUtil.createDesktopMenu(['fileitemmenu']);",
                "menuStyleContext.add_class('fileitemmenu');",
            ],
            "        imports.morphixMenu.addFileItems(this, fileItem);",
        )
    })?;
    patch_file(tmp, "app/desktopManager.js", |text| {
        inject(text, &["this._menu.add(selectAll);"], "        imports.morphixMenu.addBackgroundItem(this._menu);")
    })?;
    patch_file(tmp, "extension.js", |text| {
        let text = inject(
            text,
            &["import {Extension} from 'resource:///org/gnome/shell/extensions/extension.js';"],
            "import * as MorphixWatch from './morphixWatch.js';",
        )?;
        let text = inject(&text, &["    enable() {"], "        MorphixWatch.start();")?;
        inject(&text, &["    disable() {"], "        MorphixWatch.stop();")
    })?;
    let literal = serde_json::to_string(binary).map_err(|error| error.to_string())?;
    let menu = MENU_JS.replace("'@MORPHIX@'", &literal);
    fs::write(tmp.join("app/morphixMenu.js"), menu).map_err(|error| error.to_string())?;
    fs::write(tmp.join("morphixWatch.js"), WATCH_JS).map_err(|error| error.to_string())?;
    let marker = ForkMarker {
        source: source.to_path_buf(),
        fingerprint: fingerprint(source)?,
        morphix_version: version.to_string(),
    };
    let json = serde_json::to_string_pretty(&marker).map_err(|error| error.to_string())?;
    fs::write(tmp.join(MARKER_FILE), json).map_err(|error| error.to_string())
}

/// Dựng bản chép vào `extensions_dir/morphix-desktop@morphix`. Lỗi thì thư mục đích cũ còn nguyên và không còn thư mục tạm.
pub fn build_fork(source: &Path, extensions_dir: &Path, morphix: &Path, morphix_version: &str) -> Result<PathBuf, String> {
    let dest = extensions_dir.join(FORK_UUID);
    if dest.exists() && !dest.join(MARKER_FILE).is_file() {
        return Err(format!("{} có sẵn mà không phải của Morphix, không ghi đè", dest.display()));
    }
    fs::create_dir_all(extensions_dir).map_err(|error| format!("không tạo được {}: {error}", extensions_dir.display()))?;
    let tmp = extensions_dir.join(format!(".morphix-desktop.tmp-{}", std::process::id()));
    let _ = fs::remove_dir_all(&tmp);
    if let Err(message) = assemble(source, &tmp, morphix, morphix_version) {
        let _ = fs::remove_dir_all(&tmp);
        return Err(message);
    }
    if dest.exists() {
        fs::remove_dir_all(&dest).map_err(|error| {
            let _ = fs::remove_dir_all(&tmp);
            format!("không thay được bản chép cũ: {error}")
        })?;
    }
    fs::rename(&tmp, &dest).map_err(|error| {
        let _ = fs::remove_dir_all(&tmp);
        format!("không đặt được {}: {error}", dest.display())
    })?;
    Ok(dest)
}

pub fn read_marker(fork: &Path) -> Option<ForkMarker> {
    serde_json::from_str(&fs::read_to_string(fork.join(MARKER_FILE)).ok()?).ok()
}

#[cfg(test)]
pub(super) mod fixtures {
    use super::*;
    use std::fs;

    pub const MENU_26: &str = "class M {\n    showMenu() {\n        this._menu = DesktopIconsUtil.createDesktopMenu(['fileitemmenu']);\n        x();\n    }\n}\n";
    pub const MENU_24: &str = "class M {\n    showMenu() {\n        this._menu = new Gtk.Menu();\n        const menuStyleContext = this._menu.get_style_context();\n        menuStyleContext.add_class('fileitemmenu');\n    }\n}\n";
    pub const MANAGER: &str = "a();\n        this._menu.add(selectAll);\n        b();\n";
    pub const EXTENSION: &str = "import {Extension} from 'resource:///org/gnome/shell/extensions/extension.js';\nexport default class DING extends Extension {\n    enable() {\n        go();\n    }\n\n    disable() {\n        stop();\n    }\n}\n";
    pub const METADATA: &str = "{\"name\": \"Desktop Icons NG (DING)\", \"shell-version\": [\"46\", \"47\"], \"uuid\": \"ding@rastersoft.com\"}";

    pub fn write(path: &Path, text: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    /// DING giả có đủ mốc; `dir` là gốc kiểu `/usr/share` (chứa `gnome-shell/extensions`).
    pub fn fake_ding(dir: &Path, menu: &str) -> PathBuf {
        let root = dir.join("gnome-shell/extensions").join(DING_UUID);
        write(&root.join("metadata.json"), METADATA);
        write(&root.join("extension.js"), EXTENSION);
        write(&root.join("app/fileItemMenu.js"), menu);
        write(&root.join("app/desktopManager.js"), MANAGER);
        write(&root.join("app/other.js"), "keep();\n");
        root
    }
}

#[cfg(test)]
mod tests {
    use super::fixtures::*;
    use super::*;
    use std::fs;

    fn count(text: &str, needle: &str) -> usize {
        text.matches(needle).count()
    }

    #[test]
    fn inject_uses_the_first_anchor_present_once() {
        let out = inject("a\nb\nc\n", &["zzz", "b"], "  X").unwrap();
        assert_eq!(out, "a\nb\n  X\nc\n");
    }

    #[test]
    fn inject_fails_when_anchor_missing_or_repeated() {
        assert!(inject("a\nb\n", &["zzz"], "X").unwrap_err().contains("chưa hỗ trợ"));
        assert!(inject("b\nb\n", &["b"], "X").is_err());
    }

    #[test]
    fn build_fork_rewrites_uuid_and_name_and_keeps_shell_version() {
        let dir = tempfile::tempdir().unwrap();
        let source = fake_ding(dir.path(), MENU_26);
        let ext = dir.path().join("home/ext");
        let fork = build_fork(&source, &ext, Path::new("/x/morphix"), "0.1.7").unwrap();
        assert_eq!(fork, ext.join(FORK_UUID));
        let meta: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(fork.join("metadata.json")).unwrap()).unwrap();
        assert_eq!(meta["uuid"], FORK_UUID);
        assert_eq!(meta["name"], "Desktop Icons (Morphix)");
        assert_eq!(meta["shell-version"], serde_json::json!(["46", "47"]));
        assert_eq!(fs::read_to_string(fork.join("app/other.js")).unwrap(), "keep();\n");
    }

    #[test]
    fn build_fork_inserts_calls_for_2404_and_2604_layouts() {
        for menu in [MENU_26, MENU_24] {
            let dir = tempfile::tempdir().unwrap();
            let source = fake_ding(dir.path(), menu);
            let fork = build_fork(&source, &dir.path().join("e"), Path::new("/x/morphix"), "0.1.7").unwrap();
            let file_menu = fs::read_to_string(fork.join("app/fileItemMenu.js")).unwrap();
            assert_eq!(count(&file_menu, "imports.morphixMenu.addFileItems(this, fileItem);"), 1);
            let manager = fs::read_to_string(fork.join("app/desktopManager.js")).unwrap();
            assert_eq!(count(&manager, "imports.morphixMenu.addBackgroundItem(this._menu);"), 1);
            let ext = fs::read_to_string(fork.join("extension.js")).unwrap();
            assert_eq!(count(&ext, "import * as MorphixWatch from './morphixWatch.js';"), 1);
            assert!(ext.contains("    enable() {\n        MorphixWatch.start();\n"));
            assert!(ext.contains("    disable() {\n        MorphixWatch.stop();\n"));
            assert!(fork.join("morphixWatch.js").is_file());
        }
    }

    #[test]
    fn build_fork_embeds_binary_path_with_spaces() {
        let dir = tempfile::tempdir().unwrap();
        let source = fake_ding(dir.path(), MENU_26);
        let fork = build_fork(&source, &dir.path().join("e"), Path::new("/home/a b/.local/bin/morphix"), "0.1.7").unwrap();
        let js = fs::read_to_string(fork.join("app/morphixMenu.js")).unwrap();
        assert!(js.contains("var MORPHIX = \"/home/a b/.local/bin/morphix\";"));
        assert!(!js.contains("@MORPHIX@"));
    }

    #[test]
    fn build_fork_records_marker() {
        let dir = tempfile::tempdir().unwrap();
        let source = fake_ding(dir.path(), MENU_26);
        let fork = build_fork(&source, &dir.path().join("e"), Path::new("/x/morphix"), "0.1.7").unwrap();
        let marker = read_marker(&fork).unwrap();
        assert_eq!(marker.source, source);
        assert_eq!(marker.fingerprint, fingerprint(&source).unwrap());
        assert_eq!(marker.morphix_version, "0.1.7");
    }

    #[test]
    fn build_fork_failure_keeps_previous_fork() {
        let dir = tempfile::tempdir().unwrap();
        let source = fake_ding(dir.path(), MENU_26);
        let ext = dir.path().join("e");
        let fork = build_fork(&source, &ext, Path::new("/x/morphix"), "0.1.6").unwrap();
        write(&source.join("app/desktopManager.js"), "nothing to anchor\n");
        assert!(build_fork(&source, &ext, Path::new("/x/morphix"), "0.1.7").is_err());
        assert_eq!(read_marker(&fork).unwrap().morphix_version, "0.1.6");
        let names: Vec<_> = fs::read_dir(&ext).unwrap().map(|e| e.unwrap().file_name()).collect();
        assert_eq!(names, vec![std::ffi::OsString::from(FORK_UUID)]);
    }

    #[test]
    fn build_fork_never_replaces_a_foreign_directory() {
        let dir = tempfile::tempdir().unwrap();
        let source = fake_ding(dir.path(), MENU_26);
        let ext = dir.path().join("e");
        write(&ext.join(FORK_UUID).join("mine.txt"), "x");
        assert!(build_fork(&source, &ext, Path::new("/x/morphix"), "0.1.7").is_err());
        assert!(ext.join(FORK_UUID).join("mine.txt").is_file());
    }

    #[test]
    fn find_source_prefers_data_home_and_never_returns_the_fork() {
        let home = tempfile::tempdir().unwrap();
        let system = tempfile::tempdir().unwrap();
        let sys_src = fake_ding(system.path(), MENU_26);
        let dirs = vec![system.path().to_path_buf()];
        assert_eq!(find_source(home.path(), &dirs), Some(sys_src.clone()));
        fs::create_dir_all(home.path().join("gnome-shell/extensions").join(FORK_UUID)).unwrap();
        write(&home.path().join("gnome-shell/extensions").join(FORK_UUID).join("metadata.json"), "{}");
        assert_eq!(find_source(home.path(), &dirs), Some(sys_src));
        let user_src = fake_ding(home.path(), MENU_24);
        assert_eq!(find_source(home.path(), &dirs), Some(user_src));
        assert_eq!(find_source(home.path(), &[]), find_source(home.path(), &[]));
    }

    #[test]
    fn fingerprint_changes_with_the_source() {
        let dir = tempfile::tempdir().unwrap();
        let source = fake_ding(dir.path(), MENU_26);
        let before = fingerprint(&source).unwrap();
        write(&source.join("extension.js"), "changed");
        assert_ne!(before, fingerprint(&source).unwrap());
    }

    #[test]
    fn real_system_ding_is_supported() {
        let system = Path::new("/usr/share/gnome-shell/extensions").join(DING_UUID);
        if !system.join("metadata.json").is_file() {
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        build_fork(&system, &dir.path().join("e"), Path::new("/x/morphix"), "0.1.7").unwrap();
    }
}
