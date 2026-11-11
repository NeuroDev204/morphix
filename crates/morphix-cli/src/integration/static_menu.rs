//! File menu chuột phải sinh từ catalog cho các trình quản lý file chỉ lọc được theo loại file (Dolphin, thư mục
//! Scripts của Nautilus/Nemo/Caja). Danh sách việc lấy từ `morphix_core::static_menu_groups`, không có bảng thứ hai.

use std::path::{Path, PathBuf};

use morphix_core::{action_id, static_menu_groups};

use super::{
    files::MANAGED_MARKER,
    linux::{desktop_exec_argument, escape_desktop_value, posix_single_quote, utf8},
};

/// Thư mục gốc của cây việc trong thư mục Scripts. WHY khác tên script `Morphix` (mở Khay, Alt+C dựa vào nó):
/// một thư mục và một file cùng tên không thể cùng nằm trong Scripts.
pub const SCRIPTS_MENU_DIR: &str = "Morphix việc nhanh";

const EXECUTABLE_MODE: u32 = 0o755;

/// Một file do Morphix sinh ra; `relative` tính từ thư mục gốc của thành phần.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratedFile {
    pub relative: PathBuf,
    pub body: String,
    pub mode: u32,
}

/// Tên khoá `[Desktop Action …]` từ id việc: chỉ chữ, số và `-`.
fn desktop_action_name(id: &str) -> String {
    let safe: String = id
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    format!("Morphix-{safe}")
}

/// Mỗi nhóm một file `morphix-<key>.desktop`: `MimeType` của nhóm, `X-KDE-Submenu=Morphix` và mỗi việc một
/// `[Desktop Action …]` gọi `morphix menu --action <id> -- %F`.
/// WHY chung tên submenu: Dolphin gộp các service menu cùng `X-KDE-Submenu`, nên một file chỉ hiện việc của loại
/// của nó (cộng nhóm `any`) mà vẫn nằm trong một mục "Morphix".
pub fn dolphin_service_menus(morphix: &Path) -> Result<Vec<GeneratedFile>, String> {
    let executable = desktop_exec_argument(utf8(morphix)?);
    Ok(static_menu_groups()
        .into_iter()
        .map(|group| {
            let names: Vec<String> = group
                .actions
                .iter()
                .map(|action| desktop_action_name(&action_id(action.kind)))
                .collect();
            let mut lines = vec![
                format!("# {MANAGED_MARKER}"),
                "[Desktop Entry]".to_string(),
                "Type=Service".to_string(),
                // WHY Name ở nhóm chính: desktop-file-validate báo lỗi nếu thiếu.
                "Name=Morphix".to_string(),
                format!("MimeType={};", group.mime_types.join(";")),
                format!("Actions={};", names.join(";")),
                "ServiceTypes=KonqPopupMenu/Plugin".to_string(),
                "X-KDE-Submenu=Morphix".to_string(),
            ];
            for (action, name) in group.actions.iter().zip(&names) {
                let exec = format!(
                    "{executable} menu --action {} -- %F",
                    desktop_exec_argument(&action_id(action.kind))
                );
                lines.push(String::new());
                lines.push(format!("[Desktop Action {name}]"));
                lines.push(format!("Name={}", escape_desktop_value(&action.label)));
                lines.push(format!("Exec={}", escape_desktop_value(&exec)));
            }
            GeneratedFile {
                relative: PathBuf::from(format!("morphix-{}.desktop", group.key)),
                body: lines.join("\n") + "\n",
                // WHY thực thi được: KDE chỉ tin service menu có quyền thực thi.
                mode: EXECUTABLE_MODE,
            }
        })
        .collect())
}

/// Cây `<SCRIPTS_MENU_DIR>/<nhãn nhóm>/<nhãn việc>`; mỗi file là script sh gọi `morphix menu --action <id>`.
/// Thư mục con trong Scripts thành submenu; Scripts không lọc theo loại file nên mọi nhóm đều hiện.
pub fn scripts_menu_tree(morphix: &Path) -> Result<Vec<GeneratedFile>, String> {
    let morphix = posix_single_quote(utf8(morphix)?);
    Ok(static_menu_groups()
        .into_iter()
        .flat_map(|group| {
            let morphix = morphix.clone();
            group.actions.into_iter().map(move |action| {
                let id = action_id(action.kind);
                let body = [
                    "#!/bin/sh".to_string(),
                    format!("# {MANAGED_MARKER}"),
                    "if [ \"$#\" -eq 0 ]; then".to_string(),
                    // WHY mở Khay có lỗi: thư mục từ xa không truyền đường dẫn; thoát im lặng thì người bấm không thấy gì.
                    format!("    exec {morphix} tray --unsupported-selection"),
                    "fi".to_string(),
                    format!("exec {morphix} menu --action {} -- \"$@\"", posix_single_quote(&id)),
                ]
                .join("\n")
                    + "\n";
                GeneratedFile {
                    // WHY thay `/`: nhãn là tên file, `/` sẽ tạo thêm một cấp thư mục.
                    relative: Path::new(SCRIPTS_MENU_DIR)
                        .join(group.label)
                        .join(action.label.replace('/', "-")),
                    body,
                    mode: EXECUTABLE_MODE,
                }
            })
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use morphix_core::{action_id, static_menu_groups};
    use std::collections::BTreeSet;
    use std::path::Path;

    fn ids_after_action_flag(text: &str) -> BTreeSet<String> {
        text.split("--action ")
            .skip(1)
            .map(|rest| {
                rest.split(|c: char| c.is_whitespace() || c == '\'' || c == '"')
                    .next()
                    .unwrap()
                    .to_string()
            })
            .collect()
    }

    #[test]
    fn dolphin_menus_match_the_catalog() {
        let files = dolphin_service_menus(Path::new("/opt/morphix")).unwrap();
        let groups = static_menu_groups();
        assert_eq!(files.len(), groups.len());
        for (file, group) in files.iter().zip(&groups) {
            assert_eq!(
                file.relative,
                Path::new(&format!("morphix-{}.desktop", group.key))
            );
            assert_eq!(file.mode, 0o755);
            assert_eq!(file.body.lines().next(), Some("# Managed by Morphix"));
            assert!(file.body.contains("X-KDE-Submenu=Morphix\n"), "{}", file.body);
            assert!(file
                .body
                .contains(&format!("MimeType={};\n", group.mime_types.join(";"))));
            let wanted: BTreeSet<String> =
                group.actions.iter().map(|a| action_id(a.kind)).collect();
            assert_eq!(ids_after_action_flag(&file.body), wanted, "{}", group.key);
        }
    }

    #[test]
    fn dolphin_exec_quotes_a_binary_path_with_spaces_and_apostrophes() {
        let files = dolphin_service_menus(Path::new("/opt/My App's/morphix")).unwrap();
        assert!(
            files[0].body.contains(
                "Exec=\"/opt/My App's/morphix\" menu --action merge -- %F\n"
            ),
            "{}",
            files[0].body
        );
    }

    #[test]
    fn scripts_tree_matches_the_catalog() {
        let files = scripts_menu_tree(Path::new("/opt/morphix")).unwrap();
        let mut seen: BTreeSet<(String, String)> = BTreeSet::new();
        for group in static_menu_groups() {
            for action in &group.actions {
                let relative = Path::new(SCRIPTS_MENU_DIR)
                    .join(group.label)
                    .join(action.label.replace('/', "-"));
                let file = files
                    .iter()
                    .find(|file| file.relative == relative)
                    .unwrap_or_else(|| panic!("thiếu {}", relative.display()));
                assert_eq!(file.mode, 0o755);
                assert_eq!(file.body.lines().nth(1), Some("# Managed by Morphix"));
                assert!(file
                    .body
                    .contains(&format!("--action '{}' -- \"$@\"", action_id(action.kind))));
                assert!(seen.insert((group.key.to_string(), action_id(action.kind))));
            }
        }
        assert_eq!(files.len(), seen.len(), "có file thừa hoặc trùng tên");
    }

    #[cfg(unix)]
    #[test]
    fn generated_commands_survive_odd_binary_paths_and_file_names() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let bin_dir = dir.path().join("My App's");
        std::fs::create_dir_all(&bin_dir).unwrap();
        let record = dir.path().join("argv.txt");
        let fake = bin_dir.join("morphix");
        std::fs::write(
            &fake,
            format!("#!/bin/sh\nfor a in \"$@\"; do printf '%s\\n' \"$a\"; done > '{}'\n", record.display()),
        )
        .unwrap();
        std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();

        let files = scripts_menu_tree(&fake).unwrap();
        let script = files
            .iter()
            .find(|file| file.body.contains("--action 'convert:webp'"))
            .unwrap();
        let path = dir.path().join("script.sh");
        std::fs::write(&path, &script.body).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        // WHY thử lại: test khác chạy song song có thể đang `fork`, giữ tạm fd ghi của file vừa tạo,
        // nên exec lúc đó báo "Text file busy" (ETXTBSY) dù file đã đóng.
        let mut attempts = 0;
        let status = loop {
            attempts += 1;
            match std::process::Command::new(&path)
                .args(["ảnh của tôi.png", "a'b.png"])
                .status()
            {
                Ok(status) if status.code() != Some(126) => break status,
                Err(error) if error.kind() != std::io::ErrorKind::ExecutableFileBusy => {
                    panic!("{error}")
                }
                _ if attempts >= 40 => panic!("script vẫn báo Text file busy sau {attempts} lần thử"),
                _ => std::thread::sleep(std::time::Duration::from_millis(25)),
            }
        };
        assert!(status.success());
        assert_eq!(
            std::fs::read_to_string(&record).unwrap(),
            "menu\n--action\nconvert:webp\n--\nảnh của tôi.png\na'b.png\n"
        );
        std::fs::remove_file(&record).unwrap();
        let no_args = (0..40)
            .find_map(|_| match std::process::Command::new(&path).output() {
                Ok(output) => Some(output),
                Err(error) if error.kind() == std::io::ErrorKind::ExecutableFileBusy => {
                    std::thread::sleep(std::time::Duration::from_millis(25));
                    None
                }
                Err(error) => panic!("{error}"),
            })
            .expect("script vẫn báo Text file busy");
        // WHY mở Khay có lỗi: thư mục từ xa không truyền đường dẫn; thoát im lặng thì người dùng bấm mà không thấy gì.
        assert!(no_args.status.success());
        assert_eq!(
            std::fs::read_to_string(&record).unwrap(),
            "tray\n--unsupported-selection\n"
        );
    }
}
