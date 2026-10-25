# Nghiệm thu bản cài bằng lệnh curl — 2026-10-24

Tarball dựng trong container `ubuntu:22.04` bằng `scripts/build-tarball.sh` (glibc floor 2.34, ffmpeg tĩnh BtbN n8.1.3).
Chưa phát hành: chạy `install.sh` qua `curl file:///release/install.sh | MORPHIX_RELEASE_BASE=file:///release MORPHIX_SKIP_INTEGRATION=1 sh`
bằng user thường, sau khi cài thư viện bằng đúng lệnh `install.sh` in ra. Script: `morphix-acceptance/run.sh` (ngoài repo).

## Container (không giao diện)

| Image | Cài | JPG→PNG | AVIF→PNG | WebM→MP4 | FLAC→MP3 | PDF→TXT | `morphix uninstall` |
|---|---|---|---|---|---|---|---|
| ubuntu:22.04 | ok | ok | ok | ok | ok | ok | 0 file còn lại, giữ dữ liệu |
| ubuntu:24.04 | ok | ok | ok | ok | ok | ok | như trên |
| debian:12 | ok | ok | ok | ok | ok | ok | như trên |
| fedora:latest | ok | ok | ok | ok | ok | ok | như trên |
| archlinux:latest | ok | ok | ok | ok | ok | ok | như trên |
| opensuse/tumbleweed | ok (sau khi bỏ awk) | ok | ok | ok | ok | ok | như trên |

- fedora:latest **chưa** cài webkit: dừng, in `sudo dnf install webkit2gtk4.1 libayatana-appindicator-gtk3`, không ghi gì.
- Lần đầu openSUSE Tumbleweed thoát 127 không thông báo vì ảnh tối giản không có `awk`; đã bỏ `awk` khỏi `install.sh`
  (test `test_needs_only_basic_tools`).

## Desktop thật (máy ảo QEMU/KVM, cài bằng đúng lệnh curl, log trong `serve/results/`)

| Máy ảo | Phiên | Matrix | Công cụ | Menu & chuyển đổi | Ghi chú |
|---|---|---|---|---|---|
| Fedora 44 Workstation (UEFI, ổ ảo 30 GB) | Wayland, GNOME | 118/118 | 9/9 | Scripts → Morphix trong Files; `anh.avif` → `anh (1).png` | Cần `nautilus-python` (dnf). Tray thiếu extension AppIndicator. |
| Kubuntu 24.04.5 live | X11, KDE Plasma | 118/118 | 9/9 | Dolphin: chuột phải → Morphix; `anh.jpg` → `anh (1).webp` | Live thiếu `curl`; phải `sudo apt update && sudo apt install curl`. Log cũ ghi `desktop=` rỗng, log này ghi `desktop=KDE`. |
| Debian 13 (trixie) GNOME live | Wayland, GNOME | 118/118 | 9/9 | Scripts → Morphix; `anh.avif` → GIF | Cần `python3-nautilus`; sau khi cài, Scripts hoạt động. |
| Arch Linux (archiso + XFCE/LightDM) | X11, XFCE | 118/118 | 9/9 | Thunar: chuột phải → Morphix; `anh.avif` → `anh (1).webp` | Live chỉ có ~32 MB trống ở `/`: phải `mount -o remount,size=3G /run/archiso/cowspace` trước khi `pacman`. |
| openSUSE Tumbleweed GNOME live | Wayland, GNOME | 118/118 | 9/9 | Scripts → Morphix; `anh.avif` → TIFF | Cần `nautilus-python`. zypper hỏi xác nhận, cần `-n` khi chạy không tương tác. |
| Xubuntu 24.04.5 | X11, XFCE | 118/118 | 9/9 | (log) | |
| Ubuntu MATE 24.04.5 | X11, MATE | 118/118 | 9/9 | (log) | |
| Linux Mint 22.3 | X11, Cinnamon | 118/118 | 9/9 | (log) | |

Lưu ý phần chưa kiểm:
- Dolphin/Nemo/Caja/Thunar: Thunar kiểm trên Arch; Dolphin trên Kubuntu; Nemo/Caja chưa có máy thật.
- Alt+C: chưa kiểm trên GNOME X11 trong đợt này (Fedora, Debian, openSUSE là Wayland; các máy còn lại không phải GNOME). Kết quả kiểm trên Wayland và non-GNOME: không hiện Alt+C, đúng thiết kế.
- Tray (AppIndicator) trên GNOME: chỉ kiểm được báo cáo `missing` kèm hướng dẫn; chưa thử bật extension trên máy thật.
- ISO Linux Mint 22.3 tải về có checksum không khớp lúc tải lần đầu; kết quả Mint ở trên chạy trên ISO đó, chưa tải lại.
