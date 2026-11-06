# Morphix: convert ảnh đúng bản chất định dạng — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Convert ảnh chọn kiểu mã hoá theo định dạng nguồn (nguồn lossless → WebP lossless), và cảnh báo khi file WebP/AVIF/JPEG ra không nhỏ hơn file gốc.

**Architecture:** Thêm `encode_converted` (chỉ dùng cho `convert_image`) bên cạnh `encode_image` giữ nguyên để `compress_image`/`resize_image` không đổi hành vi. `run_job` đo kích thước file vào/ra cho `ConvertImage` và đưa cảnh báo vào trường mới `ItemReport.warnings`; CLI và GUI hiển thị.

**Tech Stack:** Rust 2021, crate `image` 0.25, `webp` 0.3.1, `assert_cmd`, `tempfile`.

**Spec:** `docs/superpowers/specs/2026-11-05-morphix-image-format-nature-design.md`

**Đã xác nhận trước khi lập plan:** `webp` 0.3.1 có `Encoder::encode_lossless(&self) -> WebPMemory` (đọc từ `~/.cargo/registry/.../webp-0.3.1/src/encoder.rs`). Không cần thêm dependency.

## Global Constraints

- Không bao giờ ghi đè hay làm mất file nguồn; đầu ra nằm cạnh file gốc với tên không va chạm (đã có sẵn qua `desired_output`/`write_output_like`).
- Lỗi và cảnh báo phải hiện cho người dùng, không nuốt (kể cả CLI và job nền).
- Chỉ offline. Comment trong code là ghi chú `WHY` tiếng Việt, khớp code xung quanh.
- Chuỗi hiển thị bằng tiếng Việt.
- `compress_image` và `resize_image` giữ nguyên hành vi (`compress_image` đã tự từ chối kết quả không nhỏ hơn bằng `MorphixError::NotSmaller`).
- Commit theo Conventional Commits có scope crate, ví dụ `feat(morphix-core): ...`. Không sửa tay `Cargo.lock`.
- `cargo fmt`: chỉ format các file đã động tới; không format cả workspace.

## Review Focus

- Nguồn PNG có alpha → WebP lossless: alpha phải giữ nguyên (điểm ảnh trong suốt hoàn toàn có thể đổi RGB, không phải lỗi) — Task 1.
- Nguồn lossy (JPEG) → WebP không được rơi vào nhánh lossless (file sẽ to bất thường) — Task 1.
- Nguồn lossless → JPEG dùng quality 95 nhưng nguồn lossy → JPEG vẫn theo preset người dùng chọn — Task 1.
- File ra bằng đúng file gốc hoặc file gốc 0 byte: không chia cho 0, thông điệp vẫn đúng — Task 2.
- Đích PNG/TIFF/BMP/GIF to hơn gốc là bình thường: không cảnh báo; cảnh báo không làm job thành lỗi hay đổi mã thoát CLI — Task 2 và 3.

---

### Task 1: Chọn kiểu mã hoá theo nguồn

**Files:**
- Modify: `crates/morphix-core/src/kind.rs` (thêm vào `impl ImgFormat`)
- Modify: `crates/morphix-core/src/ops/compress.rs:32` (`image_format_of` thành `pub(crate)`)
- Modify: `crates/morphix-core/src/ops/image_io.rs` (thêm `encode_converted`, đổi `convert_image`)
- Modify: `crates/morphix-core/tests/common/mod.rs` (thêm helper ảnh PNG có hoạ tiết)
- Test: `crates/morphix-core/tests/convert.rs`, và `#[cfg(test)] mod tests` trong `image_io.rs`

**Interfaces:**
- Produces: `ImgFormat::is_lossless_source(self) -> bool` (đúng cho `Png`, `Bmp`, `Tiff`, `Gif`).
- Produces: `pub(crate) fn encode_converted(img: &DynamicImage, from: ImgFormat, to: ImgFormat, quality: Quality) -> Result<Vec<u8>>` trong `ops/image_io.rs`.
- Produces: `pub fn make_png_pattern(dir: &Path, name: &str, w: u32, h: u32, with_alpha: bool) -> PathBuf` trong `tests/common/mod.rs`: ảnh có gradient, alpha (nếu bật) nằm trong 100..=255 để không có điểm ảnh trong suốt hoàn toàn.

- [ ] **Step 1: Viết test thất bại**

Trong `tests/convert.rs` (dùng `make_png_pattern`, `make_jpeg`), kiểm chứng kiểu mã hoá qua chữ ký chunk của WebP (byte 12..16: `VP8L` là lossless, `VP8 ` là lossy):
- `png_to_webp_is_lossless_and_pixel_exact`: chunk là `b"VP8L"` và `image::open(out).to_rgb8() == image::open(src).to_rgb8()`.
- `png_with_alpha_to_webp_keeps_alpha_exactly`: `with_alpha = true`, so sánh `to_rgba8()` bằng nhau.
- `jpeg_to_webp_is_lossy`: chunk là `b"VP8 "` (không phải `VP8L`).
- Giữ nguyên test `webp_and_avif_have_correct_container_signature` hiện có.

Trong `image_io.rs` `#[cfg(test)]`, dùng `DynamicImage` dựng tay:
- `lossless_source_to_jpeg_ignores_preset`: `encode_converted(&img, Png, Jpeg, Small) == encode_converted(&img, Png, Jpeg, High)`.
- `lossy_source_to_jpeg_follows_preset`: `encode_converted(&img, WebP, Jpeg, Small).len() < encode_converted(&img, WebP, Jpeg, High).len()` (ảnh gradient đủ lớn, ví dụ 64×64).

- [ ] **Step 2: Chạy để xác nhận thất bại**

Run: `cargo test -p morphix-core --test convert --lib -- png_to_webp jpeg_to_webp lossless_source lossy_source`
Expected: FAIL (lỗi biên dịch vì `encode_converted`, `make_png_pattern` chưa có).

- [ ] **Step 3: Cài đặt**

- `ImgFormat::is_lossless_source` trong `kind.rs`, kèm WHY: WebP nguồn bị coi là đã lossy vì không phân biệt được từ phần mở rộng, chọn phía không nâng chất lượng giả.
- `encode_converted` trong `image_io.rs` theo bảng của spec: nếu `to == WebP && from.is_lossless_source()` thì mã hoá lossless bằng `webp::Encoder::from_image(&to_eight_bit(img))?.encode_lossless()` (giữ cách đổi lỗi thành `MorphixError::InvalidInput` như nhánh WebP của `encode_image`). Ngược lại gọi `encode_image` với chất lượng hiệu lực: `quality` nếu `to == Jpeg && !from.is_lossless_source()`, còn lại `Quality::High`.
- `convert_image` lấy `from` bằng `image_format_of(input)?` (sau khi đổi sang `pub(crate)` và import qua `super::compress`) rồi gọi `encode_converted`.
- `make_png_pattern` trong `tests/common/mod.rs`.

- [ ] **Step 4: Chạy để xác nhận đạt**

Run: `cargo test -p morphix-core`
Expected: PASS, gồm mọi test `convert.rs` và `compress_resize.rs` cũ.

- [ ] **Step 5: Commit**

```bash
git add crates/morphix-core/src/kind.rs crates/morphix-core/src/ops crates/morphix-core/tests
git commit -m "feat(morphix-core): chọn lossless/lossy theo định dạng nguồn khi convert ảnh"
```

---

### Task 2: Cảnh báo khi file ra không nhỏ hơn file gốc

**Files:**
- Modify: `crates/morphix-core/src/job.rs` (`ItemReport`, `run_job`, hàm mới `size_warning`)
- Modify (thêm `warnings: Vec::new()` vào struct literal): `crates/morphix-gui/src/jobs.rs:335` (helper test), `crates/morphix-gui/tests/commands.rs` (9 chỗ dựng `ItemReport`)
- Test: `#[cfg(test)] mod tests` trong `job.rs`; `crates/morphix-core/tests/convert.rs`

**Interfaces:**
- Consumes: `Operation::ConvertImage { to: ImgFormat }`, `run_item(...)` trả `Result<PathBuf>` (không đổi).
- Produces: `ItemReport { inputs: Vec<PathBuf>, result: Result<PathBuf>, warnings: Vec<String> }`.
- Produces: `fn size_warning(input: &Path, input_len: u64, output_len: u64, to: ImgFormat) -> Option<String>` (private trong `job.rs`). Trả `Some` chỉ khi `to` là `WebP`, `Avif` hoặc `Jpeg` và `output_len >= input_len`. Thông điệp bắt đầu bằng tên file gốc, dạng `"{tên}: file mới lớn hơn file gốc {pct}%"` với `pct = (out - in) * 100 / in`; khi bằng nhau là `"{tên}: file mới không nhỏ hơn file gốc"`; khi `input_len == 0` bỏ phần phần trăm.

- [ ] **Step 1: Viết test thất bại**

Unit test `size_warning` trong `job.rs`: nhỏ hơn → `None`; bằng nhau → `Some` chứa `"không nhỏ hơn"`; lớn hơn gấp đôi → chứa `"100%"`; `input_len = 0` không panic và không chứa `%`; `to = Png` với file ra lớn hơn → `None`.

Trong `tests/convert.rs`, chạy `run_job` với `Job { operation: ConvertImage { to: Avif }, ... }` (không cần ffmpeg):
- `tiny_png_to_avif_keeps_output_and_warns`: đầu vào `make_png(.., 8, 8)`; mong đợi `result` là `Ok` với file tồn tại, và `warnings.len() == 1` chứa `"lớn hơn file gốc"`. Nếu AVIF nhỏ hơn PNG trên máy này, chọn đầu vào nhỏ hơn cho đến khi chắc chắn lớn hơn; ghi lý do trong comment.
- `jpeg_to_png_never_warns`: `warnings.is_empty()`.

- [ ] **Step 2: Chạy để xác nhận thất bại**

Run: `cargo test -p morphix-core -- size_warning tiny_png_to_avif jpeg_to_png_never`
Expected: FAIL (lỗi biên dịch vì `warnings`/`size_warning` chưa có).

- [ ] **Step 3: Cài đặt**

- Thêm trường `warnings` vào `ItemReport`; cập nhật mọi struct literal liệt kê ở trên bằng `warnings: Vec::new()`.
- Trong `run_job`, sau `run_item`, với `Operation::ConvertImage { to }` và `Ok(output)`: đọc `metadata().len()` của `inputs[0]` và `output`, gọi `size_warning`. Nếu đọc metadata lỗi thì `tracing::warn!` kèm đường dẫn và nguyên nhân, không thêm cảnh báo (không làm hỏng một job đã ghi xong file).
- WHY comment: chỉ nhóm đích WebP/AVIF/JPEG có mục đích là nhỏ hơn; PNG/TIFF/BMP/GIF to hơn là bình thường.

- [ ] **Step 4: Chạy để xác nhận đạt**

Run: `cargo test -p morphix-core -p morphix-gui`
Expected: PASS (các test GUI cũ biên dịch được sau khi thêm `warnings`).

- [ ] **Step 5: Commit**

```bash
git add crates/morphix-core crates/morphix-gui
git commit -m "feat(morphix-core): cảnh báo khi ảnh convert ra không nhỏ hơn file gốc"
```

---

### Task 3: Hiển thị cảnh báo ở CLI và GUI, rồi kiểm tra toàn bộ

**Files:**
- Modify: `crates/morphix-cli/src/output.rs` (`JsonItem`, `print_results`)
- Modify: `crates/morphix-gui/src/commands.rs:222-289` (`history_entry_for_report`)
- Test: `crates/morphix-cli/tests/cli.rs`, `crates/morphix-gui/tests/commands.rs`

**Interfaces:**
- Consumes: `ItemReport.warnings: Vec<String>` từ Task 2.
- Produces: JSON của `--json` có thêm khoá `warnings` (mảng chuỗi, có thể rỗng) trong mỗi phần tử `results`; văn bản thường in mỗi cảnh báo thành một dòng `cảnh báo  {nội dung}` ra `stderr`, sau dòng `ok`. Mã thoát không đổi.
- Produces: `HistoryEntry.detail` nối cảnh báo đầu tiên (nếu có) sau lý do lỗi, cùng dấu ` · `.

- [ ] **Step 1: Viết test thất bại**

- `cli.rs`, dùng `Sandbox` như các test lân cận (`s.png("a.png", 8, 8)`, rồi `convert a.png --to avif`): thoát thành công, `stderr` chứa `"cảnh báo"`, file `a.avif` tồn tại. Với `--json`: `results[0]["warnings"]` có đúng 1 phần tử.
- `cli.rs`: test `json_output_is_a_single_parsable_object` hiện có vẫn đạt (khoá mới chỉ thêm vào).
- `gui/tests/commands.rs`, theo mẫu các test `history_entry_for_report_*`: một `ItemReport` thành công có `warnings` không rỗng, và `entry.detail` chứa nội dung cảnh báo, `entry.status == EntryStatus::Done`.

- [ ] **Step 2: Chạy để xác nhận thất bại**

Run: `cargo test -p morphix-cli --test cli -- avif warnings; cargo test -p morphix-gui --test commands -- warning`
Expected: FAIL (chưa có `warnings` trong đầu ra).

- [ ] **Step 3: Cài đặt**

- `output.rs`: thêm `warnings: Vec<String>` vào `JsonItem`, điền từ `item.warnings.clone()`; ở nhánh văn bản, in cảnh báo bằng `eprintln!`.
- `commands.rs`: sau khối `first_error`, thêm cảnh báo đầu tiên vào `parts` với comment `WHY` (người dùng cần biết file không nhỏ đi, dù việc vẫn Done).

- [ ] **Step 4: Chạy toàn bộ cổng kiểm tra**

Run:
```
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo fmt --all -- --check
```
Expected: clippy 0 lỗi; mọi test đạt (trừ các test PDFium đã biết cần runtime PDFium và các lỗi test Windows có sẵn ghi trong `docs/superpowers/specs/2026-10-26-morphix-windows-explorer-fan-design.md`; ghi lại chính xác test nào fail trước khi bắt đầu để so sánh). `fmt --check` chỉ được báo lệch ở các file không thuộc thay đổi này.

Run: `cargo check -p morphix-cli -p morphix-gui`
Expected: PASS (trên máy Windows đây là đường `cfg(windows)`; trên Linux chạy thêm `--target x86_64-pc-windows-gnu`).

- [ ] **Step 5: Commit**

```bash
git add crates/morphix-cli crates/morphix-gui
git commit -m "feat(morphix-cli): hiện cảnh báo file không nhỏ hơn ở CLI và lịch sử GUI"
```
