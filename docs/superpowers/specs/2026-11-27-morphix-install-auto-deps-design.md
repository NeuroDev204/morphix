# install.sh tự cài gói hệ thống còn thiếu: thiết kế (bản ngắn)

Ngày: 2026-11-27. Phạm vi: `scripts/install.sh`, `scripts/tests/test_install_sh.py`, README, 2 câu trên website.

## Ý định

Người dùng Ubuntu 26.04 cài bằng `curl … install.sh | sh` nhận menu dự phòng "Scripts → Morphix"
(40 file tĩnh) thay vì menu động, vì thiếu `python3-nautilus` và không ai báo. Chủ dự án muốn: **thiếu gì thì
lệnh cài tự cài cái đó**, sau khi hỏi người dùng.

## Mục tiêu

1. Kiểm tra cả 2 thư viện bắt buộc (`webkit2gtk-4.1`, `libayatana-appindicator3`) **và** binding Python của
   Nautilus (chỉ khi máy có lệnh `nautilus`).
2. Thiếu gì: in danh sách, hỏi **một lần** `Cài luôn bằng sudo? [Y/n]` (Enter = có), rồi chạy lệnh cài của bản Linux.
3. Chạy lại đúng lệnh `curl … | sh` trên máy đã cài cũng chữa được (bước này chạy trước bước cài menu).

## Không làm

- Không thêm Rust / `morphix doctor --fix`; không đổi gói `.deb`; không đụng Windows.
- Không tự tắt Files (`nautilus -q`) vì đóng cửa sổ người dùng đang mở; chỉ in "Đóng hết cửa sổ Files rồi mở lại".
- Không `apt update` tự động.

## Điều làm hỏng tính năng dù chạy được

- Chạy sudo mà không hỏi, hoặc hỏi khi không có ai ngồi trước máy (treo CI/script).
- Thiếu Nautilus binding làm hỏng cả lần cài (nó chỉ là tuỳ chọn).

## Cách làm đã chọn

| Cách | Đánh đổi | Chọn |
|---|---|---|
| 1. Mở rộng bước 4 của `install.sh` | Ít code, dùng lại bảng tên gói và test có sẵn; chạy trước bước cài menu | **Chủ dự án chọn** |
| 2. `morphix doctor --fix` bằng Rust | Nhiều code hơn, phải chờ bản phát hành mới; chạy lại curl đã chữa được máy cũ | Bỏ |

Hỏi trước (A) thay vì cài thẳng (B): chủ dự án chọn, để người dùng biết vì sao bị hỏi mật khẩu.

## Thiết kế

1. **Phát hiện**: như hiện tại cho 2 thư viện (ldconfig). Nautilus: có `command -v nautilus` mà không có
   `libnautilus-python.so` trong 4 thư mục giống `NAUTILUS_EXTENSION_DIRS` của Rust
   (`/usr/lib64/nautilus/extensions-4`, `/usr/lib/nautilus/extensions-4`,
   `/usr/lib/x86_64-linux-gnu/nautilus/extensions-4`, `/usr/lib/nautilus/extensions-3.0`).
   Biến test `MORPHIX_NAUTILUS_DIRS` (danh sách cách nhau bởi dấu cách) thay 4 thư mục này.
2. **Tên gói theo bản Linux** (theo `ID`/`ID_LIKE`, bảng README):

   | Họ | Lệnh | webkit | appindicator | Nautilus |
   |---|---|---|---|---|
   | debian/ubuntu | `apt-get install -y` | libwebkit2gtk-4.1-0 | libayatana-appindicator3-1 | python3-nautilus |
   | fedora | `dnf install -y` | webkit2gtk4.1 | libayatana-appindicator-gtk3 | nautilus-python |
   | opensuse/suse | `zypper --non-interactive install` | libwebkit2gtk-4_1-0 | libayatana-appindicator3-1 | nautilus-python |
   | arch | `pacman -S --needed --noconfirm` | webkit2gtk-4.1 | libayatana-appindicator | python-nautilus (*chưa kiểm*) |

   Bản Linux lạ: không tự cài, in danh sách thư viện như hiện tại.
3. **Hỏi**: đọc trả lời từ `/dev/tty` (stdin đang là script do `curl |`). Biến test `MORPHIX_TTY`.
   Không mở được tty → không hỏi, không cài, in lệnh như hiện tại.
4. **Chạy**: `id -u` = 0 → chạy thẳng; có `sudo` → `sudo <lệnh>`; không có `sudo` → in lệnh.
5. **Kết quả**:
   - Đồng ý và cài thành công → đi tiếp; nếu vừa cài binding Nautilus thì cuối cùng in
     "Đóng hết cửa sổ Files rồi mở lại để thấy menu Morphix".
   - Từ chối / cài lỗi / không hỏi được: thiếu thư viện bắt buộc → in lệnh, `exit 1` (như hiện tại);
     chỉ thiếu binding Nautilus → in lệnh kèm "menu chuột phải sẽ nằm trong Scripts", **cài tiếp**.
6. **Chữ**: README dòng 21–22, `website/src/components/DownloadMatrix.tsx:142`,
   `CliDoctorTerminal.tsx:49`: "không cần sudo" → "chỉ hỏi mật khẩu khi máy thiếu thư viện".
   Header `install.sh` liệt kê thêm `MORPHIX_TTY`, `MORPHIX_NAUTILUS_DIRS`.

## Để lại cho người làm

- Test mới trong `test_install_sh.py` dùng stub `sudo`, `apt-get`, `nautilus` và file tty giả:
  đồng ý → gọi `sudo apt-get install -y python3-nautilus`; từ chối → không gọi sudo, vẫn cài xong;
  không tty → không gọi sudo; thiếu webkit + đồng ý → cài rồi đi tiếp; root → không có `sudo`.
- Chạy thật 1 lần trên Ubuntu 26.04 sạch (máy ảo) trước khi phát hành.
