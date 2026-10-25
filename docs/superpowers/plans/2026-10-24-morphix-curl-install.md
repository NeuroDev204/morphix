# Morphix cài bằng curl cho mọi distro — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Một lệnh `curl -fsSL …/install.sh | sh` cài Morphix riêng cho người dùng (không `sudo`) trên mọi distro amd64 có glibc ≥ 2.35 và `webkit2gtk-4.1`, kèm menu chuột phải cho Nautilus, Dolphin, Nemo, Thunar, Caja.

**Architecture:** CLI biết hai chế độ cài: *package* (binary dưới `/usr`, bản `.deb`) và *user* (binary dưới `~/.local`, bản curl). Ở chế độ user, mọi mục tích hợp là file per-user có ownership marker, do `morphix install-integration` ghi; gói tar chỉ chứa binary, thư viện đi kèm, icon và giấy phép theo đúng bố cục `bundled_lib_dir()` (`<prefix>/bin` → `<prefix>/lib/morphix`). `install.sh` (POSIX sh) kiểm môi trường, tải + kiểm checksum, thay bản cũ, rồi gọi `morphix install-integration`. GitHub Actions build tarball trên Ubuntu 22.04 và `.deb` Noble khi gắn tag.

**Tech Stack:** Rust (workspace hiện có, clap, tempfile cho test), POSIX sh, Python 3 unittest cho test script, GitHub Actions.

**Spec:** `docs/superpowers/specs/2026-10-24-morphix-curl-install-design.md`

**Nhánh làm việc:** `codex/morphix-ubuntu-complete` (worktree `<worktree>`). Không làm trên `feature/morphix-gui`: nhánh đó thiếu tích hợp DING/Nautilus extension/launcher.

## Global Constraints

- Không `sudo`, không ghi ngoài `$HOME` (trừ thư mục tạm). Thiếu thư viện hệ thống thì **in** lệnh cài, không tự chạy.
- Hỗ trợ: Linux x86_64, glibc ≥ 2.35. Ngoài phạm vi: ARM, Flatpak/Snap/AppImage/rpm/AUR, cập nhật ngầm.
- Alt+C (`gnome_shortcut`, `nautilus_accel`) chỉ trên GNOME **và** X11; Wayland và desktop khác là `not_applicable` kèm lý do.
- Menu desktop DING chỉ có ở `.deb`; bản curl báo `desktop_context_menu` là `not_applicable`.
- Mọi file tích hợp mang marker `Managed by Morphix` (hằng `MANAGED_MARKER` hiện có); gỡ chỉ xoá file/khối có marker. File cấu hình dùng chung (`uca.xml`) chỉ được sửa đúng khối của Morphix.
- Máy đã có gói `.deb` `morphix`: `install.sh` dừng, in `sudo apt remove morphix`. `morphix uninstall` chạy từ `/usr` chỉ gỡ tích hợp per-user và in lệnh `apt remove`, không đụng `/usr`.
- Cập nhật/gỡ: tắt `morphix-gui` đang chạy trước khi thay/xoá file; cập nhật xong khởi động lại nếu trước đó nó đang chạy.
- Giữ dữ liệu người dùng `~/.local/share/morphix` khi gỡ, và nói rõ đã giữ.
- Giấy phép GPL-3.0-or-later; tarball kèm `LICENSE`, giấy phép bên thứ ba, lời đề nghị mã nguồn ffmpeg.
- Repo công khai: `https://github.com/NeuroDev204/morphix`. Asset không gắn số phiên bản: `morphix-x86_64-linux.tar.gz`, `morphix-x86_64-linux.tar.gz.sha256`, `install.sh`, `morphix_<ver>-3_amd64.deb`.
- Chuỗi người dùng thấy: tiếng Việt. Comment code: `WHY …` tiếng Việt như repo. Commit: Conventional Commits có scope crate.
- Mỗi task Rust kết thúc bằng: `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, `cargo check --target x86_64-pc-windows-gnu -p morphix-cli -p morphix-gui` (enum `ComponentId` mới phải được Windows backend xử lý), `rustfmt --edition 2021 --check` trên file đã sửa.
- **Đã đổi so với spec (thực tế nhánh):** `.deb` jammy không còn build được (`build-deb.sh` từ chối jammy vì DING ghim Ubuntu 24.04), nên CI chỉ build `.deb` Noble. **Người viết plan chọn:** ffmpeg trong tarball là bản static GPL ghim phiên bản của BtbN/FFmpeg-Builds (có libx264), thay cho bao đóng thư viện Ubuntu của `.deb`, để không phụ thuộc thư viện distro.

## Review Focus

1. `$HOME` có khoảng trắng hoặc ký tự tiếng Việt → mọi `Exec`/`command` trong file tích hợp vẫn chạy đúng (đã quote). Test: Task 4 `nemo_and_thunar_bodies_quote_paths_with_spaces`, Task 5 `command_escapes_xml_and_shell`.
2. `uca.xml` đã có action của người dùng → sau cài và gỡ, phần ngoài khối Morphix giữ nguyên từng byte. Test: Task 5 `thunar_install_and_uninstall_preserve_other_actions`.
3. Tải hỏng / sai checksum khi đang có bản cũ → bản cũ còn nguyên và chạy được. Test: Task 8 `test_checksum_mismatch_keeps_previous_install`.
4. Chạy lại lệnh cài khi GUI đang chạy → GUI bị tắt trước khi thay, được bật lại sau. Test: Task 8 `test_update_restarts_running_gui`.
5. `morphix uninstall` gặp `lib/morphix` không có marker cài đặt (ví dụ thư mục do người khác tạo) → từ chối xoá, báo lỗi. Test: Task 6 `refuses_to_delete_lib_dir_without_install_marker`.

---

### Task 1: Giấy phép GPL-3.0-or-later

**Files:**
- Create: `LICENSE` (nguyên văn GPL-3.0 từ https://www.gnu.org/licenses/gpl-3.0.txt)
- Modify: `Cargo.toml` (`[workspace.package]` thêm `license = "GPL-3.0-or-later"`), `crates/*/Cargo.toml` (thêm `license.workspace = true`), `packaging/LICENSES-THIRD-PARTY.md` (thêm mục "Morphix: GPL-3.0-or-later")

**Interfaces:** Produces: file `LICENSE` ở root Morphix; Task 7 chép nó vào tarball.

- [ ] **Step 1:** Thêm `LICENSE` và các trường `license`.
- [ ] **Step 2:** Run `cargo metadata --no-deps --format-version 1 | python3 -c "import sys,json;print({p['name']:p['license'] for p in json.load(sys.stdin)['packages']})"` — Expected: cả 4 crate là `GPL-3.0-or-later`.
- [ ] **Step 3:** Commit `chore(morphix): license under GPL-3.0-or-later`.

### Task 2: Alt+C chỉ trên GNOME X11

**Files:**
- Modify: `crates/morphix-cli/src/integration/linux.rs` (`SessionEnv`)
- Modify: `crates/morphix-cli/src/integration/selection.rs` (`shortcut`, `accelerator`)
- Test: mod tests trong `selection.rs`

**Interfaces:**
- Produces: `SessionEnv { current_desktop: Option<String>, has_session_bus: bool, session_type: Option<String>, path: Option<OsString> }`; `SessionEnv::is_x11(&self) -> bool` (đúng khi `session_type` là `x11`, không phân biệt hoa thường; `None` coi là X11 để giữ hành vi cũ khi biến không được đặt); `SessionEnv::has_program(&self, name: &str) -> bool` (tìm file thực thi trong `path` bằng `std::env::split_paths`; dùng ở Task 4). `from_env()` đọc `XDG_SESSION_TYPE` và `PATH`. Cập nhật mọi chỗ dựng `SessionEnv` trong test.

- [ ] **Step 1: Test** `shortcut_and_accelerator_are_not_applicable_on_wayland`: backend GNOME có session bus, `session_type = Some("wayland")`; install `GnomeShortcut` và `NautilusAccel` qua `LinuxBackend::install` đều trả `NotApplicable`, message chứa `"chỉ có trên X11"`, fake tools không ghi nhận lời gọi `gsettings` nào.
- [ ] **Step 2:** Run `cargo test -p morphix-cli shortcut_and_accelerator_are_not_applicable_on_wayland` — Expected: FAIL.
- [ ] **Step 3:** Thêm `session_type`, `path`, `is_x11`, `has_program`. `shortcut` kiểm `is_x11` ngay sau `is_gnome`; `NautilusAccel` cũng `NotApplicable` khi không phải X11 (accel chỉ phục vụ Alt+C). Message: `"Alt+C chỉ có trên X11; trên Wayland hãy dùng menu chuột phải"`. Uninstall vẫn chạy trên Wayland (gỡ được phím tắt cài từ trước).
- [ ] **Step 4:** Run test + bộ lệnh của Global Constraints — Expected: PASS, 0 warning.
- [ ] **Step 5:** Commit `feat(morphix-cli): skip Alt+C integration outside GNOME on X11`.

### Task 3: Chế độ user — Nautilus extension, launcher, desktop menu

**Files:**
- Modify: `crates/morphix-cli/src/integration/{linux.rs,mod.rs}`
- Read (template, không đổi hành vi): `integration/nautilus/morphix_menu.py` — dòng `EXECUTABLE: str = "/usr/bin/morphix"` là điểm thay thế
- Test: `crates/morphix-cli/tests/integration.rs`

**Interfaces:**
- Produces: `pub enum InstallMode { Package, User }`; `pub fn install_mode(morphix: &Path) -> InstallMode` (`Package` khi path bắt đầu bằng `/usr/`); `pub fn nautilus_extension_body(morphix: &Path) -> Result<String, String>`; `pub fn application_launcher_body(morphix: &Path) -> Result<String, String>`.
- Vị trí per-user: `NautilusExtension` → `$XDG_DATA_HOME/nautilus-python/extensions/morphix_menu.py` (0644); `ApplicationLauncher` → `$XDG_DATA_HOME/applications/morphix.desktop` (0644).
- `nautilus_extension_body`: dòng đầu `# <MANAGED_MARKER>`, rồi nội dung `include_str!` của `morphix_menu.py` với đúng một lần thay dòng `EXECUTABLE: str = "/usr/bin/morphix"` bằng `EXECUTABLE: str = <serde_json::to_string(path)>` (JSON string là literal Python hợp lệ). Template không có đúng một dòng đó → `Err`.
- `application_launcher_body`: giống `packaging/morphix.desktop`, dòng đầu là marker, `Exec=<desktop_exec_argument(morphix)> tray`, thêm `TryExec=<morphix>`.

- [ ] **Step 1: Test** (binary từ `binaries_in(tempdir)` nằm ngoài `/usr` nên là chế độ user):
  - `user_mode_installs_nautilus_extension_and_launcher`: sau `integration::install`, hai file có ở vị trí per-user, cả hai `Installed`; extension chứa `EXECUTABLE: str = "<morphix>"` và không còn `/usr/bin/morphix`; launcher có `Exec=` trỏ tới morphix kèm ` tray`.
  - `user_mode_reports_desktop_menu_not_applicable`: `inspect` `DesktopContextMenu` → `NotApplicable`, message chứa `".deb"`.
  - `uninstall_keeps_unowned_launcher`: launcher per-user không có marker → sau `integration::uninstall` file vẫn nguyên (đúng hành vi `uninstall_file` hiện có với file không có marker).
  - `install_mode_detects_package_paths`: `install_mode(Path::new("/usr/bin/morphix")) == InstallMode::Package`, `install_mode(Path::new("/home/u/.local/bin/morphix")) == InstallMode::User`.
- [ ] **Step 2:** Run `cargo test -p morphix-cli --test integration user_mode` — Expected: FAIL.
- [ ] **Step 3:** Implement. Chế độ package giữ nguyên `inspect_package_file`/`inspect_desktop`; install trả kết quả inspect (gói quản lý). Chế độ user: `location`/`desired_file` trả vị trí per-user và body trên; `DesktopContextMenu` → `NotApplicable` `"menu trên desktop chỉ có trong gói .deb cho Ubuntu"`. Thêm `NautilusExtension`, `ApplicationLauncher` vào `INSTALLABLE_COMPONENTS`. Uninstall luôn thử vị trí per-user (an toàn nhờ marker).
- [ ] **Step 4:** Run bộ lệnh của Global Constraints và `python3 -m unittest discover -s integration/tests` — Expected: PASS.
- [ ] **Step 5:** Commit `feat(morphix-cli): per-user Nautilus extension and launcher for home installs`.

### Task 4: Nemo, Caja, và chỉ cài cho trình quản lý file có trên máy

**Files:**
- Modify: `crates/morphix-cli/src/integration/report.rs` (thêm `NemoAction` = `"nemo_action"`, `CajaScript` = `"caja_script"`, `ThunarAction` = `"thunar_action"`; Thunar hiện thực ở Task 5)
- Modify: `crates/morphix-cli/src/integration/{linux.rs,mod.rs,windows.rs}`
- Test: `crates/morphix-cli/tests/integration.rs`, mod tests trong `linux.rs`

**Interfaces:**
- Consumes: `SessionEnv::has_program` (Task 2).
- Produces: `pub fn nemo_action_body(morphix: &Path) -> Result<String, String>`; vị trí `NemoAction` → `$XDG_DATA_HOME/nemo/actions/morphix.nemo_action` (0644); `CajaScript` → `$XDG_CONFIG_HOME/caja/scripts/Morphix` (0755, body = `nautilus_script_body`).
- `nemo_action_body`: marker, `[Nemo Action]`, `Name=Morphix`, `Comment=Mở Morphix cho các file đã chọn`, `Exec=<desktop_exec_argument(morphix)> menu --uris %U`, `Selection=notnone`, `Extensions=nodirs;`, `Icon-Name=morphix`.
- Chương trình ↔ thành phần (chỉ cài khi có trên PATH): `nautilus` → NautilusScript, NautilusAccel, NautilusExtension; `dolphin` → DolphinServiceMenu; `nemo` → NemoAction; `thunar` → ThunarAction; `caja` → CajaScript. Thiếu chương trình: install `NotApplicable` `"không có <Tên> trên máy"`, không ghi file; inspect `NotApplicable` nếu chưa có file; uninstall luôn chạy.
- Dolphin: `MimeType=image/*;` → `MimeType=all/allfiles;` (Morphix xử lý cả âm thanh, video, văn bản, PDF, file nén).

- [ ] **Step 1: Test.**
  - `only_present_file_managers_get_integration`: PATH giả chỉ có `nemo` → `NemoAction` `Installed`; `DolphinServiceMenu`, `CajaScript`, `NautilusScript` `NotApplicable` và file của chúng không tồn tại.
  - `nemo_and_thunar_bodies_quote_paths_with_spaces`: morphix ở `/tmp/a b/Tài liệu/morphix` → dòng `Exec=` của `nemo_action_body` bằng `format!("Exec={} menu --uris %U", escape_desktop_value(&desktop_exec_argument(path)))` và chứa `"/tmp/a b/Tài liệu/morphix"` trong ngoặc kép.
  - `dolphin_menu_accepts_all_files`: body chứa `MimeType=all/allfiles;`.
- [ ] **Step 2:** Run các test trên — Expected: FAIL.
- [ ] **Step 3:** Implement; thêm `NemoAction`, `CajaScript`, `ThunarAction` vào `ALL_COMPONENTS`, hai cái đầu vào `INSTALLABLE_COMPONENTS` (Thunar thêm ở Task 5). Windows backend trả `NotApplicable` cho ba ID mới.
- [ ] **Step 4:** Run bộ lệnh của Global Constraints — Expected: PASS.
- [ ] **Step 5:** Commit `feat(morphix-cli): Nemo and Caja menus, only for file managers on PATH`.

### Task 5: Thunar custom action trong `uca.xml`

**Files:**
- Create: `crates/morphix-cli/src/integration/thunar.rs`
- Modify: `crates/morphix-cli/src/integration/{linux.rs,mod.rs}`
- Test: mod tests trong `thunar.rs`, `crates/morphix-cli/tests/integration.rs`

**Interfaces:**
- Produces: `pub const THUNAR_UNIQUE_ID: &str = "morphix-menu";`, `pub fn thunar_action_block(morphix: &Path) -> Result<String, String>`, `pub fn upsert_action(existing: Option<&str>, block: &str) -> Result<String, String>`, `pub fn remove_action(existing: &str) -> Option<String>` (`None` khi không có khối Morphix).
- Vị trí: `$XDG_CONFIG_HOME/Thunar/uca.xml`. Chưa có file → tạo `<?xml version="1.0" encoding="UTF-8"?>\n<actions>\n{block}</actions>\n`.
- Khối: `<action>` gồm `<icon>morphix</icon>`, `<name>Morphix</name>`, `<submenu></submenu>`, `<unique-id>morphix-menu</unique-id>`, `<command>` = XML-escape của `posix_single_quote(morphix) + " menu -- %F"`, `<description>Mở Morphix cho các file đã chọn</description>`, `<range>*</range>`, `<patterns>*</patterns>`, `<audio-files/><image-files/><other-files/><text-files/><video-files/>`, `</action>`.
- WHY nhận diện bằng `unique-id` chứ không bằng comment marker: Thunar ghi lại `uca.xml` khi người dùng sửa action và làm mất comment. Khối Morphix = đoạn `<action>…</action>` chứa `<unique-id>morphix-menu</unique-id>`. `upsert_action` thay khối cũ hoặc chèn trước `</actions>` cuối; phần còn lại giữ nguyên từng byte. Không có `</actions>` → `Err("uca.xml không đúng định dạng; không sửa")`, backend báo `Failed`.
- Ghi qua file tạm + rename (dùng `Transaction` hiện có trong `linux.rs` nếu hợp).

- [ ] **Step 1: Test.**
  - `thunar_install_and_uninstall_preserve_other_actions`: `uca.xml` có sẵn một action người dùng → sau upsert có 2 action; `remove_action` trả về đúng chuỗi ban đầu.
  - `upsert_replaces_existing_morphix_block`: upsert hai lần với hai đường dẫn khác nhau → chỉ một khối `morphix-menu`, chứa đường dẫn mới.
  - `command_escapes_xml_and_shell`: morphix ở `/tmp/a&b's/morphix` → `<command>` là `'/tmp/a&amp;b'\''s/morphix' menu -- %F`.
  - `malformed_uca_is_not_touched`: nội dung không có `</actions>` → install `Failed`, file không đổi.
- [ ] **Step 2:** Run `cargo test -p morphix-cli thunar` — Expected: FAIL.
- [ ] **Step 3:** Implement, nối vào backend (`ThunarAction`, cần `thunar` trên PATH), thêm vào `INSTALLABLE_COMPONENTS`.
- [ ] **Step 4:** Run bộ lệnh của Global Constraints — Expected: PASS.
- [ ] **Step 5:** Commit `feat(morphix-cli): Thunar custom action without touching other actions`.

### Task 6: Lệnh `morphix uninstall`

**Files:**
- Create: `crates/morphix-cli/src/self_uninstall.rs`
- Modify: `crates/morphix-cli/src/{main.rs,lib.rs}`
- Test: mod tests trong `self_uninstall.rs`

**Interfaces:**
- Consumes: `integration::{install_mode, InstallMode, uninstall}` (Task 3).
- Produces: `pub const INSTALL_MARKER: &str = ".morphix-install";` (file trong `<prefix>/lib/morphix/`, do `install.sh` ghi, nội dung là phiên bản); `pub fn remove_user_install(prefix: &Path) -> Result<Vec<PathBuf>, String>`: kiểm marker **trước**, rồi xoá theo thứ tự `bin/morphix-gui`, `lib/morphix/`, `share/icons/hicolor/128x128/apps/morphix.png`, `share/doc/morphix/`, cuối cùng `bin/morphix`; mục không tồn tại thì bỏ qua; trả danh sách đã xoá.
- `Command::Uninstall` (clap `uninstall`, help `"Gỡ Morphix đã cài bằng lệnh curl; giữ dữ liệu người dùng"`), prefix = cha của thư mục chứa `current_exe()`:
  - Package: chạy `integration::uninstall`, in `"Morphix được cài bằng gói .deb: chạy \`sudo apt remove morphix\` để gỡ chương trình."`, exit theo report.
  - User: `integration::uninstall`; `pkill -x morphix-gui` (không có tiến trình thì bỏ qua; lỗi khác in stderr); `remove_user_install`; in danh sách đã xoá và `"Đã giữ dữ liệu tại <data_dir>"`. Lỗi → exit 1 kèm lý do.

- [ ] **Step 1: Test.**
  - `removes_owned_files_and_keeps_others`: prefix tạm có `bin/morphix`, `bin/morphix-gui`, `bin/other-tool`, `lib/morphix/.morphix-install`, `lib/morphix/ffmpeg`, icon → sau gọi chỉ còn `bin/other-tool`.
  - `refuses_to_delete_lib_dir_without_install_marker`: thiếu marker → `Err` chứa `".morphix-install"`, không file nào bị xoá.
- [ ] **Step 2:** Run `cargo test -p morphix-cli self_uninstall` — Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run bộ lệnh của Global Constraints — Expected: PASS.
- [ ] **Step 5:** Commit `feat(morphix-cli): uninstall command for home installs`.

### Task 7: Build tarball

**Files:**
- Create: `scripts/build-tarball.sh`, `packaging/ffmpeg-static.txt` (URL, sha256, trang nguồn của bản ffmpeg static)
- Modify: `packaging/LICENSES-THIRD-PARTY.md` (mục ffmpeg static: GPL, nguồn BtbN/FFmpeg-Builds)

**Interfaces:**
- Produces: `scripts/build-tarball.sh <out-dir>` tạo `<out>/morphix-x86_64-linux.tar.gz` và `<out>/morphix-x86_64-linux.tar.gz.sha256` (định dạng `sha256sum`, tên file không kèm đường dẫn). Gói có thư mục gốc `morphix/`:
  - `bin/morphix`, `bin/morphix-gui` (release, `--features morphix-gui/custom-protocol`, `patchelf --set-rpath '$ORIGIN/../lib/morphix/pdfium'`, chạy `check-gui-embeds-ui.sh`)
  - `lib/morphix/ffmpeg` (static), `lib/morphix/pdfium/libpdfium.so` (kiểm sha256 như `build-deb.sh`)
  - `share/icons/hicolor/128x128/apps/morphix.png` (cùng nguồn icon `.deb`)
  - `share/doc/morphix/`: `LICENSE`, `LICENSES-THIRD-PARTY.md`, giấy phép PDFium, giấy phép và lời đề nghị nguồn ffmpeg
  - `VERSION` (phiên bản workspace)
- Glibc floor (đo như `build-deb.sh`) phải ≤ 2.35, ngược lại script thất bại; `MORPHIX_ALLOW_GLIBC_FLOOR=<x.y>` nới ngưỡng chỉ để thử cục bộ.
- ffmpeg: ghim một release cụ thể của BtbN/FFmpeg-Builds, biến thể `linux64-gpl` (static, có libx264); script tải, kiểm sha256, chỉ lấy `bin/ffmpeg`.

- [ ] **Step 1:** Viết script; chạy cục bộ `MORPHIX_ALLOW_GLIBC_FLOOR=2.39 scripts/build-tarball.sh ~/Documents/morphix-tarball`.
- [ ] **Step 2: Kiểm.** `tar -tzf` đúng các đường dẫn trên; `sha256sum -c` đạt; giải nén vào thư mục tạm rồi `<tmp>/morphix/bin/morphix doctor` in `ffmpeg: <tmp>/morphix/lib/morphix/ffmpeg`; `convert --to txt` trên PDF mẫu và `--to mp4` trên WebM mẫu đều `ok`; `file <tmp>/morphix/lib/morphix/ffmpeg` báo `statically linked`; `ffmpeg -buildconf` chứa `--enable-libx264`.
- [ ] **Step 3:** Commit `build(morphix): portable tarball for home installs`.

### Task 8: `install.sh`

**Files:**
- Create: `scripts/install.sh`, `scripts/tests/test_install_sh.py`

**Interfaces:**
- Consumes: tarball + sha256 (Task 7); `morphix install-integration`; `INSTALL_MARKER` (Task 6).
- Biến môi trường (mặc định): `MORPHIX_VERSION` (trống = latest; `v0.2.0` thì dùng `…/releases/download/v0.2.0`), `MORPHIX_RELEASE_BASE` (`https://github.com/NeuroDev204/morphix/releases/latest/download`; test trỏ `file://`), `MORPHIX_PREFIX` (`$HOME/.local`), `MORPHIX_OS_RELEASE` (`/etc/os-release`), `MORPHIX_SKIP_INTEGRATION` (trống; test đặt `1` để không gọi gsettings thật).
- Thứ tự (dừng ở lỗi đầu tiên, exit ≠ 0; không đụng prefix trước bước 6):
  1. `uname -s` = `Linux` và `uname -m` = `x86_64`; ngược lại `"Morphix hiện chỉ hỗ trợ Linux x86_64 (máy này: …)"`.
  2. `getconf GNU_LIBC_VERSION` ≥ 2.35; ngược lại `"Cần glibc 2.35 trở lên (máy này: …)"`.
  3. `dpkg-query -W -f='${Status}' morphix` chứa `install ok installed` → `"Máy đã cài Morphix bằng gói .deb. Hãy gỡ trước: sudo apt remove morphix"`.
  4. `ldconfig -p` phải có `libwebkit2gtk-4.1.so.0` và `libayatana-appindicator3.so.1`. Thiếu → in thư viện thiếu và lệnh theo `ID`/`ID_LIKE` của os-release: debian/ubuntu `sudo apt install libwebkit2gtk-4.1-0 libayatana-appindicator3-1`; fedora `sudo dnf install webkit2gtk4.1 libayatana-appindicator-gtk3`; arch `sudo pacman -S webkit2gtk-4.1 libayatana-appindicator`; opensuse/suse `sudo zypper install libwebkit2gtk-4_1-0 libayatana-appindicator3-1`; distro khác: chỉ in tên thư viện.
  5. Tải tarball + `.sha256` (`curl -fsSL`) vào `mktemp -d`; `sha256sum -c`; giải nén ở đó.
  6. `pgrep -x morphix-gui` có → nhớ lại, `pkill -x morphix-gui`, chờ tối đa 5 giây.
  7. Thay bản cũ: `lib/morphix` mới → `<prefix>/lib/morphix.new`, cũ → `.old`, `.new` → `morphix`, xoá `.old`; binary và icon `mv -f` từng file; `share/doc/morphix` thay cả thư mục; ghi `<prefix>/lib/morphix/.morphix-install` = nội dung `VERSION`. (`# ponytail:` mỗi bước là một rename nên cửa sổ hỏng rất nhỏ; làm giao dịch đầy đủ nếu có báo lỗi thực tế.)
  8. Trừ khi `MORPHIX_SKIP_INTEGRATION=1`: chạy `<prefix>/bin/morphix install-integration`, in nguyên output (gồm các mục `not_applicable`, ví dụ Alt+C trên Wayland).
  9. GUI đang chạy ở bước 6 → bật lại `nohup <prefix>/bin/morphix-gui >/dev/null 2>&1 &`.
  10. `<prefix>/bin` không có trong `PATH` → in `echo 'export PATH="$HOME/.local/bin:$PATH"' >> ~/.profile`. Dòng kết: `"Đã cài Morphix <ver>. Gỡ: morphix uninstall"`.
- Test harness: Python unittest dựng release giả (tarball có `bin/morphix` là script sh ghi argv, `bin/morphix-gui`, `VERSION`, `lib/morphix/ffmpeg` giả) trong thư mục tạm; `PATH` giả có stub `uname`, `getconf`, `ldconfig`, `dpkg-query`, `pgrep`, `pkill`, `curl` (chép từ `file://`), mỗi stub ghi lời gọi vào một log chung theo thứ tự.

- [ ] **Step 1: Test** (`scripts/tests/test_install_sh.py`):
  - `test_fresh_install_layout`: prefix có `bin/morphix`, `bin/morphix-gui`, `lib/morphix/ffmpeg`, `lib/morphix/.morphix-install` chứa phiên bản; exit 0; stdout chứa `Đã cài Morphix`.
  - `test_checksum_mismatch_keeps_previous_install`: prefix có bản cũ (marker `0.1.0`), `.sha256` sai → exit ≠ 0, marker vẫn `0.1.0`, `bin/morphix` cũ nguyên byte.
  - `test_missing_webkit_prints_distro_command`: os-release `ID=fedora`, `ldconfig` thiếu webkit → exit ≠ 0, stdout chứa `sudo dnf install webkit2gtk4.1`, prefix trống.
  - `test_deb_install_blocks`: `dpkg-query` trả `install ok installed` → stdout chứa `sudo apt remove morphix`, prefix trống.
  - `test_old_glibc_and_arm_rejected`: `getconf` → `glibc 2.31`; lần khác `uname -m` → `aarch64`; cả hai exit ≠ 0, prefix trống.
  - `test_update_restarts_running_gui`: `pgrep` báo đang chạy → trong log, `pkill -x morphix-gui` đứng trước lần thay `bin/morphix`, và có lần khởi động `morphix-gui` sau đó.
  - `test_path_hint`: PATH không chứa `<prefix>/bin` → stdout chứa `export PATH=`.
- [ ] **Step 2:** Run `python3 -m unittest scripts.tests.test_install_sh -v` (tại root Morphix) — Expected: FAIL.
- [ ] **Step 3:** Implement `install.sh` (POSIX sh, `set -eu`); chạy `shellcheck -s sh scripts/install.sh` nếu có.
- [ ] **Step 4:** Run test — Expected: PASS. Thử trên máy dev: `MORPHIX_RELEASE_BASE=file://~/Documents/morphix-tarball MORPHIX_PREFIX=$(mktemp -d) MORPHIX_SKIP_INTEGRATION=1 sh scripts/install.sh` — Expected: dừng ở bước 3 (máy dev đang có `.deb`), prefix trống.
- [ ] **Step 5:** Commit `feat(morphix): one-line curl installer for home installs`.

### Task 9: GitHub Actions

**Files:**
- Create: `.github/workflows/ci.yml`, `.github/workflows/release.yml` (đặt ở root repo công khai — xem Task 10; trong worktree hiện tại để ở `morphix/.github/`, `git subtree split` sẽ đưa lên root)

**Interfaces:**
- Consumes: `scripts/build-tarball.sh`, `scripts/build-deb.sh`, `scripts/check-deb.sh`, `scripts/install.sh`.
- `ci.yml` (push, pull_request): ubuntu-24.04; cài `libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev patchelf`; tải PDFium `chromium/7881` `pdfium-linux-x64.tgz` (sha256 `1470e21b8b4a3b4ad7f85684e2da11d94f3b69a86d81dee11b9b6709d927ac1d`) vào `third_party/pdfium`; chạy `cargo clippy … -D warnings`, `cargo test --workspace`, `npm ci && npm run check && npm test` trong `crates/morphix-gui/ui`, `python3 -m unittest discover -s scripts/tests`, `python3 -m unittest discover -s integration/tests`. Không chạy `cargo fmt --check` toàn workspace cho tới khi hết drift đã biết (CLAUDE.md).
- `release.yml` (push tag `v*`): job `test` (như ci); job `tarball` với `container: ubuntu:22.04` (cài Rust stable, Node 20, deps build, PDFium) chạy `scripts/build-tarball.sh dist`; job `deb` trên ubuntu-24.04 chạy `scripts/build-deb.sh dist` rồi `scripts/check-deb.sh dist/*.deb`; job `publish` (`permissions: contents: write`) tải artifact và `gh release create "$GITHUB_REF_NAME" dist/* scripts/install.sh --title "Morphix $GITHUB_REF_NAME" --generate-notes`.

- [ ] **Step 1:** Viết hai workflow; kiểm bằng `actionlint` nếu có, không thì `python3 -c "import sys,yaml;[yaml.safe_load(open(f)) for f in sys.argv[1:]]" .github/workflows/*.yml`.
- [ ] **Step 2:** Commit `ci(morphix): build and publish tarball and deb on tags`.

### Task 10: Repo công khai và bản phát hành đầu tiên

**Phải hỏi chủ dự án ngay trước Step 3** — đẩy code công khai không rút lại được.

- [ ] **Step 1:** Tại `<worktree>`: `git subtree split --prefix=morphix -b morphix-public`. Kiểm lịch sử chỉ có file Morphix; quét bí mật `git log -p morphix-public | grep -nEi "api[_-]?key|secret|token|password|BEGIN .*PRIVATE"` — Expected: không có kết quả thật; không có `third_party/` hay file > 5 MB trong lịch sử.
- [ ] **Step 2:** Trên `morphix-public`, thêm `README.md` (tiếng Việt: lệnh curl, distro hỗ trợ, Wayland chỉ có menu chuột phải, gỡ bằng `morphix uninstall`, giấy phép GPL-3.0-or-later). Commit `docs(morphix): README for public release`.
- [ ] **Step 3 (sau khi chủ dự án đồng ý):** `gh repo create NeuroDev204/morphix --public` và `git push git@github.com:NeuroDev204/morphix.git morphix-public:main`.
- [ ] **Step 4:** Đổi `version` workspace thành `0.2.0` (commit `chore(morphix): release 0.2.0` trên cả `morphix-public` và nhánh gốc), gắn tag `v0.2.0`, push tag; theo dõi workflow tới khi Release có đủ `morphix-x86_64-linux.tar.gz`, `.sha256`, `install.sh`, `morphix_0.2.0-3_amd64.deb`.
- [ ] **Step 5:** Ghi kết quả vào `docs/superpowers/plans/notes/2026-10-24-curl-install-release.md`, commit.

### Task 11: Nghiệm thu trên nhiều distro

**Files:**
- Create: `docs/superpowers/plans/notes/2026-10-24-curl-install-acceptance.md`

- [ ] **Step 1: Container (không giao diện).** Với `ubuntu:22.04`, `ubuntu:24.04`, `debian:12`, `fedora:latest`, `archlinux:latest`, `opensuse/tumbleweed`: cài `curl` và webkit/appindicator bằng đúng lệnh `install.sh` in ra, tạo user thường, chạy lệnh curl thật với `MORPHIX_SKIP_INTEGRATION=1`, rồi `morphix doctor`, `morphix convert --to png` (JPG mẫu), `--to mp4` (WebM mẫu), `--to txt` (PDF mẫu), `morphix uninstall`. Expected: đều `ok`; sau gỡ prefix không còn file Morphix, dữ liệu người dùng còn. Thêm một lần trên `fedora:latest` **chưa** cài webkit để thấy đúng lệnh `dnf`.
- [ ] **Step 2: Desktop thật.** Ít nhất: máy dev Ubuntu 24.04 X11 (sau khi gỡ `.deb`) — menu Nautilus, Alt+C, launcher, autostart; một phiên Wayland — menu Nautilus có, Alt+C `not_applicable`; một máy/VM KDE, Cinnamon hoặc XFCE — menu Dolphin/Nemo/Thunar. Môi trường không thử được thì ghi **chưa kiểm**; test container không thay cho nghiệm thu desktop.
- [ ] **Step 3:** Commit `docs(morphix): curl install acceptance results`.
