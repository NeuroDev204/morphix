# Menu desktop GNOME qua curl, bỏ .deb: Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Lệnh curl cài menu Morphix một cấp trên desktop GNOME (DING) bằng một bản chép DING có UUID riêng, tự kiểm khi đăng nhập; bỏ hẳn gói .deb.

**Architecture:** `morphix install-integration` (thành phần `DesktopContextMenu`, nay cài được ở chế độ người dùng) chép DING nguồn vào
`$XDG_DATA_HOME/gnome-shell/extensions/morphix-desktop@morphix`, chèn 3 dòng gọi vào 3 file DING theo mốc chuỗi, thêm 2 file JS của Morphix,
cài autostart `morphix desktop-check`. Lệnh kiểm khi đăng nhập đổi DING gốc ↔ bản chép bằng `gnome-extensions`, ghi trạng thái trong
`$XDG_DATA_HOME/morphix/desktop-state.json`. CI/Release/website bỏ .deb.

**Tech Stack:** Rust (morphix-cli integration, `ToolRunner` giả trong test), GJS (DING app: legacy `imports`, Gtk3; extension.js: ESM), node:test cho phần JS thuần, GitHub Actions, Next.js.

**Spec:** `docs/superpowers/specs/2026-11-27-morphix-desktop-menu-curl-only-design.md`

## Global Constraints

- UUID bản chép: `morphix-desktop@morphix`; tên: `Desktop Icons (Morphix)`; UUID gốc: `ding@rastersoft.com`.
- Không sudo, không ghi ngoài `$HOME`. Không bao giờ để `ding@rastersoft.com` và `morphix-desktop@morphix` cùng bật.
- Lúc cài **không** đụng gsettings; đổi chỉ ở `desktop-check` (lần đăng nhập sau).
- Danh sách việc: `<morphix> --json menu --list --uris <uri...>` → `{"items":[{"id","label","is_tool"}],"too_many":bool}`; chạy: `<morphix> menu --action <id> --uris <uri...>`;
  lựa chọn không hợp lệ: `<morphix> tray --unsupported-selection`; list lỗi/timeout: `<morphix> menu --uris ...`; vùng trống: `<morphix> tray`.
  Hằng số giống `integration/nautilus/morphix_menu.py`: timeout 0.3 s, tối đa 100 file, nhãn `Chọn quá nhiều file (tối đa 100)`.
- Chữ hiển thị tiếng Việt. Commit kết thúc bằng `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.

## Mốc chèn (đã kiểm DING 24.04 `46+really47.0.9-1ubuntu5` và 26.04 `50.26.04.8ubuntu`)

| File | Mốc (chèn **sau** dòng này; thử lần lượt, mốc đầu xuất hiện đúng 1 lần thắng) | Dòng chèn |
|---|---|---|
| `app/fileItemMenu.js` | `this._menu = DesktopIconsUtil.createDesktopMenu(['fileitemmenu']);` (26.04) · `menuStyleContext.add_class('fileitemmenu');` (24.04) | `imports.morphixMenu.addFileItems(this, fileItem);` |
| `app/desktopManager.js` | `this._menu.add(selectAll);` (cả hai) | `imports.morphixMenu.addBackgroundItem(this._menu);` |
| `extension.js` | `import {Extension} from 'resource:///org/gnome/shell/extensions/extension.js';` | `import * as MorphixWatch from './morphixWatch.js';` |
| `extension.js` | dòng `    enable() {` | `        MorphixWatch.start();` |
| `extension.js` | dòng `    disable() {` | `        MorphixWatch.stop();` |

## Review Focus

1. Lựa chọn trên desktop có thư mục, file `.desktop`, ổ đĩa (isSpecial) → mục "Morphix" mở `tray --unsupported-selection`, không chạy `--list`.
2. Tên file có dấu cách / tiếng Việt / ký tự đặc biệt → URI đi nguyên vẹn qua argv (không qua shell).
3. `desktop-check` chạy khi không có phiên D-Bus / không phải GNOME (SSH, KDE) → không làm gì, thoát 0.
4. `desktop-state.json` hỏng hoặc thiếu → coi như chưa đổi lần nào, không panic.
5. Nguồn DING là chính bản chép (người dùng chỉ có DING trong `$XDG_DATA_HOME` cùng tên thư mục khác) → không bao giờ chọn `morphix-desktop@morphix` làm nguồn.

---

### Task 1: JS của Morphix cho bản chép

**Files:**
- Create: `integration/ding/morphixMenu.js` (chạy trong tiến trình `ding.js`, legacy `imports`, Gtk3)
- Create: `integration/ding/morphixWatch.js` (ESM, chạy trong GNOME Shell)
- Test: `integration/tests/ding_menu.test.mjs`
- Modify: `.github/workflows/ci.yml` (thêm `node --test integration/tests/` cạnh bước python của `integration/tests`)

**Interfaces:**
- Produces (`morphixMenu.js`, chuỗi `@MORPHIX@` được Task 2 thay bằng đường dẫn binary đã escape JSON):
  - `var MORPHIX = "@MORPHIX@";`
  - thuần (không chạm `imports.gi` khi gọi): `commandForSelection(uris, allRegularLocal)`, `listCommand(uris)`, `actionCommand(id, uris)`, `parseListing(stdout)` → `{tooMany, items} | null`
  - `addFileItems(fileItemMenu, fileItem)`: lấy `fileItemMenu._desktopManager.getCurrentSelection(false)` (rỗng → `[fileItem]`); kiểm từng mục (`isSpecial`, `isDirectory`, `file.is_native()`, kiểu REGULAR) như bản vá cũ; >100 file → submenu một mục mờ; gọi list bằng `Gio.Subprocess` với argv `['timeout', '0.3', ...listCommand(uris)]`, `communicate_utf8(null, null)`; rỗng → không thêm gì; thêm `Gtk.MenuItem` "Morphix" + submenu vào `fileItemMenu._menu` rồi `fileItemMenu._addSeparator()`.
  - `addBackgroundItem(menu)`: `Gtk.MenuItem` "Morphix" → `tray`.
  - Spawn bằng `Gio.Subprocess.new(argv, Gio.SubprocessFlags.NONE)` trong try/catch, lỗi → `log`.
- Produces (`morphixWatch.js`): `export function start()`, `export function stop()`: theo dõi `Gio.Settings({schema_id: 'org.gnome.shell'})` `changed::enabled-extensions`;
  khi danh sách có `ding@rastersoft.com` → (trong `GLib.idle_add`) bỏ `morphix-desktop@morphix` khỏi `enabled-extensions`, thêm vào `disabled-extensions`. `stop()` ngắt tín hiệu.

- [ ] **Step 1: Viết test node** nạp `morphixMenu.js` bằng `vm.runInNewContext` với `imports = {}` giả, rồi:
```js
test('regular local files list actions', () => assert.deepEqual(ctx.listCommand(['file:///a b.png']), ['@MORPHIX@','--json','menu','--list','--uris','file:///a b.png']));
test('invalid selection opens khay with error', () => assert.deepEqual(ctx.commandForSelection(['file:///d'], false), ['@MORPHIX@','tray','--unsupported-selection']));
test('empty selection opens khay', () => assert.deepEqual(ctx.commandForSelection([], true), ['@MORPHIX@','tray']));
test('action command', () => assert.deepEqual(ctx.actionCommand('image:webp',['file:///x.png']), ['@MORPHIX@','menu','--action','image:webp','--uris','file:///x.png']));
test('listing parses', () => assert.deepEqual(ctx.parseListing('{"items":[{"id":"a","label":"A","is_tool":false}],"too_many":false}'), {tooMany:false, items:[{id:'a',label:'A',isTool:false}]}));
test('broken listing is null', () => { for (const s of ['', 'x', '{}', '{"items":[{"id":1}],"too_many":false}']) assert.equal(ctx.parseListing(s), null); });
```
- [ ] **Step 2:** `node --test integration/tests/` → FAIL (file không tồn tại).
- [ ] **Step 3:** Viết 2 file JS theo Interfaces.
- [ ] **Step 4:** `node --test integration/tests/` → PASS; `node --check` không áp cho legacy imports, nên thêm kiểm cú pháp: `gjs -c 'imports.searchPath.unshift("integration/ding"); imports.morphixMenu;'` nếu có gjs (máy dev có) → không lỗi; `node --check integration/ding/morphixWatch.js` (ESM, đặt tạm tên .mjs nếu cần) → OK.
- [ ] **Step 5: Commit** `feat(desktop): add Morphix menu scripts for the DING copy`

### Task 2: Dựng bản chép DING (Rust)

**Files:**
- Create: `crates/morphix-cli/src/integration/ding_fork.rs` (khai `pub mod ding_fork;` trong `integration/mod.rs`)
- Test: unit test trong cùng file

**Interfaces:**
- Consumes: 2 file JS của Task 1 qua `include_str!("../../../../integration/ding/...")`.
- Produces:
  - `pub const FORK_UUID: &str = "morphix-desktop@morphix"; pub const DING_UUID: &str = "ding@rastersoft.com";`
  - `pub fn find_source(data_home: &Path, data_dirs: &[PathBuf]) -> Option<PathBuf>`: thư mục `<dir>/gnome-shell/extensions/ding@rastersoft.com` đầu tiên có `metadata.json`, theo thứ tự data_home rồi data_dirs.
  - `pub fn fingerprint(source: &Path) -> Result<String, String>`: sha256 hex của nối `metadata.json` + `extension.js` + `app/fileItemMenu.js` + `app/desktopManager.js`.
  - `pub fn inject(file: &str, anchors: &[&str], line: &str) -> Result<String, String>`: mốc đầu tiên xuất hiện đúng 1 lần → chèn `\n` + line ngay sau **dòng** chứa mốc; mốc 0 hoặc >1 lần ở mọi ứng viên → `Err("DING bản này chưa hỗ trợ: không thấy mốc ...")`.
  - `pub fn build_fork(source: &Path, extensions_dir: &Path, morphix: &Path, morphix_version: &str) -> Result<PathBuf, String>`: các bước 1–6 của spec; ghi `morphix-source.json` = `{"source": <path>, "fingerprint": <hex>, "morphix_version": <ver>}`; thay `@MORPHIX@` bằng đường dẫn đã escape; đổi tên thư mục tạm → đích (xoá đích cũ trước đó chỉ khi nó có `morphix-source.json`); lỗi → xoá thư mục tạm, đích cũ nguyên vẹn.
  - `pub fn read_marker(fork: &Path) -> Option<ForkMarker>` với `pub struct ForkMarker { pub source: PathBuf, pub fingerprint: String, pub morphix_version: String }`.

- [ ] **Step 1: Viết test** (thư mục tạm với DING giả gồm 4 file chứa đúng mốc 26.04; một bộ chứa mốc 24.04):
  - `inject_uses_the_first_anchor_present_once`, `inject_fails_when_anchor_missing_or_repeated`
  - `build_fork_rewrites_uuid_and_name_and_keeps_shell_version` (đọc metadata.json: uuid, name, shell-version giữ nguyên)
  - `build_fork_inserts_calls_for_2404_and_2604_layouts` (mỗi file đích chứa đúng 1 lần dòng chèn)
  - `build_fork_embeds_binary_path_with_spaces` (`/home/a b/.local/bin/morphix` xuất hiện dạng JSON trong `app/morphixMenu.js`, không còn `@MORPHIX@`)
  - `build_fork_failure_keeps_previous_fork` (nguồn thiếu mốc → Err, thư mục đích cũ + marker còn nguyên, không còn thư mục tạm)
  - `find_source_prefers_data_home_and_never_returns_the_fork`
  - `real_system_ding_is_supported`: nếu `/usr/share/gnome-shell/extensions/ding@rastersoft.com` tồn tại thì `build_fork` vào thư mục tạm phải Ok (không có thì return sớm).
- [ ] **Step 2:** `cargo test -p morphix-cli ding_fork` → FAIL (không biên dịch).
- [ ] **Step 3:** Viết `ding_fork.rs` theo Interfaces (dùng `sha2` nếu crate đã có trong workspace, nếu không dùng `sha256sum`? — kiểm `Cargo.toml` của morphix-cli; chỉ thêm dep khi workspace đã có).
- [ ] **Step 4:** `cargo test -p morphix-cli ding_fork` → PASS.
- [ ] **Step 5: Commit** `feat(desktop): build a Morphix copy of DING from the installed one`

### Task 3: Đổi/khôi phục và kiểm khi đăng nhập (Rust)

**Files:**
- Create: `crates/morphix-cli/src/integration/desktop_switch.rs`
- Test: unit test trong cùng file (ToolRunner giả ghi lại lệnh)

**Interfaces:**
- Consumes: Task 2 (`find_source`, `fingerprint`, `build_fork`, `read_marker`, hằng UUID).
- Produces:
  - `#[derive(Serialize, Deserialize, Default)] pub struct DesktopState { pub switched: bool, pub ding_was_enabled: bool, pub fallback: Option<Fallback> }`, `pub struct Fallback { pub fingerprint: String, pub morphix_version: String, pub reason: String }`; file `$XDG_DATA_HOME/morphix/desktop-state.json`; đọc hỏng → `Default`.
  - `pub enum CheckOutcome { NotApplicable(String), Switched, Rebuilt, FellBack(String), Nothing }`
  - `pub fn desktop_check(ctx: &DesktopCtx, tools: &ToolRunner, wait: &dyn Fn(Duration)) -> CheckOutcome`; `DesktopCtx { data_home, data_dirs, session: SessionEnv, morphix: PathBuf, version: String }`.
  - `pub fn restore(ctx: &DesktopCtx, tools: &ToolRunner) -> Result<(), String>` (dùng cho gỡ).
  - Truy vấn: `gnome-extensions info <uuid>` (dòng `State:` / `Enabled:`), `gnome-extensions enable|disable <uuid>`, `gdbus call --session --dest org.freedesktop.DBus --object-path /org/freedesktop/DBus --method org.freedesktop.DBus.NameHasOwner com.rastersoft.ding` → `(true,)`.
- Luật (theo spec + mục "Quyết định của người viết"):
  1. Không GNOME / không D-Bus / không `gnome-extensions` → `NotApplicable`.
  2. Không có bản chép → `Nothing`.
  3. Nguồn mất → tắt bản chép nếu bật, bật lại DING nếu `ding_was_enabled` → `FellBack`.
  4. `fallback` có và fingerprint + version **không đổi** → `Nothing`.
  5. Marker khác fingerprint hoặc version → `build_fork`; lỗi → bước 7 với lý do; ok → tiếp.
  6. Chưa `switched`: DING gốc không bật → `Nothing` (người dùng đã tắt desktop icon); bật → ghi state (`ding_was_enabled=true`, `switched=true`, xoá `fallback`) rồi `disable ding`, `enable fork` → `Switched`. Nếu `gnome-extensions info fork` thất bại (GNOME chưa thấy bản chép: vừa cài, chưa đăng nhập lại) → `Nothing`.
  7. Đã `switched` và bản chép đang bật: chờ tối đa 10 s (`wait` 1 s mỗi vòng) cho `State: ACTIVE` **và** `NameHasOwner(com.rastersoft.ding)`; không đạt → `disable fork`, `enable ding`, ghi `fallback` (fingerprint, version, lý do), `notify-send` một lần → `FellBack`.
  8. Đã `switched` mà bản chép không bật → `Nothing` (người dùng chọn DING gốc hoặc tự tắt).
- [ ] **Step 1: Viết test** (mỗi luật một test, tên theo luật): `not_gnome_is_not_applicable`, `first_check_switches_when_ding_enabled` (thứ tự lệnh: disable ding trước enable fork), `first_check_respects_ding_disabled_by_user`, `unknown_fork_before_relogin_does_nothing`, `dead_fork_falls_back_and_records_reason`, `fallback_not_retried_until_source_or_version_changes`, `fallback_retried_after_version_change`, `user_opt_out_is_respected`, `corrupt_state_file_is_treated_as_fresh`, `restore_reenables_ding_only_if_it_was_enabled`.
- [ ] **Step 2:** `cargo test -p morphix-cli desktop_switch` → FAIL.
- [ ] **Step 3:** Viết `desktop_switch.rs`.
- [ ] **Step 4:** `cargo test -p morphix-cli desktop_switch` → PASS.
- [ ] **Step 5: Commit** `feat(desktop): switch to the Morphix DING copy at login and fall back on failure`

### Task 4: Nối vào install-integration / uninstall / doctor / CLI

**Files:**
- Modify: `crates/morphix-cli/src/integration/mod.rs` (thêm `ComponentId::DesktopContextMenu` vào `INSTALLABLE_COMPONENTS`, sửa kích thước mảng)
- Modify: `crates/morphix-cli/src/integration/linux.rs` (`desktop_menu_not_applicable`, nhánh `DesktopContextMenu` trong install/uninstall/inspect_component; autostart thứ hai `morphix-desktop-check.desktop` với `Exec=<morphix> desktop-check`)
- Modify: `crates/morphix-cli/src/integration/windows.rs` nếu nó không trả `Unsupported`/`NotApplicable` cho thành phần này
- Modify: `crates/morphix-cli/src/main.rs` (lệnh con ẩn `DesktopCheck` → `desktop_switch::desktop_check`, luôn thoát 0, in kết quả một dòng)
- Delete/Modify: `crates/morphix-cli/src/integration/desktop.rs`: bỏ `inspect_desktop`, `SOURCE_MANIFEST`, phần dpkg-divert; giữ `inspect_package_file` nếu `linux.rs` còn gọi
- Test: `crates/morphix-cli/tests/integration.rs`, unit test `linux.rs`

**Interfaces:**
- Consumes: Task 2, Task 3.
- Hành vi:
  - install (chế độ User): không GNOME hoặc `find_source` None → `NotApplicable` ("không có desktop icon GNOME (DING) trên máy"); `build_fork` lỗi → `Failed` kèm thông báo; ok → `Installed`/`Updated`, message "Đăng xuất rồi đăng nhập lại để thấy menu Morphix trên desktop" khi trước đó chưa có bản chép; luôn ghi autostart kiểm; **không** gọi gsettings/gnome-extensions. Chạy lại install xoá `fallback` trong state (spec: cài lại luôn thử lại).
  - uninstall: `desktop_switch::restore`, xoá bản chép (chỉ khi có `morphix-source.json`), xoá autostart kiểm, xoá state.
  - inspect (doctor): nguồn DING ở đâu; bản chép có/không, marker khớp nguồn+version không; `gnome-extensions info` của bản chép.
- [ ] **Step 1: Sửa/viết test:** đổi `user_integration_lifecycle_excludes_only_the_desktop_menu` thành `user_integration_lifecycle_includes_the_desktop_menu_when_ding_exists` (DING giả trong data_dirs tạm qua env `XDG_DATA_DIRS`); thêm `install_without_ding_reports_not_applicable`, `install_does_not_touch_gsettings` (ToolRunner giả panic nếu gặp `gsettings`/`gnome-extensions enable`), `uninstall_removes_fork_autostart_and_state`.
- [ ] **Step 2:** `cargo test -p morphix-cli` → các test mới FAIL.
- [ ] **Step 3:** Nối code theo Hành vi.
- [ ] **Step 4:** `cargo test -p morphix-cli && cargo clippy --workspace --all-targets -- -D warnings` → PASS, 0 cảnh báo.
- [ ] **Step 5: Commit** `feat(desktop): install the desktop menu from install-integration`

### Task 5: Bỏ .deb

**Files:**
- Modify: `.github/workflows/release.yml` (xoá job `deb`; `publish.needs` bỏ `deb`; bỏ bước download `deb` và `test -f dist/morphix_*_amd64.deb.sha256`)
- Modify: `.github/workflows/ci.yml` (sửa chú thích dòng 28 nhắc .deb)
- Delete: `scripts/build-deb.sh`, `scripts/check-deb.sh`, `scripts/prepare-ding.py`, `scripts/render-dpkg-scripts.py`, `packaging/deb/`, `crates/morphix-gui/packaging/deb/`, `integration/ding/morphix-menu.patch`, `integration/ding/source.json`, `docs/ubuntu-install.md`, và test python của các script này trong `scripts/tests` (grep tên file).
- Modify: `crates/morphix-gui/tauri.conf.json` `bundle.targets`: bỏ `"deb"` (kiểm `build-tarball.sh` không dùng bundle trước khi sửa)
- Modify: `README.md`, `README.txt` (bỏ dòng .deb; thêm "Đã cài bằng .deb trước đây: `sudo apt remove morphix` rồi chạy lệnh curl"; thêm dòng menu desktop GNOME cần đăng nhập lại, muốn ẩn icon thì tắt "Desktop Icons (Morphix)" trong ứng dụng Extensions)
- Modify: `website/src/components/DownloadMatrix.tsx` (cột Linux chỉ còn lệnh curl + badge; bỏ "Cách 2"), `website/src/constants/downloads.ts` (bỏ `deb`, `tarball`, `nameDeb`, `sizeDeb`, `nameTarball`, `sizeTarball` nếu không còn ai dùng; `os` sửa "Ubuntu 22.04+, Debian 12+, Fedora, Arch, openSUSE (x86_64)")
- Giữ: `integration/ding/LICENSE` (giấy phép DING, bản chép là GPL-3).

- [ ] **Step 1:** `git grep -n -i -E 'build-deb|check-deb|prepare-ding|render-dpkg|morphix-menu.patch|source.json|ubuntu-install.md|\.deb'` → ghi danh sách chỗ còn nhắc.
- [ ] **Step 2:** Xoá/sửa theo Files; chạy lại grep → chỉ còn `install.sh` (đoạn chặn bản .deb cũ), CHANGELOG/specs cũ.
- [ ] **Step 3:** `python3 -m unittest discover -s scripts/tests && python3 -m unittest discover -s integration/tests && node --test integration/tests/ && cargo test --workspace` → PASS; `(cd website && npm test && npm run build)` → PASS; `python3 -c "import yaml,sys; yaml.safe_load(open('.github/workflows/release.yml'))"` → OK.
- [ ] **Step 4: Commit** `chore: drop the .deb package; Linux installs with the curl script only`

### Task 6: Kiểm trên máy thật (cần chủ máy)

- [ ] **Step 1:** Trả máy về trạng thái gốc: `gnome-extensions disable morphix-desktop@morphix`, xoá `~/.local/share/gnome-shell/extensions/morphix-desktop@morphix` (bản thử, không có `morphix-source.json`), bỏ `morphix-desktop@morphix` khỏi `enabled-extensions`, đưa `ding@rastersoft.com` vào `disabled-extensions`. Hỏi chủ máy có bật DING để thử không.
- [ ] **Step 2:** `cargo build --release -p morphix-cli`, chép vào `~/.local/bin/morphix` (như install.sh), `morphix install-integration` → dòng `desktop_context_menu` báo đã cài + nhắc đăng nhập lại.
- [ ] **Step 3:** Chủ máy đăng nhập lại → `journalctl --user -b | grep -i morphix` thấy desktop-check `Switched`; chuột phải file desktop → Morphix → các việc; bấm một việc → file kết quả cạnh file gốc.
- [ ] **Step 4:** `morphix uninstall-integration` → DING gốc trở lại đúng trạng thái trước; ghi kết quả vào ledger.
