# Morphix Alt+C (Khay với file đang chọn) Implementation Plan

> Phạm vi thực hiện ngày 2026-10-20: **chỉ X11**, theo yêu cầu mới của người dùng. Bỏ Task 1 và Task 10 (Wayland). Tiến độ và kết quả: [ghi chép triển khai](notes/2026-10-20-alt-c-x11.md).

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Nhấn Alt+C trong Files (Nautilus) hoặc trên desktop Ubuntu thì Khay mở ra với đúng các file đang chọn và các nút chuyển đổi cho chúng, trên X11 và Wayland.

**Architecture:** `morphix tray --selection` chỉ chuyển yêu cầu cho `morphix-gui --capture-selection`; toàn bộ việc lấy file chạy trong tiến trình GUI sống lâu (cần để trả lại clipboard). Nautilus: GUI "mở chờ" 2 giây rồi bấm hộ phím tắt riêng của script Morphix, script gọi lại `morphix menu` qua IPC có sẵn, và lô đó được đưa vào Khay thay vì vòng tròn. Desktop: lưu clipboard, bấm hộ Ctrl+C, chờ chủ sở hữu clipboard đổi, đọc `x-special/gnome-copied-files`, trả clipboard. Khay dùng lại `menu_payload` và `run_action` có sẵn.

**Tech Stack:** Rust (`morphix-gui`, `morphix-cli`), `x11rb` (XTest, thuần Rust, thêm mới), `gtk` 0.18 (đã có trong lock, thêm làm phụ thuộc trực tiếp cho Linux), `url`, TypeScript/vitest cho Khay, `gsettings` cho phím tắt GNOME.

**Spec:** `docs/superpowers/specs/2026-10-20-morphix-alt-c-design.md` (người thực hiện đọc cả mục 4 và 9).

## Global Constraints

- Không sudo; offline; không ghi đè file gốc (spec mục 3, spec gốc mục 2).
- Chỉ nhắm Ubuntu 24.04 (Nautilus 46): bản Nautilus < 46 thì tính năng tự tắt, Alt+C mở Khay trống kèm "chưa hỗ trợ trên phiên bản này" (spec 9.1).
- `morphix tray` giữ nguyên là chỉ mở Khay; lệnh mới là `morphix tray --selection` (spec 9.2).
- Phím tắt riêng của Nautilus là `<Control><Shift>F7`; khoảng nghỉ giữa các sự kiện phím bấm hộ là 30 ms; hạn chờ thả phím bổ trợ 3 giây; hạn chờ file ở Files 2 giây; hạn chờ clipboard đổi 1 giây (spec 4.2).
- Clipboard không lưu trả lại trọn vẹn được (đọc một định dạng lỗi, tổng quá 10 MiB, có định dạng `x-kde-passwordManagerHint`) thì **không bấm Ctrl+C**, Khay mở trống kèm lý do (spec 9.3).
- Phím tắt GNOME nằm tại `/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/morphix/`, binding `<Alt>c`, name `Morphix: mở Khay`; chỉ thay phím tắt có command đúng `<morphix> tray`, command khác thì báo `Conflict` (spec 6, 9.2).
- Chỉ sửa dòng và phần có dấu của Morphix; không đụng cấu hình của người dùng (spec 2 phản mục tiêu).
- Lỗi phải hiện cho người dùng trong Khay (banner đỏ) và ghi log; không nuốt lỗi.
- Chuỗi giao diện tiếng Việt nằm trong `ui/src/strings.ts`; chú thích mã theo kiểu `WHY …` tiếng Việt; commit Conventional Commits có scope crate.
- Lệnh kiểm chuẩn từ `<repo>/morphix`: `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, `cargo check --target x86_64-pc-windows-gnu -p morphix-cli -p morphix-gui`; trong `crates/morphix-gui/ui`: `npm run check && npm test`. Mã Linux-only phải nằm sau `cfg(target_os = "linux")` để bản Windows vẫn biên dịch.
- Thư viện Linux-only khai báo trong `[target.'cfg(target_os = "linux")'.dependencies]`. `Cargo.lock` do cargo cập nhật, không sửa tay.

## Review Focus

- Clipboard của người dùng đang chứa ảnh, hoặc đang rỗng, khi Alt+C trên desktop: sau đó phải **y nguyên** (ảnh còn đó / vẫn rỗng). Test ở Task 5.
- Desktop không chọn file nào nhưng clipboard đang giữ file từ lần copy trước: không được coi file cũ là vừa chọn (phải dựa vào việc chủ sở hữu clipboard đổi, không dựa vào nội dung). Test ở Task 5.
- Tên thư mục tiếng Việt có dấu và khoảng trắng (`Tiếng Anh`) trong URI `file://…%20…`: giải mã đúng. Test ở Task 5.
- Nhấn Alt+C lần hai khi lần một chưa xong, hoặc giữ Alt hơn 3 giây: lần sau bị bỏ qua, hoặc báo lỗi, không bấm phím giữa lúc người dùng còn giữ Alt. Test ở Task 4 và Task 6.
- Alt+C khi cửa sổ đang chọn là chính Morphix (Khay) hoặc trình duyệt: mở Khay trống, không bấm phím nào. Test ở Task 4 và Task 6.

---

## File Structure

- Create `crates/morphix-gui/src/capture/mod.rs`: điều phối, `Target`, `CaptureError`, `KeyInjector` trait, `ClipboardAccess` trait.
- Create `crates/morphix-gui/src/capture/x11.rs`: cửa sổ đang chọn, phím bổ trợ đang giữ, bấm hộ bằng XTest.
- Create `crates/morphix-gui/src/capture/clipboard.rs`: lưu/trả/đọc clipboard bằng GTK, phân tích `x-special/gnome-copied-files`.
- Create `crates/morphix-gui/ui/src/selection.ts` + `ui/tests/selection.test.ts`: mô hình hiển thị Khay có file.
- Modify `crates/morphix-gui/src/{lib.rs,state.rs,windows.rs,commands.rs}`, `crates/morphix-gui/ui/src/{khay.ts,strings.ts,styles.css}`, `crates/morphix-cli/src/main.rs`, `crates/morphix-cli/src/integration/{report.rs,linux.rs,mod.rs}`.
- Create `docs/superpowers/spikes/2026-10-20-wayland-selection.md` (kết quả Task 1).

---

### Task 1: Thử trên Wayland (báo cáo, không đổi mã sản phẩm)

Có thể làm song song với Task 2–9; Task 10 phụ thuộc báo cáo này.

**Files:**
- Create: `docs/superpowers/spikes/2026-10-20-wayland-selection.md`
- Scratch: mã thử đặt trong thư mục nháp của phiên, không commit.

- [ ] **Step 1: Người dùng đăng xuất, vào phiên "Ubuntu" (Wayland), mở terminal.** Xác nhận `echo $XDG_SESSION_TYPE` ra `wayland`. Đây là bước của người, ghi lại trong báo cáo.
- [ ] **Step 2: Thử và ghi kết quả có/không cho từng mục**, mỗi mục kèm lệnh và đầu ra: (a) tạo phiên `org.freedesktop.portal.RemoteDesktop` chỉ có bàn phím với `persist_mode=2`, lấy `restore_token`, và lần chạy thứ hai không hỏi lại; (b) `NotifyKeyboardKeysym` bấm `<Control><Shift>F7` khi Files đang được chọn khiến Nautilus chạy script (dùng script thử ghi log như bản thử đã làm); (c) bấm Ctrl+C khi desktop đang được chọn rồi đọc `x-special/gnome-copied-files` bằng một cửa sổ GTK vừa được chọn, và bằng cổng Clipboard của phiên RemoteDesktop; (d) AT-SPI báo khung ACTIVE đúng cho Files và `Desktop Icons`.
- [ ] **Step 3: Viết kết luận** ở đầu báo cáo, một trong ba: `GO` (làm Task 10 như spec), `GO một phần` (nêu phần nào, ví dụ chỉ Files), `NO-GO` (nêu lý do). Với `NO-GO` hoặc `GO một phần`, dừng và báo người dùng trước khi làm Task 10.
- [ ] **Step 4: Commit** `docs(morphix): wayland selection capture spike results`.

---

### Task 2: Trạng thái "chờ file cho Khay" và định tuyến lô

**Files:**
- Modify: `crates/morphix-gui/src/state.rs` (thêm vào `AppState`), `crates/morphix-gui/src/lib.rs:show_menu_for`
- Test: trong `state.rs` (`#[cfg(test)]`)

**Interfaces:**
- Produces: `pub struct KhayWait { pub deadline: Instant }`; `AppState.khay_wait: Mutex<Option<KhayWait>>`; `pub fn route_for_batch(wait: &mut Option<KhayWait>, now: Instant) -> BatchRoute` với `pub enum BatchRoute { Khay, Wheel }` (hết hạn thì xoá `wait` và trả `Wheel`; còn hạn thì xoá `wait` và trả `Khay`); `AppState::khay_wait_guard(&self) -> MutexGuard<'_, Option<KhayWait>>`.

- [ ] **Step 1: Viết test** `routes_batch_to_khay_only_while_wait_is_armed`, `expired_wait_routes_to_wheel_and_is_cleared`, `no_wait_routes_to_wheel`, với `Instant` dựng bằng `Instant::now()` cộng/trừ `Duration` (không gọi sleep).
- [ ] **Step 2: Chạy `cargo test -p morphix-gui route_for_batch` thấy FAIL** (chưa định nghĩa).
- [ ] **Step 3: Thêm kiểu và hàm theo chữ ký trên**; `show_menu_for` đọc `route_for_batch` trước `windows::open_menu`: `Khay` thì gọi `windows::open_khay_with_files(app, paths)` (Task 3), `Wheel` thì giữ đường cũ. `collecting` (lô đang gom) vẫn chỉ áp dụng cho vòng tròn.
- [ ] **Step 4: Chạy lại, thấy PASS**, kèm `cargo test -p morphix-gui`.
- [ ] **Step 5: Commit** `feat(morphix-gui): route a batch to the Khay while a selection capture is pending`.

---

### Task 3: Khay nhận file: trạng thái, lệnh, cửa sổ

**Files:**
- Modify: `crates/morphix-gui/src/state.rs`, `crates/morphix-gui/src/windows.rs`, `crates/morphix-gui/src/commands.rs`, `crates/morphix-gui/src/lib.rs` (đăng ký lệnh)
- Test: `state.rs`, `commands.rs`

**Interfaces:**
- Produces: `pub struct KhaySelection { pub paths: Vec<String>, pub error: Option<String>, pub revision: u64 }` (`Serialize`, `Clone`, `Default`); `AppState.khay_selection: Mutex<KhaySelection>` và `khay_selection_guard()`; `pub const KHAY_SELECTION_EVENT: &str = "khay-selection"`; `windows::open_khay_with_files(app: &AppHandle, paths: Vec<PathBuf>) -> Result<(), String>` và `windows::open_khay_with_error(app: &AppHandle, message: String) -> Result<(), String>` (cả hai tăng `revision`, ghi trạng thái, `emit` sự kiện, rồi gọi `open_khay`); lệnh Tauri `khay_selection(state) -> KhaySelection` và `clear_khay_selection(state)` (xoá file và lỗi, tăng revision).
- Consumes: `open_khay` có sẵn; `menu_payload`, `run_action` giữ nguyên (Khay gọi trực tiếp).

- [ ] **Step 1: Viết test** `set_files_bumps_revision_and_clears_error`, `set_error_clears_files`, `clear_resets_both` trên hàm thuần `KhaySelection::with_files(prev, paths)`, `with_error(prev, message)`, `cleared(prev)` (mỗi hàm trả `KhaySelection` mới, `revision = prev.revision + 1`).
- [ ] **Step 2: Chạy `cargo test -p morphix-gui khay_selection` thấy FAIL.**
- [ ] **Step 3: Cài các hàm thuần, hai hàm `windows::*`, hai lệnh Tauri**, đăng ký lệnh trong `generate_handler!`. WHY ghi trạng thái trước khi `emit`: Khay có thể chưa tải xong khi sự kiện tới, nên nó cũng đọc `khay_selection` lúc khởi động (cùng mẫu `pending_request`).
- [ ] **Step 4: Chạy `cargo test -p morphix-gui` và `cargo clippy -p morphix-gui --all-targets -- -D warnings`, PASS.**
- [ ] **Step 5: Commit** `feat(morphix-gui): hold the files and errors handed to the Khay`.

---

### Task 4: Phát hiện cửa sổ đang chọn, phím bổ trợ, bấm hộ (X11)

**Files:**
- Create: `crates/morphix-gui/src/capture/mod.rs`, `crates/morphix-gui/src/capture/x11.rs`
- Modify: `crates/morphix-gui/Cargo.toml` (thêm `x11rb = { version = "<mới nhất 0.13.x>", features = ["xtest"] }` trong target Linux), `crates/morphix-gui/src/lib.rs` (`pub mod capture;` sau `cfg(target_os = "linux")` cho `x11`)
- Test: trong hai file trên

**Interfaces:**
- Produces trong `capture/mod.rs`:
  - `pub enum Target { Files, Desktop, Other }`
  - `pub fn classify_window(wm_class: &[String], title: &str) -> Target` (thuần): `Files` khi có lớp `org.gnome.Nautilus`; `Desktop` khi lớp có `Gjs` và tiêu đề bắt đầu bằng `Desktop Icons`; còn lại `Other` (kể cả `morphix-gui`).
  - `pub trait KeyInjector { fn modifiers_held(&mut self) -> Result<bool, CaptureError>; fn press(&mut self, keys: &[Key]) -> Result<(), CaptureError>; }` với `pub enum Key { Ctrl, Shift, C, F7 }`.
  - `pub enum CaptureError { Unsupported(String), ModifiersStuck, KeysFailed(String), NoFiles, ClipboardUnsafe(String), Timeout(String), Other(String) }` với `impl Display` cho thông báo tiếng Việt hiện trong Khay.
  - `pub fn wait_modifiers_released(injector: &mut dyn KeyInjector, sleep: &mut dyn FnMut(Duration), max: Duration) -> Result<(), CaptureError>` (hỏi mỗi 20 ms; quá `max` trả `ModifiersStuck`).
  - `pub fn press_with_gaps(injector: &mut dyn KeyInjector, chord: &[Key], sleep: &mut dyn FnMut(Duration)) -> Result<(), CaptureError>` (nhấn từng phím trong chord, nghỉ 30 ms sau mỗi sự kiện, thả theo thứ tự ngược).
- Produces trong `x11.rs`: `pub struct X11Session` implement `KeyInjector` (XTest, keycode tra bằng `get_keyboard_mapping`) và `pub fn active_window_target(&mut self) -> Result<Target, CaptureError>` (đọc `_NET_ACTIVE_WINDOW`, `WM_CLASS`, `_NET_WM_NAME`).

- [ ] **Step 1: Viết test** `classifies_nautilus_gjs_desktop_and_morphix`, `gjs_without_desktop_icons_title_is_other`, `wait_modifiers_returns_ok_once_released` (injector giả trả `true,true,false`), `wait_modifiers_gives_up_after_max` (luôn `true`, `sleep` giả cộng thời gian ảo, kỳ vọng `ModifiersStuck`), `press_with_gaps_orders_and_pauses` (injector giả ghi lại nhật ký, kỳ vọng thứ tự nhấn Ctrl,Shift,F7 rồi thả F7,Shift,Ctrl và 30 ms nghỉ sau mỗi sự kiện).
- [ ] **Step 2: Chạy `cargo test -p morphix-gui capture::` thấy FAIL.**
- [ ] **Step 3: Cài các mục trên.** `X11Session` thật không có test tự động (cần màn hình); kiểm bằng Step 4.
- [ ] **Step 4: Kiểm tay trên X11:** chạy một binary nháp hoặc test `#[ignore]` gọi `X11Session::active_window_target` khi Files đang chọn và in `Files`; xác nhận bấm hộ `Ctrl+Shift+F7` làm Nautilus gọi script thử. Ghi kết quả vào mô tả commit.
- [ ] **Step 5: Chạy `cargo clippy --workspace --all-targets -- -D warnings` và `cargo check --target x86_64-pc-windows-gnu -p morphix-gui` (mã Linux-only không được làm hỏng bản Windows), rồi commit** `feat(morphix-gui): detect the focused window and inject keys on X11`.

---

### Task 5: Clipboard: lưu, trả, đọc danh sách file

**Files:**
- Create: `crates/morphix-gui/src/capture/clipboard.rs`
- Modify: `crates/morphix-gui/Cargo.toml` (`gtk = "0.18"`, `url = "<bản đang có trong lock>"` trong target Linux)
- Test: trong `clipboard.rs`

**Interfaces:**
- Produces:
  - `pub struct Snapshot { pub entries: Vec<(String, Vec<u8>)> }` (tên định dạng và dữ liệu; rỗng nghĩa là clipboard đang rỗng).
  - `pub const MAX_SNAPSHOT_BYTES: usize = 10 * 1024 * 1024;` và `pub const SENSITIVE_TARGET: &str = "x-kde-passwordManagerHint";`
  - `pub trait ClipboardAccess { fn targets(&mut self) -> Result<Vec<String>, CaptureError>; fn read(&mut self, target: &str) -> Result<Vec<u8>, CaptureError>; fn write_all(&mut self, entries: &[(String, Vec<u8>)]) -> Result<(), CaptureError>; fn clear(&mut self) -> Result<(), CaptureError>; fn owner_changes(&mut self) -> u64; }` (`owner_changes` là bộ đếm tăng mỗi lần chủ sở hữu đổi; bản thật dựa trên tín hiệu `owner-change` của `gtk::Clipboard`).
  - `pub fn take_snapshot(cb: &mut dyn ClipboardAccess) -> Result<Snapshot, CaptureError>` (bỏ `TARGETS`, `TIMESTAMP`, `MULTIPLE`, `SAVE_TARGETS`; có `SENSITIVE_TARGET`, tổng quá `MAX_SNAPSHOT_BYTES`, hoặc một định dạng đọc lỗi thì `Err(ClipboardUnsafe(lý do))`).
  - `pub fn restore(cb: &mut dyn ClipboardAccess, snap: &Snapshot) -> Result<(), CaptureError>` (rỗng thì `clear`, ngược lại `write_all`).
  - `pub fn parse_copied_files(data: &[u8]) -> Result<Vec<PathBuf>, CaptureError>` (dạng `copy\nfile:///…`; hành động `cut` cũng nhận; dòng không phải `file://` hoặc rỗng thì lỗi có số dòng; giải mã phần trăm UTF-8).
  - `pub fn capture_selection_via_clipboard(cb: &mut dyn ClipboardAccess, copy: &mut dyn FnMut() -> Result<(), CaptureError>, sleep: &mut dyn FnMut(Duration), wait: Duration) -> Result<Vec<PathBuf>, CaptureError>`: chụp, gọi `copy` (bấm Ctrl+C), chờ `owner_changes` tăng tối đa `wait` (hỏi mỗi 20 ms), không tăng thì `restore` rồi trả `NoFiles`, tăng thì đọc `x-special/gnome-copied-files`, và **luôn `restore` trước khi trả kết quả hay lỗi**.
  - `pub struct GtkClipboard` implement `ClipboardAccess` (chạy trên luồng chính GTK).

- [ ] **Step 1: Viết test bằng `FakeClipboard`** (nhật ký thao tác, chủ sở hữu đổi theo kịch bản): `snapshot_of_empty_clipboard_restores_to_empty`, `image_clipboard_is_restored_byte_for_byte`, `sensitive_hint_refuses_snapshot`, `oversize_clipboard_refuses_snapshot`, `unreadable_target_refuses_snapshot`, `stale_copied_files_without_owner_change_is_no_files` (clipboard sẵn giữ `x-special/gnome-copied-files` cũ, `copy` không làm chủ sở hữu đổi: kỳ vọng `NoFiles`, clipboard còn nguyên), `owner_change_returns_files_then_restores`, `restore_runs_even_when_parse_fails`, `parse_decodes_vietnamese_and_spaces` (`file://~/Documents/Ti%E1%BA%BFng%20Anh/b%C3%A0i.docx` ra `~/Documents/Tiếng Anh/bài.docx`), `parse_rejects_non_file_uri_with_line_number`.
- [ ] **Step 2: Chạy `cargo test -p morphix-gui clipboard` thấy FAIL.**
- [ ] **Step 3: Cài các mục trên.** `GtkClipboard::write_all` dùng `set_with_data` giữ dữ liệu trong bộ nhớ tiến trình GUI để trả lời yêu cầu dán; không gọi `store()`.
- [ ] **Step 4: Chạy lại, PASS; kiểm tay trên desktop X11** với clipboard đang giữ một ảnh: sau thao tác dán ảnh vào ứng dụng khác vẫn ra ảnh đó. Ghi kết quả vào mô tả commit.
- [ ] **Step 5: `cargo clippy --workspace --all-targets -- -D warnings`, rồi commit** `feat(morphix-gui): borrow and restore the clipboard to read the desktop selection`.

---

### Task 6: Điều phối Alt+C và lệnh `tray --selection`

**Files:**
- Modify: `crates/morphix-gui/src/capture/mod.rs` (thêm `run`), `crates/morphix-gui/src/lib.rs` (cờ `--capture-selection` trong `GuiCli`/`StartupRequest`, `handle_second_instance`, khởi động nguội), `crates/morphix-gui/src/state.rs` (cờ `capturing: AtomicBool`), `crates/morphix-cli/src/main.rs` (`Command::Tray { selection: bool }`, `run_tray`)
- Test: `capture/mod.rs`, `lib.rs`, `crates/morphix-cli` (test phân tích tham số)

**Interfaces:**
- Produces: `pub fn run(app: &AppHandle)` (chạy trên luồng riêng, không chặn callback single-instance); `pub fn plan_for(target: Target, nautilus_major: Option<u32>) -> Result<Plan, CaptureError>` thuần, với `pub enum Plan { WaitForNautilusScript, ClipboardCopy, EmptyKhay }` (`Files` mà `nautilus_major < Some(46)` hoặc không đọc được thì `Err(Unsupported("chưa hỗ trợ trên phiên bản này"))`); `StartupRequest { show_khay, capture_selection, paths }`.
- Consumes: `KeyInjector`, `wait_modifiers_released`, `press_with_gaps`, `capture_selection_via_clipboard`, `windows::open_khay`, `open_khay_with_files`, `open_khay_with_error`, `AppState.khay_wait`.

- [ ] **Step 1: Viết test** `plan_files_on_nautilus_46_waits_for_script`, `plan_files_on_nautilus_42_is_unsupported`, `plan_desktop_uses_clipboard`, `plan_other_opens_empty_khay_without_keys`, `startup_parses_capture_selection_flag`, và ở CLI `tray_selection_flag_spawns_gui_with_capture_flag` (kiểm danh sách tham số dựng cho `spawn_gui`, tách thành hàm thuần `tray_gui_args(selection: bool) -> Vec<&'static str>` trả `["--capture-selection"]` hay `["--show-khay"]`).
- [ ] **Step 2: Chạy `cargo test -p morphix-gui plan_ startup_parses` và `cargo test -p morphix-cli tray_` thấy FAIL.**
- [ ] **Step 3: Cài `run`:** (1) nếu `capturing` đang `true` thì bỏ qua (nhấn lần hai); (2) `wait_modifiers_released` tối đa 3 giây, lỗi thì Khay hiện lỗi; (3) `active_window_target` rồi `plan_for`; (4) `WaitForNautilusScript`: đặt `khay_wait` hạn 2 giây, `press_with_gaps([Ctrl, Shift, F7])`, rồi sau 2 giây nếu `khay_wait` vẫn còn thì xoá nó và `open_khay_with_error("Không nhận được file nào…")`; (5) `ClipboardCopy`: chạy `capture_selection_via_clipboard` trên luồng chính GTK (`run_on_main_thread`), `Ok(paths)` thì `open_khay_with_files`, `Err` thì `open_khay_with_error(err.to_string())`; (6) `EmptyKhay` thì `open_khay`. Luôn đặt lại `capturing=false`. WHY không hiện Khay trước khi bấm phím: Khay cướp focus khỏi Files/desktop và phím bấm hộ trượt.
- [ ] **Step 4: Chạy lại, PASS; kiểm tay trên X11** (build `cargo build --release --features morphix-gui/custom-protocol` hoặc `scripts/build-deb.sh`, cài đè, đặt phím tắt bằng tay `/usr/bin/morphix tray --selection`): Files 2 file và 53 file, desktop 2 file, trình duyệt, Khay trống. Ghi từng kết quả.
- [ ] **Step 5: `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, rồi commit** `feat(morphix-gui): Alt+C captures the selection and opens the Khay with it`.

---

### Task 7: Khay hiện file và các nút chuyển đổi

**Files:**
- Create: `crates/morphix-gui/ui/src/selection.ts`, `crates/morphix-gui/ui/tests/selection.test.ts`
- Modify: `crates/morphix-gui/ui/src/khay.ts`, `crates/morphix-gui/ui/src/strings.ts`, `crates/morphix-gui/ui/src/styles.css`

**Interfaces:**
- Produces trong `selection.ts`: `export interface KhaySelection { paths: string[]; error: string | null; revision: number }`; `export type SelectionView = { kind: "empty" } | { kind: "error"; message: string } | { kind: "files"; summary: string; names: string[]; extra: number; items: WheelItem[]; unsupported: string | null }`; `export function composeSelectionView(selection: KhaySelection, payload: WheelPayload | null, payloadError: string | null): SelectionView`; `export const MAX_NAMES_SHOWN = 3`.
- Consumes: kiểu `WheelPayload`, `WheelItem` đã có (xem `main.ts`/`recent.ts`; import từ nơi đang định nghĩa, không khai báo lại).

- [ ] **Step 1: Viết test vitest** `empty_when_no_paths_and_no_error`, `error_wins_and_keeps_empty_drop_zone`, `shows_three_names_and_extra_count` (53 file ra 3 tên và `extra: 50`), `no_items_reports_mixed_or_unsupported_reason` (dùng chuỗi `S.emptyMixed`/`S.emptyUnsupported` đã có), `payload_error_becomes_error_view`.
- [ ] **Step 2: Chạy `npm test -- selection` thấy FAIL.**
- [ ] **Step 3: Cài `composeSelectionView`; trong `khay.ts` đọc `invoke("khay_selection")` lúc khởi động và `listen("khay-selection")`, gọi `invoke("menu_payload", { paths })` khi có file, và vẽ:** vùng kéo thả được thay bằng danh sách tên + "+N file khác" + nút "Bỏ, chọn file khác" (gọi `clear_khay_selection`), bên dưới là các nút lấy từ `items` (bấm thì `invoke("run_action", { id, paths })`, nhóm công cụ xếp sau nhóm chuyển đổi); lỗi hiện ở banner đỏ có sẵn và vùng kéo thả giữ nguyên. Chuỗi mới (`selectionTitle`, `moreFiles`, `discardSelection`, `noFilesReceived`) vào `strings.ts`.
- [ ] **Step 4: Chạy `npm run check && npm test`, PASS.**
- [ ] **Step 5: Commit** `feat(morphix-gui): show the captured files and conversion buttons in the Khay`.

---

### Task 8: `install-integration`: dòng phím tắt của script Nautilus

**Files:**
- Modify: `crates/morphix-cli/src/integration/report.rs` (`ComponentId::NautilusAccel`, `as_str` = `"nautilus_accel"`), `crates/morphix-cli/src/integration/linux.rs`, `crates/morphix-cli/src/integration/mod.rs`
- Test: `linux.rs` (theo mẫu các test Nautilus script hiện có)

**Interfaces:**
- Produces: `pub fn accel_line_for_morphix() -> &'static str` trả `<Control><Shift>F7 Morphix`; vị trí `~/.config/nautilus/scripts-accels` qua `XdgRoots.config_home`; thêm/gỡ **đúng dòng đó** mà giữ nguyên mọi dòng khác và newline cuối; `Conflict` khi `<Control><Shift>F7` đã gán cho script khác; sau khi cài thành công, báo kèm dòng "Chạy `nautilus -q` hoặc đăng xuất để Files nhận phím tắt".
- Consumes: `Transaction`, `publish_file`, `read_existing` trong `files.rs` (ghi file nguyên tử, hoàn tác khi lỗi).

- [ ] **Step 1: Viết test** `accel_installs_into_missing_file`, `accel_appends_and_keeps_user_lines`, `accel_reinstall_is_already_installed`, `accel_key_used_by_other_script_is_conflict`, `accel_uninstall_removes_only_our_line`, `accel_uninstall_deletes_file_it_created_when_empty`, `doctor_reports_missing_and_stale_accel`.
- [ ] **Step 2: Chạy `cargo test -p morphix-cli accel` thấy FAIL.**
- [ ] **Step 3: Cài thành phần** theo cùng cách `NautilusScript`; không cài khi Nautilus < 46 (`NotApplicable` kèm lý do, cùng ngưỡng với `plan_for`).
- [ ] **Step 4: `cargo test -p morphix-cli`, `cargo clippy …`, `cargo check --target x86_64-pc-windows-gnu -p morphix-cli`, PASS.**
- [ ] **Step 5: Commit** `feat(morphix-cli): install the Nautilus accelerator line for Alt+C capture`.

---

### Task 9: `install-integration`: phím tắt GNOME Alt+C

**Files:**
- Modify: `report.rs` (`ComponentId::GnomeShortcut`, `"gnome_shortcut"`), `linux.rs`
- Test: `linux.rs` (inject `ToolRunner` giả như test doctor hiện có)

**Interfaces:**
- Produces: cài qua `gsettings` (không qua shell): đặt `name`, `command` = `<đường dẫn morphix> tray --selection`, `binding` = `<Alt>c` tại đường dẫn `…/custom-keybindings/morphix/`, rồi thêm đường dẫn vào `custom-keybindings` mà không xoá phần tử khác (đọc danh sách, thêm nếu thiếu, ghi lại). Trạng thái: `AlreadyInstalled` khi đủ ba khoá đúng; thay `command` khi nó đúng bằng `<morphix> tray` (phím tắt thủ công cũ); `Conflict` khi `<Alt>c` đang thuộc một phím tắt khác hoặc `command` khác; gỡ chỉ khi `command` bắt đầu bằng `<morphix> tray`, và bỏ đường dẫn khỏi danh sách giữ các phần tử khác.
- Consumes: `ToolRunner`/`ToolOutcome` trong `tools.rs`; `BinaryPaths.morphix`.

- [ ] **Step 1: Viết test** với `ToolRunner` giả ghi lại lệnh: `shortcut_installs_when_absent_and_appends_to_list`, `shortcut_keeps_other_custom_keybindings`, `shortcut_replaces_old_manual_tray_command`, `shortcut_alt_c_owned_by_other_app_is_conflict_and_writes_nothing`, `shortcut_uninstall_removes_only_ours`, `shortcut_gsettings_missing_is_not_applicable`.
- [ ] **Step 2: Chạy `cargo test -p morphix-cli shortcut` thấy FAIL.**
- [ ] **Step 3: Cài thành phần.** WHY đọc danh sách trước khi ghi: `gsettings set` thay cả mảng, ghi mù sẽ xoá phím tắt của người dùng.
- [ ] **Step 4: Kiểm tay trên máy người dùng:** `uninstall-integration` rồi `install-integration` rồi `doctor`; xác nhận các phím tắt khác (nếu có) còn nguyên bằng `gsettings get … custom-keybindings`. Ghi kết quả.
- [ ] **Step 5: `cargo test --workspace`, clippy, check Windows; commit** `feat(morphix-cli): install the Alt+C GNOME shortcut without touching other bindings`.

---

### Task 10: Wayland (chỉ làm khi báo cáo Task 1 là GO)

**Files:**
- Create: `crates/morphix-gui/src/capture/wayland.rs`
- Modify: `crates/morphix-gui/src/capture/mod.rs` (chọn backend theo `XDG_SESSION_TYPE`), `crates/morphix-gui/Cargo.toml` (crate cổng theo báo cáo: `zbus` đã có trong lock hoặc `ashpd` thêm mới, ghi giấy phép)

**Interfaces:**
- Produces: `pub struct WaylandSession` implement `KeyInjector` qua phiên RemoteDesktop chỉ-bàn-phím, `restore_token` lưu tại `data_dir()/portal-token` (chỉ ghi khi cổng trả token; không in token vào log); `pub fn active_window_target_atspi() -> Result<Target, CaptureError>`; từ chối quyền hoặc không có cổng thì `Err(KeysFailed("…cách cấp lại quyền…"))` để Khay mở trống với lời giải thích (spec 4.3).
- Consumes: kết luận trong `docs/superpowers/spikes/2026-10-20-wayland-selection.md` về đường đọc clipboard; `capture_selection_via_clipboard` giữ nguyên.

- [ ] **Step 1: Đọc báo cáo Task 1.** Nếu không phải `GO`, dừng và báo người dùng.
- [ ] **Step 2: Viết test** cho phần thuần: `token_file_roundtrip`, `missing_portal_maps_to_keys_failed_with_guidance`, `denied_permission_maps_to_keys_failed_with_guidance` (kịch bản lỗi giả từ lớp bọc cổng).
- [ ] **Step 3: Chạy `cargo test -p morphix-gui wayland` thấy FAIL.**
- [ ] **Step 4: Cài `WaylandSession` và chọn backend theo `XDG_SESSION_TYPE`** (`wayland` thì Wayland, còn lại X11).
- [ ] **Step 5: Chạy lại, PASS; kiểm tay trên phiên Wayland theo mục 7 của spec** (hỏi quyền lần đầu, lần sau không hỏi, từ chối thì Khay trống kèm hướng dẫn). Ghi kết quả.
- [ ] **Step 6: Commit** `feat(morphix-gui): capture the selection on Wayland through the RemoteDesktop portal`.

---

### Task 11: Đóng gói và nghiệm thu

**Files:**
- Modify: `scripts/build-deb.sh`/`scripts/check-deb.sh` nếu cần (x11rb thuần Rust nên không thêm `libXtst`; xác nhận `ldd` của `morphix-gui` không có thư viện mới ngoài những gì gói đã gom)
- Test: `scripts/check-deb.sh`

- [ ] **Step 1: Chạy `scripts/build-deb.sh`** và `scripts/check-deb.sh` cho gói 24.04; xác nhận `ldd` không thiếu thư viện.
- [ ] **Step 2: Chạy toàn bộ kiểm chuẩn** trong Global Constraints, tất cả xanh.
- [ ] **Step 3: Nghiệm thu trên máy người dùng** theo spec mục 7, cả X11 và (nếu Task 10 đã làm) Wayland: sáu mục, ghi kết quả từng mục.
- [ ] **Step 4: Commit** `chore(morphix): packaging checks for Alt+C selection capture` (chỉ khi có thay đổi).
