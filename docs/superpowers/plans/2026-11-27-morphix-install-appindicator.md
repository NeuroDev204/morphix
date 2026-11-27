# install.sh AppIndicator: kế hoạch làm

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `install.sh` bật/cài extension AppIndicator trên GNOME và sửa tên gói Nautilus của openSUSE/Arch.
**Architecture:** Thêm vào bước 4 `scripts/install.sh`; stub `gnome-extensions`, `gsettings` trong `test_install_sh.py`.
**Tech Stack:** POSIX sh, Python unittest.
**Spec:** `docs/superpowers/specs/2026-11-27-morphix-install-appindicator-design.md`

## Global Constraints

- Tên gói/UUID đúng bảng "Đã kiểm trong container" của spec.
- `gnome-extensions`/`gsettings` chạy dưới user, không sudo; chỉ trong phiên GNOME.

## Review Focus

1. Ubuntu dùng `gnome-shell-ubuntu-extensions` + `ubuntu-appindicators@ubuntu.com`, không phải gói Debian. (Task 1)
2. `gsettings` khi danh sách rỗng (`@as []`) và khi đã có extension khác. (Task 1)
3. Không GNOME / chạy qua ssh: không gọi `gnome-extensions`. (Task 1)
4. openSUSE không có gói: không hỏi sudo chỉ vì extension. (Task 1)

---

### Task 1: AppIndicator + tên gói trong install.sh

- [ ] Test (RED): stub `gnome-extensions` (`list --enabled` → `$FAKE_EXT_ENABLED`, `list` → `$FAKE_EXT_ALL`, `enable` ghi log, `FAKE_EXT_ENABLE_FAIL` → exit 1), `gsettings` (get → `$FAKE_GSETTINGS` hoặc `@as []`, set ghi log). Test: Debian cài+bật qua gsettings nối danh sách; Ubuntu dùng gói/UUID Ubuntu; đã cài chưa bật → chỉ `enable`, không sudo; đã bật → không gì; KDE → không gọi; openSUSE → gợi ý extensions.gnome.org, không sudo; Arch thiếu binding → `nautilus-python`; openSUSE thiếu binding → `python3-nautilus`.
- [ ] Sửa `install.sh` theo spec 1–7. Chạy `python3 -m unittest discover -s scripts/tests` → PASS; `dash -n`.
- [ ] Commit `feat(install): install and enable the GNOME AppIndicator extension`.

### Task 2: Sửa tên gói ở README và gợi ý Rust

- [ ] README bảng distro: openSUSE `python3-nautilus`, Arch `nautilus-python` (bỏ "chưa kiểm"); `linux.rs:1132` gợi ý đúng tên. `cargo test -p morphix-cli` PASS.
- [ ] Commit `docs: correct nautilus binding package names for openSUSE and Arch`.
