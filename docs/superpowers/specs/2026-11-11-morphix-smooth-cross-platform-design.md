# Morphix: mượt, nhẹ, và phát hành được trên Linux lẫn Windows

Status: bản viết chờ người dùng duyệt (2026-11-11). Chưa có plan.

Thay thế phần "vòng tròn trên Linux" của `2026-10-19-morphix-design.md` và mục "Linux không đổi" của `2026-11-05-morphix-windows-native-menu-design.md`. Các spec cũ giữ nguyên làm lịch sử.

Ký hiệu: **[ND]** là điều người dùng đã nói. **[TK]** là lựa chọn của người viết spec, người dùng chưa xác nhận, có thể đổi.

## Vì sao làm

Người dùng báo ba vấn đề và một mong muốn mới [ND]:

1. Mở giao diện trên Linux bị khựng, phản hồi chậm.
2. Bản build nặng.
3. Microsoft Store từ chối (policy 10.1.4.3, phần mô tả trên trang Store, không liên quan code).
4. Trên Linux không muốn mở hình quạt nữa; việc phải nằm ngay trong menu chuột phải như Windows.

Đích phát hành [ND]: Microsoft Store, file exe tải về, và lệnh curl cho các distro Linux.

## Mục tiêu

- Linux: chuột phải file → Morphix → submenu liệt kê việc → bấm là chạy. Không còn cửa sổ vòng tròn ở bất kỳ nền tảng nào.
- Không lệnh nào của giao diện làm đứng luồng vẽ cửa sổ.
- Binary và gói cài nhỏ hơn, có số đo trước/sau.
- Một lần đẩy tag ra đủ: tarball Linux, `.deb`, `MorphixSetup.exe`, và gói `.msix` để tải lên Partner Center.
- Bản exe an toàn để phát công khai.

## Ngoài phạm vi

- Bản ARM (Linux aarch64, Windows arm64). "Mọi distro" nghĩa là x86_64, glibc ≥ 2.35, có webkit2gtk-4.1 như hiện nay [TK].
- macOS, Flatpak, Snap, AppImage, winget.
- Alt+C trên Windows hoặc Wayland.
- Thiết kế lại Khay, lịch sử, cài đặt.
- Thêm định dạng hay việc xử lý file mới.
- Tự động tải gói lên Partner Center (vẫn tải tay).

## Điều sẽ là thất bại dù code chạy

- Menu chuột phải làm trình quản lý file chậm đi thấy rõ.
- Bấm một việc trong menu rồi không có phản hồi nào (không thông báo, không lỗi).
- Bản exe còn cài chứng chỉ vào kho Root của máy người dùng.
- Cài đè bản mới để lại mục menu cũ trỏ tới vòng tròn đã xoá.

---

## Phần 1. Linux: việc nằm trong menu chuột phải, bỏ vòng tròn

### Hiện trạng (đã đối chiếu code)

- Mọi tích hợp Linux hiện chỉ có **một mục "Morphix"** gọi `morphix menu <file>`, GUI mở cửa sổ `menu` (vòng tròn): `integration/nautilus/morphix_menu.py`, `dolphin_service_menu_body`, `nemo_action_body`, `nautilus_script_body` (`crates/morphix-cli/src/integration/linux.rs`), `thunar_action_block` (`thunar.rs`), và bản vá DING trong `integration/ding/`.
- Đường "chạy thẳng một việc" đã có sẵn từ đợt Windows: `MenuRequest.action` (`morphix-ipc`), `show_menu_for` → `commands::start_action` (`morphix-gui/src/lib.rs`), lỗi hiện bằng hộp thoại native.
- Danh sách việc cho một nhóm file đã có hàm thuần: `morphix_shell::shell_menu` (id, nhãn, `is_tool`, giới hạn 100 file). Crate `morphix-cli` đã phụ thuộc `morphix-shell`.

### Thiết kế

**1.1 CLI**

- `morphix menu --action <id> [--uris …] -- <file…>`: gửi `MenuRequest { paths, action: Some(id) }` tới GUI (tự khởi động GUI nếu chưa chạy, như đường hiện có). Chạy được trên cả Linux và Windows.
- `morphix menu --list --json -- <file…>`: in danh sách việc cho các file đó, lấy từ `shell_menu` (`[{id,label,is_tool}]`, hoặc `{"too_many":true}`). Không đụng tới GUI.
- `morphix menu <file…>` không có `--action` trên Linux: không còn vòng tròn, nên mở **Khay** với các file đó (đường `open_khay_with_files` đã có). Windows giữ hành vi báo lỗi hiện tại. [TK]

**1.2 Hai mức tích hợp**

Trình quản lý file Linux chia hai loại, không làm khác được:

*Mức động (đúng như Windows): Nautilus có `nautilus-python`.*
- `morphix_menu.py` trả mục "Morphix" có submenu (`Nautilus.Menu`), mỗi mục con là một việc; bấm thì chạy `morphix menu --action <id> --uris …`.
- Danh sách lấy bằng `morphix menu --list --json`, nhận dạng theo nội dung file như Windows. Nhóm công cụ (`is_tool`) đứng sau đường kẻ ngăn nếu API cho phép, không thì đứng cuối.
- Không được làm Nautilus khựng: gọi CLI có hạn chờ 300 ms, nhớ kết quả của lựa chọn gần nhất, và quá 100 file thì hiện một mục vô hiệu hoá "Chọn quá nhiều file (tối đa 100)". Hết hạn chờ hoặc lỗi thì chỉ hiện mục "Morphix" mở Khay. Người dựng phải đo: nếu `--list` chậm hơn 50 ms với 20 file trên máy thử thì báo lại trước khi làm tiếp. [TK]
- Chuột phải vào nền thư mục: giữ mục "Morphix" mở Khay.

*Mức tĩnh (lọc theo loại file, không đọc nội dung): Dolphin, Thunar, và các trình dùng thư mục Scripts.*
- File tích hợp được **sinh lúc `morphix install-integration`** từ catalog của `morphix-core`, theo nhóm loại: ảnh, âm thanh, video, văn bản/tài liệu, PDF, file nén, và "mọi file" (các việc nén thành ZIP/TAR.GZ/7Z). Không viết tay bảng việc ở chỗ thứ hai; phải có test đối chiếu file sinh ra với catalog.
- **Dolphin**: mỗi nhóm một file service menu, `MimeType=` của nhóm, `X-KDE-Submenu=Morphix`, mỗi việc một `[Desktop Action …]` với `Exec=morphix menu --action <id> -- %F`.
- **Thunar**: mỗi việc một `<action>` có `<submenu>Morphix</submenu>`, lọc bằng nhóm loại và pattern đuôi file của Thunar. Mọi khối do Morphix quản lý phải gỡ sạch được như khối đơn hiện nay.
- **Nautilus không có nautilus-python, Nemo, Caja**: dùng thư mục Scripts của từng trình, vì thư mục con hiện thành submenu: `Morphix/<Nhóm>/<Tên việc>`, mỗi file là một script gọi `morphix menu --action <id>`. Scripts không lọc theo loại file nên hiện đủ mọi nhóm. Mục Nemo Action hiện có được thay bằng cách này. [TK]
- Giới hạn chấp nhận của mức tĩnh [TK]: menu liệt kê mọi đích của nhóm, kể cả đích trùng định dạng nguồn (PNG → PNG) hoặc không hợp với file cụ thể. Bấm vào thì GUI từ chối bằng hộp thoại lỗi rõ ràng đã có. `morphix doctor` nêu rõ máy đang ở mức động hay tĩnh.
- **DING (bản `.deb`)**: mục trên desktop giữ là một mục "Morphix" mở Khay; không làm submenu. [TK]

**1.3 Phản hồi khi chạy từ menu**

Chạy từ menu không mở cửa sổ nào, và GNOME thường không có khay hệ thống, nên phải có thông báo hệ thống [TK]:
- Việc xong: một thông báo "Xong: N file → <tên file hoặc thư mục>", hoặc số lỗi và lý do đầu tiên.
- Việc không bắt đầu được: hộp thoại lỗi như hiện nay.
- Dùng `tauri-plugin-notification`, áp dụng cho cả Windows. Không thêm cửa sổ tiến độ; việc dài xem ở Khay.

**1.4 Xoá vòng tròn**

- Xoá cửa sổ `menu` khỏi `tauri.conf.json` và `capabilities/default.json`; xoá `ui/index.html`, `ui/src/main.ts`, `wheel.ts` cùng CSS và test chỉ phục vụ vòng tròn; bỏ entry `index` trong `vite.config.ts`.
- Rust: xoá `windows::open_menu`, `place_near_cursor`, `cursor_anchor`, `MENU_WINDOW_LABEL`, `MENU_REQUEST_EVENT`, `BatchRoute::Wheel`, trạng thái `pending` và các lệnh `pending_request`, `discard_pending` nếu không còn nơi dùng. Mọi đường từng mở vòng tròn (kể cả `submit_menu_request` của Khay và Alt+C) đưa file vào lựa chọn của Khay.
- Đổi tên `WheelItem`/`WheelPayload`/`wheel_items` thành tên không nhắc vòng tròn chỉ khi không làm phình diff; không bắt buộc.

**1.5 Nâng cấp từ bản cũ**

`install-integration` (bản curl và `.deb`) phải thay hoặc gỡ mọi file tích hợp kiểu cũ do Morphix quản lý (nhận bằng marker đã có), gồm Nemo Action cũ. `uninstall` gỡ sạch cả cây Scripts và các file service menu mới. Không đụng file không mang marker của Morphix.

---

## Phần 2. Giao diện không khựng

### Nguyên nhân tìm thấy trong code

- Lệnh Tauri khai báo đồng bộ chạy trên luồng chính. Trong `crates/morphix-gui/src/commands.rs`: `menu_payload` mở và đọc từng file (zip còn đọc mục lục), `run_action` làm lại việc đó rồi dò ffmpeg, `open_output_directory` chờ `gio open` thoát, `submit_menu_request` ghi socket, `save_settings`/`clear_history` ghi đĩa.
- Ba webview (`menu`, `khay`, `settings`) được tạo lúc khởi động dù đều ẩn.

### Thiết kế

- Mọi lệnh có đọc/ghi đĩa, socket hoặc tiến trình con chuyển sang chạy nền (lệnh `async` + `spawn_blocking`, như `integration_status` đang làm). Lệnh chỉ đọc trạng thái trong bộ nhớ thì giữ nguyên.
- `start_action` nhận qua IPC cũng không được đọc file trên luồng chính.
- Cửa sổ `settings` tạo khi mở lần đầu, không khai trong `tauri.conf.json`. Cùng với Phần 1, lúc khởi động chỉ còn một webview (Khay). Khay vẫn tạo sẵn để bấm khay là hiện ngay. [TK]
- Đặt `app.security.csp` thay cho `null`: chỉ cho tài nguyên của chính app. [TK]

### Cách xác nhận

Chưa có số đo hiện tại; người dựng đo trước rồi đo sau trên bản release, ghi vào ghi chú của plan:
- Khởi động nguội tới lúc Khay hiện.
- Bấm khay tới lúc Khay hiện khi app đang chạy.
- Thả 50 file từ thư mục chậm vào Khay: cửa sổ vẫn kéo và cuộn được trong lúc đọc.

Mốc đề xuất [TK]: Khay hiện dưới 300 ms khi app đang chạy; không thao tác nào chặn luồng chính quá 50 ms. Nếu sửa xong vẫn khựng thì đó là vấn đề khác (WebKitGTK, driver đồ hoạ) và phải báo lại kèm số đo, không tự vá bằng biến môi trường.

---

## Phần 3. Bản build nhẹ hơn

- Thêm `[profile.release]` vào `Cargo.toml` gốc: `lto`, `codegen-units = 1`, `strip = true`. **Không** đặt `panic = "abort"`: `jobs.rs` và `capture/mod.rs` dựa vào việc bắt panic của luồng nền.
- Trước khi sửa, ghi bảng dung lượng: `morphix`, `morphix-gui`, `morphix_shell.dll`, ffmpeg, PDFium, tổng tarball, `.deb`, exe, msix. Ghi lại sau khi sửa.
- Chỉ gỡ dependency hay feature khi số đo (`cargo bloat` hoặc tương đương) cho thấy đáng kể và không mất tính năng. Ứng viên để đo: feature `avif` của `image`, hai đường WebP (`image` và crate `webp`), `clap` trong GUI.
- ffmpeg: nếu ffmpeg chiếm hơn nửa gói, người dựng tìm bản dựng gọn hơn vẫn đủ mọi codec mà `MediaFormat` cần và giữ được nghĩa vụ giấy phép đang có (`packaging/ffmpeg-*`). Chỉ đổi khi giảm được ít nhất 30% tổng gói; nếu không thì ghi số đo và để nguyên. [TK]
- Thời gian build CI tăng do LTO là chấp nhận được.

---

## Phần 4. Phát hành Windows

### Vấn đề tìm thấy

- `installer/morphix.iss` chạy `certutil -addstore Root morphix-dev.cer`: cài chứng chỉ tự ký vào kho Root của máy người dùng để gói sparse chạy được. Không được phát công khai như vậy.
- `release.yml` không có job Windows. `morphix.iss` ghi cứng 0.1.0, workspace là 0.1.2.
- `open_output_directory` trên Windows luôn trả lỗi "chỉ hỗ trợ Ubuntu/Linux".
- README chỉ nói về Linux.

### Thiết kế

**4.1 Hai gói, hai kiểu menu**

[ND] Người dùng không có chứng chỉ ký code và sẽ tự đặt `MorphixSetup.exe` lên một trang web riêng để người khác tải về cài. Vì vậy exe phải cài và chạy đầy đủ mà không cần chứng chỉ nào; file `.sha256` đi kèm để trang web đăng cùng.

- **Microsoft Store (`.msix` đầy đủ, `installer/store-package/`)**: Store ký gói; menu Windows 11 hiện đại (`IExplorerCommand`) hoạt động như hiện nay.
- **`MorphixSetup.exe`**: bỏ hẳn gói sparse, `morphix-dev.cer`, `register-package.ps1`, `sign-package.ps1` và mọi bước `certutil`. Menu chuột phải dùng shell extension kiểu cổ điển đăng ký qua registry (đường `SHELL_EXTENSION_*` đã có trong `integration/windows.rs`); trên Windows 11 nó nằm trong "Show more options". Đăng ký này nằm trong HKCU (đã kiểm trong code) nên installer cài theo người dùng, không đòi quyền admin. Kèm bước cài WebView2 Runtime khi máy chưa có.
- Bản exe không ký nên SmartScreen sẽ cảnh báo lần đầu. README nói rõ điều này và chỉ sang bản Store. Nếu sau này có chứng chỉ ký code thật thì ký exe và có thể đưa gói sparse trở lại; đó là việc riêng.

**4.2 Sửa lỗi nền tảng**

- `open_output_directory` trên Windows mở Explorer tại thư mục kết quả.
- Rà các chuỗi lỗi "chỉ hỗ trợ Linux" còn lại trong GUI; cái nào Windows làm được thì làm, cái nào không (Alt+C) thì ẩn khỏi giao diện Windows.

**4.3 CI và phiên bản**

- `ci.yml`: thêm job `windows-latest` chạy `cargo clippy` và `cargo test --workspace`.
- `release.yml`: thêm job Windows dựng release, chạy Inno Setup ra `MorphixSetup.exe` (kèm `.sha256`), và đóng `installer/store-package` thành `.msix` chưa ký. Exe đính vào GitHub Release; msix chỉ là artifact của workflow để tải lên Partner Center.
- Phiên bản lấy từ một nguồn: `version` của workspace. CI truyền vào Inno Setup và ghi vào `AppxManifest.xml` lúc đóng gói; CI dừng nếu tag không khớp phiên bản workspace.
- ffmpeg.exe và pdfium.dll cho Windows: tải theo URL và SHA256 ghim trong repo, cùng cách `scripts/ci/setup-build-deps.sh` làm cho Linux.

**4.4 Trang Store**

- Lưu mô tả tiếng Việt và tiếng Anh đã soạn trong hội thoại vào `installer/store-package/listing/description.md`, kèm ghi chú: không nhắc tên sản phẩm khác, không nhồi từ khoá, ô Search terms chỉ dùng từ mô tả chức năng.
- Nộp lại Store là việc người dùng làm tay.

**4.5 README**

Thêm mục Windows (Store, exe, cảnh báo SmartScreen), sửa câu mở đầu không còn "trên máy Linux", cập nhật mô tả menu chuột phải theo Phần 1.

---

## Phần 5. Lệnh curl trên Linux

`scripts/install.sh` và `build-tarball.sh` đã đúng hướng, không viết lại. Việc cần làm:

- Kiểm thử cài đặt hiện có phải bao phủ menu mới: sau `install.sh`, các file tích hợp mới có mặt và `morphix doctor` sạch.
- Chạy lại nghiệm thu trên các bản đã ghi trong README (Fedora, openSUSE, Debian, Arch, Ubuntu và biến thể) và cập nhật bảng "Đã kiểm trên". Bản nào không kiểm được thì ghi là chưa kiểm, không suy đoán.

---

## Thứ tự làm [TK]

1. Phần 2 và 3 (nhỏ, độc lập, có lợi ngay).
2. Phần 1 (thay đổi lớn nhất).
3. Phần 4, rồi Phần 5 để nghiệm thu.

Mỗi phần phát hành riêng được. Plan có thể tách thành nhiều plan theo thứ tự này.

## Kiểm thử chung

- Giữ xanh: `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, `npm run check && npm test`, hai bộ `python3 -m unittest`.
- Phần 1: test hàm thuần cho từng bộ sinh file (Dolphin, Thunar, Scripts) và đối chiếu với catalog; test `morphix menu --list --json` và `--action`; test Python cho submenu Nautilus (mở rộng `integration/tests/test_nautilus_menu.py`); test cài rồi gỡ không để lại file; test nâng cấp từ bố cục cũ.
- Kiểm tay, người dùng xác nhận vì không tự điều khiển được menu chuột phải: Nautilus (có và không có nautilus-python), Dolphin, Thunar, ít nhất một trình dùng Scripts; Explorer với bản exe và bản Store.

## Để người dựng quyết

- Tên file và cách chia module của các bộ sinh.
- Cách nhớ kết quả trong extension Nautilus.
- Giá trị cụ thể của profile release sau khi đo (`lto = "fat"` hay `"thin"`).
- Chuỗi CSP cụ thể.
- Chi tiết API Thunar/Dolphin/Nemo/Caja phải kiểm với tài liệu và máy thật; nếu một trình không làm được submenu như mô tả thì báo lại thay vì tự đổi sang kiểu khác.

## Rủi ro đã biết

- Mức tĩnh không chính xác bằng Windows; đây là giới hạn của các trình quản lý file đó.
- Submenu Nautilus gọi tiến trình con mỗi lần đổi lựa chọn; nếu đo thấy chậm thì phải đổi cách lấy danh sách.
- Bản exe không ký sẽ bị SmartScreen cảnh báo và có thể bị một số trình diệt virus nghi ngờ.
- Thông báo hệ thống phụ thuộc dịch vụ thông báo của desktop; máy không có thì việc vẫn chạy nhưng im lặng.
