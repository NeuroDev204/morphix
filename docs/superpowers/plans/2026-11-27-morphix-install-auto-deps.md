# install.sh tự cài gói thiếu: kế hoạch làm

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `install.sh` phát hiện thư viện bắt buộc và binding Nautilus còn thiếu, hỏi `[Y/n]` qua tty, rồi cài bằng sudo.

**Architecture:** Thay bước 4 của `scripts/install.sh`; test bằng stub trong `scripts/tests/test_install_sh.py`, chạy script qua stdin như `curl | sh`.

**Tech Stack:** POSIX sh, Python unittest.

**Spec:** `docs/superpowers/specs/2026-11-27-morphix-install-auto-deps-design.md`

## Global Constraints

- Chỉ dùng lệnh đã có trong `test_needs_only_basic_tools`; `id`/`sudo` chỉ gọi khi thực sự cài.
- Lệnh cài chạy với `</dev/null`: khi `curl | sh`, stdin là phần còn lại của script.
- Lệnh in cho người dùng không có cờ `-y` (`sudo apt install …`); lệnh chạy có cờ không hỏi.
- Biến test mới: `MORPHIX_TTY` (mặc định `/dev/tty`), `MORPHIX_NAUTILUS_DIRS` (mặc định 4 thư mục của Rust).

## Review Focus

1. `curl | sh`: lệnh cài con không được ăn stdin (phần script còn lại). (Task 1: stub sudo ghi số byte stdin = 0)
2. Không có tty (CI, cron): không bao giờ gọi sudo. (Task 1)
3. Máy không có Nautilus (KDE, XFCE): không hỏi gì thêm. (Task 1)
4. Bản Linux lạ: giữ thông báo cũ, không cài. (Task 1, test cũ)
5. Người dùng từ chối khi chỉ thiếu binding: cài vẫn xong. (Task 1)

---

### Task 1: Tự cài gói thiếu trong install.sh

**Files:** Modify `scripts/install.sh` (header, bước 4, cuối script); Test `scripts/tests/test_install_sh.py`.

- [ ] **Step 1: Test** — stub mới `sudo` (ghi `sudo $* stdin=<bytes>`, `FAKE_SUDO_FAIL` → exit 1), `apt-get` (ghi `apt-get $*`), `id` (`${FAKE_UID:-1000}`), `nautilus` (tạo khi test cần). `run_install` mặc định đặt `MORPHIX_TTY` tới file không tồn tại và `MORPHIX_NAUTILUS_DIRS` tới thư mục có `libnautilus-python.so`; tham số `piped=True` chạy `sh` với script làm stdin. Test:
  - `test_missing_nautilus_binding_installed_after_yes`: tty "\n" → log có `sudo apt-get install -y python3-nautilus stdin=0`; rc 0; stdout có "Đóng hết cửa sổ Files".
  - `test_declined_binding_still_installs`: tty "n\n" → không có `sudo` trong log; rc 0; output có `sudo apt install python3-nautilus` và "Scripts".
  - `test_no_tty_never_runs_sudo`: thiếu webkit, không tty → không `sudo`; rc ≠ 0; có `sudo apt install libwebkit2gtk-4.1-0`.
  - `test_missing_webkit_installed_after_yes`: thiếu webkit, tty "\n" → `sudo apt-get install -y libwebkit2gtk-4.1-0`; rc 0; đã cài.
  - `test_failed_required_install_stops`: thiếu webkit, tty "\n", `FAKE_SUDO_FAIL` → rc ≠ 0; `prefix` chưa tồn tại.
  - `test_root_installs_without_sudo`: `FAKE_UID=0`, thiếu binding → log có `apt-get install -y python3-nautilus`, không `sudo`.
  - `test_no_nautilus_no_prompt`: PATH chỉ bộ lệnh cơ bản (không `nautilus`), thư mục binding rỗng, tty "\n" → không `sudo`, không chữ "cần cài thêm".
- [ ] **Step 2: Chạy** `python3 -m unittest scripts/tests/test_install_sh.py` → 7 test mới FAIL, test cũ PASS.
- [ ] **Step 3: Sửa `install.sh`** theo spec mục Thiết kế 1–5.
- [ ] **Step 4: Chạy lại** → tất cả PASS; `sh -n scripts/install.sh` OK; `shellcheck` nếu có.
- [ ] **Step 5: Commit** `feat(install): offer to install missing system packages with sudo`.

### Task 2: Sửa chữ "không cần sudo"

**Files:** `README.md:21-22`, `website/src/components/DownloadMatrix.tsx:142`, `website/src/components/CliDoctorTerminal.tsx:49`.

- [ ] **Step 1:** `rg -n "không cần sudo|Không cần sudo" README.md website/src` → có kết quả.
- [ ] **Step 2:** Đổi thành "chỉ hỏi mật khẩu khi máy thiếu thư viện" (giữ giọng từng chỗ); README dòng 22 mô tả việc hỏi rồi tự cài.
- [ ] **Step 3:** Lệnh bước 1 → không kết quả; `cd website && npm run build` qua.
- [ ] **Step 4: Commit** `docs: say the installer asks for sudo only when packages are missing`.
