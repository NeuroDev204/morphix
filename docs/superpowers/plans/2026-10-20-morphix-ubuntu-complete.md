# Morphix Ubuntu 24.04 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Phát hành gói .deb Morphix cho Ubuntu 24.04, thêm mục trực tiếp vào menu Files/Desktop và cung cấp app chọn/kéo thả file chuyển đổi cục bộ.

**Architecture:** Giữ core Rust, CLI, IPC và GUI Tauri hiện có. Nautilus dùng MenuProvider API 4; Desktop dùng hai file DING phiên bản hóa do gói Morphix sở hữu, có `dpkg-divert` và phụ thuộc đúng phiên bản DING Ubuntu. CLI integration kiểm tra trạng thái; package manager cài/gỡ Desktop với quyền quản trị. Khay trở thành cửa sổ app có launcher.

**Tech Stack:** Rust/Cargo, Tauri 2, TypeScript/Vite/Vitest, Python 3/PyGObject/Nautilus 4, GJS/GTK của DING, Debian packaging.

**Spec:** `docs/superpowers/specs/2026-10-20-morphix-ubuntu-complete-design.md` — phạm vi trải nghiệm, Native và DING package-level đã được người dùng duyệt.

## Global Constraints

- Ubuntu 24.04 GNOME 46, .deb amd64; nghiệm thu cả Wayland và X11.
- Giữ menu mặc định, thêm đúng một mục Morphix trong menu Files và Desktop, gồm file và nền.
- Không mở rộng sang menu của mọi ứng dụng. Không thêm định dạng hoặc công cụ chỉnh sửa mới trong đợt này.
- DING system patch chỉ cài khi package/version/hash khớp, không có local modification hoặc diversion khác; dùng `dpkg-divert` để giữ và phục hồi bản gốc. Maintainer scripts không ghi vào HOME của root.
- Ràng buộc `Depends` đúng phiên bản DING được hỗ trợ; cập nhật DING đi cùng bản Morphix tương thích, không tự vá version lạ.
- Không tự khởi động lại Shell hoặc giết phiên desktop; hướng dẫn đăng xuất/đăng nhập khi cần.
- Giữ quy tắc không ghi đè nguồn, tránh trùng tên, xuất file nguyên tử, hủy không để đầu ra dở.
- Không tự duyệt đệ quy thư mục; không biến URI mạng thành đường dẫn cục bộ giả.
- Không thêm dependency khi thư viện đang có hoặc platform đáp ứng được; mọi hàm mới có kiểu tham số/giá trị trả về.
- Chỉ commit file của công việc. Repository gốc là `<repo>`, dự án nằm trong `morphix`; không stage toàn repository.
- Người dùng đã chọn Native; triển khai tuần tự trong phiên, không điều phối subagent.

## Review Focus

- DING bị sửa, symlink, có diversion khác hoặc source hash lệch: package preinst dừng trước khi đổi file (Task 3, 7).
- DING system package đổi version: apt yêu cầu Morphix tương thích; `doctor` kiểm version, source backup và diversion, không vá version chưa hỗ trợ (Task 4, 7, 8).
- Gỡ hoặc nâng Morphix: chỉ diversion do package Morphix sở hữu được gỡ; file DING gốc đang được dpkg giữ lại phải được phục hồi (Task 7).
- Chọn lẫn file cục bộ, thư mục, URI mạng và tên có xuống dòng/ký tự shell: không thực thi shell, không chuyển đổi một phần lựa chọn âm thầm (Task 2, 3).
- Mở lần hai khi đang convert hoặc đóng hộp chọn file: giữ job đang chạy và không mất lựa chọn/trạng thái (Task 5).
- HiDPI/nhiều màn hình và compositor từ chối focus/position: app vẫn hiện, không mất khả năng thao tác (Task 5, 8).

## Bản đồ file và hợp đồng

Đường dẫn dưới đây tính từ thư mục Morphix. Những file ghi **mới** là đề xuất có chủ đích, không phải khẳng định chúng đã tồn tại.

| Vùng | File | Trách nhiệm |
| --- | --- | --- |
| Bằng chứng | **mới** `docs/superpowers/plans/notes/2026-10-20-ubuntu-desktop-validation.md` | Phiên bản, baseline, kết quả probe và nghiệm thu |
| Nautilus | **mới** `integration/nautilus/morphix_menu.py`, `integration/tests/test_nautilus_menu.py` | Provider và kiểm tra hợp đồng argv |
| DING | **mới** `integration/ding/morphix-menu.patch`, `integration/ding/source.json`, `integration/ding/LICENSE`; **mới** `scripts/prepare-ding.py`, `scripts/tests/test_prepare_ding.py`, `scripts/tests/test_ding_menu.js` | Nguồn/version/hash, hai file system patch tối thiểu và manifest của chúng |
| CLI | `crates/morphix-cli/src/main.rs`, `src/integration/{mod,report,linux,windows,fake}.rs`; **mới** `src/integration/desktop.rs` | Doctor đọc trạng thái DING system/package; integration per-user giữ hành vi hiện có |
| CLI tests | `crates/morphix-cli/tests/{integration,cli,menu,tray}.rs` | Hồi quy contract và cài/gỡ |
| GUI | `crates/morphix-gui/src/{lib,windows,commands}.rs`, `tauri.conf.json`; **mới** `src/integration.rs` | Hiện app, cầu nối CLI, mở thư mục |
| UI | `crates/morphix-gui/ui/src/{khay,recent,strings}.ts`, `ui/src/styles.css`, `ui/tests/recent.test.ts`; **mới** `ui/src/integration.ts`, `ui/tests/integration.test.ts` | Trạng thái tích hợp, chọn file và nút kết quả |
| Packaging | `scripts/{build-deb,check-deb}.sh`, `crates/morphix-gui/packaging/deb/control.in`, `packaging/LICENSES-THIRD-PARTY.md`; **mới** `packaging/morphix.desktop`, DING `preinst`/`postrm`, `docs/ubuntu-install.md` | Gói, dependency pin, diversion, launcher và hướng dẫn |

Các lệnh Cargo chạy tại root Morphix; npm chạy tại `crates/morphix-gui/ui`; Python unittest chạy tại root Morphix. Không thay đổi cwd hệ thống trong script mà thiếu đường dẫn tuyệt đối cho nguồn/đích.

---

### Task 1: Baseline và kiểm chứng DING trước triển khai sản phẩm

**Files:** Create `docs/superpowers/plans/notes/2026-10-20-ubuntu-desktop-validation.md`.

**Interfaces:** Consumes spec và DING hệ thống; produces bảng phiên bản package/GNOME/session, SHA-256 nguồn, vị trí bản user được nạp và kết luận pass/fail. Không tạo API sản phẩm.

- [ ] Ghi `git status --short`, phiên bản package DING/Nautilus/GNOME/Python và trạng thái extension; chỉ ghi các biến môi trường phiên cần thiết, không dump secrets.
- [ ] Chạy baseline `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace --locked`; tại UI chạy `npm run check`, `npm test`, `npm run build`. Ghi lỗi có traceback trước khi phân loại lỗi cũ/mới.
- [ ] Trong tài khoản/VM Ubuntu 24.04 thử nghiệm, sao chép DING đúng phiên bản sang XDG data user, cùng UUID; thêm dấu nhận diện trong log thử nghiệm, không đổi menu sản phẩm. Ghi bằng chứng GNOME thực sự nạp bản user sau đăng nhập.
- [ ] Gỡ bản thử nghiệm và đăng nhập lại; xác nhận DING hệ thống hoạt động lại, icon/menu/cấu hình còn nguyên. Thử với bản user có sẵn: probe phải từ chối ghi đè.
- [ ] Kiểm tra trên Wayland và X11. Nếu không có desktop thực hoặc cơ chế không hoạt động, ghi blocked cho nhánh Desktop, không giả lập thành pass; có thể làm các task Files/app độc lập nhưng không phát hành đủ phạm vi.
- [ ] Ghi nguồn lấy gói DING, license và hash chính xác từ probe để Task 3 dùng; commit tài liệu bằng `git add -- docs/superpowers/plans/notes/2026-10-20-ubuntu-desktop-validation.md` rồi `git commit -m "docs(morphix): record Ubuntu desktop integration baseline"`.

### Task 2: Provider Nautilus trực tiếp

**Files:** Create `integration/nautilus/morphix_menu.py`, `integration/tests/test_nautilus_menu.py`; modify CLI `src/main.rs`, GUI `src/lib.rs` nếu cần chuyển thông báo, tests `menu.rs`, `tray.rs`.

**Interfaces:** Provider API 4 `MorphixMenuProvider.get_file_items(self, files: list[Nautilus.FileInfo]) -> list[Nautilus.MenuItem]`, `get_background_items(self, current_folder: Nautilus.FileInfo) -> list[Nautilus.MenuItem]`. Hàm thuần mới `command_for_selection(uris: list[str], all_regular_local: bool) -> list[str]`. Với file hợp lệ gọi `/usr/bin/morphix menu --uris <URI...>`; nền gọi `/usr/bin/morphix tray`. Với lựa chọn không xử lý được gọi `/usr/bin/morphix tray --unsupported-selection`; thêm boolean này vào CLI Tray và GUI startup, hiển thị thông báo cố định, không nhận chuỗi lỗi tùy ý từ shell.

- [ ] Viết unittest `test_selection_preserves_argv` với assertion `command_for_selection(['file:///tmp/a%20b.png'], True) == ['/usr/bin/morphix', 'menu', '--uris', 'file:///tmp/a%20b.png']`; `test_background_opens_app` assert `command_for_selection([], True) == ['/usr/bin/morphix', 'tray']`; `test_mixed_selection_reports_unsupported` assert lô không hợp lệ dùng `['/usr/bin/morphix', 'tray', '--unsupported-selection']`.
- [ ] Thêm test provider có đúng một item nhãn `Morphix` cho nền/file; tên Unicode, newline, dấu nháy, `$()` và `;` vẫn là một argv. Mock GI chỉ cho unit test; kiểm API thật ở Task 8. Chạy `python3 -m unittest discover -s integration/tests -v`, xác nhận fail vì provider chưa có.
- [ ] Cài provider dùng `Gio.Subprocess` argv, không shell, không chờ conversion. Bắt lỗi khởi chạy, log traceback và hiện lỗi bằng cơ chế GTK phù hợp phiên bản đã kiểm; không để callback làm crash Files. URI/directory không hợp lệ đi theo hợp đồng unsupported; file cục bộ loại chưa hỗ trợ vẫn tới GUI catalog để giải thích.
- [ ] Thêm test CLI/GUI cho cờ mới và cold/warm startup; cập nhật mọi nơi tạo `StartupRequest`. Giữ `file_uris_to_paths(uris: &[String]) -> Result<Vec<PathBuf>, String>` nguyên hợp đồng từ chối lô URI không hợp lệ.
- [ ] Chạy Python test, `cargo test -p morphix-cli --test menu --test tray`, `cargo test -p morphix-gui`; pass rồi commit các file task với message `feat(morphix): add direct Nautilus context menu provider`.

### Task 3: Tạo DING system patch có phiên bản và kiểm hash

**Files:** Create `integration/ding/{source.json,morphix-menu.patch,LICENSE}`, `scripts/prepare-ding.py`, `scripts/tests/test_prepare_ding.py`.

**Interfaces:** Script `python3 scripts/prepare-ding.py --source <extracted-ding-dir> --output <empty-output-dir>`; hàm `prepare_ding(source: Path, output: Path) -> None`. `source.json` ghim package version, source URL, hash của bốn file nguồn và hash mong đợi của hai file đã vá. Đầu ra gồm hai JS thay thế, GPL license và manifest hash; package cài chúng dưới system DING path qua `dpkg-divert`.

- [ ] Viết `test_rejects_modified_source` assert lệch hash raise `ValueError` và không công bố output; `test_refuses_existing_output` assert thư mục có dữ liệu giữ nguyên; `test_menu_patch_has_two_entry_points` assert patch thêm mục file và nền, không đổi các luồng khác. Chạy `python3 -m unittest discover -s scripts/tests -v`, xác nhận fail.
- [ ] Tạo patch từ source đúng Task 1: chèn đúng một item vào menu file và nền, lấy toàn bộ selection DING, truyền argv riêng không shell. Với thư mục, biểu tượng đặc biệt và lô hỗn hợp dùng `tray --unsupported-selection`; file local chưa hỗ trợ vẫn mở wheel.
- [ ] `prepare_ding` kiểm hash trước khi patch, dựng output tạm và publish khi thành công; từ chối symlink/đích có sẵn và giữ license/source notice. Không tải source hoặc thực thi script không kiểm hash lúc cài app.
- [ ] Chạy Python test, `gjs -I <patched-ding-app> -I /usr/share/gnome-shell/extensions/ding@rastersoft.com/app scripts/tests/test_ding_menu.js` và kiểm cú pháp GJS. Test menu xác minh argv nhiều file có Unicode/ký tự shell; không dùng desktop giả lập làm bằng chứng GNOME. Nghiệm thu menu thật thuộc Task 8.
- [ ] Commit đúng file task: `feat(morphix): prepare versioned DING context menu integration`.

### Task 4: Doctor đọc trạng thái Desktop package-level

**Files:** CLI integration `mod.rs`, `report.rs`, `linux.rs`, `windows.rs`, `fake.rs`; new `desktop.rs`; CLI `main.rs`; tests `integration.rs`, `cli.rs`.

**Interfaces:** Thêm `ComponentId::{NautilusExtension,DesktopContextMenu,ApplicationLauncher}` với JSON `nautilus_extension`, `desktop_context_menu`, `application_launcher`. Desktop cài/gỡ bởi package manager, không bởi lệnh theo người dùng. `desktop.rs` chỉ có `inspect_desktop(...) -> IntegrationComponentReport`: kiểm `dpkg-query` đúng version, diversion owner/path, source backup và hash các file patch. `install-integration`/`uninstall-integration` không chạm file package trong `/usr/share`; giữ API hiện có và không thêm `--desktop`.

- [ ] Viết test fake tool runner cho version đúng/sai, thiếu DING, diversion thiếu/sai owner, file patch đổi hash và source backup đổi hash; expect Installed/Missing/Stale/Conflict chính xác. Chạy `cargo test -p morphix-cli --test integration`, xác nhận test mới fail.
- [ ] Kiểm mọi đường dẫn tuyệt đối, kiểm exit code và output của `dpkg-query`, `dpkg-divert`, `sha256sum`; lỗi tool báo Failed có nguyên nhân, không đoán là Installed. Test không truy cập `/usr/share` thật.
- [ ] `doctor` chỉ báo Desktop Installed khi package version và toàn bộ hash/diversion khớp. Version ngoài manifest báo Stale; file lạ/diversion khác báo Conflict. `uninstall-integration` giữ nguyên system DING.
- [ ] Cập nhật toàn bộ match `ComponentId`, serialization, snapshots và backend Windows/Unsupported; giữ các tên JSON cũ. Giữ NautilusScript cho Alt+C hiện có, không làm hỏng NautilusAccel/GnomeShortcut.
- [ ] Chạy `cargo test -p morphix-cli` và clippy package; commit `feat(morphix): inspect managed Ubuntu desktop integration`.

### Task 5: App độc lập, theo dõi tích hợp và mở kết quả

**Files:** GUI `src/{lib,windows,commands}.rs`, new `src/integration.rs`, `tauri.conf.json`; UI `src/{khay,recent,strings}.ts`, `src/styles.css`, new `src/integration.ts`, tests `recent.test.ts`, new `integration.test.ts`.

**Interfaces:** Giữ `open_khay(app: &AppHandle) -> Result<(), String>` và đường `morphix tray`. Thêm Tauri command async `integration_status() -> Result<serde_json::Value, String>`; gọi binary CLI cùng thư mục với `doctor --json`, đọc JSON kể cả nonzero exit có report hợp lệ, giới hạn chờ 30 giây và reap child. Desktop được cài/gỡ bởi apt, app chỉ đọc trạng thái và đưa hướng dẫn cập nhật package khi Stale/Conflict; không chạy quyền root. Thêm `open_output_directory(path: String, state: State<'_, AppState>) -> Result<(), String>` trong commands: chỉ nhận output đã ghi trong history, mở parent bằng `gio open` argv trên Linux, trả lỗi rõ trên nền tảng chưa triển khai. Không cho phép frontend chạy command tùy ý.

- [x] Test startup `tray --unsupported-selection` hiện app với thông báo; startup nền không tự mở cửa sổ. Test pure composition recent row giữ `path` chỉ với kết quả done, không tạo nút mở cho failed/cancelled. Chạy GUI cargo tests và `npm test`, xác nhận test mới fail.
- [x] Cho Khay xuất hiện trên taskbar (`skipTaskbar: false`), có khung cửa sổ (`decorations: true`), không always-on-top; giữ vòng tròn riêng và nguyên cơ chế đóng/ẩn. Sửa `open_khay`: lỗi đọc màn hình/position chỉ log và thử center; luôn thử show, lỗi show vẫn trả Err. Không thêm abstraction window chỉ để mock.
- [x] Thêm cầu CLI bất đồng bộ bằng thread/blocking task, giới hạn timeout, không chặn UI, stderr/log đầy đủ. Viết test fake executable trả valid partial JSON, malformed JSON, missing executable và timeout; assert lỗi được hiển thị và child được thu dọn.
- [x] UI hiển thị trạng thái tích hợp khi mở app; Installed/Not applicable/Stale/Conflict/Failed có nội dung riêng. Desktop Stale/Conflict hướng dẫn cập nhật hoặc cài lại gói Morphix bằng package manager; không có nút nâng quyền trong app. `integration.test.ts` kiểm partial/failure không hiển thị thành công giả.
- [x] Thêm nút “Mở thư mục” cho output thành công; `recent.ts` bảo toàn path từ HistoryEntry. Backend test từ chối path không có trong history, file đã mất và URI; kiểm tên Unicode/ký tự shell được truyền nguyên argv. Với history cũ thiếu output hợp lệ, không bật nút.
- [ ] Kiểm UI thực: picker cancel rồi mở lại, kéo thả nhiều file, mở app lần hai khi convert, nút hủy, lỗi từng file, Tab/Enter/Esc và focus vòng tròn. Ghi bằng chứng position/focus fallback trong môi trường thật thay vì chỉ assert config.
- [x] Chạy `cargo test -p morphix-gui`, tại UI `npm run check && npm test && npm run build`; commit `feat(morphix): complete standalone conversion app workflow`.

### Task 6: Kiểm tra và sửa lỗi chuyển đổi được tái hiện

**Files:** Inspect `crates/morphix-core/src/{job,naming,catalog}.rs`, `src/ops/`; modify đúng implementation có lỗi; tests hiện có `convert.rs`, `media.rs`, `pdf.rs`, `pdf_text.rs`, `text.rs`, `archive.rs`, `job.rs` và CLI tests. Không tạo một bộ conversion mới.

**Interfaces:** Giữ API core và catalog hiện có; mọi lỗi phát hiện phải có input tái hiện, expected/actual và regression test trước diff.

- [x] Chạy test hiện có với FFmpeg/PDFium đúng cấu hình; phân biệt thiếu dependency với lỗi conversion. Dùng fixture tạm nhỏ cho ảnh→PNG/PDF, audio→WAV, video→MP4, PDF→TXT, CSV→TSV và ZIP roundtrip; kiểm nội dung/khả năng decode, không chỉ file tồn tại.
- [x] Kiểm source hash giữ nguyên, tên output trùng không bị ghi đè, hủy dọn file tạm, batch lỗi một file vẫn xử lý file hợp lệ, archive traversal/symlink không thoát đích. Chỉ bổ sung regression test nơi suite chưa bảo vệ hành vi này.
- [x] Với mỗi lỗi: test fail trước; grep toàn bộ caller; sửa nhỏ nhất tại shared code; chạy test mục tiêu rồi suite package. Không “sửa” bằng ẩn định dạng để né lỗi đã xác nhận.
- [x] Commit từng nhóm lỗi thật với `fix(morphix): <root cause>`; nếu không có lỗi thì ghi kết quả kiểm tra, không tạo diff core cho đủ task.

### Task 7: Bộ cài Ubuntu và hướng dẫn vận hành

**Files:** `scripts/build-deb.sh`, `scripts/check-deb.sh`, `crates/morphix-gui/packaging/deb/control.in`, `packaging/LICENSES-THIRD-PARTY.md`; create `packaging/morphix.desktop`, `docs/ubuntu-install.md`.

**Interfaces:** Launcher `Type=Application`, `Name=Morphix`, `Exec=/usr/bin/morphix tray`, `Icon=morphix`, `Terminal=false`, `Categories=Utility;`. Gói cài provider vào `/usr/share/nautilus-python/extensions/morphix_menu.py`, launcher vào `/usr/share/applications/morphix.desktop`, icon vào hicolor, hai file DING đã kiểm vào đường dẫn gốc bằng `dpkg-divert --package morphix`. `Depends` pin đúng package version DING. Maintainer scripts không ghi HOME người dùng. Giữ build feature `morphix-gui/custom-protocol`.

- [x] Mở rộng check-deb để kiểm launcher/provider/DING/maintainer scripts/license; chạy `desktop-file-validate`, kiểm manifest/hash, Depends Python GI/Nautilus/GTK/DING và cú pháp scripts. Gói chỉ nhắm Noble vì DING được ghim đúng phiên bản Ubuntu 24.04.
- [x] Dựng hai file DING qua Task 3; maintainer scripts xác minh package version, source hash, symlink/local edits và diversion trước khi `--rename`. Test install/upgrade/remove bằng `dpkg-divert` thật trong admin database giả tại thư mục tạm; xác nhận source gốc còn byte-identical sau gỡ.
- [x] Viết hướng dẫn cài .deb bằng apt (cần admin), dùng Files/Desktop/vòng tròn/app, doctor và gỡ package. Nêu rõ cập nhật DING cần bản Morphix tương thích và đăng xuất/đăng nhập để nạp lại. Không yêu cầu bật integration trong app.
- [x] Dựng gói Noble từ commit `e9054cb`, chạy package checker và `check-gui-embeds-ui.sh` trên binary đã giải nén; các bước đều exit 0. Không coi kiểm trên host là cài sạch.
- [x] Commit đúng files task `build(morphix): package complete Ubuntu desktop integration` (`e9054cb`).

### Task 8: Nghiệm thu trên máy sạch và bàn giao

**Files:** Update validation note Task 1 và `docs/ubuntu-install.md`; sửa code chỉ khi có bug tái hiện, theo chu kỳ regression test của task sở hữu.

**Interfaces:** Produces .deb + SHA-256, log/test report và bảng nghiệm thu từng phiên desktop. Không có tiêu chí “pass” mặc định hoặc tự bỏ qua task Desktop.

- [x] Chạy cuối `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace --locked`; UI `npm run check`, `npm test`, `npm run build`; Python suites hai thư mục; package checks. Ghi số test pass/fail/skip thực tế và lý do skip. `cargo fmt --check` còn báo khác biệt định dạng ở file Rust không chỉnh sửa; các kết quả khác được ghi trong validation note.
- [x] Cài/nâng bằng APT trên máy đang dùng Ubuntu 24.04.5: nâng morphix 0.1.0-1 lên 0.1.0-2; dpkg -V sạch, doctor xác nhận provider/launcher/DING. DING được reload và ACTIVE.
- [ ] Nghiệm thu cài bằng APT trong Ubuntu 24.04 sạch; chạy launcher khi GUI chưa chạy rồi mở lại, xác nhận UI nhúng tải thành công.
- [ ] Trên cả Wayland/X11 kiểm bốn menu Files/file, Files/nền, Desktop/file, Desktop/nền: đúng một mục Morphix ở cấp đầu, menu gốc vẫn dùng được. Chọn nhiều file có tên lạ; file mở vòng tròn, nền mở app; unsupported có thông báo.
- [ ] Convert fixture đại diện Task 6 từ cả app và vòng tròn; so sánh kết quả và hash nguồn, kiểm progress/hủy/lỗi/mở thư mục. HiDPI/nhiều màn hình kiểm cửa sổ thao tác được dù không đặt chính xác tại con trỏ.
- [ ] Kiểm cài/cấu hình lại package, nâng Morphix cùng DING tương thích, apt từ chối DING version chưa được Morphix hỗ trợ, local patch/diversion xung đột dừng an toàn, gỡ khôi phục bản gốc rồi đăng xuất/đăng nhập. Chụp trạng thái trước/sau và đọc log GNOME/Morphix.
- [ ] Đọc log/traceback toàn bộ kịch bản; sửa lỗi chưa xử lý, không chỉ kiểm exit code. Nếu môi trường ngăn nghiệm thu, ghi rõ việc còn thiếu và không tuyên bố app hoàn chỉnh.
- [x] Ghi artifact path/hash và kết quả vào validation note; commit. Bàn giao link gói và hướng dẫn; các kiểm tra desktop chưa hoàn tất được liệt kê riêng.

**Kết quả phiên nghiệm thu 2026-10-20:** artifact 0.1.0-2 đã được cài trên máy Ubuntu 24.04.5 hiện tại; bản 0.1.0-1 và tích hợp user cũ đã được thay/gỡ. Validation note ghi hash, doctor, package verification và giới hạn còn lại. Task 8 vẫn mở vì chưa kiểm bốn menu bằng thao tác thật, chưa chạy chuyển đổi từ GUI/menu, chưa cài máy clean và chưa kiểm Wayland.

## Thứ tự và cách thực hiện đề xuất

1 → 2 → 3 → 4 → 5 → 6 → 7 → 8. Thiết kế DING package-level `dpkg-divert` đã được duyệt; triển khai tuần tự.

Phương thức **Native** đã được chọn: triển khai tuần tự trong phiên này với skill executing-plans; không dispatch subagent.

## Tự rà soát kế hoạch

Đã ánh xạ menu Files→Task 2, Desktop→1/3/4/7, app→5, an toàn conversion→6, nghiệm thu→8. Review Focus có task sở hữu, và maintainer scripts được test trên dpkg root tạm, không chạm package database của host. API mới được đánh dấu là đề xuất; contract JSON hiện có được giữ. Version/hash DING lấy từ probe, apt metadata và file hệ thống hiện tại. Phạm vi sản phẩm, phương thức Native và thay đổi sở hữu/quyền quản trị cho Desktop đã được người dùng duyệt.
