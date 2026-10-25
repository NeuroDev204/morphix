# Morphix: giao diện Tauri, khay, vòng tròn, IPC (đợt 1b) và tích hợp hệ điều hành (đợt 1c) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Đợt 1b dựng `morphix-gui` (Tauri v2): khay hệ thống, cửa sổ vòng tròn Menu nhanh (phương án A), cửa sổ Khay (phương án A), tiến trình nền một thể hiện, IPC cục bộ để `morphix menu <file…>` mở vòng tròn ngay, gom lô 300 ms, lịch sử JSON và cài đặt tối thiểu. Đợt 1c nối tiếp bằng tích hợp menu chuột phải và khởi động cùng máy trên Windows, Nautilus và Dolphin, cùng lệnh cài/gỡ/doctor có thể kiểm tra được. Kết thúc cả hai đợt, người dùng chọn file từ trình quản lý file hoặc chạy lệnh là thấy đúng giao diện đã duyệt.

**Architecture:** `morphix-core` (đợt 1a) làm toàn bộ việc xử lý file. `morphix-gui` có hai lớp: lõi Rust (`settings`, `history`, `menu_items`, `jobs`, `state`) không phụ thuộc Tauri nên test được bằng `cargo test`; lớp lệnh Tauri (`commands`, `windows`, `tray`) mỏng, chỉ chuyển tiếp. Giao thức IPC nằm trong crate nhỏ `morphix-ipc`, dùng chung bởi GUI và CLI để binary `morphix` không kéo theo Tauri/WebKitGTK. Giao diện là TypeScript thuần + Vite + Tailwind v4 (`ui/`), vòng tròn vẽ bằng SVG theo hình học của wireframe. Đợt 1c thêm mô-đun tích hợp theo nền tảng trong CLI, chỉ ghi các mục per-user do Morphix quản lý. Tiến trình nền là chính `morphix-gui`; `morphix menu` trong CLI chỉ là client mỏng gửi danh sách file qua socket cục bộ.

**Tech Stack:** Tauri 2.12.4 (`tauri`, `tauri-build`, `tauri-plugin-single-instance` 2.5.4, `tauri-plugin-dialog`), `interprocess` 2.4.6 (local socket), `serde` 1.0.229, `serde_json` 1.0.154, `tempfile` 3.27.0, `directories` 6.0.0; đợt 1c thêm `winreg` chỉ trên Windows và `url` để parse file URI trong Dolphin. Frontend: TypeScript, Vite, `@tailwindcss/vite` (Tailwind v4), `vitest`, `@fontsource/inter`. Hệ thống (Debian/Ubuntu): `libwebkit2gtk-4.1-dev`, `build-essential`, `curl`, `wget`, `file`, `libxdo-dev`, `libssl-dev`, `libayatana-appindicator3-dev`, `librsvg2-dev`; để đóng gói Linux cài thêm `patchelf` và `libfuse2` nếu bundler yêu cầu.

**Spec:** `morphix/docs/superpowers/specs/2026-10-19-morphix-design.md` (mục 4.2 đến 4.5, 5.1 đến 5.3, 5.5, 6, 8, 9 đợt 1, 10). Wireframe đã duyệt: `morphix/docs/superpowers/specs/assets/2026-10-19-morphix-wireframe.html`, tham số `?man=menu&v=a&mau=mau&nhan=ngoc` (Menu nhanh) và `?man=khay&v=a&mau=mau&nhan=ngoc` (Khay). **Mọi giá trị hình học, màu, chữ lấy từ wireframe; không tự đặt.**

**Phụ thuộc:** kế hoạch 1a đã xong. Dùng các API: `actions_for`, `ActionKind`, `Action`, `detect`, `FileKind`, `Job`, `Operation`, `run_job`, `planned_outputs`, `Progress`, `JobReport`, `ItemReport`, `CancelToken`, `Quality`, `ResizeSpec`, `MorphixError`, `data_dir`, `find_ffmpeg`. Đợt 1b **không** sửa `morphix-core` trừ khi một task ghi rõ.

**Phạm vi bị cắt có chủ đích:**
- Không làm các công cụ có giao diện riêng (Cắt ảnh, Chú thích, Che thông tin PDF, Sắp xếp trang, Cắt đoạn video): đợt 5. Vòng tròn chỉ hiện các việc đã có lõi và giao diện.
- Tasks 0–15 là đợt 1b và không làm tích hợp hệ điều hành; phần 1c nằm tiếp nối trong cùng tài liệu, từ Task 16.
- Không làm mục "···" mở lớp thứ hai (spec 4.2). Bản đầu tối đa 8 múi; nếu danh sách dài hơn thì phần dư hiện trong danh sách cuộn của tab Chuyển đổi (xem Task 11).
- Không làm dark mode, không đa ngôn ngữ.

## Global Constraints

- Khớp hình học và chữ của wireframe đã duyệt. Đổi thì ghi `Ruling:` trong ledger.
- Màu nhấn `--primary` = `#0f766e` (nhãn "Xanh ngọc" trong wireframe). Các màu khác lấy từ `tokens.css` của skill ui-ux, dán vào `styles.css` ở Task 8.
- Vòng tròn: đường kính 320 px; múi có bán kính trong 58 px, ngoài 150 px; lỗ giữa chứa tóm tắt file. Múi đầu (gợi ý) được tô màu nhấn.
- Cửa sổ Khay: rộng 400 px, neo ở góc dưới phải của màn hình chính.
- Mục tiêu thời gian: cửa sổ Menu nhanh ở trạng thái đang nhận danh sách file hiện trong 150 ms từ yêu cầu đầu tiên khi tiến trình nền đã chạy; danh sách đầy đủ và thao tác được bật sau khi cửa sổ gom lô 300 ms kết thúc (đo bằng log, Task 15).
- Gom lô: các yêu cầu `menu` đến trong cửa sổ 300 ms thành một lô.
- Lịch sử tối đa 50 dòng, file JSON ghi tạm rồi đổi tên; file hỏng thì log và dùng mặc định, không làm app không khởi động được.
- Mọi lỗi được log (`tracing`) và hiện cho người dùng bằng câu nói được nguyên nhân. Không nuốt lỗi.
- `morphix-cli` không phụ thuộc `morphix-gui`, Tauri, GTK hoặc WebKitGTK; đợt 1c chỉ thêm thư viện nền tảng `winreg`/`url` cần cho tích hợp hệ điều hành.
- Không bao giờ ghi đè file gốc (kế thừa từ 1a).
- Mỗi màn có đủ trạng thái: có dữ liệu, đang chạy, rỗng, lỗi (wireframe có ví dụ tĩnh cho từng trạng thái).
- Chữ hiển thị là tiếng Việt, tập trung trong `ui/src/strings.ts`.
- Git: làm trên nhánh `feature/morphix-gui` tách từ `feature/morphix-core`; chỉ `git add` đường dẫn dưới `morphix/`.
- Trước khi báo xong: `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, `npx vitest run` và `npm run build` trong `crates/morphix-gui/ui`; kiểm log; `git status` sạch cho `morphix/`.

## Review Focus

Các đầu vào và tình huống người dùng thật sẽ gặp mà spec ngầm hiểu; mỗi dòng có test ở task sở hữu:
1. **Hai lần bấm chuột phải nhanh liên tiếp, hoặc chọn 40 file**: ra một vòng tròn với đủ file, không hai cửa sổ, không mất file (Task 6, 12).
2. **Chưa có tiến trình nền khi bấm chuột phải**: tự khởi động và vẫn hiện vòng tròn; nếu khởi động lỗi thì báo lỗi rõ (Task 12).
3. **Đường dẫn có dấu tiếng Việt, khoảng trắng, dấu nháy, ký tự xuống dòng** đi qua socket và JSON nguyên vẹn (Task 6).
4. **Lịch sử hỏng hoặc không ghi được** không làm app treo hay không khởi động (Task 4).
5. **Nhãn và tên file dài** ("Gộp thành 1 PDF", "bao-cao-quy-III-phien-ban-cuoi-cung.pdf") không tràn hay vỡ bố cục trong vòng tròn và khay (Task 10, 11, 13).
6. **Đóng vòng tròn giữa lúc đang chạy việc**: việc tiếp tục chạy nền, lịch sử ghi đúng, không mất kết quả (Task 11).
7. **Wayland không đặt được vị trí con trỏ**: vòng tròn vẫn mở ở giữa màn hình, không lỗi (Task 11).

## Cấu trúc file

```
morphix/crates/morphix-gui/
  Cargo.toml
  build.rs
  tauri.conf.json
  capabilities/default.json
  icons/                          icon app được tạo từ SVG nguồn bằng Tauri CLI ở Task 8
  src/lib.rs                      khai báo module, run(), init_logging()
  src/main.rs                     gọi run()
  src/settings.rs                 Settings: đọc an toàn, ghi tạm rồi đổi tên, kiểm giá trị
  src/history.rs                  History: JSON, tối đa 50 dòng
  src/menu_items.rs               WheelItem, wheel_items, operation_for, quality_for
  src/jobs.rs                     JobRunner: một việc tại một lúc, tiến trình, huỷ; describe_error
  src/state.rs                    AppState, AppPaths, prepare_menu
  src/commands.rs                 lệnh Tauri (mỏng) + hàm thuần parse_paths, save_settings_to
  src/windows.rs                  mở và định vị cửa sổ Menu nhanh, Khay, Cài đặt
  src/tray.rs                     khay hệ thống và menu Hiện/Ẩn/Cài đặt/Thoát
  tests/settings.rs  tests/history.rs  tests/menu_items.rs  tests/jobs.rs
  tests/state.rs  tests/commands.rs  tests/errors.rs
morphix/crates/morphix-ipc/
  Cargo.toml                      serde + interprocess; không phụ thuộc Tauri
  src/lib.rs                      MenuRequest, encode/decode, Batcher, serve, send_request
  tests/ipc.rs                    giao thức, gom lô, socket Unix/named pipe
morphix/crates/morphix-gui/ui/
  package.json  vite.config.ts  tsconfig.json
  index.html        Menu nhanh
  khay.html         Khay
  settings.html     Cài đặt
  src/main.ts       điều phối Menu nhanh
  src/khay.ts       điều phối Khay
  src/settings.ts   điều phối Cài đặt
  src/strings.ts    toàn bộ chữ tiếng Việt
  src/wheel.ts      hình học vòng tròn (thuần, có test)
  src/keys.ts       điều hướng bàn phím (thuần, có test)
  src/format.ts     định dạng dung lượng, tên file (thuần, có test)
  src/styles.css    token từ wireframe và kiểu thành phần
  tests/wheel.test.ts  tests/keys.test.ts  tests/format.test.ts
morphix/crates/morphix-cli/src/main.rs    thêm lệnh `menu` (client IPC)
morphix/docs/superpowers/plans/notes/2026-10-19-gui-spike.md    kết quả Task 0
```

---

### Task 0: Spike môi trường và API Tauri, interprocess

**Files:** Create `morphix/docs/superpowers/plans/notes/2026-10-19-gui-spike.md` (bảng ngắn kết quả). Không tạo mã.

**Interfaces:** không có.

- [ ] **Step 1: Cài thư viện hệ thống (Debian/Ubuntu)**

```bash
sudo apt install libwebkit2gtk-4.1-dev build-essential curl wget file libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev patchelf libfuse2
pkg-config --modversion webkit2gtk-4.1
```
Expected: in ra số phiên bản. Nếu thiếu gói thì dừng và báo người dùng; không đổi phiên bản Tauri để né.

- [ ] **Step 2: Xác minh API tray và single-instance của Tauri 2.12.4 trên docs.rs**

Mở `https://docs.rs/tauri/2.12.4/tauri/tray/` và `https://docs.rs/tauri-plugin-single-instance/2.5.4/`. Ghi: tên `TrayIconBuilder` và các phương thức `.menu()`, `.on_menu_event()`, `.on_tray_icon_event()`; chữ ký `tauri_plugin_single_instance::init(|app, args, cwd| …)`. Mọi task sau dùng đúng tên trong notes.

- [ ] **Step 3: Xác minh API interprocess 2.4.6 (local socket)**

Mở `https://docs.rs/interprocess/2.4.6/interprocess/local_socket/`. Ghi: cách tạo listener và stream theo tên; cách chọn tên đường dẫn (Unix socket) và named pipe (Windows); kiểu `GenericFilePath` và `GenericNamespaced`.

- [ ] **Step 4: Kiểm registry và WebKitGTK**

```bash
cargo search tauri --limit 1
```
Expected: có kết quả. Ghi kết quả vào notes.

- [ ] **Step 5: Commit**

```bash
git add morphix/docs/superpowers/plans/notes
git commit -m "docs(morphix): gui spike notes"
```

---

### Task 1: Khung crate morphix-gui (Tauri, cửa sổ ẩn)

**Files:**
- Create: `crates/morphix-gui/Cargo.toml`, `build.rs`, `tauri.conf.json`, `capabilities/default.json`, `src/lib.rs`, `src/main.rs`, `tests/smoke.rs`, placeholder `ui/index.html`, placeholder `ui/khay.html`
- Modify: `morphix/Cargo.toml` (thêm thành viên `crates/morphix-gui`)

**Interfaces:**
- Produces: `pub fn app_version() -> &'static str` trong `lib.rs` (trả `env!("CARGO_PKG_VERSION")`); `pub fn run()` mở ứng dụng Tauri với hai cửa sổ ẩn (`menu`, `khay`); `main.rs` gọi `morphix_gui::run()`.

- [ ] **Step 1: Write the failing test** (`tests/smoke.rs`)

```rust
#[test]
fn version_matches_workspace() {
    assert_eq!(morphix_gui::app_version(), env!("CARGO_PKG_VERSION"));
}
```

- [ ] **Step 2: Run to verify fail**

Run: `cd morphix && cargo test -p morphix-gui`
Expected: FAIL, không biên dịch vì crate chưa có.

- [ ] **Step 3: Implement**

`Cargo.toml`:
```toml
[package]
name = "morphix-gui"
version.workspace = true
edition.workspace = true

[lib]
name = "morphix_gui"

[[bin]]
name = "morphix-gui"
path = "src/main.rs"

[build-dependencies]
tauri-build = "2.12.4"

[dependencies]
morphix-core = { path = "../morphix-core" }
tauri = { version = "2.12.4", features = ["tray-icon", "image-png"] }
tauri-plugin-single-instance = "2.5.4"
tauri-plugin-dialog = "2"
serde = { version = "1.0.229", features = ["derive"] }
serde_json = "1.0.154"
tracing = "0.1.44"
tracing-subscriber = "0.3.23"
interprocess = "2.4.6"
directories = "6.0.0"
tempfile = "3.27.0"

[dev-dependencies]
image = { version = "0.25.10", default-features = false, features = ["png", "jpeg"] }
```

`build.rs`: `fn main() { tauri_build::build() }`.

`tauri.conf.json`: `productName: "Morphix"`, `identifier: "vn.morphix.app"`, tạm đặt `build.frontendDist: "./ui"` để smoke build có tài sản trước khi Vite được cấu hình; Task 8 đổi thành `./ui/dist`. `build.devUrl: "http://localhost:5173"`. Khai báo hai cửa sổ ẩn với URL riêng: `label: "menu"` (320×320, không viền, trong suốt, luôn trên cùng, không hiện ở thanh tác vụ, `index.html`) và `label: "khay"` (400×520, không viền, luôn trên cùng, không ở thanh tác vụ, `khay.html`). `bundle.targets: ["appimage", "deb"]`.

`capabilities/default.json`: `core:default`, và `core:window:allow-show`, `allow-hide`, `allow-set-position`, `allow-set-focus`, `allow-close`, `allow-start-dragging` cho hai cửa sổ; `dialog:default`. Lấy vị trí con trỏ ở Rust nên không cấp quyền IPC frontend cho API này.

`src/lib.rs`: `app_version()`; `init_logging()` lấy `morphix_core::data_dir()`, tạo thư mục nếu cần và mở nối thêm `morphix.log` bằng `OpenOptions`; khởi tạo `tracing_subscriber` đúng một lần trước Tauri. Nếu không mở được file log thì ghi nguyên nhân ra stderr và cài writer stderr, không panic. `run()` gọi `init_logging()` rồi dựng `tauri::Builder::default()` với hai cửa sổ đã khai báo và `.run(tauri::generate_context!())`.

`src/main.rs`: đặt `#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]`, rồi `fn main() { morphix_gui::run() }` để GUI không bật cửa sổ console trên Windows. Hai HTML scaffold chỉ để smoke build qua macro `generate_context!`; Task 8 thay nội dung bằng trang Vite.

- [ ] **Step 4: Run to verify pass**

Run: `cd morphix && cargo test -p morphix-gui && cargo build -p morphix-gui`
Expected: PASS (1 test), build thành công.

- [ ] **Step 5: Commit**

```bash
git add morphix/Cargo.toml morphix/crates/morphix-gui
git commit -m "feat(morphix-gui): tauri shell crate with two hidden windows"
```

---

### Task 2: Cài đặt JSON (`settings.rs`)

**Files:** Create `src/settings.rs`; Modify `src/lib.rs`; Test `tests/settings.rs`.

**Interfaces:**
- Produces:
  - `#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)] pub struct Settings { pub quality: QualityChoice, pub output_dir: Option<PathBuf>, pub default_resize_percent: u8 }`, `Default` = `Balanced`, `None`, `50`.
  - `#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)] #[serde(rename_all = "lowercase")] pub enum QualityChoice { Small, Balanced, High }` và `impl From<QualityChoice> for morphix_core::Quality`.
  - `pub fn load(path: &Path) -> Settings`: file không có → mặc định; JSON hỏng → `tracing::warn!` kèm đường dẫn và nguyên nhân, trả mặc định, **không** xoá hay sửa file hỏng.
  - `pub fn save(path: &Path, settings: &Settings) -> Result<(), String>`: ghi tạm cùng thư mục rồi đổi tên.
  - `pub fn validate(settings: &Settings) -> Result<(), String>`: `default_resize_percent` trong 1..=100; `output_dir` nếu có thì tạo được thư mục (`create_dir_all`).

- [ ] **Step 1: Write the failing tests** (`tests/settings.rs`)

```rust
use morphix_gui::settings::{load, save, validate, QualityChoice, Settings};

#[test]
fn missing_file_gives_defaults() {
    let d = tempfile::tempdir().unwrap();
    assert_eq!(load(&d.path().join("settings.json")), Settings::default());
}

#[test]
fn corrupt_file_gives_defaults_and_is_left_untouched() {
    let d = tempfile::tempdir().unwrap();
    let path = d.path().join("settings.json");
    std::fs::write(&path, b"{ not json").unwrap();
    assert_eq!(load(&path), Settings::default());
    assert_eq!(std::fs::read(&path).unwrap(), b"{ not json");
}

#[test]
fn save_then_load_round_trips_unicode_paths() {
    let d = tempfile::tempdir().unwrap();
    let path = d.path().join("settings.json");
    let s = Settings {
        quality: QualityChoice::High,
        output_dir: Some(d.path().join("Ảnh xuất ra")),
        default_resize_percent: 25,
    };
    save(&path, &s).unwrap();
    assert_eq!(load(&path), s);
}

#[test]
fn validate_rejects_bad_percent() {
    let mut s = Settings::default();
    s.default_resize_percent = 0;
    assert!(validate(&s).is_err());
    s.default_resize_percent = 101;
    assert!(validate(&s).is_err());
}

#[test]
fn validate_creates_missing_output_dir() {
    let d = tempfile::tempdir().unwrap();
    let s = Settings {
        output_dir: Some(d.path().join("moi/thu-muc")),
        ..Settings::default()
    };
    validate(&s).unwrap();
    assert!(d.path().join("moi/thu-muc").is_dir());
}
```

- [ ] **Step 2: Run to verify fail**

Run: `cargo test -p morphix-gui --test settings`
Expected: FAIL (chưa có module `settings`).

- [ ] **Step 3: Implement.** Ghi tạm bằng `tempfile::NamedTempFile::new_in(parent)` rồi `persist` (ghi đè được vì đây là file cài đặt của chính app, không phải kết quả xử lý). Lỗi đọc JSON dùng `tracing::warn!`, không `unwrap`.

- [ ] **Step 4: Run to verify pass**

Expected: PASS (5 tests).

- [ ] **Step 5: Commit**

```bash
git add morphix/crates/morphix-gui
git commit -m "feat(morphix-gui): settings json with safe load and atomic save"
```

---

### Task 3: Ánh xạ múi sang việc (`menu_items.rs`)

**Files:** Create `src/menu_items.rs`; Modify `src/lib.rs`; Test `tests/menu_items.rs`.

**Interfaces:**
- Consumes: `morphix_core::{actions_for, ActionKind, FileKind, ImgFormat, Operation, Quality, ResizeSpec}`, `crate::settings::{QualityChoice, Settings}`.
- Produces:
  - `#[derive(Debug, Clone, PartialEq, Eq, Serialize)] pub struct WheelItem { pub id: String, pub label: String, pub is_tool: bool, pub is_suggested: bool }`. Id ổn định: `convert:<ext>`, `merge`, `compress`, `strip`, `resize`.
  - `pub const WIRED_TOOL_IDS: [&str; 3] = ["compress", "strip", "resize"];` — công cụ có lõi và giao diện; các công cụ khác (đợt 5) bị lọc khỏi vòng tròn.
  - `pub fn wheel_items(kinds: &[FileKind]) -> Vec<WheelItem>`: lấy từ `actions_for`, bỏ công cụ không có trong `WIRED_TOOL_IDS`. `is_suggested` đúng cho mục đầu của nhóm Chuyển đổi.
  - `pub fn operation_for(id: &str, inputs: &[PathBuf], settings: &Settings) -> Result<Operation, String>`: `convert:<ext>` → `ConvertImage{to}`; `merge` → `MergeImagesToPdf`; `compress` → `CompressImage`; `strip` → `StripMetadata`; `resize` → `ResizeImage(Percent(settings.default_resize_percent))`; id khác → `Err`.
  - `pub fn quality_for(settings: &Settings) -> Quality`.

- [ ] **Step 1: Write the failing tests** (`tests/menu_items.rs`)

```rust
use morphix_core::{FileKind, ImgFormat, Operation, ResizeSpec};
use morphix_gui::menu_items::{operation_for, wheel_items};
use morphix_gui::settings::Settings;
use std::path::PathBuf;

#[test]
fn single_jpeg_wheel_starts_with_pdf_and_marks_it_suggested() {
    let items = wheel_items(&[FileKind::Image(ImgFormat::Jpeg)]);
    assert_eq!(items[0].label, "PDF");
    assert!(items[0].is_suggested);
    assert!(items.iter().skip(1).all(|i| !i.is_suggested));
}

#[test]
fn several_images_wheel_says_pdf_gop() {
    let items = wheel_items(&[FileKind::Image(ImgFormat::Png), FileKind::Image(ImgFormat::Jpeg)]);
    assert_eq!(items[0].label, "PDF gộp");
    assert_eq!(items[0].id, "merge");
}

#[test]
fn video_selection_has_no_wheel_yet() {
    assert!(wheel_items(&[FileKind::Video]).is_empty());
}

#[test]
fn tools_without_a_screen_are_not_offered() {
    let items = wheel_items(&[FileKind::Image(ImgFormat::Jpeg)]);
    assert!(items.iter().all(|i| !i.is_tool || ["compress", "strip", "resize"].contains(&i.id.as_str())));
    assert!(items.iter().any(|i| i.id == "strip"));
}

#[test]
fn operation_for_maps_ids_and_uses_settings_percent() {
    let s = Settings { default_resize_percent: 25, ..Settings::default() };
    let inputs = vec![PathBuf::from("a.jpg")];
    assert_eq!(
        operation_for("convert:png", &inputs, &s).unwrap(),
        Operation::ConvertImage { to: ImgFormat::Png }
    );
    assert_eq!(
        operation_for("resize", &inputs, &s).unwrap(),
        Operation::ResizeImage(ResizeSpec::Percent(25))
    );
    assert!(operation_for("cat-anh", &inputs, &s).is_err());
}
```

- [ ] **Step 2: Run to verify fail**

Run: `cargo test -p morphix-gui --test menu_items`
Expected: FAIL.

- [ ] **Step 3: Implement** theo chữ ký trên. Nhãn lấy nguyên văn từ `Action.label` (đã là tiếng Việt, khớp wireframe).

- [ ] **Step 4: Run to verify pass**

Expected: PASS (5 tests).

- [ ] **Step 5: Commit**

```bash
git add morphix/crates/morphix-gui
git commit -m "feat(morphix-gui): map wheel items to core operations"
```

---

### Task 4: Lịch sử JSON (`history.rs`)

**Files:** Create `src/history.rs`; Modify `src/lib.rs`; Test `tests/history.rs`.

**Interfaces:**
- Produces:
  - `#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)] pub struct HistoryEntry { pub path: String, pub title: String, pub detail: String, pub status: EntryStatus, pub finished_at_unix: u64 }`. `path` lưu đường dẫn đầu ra đầu tiên hoặc, nếu không có đầu ra, đường dẫn đầu vào đầu tiên; chuyển sang chuỗi lossily tại ranh giới JSON để không làm hỏng lịch sử vì tên file Unix không UTF-8.
  - `#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)] #[serde(rename_all = "lowercase")] pub enum EntryStatus { Done, Failed }`.
  - `pub const MAX_ENTRIES: usize = 50;`
  - `pub struct History` với `pub fn open(path: &Path) -> History` (đọc; file hỏng thì log và bắt đầu rỗng, không xoá file), `pub fn push(&mut self, entry: HistoryEntry) -> Result<(), String>` (thêm lên đầu, cắt còn 50, ghi), `pub fn recent(&self, n: usize) -> &[HistoryEntry]`, `pub fn clear(&mut self) -> Result<(), String>`.

- [ ] **Step 1: Write the failing tests** (`tests/history.rs`)

```rust
use morphix_gui::history::{EntryStatus, History, HistoryEntry, MAX_ENTRIES};

fn entry(n: usize) -> HistoryEntry {
    HistoryEntry {
        path: format!("/tmp/file-{n}.jpg"),
        title: format!("việc {n}"),
        detail: "Đã xong".into(),
        status: EntryStatus::Done,
        finished_at_unix: n as u64,
    }
}

#[test]
fn keeps_only_the_newest_fifty() {
    let d = tempfile::tempdir().unwrap();
    let mut h = History::open(&d.path().join("history.json"));
    for n in 0..(MAX_ENTRIES + 5) {
        h.push(entry(n)).unwrap();
    }
    assert_eq!(h.recent(usize::MAX).len(), MAX_ENTRIES);
    assert_eq!(h.recent(1)[0].title, format!("việc {}", MAX_ENTRIES + 4));
}

#[test]
fn survives_reopen_with_unicode_titles() {
    let d = tempfile::tempdir().unwrap();
    let path = d.path().join("history.json");
    History::open(&path).push(entry(1)).unwrap();
    assert_eq!(History::open(&path).recent(10)[0].title, "việc 1");
}

#[test]
fn corrupt_file_starts_empty_and_is_not_deleted_until_first_write() {
    let d = tempfile::tempdir().unwrap();
    let path = d.path().join("history.json");
    std::fs::write(&path, b"[[[").unwrap();
    let mut h = History::open(&path);
    assert!(h.recent(10).is_empty());
    assert_eq!(std::fs::read(&path).unwrap(), b"[[[");
    h.push(entry(1)).unwrap();
    assert_eq!(History::open(&path).recent(10).len(), 1);
}

#[test]
fn clear_empties_the_file_on_disk() {
    let d = tempfile::tempdir().unwrap();
    let path = d.path().join("history.json");
    let mut h = History::open(&path);
    h.push(entry(1)).unwrap();
    h.clear().unwrap();
    assert!(History::open(&path).recent(10).is_empty());
}
```

- [ ] **Step 2: Run to verify fail**

Run: `cargo test -p morphix-gui --test history`
Expected: FAIL.

- [ ] **Step 3: Implement.** Ghi tạm bằng `NamedTempFile::new_in` rồi `persist`. Đọc hỏng thì `tracing::warn!` rồi trả lịch sử rỗng. Lần `push` đầu tiên ghi đè file hỏng là chủ ý: app phải tiếp tục hoạt động (ghi vào ruling).

- [ ] **Step 4: Run to verify pass**

Expected: PASS (4 tests).

- [ ] **Step 5: Commit**

```bash
git add morphix/crates/morphix-gui
git commit -m "feat(morphix-gui): json history capped at fifty entries"
```

---

### Task 5: Chạy việc trên luồng nền (`jobs.rs`)

**Files:** Create `src/jobs.rs`; Modify `src/lib.rs`; Test `tests/jobs.rs`.

**Interfaces:**
- Consumes: `morphix_core::{Job, Operation, run_job, CancelToken, Progress, JobReport, ItemReport}`.
- Produces:
  - `#[derive(Debug, Clone, PartialEq, Eq, Serialize)] pub struct JobSnapshot { pub id: u64, pub label: String, pub done: usize, pub total: usize, pub state: JobState }`.
  - `#[derive(Debug, Clone, PartialEq, Eq, Serialize)] #[serde(tag = "kind", rename_all = "lowercase")] pub enum JobState { Running, Finished { ok: usize, failed: usize }, Cancelled }`.
  - `pub struct JobRunner` với `pub fn new() -> Self`; `pub fn start(&self, job: Job, label: String, on_update: Arc<dyn Fn(JobSnapshot) + Send + Sync>) -> Result<u64, String>` (từ chối nếu đang có việc hoặc báo cáo trước chưa được lấy: `Err("đang có việc chạy, đợi xong hoặc huỷ")`); `pub fn cancel(&self, id: u64) -> bool`; `pub fn active(&self) -> Option<JobSnapshot>`; `pub fn take_report(&self, id: u64) -> Option<JobReport>` (lấy một lần, sau khi việc xong); `pub fn shutdown(&self) -> Result<(), String>` (huỷ việc hiện tại nếu có, lấy và join worker, chuyển lỗi panic thành `Err`).
  - `pub fn describe_error(error: &MorphixError, input: &Path) -> String`: nếu lỗi đã nêu đường dẫn thì giữ nguyên thông báo của lõi; nếu không (ví dụ `InvalidInput`) thì thêm tên file đầu vào. Dùng cho banner lỗi và lịch sử (Task 15 kiểm).
- Quy tắc: một việc tại một thời điểm; mỗi lần tiến trình đổi thì gọi `on_update`; khi xong, báo cáo được giữ để lớp trên ghi lịch sử. Khoá bị poison vẫn dùng được trạng thái: `lock().unwrap_or_else(|p| p.into_inner())`, có comment giải thích.

- [ ] **Step 1: Write the failing tests** (`tests/jobs.rs`)

```rust
use morphix_core::{ImgFormat, Job, MorphixError, Operation, Quality};
use morphix_gui::jobs::JobRunner;
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant};

fn png(dir: &std::path::Path, name: &str) -> std::path::PathBuf {
    let p = dir.join(name);
    image::RgbImage::from_pixel(8, 8, image::Rgb([1, 2, 3])).save(&p).unwrap();
    p
}

fn job(inputs: Vec<std::path::PathBuf>, quality: Quality) -> Job {
    Job {
        inputs,
        operation: Operation::ConvertImage { to: ImgFormat::Jpeg },
        quality,
        out_dir: None,
    }
}

fn wait_for_report(runner: &JobRunner, id: u64) -> morphix_core::JobReport {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        if let Some(report) = runner.take_report(id) {
            return report;
        }
        assert!(Instant::now() < deadline, "việc không kết thúc");
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn second_job_is_refused_while_one_is_running() {
    let d = tempfile::tempdir().unwrap();
    let runner = JobRunner::new();
    let inputs: Vec<_> = (0..30).map(|i| png(d.path(), &format!("f{i}.png"))).collect();
    let (reached_first_item, reached_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let callback = Arc::new(move |snapshot: morphix_gui::jobs::JobSnapshot| {
        if snapshot.done == 1 {
            reached_first_item.send(()).unwrap();
            release_rx.recv().unwrap();
        }
    });
    let id = runner.start(job(inputs, Quality::High), "Đổi sang JPG".into(), callback).unwrap();
    reached_rx.recv_timeout(Duration::from_secs(20)).unwrap();
    let second = runner.start(job(vec![png(d.path(), "b.png")], Quality::Balanced), "Nén ảnh".into(), Arc::new(|_| {}));
    assert!(second.is_err(), "việc thứ hai phải bị từ chối khi việc đầu còn chạy");
    release_tx.send(()).unwrap();
    wait_for_report(&runner, id);
}

#[test]
fn updates_reach_the_callback_and_finish_with_counts() {
    let d = tempfile::tempdir().unwrap();
    let runner = JobRunner::new();
    let seen = Arc::new(Mutex::new(Vec::new()));
    let sink = seen.clone();
    let inputs = vec![png(d.path(), "a.png"), png(d.path(), "b.png")];
    let id = runner
        .start(job(inputs, Quality::Balanced), "Đổi sang JPG".into(), Arc::new(move |s| sink.lock().unwrap().push(s)))
        .unwrap();
    let report = wait_for_report(&runner, id);
    assert_eq!((report.succeeded(), report.failed()), (2, 0));
    let updates = seen.lock().unwrap();
    assert!(updates.iter().any(|s| s.done == 2 && s.total == 2));
}

#[test]
fn cancel_marks_pending_items_cancelled() {
    let d = tempfile::tempdir().unwrap();
    let runner = JobRunner::new();
    let inputs: Vec<_> = (0..30).map(|i| png(d.path(), &format!("g{i}.png"))).collect();
    let (reached_first_item, reached_rx) = std::sync::mpsc::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let callback = Arc::new(move |snapshot: morphix_gui::jobs::JobSnapshot| {
        if snapshot.done == 1 {
            reached_first_item.send(()).unwrap();
            release_rx.recv().unwrap();
        }
    });
    let id = runner.start(job(inputs, Quality::High), "Đổi sang JPG".into(), callback).unwrap();
    reached_rx.recv_timeout(Duration::from_secs(20)).unwrap();
    assert!(runner.cancel(id));
    release_tx.send(()).unwrap();
    let report = wait_for_report(&runner, id);
    assert!(report.items.iter().any(|i| matches!(&i.result, Err(MorphixError::Cancelled))));
}

#[test]
fn describe_error_names_the_file_when_the_core_message_does_not() {
    use morphix_gui::jobs::describe_error;
    let e = MorphixError::InvalidInput("phần trăm phải từ 1 đến 100".into());
    let text = describe_error(&e, std::path::Path::new("/a/Ảnh.jpg"));
    assert!(text.contains("Ảnh.jpg"), "{text}");
    let corrupt = MorphixError::Corrupt {
        path: "/a/x.jpg".into(),
        reason: "EOF".into(),
    };
    assert!(describe_error(&corrupt, std::path::Path::new("/a/x.jpg")).contains("bị hỏng"));
}

#[test]
fn shutdown_cancels_and_joins_the_active_worker() {
    let d = tempfile::tempdir().unwrap();
    let runner = JobRunner::new();
    let inputs: Vec<_> = (0..30).map(|i| png(d.path(), &format!("s{i}.png"))).collect();
    let (reached_first_item, reached_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let callback = Arc::new(move |snapshot: morphix_gui::jobs::JobSnapshot| {
        if snapshot.done == 1 {
            reached_first_item.send(()).unwrap();
            release_rx.recv().unwrap();
        }
    });
    runner.start(job(inputs, Quality::High), "Đổi sang JPG".into(), callback).unwrap();
    reached_rx.recv_timeout(Duration::from_secs(20)).unwrap();
    runner.cancel(runner.active().unwrap().id);
    release_tx.send(()).unwrap();
    runner.shutdown().unwrap();
    assert!(runner.active().is_none());
}
```

- [ ] **Step 2: Run to verify fail**

Run: `cargo test -p morphix-gui --test jobs`
Expected: FAIL.

- [ ] **Step 3: Implement.** `JobRunner` giữ `Mutex<Option<ActiveJob>>` (`ActiveJob { id, cancel: CancelToken, snapshot: JobSnapshot }`), `AtomicU64` cho id, `Mutex<Option<(u64, JobReport)>>` cho báo cáo và `Mutex<Option<JoinHandle<()>>>` cho worker. `start` đặt `ActiveJob`, rồi spawn worker gọi `run_job(&job, &cancel, &progress_cb)` và lưu handle; nếu còn handle đã hoàn tất từ lần trước thì join trước khi bắt đầu worker mới. `progress_cb` cập nhật snapshot và gọi `on_update`. Khi xong: xoá `ActiveJob`, lưu báo cáo, gọi `on_update` đúng một lần cuối với `Finished` hoặc `Cancelled` (`Cancelled` khi `cancel.is_cancelled()`). `shutdown` huỷ token đang hoạt động, lấy handle khỏi mutex rồi join mà không giữ lock; lỗi panic được trả về để caller log.

- [ ] **Step 4: Run to verify pass**

Expected: PASS (5 tests).

- [ ] **Step 5: Commit**

```bash
git add morphix/crates/morphix-gui
git commit -m "feat(morphix-gui): single background job runner with progress, cancel and error text"
```

---

### Task 6: Giao thức IPC và gom lô (`morphix-ipc`)

**Files:** Create `crates/morphix-ipc/Cargo.toml`, `crates/morphix-ipc/src/lib.rs`, `crates/morphix-ipc/tests/ipc.rs`; Modify workspace `Cargo.toml` and `crates/morphix-gui/Cargo.toml` (thêm dependency dùng chung, bỏ `interprocess` và `directories` trực tiếp khỏi GUI).

**Interfaces:**
- Consumes: `interprocess::local_socket` (API đã chốt ở Task 0).
- Produces:
  - `#[derive(Debug, Clone, PartialEq, Eq)] pub struct MenuRequest { pub paths: Vec<PathBuf> }`; không derive serde trực tiếp cho `PathBuf` vì đường dẫn Unix không UTF-8 cần được xử lý rõ ở ranh giới giao thức.
  - `pub fn socket_name() -> String`: `morphix-<tên người dùng>.sock` đặt trong `$XDG_RUNTIME_DIR` nếu có, không thì trong thư mục dữ liệu; trên Windows là tên named pipe `morphix-<tên người dùng>`. Tên phải duy nhất theo người dùng. Khi `MORPHIX_IPC_NAME` được đặt, dùng giá trị đó để cô lập tiến trình test.
  - `pub fn encode(request: &MenuRequest) -> String` (JSON một dòng kết thúc bằng `\n`) và `pub fn decode(line: &str) -> Result<MenuRequest, String>` (dòng rỗng hoặc JSON sai trả `Err` có nguyên nhân). Dùng DTO dây riêng có `Vec<String>`; encode chuyển từng path bằng `to_string_lossy`, decode dựng lại `PathBuf`.
  - `pub struct Batcher` với `pub fn new(window: Duration) -> Self`, `pub fn push(&mut self, paths: Vec<PathBuf>, now: Instant) -> Vec<PathBuf>`, `pub fn take_ready(&mut self, now: Instant) -> Option<Vec<PathBuf>>`: `push` trả snapshot đường dẫn đã gộp ngay lập tức; `take_ready` trả lô cuối khi đã qua `window` kể từ lần đẩy đầu tiên; gộp đường dẫn trùng, giữ thứ tự đến.
  - `pub fn send_request(name: &str, request: &MenuRequest) -> std::io::Result<()>` (client).
  - `pub struct IpcServer` với `pub fn start(name: &str, on_pending: impl Fn(Vec<PathBuf>) + Send + Sync + 'static, on_batch: impl Fn(Vec<PathBuf>) + Send + Sync + 'static) -> std::io::Result<Self>` và `pub fn shutdown(self) -> std::io::Result<()>`. `start` mở listener đồng bộ trước khi trả về, sau đó chạy accept/read/batcher ở luồng nền; gọi `on_pending` ngay sau mỗi request để hiện cửa sổ trạng thái nhận file, gọi `on_batch` khi hết 300 ms để bật thao tác. `shutdown` báo dừng và join luồng. Dùng polling hoặc kết nối đánh thức theo API đã xác minh ở Task 0 để listener không chặn teardown.
- Quy tắc lỗi: kết nối hỏng phía server → `tracing::warn!`; client không kết nối được → `io::Error` có nguyên nhân để CLI quyết định khởi động GUI.
- Giới hạn đã chấp nhận: đường dẫn không phải UTF-8 trên Linux được chuyển bằng `to_string_lossy`. Ghi vào ruling.

- [ ] **Step 1: Write the failing tests** (`crates/morphix-ipc/tests/ipc.rs`)

```rust
use morphix_ipc::{decode, encode, Batcher, MenuRequest};
use std::path::PathBuf;
use std::time::{Duration, Instant};

#[test]
fn round_trip_keeps_unicode_quotes_spaces_and_newlines_in_names() {
    let req = MenuRequest {
        paths: vec![
            PathBuf::from("/home/a/Ảnh chụp 1.jpg"),
            PathBuf::from("/home/a/it's \"quoted\".png"),
            PathBuf::from("/home/a/ghi\nchú.jpg"),
        ],
    };
    let line = encode(&req);
    assert!(line.ends_with('\n'));
    assert_eq!(line.matches('\n').count(), 1, "xuống dòng trong tên phải được thoát");
    assert_eq!(decode(line.trim_end()).unwrap(), req);
}

#[test]
fn decode_rejects_garbage_with_a_reason() {
    assert!(!decode("").unwrap_err().is_empty());
    assert!(decode("{ broken").is_err());
}

#[cfg(unix)]
#[test]
fn non_utf8_path_is_lossy_only_at_the_ipc_boundary() {
    use std::os::unix::ffi::OsStringExt;
    let path = PathBuf::from(std::ffi::OsString::from_vec(vec![b'/', b'a', 0xff]));
    let decoded = decode(encode(&MenuRequest { paths: vec![path] }).trim_end()).unwrap();
    assert_eq!(decoded.paths, vec![PathBuf::from("/a\u{fffd}")]);
}

#[test]
fn batcher_merges_requests_inside_the_window_and_dedups() {
    let t0 = Instant::now();
    let mut b = Batcher::new(Duration::from_millis(300));
    b.push(vec![PathBuf::from("a.jpg"), PathBuf::from("b.jpg")], t0);
    b.push(
        vec![PathBuf::from("b.jpg"), PathBuf::from("c.jpg")],
        t0 + Duration::from_millis(100),
    );
    assert!(b.take_ready(t0 + Duration::from_millis(250)).is_none());
    assert_eq!(
        b.take_ready(t0 + Duration::from_millis(350)).unwrap(),
        vec![PathBuf::from("a.jpg"), PathBuf::from("b.jpg"), PathBuf::from("c.jpg")]
    );
}

#[test]
fn batcher_keeps_forty_files_from_two_fast_clicks() {
    let t0 = Instant::now();
    let mut b = Batcher::new(Duration::from_millis(300));
    let first: Vec<_> = (0..20).map(|i| PathBuf::from(format!("{i}.jpg"))).collect();
    let second: Vec<_> = (20..40).map(|i| PathBuf::from(format!("{i}.jpg"))).collect();
    b.push(first, t0);
    b.push(second, t0 + Duration::from_millis(50));
    assert_eq!(b.take_ready(t0 + Duration::from_secs(1)).unwrap().len(), 40);
}

#[test]
fn server_and_client_talk_over_a_real_local_socket() {
    use std::sync::{Arc, Mutex};
    let name = format!("morphix-test-{}", std::process::id());
    let got = Arc::new(Mutex::new(Vec::<(bool, Vec<PathBuf>)>::new()));
    let pending_sink = got.clone();
    let final_sink = got.clone();
    let server = morphix_ipc::IpcServer::start(
        &name,
        move |paths| pending_sink.lock().unwrap().push((false, paths)),
        move |paths| final_sink.lock().unwrap().push((true, paths)),
    ).unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let sent = morphix_ipc::send_request(&name, &MenuRequest { paths: vec![PathBuf::from("x.png")] });
        if sent.is_ok() {
            break;
        }
        assert!(Instant::now() < deadline, "server không lên");
        std::thread::sleep(Duration::from_millis(50));
    }
    while !got.lock().unwrap().iter().any(|(is_final, _)| !is_final) {
        assert!(Instant::now() < deadline + Duration::from_secs(3), "lô không tới");
        std::thread::sleep(Duration::from_millis(20));
    }
    let pending = got.lock().unwrap()[0].1.clone();
    assert_eq!(pending, vec![PathBuf::from("x.png")]);
    while !got.lock().unwrap().iter().any(|(is_final, _)| *is_final) {
        assert!(Instant::now() < deadline + Duration::from_secs(3), "lô chưa được chốt");
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(got.lock().unwrap().last().unwrap().1, pending);
    server.shutdown().unwrap();
}

#[test]
fn shutdown_releases_the_socket_name_for_a_new_server() {
    let name = format!("morphix-shutdown-test-{}", std::process::id());
    let first = morphix_ipc::IpcServer::start(&name, |_| {}, |_| {}).unwrap();
    first.shutdown().unwrap();
    let second = morphix_ipc::IpcServer::start(&name, |_| {}, |_| {}).unwrap();
    second.shutdown().unwrap();
}
```

- [ ] **Step 2: Run to verify fail**

Run: `cargo test -p morphix-ipc --test ipc`
Expected: FAIL.

- [ ] **Step 3: Implement.** Tạo crate `morphix-ipc` chỉ phụ thuộc `serde`, `serde_json`, `interprocess`, `directories` và `tracing`; thêm crate vào workspace. Chuyển dependency IPC của GUI sang `morphix-ipc`, bỏ dependency trực tiếp `interprocess` và `directories`, không giữ hai bản giao thức riêng. `encode` dựng DTO dây `Vec<String>` bằng `Path::to_string_lossy` trước khi gọi `serde_json::to_string` (JSON đã thoát xuống dòng nên luôn một dòng); `decode` parse DTO rồi chuyển chuỗi thành `PathBuf`. `socket_name()` ưu tiên `MORPHIX_IPC_NAME`; nếu không có, dùng `$XDG_RUNTIME_DIR/morphix.sock` trên Linux, rồi `MORPHIX_DATA_DIR`, rồi `ProjectDirs::from("", "", "Morphix").data_local_dir()` làm đường lùi. Trên Windows, dùng named pipe có tên chứa tài khoản hiện tại. Server dùng listener của `interprocess` theo API đã ghi ở Task 0, đọc đúng một dòng JSON cho mỗi kết nối và đặt timeout đọc để một client kết nối dở không làm `shutdown` treo. Đặt `Batcher` sau `Mutex`, luồng đồng hồ gọi `take_ready` và `on_batch`. `shutdown` phải đánh thức accept loop, join luồng và trả lỗi join/I/O thay vì bỏ qua; Drop chỉ báo dừng như lớp bảo vệ cuối. GUI và CLI cùng dùng API crate này; crate không phụ thuộc Tauri hoặc `morphix-gui`.

- [ ] **Step 4: Run to verify pass**

Expected: PASS (7 tests trên Unix, 6 tests trên Windows). Tên socket trong test có PID nên không đụng nhau khi chạy song song. Chạy bộ test trên Linux cho Unix socket; chạy cùng bộ test trên Windows cho named pipe.

- [ ] **Step 5: Commit**

```bash
git add morphix/Cargo.toml morphix/crates/morphix-ipc
git commit -m "feat(morphix-ipc): local socket ipc with 300ms batching"
```

---

### Task 7: Trạng thái ứng dụng và chuẩn bị menu (`state.rs`)

**Files:** Create `src/state.rs`; Modify `src/lib.rs`; Test `tests/state.rs`.

**Interfaces:**
- Consumes: `history::History`, `settings::{load, Settings}`, `jobs::JobRunner`, `menu_items::{wheel_items, WheelItem}`, `morphix_core::{detect, FileKind, data_dir}`, `morphix_ipc::IpcServer`.
- Produces:
  - `#[derive(Debug, Clone, PartialEq, Eq)] pub struct AppPaths { pub data_dir: PathBuf, pub history: PathBuf, pub settings: PathBuf, pub log: PathBuf }` và `impl AppPaths { pub fn from_data_dir(dir: &Path) -> Self }` (tên cố định: `history.json`, `settings.json`, `morphix.log`).
  - `pub struct PendingMenu { pub paths: Vec<PathBuf>, pub collecting: bool, pub revision: u64, pub first_request_at: Option<Instant> }`; tăng revision mỗi snapshot. `first_request_at` chỉ dùng nội bộ để đo lần mở cửa sổ, không serialize.
  - `pub struct AppState { pub paths: AppPaths, pub history: Mutex<History>, pub settings: Mutex<Settings>, pub jobs: JobRunner, pub pending: Mutex<PendingMenu>, pub ipc_server: Mutex<Option<morphix_ipc::IpcServer>> }` với `pub fn load(paths: AppPaths) -> AppState`; khởi tạo menu rỗng và `ipc_server=None`. Đảm bảo lock poison được khôi phục theo cùng quy tắc ở Task 5.
  - `#[derive(Debug, Clone, PartialEq, Eq, Serialize)] pub struct FileSummary { pub name: String, pub kind: String }` (`kind` là `image`, `pdf`, `other`).
  - `#[derive(Debug, Clone, PartialEq, Eq, Serialize)] pub struct WheelPayload { pub files: Vec<FileSummary>, pub items: Vec<WheelItem>, pub total_bytes: u64 }`.
  - `pub fn prepare_menu(paths: &[PathBuf]) -> Result<WheelPayload, String>`: rỗng → `Err`; file không đọc được → `Err` có tên file; `total_bytes` là tổng kích thước file.
  - `pub fn record_job(state: &AppState, title: String, detail: String, status: EntryStatus)`: ghi lịch sử; lỗi ghi được `tracing::error!` (không bỏ qua).

- [ ] **Step 1: Write the failing tests** (`tests/state.rs`)

```rust
use morphix_gui::state::{prepare_menu, AppPaths};
use std::path::PathBuf;

#[test]
fn prepare_menu_summarises_images_and_reports_total_size() {
    let d = tempfile::tempdir().unwrap();
    let a = d.path().join("Ảnh 1.png");
    image::RgbImage::from_pixel(10, 10, image::Rgb([9, 9, 9])).save(&a).unwrap();
    let payload = prepare_menu(&[a]).unwrap();
    assert_eq!(payload.files[0].name, "Ảnh 1.png");
    assert_eq!(payload.files[0].kind, "image");
    assert!(payload.total_bytes > 0);
    assert_eq!(payload.items[0].label, "PDF");
}

#[test]
fn prepare_menu_names_the_missing_file() {
    let err = prepare_menu(&[PathBuf::from("/không/có/file.jpg")]).unwrap_err();
    assert!(err.contains("file.jpg"), "{err}");
}

#[test]
fn prepare_menu_rejects_empty_request() {
    assert!(prepare_menu(&[]).is_err());
}

#[test]
fn app_paths_are_fixed_names_under_data_dir() {
    let p = AppPaths::from_data_dir(std::path::Path::new("/x"));
    assert_eq!(p.history.file_name().unwrap(), "history.json");
    assert_eq!(p.settings.file_name().unwrap(), "settings.json");
    assert_eq!(p.log.file_name().unwrap(), "morphix.log");
}
```

- [ ] **Step 2: Run to verify fail**

Run: `cargo test -p morphix-gui --test state`
Expected: FAIL.

- [ ] **Step 3: Implement.** `prepare_menu` gọi `detect` cho từng file (lỗi `Io` có tên file trong thông báo), `wheel_items` trên các `FileKind`, `total_bytes` từ `metadata`. `kind` của `FileSummary`: `Image` → `image`, `Pdf` → `pdf`, còn lại → `other`. `AppState::load` thử tạo `data_dir` (nếu lỗi thì log nhưng vẫn khởi động), mở lịch sử và cài đặt bằng đường dẫn trong `AppPaths`, khởi tạo `PendingMenu` rỗng (`first_request_at=None`, revision ban đầu 0), nhưng không tự mở socket; Task 12 khởi tạo socket sau khi Tauri có `AppHandle` để callback có thể mở cửa sổ.

- [ ] **Step 4: Run to verify pass**

Expected: PASS (4 tests).

- [ ] **Step 5: Commit**

```bash
git add morphix/crates/morphix-gui
git commit -m "feat(morphix-gui): app state and menu preparation"
```

---

### Task 8: Hệ thống thiết kế: token từ wireframe và kiểu thành phần

**Files:** Create `ui/package.json`, `ui/vite.config.ts`, `ui/tsconfig.json`, `ui/app-icon.svg`, `ui/src/styles.css`, `ui/src/strings.ts`, icon trong `crates/morphix-gui/icons/`; Modify `ui/index.html`, `ui/khay.html`, `crates/morphix-gui/tauri.conf.json` (`frontendDist: "./ui/dist"` và icon bundle).

**Interfaces:**
- `styles.css` khai báo token (giá trị lấy từ wireframe và `tokens.css` của skill):
  - `--background: #f4f4f6; --surface: #ffffff; --surface-overlay: #ffffff; --foreground: #2c2c2c; --muted: #707070; --border: #f7f7f8; --border-strong: #eaeaea; --primary: #0f766e; --primary-hover: color-mix(in oklab, var(--primary) 85%, var(--background)); --primary-foreground: #ffffff; --item-hover: #f4f4f6; --secondary: #e7e8ec; --danger: #c70036; --danger-bg: rgb(255 32 86 / 0.1); --error-text: #e7000b; --error-strong: #c10007; --error-bg: #fef2f2; --error-border: #ffc9c9; --radius: 0.75rem; --elevation-popover: 0 10px 15px -3px rgb(0 0 0 / 0.1), 0 4px 6px -4px rgb(0 0 0 / 0.1);`
  - Font: `Inter` (400, 500, 600) qua `@fontsource/inter`, `font-family: var(--app-font)`.
  - Kiểu thành phần: `.wheel-sector` (fill `rgb(0 0 0 / .05)`, rê `rgb(0 0 0 / .1)`; múi gợi ý `fill: var(--primary)`), `.segmented` (rãnh `bg-background` p-1 bo `xl`, ô đang chọn `bg-surface` bóng mảnh `--shadow-segment-thumb`), `.btn` (ba biến thể outline/primary/ghost như wireframe), `.drop-zone` (viền đứt `2px dashed rgb(0 0 0 / .15)`), `.row` (rê `--item-hover`), `.banner-error` (nền `--error-bg`, viền `--error-border`, chữ `--error-strong`).
  - Mỗi khối có comment dẫn nguồn trong wireframe (ví dụ `/* wireframe: .wf-set */`).
- `strings.ts` xuất một object `S` chứa: `menuTabs`, `escHint: "Esc để đóng · giữ Alt để mở Công cụ"`, `collectingFiles: "Đang nhận danh sách file…"`, `emptyUnsupported: "Chưa hỗ trợ loại file này"`, `emptyMixed: "Hãy chọn các file cùng loại"`, `retry: "Thử lại"`, `skip: "Bỏ qua"`, `cancel: "Huỷ"`, `recentTitle: "Gần đây"`, `clearHistory: "Xoá lịch sử"`, `dropTitle: "Thả file vào đây"`, `dropHint: "Hoặc chuột phải vào file → Morphix"`, `chooseFile: "Chọn file"`, `footerPrivacy: "Xử lý ngay trên máy, không tải lên"`, `quit: "Thoát"`, `emptyHistory: "Chưa chuyển file nào"`, `errorBanner: "1 file không chuyển được. Xem dòng đánh dấu bên dưới."`, `settingsTitle: "Cài đặt"`, `keepInSource: "Lưu cạnh file gốc"`, `qualityBalanced: "Cân bằng"`, `trayShow: "Hiện"`, `trayHide: "Ẩn"`, `traySettings: "Cài đặt"`, `trayQuit: "Thoát"`. Chữ của vòng tròn lấy từ `wheel_items` (Rust), không viết lại trong TS.

- [ ] **Step 1: Tạo khung và cài phụ thuộc**

```bash
cd morphix/crates/morphix-gui/ui
npm init -y
npm i -D vite typescript vitest @tailwindcss/vite tailwindcss @tauri-apps/cli
npm i @tauri-apps/api @tauri-apps/plugin-dialog @fontsource/inter
```
Trong `package.json`, khai báo scripts `dev: vite --host 127.0.0.1`, `check: tsc --noEmit`, `build: tsc --noEmit && vite build`, `test: vitest run`, `tauri:dev: tauri dev --config ../tauri.conf.json`, `tauri:build: tauri build --config ../tauri.conf.json`, `tauri:icon: tauri icon app-icon.svg --output ../icons`. Khai báo đầu vào Vite cho hai trang tồn tại lúc này (`index.html`, `khay.html`); Task 14 sẽ thêm `settings.html`. Expected: cài thành công, `npm ls vite` không lỗi peer.

- [ ] **Step 2: Tạo icon app và viết token theo wireframe**

Tạo SVG vuông có nền màu `--primary` và chữ M đơn giản màu trắng, không dùng asset bên ngoài; đây là icon kỹ thuật ban đầu, không phải quyết định logo cuối cùng. Chạy `cd morphix/crates/morphix-gui/ui && npm run tauri:icon`; output phải vào `crates/morphix-gui/icons/` để dùng cho bundle và khay. `styles.css` bắt đầu bằng `@import "tailwindcss";` và `@theme inline` ánh xạ token sang class.

- [ ] **Step 3: Build**

Run: `cd morphix/crates/morphix-gui/ui && npm run check && npm run build`
Expected: thành công; `dist/assets/*.css` chứa `#0f766e`.

- [ ] **Step 4: Commit**

```bash
git add morphix/crates/morphix-gui/ui
git commit -m "feat(morphix-gui): design tokens and component styles from the approved wireframe"
```

---

### Task 9: Hình học vòng tròn và bàn phím (`wheel.ts`, `keys.ts`)

**Files:** Create `ui/src/wheel.ts`, `ui/src/keys.ts`, `ui/tests/wheel.test.ts`, `ui/tests/keys.test.ts`.

**Interfaces:**
- `wheel.ts`:
  - `export const WHEEL = { size: 320, outer: 150, inner: 58, center: 160 } as const;`
  - `export function sectorPath(index: number, count: number): string`: đường SVG của múi (cung ngoài, cạnh, cung trong, `Z`). Múi 0 có tâm ở góc `-π/2` (phía trên); các múi đi theo chiều kim đồng hồ, chia đều `2π/count`.
  - `export function labelPosition(index: number, count: number): { x: number; y: number }`: tâm nhãn, bán kính `(outer + inner) / 2`.
  - `export function sectorIndexAt(x: number, y: number, count: number): number | null`: múi dưới điểm `(x, y)` trong hệ 320×320; `null` nếu trong lỗ giữa hoặc ngoài vòng.
- `keys.ts`:
  - `export function nextIndex(current: number, count: number, key: "ArrowRight" | "ArrowLeft" | "ArrowDown" | "ArrowUp" | "Tab", shift: boolean): number`: vòng lặp (wrap). Mũi tên phải/xuống và Tab tới; trái/lên và Shift+Tab lui.
  - `export function actionForKey(key: string): "close" | "activate" | "tools" | null`: `Escape → close`, `Enter` và `" "` → activate, `Alt` → tools.

- [ ] **Step 1: Write the failing tests**

`ui/tests/wheel.test.ts`:
```ts
import { describe, expect, it } from "vitest";
import { WHEEL, labelPosition, sectorIndexAt, sectorPath } from "../src/wheel";

describe("wheel geometry", () => {
  it("first sector is centred at the top", () => {
    const c = labelPosition(0, 8);
    expect(c.x).toBeCloseTo(WHEEL.center, 1);
    expect(c.y).toBeLessThan(WHEEL.center);
  });
  it("each label point maps back to its own sector", () => {
    for (const count of [3, 7]) {
      for (let i = 0; i < count; i++) {
        const p = labelPosition(i, count);
        expect(sectorIndexAt(p.x, p.y, count)).toBe(i);
      }
    }
  });
  it("the hole in the middle is not a sector", () => {
    expect(sectorIndexAt(WHEEL.center, WHEEL.center, 8)).toBeNull();
  });
  it("points outside the ring are not a sector", () => {
    expect(sectorIndexAt(0, 0, 8)).toBeNull();
  });
  it("sector path is a closed shape", () => {
    expect(sectorPath(3, 6)).toMatch(/Z$/);
  });
});
```

`ui/tests/keys.test.ts`:
```ts
import { describe, expect, it } from "vitest";
import { actionForKey, nextIndex } from "../src/keys";

describe("keyboard", () => {
  it("arrows wrap around the wheel", () => {
    expect(nextIndex(7, 8, "ArrowRight", false)).toBe(0);
    expect(nextIndex(0, 8, "ArrowLeft", false)).toBe(7);
  });
  it("tab moves forward and shift-tab backward", () => {
    expect(nextIndex(2, 5, "Tab", false)).toBe(3);
    expect(nextIndex(2, 5, "Tab", true)).toBe(1);
  });
  it("escape closes, enter and space activate, alt opens tools", () => {
    expect(actionForKey("Escape")).toBe("close");
    expect(actionForKey("Enter")).toBe("activate");
    expect(actionForKey(" ")).toBe("activate");
    expect(actionForKey("Alt")).toBe("tools");
    expect(actionForKey("a")).toBeNull();
  });
});
```

- [ ] **Step 2: Run to verify fail**

Run: `cd morphix/crates/morphix-gui/ui && npx vitest run`
Expected: FAIL (chưa có module).

- [ ] **Step 3: Implement.** Hình học thuần, không đụng DOM. `sectorIndexAt`: đổi `(x, y)` sang bán kính và góc quanh tâm, loại bán kính ngoài `[inner, outer]`, chọn `floor(((angle + π/2 + step/2) mod 2π) / step)`.

- [ ] **Step 4: Run to verify pass**

Expected: PASS (8 tests).

- [ ] **Step 5: Commit**

```bash
git add morphix/crates/morphix-gui/ui
git commit -m "feat(morphix-gui): wheel geometry and keyboard navigation"
```

---

### Task 10: Định dạng hiển thị (`format.ts`)

**Files:** Create `ui/src/format.ts`, `ui/tests/format.test.ts`.

**Interfaces:**
- `export function formatSize(bytes: number): string`: dấu phẩy thập phân, không dùng `toLocaleString` (để không phụ thuộc trình duyệt). `8,4 MB`, `412 MB`, `512 KB`. Dưới 10 MB có một chữ số thập phân; từ 100 MB trở lên không có thập phân.
- `export function summaryLine(count: number, bytes: number, noun: string): string`: `3 ảnh · 8,4 MB`.
- `export function truncateName(name: string, max: number): string`: giữ đuôi file, cắt giữa bằng `…`; đếm theo ký tự (`Array.from`), không cắt giữa ký tự Unicode nhiều code unit.

- [ ] **Step 1: Write the failing tests** (`ui/tests/format.test.ts`)

```ts
import { describe, expect, it } from "vitest";
import { formatSize, summaryLine, truncateName } from "../src/format";

describe("formatSize", () => {
  it("uses comma decimals and matches the wireframe", () => {
    expect(formatSize(8.4 * 1024 * 1024)).toBe("8,4 MB");
    expect(formatSize(412 * 1024 * 1024)).toBe("412 MB");
    expect(formatSize(512 * 1024)).toBe("512 KB");
  });
});

describe("summaryLine", () => {
  it("joins count, noun and size", () => {
    expect(summaryLine(3, 8.4 * 1024 * 1024, "ảnh")).toBe("3 ảnh · 8,4 MB");
  });
});

describe("truncateName", () => {
  it("keeps short names whole", () => {
    expect(truncateName("ngan.jpg", 20)).toBe("ngan.jpg");
  });
  it("keeps the extension of long names", () => {
    const cut = truncateName("scan-hop-dong-thue-nha-ban-cuoi-cung-final-v2.png", 24);
    expect(cut.endsWith(".png")).toBe(true);
    expect(Array.from(cut).length).toBeLessThanOrEqual(24);
  });
  it("does not split Vietnamese characters", () => {
    expect(truncateName("Ảnh chụp màn hình rất dài.jpg", 12).endsWith(".jpg")).toBe(true);
  });
});
```

- [ ] **Step 2: Run to verify fail**

Expected: FAIL.

- [ ] **Step 3: Implement.** Đếm ký tự bằng `Array.from(name)`; phần giữa thay bằng `…`.

- [ ] **Step 4: Run to verify pass**

Expected: PASS (5 tests).

- [ ] **Step 5: Commit**

```bash
git add morphix/crates/morphix-gui/ui
git commit -m "feat(morphix-gui): size, summary and name formatting"
```

---

### Task 11: Cửa sổ Menu nhanh (vòng tròn, tab, trạng thái, lệnh Tauri)

**Files:**
- Create: `ui/src/main.ts`, `crates/morphix-gui/src/commands.rs`, `src/windows.rs`, `tests/commands.rs`
- Modify: `src/lib.rs` (đăng ký lệnh và trạng thái), `src/state.rs` (trạng thái request/revision), `ui/index.html`

**Interfaces:**
- `commands.rs` (lệnh Tauri mỏng):
  - Dùng `morphix_ipc::{send_request, socket_name, MenuRequest}` trong `submit_menu_request` để nhận file từ Khay qua cùng IPC server.
  - `pub fn parse_paths(raw: Vec<String>) -> Result<Vec<PathBuf>, String>`: rỗng hoặc có phần tử chỉ là khoảng trắng → `Err`; giữ thứ tự và Unicode.
  - `#[tauri::command] fn menu_payload(paths: Vec<String>, state: State<'_, AppState>) -> Result<WheelPayload, String>` → `parse_paths` rồi `state::prepare_menu`.
  - `#[tauri::command] fn run_action(id: String, paths: Vec<String>, state: State<'_, AppState>, app: AppHandle) -> Result<u64, String>` → `menu_items::operation_for` (dùng `settings`), dựng `Job` với `quality_for(settings)` và `out_dir = settings.output_dir`, rồi `jobs.start` với callback phát sự kiện `job-update`; khi xong ghi lịch sử đúng một lần và phát `job-finished`.
  - `#[tauri::command] fn cancel_job(id: u64, state: State<'_, AppState>) -> bool`.
  - `#[tauri::command] fn pending_request(state: State<'_, AppState>) -> MenuRequestState`: danh sách file đang giữ và cờ `collecting`, để tải lại trang không mất dữ liệu hoặc trạng thái gom lô.
  - `#[tauri::command] fn discard_pending(paths: Vec<String>, revision: u64, state: State<'_, AppState>) -> Result<(), String>`: xoá yêu cầu đang giữ nếu danh sách và revision khớp; dùng khi người dùng chọn Bỏ qua.
  - `#[tauri::command] fn submit_menu_request(paths: Vec<String>) -> Result<(), String>`: parse đường dẫn rồi gửi `MenuRequest` qua `morphix_ipc::send_request(&socket_name(), …)`, để Khay, hộp thoại chọn file và CLI đi chung luồng gom lô.
  - `#[tauri::command] fn active_job(state: State<'_, AppState>) -> Option<JobSnapshot>`: snapshot việc đang chạy, để trang Khay tải đúng trạng thái ngay cả khi sự kiện `job-update` đã phát trước lúc trang đăng ký lắng nghe.
- `state.rs`: `MenuRequestState { paths: Vec<String>, collecting: bool, revision: u64 }` được serialize cho UI. Trả cùng revision từ `pending_request` và đính kèm revision vào `menu-request` để UI bỏ kết quả bất đồng bộ cũ nếu một lô mới đã tới.
- `windows.rs`: `pub fn open_menu(app: &AppHandle, paths: Vec<PathBuf>, collecting: bool)`: lưu danh sách và cờ trong `AppState`, tăng revision; request đầu tiên của mỗi lần mở đặt `first_request_at`. Lấy con trỏ từ `WebviewWindow::cursor_position()`; nếu cửa sổ đang ẩn thì định vị để tâm vòng nằm ở con trỏ, kẹp vào màn hình và đổi tọa độ physical sang logical theo scale factor. Nếu không lấy được con trỏ (Wayland) thì gọi `window.center()` và log nếu compositor từ chối định vị. Khi cửa sổ đã hiện thì chỉ cập nhật payload, không định vị lại hoặc tạo cửa sổ thứ hai. Phát `menu-request` kèm `collecting` và revision.
  - `ui/src/main.ts`:
  - Khi tải: đăng ký lắng nghe trước, gọi `pending_request`, rồi `menu_payload`; theo dõi revision lớn nhất và bỏ kết quả invoke cũ nếu `menu-request` revision mới hơn đã tới, tránh kết quả bất đồng bộ cũ ghi đè yêu cầu mới. Khi `collecting=true`, hiện chỉ báo `Đang nhận danh sách file…`, không cho chọn múi; khi nhận cập nhật cuối `collecting=false`, bật các thao tác. Vẽ vòng bằng `sectorPath`/`labelPosition`, mỗi múi là `<path class="wheel-sector" role="menuitem" tabindex="0" aria-label="…">` và nhãn `<text>`.
  - Giữa vòng: trạng thái **có dữ liệu** hiện `summaryLine` và các ảnh nhỏ; **đang chạy** hiện vòng tiến độ (SVG `stroke-dasharray`) với `%` và nút Huỷ; **rỗng**: nếu cùng loại không hỗ trợ thì `emptyUnsupported` kèm tên file, nếu nhiều loại không có thao tác chung thì `emptyMixed`; **lỗi** hiện tam giác cảnh báo, nguyên nhân, nút Thử lại và Bỏ qua. Thử lại gọi lại `menu_payload` với đúng đường dẫn đang giữ; Bỏ qua gọi `discard_pending` với paths và revision hiện tại rồi ẩn cửa sổ.
  - Bộ chuyển `segmented`: Chuyển đổi | Công cụ, đổi danh sách múi (Công cụ lọc `is_tool`).
  - Bấm múi → `run_action`; nhận `job-update` để cập nhật vòng; nhận `job-finished`: nếu không lỗi thì đóng cửa sổ sau 600 ms, nếu có lỗi thì hiện `banner-error`.
  - Phím: `actionForKey` và `nextIndex` (Task 9); `Escape` gọi `getCurrentWindow().hide()`.
  - Sự kiện `blur`: ẩn cửa sổ **chỉ khi không có việc đang chạy**.
  - Chuỗi mọi nhãn lấy từ `strings.ts`.

- [ ] **Step 1: Write the failing tests** (`tests/commands.rs`)

```rust
use morphix_gui::commands::parse_paths;

#[test]
fn parse_paths_rejects_empty_and_blank_entries() {
    assert!(parse_paths(vec![]).is_err());
    assert!(parse_paths(vec!["  ".into()]).is_err());
}

#[test]
fn parse_paths_keeps_unicode_and_order() {
    let got = parse_paths(vec!["/a/Ảnh 2.jpg".into(), "/a/Ảnh 10.jpg".into()]).unwrap();
    assert_eq!(got[0].to_string_lossy(), "/a/Ảnh 2.jpg");
    assert_eq!(got[1].to_string_lossy(), "/a/Ảnh 10.jpg");
}
```

- [ ] **Step 2: Run to verify fail**

Run: `cargo test -p morphix-gui --test commands`
Expected: FAIL.

- [ ] **Step 3: Implement** các lệnh, `windows::open_menu`, và `ui/src/main.ts`. Viết lệnh bằng `#[tauri::command]`, đăng ký trong `lib.rs` với `invoke_handler`.

- [ ] **Step 4: Verify**

Run: `cargo test -p morphix-gui --test commands && cargo build -p morphix-gui`
Run: `cd morphix/crates/morphix-gui/ui && npx vitest run && npm run check && npm run build`
Expected: PASS; build thành công.

Chạy giao diện sau khi Task 12 cung cấp lệnh mở cửa sổ. Với một ảnh JPG, tab Chuyển đổi có 7 múi (PDF gợi ý + 6 định dạng khác; JPG bị ẩn vì trùng định dạng nguồn), tab Công cụ có 3 múi. Kiểm màu nhấn theo tham số wireframe `nhan=ngoc`.

- [ ] **Step 5: Commit**

```bash
git add morphix/crates/morphix-gui
git commit -m "feat(morphix-gui): circular quick menu window wired to jobs and history"
```

---

### Task 12: Lệnh `menu` của CLI và `morphix-gui menu` (client IPC, khởi động)

**Files:** Modify `crates/morphix-cli/src/main.rs`, `crates/morphix-cli/Cargo.toml`, `crates/morphix-gui/Cargo.toml`, `crates/morphix-gui/src/main.rs`, `crates/morphix-gui/src/lib.rs`; Test `crates/morphix-cli/tests/menu.rs`.

**Interfaces:**
- Consumes: `morphix_ipc::{send_request, socket_name, MenuRequest}`. `morphix-cli` adds a normal dependency on `morphix-ipc`; it must not depend on `morphix-gui` or Tauri.
- Produces:
  - `morphix menu <FILES>... [--newline-list <TEXT>]`: `--newline-list` nhận chuỗi nhiều đường dẫn phân cách bằng `\n` cho tích hợp nào chỉ cung cấp danh sách kiểu này; Nautilus 1c ưu tiên đối số file nếu thử nghiệm xác nhận API đó. Cho phép chỉ có danh sách này hoặc kết hợp với đối số file; bỏ dòng rỗng ở cuối của CRLF nhưng từ chối nếu sau khi tách không còn đường dẫn. Gửi qua `send_request`. Không kết nối được thì khởi động `morphix-gui` (đường dẫn lấy từ `MORPHIX_GUI_BIN` nếu có, không thì binary cùng thư mục với `morphix`, có xử lý phần mở rộng `.exe` trên Windows) bằng `std::process::Command` không chờ, rồi thử kết nối lại tối đa 20 lần, mỗi lần cách 100 ms. Vẫn không được thì in `lỗi: không khởi động được Morphix (<nguyên nhân>)` ra `stderr` và thoát 1. Danh sách phân cách dòng không thể biểu diễn chính xác tên file có LF; đối số CLI trực tiếp, URI `file:` và IPC JSON giữ được tên đó. Task 19 bổ sung đầu vào URI cho Dolphin.
  - `morphix-gui menu <FILES>...`: nếu đã có server thì gửi rồi thoát; nếu chưa thì tiến trình này trở thành thể hiện nền, khởi tạo Tauri, bắt đầu server, rồi gửi yêu cầu đầu tiên bằng cùng đường gửi của IPC. Khi một thể hiện GUI khác khởi chạy đồng thời, `tauri-plugin-single-instance` chuyển yêu cầu của tiến trình thứ hai tới thể hiện đang chạy thay vì mở hai cửa sổ. Không mở console trên Windows (ghi vào ruling, được xác nhận ở 1c).
- Biến môi trường chỉ dành cho test: `MORPHIX_GUI_BIN`, `MORPHIX_IPC_NAME`.

- [ ] **Step 1: Write the failing tests** (`crates/morphix-cli/tests/menu.rs`)

```rust
use assert_cmd::cargo::cargo_bin_cmd;
use predicates::str::contains;

#[test]
fn menu_without_a_running_gui_reports_a_reason_and_exits_1() {
    let d = tempfile::tempdir().unwrap();
    let file = d.path().join("a.jpg");
    std::fs::write(&file, b"x").unwrap();
    cargo_bin_cmd!("morphix")
        .env("MORPHIX_GUI_BIN", "/không/tồn/tại/morphix-gui")
        .env("MORPHIX_IPC_NAME", format!("morphix-cli-test-{}-missing", std::process::id()))
        .env("MORPHIX_DATA_DIR", d.path().join("data"))
        .arg("menu")
        .arg(&file)
        .assert()
        .code(1)
        .stderr(contains("không khởi động được"));
}

#[test]
fn menu_accepts_a_newline_separated_list_when_no_positional_paths_are_given() {
    let d = tempfile::tempdir().unwrap();
    let list = format!(
        "{}\n{}",
        d.path().join("a.jpg").display(),
        d.path().join("b.jpg").display()
    );
    cargo_bin_cmd!("morphix")
        .env("MORPHIX_GUI_BIN", "/không/tồn/tại")
        .env("MORPHIX_IPC_NAME", format!("morphix-cli-test-{}-newline", std::process::id()))
        .env("MORPHIX_DATA_DIR", d.path().join("data"))
        .args(["menu", "--newline-list", &list])
        .assert()
        .code(1)
        .stderr(contains("không khởi động được"));
}

#[test]
fn menu_forwards_positional_and_newline_paths_to_the_running_server() {
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};

    let data = tempfile::tempdir().unwrap();
    let name = format!("morphix-cli-success-{}", std::process::id());
    let received = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&received);
    let server = morphix_ipc::IpcServer::start(
        &name,
        move |_paths| {},
        move |batch| sink.lock().unwrap().push(batch),
    ).unwrap();
    let expected = vec!["/tmp/ghi\nchú.jpg", "/tmp/Ảnh 2.png", "/tmp/a 3.jpg"];
    let list = format!("{}\n{}\n", expected[1], expected[2]);
    cargo_bin_cmd!("morphix")
        .env("MORPHIX_IPC_NAME", &name)
        .env("MORPHIX_DATA_DIR", data.path())
        .arg("menu")
        .arg(expected[0])
        .arg("--newline-list")
        .arg(list)
        .assert()
        .success();

    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(batch) = received.lock().unwrap().first() {
            assert_eq!(batch.len(), 3);
            assert_eq!(batch[0].to_string_lossy(), expected[0]);
            assert_eq!(batch[1].to_string_lossy(), expected[1]);
            assert_eq!(batch[2].to_string_lossy(), expected[2]);
            break;
        }
        assert!(Instant::now() < deadline, "server không nhận được yêu cầu");
        std::thread::sleep(Duration::from_millis(20));
    }
    server.shutdown().unwrap();
}
```

Thêm vào `crates/morphix-cli/Cargo.toml` (`[dependencies]`): `morphix-ipc = { path = "../morphix-ipc" }`. Giữ `tempfile`, `predicates`, `assert_cmd` trong `[dev-dependencies]` (đã có từ 1a); `morphix-ipc` là dependency thường nên test tích hợp dùng được. Không thêm `morphix-gui` vào dependency graph của CLI.

- [ ] **Step 2: Run to verify fail**

Run: `cargo test -p morphix-cli --test menu`
Expected: FAIL (chưa có lệnh `menu`).

- [ ] **Step 3: Implement.** Trong CLI: `Command::Menu { #[arg(required_unless_present = "newline_list")] files: Vec<PathBuf>, #[arg(long)] newline_list: Option<String> }`; ghép đường dẫn theo thứ tự positional trước rồi tới danh sách newline, chuẩn hoá CRLF, từ chối danh sách rỗng, sau đó gọi `morphix_ipc::send_request`. Khi kết nối thất bại, giữ nguyên yêu cầu, khởi chạy binary GUI, rồi thử kết nối lại. Trả `Ok` ngay sau khi server nhận đủ dòng JSON, không đợi xử lý file. Trong GUI: thêm `clap` vào Cargo.toml, phân tích subcommand `menu`; nếu server có sẵn thì gửi yêu cầu rồi thoát, nếu chưa thì truyền đường dẫn vào `run(initial_paths)`. Trong Tauri `setup`, khởi động `IpcServer` trước, đăng ký `on_pending` gọi `windows::open_menu(app, paths, true)` và `on_batch` gọi `windows::open_menu(app, paths, false)`, lưu handle trong `AppState`, rồi gửi `initial_paths` qua chính socket để đi chung luồng gom lô. `on_pending` hiện khung nhận file ngay; `on_batch` cập nhật payload cuối và bật thao tác. Đăng ký single-instance plugin theo API đã chốt ở Task 0; callback của thể hiện thứ hai bỏ `argv[0]` và subcommand `menu`, chỉ chuyển phần đường dẫn còn lại vào cùng IPC server, không mở cửa sổ riêng. Khi Tauri nhận `RunEvent::Exit`, gọi `state.jobs.shutdown()` trước, log nếu worker panic; sau đó lấy `IpcServer` khỏi state, gọi `shutdown()` và log lỗi nếu việc dừng/join thất bại.

- [ ] **Step 4: Run to verify pass**

Expected: PASS (3 tests).

Chạy thủ công: mở `morphix-gui` ở nền, rồi `morphix menu a.jpg`. Khung Menu nhanh với chỉ báo nhận file hiện trong 150 ms; sau 300 ms gom lô thì nút thao tác được bật. Log `menu shown in N ms` đo thời gian đến khi gọi `show()`.

- [ ] **Step 5: Commit**

```bash
git add morphix/crates/morphix-cli morphix/crates/morphix-gui
git commit -m "feat(morphix): menu command forwards files to the running gui or starts it"
```

---

### Task 13: Cửa sổ Khay (phương án A), khay hệ thống và lịch sử

**Files:**
- Create: `ui/src/khay.ts`, `crates/morphix-gui/src/tray.rs`
- Modify: `ui/khay.html`, `src/windows.rs` (thêm `open_khay`), `src/commands.rs` (thêm `recent_history`, `clear_history`, `quit`), `src/lib.rs`

**Interfaces:**
- Lệnh mới: `recent_history(limit: usize, state: State<'_, AppState>) -> Vec<HistoryEntry>`, `clear_history(state: State<'_, AppState>) -> Result<(), String>`, `quit(app: AppHandle) -> Result<(), String>` (gọi `app.exit(0)` để thoát cả tiến trình). Dùng lại `active_job` đã đăng ký ở Task 11.
- `tray.rs`: `pub fn build_tray(app: &AppHandle) -> tauri::Result<()>`: dùng `tauri::include_image!("./icons/32x32.png")`; bấm trái → mở hoặc ẩn cửa sổ Khay; menu phải: `show` (Hiện), `hide` (Ẩn), `settings` (Cài đặt), `quit` (Thoát), chữ lấy từ `strings.ts` (cùng nội dung ở TS và Rust qua một bảng nhỏ `TRAY_LABELS` trong `tray.rs` có comment nguồn).
- `windows.rs`: `pub fn open_khay(app)` đặt cửa sổ `khay` ở góc dưới phải của màn hình chính, rộng 400 px, bù thanh tác vụ.
- `khay.ts`:
  - Ô thả file (`drop-zone`): lắng nghe sự kiện kéo thả của Tauri (`getCurrentWebview().onDragDropEvent`) và gọi `invoke("submit_menu_request", { paths })` với danh sách file.
  - Nút "Chọn file": `open({ multiple: true })` của `@tauri-apps/plugin-dialog`, rồi gọi cùng `submit_menu_request`; lệnh Rust xác thực đường dẫn và gửi qua IPC để giữ hành vi gom lô, cập nhật cửa sổ thống nhất.
- "Gần đây": tối đa 4 dòng tổng. Trước khi hiển thị, đăng ký sự kiện `job-update` và `job-finished`, gọi `active_job` để khôi phục trạng thái đang chạy, rồi nạp `recent_history(4)` và chỉ lấp các hàng còn lại. Mỗi dòng hiện tên việc, kết quả và thời gian từ `finished_at_unix`; dòng đang chạy có thanh tiến độ. Khi nhận `job-finished`, nạp lại lịch sử. Rỗng → `emptyHistory`. Có dòng lỗi → `banner-error` trên cùng. Nút `clearHistory` gọi `clear_history` rồi nạp lại danh sách.
  - Chân: `footerPrivacy`, nút `quit` gọi lệnh thoát.
  - Trạng thái: có dữ liệu, đang tải (khung chờ đúng hình dòng), rỗng, lỗi.

- [ ] **Step 1: Write the failing test** (`tests/commands.rs`, thêm)

```rust
#[test]
fn recent_history_returns_newest_first_and_respects_limit() {
    use morphix_gui::history::{EntryStatus, History, HistoryEntry};
    let d = tempfile::tempdir().unwrap();
    let mut h = History::open(&d.path().join("history.json"));
    for n in 0..6u64 {
        h.push(HistoryEntry {
            path: format!("/tmp/file-{n}.jpg"),
            title: format!("việc {n}"),
            detail: String::new(),
            status: EntryStatus::Done,
            finished_at_unix: n,
        })
        .unwrap();
    }
    let got = morphix_gui::commands::recent_from(&h, 4);
    assert_eq!(got.len(), 4);
    assert_eq!(got[0].title, "việc 5");
}
```

(Hàm `recent_from(&History, usize) -> Vec<HistoryEntry>` là phần thuần của lệnh `recent_history`; thêm vào `commands.rs`.)

- [ ] **Step 2: Run to verify fail**

Run: `cargo test -p morphix-gui --test commands`
Expected: FAIL.

- [ ] **Step 3: Implement** lệnh, tray, cửa sổ Khay, `khay.ts`. Dùng `tauri-plugin-dialog` (đã thêm ở Task 1). Ghi vào ruling: spec không nêu cách chọn file thủ công, kế hoạch dùng hộp thoại hệ thống.

- [ ] **Step 4: Verify**

Run: `cargo test -p morphix-gui --test commands`
Run: `cd morphix/crates/morphix-gui/ui && npm run check && npm run build`
Chạy thử: bấm trái khay mở đúng vị trí; thả file vào mở vòng tròn; lịch sử hiện đúng sau khi xong việc; bấm Thoát từ khay thì app tắt hẳn.

- [ ] **Step 5: Commit**

```bash
git add morphix/crates/morphix-gui
git commit -m "feat(morphix-gui): tray icon and khay window with drop zone and recent history"
```

---

### Task 14: Cửa sổ Cài đặt

**Files:** Create `ui/settings.html`, `ui/src/settings.ts`; Modify `ui/vite.config.ts` (thêm entry trang Cài đặt), `crates/morphix-gui/tauri.conf.json` (khai báo cửa sổ ẩn `settings`), `crates/morphix-gui/capabilities/default.json` (thêm label `settings` vào phạm vi cửa sổ), `src/windows.rs` (thêm `open_settings`), `src/commands.rs` (thêm `get_settings`, `save_settings`), `src/tray.rs` (mục Cài đặt mở cửa sổ), `src/lib.rs` (đăng ký lệnh mới).

**Interfaces:**
- `commands.rs`: `pub fn save_settings_to(path: &Path, settings: Settings) -> Result<(), String>` (phần thuần: `validate` rồi `save`); `#[tauri::command] fn save_settings(settings: Settings, state: State<'_, AppState>) -> Result<(), String>` gọi hàm trên và cập nhật `AppState.settings`; `#[tauri::command] fn get_settings(state: State<'_, AppState>) -> Settings`.
- Trang Cài đặt có ba mục: **Chất lượng** (segmented: Nhỏ gọn | Cân bằng | Cao), **Thư mục lưu** (mặc định "Lưu cạnh file gốc", nút chọn thư mục bằng `open({ directory: true })`), **Tỉ lệ đổi cỡ mặc định** (ô số, 1 đến 100, mặc định 50). Nút Lưu; lỗi hiện dưới ô nhập bằng kiểu lỗi của wireframe. Thêm các chuỗi dùng tại trang này vào `strings.ts`; mở hoặc đóng trang không làm mất cửa sổ Khay hay dừng tác vụ.
- Spec (mục 4.5) chưa có wireframe cho màn này. Kiểu dùng lại: segmented của Khay và ô nhập của wireframe, ghi vào ruling.

- [ ] **Step 1: Write the failing test** (`tests/commands.rs`, thêm)

```rust
#[test]
fn save_settings_rejects_invalid_values_before_writing() {
    let d = tempfile::tempdir().unwrap();
    let path = d.path().join("settings.json");
    let bad = morphix_gui::settings::Settings {
        default_resize_percent: 0,
        ..Default::default()
    };
    assert!(morphix_gui::commands::save_settings_to(&path, bad).is_err());
    assert!(!path.exists());
}

#[test]
fn save_settings_writes_valid_values() {
    let d = tempfile::tempdir().unwrap();
    let path = d.path().join("settings.json");
    let good = morphix_gui::settings::Settings {
        default_resize_percent: 30,
        ..Default::default()
    };
    morphix_gui::commands::save_settings_to(&path, good.clone()).unwrap();
    assert_eq!(morphix_gui::settings::load(&path), good);
}
```

- [ ] **Step 2: Run to verify fail**

Run: `cargo test -p morphix-gui --test commands`
Expected: FAIL.

- [ ] **Step 3: Implement** `save_settings_to`, các lệnh, trang và entry Vite. Nạp `get_settings` khi mở; nút Lưu gọi `save_settings`, chỉ cập nhật trạng thái giao diện sau khi ghi thành công. Mở cửa sổ `settings` từ khay và từ nút bánh răng ở Khay.

- [ ] **Step 4: Verify**

Run: `cargo test -p morphix-gui --test commands`
Run: `cd morphix/crates/morphix-gui/ui && npm run check && npm run build`
Chạy thử: lưu, đóng, mở lại thấy giá trị giữ nguyên; nhập 0 thì báo lỗi và không ghi file.

- [ ] **Step 5: Commit**

```bash
git add morphix/crates/morphix-gui
git commit -m "feat(morphix-gui): minimal settings window"
```

---

### Task 15: Ghi lịch sử, đo thời gian, lỗi và tiêu chí chạy thật trên Linux

**Files:** Modify `crates/morphix-gui/src/jobs.rs`, `src/commands.rs`, `src/windows.rs`, `ui/src/main.ts`, `ui/src/khay.ts`, `ui/src/strings.ts`; Test: thêm kiểm thử cho lịch sử kết quả trong `tests/commands.rs`, định dạng dung lượng trong `tests/history.rs` và trạng thái huỷ trong `tests/jobs.rs`.

**Interfaces:**
- Khi một việc xong (từ `run_action`), ghi lịch sử đúng một lần. Tiêu đề dạng `3 ảnh → hop-dong.pdf` (tên đầu ra đầu tiên nếu có, nếu không thì tên file đầu vào đầu tiên). Chi tiết ghi thao tác, số mục thành công/thất bại, tổng byte đầu vào và tổng byte đầu ra thực tế; Rust định dạng dung lượng bằng dấu phẩy thập phân (ví dụ `8,4 MB`) qua helper thuần `format_size_vn(bytes)`. Trạng thái `Done` nếu tất cả mục thành công, `Failed` nếu có lỗi và job không bị huỷ, `Cancelled` nếu người dùng huỷ (kể cả khi có đầu ra một phần; chi tiết vẫn nêu số mục hoàn tất). Lỗi lấy dung lượng/ghi lịch sử phải được log, không được làm mất sự kiện hoàn tất.
- `windows::open_menu` ghi log `menu shown in N ms` đúng một lần khi chuyển từ ẩn sang hiện, tính từ `first_request_at` tới khi `show()` xong; cập nhật thêm đường dẫn trong cùng lô không ghi thêm thời gian mở. Sau khi cửa sổ đã hiện, xoá mốc này để lần mở kế tiếp bắt đầu mốc mới.

- [ ] **Step 1: Write the failing tests**

Trong `tests/commands.rs`, thêm test gọi phần thuần `history_entry_for_report(&Job, &JobReport) -> HistoryEntry` với một báo cáo thành công, một báo cáo có cả mục thành công lẫn thất bại, và trường hợp mọi mục lỗi. Kiểm đúng `status`, số lượng, tên output đầu tiên, `path` đầu ra (hoặc input khi không có output), tổng byte input/output; trường hợp mọi mục lỗi phải dùng tên input và không báo output. Trong `tests/history.rs`, kiểm `format_size_vn` cho `8,4 MB`, `412 MB` và `512 KB`. Trong `tests/jobs.rs`, thêm test huỷ đang chạy và xác nhận trạng thái cuối `Cancelled`, callback cuối cùng được phát đúng một lần.

- [ ] **Step 2: Run to verify fail**

Run: `cargo test -p morphix-gui --test commands --test jobs --test history`
Expected: FAIL do chưa có hàm tổng hợp history entry, trạng thái lịch sử `Cancelled` và helper định dạng dung lượng.

- [ ] **Step 3: Implement**

Thêm `EntryStatus::Cancelled` và chuỗi hiển thị tương ứng. Tách hàm thuần `history_entry_for_report(&Job, &JobReport) -> HistoryEntry` khỏi callback Tauri; callback lấy report đúng một lần, chọn path đầu ra đầu tiên hoặc đầu vào đầu tiên nếu mọi mục lỗi, chuyển path sang chuỗi lossily, ghi history rồi phát `job-finished`. Thêm `history::format_size_vn(bytes: u64) -> String`, dùng đơn vị 1024 và dấu phẩy thập phân thống nhất với wireframe. Tính phần trăm từ `done / total`, chặn mẫu số 0. Khi huỷ, phát trạng thái `Cancelled` thay vì `Finished { ok, failed }`; khi cửa sổ Menu nhanh đóng, chỉ ẩn cửa sổ, không huỷ job.

- [ ] **Step 4: Kiểm chạy thật (bắt buộc, máy Linux có màn hình)**

1. Từ `crates/morphix-gui/ui`, chạy `npm run tauri:build`; nếu máy đã cài đủ bundler tương ứng thì tạo AppImage và `.deb` trong workspace `target/release/bundle/`.
2. Cài `.deb`. Khởi động `morphix-gui` ở nền. Chạy `morphix menu a.jpg`; khung nhận file phải hiện trong ≤150 ms khi thể hiện đã chạy, rồi danh sách thao tác bật sau 300 ms. Log có `menu shown in N ms`.
3. Gộp 3 ảnh bằng vòng tròn. Ra đúng một PDF cạnh file gốc, file gốc không đổi.
4. Đóng vòng tròn giữa lúc đang chạy: việc tiếp tục chạy, lịch sử ghi đúng kết quả cuối.
5. Mở vòng tròn bằng hai lệnh `morphix menu` liên tiếp trong 100 ms: chỉ một cửa sổ, khung nhận file hiện ngay, danh sách cuối có đủ file và bật trong khoảng 300 ms kể từ yêu cầu đầu.
6. Chạy trên cả phiên X11 và Wayland; ghi hành vi định vị. Trên Wayland gọi `window.center()` làm fallback và ghi vị trí thực tế nếu compositor không áp dụng yêu cầu.
7. Trên GNOME, ghi lại icon khay có hiện không và phiên bản extension AppIndicator nếu có. Không sửa `morphix doctor` trong đợt 1b; kiểm tra extension và hướng dẫn cài thuộc kế hoạch tích hợp hệ điều hành 1c.
8. Log không có lỗi bất ngờ.

- [ ] **Step 5: Sửa lỗi tìm thấy**, mỗi lỗi logic thuần có test đỏ trước khi sửa; mỗi lỗi giao diện có log và bước tái hiện.

- [ ] **Step 6: Verify**

Run: `cargo fmt --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace`
Run: `cd crates/morphix-gui/ui && npx vitest run && npm run build`
Expected: tất cả sạch.

- [ ] **Step 7: Commit**

```bash
git add morphix/crates/morphix-gui morphix/crates/morphix-cli
git commit -m "feat(morphix-gui): record history, time menu opening, linux acceptance fixes"
```

---

## Đợt 1c: Tích hợp hệ điều hành

**Mục tiêu:** nối menu chuột phải và khởi động cùng máy vào GUI đã hoàn tất ở Tasks 0–15. Tích hợp per-user, cài/gỡ lặp lại an toàn, không cần quyền admin, không ghi đè mục của ứng dụng khác và không đụng tới file đầu vào, lịch sử, cài đặt hay dữ liệu người dùng.

**Phạm vi:** Windows Explorer menu kiểu cũ, Nautilus Scripts, Dolphin service menu, XDG autostart, `morphix install-integration`, `morphix uninstall-integration`, đường mở Khay bằng CLI khi icon khay không khả dụng, và mở rộng `morphix doctor`. Nemo, Thunar, menu đầu Windows 11, phím tắt toàn cục, tự cài/bật GNOME extension và tích hợp nguồn mạng để sau.

**Tài liệu nền tảng đã đối chiếu:** [GNOME Help: Nautilus scripts](https://help.gnome.org/users/nautilus-behavior.html), [KDE Developer: Dolphin service menus](https://develop.kde.org/docs/apps/dolphin/service-menus/), [Microsoft: Shell verb selection model](https://learn.microsoft.com/en-us/windows/win32/shell/how-to-employ-the-verb-selection-model), [Microsoft: Shell verbs](https://learn.microsoft.com/en-us/windows/win32/shell/fa-verbs), [XDG Autostart Specification](https://specifications.freedesktop.org/autostart-spec/latest/). Chi tiết phụ thuộc phiên bản hoặc hành vi thực tế phải qua Task 16 trước khi chốt.

### Task 16: Spike xác minh hợp đồng Windows, Nautilus, Dolphin và autostart

**Mục đích:** kiểm chứng ranh giới hệ điều hành trước khi cài đặt thật. Kết quả và quyết định ghi vào Ruling ledger của chính file này; không tạo file kế hoạch 1c khác.

**Files:** không sửa mã sản phẩm. Dùng probe tạm trong thư mục build/temp và profile người dùng tạm; dọn sạch sau mỗi phép thử. Nếu cần giữ probe để chạy tự động, đặt trong test fixture không ghi vào registry hoặc thư mục desktop thật.

- [ ] **Step 1: Kiểm Windows verb và multiselect.** Dùng chương trình probe chỉ in argv dạng JSON. Đăng ký verb thử trong nhánh HKCU riêng có hậu tố process/test, không dùng key Morphix. Thử 1, 2, 15, 16, 100 và 101 file giả dưới thư mục tạm; tên có khoảng trắng, tiếng Việt, dấu nháy và `&`. Kiểm `MultiSelectModel=Player`, command string, thứ tự tham số, số tiến trình và hành vi ở giới hạn lựa chọn. Gỡ key thử trong cleanup kể cả khi probe lỗi. Trên Windows 10/11 với tài khoản thường, xác nhận vị trí menu; trên Windows 11 ghi rõ **Show more options**, không cần admin. Xác nhận `morphix` vẫn có console còn `morphix-gui` chạy nền không nháy console. Không coi tài liệu về `Player` là bằng chứng placeholder `%1`/`%*` truyền đúng cả danh sách.
- [ ] **Step 2: Kiểm Nautilus.** Đọc tài liệu GNOME: script người dùng nằm trong `scripts`, hiện dưới submenu **Scripts**, file được chọn truyền cho script như tham số; một số nguồn remote không có tham số. Trên phiên GNOME mục tiêu, xác minh argv hay biến môi trường thực tế; thử 1/nhiều file, khoảng trắng, Unicode, LF nếu hỗ trợ, file remote/không có file local. Dùng profile hoặc thư mục thử riêng, không ghi đè script người dùng.
- [ ] **Step 3: Kiểm Dolphin.** Thử `.desktop` tại `$XDG_DATA_HOME/kio/servicemenus` (mặc định `~/.local/share/kio/servicemenus`) với `Type=Service`, `MimeType=image/*;`, `Actions=...` và `[Desktop Action ...]`. Kiểm `%U` khi chọn nhiều ảnh, URL `file:`, executable bit, lúc Dolphin thấy mục cài/gỡ và `desktop-file-validate`. Ghi riêng nếu KDE Frameworks cũ có đường dẫn khác.
- [ ] **Step 4: Kiểm autostart và GNOME tray.** Xác minh autostart `.desktop` dưới `$XDG_CONFIG_HOME/autostart` (mặc định `~/.config/autostart`), `TryExec`/`Hidden`, một lần đăng nhập chỉ tạo một GUI. Trên GNOME ghi phiên bản shell, AppIndicator extension cài/bật hay không, icon có hiện không. Không tự cài/bật extension. Thử mở app khi thiếu extension để kiểm đường lùi `morphix tray`.
- [ ] **Step 5: Ghi kết quả.** Cập nhật ruling về Windows placeholder, Nautilus argv/env, Dolphin `%U`, executable bit, cache/refresh, XDG path và tray. Chi tiết chưa đo được phải giữ nhãn `Chưa kiểm` và thành tiêu chí acceptance; không tự giả định hành vi.

**Lưu ý kỹ thuật:** Microsoft mô tả `Player` cho verb xử lý nhiều mục và nêu giới hạn menu kiểu cũ; vẫn phải đo argv thật. GNOME đặt script trong submenu **Scripts** và truyền lựa chọn dưới dạng tham số; với file remote có thể không truyền tham số. KDE mô tả `%U` là danh sách URL, đường dẫn service menu và executable bit cho menu local. Menu đầu Windows 11 qua `IExplorerCommand`/sparse package/signing ở ngoài phạm vi.

---

### Task 17: Lớp quản lý tích hợp, báo cáo ổn định và CLI

**Files:** Create `crates/morphix-cli/src/integration/mod.rs`, `src/integration/report.rs`, `tests/integration.rs`; Modify `crates/morphix-cli/src/main.rs`, `Cargo.toml`, `src/output.rs`. Thêm dependency theo target ở Tasks 18/19; CLI không kéo Tauri vào.

**Interfaces:**

- `IntegrationOptions { dry_run: bool, install_autostart: bool }`; autostart mặc định bật, `--no-autostart` chỉ dùng khi cài.
- `ComponentId`: `windows_context_menu`, `nautilus_script`, `dolphin_service_menu`, `autostart`, `tray_support`.
- `ComponentStatus`: `installed`, `already_installed`, `removed`, `already_absent`, `missing`, `stale`, `conflict`, `unsupported`, `unknown`, `not_applicable`, `failed`; `OverallStatus`: `success`, `partial`, `failure`, `dry_run`.
- `IntegrationComponentReport { id, status, changed, path, message }`; `IntegrationReport { platform, overall, components }`. Tên trường JSON ổn định, message cho người dùng bằng tiếng Việt.
- `install(options)`, `uninstall(options)`, `inspect()` trả report; lỗi từng thành phần được báo trong report để cài/gỡ một phần có mã thoát 3. Lỗi parse CLI không được biến thành thành công.
- CLI: `morphix install-integration [--no-autostart] [--json] [--dry-run]`; `morphix uninstall-integration [--json] [--dry-run]`; `morphix doctor [--json]`. Install/uninstall dùng mã thoát spec: 0 thành công hoặc đã ở trạng thái đích, 1 lỗi toàn bộ/không thể tiếp tục, 2 dùng sai lệnh, 3 thành công một phần. `doctor` chỉ đọc; trả 0 khi mọi mục áp dụng khỏe, 1 khi có lỗi/mục bắt buộc thiếu, 3 khi một phần phép kiểm không thể kết luận, 2 khi dùng sai lệnh.

- [ ] **Step 1: Viết test đỏ cho report và CLI.** Inject XDG root, registry adapter giả và đường dẫn binary. Kiểm JSON schema, `dry-run` không ghi file/key, `--no-autostart` bỏ đúng thành phần, mã thoát 0/1/2/3, và `doctor` giữ kiểm tra `ffmpeg`/data dir từ 1a rồi bổ sung trạng thái integration.
- [ ] **Step 2: Chạy test đỏ.** `cargo test -p morphix-cli --test integration` phải FAIL do report/backend/lệnh chưa tồn tại.
- [ ] **Step 3: Implement lớp quản lý.** `main.rs` chỉ parse, gọi hàm, map output. Dùng `Command` argv riêng; không dùng `sh -c`, `cmd /c` hoặc chuỗi lệnh ghép. Tìm `morphix` bằng `current_exe()` và GUI companion theo layout gói đã chốt, hỗ trợ `.exe`; dùng path tuyệt đối, không phụ thuộc CWD. Xác nhận binary tồn tại trước khi cài mục menu/startup; binary bị di chuyển được báo `stale` để người dùng cài lại.
- [ ] **Step 4: Bảo đảm ownership và tính lặp an toàn.** Ghi file tạm cùng filesystem rồi rename; đặt quyền executable trước khi công bố script/menu. Target tồn tại nhưng không có marker Morphix thì báo `conflict`, giữ nguyên byte/quyền. Install lặp idempotent. Uninstall chỉ xoá file/key có marker và đúng ownership; xoá thư mục do app tạo chỉ khi rỗng; không đụng `data_dir()`. Nếu bước sau lỗi, rollback entry mới do lần cài này tạo ra khi có thể.
- [ ] **Step 5: Verify backend cô lập.** Test cài/gỡ hai lần, dry-run, conflict preservation, rollback, binary path có spaces/quote, JSON parse và exit code. Automated test phải inject mọi root; không ghi vào profile hoặc key sản phẩm thật.
- [ ] **Step 6: Commit.** `git add morphix/crates/morphix-cli && git commit -m "feat(morphix-cli): add integration management and reports"`.

---

### Task 18: Menu chuột phải và khởi động cùng máy trên Windows

**Files:** Create `crates/morphix-cli/src/integration/windows.rs`; Modify target-specific dependencies trong `crates/morphix-cli/Cargo.toml`, `src/integration/mod.rs`, `tests/integration.rs`. Bao toàn bộ trong `#[cfg(target_os = "windows")]`.

**Hợp đồng:**

- Verb per-user dưới `HKCU\Software\Classes\SystemFileAssociations\image\shell\Morphix` (đổi đúng class nếu probe Task 16 chứng minh Explorer cần class khác), tên hiển thị tiếng Việt, `MultiSelectModel=Player`.
- Command chạy `morphix menu` với CLI path tuyệt đối và placeholder multiselect đã xác minh ở Task 16. Quote executable đúng quy tắc Shell; không tự ghép path file do người dùng chọn ngoài placeholder Explorer. Hỗ trợ path cài app có khoảng trắng.
- Autostart dùng giá trị riêng Morphix dưới `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`, trỏ `morphix-gui` tuyệt đối đã quote. Không dùng HKLM, không yêu cầu admin.
- Ownership marker riêng cho verb/Run value. Gỡ chỉ key/value Morphix; không xoá parent `Software\Classes`/`Run` hoặc value khác.
- Sau cài/gỡ, gọi refresh Explorer đã được xác minh ở Task 16 (ví dụ `SHChangeNotify(SHCNE_ASSOCCHANGED)` nếu cần). Nếu refresh lỗi, ghi report partial thay vì báo thành công toàn bộ.

- [ ] **Step 1: Viết test đỏ trên Windows bằng registry root test.** `install_windows_at(registry_root, executable_paths)` phải dùng nhánh HKCU dành riêng cho test, không chạm key thật. Kiểm value verb/command, quote, `Player`, Run value, install/uninstall idempotent, ownership, giữ value ngoài ứng dụng và báo conflict. Guard cleanup xoá nhánh test dù assert fail.
- [ ] **Step 2: Chạy test đỏ.** Trên runner Windows: `cargo test -p morphix-cli --test integration` phải FAIL. Linux chỉ chạy test platform-neutral và đánh dấu Windows suite pending; không giả lập PASS Windows trên Linux.
- [ ] **Step 3: Implement backend.** Dùng `winreg` target-specific; kiểm lỗi từng registry operation, rollback entry mới nếu bước sau lỗi. Tách command encoder thành hàm thuần để test path space, Unicode, quote. Dùng ownership check chung cho inspect/uninstall.
- [ ] **Step 4: Verify tự động và thật.** Runner Windows chạy fmt/clippy/test. Máy Windows 10/11 tài khoản thường: cài, xác nhận menu/autostart; chọn 1, 2, 15, 16, 100 file giả và so argv theo ruling Task 16; kiểm chỉ một GUI; gỡ và xác nhận key/value Morphix mất, app data còn nguyên. Windows 11 phải ghi chú **Show more options**. Không triển khai sparse package, `IExplorerCommand` hay signing.
- [ ] **Step 5: Commit.** `git add morphix/crates/morphix-cli && git commit -m "feat(morphix-cli): register per-user Windows integration"`.

---

### Task 19: Nautilus Scripts, Dolphin service menu và XDG autostart

**Files:** Create `crates/morphix-cli/src/integration/linux.rs`; Modify `crates/morphix-cli/src/integration/mod.rs`, `crates/morphix-cli/src/main.rs`, `crates/morphix-cli/Cargo.toml` (direct dependency on `url` for URL parsing), `crates/morphix-cli/tests/integration.rs`, `crates/morphix-cli/tests/menu.rs`.

**Đường dẫn per-user:** dùng `$XDG_DATA_HOME` hoặc `~/.local/share` cho Nautilus/Dolphin, `$XDG_CONFIG_HOME` hoặc `~/.config` cho autostart. Dùng `OsString`/`PathBuf`, không giả định `HOME` luôn có. Chỉ xoá thư mục cha khi rỗng.

**Nautilus:** cài script ổn định trong `nautilus/scripts/`, mode `0755`, marker `Managed by Morphix`. Gọi CLI path tuyệt đối đã escape POSIX: `exec '<morphix path>' menu -- "$@"`; không nội suy file path vào script, không `eval`. Task 16 quyết định argv hay env. Nếu chỉ có biến newline, dùng `--newline-list` và ghi giới hạn tên có LF. Không có file local thì không gửi request rỗng im lặng: phát chẩn đoán rõ ra stderr/log; sau Task 20, có thể mở Khay bằng `morphix tray` để người dùng chọn file local. Mục nằm dưới **Scripts**, không phải menu cấp đầu.

**Dolphin:** `.desktop` trong `kio/servicemenus/`, quyền file theo ruling Task 16/KDE; khai báo `Type=Service`, `MimeType=image/*;`, `Actions=Morphix;`, `[Desktop Action Morphix]`. Dùng `%U` và đầu vào mới `morphix menu --uris <URI>...`. Parse URL `file:` bằng URL parser sang `PathBuf`, giữ Unicode và percent-encoding; từ chối scheme remote/malformed với lỗi rõ. Không replace URL bằng tay, không gọi shell. Chạy `desktop-file-validate` trước khi cài.

**Autostart:** `.desktop` trong `$XDG_CONFIG_HOME/autostart/` với `Type=Application`, `Name`, `Exec` GUI tuyệt đối, `TryExec`, `Terminal=false` và ownership marker. Theo cú pháp Desktop Entry; không nhúng shell. Mặc định cài; `--no-autostart` bỏ qua. Single-instance Task 12 ngăn GUI trùng sau đăng nhập.

- [ ] **Step 1: Viết test đỏ.** Inject XDG roots; kiểm mặc định/override, Nautilus shell quoting với binary path có spaces/apostrophe, không nội suy input, mode 0755, Dolphin `%U`, Desktop Entry escaping (`\\`, newline, quote), autostart fields, và `--uris` chuyển `file:///tmp/a%20b.png`, Unicode, `%23`, `+` chính xác thành `PathBuf`. Remote/malformed URI và danh sách rỗng bị từ chối; conflict không ghi file.
- [ ] **Step 2: Chạy test đỏ.** `cargo test -p morphix-cli --test integration --test menu` phải FAIL do adapter/serializer/`--uris` chưa có.
- [ ] **Step 3: Implement.** `--uris` loại trừ positional files và `--newline-list`; cuối cùng mọi kiểu input thành `Vec<PathBuf>` đi qua `send_request`. Serialize Desktop Entry và POSIX script theo helper đã test; ghi tạm rồi rename, đặt mode trước khi công bố.
- [ ] **Step 4: Verify.** Chạy `desktop-file-validate`; install/uninstall hai lần dưới XDG temp roots; xác nhận marker, mode, idempotency và chỉ xoá thư mục rỗng. Trên GNOME/Nautilus và KDE Plasma 6/Dolphin, thử 1 và 40 ảnh giả tiếng Việt/khoảng trắng, kiểm danh sách đến đúng vòng tròn và menu ở đúng submenu; đăng nhập thử xác nhận một GUI. Nếu refresh menu cần thao tác, chỉ ghi lệnh đã kiểm chứng Task 16.
- [ ] **Step 5: Commit.** `git add morphix/crates/morphix-cli && git commit -m "feat(morphix-cli): add Nautilus Dolphin and XDG integration"`.

---

### Task 20: `morphix tray` làm đường mở Khay và mở rộng `morphix doctor`

**Files:** Modify `crates/morphix-cli/src/main.rs`, `crates/morphix-cli/tests/integration.rs`, `crates/morphix-cli/tests/menu.rs`, `crates/morphix-gui/src/main.rs`, `crates/morphix-gui/src/lib.rs`, `crates/morphix-gui/src/windows.rs`, `crates/morphix-gui/tests/commands.rs`.

`morphix tray` mở Khay khi GNOME thiếu AppIndicator hoặc người dùng không muốn dùng icon. CLI spawn `morphix-gui --show-khay` bằng path tuyệt đối, không chờ. Nếu chưa chạy, GUI mới mở Khay sau setup; nếu đã chạy, callback single-instance gọi `windows::open_khay(app)`. Cờ được phân nhánh trước khi argv bị coi là đường dẫn file; request `menu` hiện tại vẫn đi qua IPC. Không thêm message type IPC mới.

`morphix doctor` giữ kiểm tra `ffmpeg`/data dir từ 1a và bổ sung trạng thái menu, autostart, binary path, ownership/conflict, executable mode, tính hợp lệ `.desktop` nếu có validator, và GNOME AppIndicator ở mức best-effort. Thiếu `gnome-extensions`, phiên GNOME/D-Bus hoặc dữ liệu không đủ thì báo `unknown`, không đoán `missing`. Không dùng shell; tool desktop được gọi trực tiếp, có timeout, ghi stderr/exit status vào report. `doctor` chỉ đọc, không cài/bật extension hay sửa integration.

- [ ] **Step 1: Viết test đỏ.** CLI dùng `MORPHIX_GUI_BIN`/IPC test hiện có để xác nhận `tray` truyền đúng `--show-khay`, path có spaces không tách, không chờ child, lỗi thiếu GUI trả thông báo/mã 1. GUI helper phân biệt startup `menu` với `--show-khay`, callback mở Khay đúng một lần. Doctor test `installed`, `missing`, `stale`, `conflict`, `unsupported`, `unknown`, JSON parse được và snapshot file/key không đổi sau khi gọi.
- [ ] **Step 2: Chạy test đỏ.** `cargo test -p morphix-cli --test integration --test menu` và `cargo test -p morphix-gui --test commands` phải FAIL vì lệnh/callback/trường mới chưa có.
- [ ] **Step 3: Implement.** Dùng helper spawn chung nhưng `tray` không mở socket/không gửi file. Trong Tauri callback, kiểm `--show-khay` trước; route menu cũ giữ nguyên. Mở/focus cửa sổ Khay hiện có, không tạo bản thứ hai. Doctor tổng hợp report, giữ `--json` và mã thoát; hệ không áp dụng trả `not_applicable`/`unsupported`, không tính là lỗi.
- [ ] **Step 4: Verify.** Test CLI/GUI; gọi `morphix tray` hai lần khi app chạy xác nhận một tiến trình và Khay được focus. Trên GNOME có/không extension, doctor nêu trạng thái hoặc unknown, còn tray fallback vẫn mở Khay. `doctor --json` không đổi dữ liệu.
- [ ] **Step 5: Commit.** `git add morphix/crates/morphix-cli morphix/crates/morphix-gui && git commit -m "feat(morphix): add tray fallback and integration diagnostics"`.

---

### Task 21: Acceptance, uninstall sạch và kiểm tra phát hành cho đợt 1c

**Files:** Modify `crates/morphix-cli/tests/integration.rs`, `crates/morphix-cli/tests/menu.rs` và tài liệu cài đặt hiện có nếu có. Không tạo plan riêng cho 1c.

- [ ] **Step 1: Automated tests trong profile cô lập.** Linux XDG roots và Windows registry root đều nằm trong fixture riêng. Kiểm idempotency, dry-run, `--no-autostart`, conflict preservation, rollback, JSON/exit codes, URI `%U`, newline fallback, quyền file và marker. Snapshot trước/sau xác nhận uninstall không xoá gì ngoài tài sản Morphix. Automated test không đăng ký vào key sản phẩm hay thư mục home thật.
- [ ] **Step 2: Chạy acceptance bằng tài khoản thử trên OS tương ứng.** (1) Windows 10/11, không admin: cài menu/startup, thử 1/2/15/16/100 ảnh giả, path cài có spaces, argv theo Task 16 và **Show more options** trên Windows 11. (2) GNOME/Nautilus: thử 1/40 ảnh, argv/env theo ruling, Unicode/khoảng trắng, submenu **Scripts**, trường hợp remote không có argv. (3) KDE Plasma 6/Dolphin: thử 1/40 ảnh, `%U → --uris → PathBuf`, submenu và `desktop-file-validate`. (4) Linux GNOME/KDE: đăng xuất/đăng nhập, xác nhận một GUI; `morphix tray` mở Khay khi app chạy và chưa chạy. (5) Doctor ở trạng thái installed/missing/stale/conflict và GNOME thiếu tiện ích; thông báo đúng hoặc `unknown`. (6) Gỡ, xác nhận menu/startup Morphix mất, file/key khác và data/history/settings/log còn nguyên; gỡ lần hai không lỗi.
- [ ] **Step 3: Kiểm đa nền tảng và log.** Linux và Windows runner lần lượt chạy `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`; chạy `npx vitest run && npm run build` trong `crates/morphix-gui/ui`. Ghi test nào chỉ chạy được trên OS cụ thể; không ghi PASS thay cho máy chưa kiểm.
- [ ] **Step 4: Sửa lỗi acceptance.** Lỗi parser/path/registry/installer phải có test đỏ hoặc fixture tái hiện. Nếu chưa tái hiện được, ghi OS, desktop, phiên bản, bước, log và pending; không thêm workaround đoán.
- [ ] **Step 5: Kiểm cuối.** `git diff --check`, review diff, đọc log test, kiểm `git status`; xác nhận test installer chỉ ghi vào profile/registry root thử. Workspace sạch chỉ là điều kiện trước commit; không xoá thay đổi người dùng để làm sạch.
- [ ] **Step 6: Commit.** `git add morphix/crates/morphix-cli morphix/crates/morphix-gui morphix/docs && git commit -m "feat(morphix): complete per-user OS integration"`.

---

## Ruling ledger

- **Ruling:** Giao thức IPC ở crate `morphix-ipc` dùng chung giữa CLI và GUI. CLI không phụ thuộc Tauri hoặc WebKitGTK để lệnh `morphix menu` vẫn chạy trên máy thiếu môi trường giao diện.
- **Ruling:** Để cùng đạt mục tiêu hiện cửa sổ trong 150 ms và gom yêu cầu trong 300 ms, giao diện hiện khung `Đang nhận danh sách file…` ngay khi có request đầu; khóa thao tác cho đến khi lô cuối sẵn sàng.
- **Ruling:** `--newline-list` không biểu diễn tên file có LF. Nautilus dùng argv khi phiên bản mục tiêu xác nhận; Dolphin dùng URL `file:` đã parse; newline list là fallback với giới hạn được báo rõ.
- **Ruling:** Cài đặt đợt 1b chỉ có chất lượng, thư mục đầu ra và tỉ lệ đổi cỡ. Integration 1c dùng cấu hình per-user riêng, không thêm preference ẩn vào màn Cài đặt.
- **Ruling:** Lịch sử, cài đặt và log dùng `morphix_core::data_dir()` theo hợp đồng 1a (`MORPHIX_DATA_DIR` ghi đè); integration dùng XDG user dirs/registry HKCU, không dùng `data_dir()` làm thư mục menu.
- **Ruling:** Màn Cài đặt chưa có wireframe; dùng segmented và control theo wireframe hiện có. Icon chữ M nền xanh ngọc là tài sản kỹ thuật tạm thời, không chốt logo thương hiệu.
- **Ruling:** Windows verb per-user, legacy `MultiSelectModel=Player`; Windows 11 ở **Show more options**. Placeholder chỉ chốt theo probe Task 16; không làm sparse package, `IExplorerCommand` hay signing.
- **Ruling:** Nautilus nằm trong submenu **Scripts**; Dolphin là service menu per-user dùng `%U`/`--uris` cho URL local; không tuyên bố menu cấp đầu.
- **Ruling:** Autostart cài mặc định, có `--no-autostart`; uninstall chỉ gỡ entry có ownership marker và không xoá dữ liệu Morphix hoặc file/key của ứng dụng khác.
- **Ruling:** `morphix tray` mở Khay không cần icon tray. Doctor chỉ kết luận trạng thái GNOME extension khi có đủ dữ liệu; không cài/bật extension và trả `unknown` khi chưa thể kết luận.
- **Ruling:** Task 16 là cổng xác minh; placeholder/path/API còn `Chưa kiểm` chưa được coi là đã chốt nếu chưa thử trên OS mục tiêu.
- **Ruling (Task 19):** Linux per-user: Nautilus `$XDG_DATA_HOME/nautilus/scripts/Morphix` (0755, `exec '<morphix>' menu -- "$@"`, argv); Dolphin `$XDG_DATA_HOME/kio/servicemenus/morphix.desktop` (0755, `menu --uris %U`); autostart `$XDG_CONFIG_HOME/autostart/morphix.desktop` (0644). XDG rỗng hoặc tương đối bị bỏ qua, rơi về `$HOME`.
- **Ruling (Task 19):** Thư mục do lần cài tạo ra được đánh dấu bằng `.morphix-created`; gỡ chỉ xoá ngược lên các thư mục còn đúng marker và rỗng. Thư mục có sẵn (kể cả rỗng) được giữ nguyên.
- **Ruling (Task 19):** Marker của Morphix chấp nhận thêm dạng `# Managed by Morphix` vì script shell không thể có dòng marker trần trong hai dòng đầu mà vẫn chạy đúng. Vẫn yêu cầu nguyên dòng marker, nên câu chứa cụm từ không khớp; đây là nới rule Task 17 có chủ đích.
- **Ruling (Task 19):** `morphix menu --uris <URI>...` loại trừ đường dẫn positional và `--newline-list`; URI được parse bằng crate `url` (dependency trực tiếp), chỉ nhận `file:` cục bộ, từ chối scheme remote, URI sai và danh sách rỗng. `--newline-list` không đổi.
- **Ruling (Task 19):** Dolphin `Type=Service` giữ theo brief, nhưng `desktop-file-validate` 0.27 báo lỗi `MimeType` và `Actions` vì chỉ hợp lệ với `Type=Application`. Đây là khoá mở rộng KDE, và các service menu có sẵn trên máy cũng bị báo tương tự. Chưa có cách giữ `Type=Service` mà validator đạt exit 0. `Chưa kiểm` trên Dolphin thật.
- **Chưa kiểm (Task 19):** Nautilus nhận argv hay env trên GNOME thật; refresh menu Nautilus và Dolphin; Dolphin mode 0755 có cần thiết không; Dolphin chưa cài nên `%U` với nhiều ảnh chưa đo; GUI chạy sau đăng nhập chỉ một lần; LF trong tên file qua Nautilus argv chưa đo trên GNOME.
- **Ruling (Task 19):** Dolphin giữ `Type=Service` (KDE docs: bốn khoá bắt buộc là `Type=Service`, `MimeType`, `Actions`, `[Desktop Action ...]`) và thêm `ServiceTypes=KonqPopupMenu/Plugin` theo KDE docs cho KDE Frameworks cũ (dưới 5.85); thay cho `X-KDE-ServiceTypes`. `desktop-file-validate` 0.27 chỉ còn báo lỗi `MimeType` và `Actions` vì khoá đó bị giới hạn cho `Type=Application`; đây là giới hạn của validator đã đối chiếu với KDE docs, không đổi `Type`.
- **Chưa kiểm (Task 19):** hành vi Dolphin thật: Dolphin chưa được cài trong môi trường này nên chưa xác nhận menu hiện và `%U` hoạt động.
- **Ruling (Task 19):** Thư mục do Morphix tạo chỉ được xoá khi có `.morphix-created` (ownership marker) và rỗng khi gỡ; thư mục không có marker được giữ nguyên.

- **Ruling (Task 18):** Verb Windows nằm tại `HKCU\Software\Classes\SystemFileAssociations\image\shell\Morphix`, `MUIVerb` là `Mở bằng Morphix`, `MultiSelectModel=Player`, command con `command` chạy `"<morphix tuyệt đối>" menu "%1"`. Quote theo quy tắc CommandLineToArgvW; chỉ placeholder Explorer được đưa vào command, không nối tên file. Marker sở hữu là value `MorphixOwner` trong chính key verb.
- **Ruling (Task 18):** Autostart là value `Morphix` dưới `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`, data là đường dẫn `morphix-gui` tuyệt đối đã quote, không có argument. Marker `AutostartOwner` đặt ở `HKCU\Software\Morphix`, không đặt trong Run vì mọi value trong Run được chạy khi đăng nhập. Không HKLM, không admin.
- **Ruling (Task 18):** Gỡ chỉ xoá key verb `...\shell\Morphix` và value `Morphix` cùng marker; `Software\Morphix` chỉ bị xoá khi đã rỗng; không xoá `Software\Classes`, `shell`, `Run` hoặc value khác. Entry không có marker đúng là `conflict` và giữ nguyên.
- **Ruling (Task 18):** Rollback chỉ phạm vi một thành phần: key/marker mới do lần cài tạo ra bị xoá nếu bước sau lỗi, dùng `Transaction::defer`. Nếu thành phần sau lỗi, thành phần trước vẫn giữ và report là `partial` (mã thoát 3), theo cách Linux đã làm.
- **Chưa kiểm (Task 18):** Placeholder multiselect `%1` là mặc định của kế hoạch, chưa đo. Task 16 chưa chạy, nên chưa biết Explorer với `Player` có truyền đủ 1/2/15/16/100 file qua `%1` hay cần `%*`, và số tiến trình được tạo. Chưa xác nhận argv thật.
- **Chưa kiểm (Task 18):** Refresh Explorer: chưa gọi `SHChangeNotify` vì chưa có probe xác minh. Nếu verb không xuất hiện ngay, cần mở lại cửa sổ Explorer hoặc đăng xuất; đây là tiêu chí acceptance trên Windows thật.
- **Chưa kiểm (Task 18):** Windows 11 **Show more options** và vị trí menu trên Windows 10/11 tài khoản thường; chưa chạy suite registry HKCU trên Windows runner (xem danh sách test pending trong báo cáo Task 18).
- **Chưa kiểm (Task 18):** Đường rollback khi bước sau lỗi chưa có test ép lỗi trên registry thật; chỉ có Transaction được test trên Linux.
- **Ruling (Task 18, sửa 2):** Quyền sở hữu Windows gắn với đúng giá trị Morphix đã ghi: verb lưu command trong `MorphixOwner`, autostart lưu chuỗi Run trong `AutostartOwner`. Giá trị hiện tại khác bản ghi, hoặc có giá trị mà không có bản ghi, là `conflict` và không bị ghi đè hay xoá; không còn giá trị thì là `absent`. Cài ghi value trước rồi record; gỡ xoá value trước rồi record.
- **Chưa kiểm (Task 18, sửa 2, hoãn):** Gỡ từng phần: nếu xoá marker lỗi sau khi value Run đã xoá, báo `changed: false` dù value đã mất. Chưa sửa; cần báo `changed: true` khi value đã bị xoá.
- **Chưa kiểm (Task 18, sửa 2, hoãn):** Rollback chỉ xoá key leaf; `...\SystemFileAssociations\image\shell` do lần cài tạo ra vẫn còn khi lỗi. Chưa có test ép lỗi trên registry thật.
- **Ruling (Task 18, sửa 2):** Từ chối `%` trong đường dẫn cài là chủ ý vì Explorer thay `%1` trong command bằng file đã chọn; không phải vì Windows mở rộng `%TÊN%` (REG_SZ không được mở rộng). Comment trong code đã sửa theo lý do này.
- **Chưa kiểm (Task 18, sửa 2, hoãn):** Đường dẫn chứa `"` chưa bị từ chối; `quote_windows_arg` escape `"` thành `\"`, không đúng với CreateProcess nếu đến được. NTFS cấm `"` trong tên nên chưa gặp; sẽ từ chối trong `absolute_utf8` khi sửa.
- **Chưa kiểm (Task 18, sửa 2, hoãn):** Explorer chưa được làm mới sau khi cài verb; thông báo `đã cài` chưa nói có thể cần đăng xuất. Là tiêu chí acceptance trên Windows thật.
- **Chưa kiểm (Task 18, sửa 2):** Nếu ghi record lỗi sau khi value Run đã ghi, value để lại không có record và lần cài sau báo `conflict`; chưa có rollback value vì cần trạng thái trước đó.

## Sau kế hoạch này

- Đợt 1b (Tasks 0–15) và 1c (Tasks 16–21) cùng nằm trong file này; không tạo plan riêng cho 1c.
- Đợt 2 đến 5: chưa lập kế hoạch, theo spec mục 9.
- Cần kiểm trên máy tương ứng trước khi tuyên bố hoàn thành: vị trí Menu nhanh trên Wayland, AppIndicator trên GNOME mục tiêu, `interprocess` named pipe trên Windows, Windows Shell multiselect, Nautilus argv/env và Dolphin `%U`/refresh.
