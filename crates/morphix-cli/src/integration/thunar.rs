//! Custom action Thunar trong `uca.xml` dùng chung với action của người dùng.
//! WHY nhận diện bằng `unique-id` chứ không bằng comment marker: Thunar ghi lại `uca.xml` khi người dùng
//! sửa action và làm mất comment, còn `unique-id` thì được giữ.

use std::{fs, io, path::Path};

use morphix_core::{action_id, static_menu_groups};

use super::{
    files::{publish_file, read_existing, Transaction},
    linux::{io_failure, posix_single_quote, remove_created_dirs, report},
    ComponentId, ComponentStatus, IntegrationComponentReport,
};

/// Tiền tố `unique-id` của mọi khối Morphix. Khối đơn của bản cũ dùng đúng chuỗi này (không có hậu tố).
pub const THUNAR_UNIQUE_ID: &str = "morphix-menu";
const ID: ComponentId = ComponentId::ThunarAction;
const UCA_MODE: u32 = 0o644;
const ACTIONS_END: &str = "</actions>";

fn xml_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn is_morphix_id(id: &str) -> bool {
    id == THUNAR_UNIQUE_ID || id.starts_with("morphix-menu-")
}

/// Thẻ loại file của Thunar cho một nhóm. Nhóm văn bản có cả `other-files` vì DOCX không phải `text/*`.
fn type_tags(key: &str) -> &'static [&'static str] {
    match key {
        "image" => &["image-files"],
        "audio" => &["audio-files"],
        "video" => &["video-files"],
        "text" => &["text-files", "other-files"],
        "pdf" | "archive" => &["other-files"],
        _ => &["audio-files", "image-files", "other-files", "text-files", "video-files"],
    }
}

/// Các khối `<action>` của Morphix, mỗi việc một khối nằm trong submenu "Morphix", kết thúc bằng xuống dòng.
/// WHY thêm cả đuôi viết hoa vào pattern: không chắc Thunar so khớp pattern không phân biệt hoa thường.
pub fn thunar_action_blocks(morphix: &Path) -> Result<String, String> {
    let morphix = morphix
        .to_str()
        .ok_or_else(|| format!("đường dẫn không phải UTF-8: {}", morphix.display()))?;
    let mut blocks = String::new();
    for group in static_menu_groups() {
        let patterns = if group.extensions.is_empty() {
            "*".to_string()
        } else {
            group
                .extensions
                .iter()
                .flat_map(|ext| [format!("*.{ext}"), format!("*.{}", ext.to_uppercase())])
                .collect::<Vec<_>>()
                .join(";")
        };
        let tags: String = type_tags(group.key)
            .iter()
            .map(|tag| format!("\t<{tag}/>\n"))
            .collect();
        for (index, action) in group.actions.iter().enumerate() {
            let id = action_id(action.kind);
            let command = xml_escape(&format!(
                "{} menu --action {} -- %F",
                posix_single_quote(morphix),
                posix_single_quote(&id)
            ));
            let label = xml_escape(&action.label);
            blocks.push_str(&format!(
                "<action>\n\t<icon>morphix</icon>\n\t<name>{label}</name>\n\t<submenu>Morphix</submenu>\n\t<unique-id>{THUNAR_UNIQUE_ID}-{}-{}</unique-id>\n\t<command>{command}</command>\n\t<description>Morphix: {label}</description>\n\t<range>*</range>\n\t<patterns>{}</patterns>\n{tags}</action>\n",
                group.key,
                index + 1,
                xml_escape(&patterns)
            ));
        }
    }
    Ok(blocks)
}

/// Vị trí `[đầu, cuối)` của từng khối Morphix, gồm cả xuống dòng ngay sau `</action>`.
fn morphix_block_ranges(text: &str) -> Vec<(usize, usize)> {
    let mut ranges = Vec::new();
    let mut at = 0;
    while let Some(found) = text[at..].find("<action>") {
        let start = at + found;
        let Some(len) = text[start..].find("</action>") else {
            break;
        };
        let close = start + len + "</action>".len();
        let block = &text[start..close];
        let is_ours = block
            .split("<unique-id>")
            .nth(1)
            .and_then(|rest| rest.split("</unique-id>").next())
            .is_some_and(is_morphix_id);
        if is_ours {
            let end = if text[close..].starts_with('\n') {
                close + 1
            } else {
                close
            };
            ranges.push((start, end));
        }
        at = close;
    }
    ranges
}

/// Thay mọi khối Morphix cũ (kể cả khối đơn của bản trước) bằng `blocks` tại chỗ khối đầu tiên, hoặc chèn
/// trước `</actions>` cuối; phần còn lại giữ nguyên từng byte.
pub fn upsert_action(existing: Option<&str>, blocks: &str) -> Result<String, String> {
    let Some(text) = existing else {
        return Ok(format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<actions>\n{blocks}{ACTIONS_END}\n"
        ));
    };
    let ranges = morphix_block_ranges(text);
    if ranges.is_empty() {
        let at = text
            .rfind(ACTIONS_END)
            .ok_or_else(|| "uca.xml không đúng định dạng; không sửa".to_string())?;
        return Ok(format!("{}{blocks}{}", &text[..at], &text[at..]));
    }
    Ok(replace_ranges(text, &ranges, blocks))
}

/// Bỏ mọi khối Morphix; `None` khi không có khối nào.
pub fn remove_action(existing: &str) -> Option<String> {
    let ranges = morphix_block_ranges(existing);
    if ranges.is_empty() {
        return None;
    }
    Some(replace_ranges(existing, &ranges, ""))
}

/// Bỏ các đoạn `ranges` (đã sắp tăng dần) khỏi `text` và đặt `insert` vào chỗ đoạn đầu tiên.
fn replace_ranges(text: &str, ranges: &[(usize, usize)], insert: &str) -> String {
    let mut out = String::with_capacity(text.len() + insert.len());
    let mut last = 0;
    for (index, (start, end)) in ranges.iter().enumerate() {
        out.push_str(&text[last..*start]);
        if index == 0 {
            out.push_str(insert);
        }
        last = *end;
    }
    out.push_str(&text[last..]);
    out
}

fn read_text(path: &Path) -> io::Result<Option<String>> {
    read_existing(path)?
        .map(|file| String::from_utf8(file.bytes).map_err(io::Error::other))
        .transpose()
}

fn write_text(path: &Path, text: &str) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let mut tx = Transaction::new();
    publish_file(path, text.as_bytes(), UCA_MODE, &mut tx)?;
    tx.commit();
    Ok(())
}

pub(super) fn install(path: &Path, morphix: &Path) -> IntegrationComponentReport {
    let block = match thunar_action_blocks(morphix) {
        Ok(block) => block,
        Err(message) => return report(ID, ComponentStatus::Failed, false, Some(path), &message),
    };
    let current = match read_text(path) {
        Ok(current) => current,
        Err(error) => return io_failure(ID, path, &error),
    };
    let updated = match upsert_action(current.as_deref(), &block) {
        Ok(updated) => updated,
        Err(message) => return report(ID, ComponentStatus::Failed, false, Some(path), &message),
    };
    if current.as_deref() == Some(updated.as_str()) {
        return report(
            ID,
            ComponentStatus::AlreadyInstalled,
            false,
            Some(path),
            "đã cài sẵn",
        );
    }
    match write_text(path, &updated) {
        Ok(()) => report(ID, ComponentStatus::Installed, true, Some(path), "đã cài"),
        Err(error) => io_failure(ID, path, &error),
    }
}

pub(super) fn uninstall(path: &Path) -> IntegrationComponentReport {
    let current = match read_text(path) {
        Ok(Some(current)) => current,
        Ok(None) => {
            return report(
                ID,
                ComponentStatus::AlreadyAbsent,
                false,
                None,
                "không có gì để gỡ",
            )
        }
        Err(error) => return io_failure(ID, path, &error),
    };
    let Some(updated) = remove_action(&current) else {
        return report(
            ID,
            ComponentStatus::AlreadyAbsent,
            false,
            Some(path),
            "không có action Morphix",
        );
    };
    // WHY xoá khi không còn action nào: một `uca.xml` chỉ còn `<actions>` rỗng không có nội dung của người dùng, và để
    // lại nó cùng thư mục Thunar thì thư mục cha do Morphix tạo (có marker) không bao giờ dọn được.
    if !updated.contains("<action>") {
        return match remove_empty_uca(path) {
            Ok(()) => report(ID, ComponentStatus::Removed, true, Some(path), "đã gỡ action Morphix"),
            Err(error) => io_failure(ID, path, &error),
        };
    }
    match write_text(path, &updated) {
        Ok(()) => report(
            ID,
            ComponentStatus::Removed,
            true,
            Some(path),
            "đã gỡ action Morphix",
        ),
        Err(error) => io_failure(ID, path, &error),
    }
}

/// Xoá `uca.xml`, rồi thư mục Thunar nếu rỗng, rồi các thư mục cha do Morphix tạo.
fn remove_empty_uca(path: &Path) -> io::Result<()> {
    fs::remove_file(path)?;
    if let Some(dir) = path.parent() {
        // WHY bỏ qua lỗi: thư mục còn file khác của người dùng thì giữ nguyên, không phải lỗi.
        let _ = fs::remove_dir(dir);
        if let Some(config) = dir.parent() {
            remove_created_dirs(config)?;
        }
    }
    Ok(())
}

pub(super) fn inspect(path: &Path, morphix: &Path) -> IntegrationComponentReport {
    let current = match read_text(path) {
        Ok(current) => current,
        Err(error) => return io_failure(ID, path, &error),
    };
    let text = current.unwrap_or_default();
    let ranges = morphix_block_ranges(&text);
    if ranges.is_empty() {
        return report(ID, ComponentStatus::Missing, false, Some(path), "chưa cài");
    }
    let installed: String = ranges.iter().map(|(start, end)| &text[*start..*end]).collect();
    match thunar_action_blocks(morphix) {
        Ok(block) if installed == block => report(
            ID,
            ComponentStatus::Installed,
            false,
            Some(path),
            "đang hoạt động",
        ),
        Ok(_) => report(
            ID,
            ComponentStatus::Stale,
            false,
            Some(path),
            "morphix đã được di chuyển hoặc cập nhật; chạy lại install-integration",
        ),
        Err(message) => report(ID, ComponentStatus::Failed, false, Some(path), &message),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use morphix_core::{action_id, static_menu_groups};
    use std::collections::BTreeSet;
    use std::path::Path;

    const USER_FILE: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<actions>\n<action>\n\t<icon>utilities-terminal</icon>\n\t<name>Mở terminal</name>\n\t<unique-id>1700000000000-1</unique-id>\n\t<command>exo-open --launch TerminalEmulator</command>\n</action>\n</actions>\n";
    /// Khối đơn do bản Morphix trước ghi ra (một mục "Morphix" mở vòng tròn).
    const LEGACY_BLOCK: &str = "<action>\n\t<icon>morphix</icon>\n\t<name>Morphix</name>\n\t<submenu></submenu>\n\t<unique-id>morphix-menu</unique-id>\n\t<command>'/old/morphix' menu -- %F</command>\n\t<description>Mở Morphix cho các file đã chọn</description>\n\t<range>*</range>\n\t<patterns>*</patterns>\n\t<audio-files/>\n\t<image-files/>\n\t<other-files/>\n\t<text-files/>\n\t<video-files/>\n</action>\n";

    fn blocks() -> String {
        thunar_action_blocks(Path::new("/opt/morphix")).unwrap()
    }

    fn action_ids(text: &str) -> BTreeSet<String> {
        text.split("--action '")
            .skip(1)
            .map(|rest| rest.split('\'').next().unwrap().to_string())
            .collect()
    }

    #[test]
    fn thunar_install_and_uninstall_preserve_other_actions() {
        let installed = upsert_action(Some(USER_FILE), &blocks()).unwrap();
        assert!(installed.contains("<unique-id>1700000000000-1</unique-id>"));
        assert!(installed.contains("<unique-id>morphix-menu-image-1</unique-id>"));
        assert_eq!(remove_action(&installed).as_deref(), Some(USER_FILE));
        assert_eq!(remove_action(USER_FILE), None);
    }

    #[test]
    fn thunar_blocks_match_the_catalog_and_replace_the_legacy_block() {
        let legacy = USER_FILE.replace("</actions>", &format!("{LEGACY_BLOCK}</actions>"));
        let updated = upsert_action(Some(&legacy), &blocks()).unwrap();
        assert!(!updated.contains("<unique-id>morphix-menu</unique-id>"));
        assert!(!updated.contains("/old/morphix"));
        assert!(updated.contains("<unique-id>1700000000000-1</unique-id>"));
        let wanted: BTreeSet<String> = static_menu_groups()
            .iter()
            .flat_map(|group| group.actions.iter().map(|a| action_id(a.kind)))
            .collect();
        assert_eq!(action_ids(&updated), wanted);
        assert_eq!(remove_action(&updated).as_deref(), Some(USER_FILE));
        // Chạy lại không đổi gì: cài lại phải idempotent.
        assert_eq!(upsert_action(Some(&updated), &blocks()).unwrap(), updated);
    }

    #[test]
    fn thunar_blocks_filter_by_file_type_and_extension() {
        let text = blocks();
        let image = text
            .split("</action>")
            .find(|block| block.contains("<unique-id>morphix-menu-image-1</unique-id>"))
            .unwrap();
        assert!(image.contains("<image-files/>") && !image.contains("<video-files/>"));
        assert!(image.contains("<patterns>*.jpg;*.JPG;"), "{image}");
        assert!(image.contains("<submenu>Morphix</submenu>"));
        let any = text
            .split("</action>")
            .find(|block| block.contains("<unique-id>morphix-menu-any-1</unique-id>"))
            .unwrap();
        assert!(any.contains("<patterns>*</patterns>"));
        for tag in ["audio-files", "image-files", "other-files", "text-files", "video-files"] {
            assert!(any.contains(&format!("<{tag}/>")), "{tag}");
        }
        assert!(!any.contains("<directories/>"));
    }

    #[test]
    fn upsert_replaces_existing_morphix_blocks() {
        let first = upsert_action(
            Some(USER_FILE),
            &thunar_action_blocks(Path::new("/old/morphix")).unwrap(),
        )
        .unwrap();
        let second = upsert_action(
            Some(&first),
            &thunar_action_blocks(Path::new("/new/morphix")).unwrap(),
        )
        .unwrap();
        assert_eq!(first.matches("<action>").count(), second.matches("<action>").count());
        assert!(second.contains("/new/morphix") && !second.contains("/old/morphix"));
    }

    #[test]
    fn new_file_is_a_complete_actions_document() {
        let created = upsert_action(None, &blocks()).unwrap();
        assert!(created.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<actions>\n"));
        assert!(created.ends_with("</actions>\n"));
    }

    #[test]
    fn command_escapes_xml_and_shell() {
        let text = thunar_action_blocks(Path::new("/tmp/a&b's/morphix")).unwrap();
        assert!(
            text.contains("<command>'/tmp/a&amp;b'\\''s/morphix' menu --action 'merge' -- %F</command>"),
            "{text}"
        );
    }

    #[test]
    fn malformed_uca_is_not_touched() {
        assert!(upsert_action(Some("<actions>"), &blocks()).is_err());
    }
}
