# Menu Morphix trên desktop GNOME qua lệnh curl, bỏ gói .deb: thiết kế

Ngày: 2026-11-27. Đợt 1 của 2 (đợt 2: desktop Cinnamon, MATE; kiểm lại KDE, Xfce).

## Ý định

- Người dùng Ubuntu 26.04 cài `.deb` bằng App Center không được: gói ghim `gnome-shell-extension-desktop-icons-ng (= 46+really47.0.9-1ubuntu5)`,
  26.04 không có gói đó (DING nằm trong `gnome-shell-ubuntu-extensions` 50.26.04.8ubuntu). Bản 24.04 cũng hỏng ngay khi Ubuntu cập nhật DING.
- Chủ dự án quyết định: **bỏ hẳn `.deb`, Linux chỉ cài bằng lệnh curl**, website không có dòng chọn distro.
- Nhưng **phải giữ menu chuột phải trên desktop**, và menu phải **một cấp giống trong Files**
  (chuột phải file → Morphix → các việc hợp với file), **không chỉ Ubuntu mà mọi distro** có desktop icon GNOME (DING).
- Không chấp nhận menu Scripts (2-3 cấp, liệt kê cả việc không hợp).

## Đã kiểm trên máy thật (Ubuntu 26.04 GNOME Wayland, 2026-11-27)

1. Chép DING vào `~/.local/share/gnome-shell/extensions/ding@rastersoft.com` (cùng UUID): **GNOME từ chối**, log:
   `Found user extension ding@rastersoft.com, but not loading ... as part of session mode` (DING nằm trong `/usr/share/gnome-shell/modes/ubuntu.json`).
2. Chép thành UUID khác `morphix-desktop@morphix`, thêm vào `enabled-extensions`, đưa `ding@rastersoft.com` vào `disabled-extensions`,
   đăng nhập lại: **chạy được**, `ding.js` chạy từ thư mục bản chép, mục menu thêm vào hiện trên desktop.
3. Code DING không ghi cứng UUID (chỉ có tên D-Bus `com.rastersoft.dingextension`, không trùng vì DING gốc đã tắt; schema GSettings dùng chung).
4. Bản vá cũ `integration/ding/morphix-menu.patch` áp lên DING 26.04: 2/3 hunk khớp, 1 hunk lỗi → không dùng diff, dùng chèn theo mốc.
5. Bật/tắt extension lúc đang chạy (`gnome-extensions enable/disable`) có hiệu lực ngay, không cần đăng nhập lại; extension **mới chép** thì GNOME chỉ thấy sau khi đăng nhập lại.

## Cách làm (đã chọn)

| Cách | Đánh giá |
|---|---|
| **A. Bản chép DING với UUID riêng trong thư mục người dùng (chọn)** | Không sudo, mọi distro có DING, đã thử chạy. Rủi ro: bản chép không tự cập nhật → cần tự kiểm khi đăng nhập. |
| B. Vá DING hệ thống bằng sudo (như .deb cũ) | apt cập nhật DING là mất bản vá; chỉ Debian/Ubuntu. Loại. |
| C. Menu Scripts có sẵn của DING | Không đạt "một cấp giống Files". Chủ dự án từ chối. |
| D. Chép DING cùng UUID | Ubuntu chặn (mục 1). Loại. |

## Mục tiêu

1. Cài bằng curl trên GNOME có DING đang bật → sau một lần đăng nhập lại, chuột phải file trên desktop có **Morphix → [việc]**,
   cùng danh sách, cùng hành vi như menu Nautilus (`integration/nautilus/morphix_menu.py`):
   - lấy danh sách: `morphix --json menu --list --uris <uri...>` → `{"items":[{"id","label","is_tool"}],"too_many":bool}`;
   - bấm việc: `morphix menu --action <id> --uris <uri...>`;
   - lựa chọn có thư mục / file không cục bộ / không phải file thường → mục "Morphix" mở `morphix tray --unsupported-selection` (giống Nautilus);
   - JSON hỏng, lệnh lỗi hoặc quá thời gian → mục "Morphix" mở `morphix menu --uris ...` (giống Nautilus);
   - `too_many` → submenu có một mục mờ (nhãn giống Nautilus); danh sách rỗng → không thêm mục nào;
   - chuột phải vùng trống desktop → mục "Morphix" mở `morphix tray`.
   Đường dẫn binary là đường dẫn `morphix` của bản cài (như cách thay `EXECUTABLE` trong menu Nautilus), không phải `/usr/bin/morphix`.
2. **Icon desktop không bao giờ mất quá một lần đăng nhập**: bản chép lỗi thì tự quay về DING gốc.
3. Bỏ `.deb`: CI không build, Release không có, website/README/doc không nhắc. Linux = curl.
4. Phát hành v0.1.7.

## Không làm (đợt này)

Desktop Cinnamon/MATE/KDE/Xfce (đợt 2); sửa DING hệ thống; gói .rpm/AUR; tự đăng xuất.

## Không được xảy ra (anti-goals)

- Mất icon desktop vĩnh viễn, hoặc người dùng phải tự sửa gsettings mới có lại icon.
- Bật desktop icon cho người **đã tự tắt DING** (máy chủ dự án là ví dụ: `ding@rastersoft.com` nằm trong `disabled-extensions`).
- Gỡ Morphix để lại DING gốc bị tắt.
- Đụng file hệ thống hoặc cần sudo cho phần này.

## Thiết kế

### Thành phần tích hợp mới: desktop GNOME (Rust, `morphix-cli` integration)

Một `ComponentId` mới (tên do người dựng chọn, ví dụ `GnomeDesktop`), đi qua `install-integration` / `uninstall` / `doctor` như các mục khác.

**Nguồn DING:** thư mục `ding@rastersoft.com` đầu tiên tìm thấy theo thứ tự `$XDG_DATA_HOME/gnome-shell/extensions`, rồi `$XDG_DATA_DIRS/gnome-shell/extensions`
(mặc định `/usr/local/share`, `/usr/share`). Không thấy → mục "không áp dụng".

**Điều kiện áp dụng:** phiên GNOME (`XDG_CURRENT_DESKTOP` chứa GNOME), có `gnome-extensions`, và
DING đang bật **hoặc** bản chép Morphix đang bật (lần cài lại). DING không bật và bản chép không bật → "không áp dụng: desktop icon đang tắt".

**Dựng bản chép** vào `$XDG_DATA_HOME/gnome-shell/extensions/morphix-desktop@morphix`:
1. Chép toàn bộ thư mục nguồn vào thư mục tạm cạnh đích.
2. `metadata.json`: đổi `uuid` → `morphix-desktop@morphix`, `name` → "Desktop Icons (Morphix)"; giữ `shell-version` của nguồn.
3. Chèn code Morphix vào `app/fileItemMenu.js` và `app/desktopManager.js` theo **mốc chuỗi**, mỗi mốc phải xuất hiện **đúng một lần**:
   - file menu: ngay sau `this._menu = DesktopIconsUtil.createDesktopMenu(['fileitemmenu']);` (đã thử trên 26.04);
   - menu vùng trống: mốc trong `desktopManager.js` (bản vá cũ dùng mục "Select All"); người dựng chọn mốc cụ thể, kiểm trên cả DING 24.04 (47.0.9) và 26.04.
   - Lấy lựa chọn hiện tại: `this._desktopManager.getCurrentSelection(false)` (bản vá cũ), lùi về `[fileItem]`.
   - Gọi `morphix --json menu --list` **đồng bộ** khi mở menu, có giới hạn thời gian như Nautilus (`LIST_TIMEOUT_SECONDS`); mở việc bằng spawn không chờ.
4. Ghi file đánh dấu trong bản chép (ví dụ `morphix-source.json`): đường dẫn nguồn, `version`/hash `metadata.json` nguồn, hash 2 file JS nguồn, phiên bản Morphix.
5. Thiếu mốc / mốc trùng / lỗi ghi → xoá thư mục tạm, **không đụng bản chép cũ và gsettings**, báo lỗi rõ (`DING bản X chưa hỗ trợ`).
6. Thành công → thay thư mục đích bằng thư mục tạm (đổi tên, không để nửa vời).

**Bật bản chép: đổi ở lần đăng nhập sau, không đổi lúc cài.** Ghi `enabled-extensions`/`disabled-extensions` có hiệu lực ngay trong phiên
(GNOME nghe gsettings), mà GNOME chỉ thấy thư mục extension mới sau khi đăng nhập lại. Nên:
- Lúc cài: chỉ dựng bản chép + cài autostart kiểm; **không** đụng gsettings. Báo "Đăng xuất rồi đăng nhập lại để thấy menu Morphix trên desktop".
- Lần đăng nhập sau, bước kiểm thấy bản chép đã được GNOME biết (`gnome-extensions info morphix-desktop@morphix` thành công) mà chưa bật
  → ghi lại trạng thái DING gốc (vào thư mục dữ liệu Morphix, để gỡ trả đúng như cũ), `gnome-extensions disable ding@rastersoft.com`
  rồi `gnome-extensions enable morphix-desktop@morphix` (cả hai có hiệu lực ngay; icon chớp vài giây).
  Không bao giờ để cả hai cùng bật (hai tiến trình vẽ desktop, trùng tên D-Bus).
- Lần đổi chỉ làm một lần; người dùng tự tắt bản chép sau đó thì tôn trọng (xem bước 1 dưới).

**Tự kiểm khi đăng nhập** (autostart XDG riêng, chạy `morphix` với một lệnh con mới, ví dụ `morphix desktop-check`; không phụ thuộc GUI đang bật):
0. Bản chép chưa từng được đổi sang (lần đầu sau khi cài) → làm bước đổi ở trên.
1. Bản chép đã đổi sang mà nay không còn được bật (người dùng tắt) → không làm gì.
2. Nguồn DING đổi so với file đánh dấu (cập nhật Ubuntu) → dựng lại bản chép (các bước trên). Có hiệu lực từ đăng nhập sau.
3. Dựng lại thất bại, hoặc `gnome-extensions info morphix-desktop@morphix` báo trạng thái không chạy (ERROR / OUT OF DATE) sau vài giây chờ
   → tắt bản chép, bật lại `ding@rastersoft.com` **ngay trong phiên** (`gnome-extensions enable` có hiệu lực ngay), ghi lý do vào log Morphix,
   gửi một thông báo hệ thống. Icon desktop trở lại sau vài giây.
4. Nguồn DING biến mất → tắt bản chép (không còn gì để chép theo), không bật gì.

**Gỡ** (`morphix uninstall`, `uninstall-integration`): xoá bản chép và autostart kiểm; bỏ `morphix-desktop@morphix` khỏi `enabled-extensions`;
trả `ding@rastersoft.com` về trạng thái đã ghi (bật lại nếu trước đó bật); bật ngay trong phiên nếu được.

**doctor**: thay phần kiểm `.deb`/dpkg/divert trong `desktop.rs` bằng: nguồn DING tìm thấy ở đâu, bản chép có/khớp nguồn, đang bật, trạng thái GNOME báo.

### Bỏ .deb

- `.github/workflows/release.yml`: xoá job `deb`, bỏ `deb` khỏi `needs` của `publish`, bỏ bước tải artifact `deb` và `test -f dist/morphix_*_amd64.deb.sha256`.
- Xoá file chỉ phục vụ .deb: `scripts/build-deb.sh`, `scripts/check-deb.sh`, `scripts/prepare-ding.py`, `scripts/render-dpkg-scripts.py`,
  `packaging/deb/`, `crates/morphix-gui/packaging/deb/` (control.in, copyright… kiểm từng file còn ai dùng), `integration/ding/morphix-menu.patch`, `integration/ding/source.json`,
  test của chúng. **Giữ** `integration/ding/LICENSE` nếu bản chép vẫn cần ghi giấy phép DING (GPL-3).
- Code Rust nhánh "bản .deb" (`/usr/bin/morphix`, `is_user_install`, chú thích `linux.rs:31`) — người dựng kiểm còn cần không; `install.sh` **giữ** đoạn chặn khi máy còn gói `.deb` cũ (`sudo apt remove morphix`).
- `docs/ubuntu-install.md`: xoá; README/README.txt: bỏ dòng .deb, thêm "đã cài .deb trước đây: `sudo apt remove morphix` rồi chạy lệnh curl", thêm câu menu desktop GNOME cần đăng nhập lại.

### Website

`DownloadMatrix.tsx`: cột Linux chỉ còn lệnh curl (nút Copy) và dòng "tự cài thư viện còn thiếu, hỏi trước khi dùng sudo".
Bỏ "Cách 2" (nút .deb và tarball) — **quyết định của người viết**: chủ dự án nói "hoàn toàn chạy lệnh curl"; tarball vẫn có trên Release vì `install.sh` cần.
`constants/downloads.ts`: bỏ các trường deb/tarball không còn dùng. Bản 0.1.7 cập nhật như các lần trước sau khi Release xong.

## Ràng buộc

- POSIX sh cho `install.sh`; Rust cho phần tích hợp (theo khuôn `linux.rs`: component, report, `FakeBackend` trong test).
- Không sudo, không ghi ngoài `$HOME`.
- Chữ hiển thị tiếng Việt như các mục khác.
- Test chạy được trong CI không có GNOME: gsettings/gnome-extensions qua lớp `tools` giả như code hiện tại.

## Quyết định của người viết (sau vòng đọc độc lập)

1. **Lùi về DING do lỗi ≠ người dùng tự tắt.** File trạng thái của Morphix ghi lý do lùi (`fallback`, kèm hash nguồn DING và phiên bản Morphix lúc lùi).
   Bước kiểm khi đăng nhập **thử lại** (dựng + đổi) chỉ khi nguồn DING **hoặc** phiên bản Morphix khác lúc lùi; không đổi gì thì không thử lại (tránh vòng lặp dựng-đổi-lỗi mỗi lần đăng nhập).
   Chạy lại lệnh curl / `install-integration` luôn được thử lại.
2. **Người dùng bật lại DING gốc (vd. Ubuntu Settings → Desktop icons) khi bản chép đang chạy:** bản chép tự nhường ngay —
   `extension.js` của bản chép theo dõi `org.gnome.shell enabled-extensions`; thấy `ding@rastersoft.com` được bật thì tự tắt mình
   (ghi `morphix-desktop@morphix` vào `disabled-extensions`, bỏ khỏi `enabled-extensions`). Morphix coi đó là **người dùng chọn DING gốc**:
   ghi trạng thái `user-opt-out`, không tự đổi lại; chỉ chạy lại `install-integration` mới đổi lại. Muốn ẩn icon khi đang dùng bản chép:
   tắt "Desktop Icons (Morphix)" trong ứng dụng Extensions (README ghi một dòng).
3. **"Bản chép đang bật" chỉ tính là cài lại khi có file trạng thái của Morphix.** Không có file trạng thái → chỉ xét DING gốc có bật không.
   Máy chủ dự án đang ở trạng thái thử tay (bản chép bật, DING tắt, không có file trạng thái): trước khi thử bản thật, xoá bản thử và
   trả `ding@rastersoft.com` về `disabled-extensions`, bỏ `morphix-desktop@morphix` khỏi `enabled-extensions` (trạng thái gốc của chủ máy), hỏi chủ máy có muốn bật DING để thử không.
4. **Phiên bản Morphix đổi cũng dựng lại bản chép** ở bước kiểm khi đăng nhập (JS chèn và đường dẫn binary đi theo phiên bản).
5. **Kiểm bản chép còn sống** bằng cả `gnome-extensions info` (trạng thái) và tên D-Bus `com.rastersoft.dingextension` có chủ
   (tiến trình `ding.js` chết thì extension vẫn báo ACTIVE nhưng desktop trống).

## Để người dựng quyết

Tên ComponentId/lệnh con/file đánh dấu; cách viết đoạn JS chèn (giữ ngắn, ưu tiên giống `morphix_menu.py`); thời gian chờ ở bước 3 tự kiểm;
chọn mốc trong `desktopManager.js`.

## Chưa kiểm

DING trên Fedora/Arch/openSUSE (cài từ extensions.gnome.org vào thư mục người dùng: nguồn ở `$XDG_DATA_HOME`, không bị session mode chặn);
Ubuntu 24.04 trên máy thật; X11.
