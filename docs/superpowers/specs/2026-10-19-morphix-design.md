# Morphix: thiết kế

Ngày: 2026-10-19. Trạng thái: **chờ người dùng duyệt**, chưa có dòng code nào.

Quy ước đánh dấu trong tài liệu:
- **[Đã chốt]** người dùng đã nói rõ.
- **[Mình chọn]** người viết tài liệu chọn thay người dùng, cần được kiểm lại.
- **[Chưa kiểm]** thông tin chưa được xác minh, người lập kế hoạch phải kiểm trước khi dựa vào.

---

## 1. Ý định và lý do

Người dùng mỗi lần cần đổi ảnh sang PDF lại phải lên Google, tìm một trang web, tải file lên, chờ rồi tải về. Họ muốn một app cài sẵn trên máy làm việc đó trong vài giây, và rộng hơn là mọi việc "lặt vặt" với file: đổi định dạng, nén, xoá metadata, che thông tin trong tài liệu, chú thích ảnh.

App tham chiếu là **Tangerine** (chỉ có trên Mac, mua một lần). Người dùng muốn một app tương tự trên **Linux và Windows, viết bằng Rust**, giao diện giống app gốc.

Điều đã biết về Tangerine (từ trang Product Hunt và bài của MacGeneration; **chưa xem video demo** vì cần đăng nhập Facebook):
- Chạy ở menu bar, xử lý offline, không tải file lên.
- Giữ Shift rồi kéo file thì hiện một "vòng tròn nhỏ" (*une petite roue*) gồm các định dạng đầu ra, chọn một là chuyển ngay. Giữ thêm Option thì ra menu công cụ.
- Định dạng đầu ra đổi theo loại file: ảnh (JPEG, PNG, HEIC, WebP, TIFF, BMP, SVG, GIF, PDF), âm thanh (MP3, M4A, WAV, FLAC, OGG, Opus, AIFF, WMA), video (MP4, MOV, MKV, WebM…), phụ đề (SRT, VTT), nén (ZIP, TAR, GZIP, RAR).
- Công cụ: chú thích ảnh, xoá metadata, cắt, nén, đọc QR, tách và sắp xếp lại trang PDF, che thông tin (redact) tài liệu; với video thì cắt, đổi tốc độ, cắt khung, tách âm thanh.
- Tuỳ chỉnh phím tắt và chất lượng trong cài đặt. Miễn phí 20 lần dùng, sau đó mua một lần (17 €).

## 2. Mục tiêu, không mục tiêu, phản mục tiêu

### Mục tiêu
1. Chọn file trong trình quản lý file, chuột phải, bấm một múi trên vòng tròn là xong việc phổ biến nhất (ví dụ ba ảnh thành một PDF).
2. Mọi thứ chạy **offline** trên máy người dùng.
3. Chạy trên **Linux và Windows**.
4. Có **CLI** (`morphix`) làm được mọi việc mà giao diện làm được, dùng được trong script.
5. Phạm vi chức năng, làm theo thứ tự ở mục 9: ảnh, âm thanh, video, PDF, file nén, xoá metadata, che thông tin trong PDF, chú thích ảnh.

### Không mục tiêu (bản đầu)
- macOS, điện thoại, web.
- OCR, đọc/ghi file Office (docx, xlsx…), tải lên bất kỳ dịch vụ nào.
- Tạo file RAR (giấy phép đóng; chỉ cân nhắc giải nén sau).
- Dark mode, nhiều ngôn ngữ giao diện (bản đầu chỉ tiếng Việt, nhưng chuỗi chữ để trong một chỗ).
- Tự cập nhật. Menu top-level kiểu mới của Windows 11 (xem mục 6).
- Đổi tên hàng loạt theo mẫu (một người dùng Product Hunt gợi ý; chưa cần).

### Phản mục tiêu (chạy được mà vẫn là thất bại)
- Có file nào rời khỏi máy mà người dùng không chủ động.
- **Ghi đè hoặc làm mất file gốc.**
- Lỗi xảy ra mà người dùng không thấy gì (kể cả khi chạy từ CLI hay nền).
- Vòng tròn hiện chậm, khiến người dùng bấm xong phải đợi nhìn màn hình trống.
- Phải cài bằng quyền quản trị (admin) mới dùng được.

## 3. Những điều đã chốt với người dùng

- **[Đã chốt]** Ngôn ngữ Rust; hệ điều hành Linux và Windows.
- **[Đã chốt]** Cách kích hoạt: chuột phải vào file đã chọn, rồi một vòng tròn hiện ra tại con trỏ. Thêm icon ở khay hệ thống cho phép kéo thả file vào.
- **[Đã chốt]** Thêm CLI để thao tác trên terminal.
- **[Đã chốt]** Giao diện giống app gốc. Menu nhanh là **vòng tròn chia thành các múi** (mỗi mục là một phần của hình tròn, không phải nút rời). Chọn **phương án A** cho Menu nhanh và **phương án A** cho Khay.
- **[Đã chốt]** Màu nhấn **xanh ngọc `#0f766e`** (màu thứ hai trên thanh công cụ của wireframe).
- **[Đã chốt]** Thứ tự làm: đợt 1 là lõi cộng ảnh, xoá metadata, nén ảnh, ảnh sang PDF; các phần còn lại theo mục 9.
- Wireframe đã duyệt: `docs/superpowers/specs/assets/2026-10-19-morphix-wireframe.html` (mở bằng trình duyệt, cần mạng để tải Tailwind, font, icon; tham số `?man=menu&v=a&mau=mau&nhan=ngoc` và `?man=khay&v=a&mau=mau&nhan=ngoc`).

## 4. Trải nghiệm người dùng

### 4.1 Luồng chính
1. Người dùng chọn một hay nhiều file trong trình quản lý file, chuột phải, chọn **Morphix** (tên app **[Mình chọn]**, lấy từ tên thư mục).
2. Vòng tròn hiện **tại con trỏ**, ghi tóm tắt file ở giữa ("3 ảnh · 8,4 MB").
3. Người dùng bấm một múi. Vòng tròn đổi thành vòng tiến độ ("62% · còn khoảng 4 giây", nút Huỷ).
4. Xong: file mới nằm cạnh file gốc; dòng mới xuất hiện ở "Gần đây" của Khay. Vòng tròn tự đóng.

### 4.2 Menu nhanh (vòng tròn): đã chốt phương án A
- Vòng tròn đường kính 320px, chia đều thành N múi (6 đến 8), lỗ tròn ở giữa chứa tóm tắt file. Múi đầu nằm chính giữa phía trên và là múi **gợi ý**, tô màu nhấn; các múi khác xám nhạt và sáng lên khi rê chuột.
- Dưới vòng có bộ chuyển **Chuyển đổi | Công cụ**. Giữ phím **Alt** cũng chuyển sang Công cụ (tương ứng Option của Tangerine). Dưới cùng là dòng gợi ý phím: "Esc để đóng · giữ Alt để mở Công cụ".
- Nội dung các múi **theo loại file** (nguồn: wireframe, các loại ngoài wireframe là **[Mình chọn]**):

| Loại | Múi Chuyển đổi (múi đầu là gợi ý) | Múi Công cụ |
| --- | --- | --- |
| Ảnh | PDF gộp, JPG, PNG, WebP, AVIF, GIF, TIFF, BMP | Nén ảnh, Xoá metadata, Cắt ảnh, Chú thích, Đọc mã QR, Đổi kích thước |
| Video | MP4, MKV, WebM, MOV, GIF, MP3 | Cắt đoạn, Đổi tốc độ, Tách âm thanh, Nén video, Xoá metadata, Cắt khung hình |
| PDF | Gộp, PNG, JPG, TXT, WebP | Tách trang, Sắp xếp trang, Nén PDF, Che thông tin, Xoá metadata, Chú thích |
| Âm thanh **[Mình chọn]** | MP3, M4A, WAV, FLAC, OGG, Opus | Cắt đoạn, Nén (bitrate), Xoá metadata |
| File nén **[Mình chọn]** | Giải nén, ZIP, TAR.GZ, 7Z | Xem nội dung |

- "PDF gộp" khi chọn từ hai ảnh trở lên: **tạo một PDF duy nhất** **[Mình chọn]**; nếu chỉ một ảnh thì múi chỉ ghi "PDF". **Thứ tự trang:** trình quản lý file không giữ thứ tự người dùng bấm chọn, nên các trang xếp theo **tên file, sắp xếp tự nhiên** (`IMG_2` đứng trước `IMG_10`) **[Mình chọn]**. Bản đầu chưa có cách đổi thứ tự trong vòng tròn; ai cần thì đổi tên file hoặc dùng "Sắp xếp trang" ở đợt 3.
- Chọn nhiều file khác loại: chỉ hiện các múi hợp lệ cho **mọi** file được chọn **[Mình chọn]**. Không có múi nào hợp lệ thì hiện trạng thái Rỗng với câu "Hãy chọn các file cùng loại".
- Hơn 8 định dạng (ví dụ video có tới 14): giữ 7 múi phổ biến nhất, múi thứ 8 là "···" mở lớp thứ hai **[Mình chọn]**; chưa vẽ wireframe cho lớp này, người lập kế hoạch làm theo cùng ngôn ngữ thiết kế.
- Trạng thái (đều có trong wireframe): có dữ liệu, đang chạy, rỗng (loại file chưa hỗ trợ), lỗi (kèm nút Thử lại và Bỏ qua, nội dung lỗi nói được nguyên nhân).
- **Wayland:** không đặt được cửa sổ tại con trỏ, nên vòng hiện **giữa cửa sổ file manager** hoặc giữa màn hình. Windows và X11 đặt đúng tại con trỏ. **[Chưa kiểm]** cách lấy vị trí con trỏ trong từng môi trường.
- Bàn phím **[Mình chọn]**: Tab hoặc phím mũi tên đi qua các múi, Enter chọn, Esc đóng. Mỗi múi có `aria-label`.

### 4.3 Khay hệ thống (cửa sổ nhỏ): đã chốt phương án A
Cửa sổ 400px neo ở góc khay hệ thống:
1. Tiêu đề "Morphix" và nút Cài đặt.
2. Ô thả file (viền đứt): "Thả file vào đây · Hoặc chuột phải vào file → Morphix" và nút **Chọn file**. Thả file vào thì mở vòng tròn Menu nhanh như ở 4.2.
3. "Gần đây": tối đa 4 dòng (tên việc, kết quả hoặc thanh tiến độ, thời gian) kèm nút "Xoá lịch sử".
4. Chân: biểu tượng khiên và "Xử lý ngay trên máy, không tải lên", nút Thoát.

Trạng thái: có dữ liệu, đang chạy (khung chờ), rỗng ("Chưa chuyển file nào"), lỗi (banner đỏ nhạt trên cùng, dòng lỗi có nút Thử lại).

### 4.4 Phong cách thị giác [Đã chốt màu, còn lại theo wireframe]
Phẳng, không đổ bóng ở thân trang (chỉ lớp nổi như vòng tròn và cửa sổ khay có bóng nhẹ). Font Inter. Token màu lấy từ khối token của skill ui-ux (nền `#f4f4f6`, card trắng, viền rất nhạt), chỉ đổi `--primary` thành `#0f766e` (chữ trắng trên nền này đạt tương phản 5.5:1). Bo góc 12px cho khung lớn, 8px cho control thấp hơn 40px. Không dark mode ở bản đầu.

### 4.5 Chưa có wireframe (người lập kế hoạch cần thiết kế cùng ngôn ngữ)
- Màn **Cài đặt** (phím tắt, chất lượng, thư mục lưu, chạy khi khởi động).
- Các công cụ có giao diện riêng: **Cắt ảnh, Chú thích ảnh, Che thông tin PDF, Sắp xếp trang PDF, Cắt đoạn video**. Mỗi cái mở một cửa sổ riêng sau khi chọn múi tương ứng.
- Lớp "···" của vòng tròn.

## 5. Kiến trúc

### 5.1 Cấu trúc
Một Cargo workspace **[Mình chọn]**:
- `morphix-core` (thư viện): mọi xử lý file. **Không phụ thuộc giao diện**, không có trạng thái toàn cục. Gồm: nhận dạng loại file (theo nội dung, không theo đuôi), bảng "loại file nào làm được việc gì" (nguồn duy nhất cho các múi của vòng tròn, CLI và Khay), mô hình việc (`Job`: đầu vào, thao tác, tham số, trạng thái, tiến độ, đầu ra, lỗi), bộ chạy việc có huỷ và báo tiến độ.
- `morphix-cli` (binary `morphix`): dùng `clap`, chạy được không cần giao diện, dùng trong script.
- `morphix-gui` (binary, Tauri): khay hệ thống, cửa sổ vòng tròn, cửa sổ khay, cài đặt, cửa sổ công cụ.
- `morphix-integration` (thư viện nhỏ): cài và gỡ mục menu chuột phải và mục khởi động cùng máy, tách riêng theo hệ điều hành.

### 5.2 Công nghệ giao diện [Mình chọn, cần bạn kiểm]
**Tauri v2** (hiện là 2.12.4, **[Chưa kiểm]** bản cuối khi bắt đầu): phần lõi Rust, phần giao diện là HTML, CSS và TypeScript chạy trong webview của hệ điều hành (WebView2 trên Windows, WebKitGTK trên Linux). Lý do: wireframe đã duyệt là HTML + token Tailwind nên chép gần như nguyên, vòng tròn vẽ bằng SVG là tự nhiên, khay hệ thống và cửa sổ trong suốt có sẵn. Giao diện **không dùng framework**: TypeScript thuần, Vite, Tailwind v4 **[Mình chọn]**.

Đánh đổi cần nói rõ: người dùng Linux phải có WebKitGTK; app không phải "thuần Rust" về phần giao diện; khởi động nguội của webview chậm hơn một app đồ hoạ thuần. Lựa chọn thay thế là egui hoặc Slint (thuần Rust, một file chạy). Vì lõi (`morphix-core`) tách biệt giao diện, đổi sau vẫn được với chi phí viết lại phần giao diện.

### 5.3 Mô hình tiến trình
Để vòng tròn hiện ngay, app giữ **một tiến trình nền thường trực** (đang chạy ở khay hệ thống, cửa sổ vòng tròn tạo sẵn và ẩn):
- Mục menu chuột phải chạy `morphix menu <file…>`. Lệnh này là client mỏng, gửi danh sách file cho tiến trình nền qua kênh cục bộ (named pipe trên Windows, Unix socket trên Linux; crate `interprocess`, **[Chưa kiểm]** API), rồi thoát. Chưa có tiến trình nền thì khởi động nó.
- **Gom lô:** Windows dễ chạy lệnh nhiều lần cho nhiều file (xem mục 6), nên tiến trình nền gom các yêu cầu đến trong cửa sổ khoảng 300ms thành một lô **[Mình chọn]**.
- Một thể hiện duy nhất (single instance, `tauri-plugin-single-instance`).
- **Mục tiêu thời gian [Mình chọn]:** vòng tròn hiện không quá 150 ms sau khi bấm khi tiến trình nền đã chạy; khi phải khởi động nguội thì chấp nhận chậm hơn nhưng hiện trạng thái "Đang mở…".

### 5.4 Xử lý file: quy tắc cứng
- **Không bao giờ ghi đè file gốc.** Đầu ra nằm cạnh file gốc **[Mình chọn]** với hậu tố: `-gop` (gộp), `-nen` (nén), `-sach` (xoá metadata), đổi đuôi khi chuyển định dạng. Trùng tên thì thêm ` (1)`, ` (2)`.
- Ghi vào file tạm cùng thư mục rồi đổi tên (atomic). Huỷ hay lỗi giữa chừng thì xoá file tạm, không để lại file dở.
- Tên file ghép: PDF gộp đặt theo tên file đầu tiên cộng `-gop.pdf`.
- Preset chất lượng **[Mình chọn]**: Nhỏ gọn, Cân bằng (mặc định), Cao. Giá trị JPEG ban đầu 70 / 85 / 95; người lập kế hoạch có thể chỉnh sau khi đo.
- Mọi lỗi ghi vào file log (`tracing`) cùng ngữ cảnh đủ để điều tra, và hiện cho người dùng bằng một câu nói được nguyên nhân (ví dụ "file được bảo vệ bằng mật khẩu"). **Không bao giờ nuốt lỗi im lặng.**
- Lỗi một file trong lô không làm hỏng cả lô: các file còn lại vẫn chạy, kết quả báo từng file.

### 5.5 Lưu trữ cục bộ [Mình chọn]
- **Lịch sử** ("Gần đây" ở Khay) và **cài đặt** lưu thành file JSON trong thư mục dữ liệu của app theo từng hệ điều hành (`%APPDATA%` trên Windows, `~/.local/share` và `~/.config` trên Linux), không dùng cơ sở dữ liệu. Lịch sử giữ tối đa 50 dòng, ghi file tạm rồi đổi tên.
- File hỏng hoặc thiếu thì bắt lỗi, ghi log và dùng giá trị mặc định; không được làm app không khởi động được. (Quy tắc `AGENTS.md` mục 3B: bắt lỗi thiếu dữ liệu khi khởi động.)
- Lịch sử chỉ chứa đường dẫn, tên việc, kết quả và thời gian; không chứa nội dung file. Nút "Xoá lịch sử" xoá thật.

## 6. Tích hợp hệ điều hành

### Windows
- Mục menu chuột phải đăng ký **theo người dùng** (`HKCU\Software\Classes\…`, crate `winreg`), không cần quyền admin.
- **Nhiều file một lần:** lệnh kiểu cũ (`command` với `%1`) mặc định mỗi file chạy một tiến trình (kiểu mô hình chọn `Document`, giới hạn 15 mục). Đặt `MultiSelectModel = Player` để Windows gom tối đa 100 mục vào một lần gọi; vượt quá thì gom lô như 5.3. (Nguồn: tài liệu Microsoft về verb selection model; **[Chưa kiểm]** cách tiến trình nhận danh sách file thực tế khi dùng `Player` với lệnh đường dẫn, phải thử trên máy thật.)
- **Windows 11:** mục menu kiểu này xuất hiện dưới **"Show more options"**, không ở menu đầu. Menu đầu cần **sparse package + `IExplorerCommand` + chữ ký số**, làm phần **sau** (ngoài phạm vi bản đầu). Người dùng cần biết điều này trước.
- Khởi động cùng máy: `HKCU\…\Run`.

### Linux
Mỗi trình quản lý file làm một kiểu **[Chưa kiểm: chi tiết từng loại, người lập kế hoạch phải đọc tài liệu chính thức trước khi viết]**:
- **Nautilus (GNOME Files):** thư mục script `~/.local/share/nautilus/scripts`; script nhận danh sách file qua biến `NAUTILUS_SCRIPT_SELECTED_FILE_PATHS` (mỗi dòng một đường dẫn, **chỉ cho file cục bộ**; file từ mạng thì biến rỗng). Mục hiện trong menu con **Scripts**, không ở menu đầu. Menu đầu cần extension Python (`nautilus-python`), để sau.
- **Dolphin (KDE):** service menu bằng file `.desktop`. **[Chưa kiểm]** đường dẫn cho KDE 6.
- **Nemo:** file `.nemo_action`. **[Chưa kiểm]** định dạng.
- **Thunar:** custom actions. **[Chưa kiểm]** cách cài bằng chương trình.
- Làm Nautilus và Dolphin trước **[Mình chọn]**, hai cái kia sau.
- Khởi động cùng máy: file `.desktop` trong `~/.config/autostart/`.
- Khay hệ thống: GNOME bản gốc không hiện icon khay nếu thiếu extension AppIndicator **[Chưa kiểm]**; cần kiểm khi thử và ghi hướng dẫn cho người dùng.
- `morphix install-integration` và `morphix uninstall-integration` cài và gỡ các mục trên, in ra việc đã làm; gỡ phải sạch.

## 7. Thư viện dự kiến (phiên bản kiểm trên crates.io ngày 2026-10-19)

Đây là danh sách xuất phát; người lập kế hoạch kiểm tính phù hợp từng cái bằng một thử nghiệm nhỏ trước khi chốt.

| Việc | Crate dự kiến | Ghi chú |
| --- | --- | --- |
| Giao diện, khay, cửa sổ | `tauri` 2.12.4, `tauri-plugin-single-instance` 2.5.4 | xem 5.2 |
| CLI | `clap` 4.6.7 | |
| Lỗi, log | `thiserror` 2.0.21, `tracing` 0.1.44 | |
| IPC cục bộ | `interprocess` 2.4.6 | **[Chưa kiểm]** |
| Registry Windows | `winreg` 0.56.0 | |
| Nhận dạng loại file | `infer` 0.22.0 | theo nội dung |
| Đọc/ghi ảnh | `image` 0.25.10 | JPEG, PNG, GIF, BMP, TIFF, WebP (WebP có mất mát cần crate khác) |
| WebP có mất mát | `webp` 0.3.1 | cần thư viện C `libwebp` |
| AVIF | `ravif` 0.13.0 (ghi) | đọc AVIF **[Chưa kiểm]** |
| Đổi cỡ ảnh nhanh | `fast_image_resize` 6.1.0 | |
| Xoá metadata ảnh | `img-parts` 0.4.1 (xoá EXIF/ICC, không giải mã lại ảnh) | thay thế: `little_exif` 0.6.23 |
| Tạo PDF từ ảnh | `pdf-writer` 0.15.0 (nhúng thẳng JPEG, không nén lại) hoặc `printpdf` 0.12.8 | chọn sau thử nghiệm |
| Sửa PDF (gộp, tách, sắp xếp, metadata) | `lopdf` 0.45.0 | |
| Vẽ trang PDF thành ảnh | `pdfium-render` 0.9.4 | cần tệp thư viện PDFium đi kèm; đợt 3 |
| File nén | `zip` 9.0.5, `tar` 0.4.46, `flate2` 1.1.10, `sevenz-rust2` 0.24.1 | |
| Video, âm thanh | **chương trình `ffmpeg` ngoài** | xem dưới |

**ffmpeg [Mình chọn]:** không viết lại bộ mã hoá. Gọi chương trình `ffmpeg` ngoài. Bản Windows đóng kèm một bản ffmpeg LGPL cùng bộ cài (sidecar); trên Linux gói phụ thuộc vào gói `ffmpeg` của hệ thống, và nếu thiếu thì `morphix doctor` và vòng tròn báo rõ cách cài. Cần kiểm lại kích thước bộ cài và điều khoản giấy phép trước khi phát hành. **[Chưa kiểm]** HEIC (cần `libheif`, có vướng bằng sáng chế) chưa nằm trong phạm vi.

## 8. CLI

Dạng lệnh (tên cờ cuối cùng do người lập kế hoạch chốt, cần giữ nhất quán):

```
morphix convert <file…> --to <định dạng> [--merge] [-o <thư mục>] [--quality small|balanced|high]
morphix compress <file…>
morphix strip-meta <file…>
morphix formats [<file>]          # liệt kê việc làm được với loại file này
morphix menu <file…>              # mở vòng tròn (dùng bởi menu chuột phải)
morphix install-integration | uninstall-integration
morphix doctor                    # kiểm ffmpeg, tích hợp menu, khay, thư viện thiếu
```

- Cờ chung: `--json` (kết quả cho máy đọc), `--dry-run` (nói sẽ tạo file gì, không ghi).
- Mã thoát: `0` thành công, `1` có lỗi, `2` dùng sai lệnh, `3` làm được một phần (một số file lỗi). Lỗi in ra `stderr`, kết quả ra `stdout`.
- CLI và giao diện dùng chung `morphix-core`, nên **cùng kết quả** cho cùng đầu vào.
- Trên Windows, binary `morphix` là chương trình dòng lệnh (có console); `morphix-gui` là chương trình cửa sổ. Cả hai cài cùng nhau.

## 9. Các đợt làm

Mỗi đợt có kế hoạch riêng và đi qua cùng quy trình (thiết kế tiếp nếu cần, kế hoạch, làm, kiểm). **Chỉ đợt 1 được lên kế hoạch ngay sau khi tài liệu này được duyệt.**

1. **Đợt 1: nền tảng và ảnh.** Lõi, nhận dạng loại file, bộ chạy việc; ảnh (đổi định dạng, nén, đổi kích thước, **ảnh sang PDF gộp**), xoá metadata ảnh; CLI (`convert`, `compress`, `strip-meta`, `formats`, `doctor`); vòng tròn, Khay, mục menu chuột phải trên Windows (`HKCU`) và Nautilus + Dolphin; trạng thái đang chạy, rỗng, lỗi.
2. **Đợt 2: âm thanh và video** qua ffmpeg: đổi định dạng, nén, cắt đoạn, đổi tốc độ, tách âm thanh.
3. **Đợt 3: PDF:** gộp, tách, sắp xếp, nén, xoá metadata, PDF sang ảnh, **che thông tin**.
4. **Đợt 4: file nén** (ZIP, TAR.GZ, 7Z).
5. **Đợt 5: chú thích ảnh, cắt ảnh, đọc QR** (có giao diện riêng, nặng nhất).

Sau đợt 1: Nemo, Thunar, menu Windows 11 kiểu mới, lớp "···" của vòng tròn nếu chưa kịp.

### Tiêu chí hoàn thành đợt 1 (kiểm bằng chạy thật, không bằng đọc code)
- Trên Windows 10/11 và một bản Linux dùng GNOME và một bản dùng KDE: chọn 3 ảnh, chuột phải, chọn múi "PDF gộp", ra **đúng một** file PDF cạnh ảnh gốc, 3 trang, đúng thứ tự tên file (sắp tự nhiên), mở được. Ảnh gốc còn nguyên.
- Cùng thao tác bằng `morphix convert a.jpg b.jpg c.png --to pdf --merge` ra cùng kết quả.
- Huỷ giữa chừng không để lại file tạm. File lỗi (hỏng, có mật khẩu, không phải ảnh) cho ra câu lỗi đúng nguyên nhân, không treo, không crash.
- Vòng tròn hiện trong 150 ms khi tiến trình nền đang chạy (đo bằng nhật ký thời gian).
- `install-integration` rồi `uninstall-integration` để lại hệ thống sạch như trước.
- Mọi kiểm tra bắt buộc của `AGENTS.md` mục 4: linter không lỗi, test đạt hết, không còn lỗi ẩn trong log, `git status` sạch.

## 10. Rủi ro đã biết
1. **Wayland** không cho đặt cửa sổ tại con trỏ và hạn chế phím tắt toàn cục.
2. **Khay hệ thống trên GNOME** cần extension; nếu thiếu thì Khay không mở được, và cần đường lùi (ví dụ `morphix` không đối số mở cửa sổ Khay thường).
3. **Windows 11** menu đầu cần ký số và gói sparse (chi phí và quy trình ngoài kỹ thuật).
4. **Che thông tin PDF** phải **xoá thật nội dung bên dưới**, không chỉ vẽ ô đen đè lên (nếu không chữ vẫn sao chép ra được). Hướng an toàn: chuyển trang thành ảnh rồi ghép lại (mất khả năng chọn chữ); cần `pdfium-render`.
5. **Giấy phép và bằng sáng chế:** ffmpeg (LGPL hay GPL tuỳ bản dựng), HEIC, RAR.
6. **Ký số bộ cài Windows:** chưa ký thì SmartScreen cảnh báo.
7. **Phụ thuộc webview:** WebKitGTK trên Linux (phiên bản phải khớp bản Tauri dùng).

## 11. Những điều để cho người lập kế hoạch
- Tên cờ CLI cụ thể, cấu trúc module trong từng crate, cách đặt tên test.
- Cách chọn giữa `pdf-writer` và `printpdf`, `img-parts` và `little_exif`, bằng thử nghiệm.
- Cách đo thời gian hiện vòng tròn.
- Thiết kế chi tiết các phần ở mục 4.5, theo cùng ngôn ngữ với wireframe.
- Cách kiểm các mục **[Chưa kiểm]**: làm một thử nghiệm nhỏ cho mỗi mục trước khi viết code thật.

## 12. Câu hỏi đã được người dùng trả lời (2026-10-19)

1. **[Đã chốt]** Giao diện dùng **Tauri** (webview). Mục 5.2 không còn là "Mình chọn".
2. **[Đã chốt]** App **miễn phí**. Không có cơ chế cấp phép hay giới hạn số lần dùng. Việc còn lại: chọn giấy phép mã nguồn cho dự án (chưa quyết, chưa chặn việc làm đợt 1) và kiểm giấy phép các thư viện đi kèm (ffmpeg, PDFium) khi phát hành. Bản Windows chưa ký số thì SmartScreen vẫn cảnh báo (mục 10).
3. **[Đã chốt]** Chọn nhiều ảnh thì "PDF gộp" tạo **một file PDF duy nhất**.
4. **[Đã chốt]** Tên app giữ là **Morphix**.
