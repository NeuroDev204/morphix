use std::path::PathBuf;

use morphix_core::{detect, menu_actions, FileKind};

/// Quá số file này thì không đọc nội dung để nhận dạng. WHY: `detect` đọc tới 8 KB mỗi file ngay trong luồng
/// giao diện của Explorer, chọn nhiều file trên ổ chậm sẽ làm cả menu chuột phải treo.
pub const MAX_FILES: usize = 100;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShellItem {
    /// Id việc, cùng dạng với id vòng tròn và IPC (`convert:webp`, `archive:zip`...).
    pub id: String,
    pub label: String,
    pub is_tool: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShellMenu {
    /// Không chèn mục "Morphix" vào menu chuột phải.
    Hidden,
    /// Chọn quá `MAX_FILES` file: chỉ hiện một mục bị vô hiệu hoá.
    TooMany,
    Items(Vec<ShellItem>),
}

/// Danh sách việc cho các file Explorer đang chọn, lấy từ cùng catalog với vòng tròn và GUI.
pub fn shell_menu(paths: &[PathBuf]) -> ShellMenu {
    if paths.is_empty() {
        return ShellMenu::Hidden;
    }
    if paths.len() > MAX_FILES {
        return ShellMenu::TooMany;
    }
    // WHY: file đọc không được coi như loại không rõ; chỉ còn các việc nén, và việc thật sự chạy sẽ báo lỗi rõ ràng.
    let kinds: Vec<FileKind> = paths
        .iter()
        .map(|path| detect(path).unwrap_or(FileKind::Unknown))
        .collect();
    let items: Vec<ShellItem> = menu_actions(&kinds)
        .into_iter()
        .map(|action| ShellItem {
            id: morphix_core::action_id(action.kind),
            label: morphix_core::menu_label(&action),
            is_tool: action.is_tool,
        })
        .collect();
    if items.is_empty() {
        ShellMenu::Hidden
    } else {
        ShellMenu::Items(items)
    }
}

/// Số mục con vừa trong khoảng id Explorer cấp: mục cha dùng `first_id`, mục con dùng `first_id + 1 ..= last_id`.
pub fn child_slots(first_id: u32, last_id: u32, wanted: usize) -> usize {
    if first_id >= last_id {
        return 0;
    }
    wanted.min((last_id - first_id) as usize)
}

/// Đổi `lpVerb` của `InvokeCommand` thành chỉ số mục con. Từ chối verb dạng chuỗi (HIWORD khác 0), mục cha
/// (offset 0) và offset ngoài số mục con.
pub fn child_for_verb(verb: usize, children: usize) -> Option<usize> {
    if verb >> 16 != 0 {
        return None;
    }
    let offset = verb.checked_sub(1)?;
    (offset < children).then_some(offset)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn png(dir: &Path, name: &str) -> PathBuf {
        let path = dir.join(name);
        image::RgbImage::from_pixel(8, 8, image::Rgb([10, 120, 200]))
            .save(&path)
            .unwrap();
        path
    }

    fn ids(menu: ShellMenu) -> Vec<String> {
        match menu {
            ShellMenu::Items(items) => items.into_iter().map(|item| item.id).collect(),
            other => panic!("cần Items, được {other:?}"),
        }
    }

    #[test]
    fn one_png_lists_image_targets() {
        let dir = tempfile::tempdir().unwrap();
        let ids = ids(shell_menu(&[png(dir.path(), "a.png")]));
        assert!(ids.contains(&"convert:webp".to_string()), "{ids:?}");
    }

    #[test]
    fn mixed_kinds_offer_only_archives() {
        let dir = tempfile::tempdir().unwrap();
        let text = dir.path().join("ghi-chu.txt");
        std::fs::write(&text, "xin chào").unwrap();
        let ids = ids(shell_menu(&[png(dir.path(), "a.png"), text]));
        assert!(!ids.is_empty());
        assert!(ids.iter().all(|id| id.starts_with("archive:")), "{ids:?}");
    }

    #[test]
    fn unreadable_file_counts_as_unknown_and_offers_only_archives() {
        let ids = ids(shell_menu(&[PathBuf::from(r"Z:\khong-ton-tai\a.png")]));
        assert!(ids.iter().all(|id| id.starts_with("archive:")), "{ids:?}");
    }

    #[test]
    fn rar_only_selection_is_hidden() {
        let dir = tempfile::tempdir().unwrap();
        let rar = dir.path().join("a.rar");
        std::fs::write(&rar, b"Rar!\x1A\x07\x00\xCF\x90").unwrap();
        assert_eq!(shell_menu(&[rar]), ShellMenu::Hidden);
    }

    #[test]
    fn empty_selection_is_hidden() {
        assert_eq!(shell_menu(&[]), ShellMenu::Hidden);
    }

    #[test]
    fn more_than_max_files_is_too_many() {
        let paths: Vec<PathBuf> = (0..=MAX_FILES)
            .map(|index| PathBuf::from(format!(r"Z:\khong-ton-tai\{index}.png")))
            .collect();
        assert_eq!(shell_menu(&paths), ShellMenu::TooMany);
    }

    #[test]
    fn child_slots_fit_the_id_range() {
        assert_eq!(child_slots(100, 105, 10), 5);
        assert_eq!(child_slots(100, 105, 2), 2);
        assert_eq!(child_slots(100, 100, 3), 0);
        assert_eq!(child_slots(5, 4, 3), 0);
    }

    #[test]
    fn string_verbs_and_parent_offset_are_rejected() {
        assert_eq!(child_for_verb(0x1_0000, 3), None);
        assert_eq!(child_for_verb(0, 3), None);
        assert_eq!(child_for_verb(1, 3), Some(0));
        assert_eq!(child_for_verb(3, 3), Some(2));
        assert_eq!(child_for_verb(4, 3), None);
    }
}
