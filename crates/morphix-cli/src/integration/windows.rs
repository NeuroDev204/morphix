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

/// WHY `*`: Morphix làm việc với mọi loại file (ảnh, PDF, media, văn bản, file nén), không chỉ ảnh.
pub const VERB_KEY_PATH: &str = r"Software\Classes\*\shell\Morphix";
/// Key của bản 0.1.x, chỉ hiện với ảnh. Cài và gỡ đều dọn key này (nếu là của Morphix) để không hiện hai mục.
pub const LEGACY_VERB_KEY_PATH: &str =
    r"Software\Classes\SystemFileAssociations\image\shell\Morphix";
pub const SHELL_EXTENSION_CLSID: &str = morphix_shell::CLSID_STRING;
pub const SHELL_EXTENSION_KEY_PATH: &str =
    r"Software\Classes\*\shellex\ContextMenuHandlers\Morphix";
pub const SHELL_EXTENSION_CLSID_KEY_PATH: &str =
    r"Software\Classes\CLSID\{2131F98F-8888-464C-B3D7-80E2BBEEDDE1}";
pub const SHELL_EXTENSION_SERVER_KEY_PATH: &str =
    r"Software\Classes\CLSID\{2131F98F-8888-464C-B3D7-80E2BBEEDDE1}\InprocServer32";
pub const SHELL_EXTENSION_DLL_NAME: &str = morphix_shell::DLL_FILE_NAME;
pub const SHELL_EXTENSION_OWNER_VALUE: &str = "MorphixOwner";
pub const SHELL_EXTENSION_THREADING_MODEL: &str = "Apartment";
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

pub fn shell_extension_dll_path(morphix: &Path) -> std::path::PathBuf {
    morphix.with_file_name(SHELL_EXTENSION_DLL_NAME)
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
        let path = if cfg!(windows) {
            r"C:\opt\100%\morphix"
        } else {
            "/opt/100%/morphix"
        };
        let gui_path = if cfg!(windows) {
            r"C:\opt\100%\morphix-gui"
        } else {
            "/opt/100%/morphix-gui"
        };
        let error = absolute_utf8(Path::new(path), "morphix")
            .expect_err("đường dẫn chứa % phải bị từ chối");
        assert!(error.contains("'%'"), "{error}");
        assert!(error.contains("morphix"), "{error}");
        assert!(menu_command(Path::new(path)).is_err());
        assert!(autostart_command(Path::new(gui_path)).is_err());
    }

    #[test]
    fn absolute_utf8_accepts_a_plain_absolute_path() {
        let path = if cfg!(windows) {
            r"C:\opt\morphix\morphix"
        } else {
            "/opt/morphix/morphix"
        };
        assert_eq!(absolute_utf8(Path::new(path), "morphix"), Ok(path));
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
        autostart_command, bound_ownership, shell_extension_dll_path, MenuEntry,
        AUTOSTART_OWNER_VALUE, COMMAND_SUBKEY, LEGACY_VERB_KEY_PATH, OWNER_KEY_PATH, RUN_KEY_PATH,
        RUN_VALUE_NAME, SHELL_EXTENSION_CLSID, SHELL_EXTENSION_CLSID_KEY_PATH,
        SHELL_EXTENSION_KEY_PATH, SHELL_EXTENSION_OWNER_VALUE, SHELL_EXTENSION_SERVER_KEY_PATH,
        SHELL_EXTENSION_THREADING_MODEL, VERB_KEY_PATH, VERB_OWNER_VALUE,
    };
    use crate::integration::{
        files::{FileOwnership, Transaction},
        BinaryPaths, ComponentId, ComponentStatus, IntegrationBackend, IntegrationComponentReport,
    };

    pub struct WindowsBackend {
        // WHY Rc: thao tác rollback cần giữ root sau khi hàm cài đã trả về.
        root: Rc<RegKey>,
    }

    struct ShellHandlerState {
        ownership: FileOwnership,
        handler_ownership: FileOwnership,
        server_ownership: FileOwnership,
        class_id: Option<String>,
        server_path: Option<String>,
        threading_model: Option<String>,
    }

    impl ShellHandlerState {
        fn matches(&self, expected_server_path: &str) -> bool {
            self.ownership == FileOwnership::Owned
                && self.class_id.as_deref() == Some(SHELL_EXTENSION_CLSID)
                && self.server_path.as_deref() == Some(expected_server_path)
                && self.threading_model.as_deref() == Some(SHELL_EXTENSION_THREADING_MODEL)
        }
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

        fn install_shell_handler(
            &self,
            id: ComponentId,
            binaries: &BinaryPaths,
        ) -> IntegrationComponentReport {
            let library_path = shell_extension_dll_path(&binaries.morphix);
            let Some(library_path_text) = library_path.to_str() else {
                return report(
                    id,
                    ComponentStatus::Failed,
                    false,
                    SHELL_EXTENSION_KEY_PATH,
                    "đường dẫn shell extension không phải UTF-8",
                );
            };
            if !library_path.is_file() {
                return report(
                    id,
                    ComponentStatus::Failed,
                    false,
                    SHELL_EXTENSION_KEY_PATH,
                    &format!(
                        "không tìm thấy shell extension tại {}",
                        library_path.display()
                    ),
                );
            }
            let previous_verb_ownership = match self.read_verb_at(VERB_KEY_PATH) {
                Ok((ownership, _)) => ownership,
                Err(error) => return io_failure(id, VERB_KEY_PATH, &error),
            };
            if previous_verb_ownership == FileOwnership::Foreign {
                return conflict(id, VERB_KEY_PATH);
            }
            let state = match self.read_shell_handler_state() {
                Ok(state) => state,
                Err(error) => return io_failure(id, SHELL_EXTENSION_KEY_PATH, &error),
            };
            match state.ownership {
                FileOwnership::Foreign => return conflict(id, SHELL_EXTENSION_KEY_PATH),
                FileOwnership::Owned if state.matches(library_path_text) => {
                    match self.remove_previous_verbs() {
                        Ok(true) => {
                            return report(
                                id,
                                ComponentStatus::Installed,
                                true,
                                SHELL_EXTENSION_KEY_PATH,
                                "đã cập nhật menu Morphix native",
                            )
                        }
                        Ok(false) => {
                            return report(
                                id,
                                ComponentStatus::AlreadyInstalled,
                                false,
                                SHELL_EXTENSION_KEY_PATH,
                                "đã cài sẵn",
                            )
                        }
                        Err(error) => return io_failure(id, SHELL_EXTENSION_KEY_PATH, &error),
                    }
                }
                FileOwnership::Owned | FileOwnership::Absent => {}
            }
            let mut tx = Transaction::new();
            match self.write_shell_handler(library_path_text, &mut tx) {
                Ok(()) => {
                    tx.commit();
                    if let Err(error) = self.remove_previous_verbs() {
                        return io_failure(id, SHELL_EXTENSION_KEY_PATH, &error);
                    }
                    report(
                        id,
                        ComponentStatus::Installed,
                        true,
                        SHELL_EXTENSION_KEY_PATH,
                        "đã cài",
                    )
                }
                Err(error) => io_failure(id, SHELL_EXTENSION_KEY_PATH, &error),
            }
        }

        fn write_shell_handler(&self, library_path: &str, tx: &mut Transaction) -> io::Result<()> {
            let (handler, disposition) = self.root.create_subkey(SHELL_EXTENSION_KEY_PATH)?;
            self.defer_delete_if_created(disposition, SHELL_EXTENSION_KEY_PATH, tx);
            write_string(&handler, "", SHELL_EXTENSION_CLSID)?;
            write_string(&handler, SHELL_EXTENSION_OWNER_VALUE, SHELL_EXTENSION_CLSID)?;

            let (server, disposition) = self.root.create_subkey(SHELL_EXTENSION_SERVER_KEY_PATH)?;
            self.defer_delete_if_created(disposition, SHELL_EXTENSION_SERVER_KEY_PATH, tx);
            write_string(&server, "", library_path)?;
            write_string(&server, "ThreadingModel", SHELL_EXTENSION_THREADING_MODEL)?;
            write_string(&server, SHELL_EXTENSION_OWNER_VALUE, library_path)
        }

        fn read_shell_handler_state(&self) -> io::Result<ShellHandlerState> {
            let (handler_ownership, class_id, _) =
                read_registration_pair(&self.root, SHELL_EXTENSION_KEY_PATH)?;
            let (server_ownership, server_path, _) =
                read_registration_pair(&self.root, SHELL_EXTENSION_SERVER_KEY_PATH)?;
            let threading_model = match open_optional(&self.root, SHELL_EXTENSION_SERVER_KEY_PATH)?
            {
                Some(server) => read_string(&server, "ThreadingModel")?,
                None => None,
            };
            let ownership = match (handler_ownership, server_ownership) {
                (FileOwnership::Foreign, _) | (_, FileOwnership::Foreign) => FileOwnership::Foreign,
                (FileOwnership::Absent, FileOwnership::Absent) => FileOwnership::Absent,
                _ => FileOwnership::Owned,
            };
            Ok(ShellHandlerState {
                ownership,
                handler_ownership,
                server_ownership,
                class_id,
                server_path,
                threading_model,
            })
        }

        fn remove_shell_handler(&self, state: &ShellHandlerState) -> io::Result<bool> {
            if state.ownership == FileOwnership::Foreign {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "shell handler không thuộc Morphix",
                ));
            }
            let mut removed = false;
            if state.handler_ownership == FileOwnership::Owned {
                self.root.delete_subkey_all(SHELL_EXTENSION_KEY_PATH)?;
                removed = true;
            }
            if state.server_ownership == FileOwnership::Owned {
                self.root
                    .delete_subkey_all(SHELL_EXTENSION_CLSID_KEY_PATH)?;
                removed = true;
            }
            Ok(removed)
        }

        fn remove_previous_verbs(&self) -> io::Result<bool> {
            let (ownership, _) = self.read_verb_at(VERB_KEY_PATH)?;
            let mut removed = match ownership {
                FileOwnership::Owned => {
                    self.root.delete_subkey_all(VERB_KEY_PATH)?;
                    true
                }
                FileOwnership::Absent => false,
                FileOwnership::Foreign => {
                    return Err(io::Error::new(
                        io::ErrorKind::PermissionDenied,
                        "verb Morphix hiện có không thuộc Morphix",
                    ));
                }
            };
            removed |= self.remove_legacy_verb()?;
            Ok(removed)
        }

        /// Xoá verb của bản cũ nếu nó mang marker Morphix; trả `true` khi đã xoá.
        fn remove_legacy_verb(&self) -> io::Result<bool> {
            match self.read_verb_at(LEGACY_VERB_KEY_PATH)?.0 {
                FileOwnership::Owned => {
                    self.root.delete_subkey_all(LEGACY_VERB_KEY_PATH)?;
                    Ok(true)
                }
                FileOwnership::Absent | FileOwnership::Foreign => Ok(false),
            }
        }

        fn uninstall_shell_handler(&self, id: ComponentId) -> IntegrationComponentReport {
            let state = match self.read_shell_handler_state() {
                Ok(state) => state,
                Err(error) => return io_failure(id, SHELL_EXTENSION_KEY_PATH, &error),
            };
            if state.ownership == FileOwnership::Foreign {
                return conflict(id, SHELL_EXTENSION_KEY_PATH);
            }
            let previous_verb = match self.read_verb_at(VERB_KEY_PATH) {
                Ok((ownership, _)) => ownership,
                Err(error) => return io_failure(id, VERB_KEY_PATH, &error),
            };
            if previous_verb == FileOwnership::Foreign {
                return conflict(id, VERB_KEY_PATH);
            }
            let had_owned_registration =
                state.ownership == FileOwnership::Owned || previous_verb == FileOwnership::Owned;
            match self.remove_shell_handler(&state) {
                Ok(_) => {}
                Err(error) => return io_failure(id, SHELL_EXTENSION_KEY_PATH, &error),
            }
            match self.remove_previous_verbs() {
                Ok(removed) if had_owned_registration || removed => report(
                    id,
                    ComponentStatus::Removed,
                    true,
                    SHELL_EXTENSION_KEY_PATH,
                    "đã gỡ",
                ),
                Ok(_) => absent(id, SHELL_EXTENSION_KEY_PATH),
                Err(error) => io_failure(id, SHELL_EXTENSION_KEY_PATH, &error),
            }
        }

        fn inspect_shell_handler(
            &self,
            id: ComponentId,
            binaries: &BinaryPaths,
        ) -> IntegrationComponentReport {
            let state = match self.read_shell_handler_state() {
                Ok(state) => state,
                Err(error) => return io_failure(id, SHELL_EXTENSION_KEY_PATH, &error),
            };
            if state.ownership == FileOwnership::Foreign {
                return conflict(id, SHELL_EXTENSION_KEY_PATH);
            }
            let previous_verb = match self.read_verb_at(VERB_KEY_PATH) {
                Ok((ownership, _)) => ownership,
                Err(error) => return io_failure(id, VERB_KEY_PATH, &error),
            };
            if previous_verb == FileOwnership::Foreign {
                return conflict(id, VERB_KEY_PATH);
            }
            let library_path = shell_extension_dll_path(&binaries.morphix);
            let library_path_text = library_path.to_str();
            match state.ownership {
                FileOwnership::Foreign => conflict(id, SHELL_EXTENSION_KEY_PATH),
                FileOwnership::Owned
                    if library_path.is_file()
                        && library_path_text.is_some_and(|path| state.matches(path)) =>
                {
                    report(
                        id,
                        ComponentStatus::Installed,
                        false,
                        SHELL_EXTENSION_KEY_PATH,
                        "đang hoạt động",
                    )
                }
                FileOwnership::Owned => stale(id, SHELL_EXTENSION_KEY_PATH),
                FileOwnership::Absent if previous_verb == FileOwnership::Owned => {
                    stale(id, SHELL_EXTENSION_KEY_PATH)
                }
                FileOwnership::Absent => match self.read_verb_at(LEGACY_VERB_KEY_PATH) {
                    Ok((FileOwnership::Owned, _)) => stale(id, SHELL_EXTENSION_KEY_PATH),
                    Ok(_) => missing(id, SHELL_EXTENSION_KEY_PATH),
                    Err(error) => io_failure(id, LEGACY_VERB_KEY_PATH, &error),
                },
            }
        }

        fn read_verb_at(&self, path: &str) -> io::Result<(FileOwnership, Option<MenuEntry>)> {
            let Some(verb) = open_optional(&self.root, path)? else {
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
                ComponentId::WindowsContextMenu => self.install_shell_handler(id, binaries),
                ComponentId::Autostart => self.install_autostart(id, binaries),
                _ => not_applicable(id),
            }
        }

        fn uninstall(&self, id: ComponentId) -> IntegrationComponentReport {
            match id {
                ComponentId::WindowsContextMenu => self.uninstall_shell_handler(id),
                ComponentId::Autostart => self.uninstall_autostart(id),
                _ => not_applicable(id),
            }
        }

        fn inspect(&self, id: ComponentId, binaries: &BinaryPaths) -> IntegrationComponentReport {
            match id {
                ComponentId::WindowsContextMenu => self.inspect_shell_handler(id, binaries),
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

    fn read_registration_pair(
        root: &RegKey,
        path: &str,
    ) -> io::Result<(FileOwnership, Option<String>, Option<String>)> {
        let Some(key) = open_optional(root, path)? else {
            return Ok((FileOwnership::Absent, None, None));
        };
        let value = read_string(&key, "")?;
        let owner = read_string(&key, SHELL_EXTENSION_OWNER_VALUE)?;
        let ownership = match (owner.as_deref(), value.as_deref()) {
            (Some(owner), Some(value)) if owner == value => FileOwnership::Owned,
            (Some(_), None) => FileOwnership::Owned,
            _ => FileOwnership::Foreign,
        };
        Ok((ownership, value, owner))
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
