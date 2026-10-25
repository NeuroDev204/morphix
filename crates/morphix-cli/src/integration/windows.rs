//! Tích hợp Windows theo từng người dùng: verb menu chuột phải và giá trị autostart trong HKCU.
//! Phần thuần (dựng command, quote, phân quyền sở hữu) biên dịch trên mọi nền tảng để test chạy trên Linux;
//! phần chạm registry chỉ có trên Windows.

use std::path::Path;

use super::files::FileOwnership;

pub const MENU_DISPLAY_NAME: &str = "Mở bằng Morphix";
pub const MULTI_SELECT_MODEL: &str = "Player";

/// WHY `%1`: placeholder multiselect CHƯA KIỂM. Task 16 (probe argv trên Windows thật) chưa chạy, nên đây là
/// mặc định của kế hoạch, không phải hành vi đã đo. Xem ledger "Chưa kiểm (Task 18)".
const EXPLORER_PLACEHOLDER: &str = "%1";

pub const VERB_KEY_PATH: &str = r"Software\Classes\SystemFileAssociations\image\shell\Morphix";
pub const COMMAND_SUBKEY: &str = "command";
pub const VERB_OWNER_VALUE: &str = "MorphixOwner";
pub const RUN_KEY_PATH: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
pub const RUN_VALUE_NAME: &str = "Morphix";
/// WHY key riêng: value trong Run được Windows chạy khi đăng nhập, nên marker không được đặt ở đó.
pub const OWNER_KEY_PATH: &str = r"Software\Morphix";
pub const AUTOSTART_OWNER_VALUE: &str = "AutostartOwner";

/// Mục menu đã dựng từ đường dẫn binary; registry chỉ ghi lại đúng các giá trị này.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MenuEntry {
    pub display_name: String,
    pub multi_select_model: String,
    pub command: String,
}

/// Quote một argument theo quy tắc CommandLineToArgvW: `"` thành `\"`, backslash đứng trước `"` hoặc
/// cuối chuỗi được nhân đôi. WHY tự quote: không qua shell, nên chỉ cần đúng quy tắc parse argv của Windows.
pub fn quote_windows_arg(text: &str) -> String {
    let mut quoted = String::with_capacity(text.len() + 2);
    quoted.push('"');
    let mut pending_backslashes: usize = 0;
    for ch in text.chars() {
        match ch {
            '\\' => pending_backslashes += 1,
            '"' => {
                quoted.extend(std::iter::repeat_n('\\', pending_backslashes * 2 + 1));
                quoted.push('"');
                pending_backslashes = 0;
            }
            _ => {
                quoted.extend(std::iter::repeat_n('\\', pending_backslashes));
                quoted.push(ch);
                pending_backslashes = 0;
            }
        }
    }
    // WHY nhân đôi cuối: backslash cuối nằm ngay trước dấu nháy đóng, nếu không nó sẽ escape dấu nháy đó.
    quoted.extend(std::iter::repeat_n('\\', pending_backslashes * 2));
    quoted.push('"');
    quoted
}

/// Command của verb: `"<morphix>" menu "%1"`. Không nối tên file nào khác ngoài placeholder Explorer.
pub fn menu_command(morphix: &Path) -> Result<String, String> {
    let executable = absolute_utf8(morphix, "morphix")?;
    Ok(format!(
        "{} menu \"{EXPLORER_PLACEHOLDER}\"",
        quote_windows_arg(executable)
    ))
}

pub fn menu_entry(morphix: &Path) -> Result<MenuEntry, String> {
    Ok(MenuEntry {
        display_name: MENU_DISPLAY_NAME.to_string(),
        multi_select_model: MULTI_SELECT_MODEL.to_string(),
        command: menu_command(morphix)?,
    })
}

/// Giá trị Run chỉ là đường dẫn đã quote; không có argument vì GUI không nhận đầu vào lúc khởi động.
pub fn autostart_command(gui: &Path) -> Result<String, String> {
    Ok(quote_windows_arg(absolute_utf8(gui, "morphix-gui")?))
}

/// Sở hữu chỉ thuộc Morphix khi giá trị hiện tại đúng bằng giá trị Morphix đã ghi (record).
/// WHY so khớp từng chữ: record nằm ở key khác với value, nên chương trình khác ghi đè value sẽ không
/// làm record đổi; chỉ so sánh nội dung mới phát hiện được và tránh xoá nhầm.
pub fn bound_ownership(recorded: Option<&str>, live: Option<&str>) -> FileOwnership {
    match (recorded, live) {
        (_, None) => FileOwnership::Absent,
        (Some(recorded), Some(live)) if recorded == live => FileOwnership::Owned,
        _ => FileOwnership::Foreign,
    }
}

fn absolute_utf8<'a>(path: &'a Path, label: &str) -> Result<&'a str, String> {
    if !path.is_absolute() {
        return Err(format!(
            "đường dẫn {label} phải tuyệt đối: {}",
            path.display()
        ));
    }
    let text = path
        .to_str()
        .ok_or_else(|| format!("đường dẫn {label} không phải UTF-8: {}", path.display()))?;
    // WHY từ chối `%`: Explorer thay `%1` trong command bằng đường dẫn file đã chọn, nên một `%1` nằm
    // trong đường dẫn cài đặt sẽ bị thay nhầm; từ chối để không cài một command sai.
    if text.contains('%') {
        return Err(format!(
            "đường dẫn {label} không được chứa ký tự '%' vì Windows có thể mở rộng nó thành biến môi trường khi chạy lệnh; hãy chuyển Morphix sang thư mục không có '%': {}",
            path.display()
        ));
    }
    Ok(text)
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{absolute_utf8, autostart_command, bound_ownership, menu_command};
    use crate::integration::files::FileOwnership;

    // WHY test thuần: quyết định sở hữu là logic đúng/sai thuần, kiểm tra được trên Linux không cần registry.
    #[test]
    fn bound_ownership_is_owned_only_when_live_value_equals_the_recorded_one() {
        assert_eq!(
            bound_ownership(
                Some("\"C:\\App\\morphix-gui.exe\""),
                Some("\"C:\\App\\morphix-gui.exe\"")
            ),
            FileOwnership::Owned
        );
    }

    #[test]
    fn bound_ownership_reports_a_rewritten_value_as_foreign() {
        assert_eq!(
            bound_ownership(
                Some("\"C:\\App\\morphix-gui.exe\""),
                Some("\"C:\\Other\\other.exe\"")
            ),
            FileOwnership::Foreign
        );
    }

    #[test]
    fn bound_ownership_reports_a_missing_live_value_as_absent() {
        assert_eq!(bound_ownership(Some("cmd"), None), FileOwnership::Absent);
        assert_eq!(bound_ownership(None, None), FileOwnership::Absent);
    }

    #[test]
    fn bound_ownership_reports_a_live_value_without_record_as_foreign() {
        assert_eq!(bound_ownership(None, Some("cmd")), FileOwnership::Foreign);
    }

    // WHY test thuần: lỗi `%` là lớp kiểm tra đầu vào, chạy được trên Linux mà không cần registry.
    #[test]
    fn absolute_utf8_rejects_percent_in_either_binary_path() {
        let error = absolute_utf8(Path::new("/opt/100%/morphix"), "morphix")
            .expect_err("đường dẫn chứa % phải bị từ chối");
        assert!(error.contains("'%'"), "{error}");
        assert!(error.contains("morphix"), "{error}");
        assert!(menu_command(Path::new("/opt/100%/morphix")).is_err());
        assert!(autostart_command(Path::new("/opt/100%/morphix-gui")).is_err());
    }

    #[test]
    fn absolute_utf8_accepts_a_plain_absolute_path() {
        assert_eq!(
            absolute_utf8(Path::new("/opt/morphix/morphix"), "morphix"),
            Ok("/opt/morphix/morphix")
        );
    }
}

#[cfg(target_os = "windows")]
pub use registry::WindowsBackend;

/// Chạm registry. Mọi thao tác đi qua `root` được inject: production dùng HKCU, test dùng một nhánh con riêng.
#[cfg(target_os = "windows")]
mod registry {
    use std::{io, path::PathBuf, rc::Rc};

    use winreg::{
        enums::{RegDisposition, HKEY_CURRENT_USER, REG_CREATED_NEW_KEY},
        RegKey,
    };

    use super::{
        autostart_command, bound_ownership, menu_entry, MenuEntry, AUTOSTART_OWNER_VALUE,
        COMMAND_SUBKEY, OWNER_KEY_PATH, RUN_KEY_PATH, RUN_VALUE_NAME, VERB_KEY_PATH,
        VERB_OWNER_VALUE,
    };
    use crate::integration::{
        files::{FileOwnership, Transaction},
        BinaryPaths, ComponentId, ComponentStatus, IntegrationBackend, IntegrationComponentReport,
    };

    pub struct WindowsBackend {
        // WHY Rc: thao tác rollback cần giữ root sau khi hàm cài đã trả về.
        root: Rc<RegKey>,
    }

    impl WindowsBackend {
        pub fn from_current_user() -> Self {
            Self::new(RegKey::predef(HKEY_CURRENT_USER))
        }

        pub fn new(root: RegKey) -> Self {
            WindowsBackend {
                root: Rc::new(root),
            }
        }

        fn install_verb(
            &self,
            id: ComponentId,
            binaries: &BinaryPaths,
        ) -> IntegrationComponentReport {
            let entry = match menu_entry(&binaries.morphix) {
                Ok(entry) => entry,
                Err(message) => {
                    return report(id, ComponentStatus::Failed, false, VERB_KEY_PATH, &message)
                }
            };
            let (ownership, current) = match self.read_verb() {
                Ok(state) => state,
                Err(error) => return io_failure(id, VERB_KEY_PATH, &error),
            };
            match ownership {
                FileOwnership::Foreign => return conflict(id, VERB_KEY_PATH),
                FileOwnership::Owned if current.as_ref() == Some(&entry) => {
                    return report(
                        id,
                        ComponentStatus::AlreadyInstalled,
                        false,
                        VERB_KEY_PATH,
                        "đã cài sẵn",
                    )
                }
                FileOwnership::Owned | FileOwnership::Absent => {}
            }
            let mut tx = Transaction::new();
            match self.write_verb(&entry, &mut tx) {
                Ok(()) => {
                    tx.commit();
                    report(
                        id,
                        ComponentStatus::Installed,
                        true,
                        VERB_KEY_PATH,
                        "đã cài",
                    )
                }
                // WHY không commit: `tx` xoá key vừa tạo khi rời phạm vi, nên lần lỗi không để lại verb dở dang.
                Err(error) => io_failure(id, VERB_KEY_PATH, &error),
            }
        }

        /// WHY record cuối: record chỉ được ghi sau khi command đã có, nên không có record nào trỏ tới
        /// một command Morphix chưa ghi xong.
        fn write_verb(&self, entry: &MenuEntry, tx: &mut Transaction) -> io::Result<()> {
            let (verb, disposition) = self.root.create_subkey(VERB_KEY_PATH)?;
            self.defer_delete_if_created(disposition, VERB_KEY_PATH, tx);
            write_string(&verb, "MUIVerb", &entry.display_name)?;
            write_string(&verb, "MultiSelectModel", &entry.multi_select_model)?;
            let (command, _) = verb.create_subkey(COMMAND_SUBKEY)?;
            write_string(&command, "", &entry.command)?;
            write_string(&verb, VERB_OWNER_VALUE, &entry.command)
        }

        fn uninstall_verb(&self, id: ComponentId) -> IntegrationComponentReport {
            match self.read_verb() {
                Err(error) => io_failure(id, VERB_KEY_PATH, &error),
                Ok((FileOwnership::Absent, _)) => absent(id, VERB_KEY_PATH),
                Ok((FileOwnership::Foreign, _)) => conflict(id, VERB_KEY_PATH),
                Ok((FileOwnership::Owned, _)) => match self.root.delete_subkey_all(VERB_KEY_PATH) {
                    Ok(()) => report(id, ComponentStatus::Removed, true, VERB_KEY_PATH, "đã gỡ"),
                    Err(error) => io_failure(id, VERB_KEY_PATH, &error),
                },
            }
        }

        fn inspect_verb(
            &self,
            id: ComponentId,
            binaries: &BinaryPaths,
        ) -> IntegrationComponentReport {
            let entry = match menu_entry(&binaries.morphix) {
                Ok(entry) => entry,
                Err(message) => {
                    return report(id, ComponentStatus::Failed, false, VERB_KEY_PATH, &message)
                }
            };
            match self.read_verb() {
                Err(error) => io_failure(id, VERB_KEY_PATH, &error),
                Ok((FileOwnership::Absent, _)) => missing(id, VERB_KEY_PATH),
                Ok((FileOwnership::Foreign, _)) => conflict(id, VERB_KEY_PATH),
                Ok((FileOwnership::Owned, current)) if current.as_ref() == Some(&entry) => report(
                    id,
                    ComponentStatus::Installed,
                    false,
                    VERB_KEY_PATH,
                    "đang hoạt động",
                ),
                Ok((FileOwnership::Owned, _)) => stale(id, VERB_KEY_PATH),
            }
        }

        /// Đọc verb và sở hữu trong một lần; `None` khi key không có đủ value để dựng lại mục menu.
        fn read_verb(&self) -> io::Result<(FileOwnership, Option<MenuEntry>)> {
            let Some(verb) = open_optional(&self.root, VERB_KEY_PATH)? else {
                return Ok((FileOwnership::Absent, None));
            };
            let marker = read_string(&verb, VERB_OWNER_VALUE)?;
            let command = match open_optional(&verb, COMMAND_SUBKEY)? {
                Some(command_key) => read_string(&command_key, "")?,
                None => None,
            };
            let ownership = bound_ownership(marker.as_deref(), command.as_deref());
            let entry = match (
                read_string(&verb, "MUIVerb")?,
                read_string(&verb, "MultiSelectModel")?,
                command,
            ) {
                (Some(display_name), Some(multi_select_model), Some(command)) => Some(MenuEntry {
                    display_name,
                    multi_select_model,
                    command,
                }),
                _ => None,
            };
            Ok((ownership, entry))
        }

        fn install_autostart(
            &self,
            id: ComponentId,
            binaries: &BinaryPaths,
        ) -> IntegrationComponentReport {
            let command = match autostart_command(&binaries.gui) {
                Ok(command) => command,
                Err(message) => {
                    return report(id, ComponentStatus::Failed, false, RUN_KEY_PATH, &message)
                }
            };
            let (ownership, current) = match self.read_autostart() {
                Ok(state) => state,
                Err(error) => return io_failure(id, RUN_KEY_PATH, &error),
            };
            match ownership {
                FileOwnership::Foreign => return conflict(id, RUN_KEY_PATH),
                FileOwnership::Owned if current.as_deref() == Some(command.as_str()) => {
                    return report(
                        id,
                        ComponentStatus::AlreadyInstalled,
                        false,
                        RUN_KEY_PATH,
                        "đã cài sẵn",
                    )
                }
                FileOwnership::Owned | FileOwnership::Absent => {}
            }
            let mut tx = Transaction::new();
            match self.write_autostart(&command, &mut tx) {
                Ok(()) => {
                    tx.commit();
                    report(id, ComponentStatus::Installed, true, RUN_KEY_PATH, "đã cài")
                }
                Err(error) => io_failure(id, RUN_KEY_PATH, &error),
            }
        }

        /// WHY value trước record: nếu ghi value lỗi thì không có record nào trỏ tới value chưa tồn tại.
        fn write_autostart(&self, command: &str, tx: &mut Transaction) -> io::Result<()> {
            let (run, _) = self.root.create_subkey(RUN_KEY_PATH)?;
            write_string(&run, RUN_VALUE_NAME, command)?;
            let (owner, disposition) = self.root.create_subkey(OWNER_KEY_PATH)?;
            self.defer_delete_if_created(disposition, OWNER_KEY_PATH, tx);
            write_string(&owner, AUTOSTART_OWNER_VALUE, command)
        }

        fn uninstall_autostart(&self, id: ComponentId) -> IntegrationComponentReport {
            match self.read_autostart() {
                Err(error) => io_failure(id, RUN_KEY_PATH, &error),
                Ok((FileOwnership::Absent, _)) => absent(id, RUN_KEY_PATH),
                Ok((FileOwnership::Foreign, _)) => conflict(id, RUN_KEY_PATH),
                Ok((FileOwnership::Owned, _)) => match self.remove_autostart() {
                    Ok(()) => report(id, ComponentStatus::Removed, true, RUN_KEY_PATH, "đã gỡ"),
                    Err(error) => io_failure(id, RUN_KEY_PATH, &error),
                },
            }
        }

        /// Chỉ xoá value Run và marker của Morphix; key `Software\Morphix` chỉ bị xoá khi đã rỗng.
        fn remove_autostart(&self) -> io::Result<()> {
            let (run, _) = self.root.create_subkey(RUN_KEY_PATH)?;
            run.delete_value(RUN_VALUE_NAME)?;
            let (owner, _) = self.root.create_subkey(OWNER_KEY_PATH)?;
            owner.delete_value(AUTOSTART_OWNER_VALUE)?;
            let is_empty =
                owner.enum_values().next().is_none() && owner.enum_keys().next().is_none();
            drop(owner);
            if is_empty {
                self.root.delete_subkey(OWNER_KEY_PATH)?;
            }
            Ok(())
        }

        fn inspect_autostart(
            &self,
            id: ComponentId,
            binaries: &BinaryPaths,
        ) -> IntegrationComponentReport {
            let command = match autostart_command(&binaries.gui) {
                Ok(command) => command,
                Err(message) => {
                    return report(id, ComponentStatus::Failed, false, RUN_KEY_PATH, &message)
                }
            };
            match self.read_autostart() {
                Err(error) => io_failure(id, RUN_KEY_PATH, &error),
                Ok((FileOwnership::Absent, _)) => missing(id, RUN_KEY_PATH),
                Ok((FileOwnership::Foreign, _)) => conflict(id, RUN_KEY_PATH),
                Ok((FileOwnership::Owned, current))
                    if current.as_deref() == Some(command.as_str()) =>
                {
                    report(
                        id,
                        ComponentStatus::Installed,
                        false,
                        RUN_KEY_PATH,
                        "đang hoạt động",
                    )
                }
                Ok((FileOwnership::Owned, _)) => stale(id, RUN_KEY_PATH),
            }
        }

        fn read_autostart(&self) -> io::Result<(FileOwnership, Option<String>)> {
            let value = match open_optional(&self.root, RUN_KEY_PATH)? {
                Some(run) => read_string(&run, RUN_VALUE_NAME)?,
                None => None,
            };
            let marker = match open_optional(&self.root, OWNER_KEY_PATH)? {
                Some(owner) => read_string(&owner, AUTOSTART_OWNER_VALUE)?,
                None => None,
            };
            Ok((bound_ownership(marker.as_deref(), value.as_deref()), value))
        }

        /// WHY chỉ khi key do lần này tạo: key đã có từ trước (kể cả của Morphix cũ) không được xoá khi rollback.
        fn defer_delete_if_created(
            &self,
            disposition: RegDisposition,
            key: &'static str,
            tx: &mut Transaction,
        ) {
            if disposition == REG_CREATED_NEW_KEY {
                let root = Rc::clone(&self.root);
                tx.defer(move || root.delete_subkey_all(key));
            }
        }
    }

    impl IntegrationBackend for WindowsBackend {
        fn platform(&self) -> &'static str {
            "windows"
        }

        fn install(&self, id: ComponentId, binaries: &BinaryPaths) -> IntegrationComponentReport {
            match id {
                ComponentId::WindowsContextMenu => self.install_verb(id, binaries),
                ComponentId::Autostart => self.install_autostart(id, binaries),
                _ => not_applicable(id),
            }
        }

        fn uninstall(&self, id: ComponentId) -> IntegrationComponentReport {
            match id {
                ComponentId::WindowsContextMenu => self.uninstall_verb(id),
                ComponentId::Autostart => self.uninstall_autostart(id),
                _ => not_applicable(id),
            }
        }

        fn inspect(&self, id: ComponentId, binaries: &BinaryPaths) -> IntegrationComponentReport {
            match id {
                ComponentId::WindowsContextMenu => self.inspect_verb(id, binaries),
                ComponentId::Autostart => self.inspect_autostart(id, binaries),
                _ => not_applicable(id),
            }
        }
    }

    /// `None` khi key không tồn tại; lỗi khác được trả lên để không đoán sở hữu khi không đọc được.
    fn open_optional(parent: &RegKey, path: &str) -> io::Result<Option<RegKey>> {
        match parent.open_subkey(path) {
            Ok(key) => Ok(Some(key)),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error),
        }
    }

    fn read_string(key: &RegKey, name: &str) -> io::Result<Option<String>> {
        match key.get_value::<String, _>(name) {
            Ok(value) => Ok(Some(value)),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error),
        }
    }

    /// WHY `to_owned`: `set_value` nhận tham chiếu tới kiểu `ToRegValue`, và `String` là kiểu được hỗ trợ chắc chắn.
    fn write_string(key: &RegKey, name: &str, value: &str) -> io::Result<()> {
        key.set_value(name, &value.to_owned())
    }

    fn key_label(key: &str) -> PathBuf {
        PathBuf::from(format!(r"HKCU\{key}"))
    }

    fn report(
        id: ComponentId,
        status: ComponentStatus,
        changed: bool,
        key: &str,
        message: &str,
    ) -> IntegrationComponentReport {
        IntegrationComponentReport {
            id,
            status,
            changed,
            path: Some(key_label(key)),
            message: message.to_string(),
        }
    }

    fn io_failure(id: ComponentId, key: &str, error: &io::Error) -> IntegrationComponentReport {
        report(
            id,
            ComponentStatus::Failed,
            false,
            key,
            &format!("không đọc/ghi được registry {key}: {error}"),
        )
    }

    fn conflict(id: ComponentId, key: &str) -> IntegrationComponentReport {
        report(
            id,
            ComponentStatus::Conflict,
            false,
            key,
            "mục registry không thuộc Morphix tại đường dẫn này; giữ nguyên",
        )
    }

    fn absent(id: ComponentId, key: &str) -> IntegrationComponentReport {
        report(id, ComponentStatus::AlreadyAbsent, false, key, "chưa cài")
    }

    fn missing(id: ComponentId, key: &str) -> IntegrationComponentReport {
        report(id, ComponentStatus::Missing, false, key, "chưa cài")
    }

    fn stale(id: ComponentId, key: &str) -> IntegrationComponentReport {
        report(
            id,
            ComponentStatus::Stale,
            false,
            key,
            "morphix đã được di chuyển hoặc cập nhật; chạy lại install-integration",
        )
    }

    fn not_applicable(id: ComponentId) -> IntegrationComponentReport {
        IntegrationComponentReport {
            id,
            status: ComponentStatus::NotApplicable,
            changed: false,
            path: None,
            message: "tích hợp này không áp dụng trên Windows".to_string(),
        }
    }
}
