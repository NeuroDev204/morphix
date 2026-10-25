//! Custom action Thunar trong `uca.xml` dùng chung với action của người dùng.
//! WHY nhận diện bằng `unique-id` chứ không bằng comment marker: Thunar ghi lại `uca.xml` khi người dùng
//! sửa action và làm mất comment, còn `unique-id` thì được giữ.

use std::{fs, io, path::Path};

use super::{
    files::{publish_file, read_existing, Transaction},
    linux::{io_failure, posix_single_quote, report},
    ComponentId, ComponentStatus, IntegrationComponentReport,
};

pub const THUNAR_UNIQUE_ID: &str = "morphix-menu";
const ID: ComponentId = ComponentId::ThunarAction;
const UCA_MODE: u32 = 0o644;
const ACTIONS_END: &str = "</actions>";

fn xml_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Khối `<action>` của Morphix, kết thúc bằng xuống dòng.
pub fn thunar_action_block(morphix: &Path) -> Result<String, String> {
    let morphix = morphix
        .to_str()
        .ok_or_else(|| format!("đường dẫn không phải UTF-8: {}", morphix.display()))?;
    let command = xml_escape(&format!("{} menu -- %F", posix_single_quote(morphix)));
    Ok(format!(
        "<action>\n\t<icon>morphix</icon>\n\t<name>Morphix</name>\n\t<submenu></submenu>\n\t<unique-id>{THUNAR_UNIQUE_ID}</unique-id>\n\t<command>{command}</command>\n\t<description>Mở Morphix cho các file đã chọn</description>\n\t<range>*</range>\n\t<patterns>*</patterns>\n\t<audio-files/>\n\t<image-files/>\n\t<other-files/>\n\t<text-files/>\n\t<video-files/>\n</action>\n"
    ))
}

/// Vị trí `[đầu, cuối)` của khối Morphix, gồm cả xuống dòng ngay sau `</action>`.
fn morphix_block_range(text: &str) -> Option<(usize, usize)> {
    let id = text.find(&format!("<unique-id>{THUNAR_UNIQUE_ID}</unique-id>"))?;
    let start = text[..id].rfind("<action>")?;
    let close = id + text[id..].find("</action>")? + "</action>".len();
    let end = if text[close..].starts_with('\n') {
        close + 1
    } else {
        close
    };
    Some((start, end))
}

/// Thay khối cũ hoặc chèn trước `</actions>` cuối; phần còn lại giữ nguyên từng byte.
pub fn upsert_action(existing: Option<&str>, block: &str) -> Result<String, String> {
    let Some(text) = existing else {
        return Ok(format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<actions>\n{block}{ACTIONS_END}\n"
        ));
    };
    if let Some((start, end)) = morphix_block_range(text) {
        return Ok(format!("{}{block}{}", &text[..start], &text[end..]));
    }
    let at = text
        .rfind(ACTIONS_END)
        .ok_or_else(|| "uca.xml không đúng định dạng; không sửa".to_string())?;
    Ok(format!("{}{block}{}", &text[..at], &text[at..]))
}

/// Bỏ khối Morphix; `None` khi không có khối nào.
pub fn remove_action(existing: &str) -> Option<String> {
    let (start, end) = morphix_block_range(existing)?;
    Some(format!("{}{}", &existing[..start], &existing[end..]))
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
    let block = match thunar_action_block(morphix) {
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

pub(super) fn inspect(path: &Path, morphix: &Path) -> IntegrationComponentReport {
    let current = match read_text(path) {
        Ok(current) => current,
        Err(error) => return io_failure(ID, path, &error),
    };
    let Some(range) = current.as_deref().and_then(morphix_block_range) else {
        return report(ID, ComponentStatus::Missing, false, Some(path), "chưa cài");
    };
    let text = current.unwrap_or_default();
    match thunar_action_block(morphix) {
        Ok(block) if text[range.0..range.1] == block => report(
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
    use std::path::Path;

    const USER_FILE: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<actions>\n<action>\n\t<icon>utilities-terminal</icon>\n\t<name>Mở terminal</name>\n\t<unique-id>1700000000000-1</unique-id>\n\t<command>exo-open --launch TerminalEmulator</command>\n</action>\n</actions>\n";

    #[test]
    fn thunar_install_and_uninstall_preserve_other_actions() {
        let block = thunar_action_block(Path::new("/opt/morphix")).unwrap();
        let installed = upsert_action(Some(USER_FILE), &block).unwrap();
        assert_eq!(installed.matches("<action>").count(), 2);
        assert!(installed.contains("<unique-id>1700000000000-1</unique-id>"));
        assert_eq!(remove_action(&installed).as_deref(), Some(USER_FILE));
        assert_eq!(remove_action(USER_FILE), None);
    }

    #[test]
    fn upsert_replaces_existing_morphix_block() {
        let first = upsert_action(
            Some(USER_FILE),
            &thunar_action_block(Path::new("/old/morphix")).unwrap(),
        )
        .unwrap();
        let second = upsert_action(
            Some(&first),
            &thunar_action_block(Path::new("/new/morphix")).unwrap(),
        )
        .unwrap();
        assert_eq!(second.matches(THUNAR_UNIQUE_ID).count(), 1);
        assert!(second.contains("/new/morphix") && !second.contains("/old/morphix"));
    }

    #[test]
    fn new_file_is_a_complete_actions_document() {
        let block = thunar_action_block(Path::new("/opt/morphix")).unwrap();
        let created = upsert_action(None, &block).unwrap();
        assert!(created.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<actions>\n"));
        assert!(created.ends_with("</actions>\n"));
    }

    #[test]
    fn command_escapes_xml_and_shell() {
        let block = thunar_action_block(Path::new("/tmp/a&b's/morphix")).unwrap();
        assert!(
            block.contains("<command>'/tmp/a&amp;b'\\''s/morphix' menu -- %F</command>"),
            "{block}"
        );
    }

    #[test]
    fn malformed_uca_is_not_touched() {
        let block = thunar_action_block(Path::new("/opt/morphix")).unwrap();
        assert!(upsert_action(Some("<actions>"), &block).is_err());
    }
}
