# Morphix Windows: menu native thay hình quạt — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Trên Windows, chuột phải → Morphix mở submenu native liệt kê việc áp dụng được; bấm là GUI chạy job. Bỏ mọi hình quạt/vòng tròn trên Windows.

**Architecture:** Danh sách việc và id chuỗi (`convert:webp`...) chuyển vào `morphix-core` để DLL và GUI dùng chung. DLL dựng submenu từ hàm thuần trong file mới không phụ thuộc Windows (test được trên mọi nền tảng); `InvokeCommand` gửi `MenuRequest { paths, action: Some(id) }`; GUI chạy qua cùng đường với lệnh `run_action` của webview. Thêm phần mới trước (task 1–4, 6), xoá phần quạt sau (task 5, 7) để mỗi task giữ build xanh.

**Tech Stack:** Rust 2021, crate `windows` 0.62, Tauri 2 + `tauri-plugin-dialog` 2 (đã có), TypeScript/vitest cho UI.

**Spec:** `docs/superpowers/specs/2026-11-05-morphix-windows-native-menu-design.md`

## Global Constraints

- Linux không đổi hành vi: vòng tròn đầy đủ, `morphix menu`, gom lô IPC giữ nguyên.
- Lỗi phải hiện cho người dùng: DLL dùng `MessageBoxW`, GUI dùng `tauri-plugin-dialog`; không chỉ ghi log.
- Code Windows nằm sau `cfg(windows)` / `[target.'cfg(windows)'.dependencies]`.
- Hàm COM không được panic ra ngoài hay làm sập Explorer: lỗi trả `HRESULT`.
- Giới hạn: quá **100 file** thì submenu chỉ có một mục bị vô hiệu hoá "Chọn quá nhiều file (tối đa 100)". DLL release tăng quá **2 MB** sau khi thêm `morphix-core` thì dừng và hỏi người dùng (phương án dự phòng trong spec: bảng đuôi file viết tay).
- Chuỗi hiển thị tiếng Việt; comment là ghi chú `WHY` tiếng Việt.
- Commit Conventional Commits có scope crate. Máy hiện chưa có danh tính git: nếu `git commit` báo thiếu `user.name`/`user.email` thì bỏ qua commit và ghi ruling, không tự đặt danh tính.
- Chỉ `rustfmt` các file đã động tới.
- Trước Task 1, chạy `cargo test --workspace --no-fail-fast` và ghi danh sách test đang fail vào ledger làm mốc so sánh.

## Review Focus

- Explorer gọi `QueryContextMenu` nhiều lần cho cùng một object (menu mở lại): `items` được thay mới, không chồng mục — Task 6 (kiểm tay).
- Bấm mục khi GUI chưa chạy: DLL khởi động GUI rồi gửi lại; nếu `morphix-gui.exe` không có thì hộp thoại lỗi nêu nguyên nhân — Task 6 (kiểm tay).
- Client cũ (DLL/CLI phiên bản trước) gửi dòng JSON có `presentation`/`session_id`: GUI mới phải từ chối và log, không mở vòng tròn — Task 7.
- Yêu cầu có `action` không được gom lô với yêu cầu khác (Explorer đã gửi đủ file trong một lần) — Task 2.
- Id gửi qua IPC không còn áp dụng cho file (file bị đổi giữa lúc mở menu và lúc bấm): GUI báo lỗi bằng hộp thoại, không chạy job — Task 3.

---

### Task 1: Danh sách việc và id dùng chung trong core

**Files:**
- Modify: `crates/morphix-core/src/catalog.rs`, `crates/morphix-core/src/lib.rs` (export)
- Modify: `crates/morphix-gui/src/menu_items.rs` (xoá `action_to_id`, `WIRED_TOOL_IDS` và phần lọc; gọi core)
- Test: `#[cfg(test)]` trong `catalog.rs`; `crates/morphix-gui/tests/menu_items.rs` (đổi import `WIRED_TOOL_IDS` sang core)

**Interfaces:**
- Produces: `pub fn action_id(kind: ActionKind) -> String` — đúng bảng hiện có của `action_to_id` trong `menu_items.rs` (chuyển nguyên văn).
- Produces: `pub const WIRED_TOOL_IDS: [&str; 4] = ["compress", "strip", "resize", "compress-media"];`
- Produces: `pub fn menu_actions(kinds: &[FileKind]) -> Vec<Action>` — `actions_for(kinds)` bỏ `CutMedia`, `ChangeSpeed`, `ExtractAudio(_)` và mọi công cụ có id không thuộc `WIRED_TOOL_IDS` (đúng phần lọc đang ở `wheel_items`). Export cả ba ở `lib.rs`.

- [ ] **Step 1: Viết test thất bại.** Trong `catalog.rs`: `action_id_matches_wire_ids` (`ConvertTo(WebP)` → `"convert:webp"`, `CreateArchive(Zip)` → `"archive:zip"`, `Compress` → `"compress"`); `menu_actions_hides_cli_only_and_unwired_tools` (với `[FileKind::Media(MediaFormat::Mp4)]` không có `CutMedia`/`ChangeSpeed`/`ExtractAudio`; mọi mục `is_tool` có `action_id` thuộc `WIRED_TOOL_IDS`). Trong `gui/tests/menu_items.rs`: `every_wheel_item_id_maps_to_an_operation` — với ảnh JPEG, PDF, MP4, SRT, ZIP và một cặp lẫn loại, mọi `item.id` của `wheel_items` cho `operation_for(&item.id, &[], &Settings::default())` là `Ok` (nếu test tương tự đã có ở dòng ~246 thì mở rộng nó thay vì thêm).
- [ ] **Step 2: Chạy.** `cargo test -p morphix-core --lib catalog` → FAIL (chưa có `action_id`, `menu_actions`).
- [ ] **Step 3: Cài đặt.** Chuyển `action_to_id` và `WIRED_TOOL_IDS` sang `catalog.rs`; `wheel_items` gọi `menu_actions` rồi chỉ còn gán `is_suggested`.
- [ ] **Step 4: Chạy.** `cargo test -p morphix-core --lib catalog; cargo test -p morphix-gui --test menu_items` → PASS.
- [ ] **Step 5: Commit** `refactor(morphix-core): đưa id và danh sách việc của menu vào core`.

---

### Task 2: IPC mang `action`

**Files:**
- Modify: `crates/morphix-ipc/src/lib.rs` (`MenuRequest`, `WireRequest`, `encode`, `decode`, `validate_request`, `run_accept_loop`)
- Test: `crates/morphix-ipc/tests/ipc.rs`

**Interfaces:**
- Produces: `MenuRequest.action: Option<String>`; `WireRequest.action` có `#[serde(default)]`.
- `validate_request` thêm: `action = Some(id)` thì `id` không rỗng, `paths` không rỗng, `presentation == Full`, `anchor`/`session_id` là `None`, `close_fan == false`. Thông điệp lỗi tiếng Việt.
- `run_accept_loop`: yêu cầu có `action` đi thẳng `on_batch` (như `ExplorerFan`), không qua `batcher`, ở cả vòng chính lẫn vòng gom cuối.

- [ ] **Step 1: Viết test thất bại** trong `ipc.rs`: `action_request_round_trips` (encode rồi decode giữ `paths` và `action`); `action_request_without_paths_is_rejected`; `empty_action_id_is_rejected`; `action_requests_are_not_merged_into_a_batch` (hai yêu cầu có `action` gửi liên tiếp tới server test → `on_batch` nhận hai lần, mỗi lần đúng một bộ file; dựng server theo cách các test server hiện có trong file, socket riêng qua `MORPHIX_IPC_NAME`).
- [ ] **Step 2: Chạy.** `cargo test -p morphix-ipc` → FAIL (chưa có trường `action`).
- [ ] **Step 3: Cài đặt** theo Interfaces.
- [ ] **Step 4: Chạy.** `cargo test -p morphix-ipc; cargo check --workspace --all-targets` → PASS (struct literal `MenuRequest` ở crate khác không dùng `..Default::default()` thì thêm `action: None`).
- [ ] **Step 5: Commit** `feat(morphix-ipc): yêu cầu chạy thẳng một việc qua trường action`.

---

### Task 3: GUI chạy việc nhận qua IPC

**Files:**
- Modify: `crates/morphix-gui/src/commands.rs` (`run_action`), `crates/morphix-gui/src/lib.rs` (`show_menu_for`)
- Test: `crates/morphix-gui/tests/commands.rs`

**Interfaces:**
- Consumes: `MenuRequest.action` (Task 2).
- Produces: `pub fn job_for_action(id: &str, inputs: Vec<PathBuf>, settings: &Settings) -> Result<(Job, String), String>` trong `commands.rs` — phần thuần của `run_action` hiện tại: `prepare_menu`, tìm item có `id` (lỗi giữ nguyên `"Thao tác này không áp dụng cho các file đã chọn."`), `operation_for`, kiểm ffmpeg cho media, dựng `Job` với `quality_for(settings)` và `settings.output_dir`; trả kèm nhãn.
- Produces: `pub fn start_action(app: &AppHandle, id: &str, inputs: Vec<PathBuf>) -> Result<u64, String>` — gọi `job_for_action` rồi phần còn lại của `run_action` (clone job cho lịch sử, `publish_update`, `state.jobs.start`). Lệnh `run_action` chỉ còn `parse_paths` rồi gọi `start_action`.
- `show_menu_for`: nếu `request.action` là `Some(id)` thì gọi `start_action` trước mọi nhánh khác rồi `return`; `Err(message)` thì `tracing::error!` và hiện `app.dialog().message(message).title("Morphix").kind(MessageDialogKind::Error).show(|_| {})` (`tauri_plugin_dialog::DialogExt`).

- [ ] **Step 1: Viết test thất bại** trong `gui/tests/commands.rs`, file tạm là PNG thật ghi bằng `image` (dev-dependency sẵn có):
  - `job_for_action_builds_the_convert_job`: id `"convert:webp"` → `operation == Operation::ConvertImage { to: ImgFormat::WebP }`, `inputs` đúng, `quality == quality_for(&settings)`.
  - `job_for_action_rejects_an_id_that_does_not_apply`: file `.txt` UTF-8 với `"convert:webp"` → `Err` bằng đúng chuỗi ở Interfaces.
- [ ] **Step 2: Chạy.** `cargo test -p morphix-gui --test commands -- job_for_action` → FAIL.
- [ ] **Step 3: Cài đặt** theo Interfaces.
- [ ] **Step 4: Chạy.** `cargo test -p morphix-gui` → PASS.
- [ ] **Step 5: Commit** `feat(morphix-gui): chạy việc nhận qua IPC và hiện lỗi bằng hộp thoại`.

---

### Task 4: Hàm thuần dựng submenu cho DLL

**Files:**
- Modify: `crates/morphix-shell/Cargo.toml` (thêm `morphix-core = { path = "../morphix-core" }` vào `[dependencies]` thường, không gate Windows, để test chạy mọi nền tảng)
- Create: `crates/morphix-shell/src/entries.rs`; Modify: `crates/morphix-shell/src/lib.rs` (`pub mod entries;`)
- Test: `#[cfg(test)]` trong `entries.rs`

**Interfaces:**
- Consumes: `morphix_core::{detect, menu_actions, action_id, FileKind}` (Task 1).
- Produces:
  - `pub const MAX_FILES: usize = 100;`
  - `pub struct ShellItem { pub id: String, pub label: String, pub is_tool: bool }`
  - `pub enum ShellMenu { Hidden, TooMany, Items(Vec<ShellItem>) }` — `Hidden`: không chèn "Morphix".
  - `pub fn shell_menu(paths: &[PathBuf]) -> ShellMenu` — rỗng → `Hidden`; `> MAX_FILES` → `TooMany` (không gọi `detect`); `detect` lỗi → `FileKind::Unknown`; `menu_actions` rỗng → `Hidden`; còn lại `Items`, giữ thứ tự của `menu_actions`.
  - `pub fn child_slots(first_id: u32, last_id: u32, wanted: usize) -> usize` — mục cha dùng `first_id`, con dùng `first_id + 1 ..= last_id`; trả số con vừa (≤ `wanted`). `first_id >= last_id` → 0.
  - `pub fn child_for_verb(verb: usize, children: usize) -> Option<usize>` — `verb >> 16 != 0` (chuỗi) → `None`; offset 0 (mục cha) → `None`; offset `n` → `Some(n - 1)` nếu `n - 1 < children`.

- [ ] **Step 1: Ghi mốc kích thước.** `cargo build -p morphix-shell --release`, ghi kích thước `target/release/morphix_shell.dll` vào ledger.
- [ ] **Step 2: Viết test thất bại:** `one_png_lists_image_targets` (PNG thật trong thư mục tạm → có id `"convert:webp"`); `mixed_kinds_offer_only_archives` (PNG + `.txt` UTF-8 → mọi id bắt đầu bằng `"archive:"`); `unreadable_file_counts_as_unknown` (đường dẫn không tồn tại → `Hidden`); `more_than_max_files_is_too_many` (101 đường dẫn không tồn tại → `TooMany`); `child_slots_fit_the_id_range` (`(100, 105, 10)` → 5, `(100, 105, 2)` → 2, `(100, 100, 3)` → 0, `(5, 4, 3)` → 0); `string_verbs_and_parent_offset_are_rejected` (`child_for_verb(0x1_0000, 3)` → `None`, `(0, 3)` → `None`, `(1, 3)` → `Some(0)`, `(4, 3)` → `None`). PNG trong test ghi bằng `image` (thêm vào `[dev-dependencies]` của `morphix-shell` cùng phiên bản/feature như `morphix-gui`).
- [ ] **Step 3: Chạy.** `cargo test -p morphix-shell` → FAIL.
- [ ] **Step 4: Cài đặt** theo Interfaces.
- [ ] **Step 5: Chạy.** `cargo test -p morphix-shell` → PASS. Build release lại, so với mốc Step 1; tăng quá 2 MB thì dừng và hỏi người dùng.
- [ ] **Step 6: Commit** `feat(morphix-shell): dựng danh sách mục submenu từ catalog của core`.

---

### Task 5: CLI — `morphix menu` báo lỗi trên Windows, bỏ cờ quạt

**Files:**
- Modify: `crates/morphix-cli/src/main.rs` (lệnh `Menu`, `run_menu`; xoá `parse_fan_anchor`, `parse_fan_popup`, `resolve_fan_popup`, `run_close_fan`, `#[allow(clippy::too_many_arguments)]`)
- Modify: `crates/morphix-cli/src/integration/windows.rs` (xoá `menu_command`, `menu_entry` và test của chúng)
- Test: `crates/morphix-cli/tests/menu.rs`

**Interfaces:**
- `run_menu(files, newline_list, uris) -> ExitCode`. Trên Windows (`#[cfg(windows)]` ở đầu hàm) in ra stderr đúng chuỗi `lỗi: trên Windows hãy chuột phải file → Morphix trong Explorer, hoặc dùng morphix convert.` và trả `ExitCode::from(2)`, không gửi IPC. Linux giữ nguyên.
- Lệnh `Menu` mất các trường `explorer_fan`, `fan_anchor`, `fan_popup`, `session_id`, `close_fan`.

- [ ] **Step 1: Viết test thất bại** trong `menu.rs`: `#[cfg(windows)] menu_on_windows_points_to_explorer_and_exits_2` (thoát mã 2, stderr chứa `"Morphix trong Explorer"`). Xoá `explorer_fan_survives_sender_process_exit`. Gắn `#[cfg(unix)]` cho các test chuyển tiếp tới server (`menu_forwards_*`, `menu_uris_are_converted_*`, `menu_without_a_running_gui_*`, `menu_accepts_a_newline_*`) vì Windows không còn chuyển tiếp; test chỉ đổi URI giữ nguyên.
- [ ] **Step 2: Chạy.** `cargo test -p morphix-cli --test menu` → test mới FAIL.
- [ ] **Step 3: Cài đặt** theo Interfaces.
- [ ] **Step 4: Chạy.** `cargo test -p morphix-cli --test menu --test cli; cargo clippy -p morphix-cli --all-targets -- -D warnings` → PASS, trừ các lỗi PDFium có sẵn trong mốc.
- [ ] **Step 5: Commit** `feat(morphix-cli): morphix menu trên Windows chỉ sang menu Explorer`.

---

### Task 6: DLL dựng submenu native và chạy việc

**Files:**
- Modify: `crates/morphix-shell/src/windows.rs`, `crates/morphix-shell/src/lib.rs` (bỏ export `popup_rect`)

**Interfaces:**
- Consumes: `entries::{shell_menu, ShellMenu, ShellItem, child_slots, child_for_verb}` (Task 4); `MenuRequest.action` (Task 2).
- `ShellContextMenu` giữ `paths`, thêm `items: RefCell<Vec<ShellItem>>` (thay mới mỗi lần `QueryContextMenu`); bỏ `submenu`, `fan_session`. `#[implement(IShellExtInit, IContextMenu)]` (bỏ `IContextMenu2/3` vì không còn message nào).
- `QueryContextMenu`: giữ các điều kiện thoát sớm hiện có; `ShellMenu::Hidden` → `HRESULT(0)`; `TooMany` → submenu có một mục `MFS_DISABLED` với nhãn ở Global Constraints; `Items` → `MFT_STRING` theo thứ tự, `MFT_SEPARATOR` trước mục `is_tool` đầu tiên, cắt theo `child_slots`. Trả `HRESULT(1 + số con đã chèn)`.
- `InvokeCommand`: `child_for_verb((*command).lpVerb.0 as usize, items.len())`; `None` → `Err(E_INVALIDARG)`. Có mục → `dispatch_request(MenuRequest { paths, action: Some(id), ..Default::default() })`.
- `dispatch_request` = `dispatch_fan` đổi tên, bỏ tham số `menu_id`; `send_request_or_start_gui` = `send_fan` bỏ phần đợi `popup_rect`. Lỗi cuối cùng trên luồng nền → `MessageBoxW` với nội dung `Morphix không chạy được việc đã chọn: {lỗi}`, tiêu đề `Morphix`, `MB_OK | MB_ICONERROR`.
- `GetCommandString`: `GCS_HELPTEXTW` → nhãn của mục; trường hợp khác giữ `Ok(())`.
- Xoá toàn bộ phần quạt liệt kê ở spec mục 5 (`morphix-shell`) và ba test quạt hiện có.

- [ ] **Step 1: Cài đặt** theo Interfaces (logic có test nằm ở Task 4; phần này là keo COM).
- [ ] **Step 2: Chạy.** `cargo test -p morphix-shell; cargo clippy --workspace --all-targets -- -D warnings; cargo build -p morphix-shell --release` → PASS; ghi kích thước DLL release vào ledger.
- [ ] **Step 3: Kiểm tay (người dùng):** cài lại tích hợp, mở Explorer → "Show more options" → Morphix: một PNG, nhiều PNG, PNG + TXT (chỉ mục nén), file trên Desktop; bấm WebP ra `*.webp` cạnh file; tắt GUI rồi bấm (GUI tự khởi động); đổi tên `morphix-gui.exe` rồi bấm (hộp thoại lỗi); chuột phải hai lần liên tiếp (không nhân đôi mục). Ghi kết quả từng mục vào ledger.
- [ ] **Step 4: Commit** `feat(morphix-shell): submenu native thay hình quạt trong Explorer`.

---

### Task 7: Xoá hình quạt khỏi IPC, GUI và UI; kiểm tra toàn bộ

**Files:**
- Modify: `crates/morphix-ipc/src/lib.rs` (xoá `MenuPresentation`, `PhysicalRect`, `anchor`, `session_id`, `close_fan`; `WireRequest` thêm `#[serde(deny_unknown_fields)]`), `crates/morphix-ipc/tests/ipc.rs`
- Modify: `crates/morphix-gui/src/lib.rs`, `state.rs`, `windows.rs` (xoá nhánh `ExplorerFan`, `close_explorer_fan`, `fan_window_region`, `set_fan_window_region`, `closed_fans` và test `fan_window_region_*`); phần còn dùng `MenuPresentation` trong `crates/morphix-cli/src/main.rs`
- Modify: `crates/morphix-gui/ui/src/main.ts`, `ui/src/wheel.ts`, `ui/src/menu-types.ts`, `ui/src/styles.css` (bỏ `isExplorerFan`, `explorerFanItems`, `fanActiveIndex`, `fanWheelDelta`, `FAN_*`, `fanSectorPath`, `fanLabelPosition`, `fanFormatLabel`, class `explorer-fan`), `ui/tests/wheel.test.ts`

**Interfaces:**
- `MenuRequest { paths: Vec<PathBuf>, action: Option<String> }`. `validate_request`: `action = Some(id)` thì `id` và `paths` không rỗng.
- Dòng JSON có trường lạ (ví dụ `"presentation"`, `"session_id"` của client cũ) bị `decode` từ chối.

- [ ] **Step 1: Viết test thất bại** trong `ipc.rs`: `old_fan_request_line_is_rejected` (`decode` dòng `{"paths":["a.png"],"presentation":"explorer_fan","session_id":"s"}` → `Err`). Xoá các test chỉ kiểm quạt trong `ipc.rs` và `ui/tests/wheel.test.ts`.
- [ ] **Step 2: Chạy.** `cargo test -p morphix-ipc -- old_fan_request` → FAIL (dòng đó hiện hợp lệ).
- [ ] **Step 3: Cài đặt** theo Interfaces và danh sách xoá.
- [ ] **Step 4: Chạy toàn bộ cổng.**
  ```
  cargo clippy --workspace --all-targets -- -D warnings
  cargo test --workspace --no-fail-fast
  cd crates/morphix-gui/ui && npm run check && npm test
  ```
  Expected: clippy 0 lỗi; `npm` đạt; test Rust chỉ còn các lỗi có trong mốc ghi trước Task 1, không có lỗi mới. `grep -rn "ExplorerFan\|explorer_fan\|close_fan\|fanWheel" crates --include=*.rs --include=*.ts` không còn kết quả.
- [ ] **Step 5: Commit** `refactor(morphix-ipc): bỏ giao thức hình quạt`.
