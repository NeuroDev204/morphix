# Morphix hoàn chỉnh cho Ubuntu 24.04

Trạng thái: người dùng duyệt phạm vi trải nghiệm, chọn triển khai Native và duyệt bản vá DING cấp package ngày 2026-10-20. Cài/gỡ Desktop do Morphix package quản lý bằng `dpkg-divert`, cần quyền quản trị, phụ thuộc phiên bản DING chính xác và phục hồi file gốc khi gỡ.

## Mục tiêu đã chốt

Giữ menu chuột phải mặc định, thêm đúng một mục Morphix trong Files/Nautilus và trên Desktop, gồm menu của file và menu vùng trống. Chọn Morphix trên file mở vòng chọn định dạng; chọn trên vùng trống mở ứng dụng chọn/kéo thả file. Có launcher Morphix trong danh sách ứng dụng. Chuyển đổi cục bộ bằng lõi hiện có. Đầu ra là gói .deb cài được và luồng sử dụng đã kiểm thử trên Ubuntu 24.04.

Không mở rộng sang menu của mọi ứng dụng. Không thêm định dạng hoặc công cụ chỉnh sửa mới trong đợt này. Thư mục, biểu tượng hệ thống và lựa chọn không chứa file cục bộ chuyển được sẽ mở app với thông báo phù hợp; không tự duyệt đệ quy thư mục.

## Bằng chứng từ source và môi trường

- Cargo workspace có core, CLI, IPC và GUI Tauri; GUI có vòng chọn định dạng, Khay, chọn/kéo thả file, lịch sử và cài đặt.
- `crates/morphix-cli/src/integration/linux.rs` hiện cài Nautilus script dưới menu Scripts, Dolphin service menu và autostart; chưa có Nautilus MenuProvider.
- `scripts/build-deb.sh` dựng CLI/GUI và đóng kèm FFmpeg/PDFium; chưa cài launcher và tích hợp menu trực tiếp.
- `crates/morphix-gui/src/windows.rs::open_khay` trả lỗi khi đặt vị trí thất bại trước khi hiện cửa sổ; cần kiểm tra và xử lý fallback.
- Máy khảo sát báo Ubuntu 24.04.5. DING tại `/usr/share/gnome-shell/extensions/ding@rastersoft.com` khai báo GNOME Shell 45/46. `app/fileItemMenu.js` tạo menu file; `app/desktopManager.js::_createDesktopBackgroundMenu` tạo menu nền. Hai luồng này không gọi Nautilus MenuProvider.
- Git đang có file chưa theo dõi trong Morphix và thay đổi ngoài dự án ở repository cha. Chỉ commit tài liệu/thay đổi thuộc công việc này; không dọn hoặc commit thay đổi khác.

## Quyết định đã duyệt

Người dùng đã chọn giữ menu Desktop trực tiếp, để gói Morphix quản lý DING và chấp nhận quyền quản trị cùng việc theo dõi cập nhật. Probe trên Ubuntu 24.04 GNOME 46 chứng minh bản DING theo người dùng không được nạp trong session `ubuntu`; người dùng đã duyệt thiết kế package-level dưới đây.

1. **Bản vá DING do gói Morphix quản lý — đã duyệt.** Dùng bản DING của Ubuntu 24.04 đúng phiên bản/hash đã kiểm tra. Gói Morphix khai báo phụ thuộc đúng phiên bản DING được hỗ trợ, dùng `dpkg-divert` để giữ bản gốc và cài hai file đã vá tại đúng vị trí hệ thống. Bản vá chỉ thêm mục Morphix cho menu file và nền, không thay toàn bộ DING. Package manager cần quyền quản trị. Doctor kiểm phiên bản, hash và diversion; gói Morphix mới phải phát hành cùng phiên bản DING mới trước khi apt nâng cấp DING. Khi gỡ Morphix, khôi phục file DING nguyên bản. Không áp patch lên phiên bản chưa hỗ trợ.
2. **Đóng gói hoặc thay thế toàn bộ DING bằng bản fork Morphix.** Tạo xung đột với gói Ubuntu, tăng lượng mã cần duy trì và khiến cập nhật upstream khó hơn; không đề xuất.
3. **Không tích hợp Desktop.** Chỉ giữ provider Nautilus. Cách này không đáp ứng phạm vi menu Desktop đã chốt.

Người dùng đã duyệt phương án 1 trước khi sửa package, maintainer scripts hoặc file DING hệ thống. Nếu file DING bị sửa/bị divert bởi bên khác, cài đặt phải dừng mà không ghi đè. Nếu phiên bản DING không khớp đúng phiên bản package Morphix, apt phải giữ tính nhất quán qua ràng buộc Depends; doctor phải báo cần bản Morphix tương thích. Cài/gỡ không chỉnh danh sách extension đang bật, không ghi vào HOME và không khởi động lại GNOME Shell.

## Thành phần và luồng dữ liệu

### Files

Thêm provider Python dùng Nautilus API 4, hỗ trợ `get_file_items` và `get_background_items`. Truyền URI theo argv riêng tới `morphix menu --uris`; vùng trống gọi đường mở app hiện có `morphix tray`. Không ghép tên file thành shell command. Mục Morphix vẫn xuất hiện với lựa chọn không hỗ trợ; app giải thích trạng thái thay vì chạy conversion sai.

Nguồn API: https://gnome.pages.gitlab.gnome.org/nautilus-python/nautilus-python-migrating-to-4.html và https://gnome.pages.gitlab.gnome.org/nautilus/iface.MenuProvider.html

### Desktop

Dùng bản DING của Ubuntu 24.04 đã kiểm tra làm nền, giữ hành vi menu và icon. Gói Morphix phụ thuộc chính xác vào phiên bản package DING được hỗ trợ; gói build kèm hai file đã vá theo source/hash đó. Maintainer script dùng `dpkg-divert` để chuyển file DING gốc sang tên giữ lại, sau đó package Morphix cung cấp file tại đường dẫn gốc. Bản vá chỉ thêm mục Morphix vào menu lựa chọn và nền, lấy toàn bộ lựa chọn từ DING và gọi cùng CLI như Nautilus. Giữ license cùng thông tin nguồn DING.

Đây là tích hợp hệ thống, áp dụng cho mọi người dùng Desktop và cần quyền quản trị khi cài/gỡ package. Cài đặt từ chối version/hash không khớp, file có thay đổi cục bộ hoặc diversion của package khác. Doctor kiểm package version, SHA-256 file gốc đã divert, SHA-256 hai file Morphix cung cấp và trạng thái diversion. Ràng buộc Depends buộc nâng Morphix cùng lần khi DING đổi version; không tải source hoặc tự vá version lạ. Gỡ package khôi phục bản DING đang được dpkg giữ lại. Không thay cài đặt extension, không ghi vào HOME, không tự khởi động lại Shell; đăng xuất/đăng nhập để nạp thay đổi.

### App và vòng tròn

Tái sử dụng Khay làm cửa sổ ứng dụng chọn file, cho phép xuất hiện trong thanh tác vụ và mở từ launcher. Đường khởi động nền phải tiếp tục chạy ẩn. Chọn/kéo thả file dùng chung catalog và job với vòng tròn, hiển thị tiến độ, hủy và lỗi từng file; kết quả có đường mở vị trí lưu.

Vòng tròn vẫn là cửa sổ riêng. X11 đặt gần con trỏ khi lấy được vị trí; Wayland dùng vị trí compositor cho phép, không để lỗi định vị chặn hiện cửa sổ. Không cam kết tọa độ chính xác trên Wayland.

### Bộ cài và vận hành

Gói .deb amd64 chứa launcher/icon, provider Nautilus, hai file DING đã vá và các binary hiện có; khai báo dependency Nautilus/GTK/DING với phiên bản chính xác. Maintainer scripts chạy dưới quyền package manager và không ghi vào HOME của root. Package cài/gỡ Desktop tự động; app chỉ hiển thị trạng thái và hướng xử lý, không tự gọi quyền root. Bộ cài không khởi động lại phiên desktop.

Mở rộng `doctor` để báo trạng thái Files/Desktop, phiên bản DING và tính toàn vẹn diversion. `install-integration`/`uninstall-integration` tiếp tục chỉ quản lý mục tùy chọn theo người dùng; không được xóa hay sửa file hệ thống thuộc package Morphix. Giữ hỗ trợ hiện có ngoài Ubuntu, không tái cấu trúc backend không liên quan.

## An toàn dữ liệu và xử lý lỗi

Giữ quy tắc không ghi đè nguồn, tránh trùng tên, xuất file nguyên tử, hủy không để đầu ra dở. Kiểm tra các quy tắc này trong core dùng chung trước khi sửa UI. URI mạng không được biến thành đường dẫn cục bộ giả. Thiếu decoder/FFmpeg/PDFium hoặc file hỏng phải báo nguyên nhân và ghi log. Extension không làm file manager chờ conversion; chỉ khởi chạy client và trả lại điều khiển.

## Phạm vi thay đổi và kiểm thử

Các vùng cần xét: integration backend, ComponentId/report/doctor và caller CLI; provider Nautilus mới; adapter/tài nguyên DING mới; GUI windows/startup/Khay và test UI; packaging/build/check scripts, dependency và tài liệu cài/gỡ. Kiểm thử maintainer scripts bằng dpkg database/root giả ở thư mục tạm, không chạy diversion trên máy phát triển. Cập nhật fixture/test tương ứng hợp đồng thay đổi; chỉ sửa core khi có lỗi được tái hiện.

Kiểm tra Rust fmt/clippy/test, TypeScript check/test/build và kiểm tra provider/adapter. Test đường dẫn Unicode, khoảng trắng, dấu nháy, ký tự shell, URI không cục bộ, nhiều file, file hỏng, hủy, trùng tên và khởi động lại ứng dụng. Kiểm tra cài/gỡ không đụng tài nguyên người dùng.

Nghiệm thu desktop thật trên Ubuntu 24.04 GNOME 46, cả Wayland và X11: bốn vị trí menu (Files/file, Files/nền, Desktop/file, Desktop/nền), mở nguội và mở khi app đang chạy, kéo thả/chọn file, conversion đại diện cho từng nhóm core đang hỗ trợ, giữ nguyên file nguồn và kiểm tra nội dung kết quả. Kiểm tra cài gói trên môi trường sạch, launcher, kích hoạt integration, cập nhật/gỡ và khôi phục DING. Đọc log sau các thao tác. Những môi trường không chạy được phải ghi là chưa kiểm; không coi test mô phỏng là desktop nghiệm thu.

## Điều kiện hoàn tất

Giao đủ .deb và hướng dẫn cài/kích hoạt/gỡ; menu trực tiếp hoạt động ở cả Files và Desktop; app tự chọn file hoạt động; kiểm tra bắt buộc đạt và bằng chứng kiểm thử desktop được ghi lại. Workspace không có thay đổi chưa commit do công việc này; thay đổi sẵn có của người dùng giữ nguyên và được báo riêng.

## Trạng thái phê duyệt

Người dùng duyệt phạm vi trải nghiệm, chọn Native, và duyệt bản vá DING package-level do Morphix quản lý ngày 2026-10-20. Cài/gỡ qua package manager cần quyền quản trị; package ghim chính xác phiên bản DING hỗ trợ, giữ file gốc bằng `dpkg-divert`, và phục hồi file đó khi gỡ.
