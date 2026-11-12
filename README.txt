Morphix

Đổi định dạng, nén, đổi cỡ và xử lý file ngay trên máy của bạn (Linux và Windows), không gửi gì ra ngoài. Bấm chuột phải lên file, chọn Morphix rồi chọn việc cần làm; file kết quả nằm cạnh file gốc và file gốc không bao giờ bị ghi đè. Khi việc xong, Morphix gửi một thông báo hệ thống.

Hỗ trợ: ảnh (JPG, PNG, WebP, AVIF, GIF, TIFF, BMP, sang PDF), âm thanh và video (MP3, M4A, WAV, FLAC, OGG, OPUS, MP4, MKV, WebM, MOV, AVI, GIF), văn bản và phụ đề (TXT, MD, SRT, VTT, CSV, TSV, HTML, DOCX), PDF sang TXT hoặc MD, nén/giải nén ZIP, TAR.GZ, 7Z.

Cài đặt trên Windows

- Microsoft Store: tìm "Morphix" trên Microsoft Store. Bản này đặt mục Morphix ngay trong menu chuột phải chính của Windows 11.
- Bản exe (MorphixSetup.exe): tải từ trang tải về của Morphix, kèm file MorphixSetup.exe.sha256 để đối chiếu (certutil -hashfile MorphixSetup.exe SHA256). Cài cho người dùng hiện tại, không cần quyền quản trị, và tự cài WebView2 Runtime nếu máy chưa có.
- Cảnh báo SmartScreen: bản exe chưa được ký số nên lần chạy đầu Windows hiện "Windows protected your PC". Bấm More info rồi Run anyway. Đối chiếu SHA256 ở trên nếu bạn muốn chắc file không bị thay.
- Vị trí menu: trên Windows 10 mục Morphix nằm ngay trong menu chuột phải. Trên Windows 11, bản exe nằm trong Show more options; bản Store nằm ở menu chính.
- Gỡ: Settings → Apps, hoặc morphix uninstall-integration rồi xoá thư mục cài.

Cài đặt trên Linux

curl -fsSL https://github.com/NeuroDev204/morphix/releases/latest/download/install.sh | sh

- Cài cho người dùng hiện tại vào ~/.local, không cần sudo. Chạy lại đúng lệnh trên để cập nhật.
- Cần Linux x86_64, glibc 2.35 trở lên (Ubuntu 22.04+, Debian 12+, Fedora 36+, Arch, openSUSE Tumbleweed, Mint 21+…) và hai thư viện hệ thống webkit2gtk-4.1, libayatana-appindicator3. Thiếu thì lệnh cài in đúng lệnh sudo cho bản Linux của bạn để bạn tự chạy.
- Menu chuột phải được thêm cho trình quản lý file có trên máy: Nautilus (Files), Dolphin, Nemo, Thunar, Caja. Mỗi việc là một mục trong submenu Morphix, bấm là chạy ngay, không mở cửa sổ nào:
  - Nautilus có nautilus-python: submenu Morphix liệt kê đúng các việc hợp với các file đang chọn (đọc theo nội dung file), giống Windows.
  - Dolphin, Thunar: submenu Morphix lọc theo loại file (ảnh, âm thanh, video, văn bản, PDF, file nén).
  - Nemo, Caja, và Nautilus thiếu nautilus-python: chuột phải → Scripts → Morphix việc nhanh → nhóm loại file → việc.
  - Ở hai kiểu sau menu liệt kê mọi việc của nhóm nên có thể có việc không hợp với file cụ thể (ví dụ PNG sang PNG); bấm vào sẽ hiện thông báo lỗi rõ ràng, không làm gì với file. morphix doctor cho biết máy bạn đang dùng menu động hay menu tĩnh.
- Lệnh trên cần curl (Kubuntu 24.04 live không có sẵn; cài bằng sudo apt install curl).
- Gói Python của Nautilus (nautilus-python trên Fedora, Arch, openSUSE; python3-nautilus trên Debian, Ubuntu) cho menu động trong Files. Thiếu nó thì Files dùng menu Scripts như trên. morphix doctor sẽ in đúng lệnh nếu thiếu.
- Trên GNOME không có icon khay (AppIndicator), cài extension gnome-shell-extension-appindicator rồi bật bằng gnome-extensions enable appindicatorsupport@rgcjonas.gmail.com, hoặc dùng morphix tray.
- Phím Alt+C (mở Morphix với các file đang chọn) chỉ có trên GNOME chạy X11. Trên Wayland và các desktop khác, dùng menu chuột phải.
- Ubuntu 24.04 còn có gói .deb trong mục Releases, kèm menu chuột phải ngay trên desktop.

Theo từng bản Linux

Lệnh cài giống nhau cho mọi bản. Khác nhau là thư viện hệ thống và gói để Files (Nautilus) hiện menu động. Các lệnh dưới đây là lệnh đã dùng khi kiểm thử; nếu install.sh in ra lệnh khác cho máy bạn, hãy chạy lệnh đó.

Chưa kiểm lại sau khi đổi menu. Cột "Đã kiểm trên" ghi các máy đã kiểm với bản cũ (menu một mục mở vòng tròn). Menu submenu mới mới được kiểm bằng test tự động và với thư viện Nautilus thật; chưa kiểm trên Dolphin, Thunar, Nemo, Caja hay từng bản Linux dưới đây.

Bản Linux	Thư viện còn thiếu	Gói Nautilus cho menu Files	Đã kiểm trên
Fedora 44	sudo dnf install webkit2gtk4.1 libayatana-appindicator-gtk3	sudo dnf install nautilus-python	GNOME, Wayland
openSUSE Tumbleweed	sudo zypper install libwebkit2gtk-4_1-0 libayatana-appindicator3-1	sudo zypper install nautilus-python	GNOME, Wayland
Debian 13	sudo apt install libwebkit2gtk-4.1-0 libayatana-appindicator3-1	sudo apt install python3-nautilus	GNOME, Wayland
Arch Linux	sudo pacman -S webkit2gtk-4.1 libayatana-appindicator	sudo pacman -S nautilus-python (chưa kiểm tên gói này)	XFCE, X11
Ubuntu 24.04 và các biến thể (Kubuntu, Xubuntu, Ubuntu MATE, Linux Mint 22)	sudo apt install libwebkit2gtk-4.1-0 libayatana-appindicator3-1	sudo apt install python3-nautilus (chưa kiểm trên Ubuntu GNOME)	Kubuntu KDE, Xubuntu XFCE, Ubuntu MATE, Mint Cinnamon, X11

- Lệnh curl cần curl. Kubuntu 24.04 live không có sẵn: sudo apt update && sudo apt install curl.
- Menu theo trình quản lý file: GNOME Files → Morphix (có nautilus-python) hoặc Scripts → Morphix việc nhanh (không có); KDE Dolphin → Morphix; Xfce Thunar → Morphix; Cinnamon (Nemo) và MATE (Caja) → Scripts → Morphix việc nhanh. Nemo và Caja chưa từng kiểm được vì máy thử không có các trình quản lý này.
- GNOME Wayland: không có phím Alt+C; dùng chuột phải.
- GNOME X11: Alt+C mở Morphix với các file đang chọn (chưa kiểm trên máy thật).
- Tray (icon khay) trên GNOME: GNOME Fedora, Debian và openSUSE không có sẵn AppIndicator. Cài gnome-shell-extension-appindicator, chạy gnome-extensions enable appindicatorsupport@rgcjonas.gmail.com, rồi đăng nhập lại. Không muốn cài thì dùng morphix tray.
- Kiểm tra lại: morphix doctor liệt kê từng tích hợp và báo đúng lệnh còn thiếu.

Cài một phiên bản cụ thể: curl -fsSL …/install.sh | MORPHIX_VERSION=v0.1.4 sh.

Gỡ cài đặt

morphix uninstall

Gỡ chương trình và các mục menu; dữ liệu của bạn (~/.local/share/morphix: lịch sử, cài đặt, log) được giữ lại. Bản cài bằng .deb: sudo apt remove morphix.

Dòng lệnh

morphix --help liệt kê mọi lệnh, ví dụ morphix convert --to webp ảnh.png, morphix doctor để kiểm tra cài đặt.

Giấy phép

Morphix phát hành theo GPL-3.0-or-later. ffmpeg và PDFium đi kèm có giấy phép riêng, xem packaging/LICENSES-THIRD-PARTY.md.
