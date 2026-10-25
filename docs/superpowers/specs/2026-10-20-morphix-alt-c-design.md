# Morphix: Alt+C mở Khay với file đang chọn

Tài liệu này bổ sung cho `2026-10-19-morphix-design.md`. Mục không nhắc ở đây giữ nguyên như spec gốc. **[Đã chốt]** là người dùng đã chốt; **[Mình chọn]** là quyết định của người viết, người dùng xem lại khi đọc; **[Chưa kiểm]** là chưa thử thật.

## 1. Ý định và lý do

Hiện tại nhấn Alt+C (phím tắt GNOME gọi `/usr/bin/morphix tray`) chỉ mở Khay trống, người dùng phải bấm "Chọn file" hoặc kéo thả. Người dùng muốn: chọn một hoặc nhiều file trong Files (Nautilus) hoặc trên desktop, nhấn Alt+C, **Khay mở ra đã có sẵn các file đó và các lựa chọn chuyển đổi**, bấm một nút là chạy. Không phải "upload" file thủ công nữa.

## 2. Mục tiêu, không mục tiêu, phản mục tiêu

### Mục tiêu
1. **[Đã chốt]** Alt+C trong Files (Nautilus) và trên desktop Ubuntu (extension `ding@rastersoft.com`) lấy **đủ mọi file đang chọn**, kể cả khi chọn nhiều hơn số dòng đang hiện trên màn hình (đã thử: Ctrl+A 53 file).
2. **[Đã chốt]** Chạy được trên **cả X11 và Wayland**.
3. **[Đã chốt]** Khay hiện danh sách file đã nhận và các nút lựa chọn chuyển đổi cho các file đó (xem mục 5).
4. **[Đã chốt]** Chuột phải vào file → Scripts → Morphix vẫn mở **vòng tròn đầy đủ như hiện có**, không đổi.

### Không mục tiêu
- Windows (Explorer, desktop): làm sau, theo cách hỏi Explorer qua Shell COM. Không thuộc đợt này. **[Đã chốt: Ubuntu trước]**
- Dolphin, Nemo, Thunar và trình quản lý file khác: Alt+C ở đó mở Khay trống như hiện tại. **[Mình chọn]**
- Nửa vòng tròn gắn cạnh menu chuột phải: người dùng đã bỏ ý này. **[Đã chốt]**
- Khay mở từ biểu tượng khay hệ thống (không qua Alt+C) vẫn như hiện tại. **[Mình chọn]**

### Phản mục tiêu
- Âm thầm nhận **thiếu** file (ví dụ chỉ các dòng đang thấy trên màn hình) mà vẫn hiện như đủ.
- Dùng **nội dung clipboard cũ** tưởng là file vừa chọn.
- **Làm mất nội dung clipboard** của người dùng: mượn xong phải trả lại.
- Lỗi khi lấy file mà Khay im lặng hoặc không mở.
- Sửa hay xoá dòng cấu hình không phải của Morphix (file phím tắt Nautilus, phím tắt GNOME).

## 3. Ràng buộc

- Kế thừa spec gốc: offline, không ghi đè file gốc, lỗi phải hiện cho người dùng và ghi log, chuỗi giao diện tiếng Việt nằm trong `ui/src/strings.ts`.
- Việc dùng app và `install-integration` chạy bằng quyền người dùng, không sudo.
- Nhiều chương trình khác cũng có Alt+C: phím tắt GNOME bắt trước, nên Alt+C sẽ không còn tới ứng dụng đang mở. Người dùng đã chọn Alt+C, giữ nguyên. **[Đã chốt]**

## 4. Cách lấy file đang chọn (đã thử trên máy người dùng, 2026-10-20)

Máy thử: Ubuntu 24.04, GNOME, Nautilus 46.4, phiên X11, desktop `ding@rastersoft.com`.

### 4.1 Những cách đã thử và loại

- **Công cụ trợ năng (AT-SPI) đọc trạng thái "đã chọn":** Nautilus chỉ đưa ra các dòng đang hiện (Ctrl+A trên 53 file chỉ đọc được 9 tên; nhãn trạng thái vẫn ghi "53 items selected"). Desktop không đánh dấu file nào được chọn, chỉ cho biết file đang được trỏ vào. **Loại**, vì vi phạm mục tiêu 1.
- **Hỏi Nautilus qua D-Bus:** không có hàm lấy file đang chọn. `org.freedesktop.FileManager1` chỉ có thuộc tính `OpenWindowsWithLocations` (thư mục của từng cửa sổ). **Loại.**
- **Bấm hộ bằng AT-SPI (`Atspi.generate_keyboard_event`):** Nautilus không nhận. **Loại.**

### 4.2 Cách được chọn

Khi Alt+C chạy:

1. **Đợi người dùng thả hết phím bổ trợ** (Alt, Ctrl, Shift, Super), tối đa khoảng 3 giây. Nếu không đợi, phím bấm hộ sẽ bị cộng thêm Alt đang giữ và không khớp. Đã thử: đợi 0,08–0,14 giây là đủ.
2. **Xác định cửa sổ đang được chọn:**
   - X11: `_NET_ACTIVE_WINDOW` rồi đọc `WM_CLASS`. Files là `org.gnome.Nautilus`. Desktop là `gjs` có tiêu đề bắt đầu bằng `Desktop Icons`, hoặc kiểu cửa sổ `_NET_WM_WINDOW_TYPE_DESKTOP`; người lập kế hoạch chọn dấu hiệu chắc nhất.
   - Wayland: không có `_NET_ACTIVE_WINDOW`. Dùng AT-SPI tìm khung (frame) có trạng thái ACTIVE và ứng dụng chứa nó (`org.gnome.Nautilus` hoặc `gjs` với khung `Desktop Icons N`). Đã thấy trên X11 rằng AT-SPI báo đúng khung ACTIVE. **[Chưa kiểm trên Wayland]**
3. **Files (Nautilus): gọi script Morphix bằng phím tắt riêng của Nautilus.**
   - Nautilus đọc `~/.config/nautilus/scripts-accels`, mỗi dòng `<accel> <tên script>`. Khi phím tắt chạy, Nautilus gọi script với `NAUTILUS_SCRIPT_SELECTED_FILE_PATHS` chứa **đủ đường dẫn mọi file đang chọn**. Đã thử: 2 file và 53 file đều đủ.
   - Phím tắt riêng **[Mình chọn]**: `<Control><Shift>F7`. Không dùng tổ hợp có Ctrl+Alt+F… vì X11 dùng nó để chuyển sang console.
   - Morphix bấm hộ phím tắt đó tới cửa sổ Files. Giữa từng sự kiện phím có nghỉ khoảng 30 ms (desktop không nhận khi bấm dồn; giữ khoảng nghỉ cho cả hai cho đơn giản).
   - Nautilus chỉ đọc file phím tắt khi khởi động. Cửa sổ Files đóng nhưng tiến trình vẫn chạy ngầm, nên sau khi cài phải chạy `nautilus -q` hoặc đăng xuất. `install-integration` **báo điều này cho người dùng**, không tự tắt Nautilus. **[Mình chọn]**
4. **Desktop: mượn clipboard.** **[Đã chốt]**
   - Lưu nội dung clipboard hiện tại (mọi định dạng có trong đó), bấm hộ Ctrl+C tới desktop, **chờ clipboard thật sự đổi** (chủ sở hữu hoặc dấu thời gian đổi, không chỉ "có định dạng file"), đọc `x-special/gnome-copied-files` (dạng `copy\nfile:///…\nfile:///…`), rồi trả lại nội dung cũ.
   - Hết khoảng 1 giây mà clipboard không đổi: coi như không có file nào được chọn, Khay mở trống kèm thông báo. **[Mình chọn]**
   - Đã thử: tự nhấn Ctrl+C trên desktop thì nhận đủ 2 file; bấm hộ (có nghỉ 30 ms) cũng nhận được. Một lần thử đọc nhầm clipboard cũ, nên điều kiện "chờ clipboard đổi" là bắt buộc.
   - Trả lại clipboard cần một tiến trình sống lâu giữ quyền sở hữu clipboard. Vì vậy việc này nên do `morphix-gui` làm, không do CLI chạy rồi thoát.
5. **Nơi khác (trình duyệt, terminal, Dolphin…) hoặc không có file nào được chọn:** Khay mở như hiện tại.

### 4.3 Bấm hộ phím: X11 và Wayland

- **X11:** dùng XTest (`XTestFakeKeyEvent`), không hỏi quyền. Đã thử và chạy được.
- **Wayland:** **[Đã chốt]** xin quyền **một lần** qua cổng chính thức `org.freedesktop.portal.RemoteDesktop` của xdg-desktop-portal: tạo phiên chỉ có bàn phím, `persist_mode` = lưu lâu dài, cất `restore_token` để các lần sau không hỏi lại. Bấm hộ bằng `NotifyKeyboardKeysym` hoặc `NotifyKeyboardKeycode`. **[Chưa kiểm]**
  - Người dùng từ chối quyền, hoặc cổng không có: Khay mở trống với dòng giải thích cách cấp lại quyền. **[Mình chọn]**
  - **Đọc và trả lại clipboard trên Wayland:** Wayland chỉ cho cửa sổ đang được chọn đọc clipboard. Hướng dự kiến: bấm Ctrl+C khi desktop còn đang được chọn, rồi đọc và trả lại clipboard từ cửa sổ Khay ngay khi Khay được chọn; hoặc dùng cổng Clipboard của phiên RemoteDesktop. **[Chưa kiểm]**
- **Việc đầu tiên của kế hoạch phải là thử trên phiên Wayland** (bấm hộ qua cổng tới Nautilus, Ctrl+C tới desktop, đọc và trả clipboard, nhận khung ACTIVE qua AT-SPI). Nếu một phần không làm được, dừng và báo người dùng trước khi làm tiếp. Người dùng phải đăng xuất và đăng nhập phiên "Ubuntu" (Wayland) để thử.

### 4.4 Phân biệt "script chạy từ Alt+C" với "script chạy từ chuột phải"

Nautilus gọi cùng một script theo cùng một cách, dù từ menu chuột phải hay từ phím tắt. Chuột phải phải mở vòng tròn, còn Alt+C phải mở Khay.

**[Mình chọn]** Dùng **một script `Morphix` duy nhất** (như hiện có). Trước khi bấm hộ, Alt+C báo GUI "đang chờ file cho Khay", hạn khoảng 2 giây. Yêu cầu `menu` tới trong hạn đó thì được chuyển sang Khay; ngoài hạn thì mở vòng tròn như cũ. Lý do: không thêm mục thứ hai vào menu Scripts của người dùng. Người lập kế hoạch có thể đổi sang script riêng nếu cách này không chắc chắn, nhưng phải ghi lý do.

## 5. Khay khi có file từ Alt+C

**[Đã chốt]** (người dùng duyệt mô tả này):
- **Đầu Khay:** giữ như cũ (Morphix, nút Cài đặt).
- **Vùng kéo thả được thay bằng danh sách file đã nhận**, ví dụ "2 file: bài tập buổi 5.docx, bài tập buổi 6.docx"; nhiều file thì hiện vài tên đầu và "+51 file khác". Có nút **"Bỏ, chọn file khác"** để quay về vùng kéo thả.
- **Các nút lựa chọn chuyển đổi** ngay dưới, lấy **đúng danh sách múi mà vòng tròn sẽ hiện** cho các file đó (cùng nguồn `catalog`, không viết bảng riêng). Bấm một nút là chạy luôn; tiến độ hiện ở mục "Gần đây" như hiện tại.
- **Không có lựa chọn chung** (ví dụ 1 ảnh và 1 .docx): báo lý do, như vòng tròn đang báo.
- **Lấy file thất bại hoặc chỉ được một phần:** dòng lỗi đỏ ở banner hiện có, giữ vùng kéo thả để người dùng tự chọn.
- **"Gần đây" và chân trang:** giữ nguyên.

Các lựa chọn có tham số (ví dụ chọn chất lượng, cắt đoạn) xử lý giống cách vòng tròn đang làm. Người lập kế hoạch đọc `ui/src/wheel.ts` và dùng lại logic chứ không chép.

## 6. Cài đặt và gỡ

`morphix install-integration` (Linux, GNOME) cài thêm:
- **Phím tắt GNOME Alt+C** tại đúng đường dẫn của Morphix `/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/morphix/` và thêm đường dẫn đó vào danh sách `custom-keybindings` mà không xoá phần tử khác. Hiện phím tắt này mới được đặt bằng tay trên máy người dùng, chưa có trong code. Nếu tổ hợp Alt+C đã được một phím tắt khác dùng, báo xung đột và không ghi đè. **[Mình chọn]**
- **Dòng phím tắt trong `~/.config/nautilus/scripts-accels`**: thêm dòng của Morphix, giữ mọi dòng khác. Cách đánh dấu "của Morphix" phải theo quy tắc ownership hiện có. **[Chưa kiểm]** cú pháp chú thích của file này với Nautilus 46; nếu Nautilus không bỏ qua dòng chú thích thì nhận dạng dòng của Morphix bằng đúng nội dung dòng đã ghi.

`uninstall-integration` gỡ đúng các phần trên và chỉ gỡ phần của Morphix. `doctor` báo trạng thái của chúng.

Gói `.deb` hiện chưa có `libXtst` trong danh sách thư viện. Nếu dùng XTest thì thêm vào gói và kiểm bằng `scripts/check-deb.sh`.

## 7. Tiêu chí hoàn thành (chạy thật trên máy người dùng)

Trên **cả X11 và Wayland**:
1. Files: chọn 2 file, Alt+C → Khay hiện đúng 2 file và lựa chọn; Ctrl+A trong thư mục 53 file → đủ 53.
2. Desktop: chọn 2 file, Alt+C → Khay hiện đúng 2 file; clipboard sau đó **giống hệt trước** khi nhấn (thử với văn bản trong clipboard).
3. Không chọn gì trên desktop, hoặc đang ở trình duyệt → Khay trống, không lấy nhầm clipboard cũ.
4. Chuột phải → Scripts → Morphix vẫn mở vòng tròn.
5. Wayland: lần đầu hỏi quyền, lần sau không hỏi; từ chối thì Khay mở trống kèm hướng dẫn.
6. `install-integration` rồi `uninstall-integration`: các dòng cấu hình không phải của Morphix không đổi.

## 8. Để người lập kế hoạch quyết

- Thư viện Rust cho XTest, clipboard X11, AT-SPI và cổng RemoteDesktop (ưu tiên crate đã có trong `Cargo.lock`, ghi giấy phép).
- Phần nào chạy trong CLI, phần nào trong GUI, và lệnh Alt+C mới gọi gì (ví dụ `morphix tray --selection`), miễn là đáp ứng ràng buộc trả lại clipboard ở 4.2.
- Cách giao tiếp "đang chờ file cho Khay" giữa CLI và GUI qua IPC hiện có.
- Bố cục chi tiết, chữ trên các nút trong Khay (theo phong cách hiện có).

## 9. Quyết định bổ sung sau khi người đọc kiểm tra (đều là **[Mình chọn]**)

1. **Phiên bản hỗ trợ:** đợt này nhắm **Ubuntu 24.04 (GNOME 46, Nautilus 46.4)**, đã thử ở đó. Trên Ubuntu 22.04 (Nautilus 42, xdg-desktop-portal 1.14) tính năng **tự tắt**: Alt+C mở Khay trống kèm thông báo "chưa hỗ trợ trên phiên bản này". Lý do: `persist_mode`/`restore_token` của cổng RemoteDesktop và việc Nautilus 42 xử lý `scripts-accels` chưa được kiểm. Nếu người lập kế hoạch kiểm được 22.04 chạy đúng thì có thể mở lại, nhưng phải thử thật trên 22.04 trước.
2. **Lệnh Alt+C:** `morphix tray` giữ nguyên là "chỉ mở Khay" (biểu tượng khay, `.desktop` và autostart không đổi). Lệnh mới là `morphix tray --selection`. `install-integration` **thay** phím tắt Alt+C hiện có nếu lệnh của nó đúng bằng `<đường dẫn morphix> tray` (phím tắt thủ công cũ của người dùng); lệnh khác thì báo xung đột, không ghi đè.
3. **Clipboard không lưu trả lại trọn vẹn được** (đọc một định dạng lỗi, tổng dung lượng quá 10 MiB, hoặc nội dung được đánh dấu nhạy cảm): **bỏ qua việc lấy file**, không bấm Ctrl+C, Khay mở trống kèm thông báo nói rõ lý do. Không bao giờ chấp nhận nguy cơ mất clipboard. Trình quản lý lịch sử clipboard có thể ghi lại danh sách file tạm thời; chấp nhận được và ghi trong tài liệu hướng dẫn.
4. **Hết hạn chờ** ở Files (không có file nào tới sau khoảng 2 giây): luôn hiện thông báo "không nhận được file nào" trong Khay, không im lặng.
5. **GUI chưa chạy** khi nhấn Alt+C: khởi động GUI trước, rồi mới báo "đang chờ file cho Khay".
