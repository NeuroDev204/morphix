# Morphix Windows: menu native thay hình quạt và vòng tròn

Status: thiết kế đã được người dùng duyệt trong hội thoại (2026-11-05), đã sửa sau lượt review đối chiếu với code; chờ duyệt bản viết này trước khi lập plan.

Thay thế phần Windows của `2026-10-26-morphix-windows-explorer-fan-design.md` và `2026-10-26-morphix-windows-liquid-glass-fan-design.md` (hai file cũ giữ nguyên làm lịch sử). Linux không đổi.

Đây là dự án con **A** trong lần thiết kế lại Morphix. Thứ tự: B (ảnh, đã làm) → A → C (video/audio) → D (PDF/tài liệu).

## Mục tiêu

Trên Windows, chọn việc cho file trông và dùng như menu chuột phải thông thường, không còn hình quạt hay vòng tròn nào.

## Quyết định của người dùng

- Chuột phải file trong Explorer: mục "Morphix" mở **submenu native** liệt kê các việc áp dụng được cho file đã chọn; bấm một mục là chạy việc đó.
- Bỏ **mọi hình tròn trên Windows**.
- `morphix menu <file>` trên Windows (giờ chỉ còn khi gõ lệnh tay) **báo lỗi rõ ràng** bằng tiếng Việt, chỉ sang chuột phải → Morphix trong Explorer hoặc `morphix convert`. Không làm popup thay thế.
- Cửa sổ khay, lịch sử và cài đặt không phải hình tròn nên giữ nguyên.

## Hiện trạng (đã đối chiếu code)

- `crates/morphix-shell/src/windows.rs`: submenu của "Morphix" chỉ có **một mục trống** vẽ tay (`FAN_ITEM_DATA`, `WM_MEASUREITEM`, `WM_DRAWITEM`). Khi submenu mở (`WM_INITMENUPOPUP`), `open_fan` → `dispatch_fan` → `send_fan` gửi `MenuRequest` có `MenuPresentation::ExplorerFan` cho GUI. `send_fan` chạy trên luồng riêng, tự khởi động `morphix-gui.exe` nếu IPC chưa kết nối rồi thử lại.
- `InvokeCommand` hiện **không làm gì**. `QueryContextMenu` trả `HRESULT(1)` vì chỉ dùng một id.
- Lỗi của DLL chỉ đi vào `log_debug`; người dùng không thấy.
- DLL chỉ phụ thuộc `morphix-ipc`, `windows`, `windows-core`. `morphix-ipc` không phụ thuộc `morphix-core`.
- GUI đã có giao thức id dạng chuỗi cho mỗi việc (`convert:webp`, `compress`, `strip`, `resize`, `archive:zip`, ...): `menu_items::wheel_items` dựng id từ `ActionKind` (`crates/morphix-gui/src/menu_items.rs`), lệnh Tauri `run_action` (`crates/morphix-gui/src/commands.rs`) kiểm id có áp dụng cho các file không rồi đổi id thành `Job` qua `operation_for` và chạy bằng `state.jobs.start`.
- `morphix_core::detect` đọc tối đa 8 KB đầu mỗi file (`SNIFF_BYTES`). `actions_for` chỉ trả việc hợp lệ cho mọi file; chọn lẫn loại thì chỉ còn các việc nén file (`CreateArchive`).
- Trên Windows không còn verb registry nào gọi `morphix menu`: `menu_entry`/`menu_command` trong `crates/morphix-cli/src/integration/windows.rs` không có nơi gọi, và khi cài shell extension `remove_previous_verbs` xoá verb cũ.

## Thiết kế

### 1. Id của việc dùng chung từ core

- Chuyển hàm đổi `ActionKind` → id chuỗi từ `menu_items.rs` sang `morphix-core` (cạnh `catalog::actions_for`). `menu_items::wheel_items` của GUI và DLL cùng gọi hàm đó, để hai bên luôn sinh cùng một id.
- `operation_for` (id → `Operation`) giữ ở GUI vì cần `Settings` (ví dụ phần trăm resize mặc định).

### 2. Submenu native trong Explorer

- DLL thêm phụ thuộc `morphix-core` để dùng `detect`, `actions_for` và hàm id ở mục 1.
- `QueryContextMenu`:
  - Đọc đường dẫn từ `IShellExtInit` (đã có), gọi `detect` cho từng file; file nào đọc lỗi thì coi là `FileKind::Unknown`.
  - Chọn quá **100 file** thì không sniff; submenu chỉ có một mục bị vô hiệu hoá "Chọn quá nhiều file (tối đa 100)". WHY: `detect` chạy đồng bộ trong luồng giao diện của Explorer, nhiều file trên ổ chậm sẽ làm treo menu. 100 là giá trị mặc định đề xuất.
  - Không có việc nào áp dụng được thì không chèn mục "Morphix".
  - Submenu gồm mục thường (`MFT_STRING`) với nhãn lấy từ `Action::label`. Nhóm công cụ (`is_tool`) đứng sau một đường kẻ ngăn.
  - Cấp id từ `first_id` trở đi, không vượt `last_id`; thiếu chỗ thì bỏ bớt mục cuối. Trả `MAKE_HRESULT(SEVERITY_SUCCESS, 0, số id đã dùng)` để Explorer không cấp trùng id cho shell extension khác.
- `InvokeCommand`:
  - Từ chối với `E_INVALIDARG` khi `lpVerb` là chuỗi (HIWORD khác 0) hoặc offset không thuộc mục của Morphix.
  - Gửi IPC tới GUI bằng logic hiện có của `dispatch_fan`/`send_fan` (đổi tên, bỏ phần đợi `popup_rect` để lấy vị trí): chạy nền, tự khởi động `morphix-gui.exe` nếu cần, thử lại.
  - Gửi thất bại thì hiện hộp thoại lỗi tiếng Việt (`MessageBoxW`) nêu nguyên nhân, không chỉ ghi `log_debug`.
- `GetCommandString` trả chuỗi trợ giúp tiếng Việt cho từng mục.
- Lỗi hoặc panic trong các hàm COM phải trả `HRESULT` lỗi, không được làm sập Explorer.

### 3. IPC và GUI

- `MenuRequest` còn `paths: Vec<PathBuf>` và thêm `action: Option<String>` (id ở mục 1).
  - `action = Some(id)`: GUI chạy việc đó ngay, không mở cửa sổ nào ngoài khay/lịch sử như khi chạy từ vòng tròn.
  - `action = None`: hành vi vòng tròn hiện có trên Linux.
- Bỏ enum `MenuPresentation` và các trường `anchor`, `session_id`, `close_fan` (chỉ phục vụ hình quạt). Linux chỉ còn trường hợp vòng tròn đầy đủ nên không cần enum.
- Tách thân `run_action` thành một hàm không phụ thuộc lệnh Tauri, dùng chung cho lệnh `run_action` của webview và cho IPC. Hàm này giữ các kiểm tra hiện có (id có áp dụng cho file không, thiếu ffmpeg).
- Lỗi khi chạy việc nhận qua IPC phải hiện cho người dùng bằng hộp thoại native của GUI, vì lúc đó không có webview nào mở để hiện banner. Plan xác nhận cơ chế hộp thoại có sẵn trong GUI trước khi chọn.

### 4. CLI

- `morphix menu` trên Windows thoát với mã 2 và in ra stderr: dùng chuột phải → Morphix trong Explorer, hoặc `morphix convert`. Không gửi gì tới GUI.
- Linux không đổi.

### 5. Dọn dẹp

Xoá hẳn, vì không còn nơi dùng:
- `morphix-shell`: `FAN_ITEM_*`, `dark_menu_background`, xử lý `WM_MEASUREITEM`/`WM_DRAWITEM`/`WM_INITMENUPOPUP`/`WM_UNINITMENUPOPUP`/`WM_EXITMENULOOP`, `open_fan`, `close_fan`, `popup_fan_region`, `FAN_INNER_RADIUS`, `clear_popup_region`, `popup_rect`, `popup_menu_id`, `new_session_id`, trường `fan_session`, `IContextMenu2`/`IContextMenu3` nếu không còn message nào cần xử lý, và các test của quạt.
- `morphix-ipc`: `MenuPresentation`, `anchor`, `session_id`, `close_fan`, nhánh xử lý `ExplorerFan`; cập nhật `crates/morphix-ipc/tests/ipc.rs`.
- `morphix-gui`: mọi nhánh `ExplorerFan` trong `lib.rs`, `state.rs`, `windows.rs`; mã mở cửa sổ vòng tròn trên Windows.
- `morphix-cli`: cờ `--explorer-fan`, `--fan-anchor`, `--fan-popup`, `--close-fan`, `--session-id`; hàm `parse_fan_anchor`, `parse_fan_popup`, `resolve_fan_popup`, `run_close_fan`; `menu_entry`, `menu_command` và test của chúng; cập nhật `crates/morphix-cli/tests/menu.rs`. Gỡ `#[allow(clippy::too_many_arguments)]` của `run_menu` khi số tham số đã giảm.
- UI webview: phần chỉ phục vụ quạt (`explorerFanItems`, `fanWheelDelta`, xử lý bánh xe cho quạt) và test tương ứng trong `ui/tests`.

## Kiểm thử

- Dựng danh sách mục từ danh sách file (hàm thuần, không cần Explorer): một PNG; nhiều PNG; lẫn loại (chỉ còn mục nén file); file không nhận dạng được (không có mục); file không đọc được (coi là `Unknown`); quá 100 file (một mục bị vô hiệu hoá).
- Cấp id: dùng đúng `first_id..`, không vượt `last_id`, số trả về bằng số id đã dùng; id ngoài khoảng và `lpVerb` dạng chuỗi bị từ chối.
- Id chung: với mọi `ActionKind` mà `actions_for` trả, id từ core đổi ngược được bằng `operation_for` của GUI.
- IPC: round-trip `MenuRequest` có và không có `action`.
- GUI: yêu cầu có `action` tạo đúng `Job` và ghi lịch sử; id không áp dụng cho file thì trả lỗi.
- CLI: `morphix menu` trên Windows thoát mã 2 với thông báo; Linux giữ test hiện có.
- Test registry/cài đặt hiện có vẫn đạt.
- Kích thước DLL: ghi lại trước và sau khi thêm `morphix-core`. Tăng quá 2 MB thì chuyển sang bảng đuôi file viết tay trong DLL kèm test đối chiếu với catalog của core (2 MB là giá trị mặc định đề xuất).
- Kiểm tay trong Explorer thật (người dùng xác nhận): chuột phải một file, nhiều file, file trên Desktop, lẫn loại; bấm một mục ra đúng file; GUI tắt sẵn trước khi bấm; hộp thoại lỗi khi `morphix-gui.exe` bị đổi tên. Tôi không tự điều khiển được menu chuột phải.

## Ngoài phạm vi

- Menu Windows 11 hiện đại (cần `IExplorerCommand` và package identity). Vẫn chỉ nằm trong "Show more options" như spec cũ.
- Bắt chọn file bằng Alt+C trên Windows.
- Đổi vòng tròn trên Linux.
- Thiết kế lại cửa sổ khay, lịch sử, cài đặt.

## Rủi ro đã biết

- Submenu native không cuộn bằng bánh xe và không có hiệu ứng như hình quạt; đây là hệ quả của lựa chọn.
- DLL chạy trong `explorer.exe`; thêm phụ thuộc `morphix-core` làm DLL lớn hơn và đưa thêm code vào tiến trình của Explorer.
- Sniff tới 100 file × 8 KB vẫn có thể chậm trên ổ mạng; nếu kiểm tay thấy trễ thì hạ ngưỡng.
- Đổi `MenuRequest` là đổi giao thức IPC: CLI, DLL và GUI phải cùng phiên bản; DLL hoặc CLI cũ gửi tới GUI mới sẽ bị từ chối và phải báo lỗi rõ ràng.
