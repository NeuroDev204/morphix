# Cài Morphix trên Ubuntu 24.04

Morphix hỗ trợ Ubuntu 24.04 amd64. Gói cần quyền quản trị khi cài; sau đó mở ứng dụng và chuyển đổi file bằng tài khoản thông thường.

## Cài đặt

Trong thư mục có file `.deb`, chạy:

```sh
sudo apt install ./morphix_<version>-<revision>_amd64.deb
```

APT sẽ cài các thư viện cần thiết, Nautilus Python, DING được hỗ trợ và Morphix. Package Morphix ghim đúng phiên bản DING có bản vá. Không cài bản DING khác thủ công trong khi chưa có bản Morphix tương thích.

Đăng xuất rồi đăng nhập lại để GNOME Shell và Files nạp các mục menu mới.

## Mở và chuyển đổi file

- Mở Morphix từ danh sách ứng dụng để chọn file hoặc kéo thả file vào cửa sổ.
- Trong Files hoặc trên Desktop, nhấn chuột phải vào một file rồi chọn **Morphix** để mở vòng chọn định dạng.
- Nhấn chuột phải vào vùng trống của Files hoặc Desktop rồi chọn **Morphix** để mở ứng dụng.
- Chọn **Mở thư mục** ở kết quả thành công để xem file đầu ra.
- Chạy `morphix doctor` để xem trạng thái tích hợp Files/Desktop và các công cụ cần thiết.

Menu gốc của Files và Desktop vẫn giữ nguyên. Thư mục, vị trí mạng và lựa chọn không hỗ trợ sẽ mở thông báo phù hợp trong ứng dụng.

## Cập nhật và gỡ

Cập nhật bằng APT như các gói Ubuntu khác. Nếu phiên bản DING mới cần bản vá khác, APT chỉ có thể cập nhật DING cùng một gói Morphix tương thích.

Gỡ bằng:

```sh
sudo apt remove morphix
```

Khi gỡ, Morphix yêu cầu `dpkg-divert` phục hồi hai file DING gốc đã lưu. Nếu các file hệ thống đó bị sửa hoặc bị chuyển hướng bởi package khác, thao tác dừng để tránh ghi đè; xem thông báo từ APT/dpkg trước khi thử lại. Dữ liệu người dùng, lịch sử và cài đặt Morphix không bị gỡ.
