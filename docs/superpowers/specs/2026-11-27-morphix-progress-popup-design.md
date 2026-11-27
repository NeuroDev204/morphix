# Popup tiến trình khi chạy việc từ menu: thiết kế (bản ngắn)

Ngày: 2026-11-27. Chủ dự án duyệt phác thảo này cùng thiết kế "menu desktop qua curl" (cùng phát hành v0.1.7, làm sau).

## Ý định

Bấm một việc từ menu chuột phải (Files, desktop, Dolphin, Thunar, Windows Explorer…) thì việc chạy ngầm trong `morphix-gui`;
người dùng không thấy gì cho tới thông báo "xong". Việc dài (video) trông như không có gì xảy ra. Chủ dự án muốn một popup tiến trình.

## Hiện trạng

- Menu → IPC → `lib.rs:151` `commands::start_action(app, id, paths, notify = true)` → `JobRunner` (một việc một lúc) →
  `publish_update` phát `JOB_UPDATE_EVENT` / `JOB_FINISHED_EVENT`, ghi lịch sử, gửi thông báo hệ thống khi `notify`.
- `JobSnapshot { id, label, done, total, state }`; tiến trình chỉ theo **số mục** (`Progress { done, total }`), không có % trong một file.
- Cửa sổ Tauri: `khay` (440×700, khai trong `tauri.conf.json`), `settings` (tạo lười trong `windows.rs`); mỗi cửa sổ một trang HTML trong `ui/vite.config.ts`.

## Cách làm (một cách hợp lý)

Một cửa sổ Tauri mới, nhỏ, dùng lại sự kiện job sẵn có. Loại: thông báo hệ thống có thanh tiến trình (không đồng nhất giữa Linux/Windows,
không có nút Huỷ chắc chắn); dùng cửa sổ Khay (quá to, che màn hình).

## Thiết kế (đã duyệt theo phác thảo)

```
┌──────────────────────────────── ✕ ┐
│ Morphix · Đổi sang MP4            │
│ video-du-lich.mov  (2/5 file)     │
│ ███████████░░░░░░░░  40%          │
│                          [ Huỷ ]  │
└───────────────────────────────────┘
```

1. Chỉ việc chạy **từ menu** (`notify = true`) mới có popup; việc chạy từ Khay không (Khay đã hiện tiến trình).
2. Hiện khi việc còn chạy sau **1 giây** (việc nhanh không chớp popup). Góc dưới phải vùng làm việc (dùng lại `bottom_right_origin`), luôn trên cùng, không lấy focus, không hiện trên thanh tác vụ.
3. Nội dung: `label` của việc; tên file đầu tiên + "(done/total file)" (thêm "và N file khác" nếu nhiều file); thanh tiến trình; nút **Huỷ** (`cancel_job`).
4. **Quyết định của người viết:** lõi chỉ báo tiến trình theo số mục, nên `total = 1` (một file, ví dụ một video) thì thanh chạy **không xác định** (vạch chạy qua lại, chữ "Đang xử lý…") thay vì đứng 0% rồi nhảy 100%. Muốn % trong một file (đọc tiến trình ffmpeg) là việc sau.
5. Xong: đổi thành "Xong · [Mở thư mục]", tự đóng sau **4 giây** (rê chuột vào thì giữ). Lỗi: ghi lý do đầu tiên (như `notification_body`), **chỉ đóng khi bấm ✕**. Huỷ: "Đã huỷ", tự đóng sau 2 giây.
6. Việc mới chạy khi popup đang hiện kết quả cũ → popup chuyển sang việc mới.
7. Giữ thông báo hệ thống khi xong như cũ.
8. Linux và Windows như nhau (cùng `morphix-gui`).

## Không làm

% trong từng file; nhiều việc song song (JobRunner chỉ chạy một); đổi giao diện Khay.

## Để người dựng quyết

Nhãn cửa sổ/tên trang; tạo cửa sổ lúc khởi động hay lười (lưu ý chú thích `windows.rs`: tạo cửa sổ từ lệnh đồng bộ/event handler bị treo trên Windows);
cách giữ hẹn giờ 1 giây; cách lấy tên file đầu (từ `inputs` lúc `start_action`).

## Chưa kiểm

Wayland GNOME có thể bỏ qua vị trí cửa sổ do app đặt (popup có thể ra giữa màn hình) và bỏ qua "luôn trên cùng"; Windows chưa thử.
