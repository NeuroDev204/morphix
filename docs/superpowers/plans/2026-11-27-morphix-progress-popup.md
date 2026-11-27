# Popup tiến trình: Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Việc chạy từ menu chuột phải hiện một popup nhỏ góc dưới phải có tiến trình, nút Huỷ, kết quả.

**Architecture:** Cửa sổ Tauri `tien-trinh` khai sẵn (ẩn) trong `tauri.conf.json` (tránh tạo cửa sổ từ luồng việc, xem chú thích `windows.rs`).
`JobSnapshot` thêm `from_menu` và `first_name`. Luồng việc, sau 1 s nếu việc (từ menu) còn chạy, gọi `windows::show_progress`.
Trang `progress.html` nghe `job-update`/`job-finished`, hiển thị theo một hàm thuần `progressView` và tự đóng theo luật.

**Tech Stack:** Rust + Tauri 2, TypeScript + Vite + vitest (`crates/morphix-gui/ui`).

**Spec:** `docs/superpowers/specs/2026-11-27-morphix-progress-popup-design.md`

## Global Constraints

- Chỉ việc `notify = true` (từ menu) có popup. Trễ hiện: **1 s**. Tự đóng: xong **4 s** (rê chuột giữ), huỷ **2 s**, lỗi **không tự đóng**.
- `total == 1` → thanh không xác định, chữ `Đang xử lý…`; `total > 1` → `done/total file` và %.
- Góc dưới phải (dùng lại `bottom_right_origin`), luôn trên cùng, không focus, không trên thanh tác vụ, không khung. Kích thước logic 360×120.
- Giữ thông báo hệ thống như cũ. Chữ tiếng Việt.

## Review Focus

1. Việc xong trong < 1 s → popup không bao giờ hiện (kể cả khi luồng hẹn giờ tỉnh dậy sau khi việc kết thúc).
2. Hai việc liên tiếp từ menu: popup đang đếm 4 s đóng của việc 1 thì việc 2 bắt đầu → hẹn giờ đóng bị huỷ, hiện việc 2.
3. Việc chạy từ Khay khi popup đang hiện → popup bỏ qua sự kiện `from_menu == false`.
4. Tên file rất dài / tiếng Việt → cắt bằng CSS (ellipsis), không vỡ bố cục.
5. App đang thoát khi luồng hẹn giờ tỉnh → không panic, không hiện cửa sổ.

---

### Task 1: Dữ liệu snapshot

**Files:**
- Modify: `crates/morphix-gui/src/jobs.rs` (`JobSnapshot` thêm `pub from_menu: bool`, `pub first_name: String`; `JobRunner::start(job, label, from_menu: bool, on_update)`; `first_name` = tên file (không đường dẫn) của input đầu tiên của `job`, rỗng nếu không có)
- Modify: `crates/morphix-gui/src/commands.rs` (`start_action` truyền `notify` làm `from_menu`)
- Modify: `crates/morphix-gui/ui/src/recent.ts` (`JobSnapshot` thêm `from_menu: boolean; first_name: string`)
- Test: `crates/morphix-gui/tests/jobs.rs`

- [ ] **Step 1:** Test `snapshot_carries_menu_origin_and_first_file_name`: start với `from_menu = true`, job 2 input `/tmp/x/ảnh 1.png`, `/tmp/x/b.png` → mọi snapshot nhận được có `from_menu == true`, `first_name == "ảnh 1.png"`.
- [ ] **Step 2:** `cargo test -p morphix-gui --test jobs` → FAIL (không biên dịch).
- [ ] **Step 3:** Sửa theo Files; sửa mọi chỗ gọi `JobRunner::start` (`git grep -n 'jobs.start\|\.start(' crates/morphix-gui`).
- [ ] **Step 4:** `cargo test -p morphix-gui` → PASS.
- [ ] **Step 5: Commit** `feat(gui): tag job snapshots with menu origin and first file name`

### Task 2: Giao diện popup (thuần + trang)

**Files:**
- Create: `crates/morphix-gui/ui/src/progress-view.ts`, `crates/morphix-gui/ui/progress.html`, `crates/morphix-gui/ui/src/progress.ts`
- Modify: `crates/morphix-gui/ui/vite.config.ts` (thêm input `progress`)
- Test: `crates/morphix-gui/ui/tests/progress-view.test.ts`

**Interfaces:**
- `export interface ProgressView { title: string; fileLine: string; percent: number | null; status: "running" | "done" | "failed" | "cancelled"; detail: string; closeAfterMs: number | null; showOpenFolder: boolean; showCancel: boolean }`
- `export function progressView(snapshot: JobSnapshot, latest: HistoryEntry | null): ProgressView`
  - title `Morphix · <label>`; fileLine: `<first_name>` + (total > 1 ? ` (<done>/<total> file)` : "") + (total > 2 ? `, và <total-1> file khác` chỉ khi running? — không: chỉ `(done/total file)`); percent: total ≤ 1 → `null`, ngược lại `progressPercent(done,total)`.
  - running: showCancel true, closeAfterMs null; finished failed == 0: status done, detail `Xong`, showOpenFolder = latest?.path có, closeAfterMs 4000; failed > 0: status failed, detail = `latest.detail`, closeAfterMs null; cancelled: detail `Đã huỷ`, closeAfterMs 2000.
- `progress.ts`: nghe `job-update`/`job-finished` (bỏ qua `!from_menu`); khi finished gọi `invoke("recent_history",{limit:1})`; nút Huỷ → `cancel_job`; Mở thư mục → `open_output_directory`; ✕ và hết giờ → `invoke("hide_progress")`; `mouseenter` dừng đếm, `mouseleave` đếm lại từ đầu; sự kiện `job-update` mới → huỷ hẹn giờ đóng.
- [ ] **Step 1:** Test vitest: `running_multi_file_shows_count_and_percent` (done 2 total 5 → fileLine `a.mov (2/5 file)`, percent 40), `single_file_is_indeterminate` (percent null), `success_closes_after_4s_with_open_folder`, `failure_stays_with_reason`, `cancel_closes_after_2s`.
- [ ] **Step 2:** `npm test -- progress-view` (trong `crates/morphix-gui/ui`) → FAIL.
- [ ] **Step 3:** Viết `progress-view.ts`, rồi `progress.html` + `progress.ts` (Tailwind như `khay.html`; thanh không xác định bằng animation CSS; tên file `truncate`).
- [ ] **Step 4:** `npm run check && npm test && npm run build` → PASS, `dist/progress.html` tồn tại.
- [ ] **Step 5: Commit** `feat(gui): add the progress popup page`

### Task 3: Cửa sổ và hẹn giờ hiện

**Files:**
- Modify: `crates/morphix-gui/tauri.conf.json` (cửa sổ `tien-trinh`: url `progress.html`, 360×120, `decorations:false`, `alwaysOnTop:true`, `skipTaskbar:true`, `focus:false`, `visible:false`, `resizable:false`)
- Modify: `crates/morphix-gui/src/windows.rs` (`pub const PROGRESS_WINDOW_LABEL: &str = "tien-trinh";` `pub fn show_progress(app) -> Result<(), String>` đặt góc dưới phải như `open_khay` nhưng **không** `set_focus`; `pub fn hide_progress(app)`; `close_hides_window` thêm nhãn này)
- Modify: `crates/morphix-gui/src/commands.rs` (lệnh `hide_progress`; trong `publish_update`, snapshot Running đầu tiên của việc `notify` → `std::thread::spawn` ngủ 1 s rồi: nếu `state.jobs.active()` còn đúng `id` và app chưa thoát → `show_progress`)
- Modify: `crates/morphix-gui/src/lib.rs` (đăng ký `commands::hide_progress`); capability/permission của cửa sổ mới nếu `capabilities/` liệt kê theo nhãn cửa sổ (grep `"khay"` trong `crates/morphix-gui/capabilities`)
- Test: `crates/morphix-gui/src/state.rs` hoặc test thuần mới

**Interfaces:**
- Hàm thuần để test: `pub fn should_show_progress(started_id: u64, active: Option<&JobSnapshot>, exiting: bool) -> bool` (true khi active.id == started_id && active.from_menu && !exiting).
- "Snapshot Running đầu tiên": `JobRunner` không phát snapshot lúc bắt đầu, nên hẹn giờ đặt trong `start_action` ngay sau `state.jobs.start` trả `Ok(id)` khi `notify`.
- [ ] **Step 1:** Test `progress_shows_only_for_the_same_running_menu_job` (4 ca: cùng id/menu → true; id khác → false; không menu → false; đang thoát → false).
- [ ] **Step 2:** `cargo test -p morphix-gui` → FAIL.
- [ ] **Step 3:** Viết theo Files.
- [ ] **Step 4:** `cargo test -p morphix-gui && cargo clippy --workspace --all-targets -- -D warnings` → PASS.
- [ ] **Step 5:** Thử tay: `morphix menu --action <id> --uris file://<video>` với một video vài chục MB → sau ~1 s popup hiện góc dưới phải, thanh chạy, Huỷ dừng việc; việc ảnh nhỏ → không popup. Ghi kết quả (kể cả nếu Wayland đặt sai vị trí) vào ledger.
- [ ] **Step 6: Commit** `feat(gui): show the progress popup for menu jobs that run over a second`
