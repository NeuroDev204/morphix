# Morphix mượt, nhẹ, phát hành hai nền tảng: Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Bỏ vòng tròn, đưa việc vào menu chuột phải trên Linux, gỡ mọi thao tác chặn luồng giao diện, giảm dung lượng build, và để một tag sinh ra tarball, `.deb`, `MorphixSetup.exe` không cần chứng chỉ và `.msix` cho Store.

**Architecture:** Danh sách việc vẫn chỉ có một nguồn là catalog của `morphix-core`. CLI có thêm `menu --action` (chạy thẳng qua IPC đã có) và `menu --list --json` (cho extension Nautilus). File tích hợp tĩnh cho Dolphin, Thunar và thư mục Scripts được sinh từ catalog lúc `install-integration`. GUI chỉ còn webview Khay lúc khởi động; lệnh có I/O chạy nền.

**Tech Stack:** Rust (Tauri 2, clap, windows-rs), TypeScript + Vite, Python (nautilus-python, unittest), Inno Setup, GitHub Actions.

**Spec:** `docs/superpowers/specs/2026-11-11-morphix-smooth-cross-platform-design.md`

## Global Constraints

- Làm trên nhánh `feature/smooth-cross-platform`, không commit thẳng vào `main`.
- Sau mỗi task phải xanh: `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, `npm run check && npm test` (trong `crates/morphix-gui/ui`), `python3 -m unittest discover -s scripts/tests`, `python3 -m unittest discover -s integration/tests`.
- Không đặt `panic = "abort"` trong profile release.
- Không viết bảng việc ở chỗ thứ hai: mọi danh sách việc lấy từ `morphix_core::menu_actions` / `action_id`.
- Chỉ đụng file tích hợp mang marker `Managed by Morphix` (`integration/files.rs::MANAGED_MARKER`).
- Giới hạn 100 file cho menu (`morphix_shell::MAX_FILES`), chuỗi hiển thị: `Chọn quá nhiều file (tối đa 100)`.
- Chuỗi hiển thị cho người dùng bằng tiếng Việt; comment giữ kiểu `WHY:` của repo.
- Installer exe không được chứa `certutil`, file `.cer`, hay gói sparse.
- Chỉ x86_64; glibc ≥ 2.35; không thêm đích phát hành nào ngoài spec.
- Nếu một trình quản lý file không làm được submenu như mô tả, hoặc `menu --list` chậm hơn 50 ms với 20 file, dừng và báo lại; không tự đổi thiết kế.

## Review Focus

1. Tên file có dấu cách, nháy đơn, ký tự tiếng Việt đi qua script/`Exec` sinh ra: phải chạy đúng file đó, không tách đối số. → test ở Task 7.
2. Nâng cấp từ bản cũ: mục Nemo Action cũ và service menu Dolphin một mục cũ phải biến mất, không còn mục nào mở vòng tròn. → test ở Task 8.
3. File của người dùng trùng chỗ (thư mục Scripts cùng tên không có marker, `uca.xml` có action khác): không bị sửa hay xoá. → test ở Task 8.
4. Bấm một việc khi GUI chưa chạy: CLI khởi động GUI rồi việc vẫn chạy. → test ở Task 5.
5. Việc không hợp với file (mức tĩnh, ví dụ PNG → PNG) hoặc đang có việc khác chạy: phải ra lỗi nhìn thấy được, không im lặng. → test ở Task 6.
6. `menu --list` treo hoặc trả JSON hỏng: Nautilus vẫn hiện mục "Morphix" mở Khay, không khựng. → test ở Task 9.

---

## Cột mốc A: nhẹ và không khựng (spec Phần 3, 2)

### Task 1: Số đo gốc và profile release

**Files:**
- Modify: `Cargo.toml`
- Create: `docs/superpowers/plans/notes/2026-11-11-sizes-and-timings.md`

- [ ] **Step 1: Đo trước khi sửa.** Chạy `npm run build` trong `crates/morphix-gui/ui`, rồi `cargo build --release -p morphix-cli -p morphix-gui --features morphix-gui/custom-protocol`, rồi `scripts/build-tarball.sh <thư mục ngoài repo>`. Ghi vào file notes một bảng "Trước": dung lượng `morphix`, `morphix-gui`, ffmpeg, `libpdfium.so`, tarball. Ghi thêm ba số thời gian của spec Phần 2 (khởi động nguội tới lúc Khay hiện, bấm khay tới lúc Khay hiện, thả 50 file); số nào không đo tự động được thì ghi cách đo tay và để trống cho người dùng.
- [ ] **Step 2: Thêm vào `Cargo.toml` gốc**

```toml
[profile.release]
lto = "fat"
codegen-units = 1
strip = true
```

- [ ] **Step 3: Build lại, ghi bảng "Sau".** Nếu `lto = "fat"` làm build quá 20 phút thì đổi sang `"thin"` và ghi lý do.
- [ ] **Step 4: Đo ứng viên gỡ bớt.** Chạy `cargo bloat --release -p morphix-gui --crates` (cài `cargo-bloat` nếu thiếu). Ghi 15 crate lớn nhất vào notes. Chỉ gỡ feature/dependency khi nó chiếm hơn 5% binary và không mất tính năng nào có test; mỗi lần gỡ là một commit riêng kèm số đo. Nếu ffmpeg chiếm hơn nửa tarball, ghi đề xuất bản dựng gọn hơn vào notes (URL, codec có đủ cho mọi `MediaFormat` không, mức giảm); chỉ đổi khi giảm ≥ 30% tổng gói.
- [ ] **Step 5: Chạy bộ kiểm chung** (Global Constraints). Expected: xanh.
- [ ] **Step 6: Commit** `perf(build): add release profile and record size baseline`

### Task 2: Lệnh có I/O không chạy trên luồng chính

**Files:**
- Modify: `crates/morphix-gui/src/commands.rs`, `crates/morphix-gui/src/lib.rs` (`show_menu_for`)
- Test: `crates/morphix-gui/tests/commands.rs`

**Interfaces:**
- Produces: các lệnh `menu_payload`, `run_action`, `open_output_directory`, `submit_menu_request`, `save_settings`, `clear_history` thành `pub async fn`, thân chạy trong `tauri::async_runtime::spawn_blocking` (mẫu: `integration::integration_status`). Tên lệnh và kiểu trả về phía JS không đổi.

- [ ] **Step 1: Viết test thất bại**

```rust
#[test]
fn commands_with_io_are_async() {
    let source = include_str!("../src/commands.rs");
    for name in ["menu_payload", "run_action", "open_output_directory",
                 "submit_menu_request", "save_settings", "clear_history"] {
        assert!(source.contains(&format!("pub async fn {name}(")), "{name} phải là async");
    }
}
```

- [ ] **Step 2:** `cargo test -p morphix-gui --test commands commands_with_io_are_async` → FAIL.
- [ ] **Step 3: Chuyển sáu lệnh sang async + `spawn_blocking`.** Lệnh nhận `State<'_, AppState>` đổi sang nhận `AppHandle` và lấy state bên trong closure. `save_settings` vẫn phải ghi đĩa thành công rồi mới thay giá trị trong bộ nhớ.
- [ ] **Step 4: `show_menu_for` nhánh có `action`:** gọi `commands::start_action` trong `spawn_blocking`; hộp thoại lỗi giữ nguyên nội dung.
- [ ] **Step 5:** chạy lại test → PASS; chạy bộ kiểm chung.
- [ ] **Step 6: Commit** `fix(morphix-gui): run blocking commands off the UI thread`

### Task 3: Cửa sổ Cài đặt tạo khi cần, đặt CSP

**Files:**
- Modify: `crates/morphix-gui/tauri.conf.json`, `crates/morphix-gui/src/windows.rs` (`open_settings`)
- Test: `crates/morphix-gui/tests/windows.rs`

- [ ] **Step 1: Viết test thất bại**

```rust
#[test]
fn settings_window_is_not_created_at_startup_and_csp_is_set() {
    let config: serde_json::Value =
        serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
    let labels: Vec<&str> = config["app"]["windows"].as_array().unwrap()
        .iter().map(|w| w["label"].as_str().unwrap()).collect();
    assert!(!labels.contains(&"settings"));
    assert!(config["app"]["security"]["csp"].is_string());
}
```

- [ ] **Step 2:** chạy test → FAIL.
- [ ] **Step 3:** Xoá khối `settings` khỏi `tauri.conf.json`. Trong `open_settings`, khi `get_webview_window("settings")` trả `None` thì tạo bằng `WebviewWindowBuilder::new(app, SETTINGS_WINDOW_LABEL, WebviewUrl::App("settings.html".into()))`, tiêu đề `Morphix Cài đặt`, kích thước 420×520, `visible(false)`; phần hiện và focus giữ nguyên. Đóng vẫn ẩn (đã có trong `close_hides_window`).
- [ ] **Step 4:** Đặt `csp` thành `default-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data: asset: http://asset.localhost; font-src 'self' data:; connect-src ipc: http://ipc.localhost`. Chạy `npm run tauri:dev`, mở Khay và Cài đặt, xem console không có lỗi CSP; nếu có thì nới đúng chỉ thị bị chặn và ghi lý do bằng comment `WHY`.
- [ ] **Step 5:** test PASS; bộ kiểm chung xanh; ghi lại ba số thời gian vào notes (cột "Sau cột mốc A").
- [ ] **Step 6: Commit** `perf(morphix-gui): create settings window lazily and set a CSP`

---

## Cột mốc B: menu chuột phải Linux, bỏ vòng tròn (spec Phần 1)

### Task 4: Nhóm việc tĩnh trong core

**Files:**
- Modify: `crates/morphix-core/src/catalog.rs`, `crates/morphix-core/src/lib.rs` (export)

**Interfaces:**
- Produces:

```rust
pub struct MenuGroup {
    pub key: &'static str,        // "image" | "audio" | "video" | "text" | "pdf" | "archive" | "any"
    pub label: &'static str,      // "Ảnh" | "Âm thanh" | "Video" | "Văn bản" | "PDF" | "File nén" | "Nén file"
    pub mime_types: Vec<&'static str>,   // "any" dùng ["all/allfiles"]
    pub extensions: Vec<&'static str>,   // đuôi không có dấu chấm; "any" để rỗng
    pub actions: Vec<Action>,
}
pub fn static_menu_groups() -> Vec<MenuGroup>;
```

- `actions` của một nhóm là hợp (giữ thứ tự, bỏ trùng theo `action_id`) của `menu_actions(&[kind])` trên mọi định dạng thuộc nhóm, **trừ** các việc `archive:*`; nhóm `any` chỉ gồm các việc `archive:*`. Nhóm `audio`/`video` chia theo `MediaFormat::is_audio`. Nhóm `archive` không gồm RAR.

- [ ] **Step 1: Viết test thất bại** trong `catalog.rs`:

```rust
#[test]
fn static_groups_cover_every_menu_action_exactly_like_the_catalog() {
    let groups = static_menu_groups();
    let keys: Vec<_> = groups.iter().map(|g| g.key).collect();
    assert_eq!(keys, ["image", "audio", "video", "text", "pdf", "archive", "any"]);
    let any = groups.iter().find(|g| g.key == "any").unwrap();
    assert!(any.actions.iter().all(|a| action_id(a.kind).starts_with("archive:")));
    for group in groups.iter().filter(|g| g.key != "any") {
        assert!(!group.actions.is_empty(), "{}", group.key);
        assert!(group.actions.iter().all(|a| !action_id(a.kind).starts_with("archive:")));
    }
    let image = &groups[0];
    for format in ImgFormat::ALL_ORDERED {
        for action in menu_actions(&[FileKind::Image(format)]) {
            let id = action_id(action.kind);
            assert!(id.starts_with("archive:")
                || image.actions.iter().any(|a| action_id(a.kind) == id), "{id}");
        }
    }
}
```

- [ ] **Step 2:** `cargo test -p morphix-core static_groups` → FAIL.
- [ ] **Step 3:** Cài đặt `MenuGroup` và `static_menu_groups`. Nếu `TextFormat`/`MediaFormat` chưa có danh sách mọi biến thể thì thêm hằng `ALL` cạnh enum (như `ImgFormat::ALL_ORDERED`).
- [ ] **Step 4:** test PASS. **Step 5: Commit** `feat(morphix-core): static menu groups derived from the catalog`

### Task 5: CLI `menu --action` và `menu --list --json`

**Files:**
- Modify: `crates/morphix-cli/src/main.rs` (`Command::Menu`, `run_menu`)
- Test: `crates/morphix-cli/tests/menu.rs`

**Interfaces:**
- Produces: `morphix menu --action <id> [--uris U… | --newline-list F | -- FILE…]` gửi `MenuRequest { paths, action: Some(id) }`, chạy được trên Windows lẫn Linux.
- Produces: `morphix menu --list -- FILE…` (cùng với cờ toàn cục `--json`) in đúng một dòng JSON `{"too_many":<bool>,"items":[{"id":"…","label":"…","is_tool":<bool>}]}` từ `morphix_shell::shell_menu`; `Hidden` → `items` rỗng. Không kết nối GUI. `--list` xung đột với `--action`.
- `morphix menu FILE…` không cờ: Linux gửi yêu cầu như cũ (GUI sẽ mở Khay sau Task 6); Windows giữ lỗi mã 2 hiện có.

- [ ] **Step 1: Viết test thất bại** (theo kiểu test hiện có trong `tests/menu.rs`, dùng socket giả của file đó):
  - `list_json_for_one_png_contains_convert_webp`: `items` có `id == "convert:webp"`, `too_many == false`.
  - `list_json_for_101_files_reports_too_many`: `too_many == true`, `items` rỗng.
  - `action_request_carries_the_action_id`: server giả nhận `MenuRequest` có `action == Some("convert:webp")` và đúng đường dẫn.
  - `action_request_starts_the_gui_when_nothing_is_listening` (Review Focus 4): không có server → CLI đi nhánh `start_gui_and_send`; kiểm bằng cơ chế GUI giả mà các test khởi động GUI hiện có trong file dùng.
  - `list_and_action_conflict`: thoát mã 2.
- [ ] **Step 2:** `cargo test -p morphix-cli --test menu` → các test mới FAIL.
- [ ] **Step 3:** Thêm hai cờ vào `Command::Menu`, mở rộng `run_menu`. Kiểm `cfg!(windows)` chỉ còn áp dụng khi không có `--action` và không có `--list`.
- [ ] **Step 4:** test PASS; đo `morphix menu --list --json` với 20 file ảnh bằng `hyperfine` hoặc `time` lặp 20 lần, ghi vào notes. Trung vị > 50 ms → dừng, báo lại.
- [ ] **Step 5: Commit** `feat(morphix-cli): menu --action and menu --list`

### Task 6: GUI bỏ vòng tròn, mọi lô file không có việc đi vào Khay

**Files:**
- Modify: `crates/morphix-gui/tauri.conf.json`, `capabilities/default.json`, `src/lib.rs`, `src/windows.rs`, `src/state.rs`, `src/commands.rs`, `ui/vite.config.ts`, `ui/src/styles.css`
- Delete: `ui/index.html`, `ui/src/main.ts`, `ui/src/wheel.ts`, `ui/tests/wheel.test.ts`, và module/test UI nào chỉ `main.ts` dùng (kiểm bằng `grep` trước khi xoá)
- Test: `crates/morphix-gui/tests/windows.rs`, `tests/state.rs`, `tests/commands.rs`

**Interfaces:**
- Consumes: `MenuRequest.action` (đã có).
- Produces: `show_menu_for` với `action == None` luôn gọi `windows::open_khay_with_files`. Giữ quy tắc hiện có: khi đang có `KhayWait` và lô còn `collecting` thì bỏ qua lô đó; lô cuối tiêu thụ `KhayWait`.
- Xoá: cửa sổ `menu`, `windows::open_menu`, `place_near_cursor`, `cursor_anchor`, `MENU_WINDOW_LABEL`, `MENU_REQUEST_EVENT`, `state::BatchRoute`, `route_for_batch`, `PendingMenu` cùng `pending_guard`, lệnh `pending_request` và `discard_pending`. `menu_payload`, `run_action`, `submit_menu_request` giữ nguyên vì Khay dùng.

- [ ] **Step 1: Viết test thất bại**
  - `tests/windows.rs::only_the_khay_window_exists_at_startup`: `tauri.conf.json` có đúng một cửa sổ, nhãn `khay`; `close_hides_window("menu") == false`.
  - `tests/commands.rs::inapplicable_action_is_rejected_with_a_message` (Review Focus 5): `job_for_action("convert:png", vec![<một file PNG thật>], &Settings::default())` trả `Err` chứa `không áp dụng`.
  - `tests/commands.rs::second_job_is_rejected_while_one_runs` nếu chưa có test tương đương trong `tests/jobs.rs`: `JobRunner::start` lần hai trả `Err` chứa `đang có việc chạy`.
- [ ] **Step 2:** chạy → FAIL (test đầu); hai test sau có thể đã PASS, giữ lại làm chốt.
- [ ] **Step 3:** Xoá cửa sổ, file UI, mã Rust theo danh sách trên; sửa `show_menu_for`; gỡ entry `index` khỏi `vite.config.ts` và CSS chỉ vòng tròn dùng. Sửa hoặc xoá test cũ của `route_for_batch`/`pending` cho khớp.
- [ ] **Step 4:** `npm run build` và `bash scripts/check-gui-embeds-ui.sh` vẫn đạt (sửa script nếu nó tìm `index.html`). Bộ kiểm chung xanh.
- [ ] **Step 5: Kiểm tay:** `morphix menu a.png` mở Khay có `a.png`; `morphix menu --action convert:webp a.png` sinh `a.webp`.
- [ ] **Step 6: Commit** `feat(morphix-gui)!: remove the wheel window; file batches open the tray panel`

### Task 7: Thông báo hệ thống khi việc xong

**Files:**
- Modify: `crates/morphix-gui/Cargo.toml` (thêm `tauri-plugin-notification = "2"`), `src/lib.rs` (đăng ký plugin), `src/commands.rs` (`record_finished`)

- [ ] **Step 1:** Trong `record_finished`, sau khi ghi lịch sử: nếu cửa sổ Khay không hiện, gửi thông báo tiêu đề `Morphix`, nội dung là tiêu đề và mô tả của `HistoryEntry` vừa tạo (ví dụ `3 file → anh.webp` và `Xong · Đổi sang WEBP · 3 thành công, 0 lỗi · …`). Gửi lỗi thì chỉ `tracing::warn!`, không làm hỏng việc đã xong.
- [ ] **Step 2: Kiểm tay** trên Linux: đóng Khay, chạy `morphix menu --action convert:webp a.png` → có thông báo; mở Khay rồi chạy lại → không có thông báo, Khay tự cập nhật.
- [ ] **Step 3:** bộ kiểm chung xanh. **Commit** `feat(morphix-gui): notify when a job started from the context menu finishes`

### Task 8: Bộ sinh file menu tĩnh

**Files:**
- Create: `crates/morphix-cli/src/integration/static_menu.rs`
- Modify: `crates/morphix-cli/src/integration/mod.rs` (khai báo module), `thunar.rs`

**Interfaces:**
- Consumes: `morphix_core::static_menu_groups`, `action_id`; `posix_single_quote`, `desktop_exec_argument`, `escape_desktop_value` trong `linux.rs`; `MANAGED_MARKER`.
- Produces:

```rust
pub const SCRIPTS_MENU_DIR: &str = "Morphix việc nhanh";
pub struct GeneratedFile { pub relative: PathBuf, pub body: String, pub mode: u32 }

/// Mỗi nhóm một file `morphix-<key>.desktop`: MimeType của nhóm, `X-KDE-Submenu=Morphix`,
/// mỗi việc một `[Desktop Action …]`, `Exec=<morphix> menu --action <id> -- %F`.
pub fn dolphin_service_menus(morphix: &Path) -> Result<Vec<GeneratedFile>, String>;

/// Cây `<Nhãn nhóm>/<Nhãn việc>`; mỗi file là script sh có marker, `exec <morphix> menu --action <id> -- "$@"`,
/// không có đối số thì in lỗi như `nautilus_script_body`. `/` trong nhãn đổi thành `-`.
pub fn scripts_menu_tree(morphix: &Path) -> Result<Vec<GeneratedFile>, String>;
```

- Trong `thunar.rs`: `pub fn thunar_action_blocks(morphix: &Path) -> Result<String, String>` sinh mỗi việc một `<action>` với `<submenu>Morphix</submenu>`, `<unique-id>morphix-menu-<key>-<số thứ tự></unique-id>`, lọc bằng thẻ loại của Thunar (`image-files`, `audio-files`, `video-files`, `text-files`, `other-files`) và `<patterns>` là `*.<đuôi>` của nhóm nối bằng `;` (`*` cho nhóm `any`). `upsert_action`/`remove_action` xử lý **mọi** khối có `unique-id` bắt đầu bằng `morphix-menu`, gồm cả khối đơn kiểu cũ.

- [ ] **Step 1: Viết test thất bại** (trong từng file):
  - `dolphin_menus_match_the_catalog`: số file bằng số nhóm; với mỗi nhóm, tập `--action <id>` trong file bằng tập id của `static_menu_groups()`; dòng đầu là `# Managed by Morphix`; có `X-KDE-Submenu=Morphix`.
  - `scripts_tree_matches_the_catalog`: tập (nhóm, id) của cây bằng catalog; mọi file `mode == 0o755`, dòng 2 là marker.
  - `generated_commands_survive_odd_binary_paths` (Review Focus 1): với `morphix = "/opt/My App's/morphix"`, chạy thật một script sinh ra bằng `sh` với đối số `"ảnh của tôi.png"` và `"a'b.png"`, trong đó `morphix` là một script giả ghi lại argv; kiểm argv đúng `["menu","--action",<id>,"--","ảnh của tôi.png","a'b.png"]`. Với Dolphin, kiểm `Exec=` đã qua `desktop_exec_argument`.
  - `thunar_blocks_match_the_catalog_and_replace_the_legacy_block`: `upsert_action(Some(<uca.xml có khối cũ morphix-menu và một action của người dùng>), &blocks)` giữ nguyên action của người dùng, không còn khối cũ, có đủ id; `remove_action` trả lại đúng nội dung chỉ còn action của người dùng.
- [ ] **Step 2:** `cargo test -p morphix-cli static_menu thunar` → FAIL.
- [ ] **Step 3:** Cài đặt. **Kiểm tài liệu Dolphin (KDE 5 và 6) và Thunar trước:** nếu `X-KDE-Submenu` hoặc `<submenu>` không hoạt động trên phiên bản đang hỗ trợ thì dừng, báo lại.
- [ ] **Step 4:** test PASS. **Step 5: Commit** `feat(morphix-cli): generate static context-menu files from the catalog`

### Task 9: Cài, gỡ, nâng cấp, doctor cho menu tĩnh

**Files:**
- Modify: `crates/morphix-cli/src/integration/linux.rs`, `mod.rs`, `report.rs`, `fake.rs`
- Test: `crates/morphix-cli/tests/integration.rs`

**Interfaces:**
- Consumes: Task 8.
- Produces: `ComponentId` mới `NautilusScriptMenu` (`nautilus_script_menu`), `NemoScriptMenu` (`nemo_script_menu`), `CajaScriptMenu` (`caja_script_menu`); thêm vào `ALL_COMPONENTS` và `INSTALLABLE_COMPONENTS`.
  - Gốc cây Scripts: `data_home/nautilus/scripts/`, `data_home/nemo/scripts/`, `config_home/caja/scripts/`, mỗi nơi thư mục `SCRIPTS_MENU_DIR`.
  - `DolphinServiceMenu` giờ quản lý nhiều file `morphix-<key>.desktop` trong `data_home/kio/servicemenus/`; file cũ `morphix.desktop` (có marker) bị gỡ.
  - `ThunarAction` dùng `thunar_action_blocks`.
  - `NemoAction` ra khỏi `INSTALLABLE_COMPONENTS`; file `morphix.nemo_action` có marker bị gỡ khi cài và khi gỡ.
  - `NautilusScript` và `CajaScript` (một file `Morphix` mở Khay) **giữ nguyên**: Alt+C trên X11 gọi script này.
  - `NautilusScriptMenu` chỉ cài khi `nautilus_python_present` trả `false`; khi có nautilus-python thì gỡ cây (tránh hai menu Morphix).
- Thành phần nhiều file: cài trong một `Transaction` (lỗi giữa chừng thì hoàn lại hết); gỡ thì xoá mọi file có marker dưới thư mục được quản lý rồi dọn thư mục rỗng do Morphix tạo (`CREATED_DIR_MARKER`); trạng thái `inspect` là "đã cài" chỉ khi mọi file mong muốn có mặt và đúng nội dung.
- `morphix doctor` thêm một dòng cho mỗi trình quản lý file có mặt: `menu động (theo file đang chọn)` hoặc `menu tĩnh (theo loại file)`.

- [ ] **Step 1: Viết test thất bại** trong `tests/integration.rs` (dùng thư mục XDG tạm như các test hiện có):
  - `install_writes_every_generated_file_and_uninstall_leaves_nothing`.
  - `upgrade_from_single_entry_layout_removes_legacy_files` (Review Focus 2): đặt sẵn `kio/servicemenus/morphix.desktop` và `nemo/actions/morphix.nemo_action` có marker → sau cài, cả hai không còn; có file mới.
  - `unmarked_user_files_are_left_alone` (Review Focus 3): đặt sẵn `nemo/scripts/Morphix việc nhanh/Ảnh/WebP` **không** marker và `kio/servicemenus/morphix-image.desktop` không marker → cài báo xung đột cho thành phần đó theo cách thành phần một file đang báo, nội dung hai file không đổi; gỡ không xoá chúng.
  - `nautilus_script_menu_is_skipped_when_nautilus_python_is_present`.
  - `install_is_idempotent`: cài hai lần, lần hai không thay đổi file nào.
  - `doctor_json_reports_menu_tier`.
- [ ] **Step 2:** `cargo test -p morphix-cli --test integration` → test mới FAIL.
- [ ] **Step 3:** Cài đặt. Đổi `desired_file` thành trả `Vec<DesiredFile>`; thành phần một file trả vec một phần tử để mọi đường cài/gỡ/kiểm dùng chung một mã.
- [ ] **Step 4:** test PASS; `python3 -m unittest discover -s scripts/tests` xanh (sửa `test_install_sh.py`, `test_dpkg_scripts.py` nếu chúng liệt kê file tích hợp). Kiểm `scripts/check-deb.sh` và `packaging/deb/*.in` không còn nhắc file đã bỏ.
- [ ] **Step 5: Commit** `feat(morphix-cli): install static submenus for Dolphin, Thunar and Scripts folders`

### Task 10: Submenu động trong Nautilus

**Files:**
- Modify: `integration/nautilus/morphix_menu.py`
- Test: `integration/tests/test_nautilus_menu.py`

**Interfaces:**
- Consumes: `morphix menu --list --json` và `morphix menu --action <id> --uris …` (Task 5).
- Produces (hàm thuần, test được không cần Nautilus):

```python
LIST_TIMEOUT_SECONDS: float = 0.3
MAX_FILES: int = 100
TOO_MANY_LABEL: str = "Chọn quá nhiều file (tối đa 100)"

def list_command(paths: list[str]) -> list[str]          # [EXECUTABLE, "--json", "menu", "--list", "--", *paths]
def parse_listing(stdout: str) -> tuple[bool, list[dict]] | None   # None khi JSON hỏng hoặc thiếu khoá
def action_command(action_id: str, uris: list[str]) -> list[str]   # [EXECUTABLE, "menu", "--action", id, "--uris", *uris]
```

- `get_file_items`: lựa chọn không phải toàn file thường cục bộ → hành vi hiện có. Hơn 100 file → mục "Morphix" có submenu một mục `TOO_MANY_LABEL` bị vô hiệu hoá (`sensitive=False`), không gọi CLI. Còn lại: gọi CLI với hạn chờ `LIST_TIMEOUT_SECONDS`, nhớ kết quả theo tuple URI của lựa chọn gần nhất; có mục thì dựng `Nautilus.Menu` (việc `is_tool` xếp cuối); hết hạn chờ, lỗi, JSON hỏng → một mục "Morphix" mở Khay với các file đó; danh sách rỗng → không chèn mục nào.

- [ ] **Step 1: Viết test thất bại:** `test_list_command`, `test_action_command`, `test_parse_listing_valid`, `test_parse_listing_rejects_garbage` (Review Focus 6: `""`, `"not json"`, `"{}"`, `'{"too_many":false}'` đều trả `None`), `test_too_many_files_do_not_call_cli`, `test_timeout_falls_back_to_tray_item` (giả `subprocess.run` ném `TimeoutExpired`), `test_second_call_with_same_selection_uses_cache`.
- [ ] **Step 2:** `python3 -m unittest discover -s integration/tests` → FAIL.
- [ ] **Step 3:** Cài đặt. Kiểm `nautilus_extension_body` trong `linux.rs` vẫn thay đúng dòng `EXECUTABLE` và test Rust của nó xanh.
- [ ] **Step 4:** test PASS. **Kiểm tay** trên Nautilus thật: một PNG; ba PNG; PNG + TXT (chỉ còn nén); 101 file; đổi lựa chọn nhanh liên tục không khựng. Ghi kết quả vào notes.
- [ ] **Step 5: Commit** `feat(nautilus): native Morphix submenu with one entry per action`

---

## Cột mốc C: phát hành Windows và nghiệm thu Linux (spec Phần 4, 5)

### Task 11: Mở thư mục kết quả trên Windows

**Files:**
- Modify: `crates/morphix-gui/src/commands.rs`, `crates/morphix-gui/src/lib.rs` (`capture_selection`), UI Khay nếu có nút chỉ dùng được trên Linux

- [ ] **Step 1: Viết test thất bại** cạnh `output_directory_tests`:

```rust
#[cfg(windows)]
#[test]
fn opens_the_folder_with_explorer() {
    assert_eq!(super::open_directory_command(Path::new(r"C:\a b")),
               ("explorer".into(), vec![std::ffi::OsString::from(r"C:\a b")]));
}
```

- [ ] **Step 2:** Thêm `fn open_directory_command(directory: &Path) -> (OsString, Vec<OsString>)`: Linux trả `gio open <dir>` (thay `gio_open_args`, sửa test cũ), Windows trả `explorer <dir>`. `open_output_directory` dùng hàm này trên cả hai nền tảng; trên Windows không coi mã thoát khác 0 của `explorer` là lỗi (Explorer luôn trả 1).
- [ ] **Step 3:** `grep -rn "chỉ hỗ trợ" crates/morphix-gui`: chuỗi nào Windows làm được thì sửa; phần Alt+C thì ẩn khỏi giao diện Khay/Cài đặt trên Windows.
- [ ] **Step 4:** bộ kiểm chung xanh (test `cfg(windows)` chạy ở Task 13). **Commit** `fix(morphix-gui): open the output folder on Windows`

### Task 12: Installer exe không cần chứng chỉ

**Files:**
- Modify: `installer/morphix.iss`
- Delete: `installer/register-package.ps1`, `installer/sign-package.ps1`
- Create: `scripts/tests/test_installer_iss.py`

- [ ] **Step 1: Viết test thất bại**

```python
class InstallerScript(unittest.TestCase):
    def setUp(self):
        self.text = (ROOT / "installer" / "morphix.iss").read_text(encoding="utf-8")
    def test_touches_no_certificate_store(self):
        for banned in ("certutil", ".cer", "addstore", "msix", "register-package"):
            self.assertNotIn(banned, self.text.lower())
    def test_installs_per_user(self):
        self.assertIn("PrivilegesRequired=lowest", self.text)
    def test_version_comes_from_the_command_line(self):
        self.assertNotRegex(self.text, r'#define AppVersion "')
        self.assertIn("#ifndef AppVersion", self.text)
    def test_registers_and_unregisters_the_menu(self):
        self.assertIn("install-integration", self.text)
        self.assertIn("uninstall-integration", self.text)
```

- [ ] **Step 2:** chạy → FAIL.
- [ ] **Step 3: Viết lại `morphix.iss`:** `PrivilegesRequired=lowest`, `DefaultDirName={autopf}\Morphix` (với `lowest` nó trỏ vào thư mục của người dùng); `[Files]` gồm `morphix-gui.exe`, `morphix.exe`, `morphix_shell.dll`, `bundle\ffmpeg.exe`, `bundle\pdfium.dll`, hai file giấy phép, `bundle\MicrosoftEdgeWebview2Setup.exe` (cờ `deleteafterinstall`); `[Run]`: chạy bootstrapper WebView2 với `/silent /install` chỉ khi chưa có runtime (kiểm khoá `pv` của client `{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}` trong cả HKLM và HKCU `…\EdgeUpdate\Clients`), rồi `{app}\morphix.exe install-integration` (`runhidden`); `[UninstallRun]`: `{app}\morphix.exe uninstall-integration`; `#ifndef AppVersion` → `#error` yêu cầu truyền `/DAppVersion=`. Trước khi gỡ, đóng `morphix-gui.exe` đang chạy (`CloseApplications=yes` hoặc `taskkill`).
- [ ] **Step 4:** Xác nhận bằng đọc `integration/windows.rs` rằng `install-integration` đăng ký shell extension cổ điển trong HKCU và trỏ tới `morphix_shell.dll` cạnh `morphix.exe`; sai thì dừng, báo lại.
- [ ] **Step 5:** test PASS. **Commit** `fix(installer)!: per-user setup with no certificate or sparse package`

### Task 13: CI và release cho Windows, một nguồn phiên bản

**Files:**
- Modify: `.github/workflows/ci.yml`, `.github/workflows/release.yml`, `scripts/tests/test_release_workflow.py`, `installer/store-package/AppxManifest.xml` (phiên bản thành chỗ giữ `0.0.0.0`)
- Create: `scripts/ci/setup-windows-deps.ps1`, `packaging/windows-deps.txt`

**Interfaces:**
- `packaging/windows-deps.txt`: các dòng `PDFIUM_URL`, `PDFIUM_SHA256`, `FFMPEG_URL`, `FFMPEG_SHA256` (cùng dạng `packaging/ffmpeg-static.txt`). PDFium lấy cùng bản `chromium/7881` như Linux; ffmpeg là bản tĩnh x64 có `libx264`, phải khớp nội dung `installer/bundle/FFMPEG-NOTICE.txt` (sửa notice nếu đổi nguồn).
- `setup-windows-deps.ps1`: tải, kiểm SHA256, đặt `ffmpeg.exe`, `pdfium.dll` vào `installer/bundle/` và `installer/store-package/`; tải `MicrosoftEdgeWebview2Setup.exe` và dừng nếu `Get-AuthenticodeSignature` không `Valid` hoặc người ký không phải Microsoft Corporation.

- [ ] **Step 1: Mở rộng `test_release_workflow.py` (thất bại trước):** `release.yml` có job chạy `windows-latest`; job đó gọi `setup-windows-deps.ps1`, `ISCC` với `/DAppVersion`, `makeappx`; job `publish` phụ thuộc job Windows và đính `MorphixSetup.exe` cùng `MorphixSetup.exe.sha256`; có bước so tag với phiên bản workspace; `ci.yml` có job `windows-latest` chạy `cargo clippy` và `cargo test --workspace`.
- [ ] **Step 2:** chạy → FAIL.
- [ ] **Step 3: `ci.yml`:** thêm job `test-windows` (checkout, toolchain có clippy, Node 22, `npm ci && npm run build` trong UI, `setup-windows-deps.ps1`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`). Test nào hỏng trên Windows vì lỗi thật thì sửa; test vốn chỉ dành cho Linux thì gắn `#[cfg(unix)]`.
- [ ] **Step 4: `release.yml`:** job `windows` sau `test`: build release với `--features morphix-gui/custom-protocol`; lấy phiên bản từ `Cargo.toml` gốc; dừng nếu `v<phiên bản>` khác `GITHUB_REF_NAME`; `ISCC /DAppVersion=<phiên bản> installer\morphix.iss`; sinh `.sha256`; chép binary vào bản sao của `installer/store-package` (không gồm thư mục `listing`), ghi `<phiên bản>.0` vào `Identity Version`, `makeappx pack` ra `Morphix-<phiên bản>.msix`; upload hai artifact `windows-setup` và `store-msix`. Job `publish` chỉ đính exe và `.sha256`; msix **không** đính vào Release.
- [ ] **Step 5:** test PASS; đẩy nhánh, xem job `test-windows` xanh trên GitHub.
- [ ] **Step 6: Commit** `ci: build and test on Windows; release the setup exe and Store msix`

### Task 14: Trang Store, README, nghiệm thu

**Files:**
- Create: `installer/store-package/listing/description.md`
- Modify: `README.md`, `scripts/tests/test_install_sh.py`, `docs/superpowers/plans/notes/2026-11-11-sizes-and-timings.md`

- [ ] **Step 1:** Ghi vào `description.md` nguyên văn mô tả tiếng Việt và tiếng Anh ở mục "Mô tả Store" bên dưới, kèm ba quy tắc: không nhắc tên sản phẩm khác; không nhồi từ khoá; Search terms chỉ dùng từ mô tả chức năng (`file converter`, `image compressor`, `pdf`, `video converter`).
- [ ] **Step 2: README:** câu mở đầu bỏ "trên máy Linux"; thêm mục Windows (Microsoft Store; `MorphixSetup.exe` kèm SHA256; cảnh báo SmartScreen "More info → Run anyway"; trên Windows 11 bản exe nằm trong "Show more options", bản Store nằm ở menu chính); mục Linux mô tả menu theo từng trình quản lý file (động/tĩnh) và bỏ mọi câu về vòng tròn.
- [ ] **Step 3:** `test_install_sh.py`: sau khi cài vào prefix tạm với `HOME` giả có thư mục `kio/servicemenus`, kiểm có `morphix-image.desktop`; sau `morphix uninstall` không còn file nào mang marker.
- [ ] **Step 4:** Ghi bảng dung lượng và thời gian cuối cùng vào notes.
- [ ] **Step 5: Danh sách kiểm tay cho người dùng** (ghi vào notes, đánh dấu từng dòng đã kiểm hay chưa): Nautilus có/không nautilus-python, Dolphin, Thunar, Nemo hoặc Caja; `install.sh` trên Fedora, openSUSE, Debian, Arch, Ubuntu; `MorphixSetup.exe` trên Windows 10 và 11 sạch (không quyền admin, không WebView2 sẵn); gói Store. Cập nhật bảng "Đã kiểm trên" của README chỉ với dòng thật sự đã kiểm.
- [ ] **Step 6:** bộ kiểm chung xanh. **Commit** `docs: Windows install, Linux native menu, Store listing text`

#### Mô tả Store

**Tiếng Việt**

Morphix đổi định dạng, nén và xử lý file ngay trên máy của bạn. Không tải file lên mạng, không cần tài khoản.

Bấm chuột phải lên file trong File Explorer, chọn Morphix, rồi chọn việc cần làm. File kết quả nằm cạnh file gốc; file gốc không bao giờ bị ghi đè.

Tính năng:
• Đổi định dạng ảnh, nén ảnh, đổi kích thước, xoá metadata
• Gộp nhiều ảnh thành một file PDF
• Đổi định dạng và nén video, âm thanh
• Chuyển PDF, Word, HTML và Markdown qua lại dạng văn bản
• Tạo và giải nén file ZIP, TAR.GZ, 7Z
• Xử lý nhiều file cùng lúc, xem lại lịch sử các việc đã làm

Morphix là phần mềm mã nguồn mở, giấy phép GPL-3.0.

**English**

Morphix converts, compresses and processes files on your own computer. Nothing is uploaded and no account is needed.

Right-click a file in File Explorer, choose Morphix, and pick what to do. The result is saved next to the original, which is never overwritten.

Features:
• Convert, compress and resize images, and remove metadata
• Merge several images into one PDF
• Convert and compress video and audio
• Convert between PDF, Word, HTML, Markdown and plain text
• Create and extract ZIP, TAR.GZ and 7Z archives
• Process many files at once and review your history

Morphix is open source under GPL-3.0.
