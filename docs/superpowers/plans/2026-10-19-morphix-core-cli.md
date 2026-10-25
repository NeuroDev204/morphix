# Morphix: lõi và CLI (đợt 1a) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Dựng `morphix-core` (nhận dạng file, bảng việc theo loại file, bộ chạy việc, xử lý ảnh gồm ảnh sang PDF gộp, nén, đổi cỡ, xoá metadata) và CLI `morphix` dùng được trong terminal.

**Architecture:** Cargo workspace trong `morphix/`. `morphix-core` là thư viện không phụ thuộc giao diện, không trạng thái toàn cục; `morphix-cli` chỉ phân tích tham số rồi gọi lõi. Giao diện Tauri (đợt 1b) và tích hợp menu chuột phải (đợt 1c) sẽ dùng đúng các hàm này, nên mọi chữ ký ở dưới là hợp đồng.

**Tech Stack:** Rust 1.99, `image` 0.25.10 (features `jpeg png gif bmp tiff webp avif`), `webp` 0.3.1, `img-parts` 0.4.1, `pdf-writer` 0.15.0, `flate2` 1.1.10, `infer` 0.22.0, `natord` 1.0.9, `tempfile` 3.27.0, `thiserror` 2.0.21, `tracing` 0.1.44, `tracing-subscriber` 0.3.23, `directories` 6.0.0, `clap` 4.6.7 (derive), `serde` 1.0.229 + `serde_json` 1.0.154, `which` (bản ổn định mới nhất). Dev: `lopdf` 0.45.0, `kamadak-exif` 0.6.1, `assert_cmd` 2.2.2, `predicates` 3.1.4.

**Spec:** `morphix/docs/superpowers/specs/2026-10-19-morphix-design.md` (mục 5.4, 7, 8, 9). Wireframe: `morphix/docs/superpowers/specs/assets/2026-10-19-morphix-wireframe.html`.

**Phạm vi bị cắt có chủ đích:** chỉ ảnh (đợt 1). Cắt ảnh, chú thích, đọc QR là đợt 5; video, âm thanh, PDF, file nén chưa có việc nào trong bảng việc. Lệnh `menu` và `install-integration` thuộc kế hoạch 1b, 1c.

## Global Constraints

- Rust, chạy trên Linux và Windows; đường dẫn dùng `std::path`, không giả định dấu `/`.
- **Không bao giờ ghi đè file gốc.** Ghi vào file tạm cùng thư mục rồi đổi tên; huỷ hay lỗi giữa chừng thì không để lại file dở.
- Hậu tố đầu ra: `-gop` (gộp), `-nen` (nén), `-sach` (xoá metadata), đổi đuôi khi chuyển định dạng; trùng tên thì thêm ` (1)`, ` (2)`.
- Preset chất lượng: Nhỏ gọn, Cân bằng (mặc định), Cao; JPEG 70 / 85 / 95.
- Ảnh nhiều file sang PDF: **một file PDF duy nhất**, trang xếp theo tên file, sắp xếp tự nhiên (`IMG_2` trước `IMG_10`).
- Lỗi một file trong lô không làm hỏng cả lô. Mọi lỗi được ghi log (`tracing`) với đường dẫn và nguyên nhân, và hiện cho người dùng bằng một câu nói được nguyên nhân. **Không nuốt lỗi im lặng.**
- CLI: lỗi in ra `stderr`, kết quả ra `stdout`; cờ chung `--json`, `--dry-run`; mã thoát `0` thành công, `1` có lỗi, `2` dùng sai lệnh, `3` làm được một phần.
- Chữ hiển thị (nhãn việc, thông báo lỗi) là tiếng Việt; nhãn việc chỉ khai báo trong `catalog.rs`.
- Chữ ký hàm có kiểu tường minh; comment giải thích "tại sao", không giải thích "cái gì".
- **Git:** kho git gốc là `<repo>`, đang có thay đổi chưa commit không liên quan. Làm trên nhánh mới `feature/morphix-core`, chỉ `git add` đường dẫn dưới `morphix/`, không dùng `git add -A` hay `git commit -a`.
- Trước khi báo xong (từ `AGENTS.md` mục 4): `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` đều sạch; kiểm log; `git status` sạch cho đường dẫn dưới `morphix/`.

## Review Focus

Các đầu vào mà spec ngầm hiểu nhưng không nói; người dùng thật sẽ gặp; mỗi dòng có test ở task sở hữu:
1. **Ảnh điện thoại có EXIF Orientation khác 1** phải ra thẳng đứng ở đổi định dạng, PDF, và xoá metadata không được làm ảnh nằm ngang (Task 4, 5, 6).
2. **PNG trong suốt sang JPG hoặc PDF** ra nền trắng, không đen (Task 4, 5).
3. **Tên file có dấu tiếng Việt, khoảng trắng, trùng tên có sẵn:** không lỗi, không ghi đè (Task 3, 4).
4. **File hỏng hoặc giả đuôi** (0 byte, văn bản đặt tên `.jpg`) trong một lô: file đó báo lỗi đúng nguyên nhân, các file còn lại vẫn chạy, không panic (Task 4, 9, 10).
5. **Nén xong không nhỏ hơn bản gốc** (ảnh đã tối ưu): không tạo file, báo "không nén thêm được"; và chọn toàn ảnh JPG thì không có việc "đổi sang JPG" (Task 7, 8).

## Cấu trúc file

```
morphix/
  Cargo.toml                     workspace
  .gitignore                     /target
  crates/morphix-core/
    Cargo.toml
    src/lib.rs                   khai báo module, re-export kiểu công khai
    src/error.rs                 MorphixError, Result
    src/paths.rs                 data_dir(), find_ffmpeg()
    src/cancel.rs                CancelToken
    src/kind.rs                  ImgFormat, FileKind, detect
    src/naming.rs                desired_output, write_output, natural_sort
    src/quality.rs               Quality
    src/catalog.rs               ActionKind, Action, actions_for
    src/job.rs                   Operation, Job, Progress, JobReport, run_job, planned_outputs
    src/ops/mod.rs
    src/ops/image_io.rs          load_oriented, encode_image, convert_image
    src/ops/image_pdf.rs         images_to_pdf
    src/ops/strip_meta.rs        strip_metadata, exif_with_orientation
    src/ops/compress.rs          compress_image, resize_image, ResizeSpec
    tests/common/mod.rs          hàm tạo ảnh mẫu cho test
    tests/convert.rs  tests/pdf.rs  tests/strip_meta.rs  tests/compress_resize.rs  tests/job.rs
  crates/morphix-cli/
    Cargo.toml
    src/main.rs                  clap, điều phối, mã thoát, log
    src/output.rs                in kết quả (người đọc và --json)
    tests/cli.rs
```

---

### Task 1: Workspace, lỗi, đường dẫn dữ liệu, CancelToken

**Files:**
- Create: `morphix/Cargo.toml`, `morphix/.gitignore`, `crates/morphix-core/Cargo.toml`, `crates/morphix-core/src/{lib.rs,error.rs,paths.rs,cancel.rs}`
- Test: các `#[cfg(test)]` trong `paths.rs` và `cancel.rs`

**Interfaces:**
- Produces:
  - `pub enum MorphixError { Io{path: PathBuf, source: std::io::Error}, Unsupported{path: PathBuf}, Corrupt{path: PathBuf, reason: String}, NotSmaller{path: PathBuf}, InvalidInput(String), Cancelled }` (derive `Debug, thiserror::Error`, thông báo tiếng Việt nói rõ nguyên nhân, ví dụ `"{path} bị hỏng: {reason}"`, `"không nén thêm được {path}"`, `"đã huỷ"`) và `pub type Result<T> = std::result::Result<T, MorphixError>`.
  - `pub fn data_dir() -> PathBuf`: biến môi trường `MORPHIX_DATA_DIR` nếu có, không thì thư mục dữ liệu cục bộ theo `directories::ProjectDirs::from("", "", "Morphix")`; không xác định được thì `std::env::temp_dir().join("morphix")` kèm `tracing::warn!` (lùi có log, không panic).
  - `#[derive(Clone, Default)] pub struct CancelToken` với `fn new() -> Self`, `fn cancel(&self)`, `fn is_cancelled(&self) -> bool`, `fn check(&self) -> Result<()>` (trả `Err(MorphixError::Cancelled)` khi đã huỷ). Các bản sao chia sẻ cùng cờ (`Arc<AtomicBool>`).

- [ ] **Step 1: Write the failing tests**

```rust
// paths.rs
#[test]
fn data_dir_honours_env_override() {
    std::env::set_var("MORPHIX_DATA_DIR", "/tmp/morphix-test-dir");
    assert_eq!(data_dir(), std::path::PathBuf::from("/tmp/morphix-test-dir"));
}
// cancel.rs
#[test]
fn clones_share_the_cancel_flag() {
    let token = CancelToken::new();
    let clone = token.clone();
    assert!(clone.check().is_ok());
    token.cancel();
    assert!(matches!(clone.check(), Err(MorphixError::Cancelled)));
}
```

- [ ] **Step 2: Run to verify fail**

Run: `cd morphix && cargo test -p morphix-core`
Expected: FAIL (biên dịch lỗi vì chưa có workspace hoặc kiểu).

- [ ] **Step 3: Implement**

Tạo workspace (`resolver = "2"`, bây giờ chỉ khai `morphix-core`; `morphix-cli` thêm ở Task 10), `.gitignore` với `/target`, và các module trên. Test `data_dir` đặt biến môi trường nên giữ **một** test đọc biến này trong cả crate.

- [ ] **Step 4: Run to verify pass**

Run: `cd morphix && cargo test -p morphix-core`
Expected: PASS (2 tests).

- [ ] **Step 5: Commit**

```bash
git switch -c feature/morphix-core
git add morphix/Cargo.toml morphix/.gitignore morphix/crates/morphix-core
git commit -m "feat(morphix-core): workspace, error type, data dir, cancel token"
```

---

### Task 2: Nhận dạng loại file

**Files:**
- Create: `crates/morphix-core/src/kind.rs`; Modify: `lib.rs` (khai báo module)
- Test: `#[cfg(test)]` trong `kind.rs`

**Interfaces:**
- Consumes: `MorphixError`, `Result`.
- Produces:
  - `#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)] pub enum ImgFormat { Jpeg, Png, WebP, Avif, Gif, Tiff, Bmp }` với `pub const ALL_ORDERED: [ImgFormat; 7]` (đúng thứ tự trên, là thứ tự múi của vòng tròn), `fn extension(self) -> &'static str` (`jpg png webp avif gif tiff bmp`), `fn label(self) -> &'static str` (`JPG PNG WebP AVIF GIF TIFF BMP`), `fn from_extension(ext: &str) -> Option<ImgFormat>` (không phân biệt hoa thường; `jpeg`→Jpeg, `tif`→Tiff), `fn supports_metadata_tools(self) -> bool` (chỉ Jpeg, Png, WebP).
  - `#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)] pub enum FileKind { Image(ImgFormat), Pdf, Video, Audio, Archive, Unknown }`.
  - `pub fn detect(path: &Path) -> Result<FileKind>`: đọc tối đa 8 KiB đầu file và nhận dạng **theo nội dung** bằng `infer`, bỏ qua đuôi. File không tồn tại hoặc không đọc được: `Err(MorphixError::Io{..})`. File rỗng hoặc không nhận ra: `Ok(FileKind::Unknown)`.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn detects_by_content_not_extension() {
    // JPEG thật đặt tên .png, PDF thật đặt tên .txt, zip thật đặt tên .jpg
    assert_eq!(detect(&write_tmp("a.png", &tiny_jpeg())).unwrap(), FileKind::Image(ImgFormat::Jpeg));
    assert_eq!(detect(&write_tmp("b.txt", b"%PDF-1.4\n%....")).unwrap(), FileKind::Pdf);
    assert_eq!(detect(&write_tmp("c.jpg", b"PK\x03\x04rest")).unwrap(), FileKind::Archive);
}
#[test]
fn empty_and_unknown_files_are_unknown() {
    assert_eq!(detect(&write_tmp("d.jpg", b"")).unwrap(), FileKind::Unknown);
    assert_eq!(detect(&write_tmp("e.bin", b"hello world")).unwrap(), FileKind::Unknown);
}
#[test]
fn missing_file_is_io_error() {
    assert!(matches!(detect(Path::new("/no/such/file.jpg")), Err(MorphixError::Io { .. })));
}
#[test]
fn extension_mapping_is_case_insensitive() {
    assert_eq!(ImgFormat::from_extension("JPEG"), Some(ImgFormat::Jpeg));
    assert_eq!(ImgFormat::from_extension("tif"), Some(ImgFormat::Tiff));
    assert_eq!(ImgFormat::from_extension("doc"), None);
}
```
`write_tmp` và `tiny_jpeg` là hàm phụ trong module test (`tempfile::tempdir`, `image::codecs::jpeg::JpegEncoder`).

- [ ] **Step 2: Run to verify fail**

Run: `cd morphix && cargo test -p morphix-core kind::`
Expected: FAIL (chưa có hàm).

- [ ] **Step 3: Implement** `detect` bằng `infer::get`; ánh xạ MIME ảnh sang `ImgFormat`, `application/pdf` sang `Pdf`, loại matcher Video/Audio/Archive của `infer` sang các biến còn lại.

- [ ] **Step 4: Run to verify pass**

Run: `cd morphix && cargo test -p morphix-core kind::`
Expected: PASS (4 tests).

- [ ] **Step 5: Commit**

```bash
git add morphix/crates/morphix-core/src
git commit -m "feat(morphix-core): detect file kind by content"
```

---

### Task 3: Đặt tên đầu ra, ghi an toàn, sắp xếp tự nhiên

**Files:**
- Create: `crates/morphix-core/src/naming.rs`; Modify: `lib.rs`
- Test: `#[cfg(test)]` trong `naming.rs`

**Interfaces:**
- Consumes: `MorphixError`, `Result`.
- Produces:
  - `pub fn desired_output(input: &Path, suffix: &str, extension: &str, out_dir: Option<&Path>) -> PathBuf`: thư mục là `out_dir` hoặc thư mục cha của `input`; tên là `stem` + `suffix` + `.` + `extension`. Không đụng đến đĩa.
  - `pub fn write_output<F>(desired: &Path, write: F) -> Result<PathBuf> where F: FnOnce(&mut std::fs::File) -> Result<()>`: tạo thư mục cha nếu thiếu; ghi vào file tạm cùng thư mục (`tempfile::NamedTempFile::new_in`); đổi tên bằng `persist_noclobber`; nếu `desired` đã tồn tại thì thử `stem (1).ext`, `stem (2).ext`… và trả đường dẫn thật đã tạo. `write` trả lỗi thì xoá file tạm và trả lỗi đó.
  - `pub fn natural_sort(paths: &mut [PathBuf])`: sắp theo tên file, không phân biệt hoa thường, tự nhiên (`natord::compare_ignore_case`).

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn desired_output_keeps_unicode_stem_and_adds_suffix() {
    let p = desired_output(Path::new("/x/Ảnh chụp màn hình 1.png"), "-nen", "png", None);
    assert_eq!(p, PathBuf::from("/x/Ảnh chụp màn hình 1-nen.png"));
}
#[test]
fn write_output_never_overwrites_existing_file() {
    let dir = tempfile::tempdir().unwrap();
    let wanted = dir.path().join("a.pdf");
    std::fs::write(&wanted, b"original").unwrap();
    let made = write_output(&wanted, |f| { f.write_all(b"new").map_err(|e| io_err(&wanted, e)) }).unwrap();
    assert_eq!(made, dir.path().join("a (1).pdf"));
    assert_eq!(std::fs::read(&wanted).unwrap(), b"original");
    let again = write_output(&wanted, |f| { f.write_all(b"x").map_err(|e| io_err(&wanted, e)) }).unwrap();
    assert_eq!(again, dir.path().join("a (2).pdf"));
}
#[test]
fn failed_write_leaves_no_temp_file() {
    let dir = tempfile::tempdir().unwrap();
    let r = write_output(&dir.path().join("a.pdf"), |_| Err(MorphixError::Cancelled));
    assert!(matches!(r, Err(MorphixError::Cancelled)));
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
}
#[test]
fn natural_sort_orders_numbers_by_value() {
    let mut v: Vec<PathBuf> = ["IMG_10.jpg", "img_2.jpg", "IMG_1.jpg"].iter().map(PathBuf::from).collect();
    natural_sort(&mut v);
    assert_eq!(v, vec![PathBuf::from("IMG_1.jpg"), PathBuf::from("img_2.jpg"), PathBuf::from("IMG_10.jpg")]);
}
```
(`io_err` là hàm phụ trong test dựng `MorphixError::Io`.)

- [ ] **Step 2: Run to verify fail**

Run: `cd morphix && cargo test -p morphix-core naming::`
Expected: FAIL.

- [ ] **Step 3: Implement** theo các chữ ký trên. Dùng `PersistError` của `tempfile` để lấy lại file tạm khi đụng tên rồi thử số kế tiếp, không tạo file tạm mới mỗi vòng.

- [ ] **Step 4: Run to verify pass**

Run: `cd morphix && cargo test -p morphix-core naming::`
Expected: PASS (4 tests).

- [ ] **Step 5: Commit**

```bash
git add morphix/crates/morphix-core/src
git commit -m "feat(morphix-core): safe output naming, atomic no-clobber write, natural sort"
```

---

### Task 4: Đổi định dạng ảnh (và nền tảng đọc/mã hoá ảnh)

**Files:**
- Create: `src/quality.rs`, `src/ops/mod.rs`, `src/ops/image_io.rs`, `tests/common/mod.rs`, `tests/convert.rs`; Modify: `lib.rs`

**Interfaces:**
- Consumes: `ImgFormat`, `MorphixError`, `desired_output`, `write_output`.
- Produces:
  - `#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)] pub enum Quality { Small, #[default] Balanced, High }` với `fn jpeg(self) -> u8` (70, 85, 95), `fn webp(self) -> f32` (cùng giá trị), `fn avif(self) -> u8` (55, 70, 85).
  - `pub fn load_oriented(path: &Path) -> Result<image::DynamicImage>`: mở bằng `ImageReader` đoán định dạng theo nội dung, **áp dụng EXIF Orientation** (`ImageDecoder::orientation` rồi `DynamicImage::apply_orientation`; kiểu `image::metadata::Orientation`). Lỗi giải mã trả `Corrupt{path, reason}`; không phải ảnh trả `Unsupported`; không mở được trả `Io`.
  - `pub fn encode_image(img: &image::DynamicImage, to: ImgFormat, quality: Quality) -> Result<Vec<u8>>`: JPEG **dán ảnh trong suốt lên nền trắng** trước khi mã hoá (`JpegEncoder::new_with_quality`); WebP dùng `webp::Encoder::from_image(..).encode(quality.webp())`; AVIF dùng `AvifEncoder::new_with_speed_quality(.., 8, quality.avif())`; PNG, GIF, TIFF, BMP dùng `write_to` của `image`. GIF đầu vào động chỉ lấy khung đầu (đã biết, ghi trong `--help` ở Task 10).
  - `pub fn convert_image(input: &Path, to: ImgFormat, quality: Quality, out_dir: Option<&Path>) -> Result<PathBuf>`: đổi định dạng, đầu ra `desired_output(input, "", to.extension(), out_dir)` qua `write_output`. Mọi siêu dữ liệu bị bỏ khi mã hoá lại (chỉ giữ hướng ảnh đã bake vào điểm ảnh); ghi chú này vào `--help`.
- Test helpers (`tests/common/mod.rs`): `make_jpeg(dir: &Path, name: &str, w: u32, h: u32, exif_orientation: Option<u16>) -> PathBuf`, `make_png_rgba(dir: &Path, name: &str, w: u32, h: u32) -> PathBuf` (toàn pixel trong suốt hoàn toàn), `make_png(dir, name, w, h) -> PathBuf`. `make_jpeg` ghi hướng ảnh bằng `img_parts::jpeg::Jpeg::set_exif(Some(..))` với payload TIFF **không** có tiền tố `Exif\0\0` (đã kiểm: cả `kamadak-exif` và `image` đọc được), tạo bằng `exif_with_orientation` của Task 6; vì task này chạy trước, tạm chép hàm 12 dòng đó vào `common` rồi ở Task 6 đổi sang gọi hàm công khai.

- [ ] **Step 1: Write the failing tests** (`tests/convert.rs`)

```rust
#[test] fn jpg_to_png_keeps_dimensions() {
    let d = tempdir().unwrap();
    let src = make_jpeg(d.path(), "a.jpg", 64, 32, None);
    let out = convert_image(&src, ImgFormat::Png, Quality::Balanced, None).unwrap();
    assert_eq!(out, d.path().join("a.png"));
    assert_eq!(image::image_dimensions(&out).unwrap(), (64, 32));
}
#[test] fn transparent_png_to_jpg_is_white_not_black() {
    let d = tempdir().unwrap();
    let src = make_png_rgba(d.path(), "t.png", 8, 8);
    let out = convert_image(&src, ImgFormat::Jpeg, Quality::High, None).unwrap();
    let px = image::open(&out).unwrap().to_rgb8().get_pixel(4, 4).0;
    assert!(px.iter().all(|c| *c > 240), "pixel was {px:?}");
}
#[test] fn exif_orientation_is_applied_to_pixels() {
    let d = tempdir().unwrap();
    let src = make_jpeg(d.path(), "p.jpg", 40, 20, Some(6)); // lưu ngang, hiển thị đứng
    let out = convert_image(&src, ImgFormat::Png, Quality::Balanced, None).unwrap();
    assert_eq!(image::image_dimensions(&out).unwrap(), (20, 40));
}
#[test] fn webp_and_avif_have_correct_container_signature() {
    let d = tempdir().unwrap();
    let src = make_jpeg(d.path(), "a.jpg", 32, 32, None);
    let w = std::fs::read(convert_image(&src, ImgFormat::WebP, Quality::Small, None).unwrap()).unwrap();
    assert_eq!((&w[0..4], &w[8..12]), (&b"RIFF"[..], &b"WEBP"[..]));
    let a = std::fs::read(convert_image(&src, ImgFormat::Avif, Quality::Small, None).unwrap()).unwrap();
    assert_eq!(&a[4..8], b"ftyp");
}
#[test] fn unicode_name_with_spaces_and_existing_output_are_safe() {
    let d = tempdir().unwrap();
    let src = make_jpeg(d.path(), "Ảnh chụp màn hình 1.jpg", 16, 16, None);
    std::fs::write(d.path().join("Ảnh chụp màn hình 1.png"), b"keep me").unwrap();
    let out = convert_image(&src, ImgFormat::Png, Quality::Balanced, None).unwrap();
    assert_eq!(out, d.path().join("Ảnh chụp màn hình 1 (1).png"));
    assert_eq!(std::fs::read(d.path().join("Ảnh chụp màn hình 1.png")).unwrap(), b"keep me");
}
#[test] fn corrupt_inputs_return_errors_not_panics() {
    let d = tempdir().unwrap();
    let empty = d.path().join("zero.jpg"); std::fs::write(&empty, b"").unwrap();
    let text = d.path().join("note.jpg"); std::fs::write(&text, b"just text").unwrap();
    for p in [&empty, &text] {
        assert!(matches!(convert_image(p, ImgFormat::Png, Quality::Balanced, None),
            Err(MorphixError::Corrupt { .. }) | Err(MorphixError::Unsupported { .. })));
    }
    assert_eq!(std::fs::read_dir(d.path()).unwrap().count(), 2, "không được tạo file đầu ra");
}
```

- [ ] **Step 2: Run to verify fail**

Run: `cd morphix && cargo test -p morphix-core --test convert`
Expected: FAIL (biên dịch lỗi).

- [ ] **Step 3: Implement** `Quality`, `load_oriented`, `encode_image`, `convert_image`. Thêm vào `Cargo.toml` của `morphix-core` các dependency trong header; `image` với `default-features = false` và features đã liệt kê.

- [ ] **Step 4: Run to verify pass**

Run: `cd morphix && cargo test -p morphix-core --test convert`
Expected: PASS (6 tests). Nếu build `webp` lỗi vì thiếu trình biên dịch C, ghi nguyên văn lỗi vào log công việc và dừng hỏi người dùng, đừng đổi thư viện.

- [ ] **Step 5: Commit**

```bash
git add morphix/crates/morphix-core
git commit -m "feat(morphix-core): image conversion with orientation, alpha flattening, safe output"
```

---

### Task 5: Ảnh sang PDF gộp

**Files:**
- Create: `src/ops/image_pdf.rs`, `tests/pdf.rs`; Modify: `src/ops/mod.rs`

**Interfaces:**
- Consumes: `load_oriented`, `naming::{desired_output, write_output, natural_sort}`, `CancelToken`, `MorphixError`.
- Produces: `pub fn images_to_pdf(inputs: &[PathBuf], out_dir: Option<&Path>, cancel: &CancelToken) -> Result<PathBuf>`.
  - Sắp một bản sao của `inputs` bằng `natural_sort`, mỗi ảnh một trang, **một file PDF duy nhất**.
  - Tên đầu ra: một ảnh thì `<stem>.pdf`; từ hai ảnh thì `<stem của ảnh đầu sau khi sắp>-gop.pdf`; qua `desired_output` + `write_output`.
  - Khổ trang **A4 tự chọn hướng** (595.276 × 841.890 pt, hoán đổi nếu ảnh rộng hơn cao sau khi áp hướng EXIF); ảnh co vừa trang giữ tỉ lệ, đặt giữa, không cắt, không lề **[quyết định của kế hoạch; spec 4.5 để cài đặt khổ giấy cho đợt sau]**.
  - JPEG gốc có hướng bình thường và giải mã ra `L8` hoặc `Rgb8` thì **nhúng thẳng** (`Filter::DctDecode`, không nén lại). Mọi trường hợp khác: `load_oriented`, dán nền trắng nếu có alpha, RGB8, nén zlib bằng `flate2`, `Filter::FlateDecode`.
  - Gọi `cancel.check()` trước mỗi ảnh; ảnh nào lỗi thì cả hàm trả lỗi của ảnh đó (có đường dẫn) và **không để lại file đầu ra**.
  - Danh sách rỗng: `Err(InvalidInput("không có ảnh nào để gộp"))`.

- [ ] **Step 1: Write the failing tests** (`tests/pdf.rs`, đọc lại PDF bằng `lopdf::Document::load`)

```rust
#[test] fn three_images_make_one_pdf_with_three_pages_and_a4_orientation() {
    let d = tempdir().unwrap();
    let files = vec![
        make_jpeg(d.path(), "IMG_1.jpg", 100, 200, None),      // đứng
        make_png(d.path(), "IMG_2.png", 200, 100),             // ngang
        make_png_rgba(d.path(), "IMG_3.png", 50, 50),
    ];
    let out = images_to_pdf(&files, None, &CancelToken::new()).unwrap();
    assert_eq!(out, d.path().join("IMG_1-gop.pdf"));
    let doc = lopdf::Document::load(&out).unwrap();
    assert_eq!(doc.get_pages().len(), 3);
    assert_eq!(media_box(&doc, 1), (595.276, 841.890));
    assert_eq!(media_box(&doc, 2), (841.890, 595.276));
}
#[test] fn pages_follow_natural_filename_order() {
    let d = tempdir().unwrap();
    let ten = make_png(d.path(), "IMG_10.png", 200, 100);   // ngang
    let two = make_jpeg(d.path(), "IMG_2.jpg", 100, 200, None); // đứng
    let out = images_to_pdf(&[ten, two], None, &CancelToken::new()).unwrap();
    assert_eq!(out.file_name().unwrap(), "IMG_2-gop.pdf");
    assert_eq!(media_box(&lopdf::Document::load(&out).unwrap(), 1), (595.276, 841.890));
}
#[test] fn single_image_is_named_without_suffix() {
    let d = tempdir().unwrap();
    let out = images_to_pdf(&[make_png(d.path(), "scan.png", 10, 10)], None, &CancelToken::new()).unwrap();
    assert_eq!(out, d.path().join("scan.pdf"));
}
#[test] fn plain_jpeg_is_embedded_without_recompression() {
    let d = tempdir().unwrap();
    let src = make_jpeg(d.path(), "a.jpg", 64, 64, None);
    let out = images_to_pdf(&[src.clone()], None, &CancelToken::new()).unwrap();
    let jpeg = std::fs::read(&src).unwrap();
    let pdf = std::fs::read(&out).unwrap();
    assert!(pdf.windows(jpeg.len()).any(|w| w == jpeg.as_slice()));
}
#[test] fn exif_rotated_jpeg_gets_portrait_page() {
    let d = tempdir().unwrap();
    let src = make_jpeg(d.path(), "p.jpg", 40, 20, Some(6));
    let out = images_to_pdf(&[src], None, &CancelToken::new()).unwrap();
    assert_eq!(media_box(&lopdf::Document::load(&out).unwrap(), 1), (595.276, 841.890));
}
#[test] fn transparent_png_is_flattened_to_white() {
    let d = tempdir().unwrap();
    let out = images_to_pdf(&[make_png_rgba(d.path(), "t.png", 4, 4)], None, &CancelToken::new()).unwrap();
    assert_eq!(first_pixel_rgb(&lopdf::Document::load(&out).unwrap()), [255, 255, 255]);
}
#[test] fn bad_input_or_cancel_leaves_no_output() {
    let d = tempdir().unwrap();
    let good = make_png(d.path(), "ok.png", 8, 8);
    let bad = d.path().join("bad.jpg"); std::fs::write(&bad, b"nope").unwrap();
    let e = images_to_pdf(&[good.clone(), bad.clone()], None, &CancelToken::new()).unwrap_err();
    assert!(e.to_string().contains("bad.jpg"));
    let token = CancelToken::new(); token.cancel();
    assert!(matches!(images_to_pdf(&[good], None, &token), Err(MorphixError::Cancelled)));
    assert!(std::fs::read_dir(d.path()).unwrap().all(|f| f.unwrap().path().extension().unwrap() != "pdf"));
}
```
`media_box(&Document, page_number) -> (f64, f64)` (so sánh với sai số 0.01) và `first_pixel_rgb` (giải nén stream ảnh trang 1 bằng `lopdf::Stream::decompressed_content`) là hàm phụ trong file test.

- [ ] **Step 2: Run to verify fail**

Run: `cd morphix && cargo test -p morphix-core --test pdf`
Expected: FAIL.

- [ ] **Step 3: Implement** với `pdf-writer` (API đã kiểm biên dịch: `Pdf::catalog().pages()`, `pages().kids().count()`, `page().media_box().parent().contents().resources().x_objects().pair()`, `image_xobject().filter().width().height().color_space().device_rgb()` hoặc `device_gray()`, `Content::save_state().transform().x_object().restore_state()`, `Pdf::stream`). Cấp `Ref` tuần tự bằng một bộ đếm. Nếu API `pdf-writer` gây vướng, được phép chuyển sang `printpdf` 0.12.8 miễn các test trên vẫn qua; ghi quyết định vào commit.

- [ ] **Step 4: Run to verify pass**

Run: `cd morphix && cargo test -p morphix-core --test pdf`
Expected: PASS (7 tests).

- [ ] **Step 5: Commit**

```bash
git add morphix/crates/morphix-core
git commit -m "feat(morphix-core): merge images into one A4 PDF"
```

---

### Task 6: Xoá metadata ảnh

**Files:**
- Create: `src/ops/strip_meta.rs`, `tests/strip_meta.rs`; Modify: `src/ops/mod.rs`, `tests/common/mod.rs` (dùng `exif_with_orientation` công khai thay bản chép ở Task 4)

**Interfaces:**
- Consumes: `naming`, `ImgFormat`, `MorphixError`, `image::ImageDecoder::orientation`.
- Produces:
  - `pub fn exif_with_orientation(exif_orientation: u16) -> Vec<u8>`: payload TIFF big-endian chỉ có thẻ Orientation (0x0112, kiểu SHORT), **không** có tiền tố `Exif\0\0` (đã kiểm với `img-parts` 0.4.1).
  - `pub fn strip_metadata(input: &Path, out_dir: Option<&Path>) -> Result<PathBuf>`: đầu ra `<stem>-sach.<đuôi cũ>`; **không mã hoá lại ảnh** (dùng `img_parts::DynImage` và các phương thức chỉnh đoạn/chunk).
    - JPEG: bỏ mọi đoạn APP1 (EXIF và XMP: `remove_segments_by_marker(markers::APP1)`), APP13 (IPTC) và COM; **giữ ICC** để màu không đổi; nếu hướng ảnh gốc khác bình thường thì ghi lại một EXIF chỉ chứa Orientation bằng `set_exif(Some(..))`, để ảnh không bị nằm ngang.
    - PNG: `remove_chunks_by_type` cho `tEXt`, `zTXt`, `iTXt`, `eXIf`; giữ ICC (`iCCP`).
    - WebP: `set_exif(None)`, `remove_chunks_by_id(*b"XMP ")`, giữ ICC; hướng ảnh xử lý như JPEG.
    - Định dạng khác (GIF, TIFF, BMP, AVIF) hoặc không phải ảnh: `Err(Unsupported{path})`.

- [ ] **Step 1: Write the failing tests** (`tests/strip_meta.rs`; dùng `img_parts` để cài dữ liệu "bí mật" và `exif::Reader` để đọc lại)

```rust
#[test] fn jpeg_loses_exif_and_xmp_text_but_keeps_pixels_and_icc() {
    let d = tempdir().unwrap();
    let src = jpeg_with_secrets(d.path(), "a.jpg"); // EXIF có "SECRET-EXIF", APP1 XMP "SECRET-XMP", COM "SECRET-COM", ICC giả
    let out = strip_metadata(&src, None).unwrap();
    assert_eq!(out, d.path().join("a-sach.jpg"));
    let bytes = std::fs::read(&out).unwrap();
    assert!(!contains(&bytes, b"SECRET"), "metadata còn sót");
    assert!(img_parts::jpeg::Jpeg::from_bytes(bytes.into()).unwrap().icc_profile().is_some());
    assert_eq!(image::open(&out).unwrap().to_rgb8(), image::open(&src).unwrap().to_rgb8()); // không mã hoá lại
}
#[test] fn rotated_photo_stays_upright_after_strip() {
    let d = tempdir().unwrap();
    let src = jpeg_with_secrets_and_orientation(d.path(), "p.jpg", 6);
    let out = strip_metadata(&src, None).unwrap();
    let r = exif::Reader::new().read_from_container(&mut std::io::Cursor::new(std::fs::read(&out).unwrap())).unwrap();
    assert_eq!(r.get_field(exif::Tag::Orientation, exif::In::PRIMARY).unwrap().value.get_uint(0), Some(6));
    assert!(!contains(&std::fs::read(&out).unwrap(), b"SECRET"));
}
#[test] fn png_text_chunks_are_removed() {
    let d = tempdir().unwrap();
    let src = png_with_text_chunk(d.path(), "a.png", "SECRET-PNG");
    let out = strip_metadata(&src, None).unwrap();
    assert!(!contains(&std::fs::read(&out).unwrap(), b"SECRET"));
    assert_eq!(image::open(&out).unwrap().to_rgba8(), image::open(&src).unwrap().to_rgba8());
}
#[test] fn unsupported_formats_and_non_images_error_cleanly() {
    let d = tempdir().unwrap();
    let gif = convert_image(&make_png(d.path(), "x.png", 4, 4), ImgFormat::Gif, Quality::Balanced, None).unwrap();
    assert!(matches!(strip_metadata(&gif, None), Err(MorphixError::Unsupported { .. })));
    let txt = d.path().join("t.jpg"); std::fs::write(&txt, b"text").unwrap();
    assert!(strip_metadata(&txt, None).is_err());
}
```
Hàm phụ `jpeg_with_secrets*`, `png_with_text_chunk`, `contains` nằm trong file test hoặc `common`.

- [ ] **Step 2: Run to verify fail**

Run: `cd morphix && cargo test -p morphix-core --test strip_meta`
Expected: FAIL.

- [ ] **Step 3: Implement** các hàm trên. Đọc hướng ảnh gốc bằng `ImageDecoder::orientation()`; chuyển `Orientation` sang giá trị EXIF 1 đến 8 bằng phương thức `to_exif()` của `image` nếu có (kiểm trên docs.rs của 0.25.10; không có thì tự ánh xạ bằng `match`).

- [ ] **Step 4: Run to verify pass**

Run: `cd morphix && cargo test -p morphix-core --test strip_meta`
Expected: PASS (4 tests).

- [ ] **Step 5: Commit**

```bash
git add morphix/crates/morphix-core
git commit -m "feat(morphix-core): strip image metadata without re-encoding"
```

---

### Task 7: Nén ảnh và đổi kích thước

**Files:**
- Create: `src/ops/compress.rs`, `tests/compress_resize.rs`; Modify: `src/ops/mod.rs`

**Interfaces:**
- Consumes: `load_oriented`, `encode_image`, `naming`, `ImgFormat`, `Quality`, `MorphixError`.
- Produces:
  - `pub fn compress_image(input: &Path, quality: Quality, out_dir: Option<&Path>) -> Result<PathBuf>`: giữ **nguyên định dạng**, hậu tố `-nen`; chỉ nhận JPEG, PNG, WebP (khác thì `Unsupported`). JPEG và WebP mã hoá lại theo preset; PNG mã hoá lại với `CompressionType::Best`. Nếu kết quả **không nhỏ hơn** bản gốc thì không tạo file và trả `Err(NotSmaller{path})`.
  - `#[derive(Debug, Clone, Copy, PartialEq, Eq)] pub enum ResizeSpec { Percent(u8), MaxEdge(u32) }`.
  - `pub fn resize_image(input: &Path, spec: ResizeSpec, quality: Quality, out_dir: Option<&Path>) -> Result<PathBuf>`: giữ nguyên định dạng, tên `<stem>-<w>x<h>.<đuôi>`, lọc `Lanczos3` của `image::imageops` (không thêm thư viện). `Percent(0)` hoặc lớn hơn 100, `MaxEdge(0)` trả `InvalidInput`; cạnh dài nhất của ảnh đã nhỏ hơn hoặc bằng `MaxEdge` thì `Err(NotSmaller{path})` (không phóng to). Định dạng GIF, TIFF, BMP, AVIF đều được phép (mã hoá qua `encode_image`).

- [ ] **Step 1: Write the failing tests**

```rust
#[test] fn compress_shrinks_a_high_quality_jpeg_and_keeps_format() {
    let d = tempdir().unwrap();
    let src = noisy_jpeg(d.path(), "big.jpg", 800, 600, 98);   // JPEG chất lượng 98 từ ảnh nhiễu
    let out = compress_image(&src, Quality::Small, None).unwrap();
    assert_eq!(out, d.path().join("big-nen.jpg"));
    assert!(std::fs::metadata(&out).unwrap().len() < std::fs::metadata(&src).unwrap().len());
}
#[test] fn compress_that_cannot_shrink_creates_nothing() {
    let d = tempdir().unwrap();
    let src = tiny_jpeg_quality(d.path(), "small.jpg", 8, 8, 20);
    assert!(matches!(compress_image(&src, Quality::High, None), Err(MorphixError::NotSmaller { .. })));
    assert_eq!(std::fs::read_dir(d.path()).unwrap().count(), 1);
}
#[test] fn resize_percent_and_max_edge_set_expected_dimensions_and_names() {
    let d = tempdir().unwrap();
    let src = make_jpeg(d.path(), "a.jpg", 100, 200, None);
    let half = resize_image(&src, ResizeSpec::Percent(50), Quality::Balanced, None).unwrap();
    assert_eq!(half, d.path().join("a-50x100.jpg"));
    assert_eq!(image::image_dimensions(&half).unwrap(), (50, 100));
    let wide = make_png(d.path(), "w.png", 200, 100);
    let capped = resize_image(&wide, ResizeSpec::MaxEdge(80), Quality::Balanced, None).unwrap();
    assert_eq!(image::image_dimensions(&capped).unwrap(), (80, 40));
}
#[test] fn resize_never_upscales_and_rejects_bad_specs() {
    let d = tempdir().unwrap();
    let src = make_png(d.path(), "a.png", 200, 100);
    assert!(matches!(resize_image(&src, ResizeSpec::MaxEdge(500), Quality::Balanced, None), Err(MorphixError::NotSmaller { .. })));
    assert!(matches!(resize_image(&src, ResizeSpec::Percent(0), Quality::Balanced, None), Err(MorphixError::InvalidInput(_))));
    assert!(matches!(resize_image(&src, ResizeSpec::Percent(150), Quality::Balanced, None), Err(MorphixError::InvalidInput(_))));
}
```

- [ ] **Step 2: Run to verify fail**

Run: `cd morphix && cargo test -p morphix-core --test compress_resize`
Expected: FAIL.

- [ ] **Step 3: Implement.** Mã hoá vào bộ nhớ trước, so kích thước, rồi mới `write_output` (để `NotSmaller` không tạo file).

- [ ] **Step 4: Run to verify pass**

Run: `cd morphix && cargo test -p morphix-core --test compress_resize`
Expected: PASS (4 tests).

- [ ] **Step 5: Commit**

```bash
git add morphix/crates/morphix-core
git commit -m "feat(morphix-core): compress and resize images"
```

---

### Task 8: Bảng việc theo loại file

**Files:**
- Create: `src/catalog.rs`; Modify: `lib.rs`
- Test: `#[cfg(test)]` trong `catalog.rs`

**Interfaces:**
- Consumes: `FileKind`, `ImgFormat`.
- Produces:
  - `#[derive(Debug, Clone, Copy, PartialEq, Eq)] pub enum ActionKind { ConvertTo(ImgFormat), MergeToPdf, Compress, StripMetadata, Resize }`.
  - `#[derive(Debug, Clone, PartialEq, Eq)] pub struct Action { pub kind: ActionKind, pub label: String, pub is_tool: bool }`.
  - `pub fn actions_for(kinds: &[FileKind]) -> Vec<Action>`: nguồn duy nhất cho các múi của vòng tròn, CLI `formats` và Khay. Quy tắc (spec 4.2, phần ảnh của đợt 1):
    - Rỗng, hoặc có loại khác `Image` (kể cả trộn ảnh với PDF): `vec![]`.
    - Toàn ảnh: trước hết `MergeToPdf` với nhãn `"PDF gộp"` khi có từ hai file, `"PDF"` khi một file (`is_tool: false`), rồi `ConvertTo(f)` theo `ImgFormat::ALL_ORDERED` với nhãn `f.label()`, **bỏ** định dạng mà **mọi** file được chọn đã là nó.
    - Công cụ (`is_tool: true`), đúng thứ tự: `Compress` `"Nén ảnh"` và `StripMetadata` `"Xoá metadata"` chỉ khi **mọi** file thoả `supports_metadata_tools()`; `Resize` `"Đổi kích thước"` luôn có.

- [ ] **Step 1: Write the failing tests**

```rust
fn labels(a: &[Action], tool: bool) -> Vec<&str> { a.iter().filter(|x| x.is_tool == tool).map(|x| x.label.as_str()).collect() }
#[test] fn single_jpeg_gets_pdf_first_then_other_formats_and_three_tools() {
    let a = actions_for(&[FileKind::Image(ImgFormat::Jpeg)]);
    assert_eq!(labels(&a, false), ["PDF", "PNG", "WebP", "AVIF", "GIF", "TIFF", "BMP"]);
    assert_eq!(labels(&a, true), ["Nén ảnh", "Xoá metadata", "Đổi kích thước"]);
    assert_eq!(a[0].kind, ActionKind::MergeToPdf);
}
#[test] fn several_images_get_merge_label() {
    let a = actions_for(&[FileKind::Image(ImgFormat::Png), FileKind::Image(ImgFormat::Jpeg)]);
    assert_eq!(a[0].label, "PDF gộp");
    assert_eq!(labels(&a, false).len(), 8); // trộn định dạng thì không bỏ định dạng nào
}
#[test] fn all_already_in_target_format_hides_that_format() {
    let a = actions_for(&[FileKind::Image(ImgFormat::Jpeg), FileKind::Image(ImgFormat::Jpeg)]);
    assert!(!labels(&a, false).contains(&"JPG"));
}
#[test] fn gif_only_has_resize_tool() {
    assert_eq!(labels(&actions_for(&[FileKind::Image(ImgFormat::Gif)]), true), ["Đổi kích thước"]);
}
#[test] fn mixed_or_empty_or_unsupported_selection_yields_nothing() {
    assert!(actions_for(&[]).is_empty());
    assert!(actions_for(&[FileKind::Image(ImgFormat::Png), FileKind::Pdf]).is_empty());
    assert!(actions_for(&[FileKind::Video]).is_empty());
}
```

- [ ] **Step 2: Run to verify fail**

Run: `cd morphix && cargo test -p morphix-core catalog::`
Expected: FAIL.

- [ ] **Step 3: Implement** theo quy tắc trên.

- [ ] **Step 4: Run to verify pass**

Run: `cd morphix && cargo test -p morphix-core catalog::`
Expected: PASS (5 tests).

- [ ] **Step 5: Commit**

```bash
git add morphix/crates/morphix-core/src
git commit -m "feat(morphix-core): action catalog per file kind"
```

---

### Task 9: Bộ chạy việc

**Files:**
- Create: `src/job.rs`, `tests/job.rs`; Modify: `lib.rs`

**Interfaces:**
- Consumes: mọi hàm của Task 4 đến 7, `CancelToken`, `Quality`, `ImgFormat`, `ResizeSpec`, `naming::desired_output`.
- Produces:
  - `#[derive(Debug, Clone, PartialEq, Eq)] pub enum Operation { ConvertImage { to: ImgFormat }, MergeImagesToPdf, CompressImage, StripMetadata, ResizeImage(ResizeSpec) }`.
  - `#[derive(Debug, Clone)] pub struct Job { pub inputs: Vec<PathBuf>, pub operation: Operation, pub quality: Quality, pub out_dir: Option<PathBuf> }`.
  - `#[derive(Debug, Clone, Copy, PartialEq, Eq)] pub struct Progress { pub done: usize, pub total: usize }`.
  - `pub struct ItemReport { pub inputs: Vec<PathBuf>, pub result: Result<PathBuf> }` và `pub struct JobReport { pub items: Vec<ItemReport> }` với `fn succeeded(&self) -> usize`, `fn failed(&self) -> usize`.
  - `pub fn run_job(job: &Job, cancel: &CancelToken, on_progress: &dyn Fn(Progress)) -> JobReport`: `MergeImagesToPdf` là **một** mục chứa mọi đầu vào; các việc khác mỗi đầu vào một mục, chạy tuần tự. Mục lỗi **không chặn** mục sau. Khi `cancel` đã bật, mọi mục chưa chạy có `Err(Cancelled)`. Gọi `on_progress` sau mỗi mục xong. Mỗi lỗi ghi `tracing::error!` kèm đường dẫn và nguyên nhân.
  - `pub fn planned_outputs(job: &Job) -> Vec<(Vec<PathBuf>, PathBuf)>`: tên đầu ra dự kiến cho từng mục (cho `--dry-run`), không đụng đĩa, không xử lý trùng tên.

- [ ] **Step 1: Write the failing tests** (`tests/job.rs`)

```rust
#[test] fn one_bad_file_does_not_stop_the_batch() {
    let d = tempdir().unwrap();
    let good1 = make_png(d.path(), "a.png", 8, 8);
    let bad = d.path().join("zero.png"); std::fs::write(&bad, b"").unwrap();
    let good2 = make_jpeg(d.path(), "b.jpg", 8, 8, None);
    let job = Job { inputs: vec![good1, bad.clone(), good2], operation: Operation::ConvertImage { to: ImgFormat::WebP },
                    quality: Quality::Balanced, out_dir: None };
    let seen = std::cell::RefCell::new(vec![]);
    let report = run_job(&job, &CancelToken::new(), &|p| seen.borrow_mut().push(p));
    assert_eq!((report.succeeded(), report.failed()), (2, 1));
    assert!(report.items[1].result.as_ref().unwrap_err().to_string().contains("zero.png"));
    assert_eq!(seen.borrow().last().copied(), Some(Progress { done: 3, total: 3 }));
}
#[test] fn merge_is_a_single_item_and_cancel_marks_everything_cancelled() {
    let d = tempdir().unwrap();
    let files = vec![make_png(d.path(), "1.png", 8, 8), make_png(d.path(), "2.png", 8, 8)];
    let job = Job { inputs: files, operation: Operation::MergeImagesToPdf, quality: Quality::Balanced, out_dir: None };
    assert_eq!(run_job(&job, &CancelToken::new(), &|_| {}).items.len(), 1);
    let token = CancelToken::new(); token.cancel();
    let report = run_job(&job, &token, &|_| {});
    assert!(matches!(report.items[0].result, Err(MorphixError::Cancelled)));
}
#[test] fn planned_outputs_match_real_names_when_no_collision() {
    let d = tempdir().unwrap();
    let a = make_jpeg(d.path(), "a.jpg", 8, 8, None);
    let job = Job { inputs: vec![a.clone()], operation: Operation::StripMetadata, quality: Quality::Balanced, out_dir: None };
    let plan = planned_outputs(&job);
    let real = run_job(&job, &CancelToken::new(), &|_| {}).items.remove(0).result.unwrap();
    assert_eq!(plan, vec![(vec![a], real)]);
}
```

- [ ] **Step 2: Run to verify fail**

Run: `cd morphix && cargo test -p morphix-core --test job`
Expected: FAIL.

- [ ] **Step 3: Implement.** `planned_outputs` dùng cùng hậu tố và đuôi với các hàm xử lý (đặt các hằng `SUFFIX_*` ở một nơi để hai phía không lệch nhau).

- [ ] **Step 4: Run to verify pass**

Run: `cd morphix && cargo test -p morphix-core`
Expected: PASS (toàn bộ test của lõi).

- [ ] **Step 5: Commit**

```bash
git add morphix/crates/morphix-core
git commit -m "feat(morphix-core): job runner with partial failure, cancel and progress"
```

---

### Task 10: CLI: convert, compress, resize, strip-meta, mã thoát, JSON, dry-run

**Files:**
- Create: `crates/morphix-cli/{Cargo.toml,src/main.rs,src/output.rs,tests/cli.rs}`; Modify: `morphix/Cargo.toml` (thêm thành viên)

**Interfaces:**
- Consumes: `morphix_core::{run_job, planned_outputs, Job, Operation, ImgFormat, Quality, ResizeSpec, CancelToken, JobReport, data_dir}`.
- Produces: binary `morphix` (clap derive) với:
  - Cờ chung (toàn cục): `-o, --out-dir <DIR>`, `--quality <small|balanced|high>` (mặc định `balanced`), `--json`, `--dry-run`.
  - `convert <FILES>... --to <jpg|jpeg|png|webp|avif|gif|tiff|tif|bmp|pdf> [--merge]`: `--to pdf --merge` là **một** `Job` `MergeImagesToPdf` với mọi file; `--to pdf` không `--merge` là mỗi file một `Job` (mỗi ảnh một PDF); các định dạng khác là một `Job` `ConvertImage`. `--merge` với định dạng không phải pdf là lỗi dùng sai (mã 2).
  - `compress <FILES>...`, `strip-meta <FILES>...`, `resize <FILES>... (--percent <1-100> | --max-edge <PX>)` (hai tuỳ chọn loại trừ nhau, bắt buộc một).
  - Đầu ra người đọc: mỗi mục một dòng `ok  <vào> -> <ra>` hoặc `lỗi  <vào>: <nguyên nhân>`; dòng lỗi đi `stderr`, dòng `ok` đi `stdout`. `--json`: **một** object JSON trên `stdout`: `{"results":[{"inputs":[..],"output":"..."|null,"error":"..."|null}],"ok":N,"failed":M}`. `--dry-run`: in `<vào> -> <ra dự kiến>` từ `planned_outputs`, không tạo file nào, thoát 0.
  - Mã thoát: `0` mọi mục ok; `1` mọi mục lỗi hoặc lỗi không phục hồi; `3` có ok lẫn lỗi; `2` do `clap` khi dùng sai.
  - Khởi tạo `tracing-subscriber` ghi vào `data_dir()/morphix.log` (tạo thư mục nếu thiếu) và `warn` trở lên ra `stderr`; không tạo được file log thì chỉ dùng `stderr` và in một cảnh báo (lùi có thông báo).
  - `--help` của `convert` nói rõ: GIF động chỉ lấy khung đầu; mã hoá lại bỏ siêu dữ liệu (dùng `strip-meta` để xoá mà không mã hoá lại).

- [ ] **Step 1: Write the failing tests** (`tests/cli.rs`, dùng API tìm binary hiện hành của `assert_cmd` 2.2.2, kiểm trên docs.rs; mọi test đặt `MORPHIX_DATA_DIR` là thư mục tạm)

```rust
#[test] fn merge_three_images_to_one_pdf() { /* convert a.jpg b.jpg c.png --to pdf --merge
   → exit 0, stdout chứa "a-gop.pdf", file tồn tại, lopdf thấy 3 trang */ }
#[test] fn pdf_without_merge_makes_one_pdf_per_image() { /* 2 ảnh → 2 file .pdf, mỗi file 1 trang */ }
#[test] fn partial_failure_exits_3_and_still_converts_the_rest() { /* 1 ảnh tốt + 1 file 0 byte → exit 3, stderr nêu tên file lỗi, file tốt đã chuyển */ }
#[test] fn all_failures_exit_1_with_cause_on_stderr() { /* chỉ file 0 byte → exit 1 */ }
#[test] fn usage_errors_exit_2() { /* convert không có --to; --merge với --to png; resize không --percent/--max-edge */ }
#[test] fn json_output_is_a_single_parsable_object() { /* --json → serde_json parse, "ok":N,"failed":M, results[i].output là chuỗi hoặc null */ }
#[test] fn dry_run_prints_plan_and_creates_nothing() { /* thư mục chỉ còn đúng file đầu vào, exit 0, stdout chứa tên ra dự kiến */ }
#[test] fn strip_meta_compress_resize_produce_expected_names() { /* a-sach.jpg, a-nen.jpg (ảnh nhiễu), a-50x100.jpg */ }
#[test] fn incompressible_file_reports_cannot_shrink() { /* ảnh nhỏ → exit 1, stderr chứa "không nén thêm được", không có file -nen */ }
#[test] fn failures_are_written_to_the_log_file() { /* sau test "all failures", MORPHIX_DATA_DIR/morphix.log chứa tên file lỗi */ }
```
Viết đầy đủ thân từng test (mỗi test dưới 15 dòng) với ảnh sinh bằng `image` trong `tempdir`; assert đúng giá trị mã thoát ở trên.

- [ ] **Step 2: Run to verify fail**

Run: `cd morphix && cargo test -p morphix-cli`
Expected: FAIL.

- [ ] **Step 3: Implement** `main.rs` (clap, dựng `Job`, gọi `run_job`, ánh xạ `JobReport` sang mã thoát) và `output.rs` (in người đọc và JSON bằng `serde_json`). Một `CancelToken` cho cả lệnh; chưa nối Ctrl+C (ngoài phạm vi).

- [ ] **Step 4: Run to verify pass**

Run: `cd morphix && cargo test -p morphix-cli`
Expected: PASS (10 tests).

- [ ] **Step 5: Commit**

```bash
git add morphix/Cargo.toml morphix/crates/morphix-cli
git commit -m "feat(morphix-cli): convert, compress, resize, strip-meta with exit codes, json, dry-run"
```

---

### Task 11: CLI: formats và doctor

**Files:**
- Modify: `crates/morphix-cli/src/main.rs`, `src/output.rs`, `tests/cli.rs`; Modify: `morphix-core` thêm `pub fn find_ffmpeg() -> Option<PathBuf>` trong `src/paths.rs` (dùng crate `which`)

**Interfaces:**
- Consumes: `actions_for`, `detect`, `data_dir`, `find_ffmpeg`.
- Produces:
  - `morphix formats [FILES]...`: có file thì nhận dạng từng file bằng `detect`, gọi `actions_for` trên tập đó và in mỗi việc một dòng `convert<TAB><nhãn>` hoặc `tool<TAB><nhãn>`; không có file thì in các định dạng ảnh đầu ra hỗ trợ (`ImgFormat::ALL_ORDERED` cộng `PDF`), mỗi dòng một nhãn. Không có việc nào cho tập file thì in `không có việc nào làm được với tập file này` ra `stderr` và thoát 1. Hỗ trợ `--json`.
  - `morphix doctor`: in `ffmpeg: <đường dẫn>` hoặc `ffmpeg: không tìm thấy (cần cho video và âm thanh từ đợt 2)`, `thư mục dữ liệu: <đường dẫn> (ghi được|KHÔNG ghi được)`, `log: <đường dẫn>`. Thoát 0, trừ khi thư mục dữ liệu không ghi được thì thoát 1. Thiếu ffmpeg **không** làm lỗi.

- [ ] **Step 1: Write the failing tests**

```rust
#[test] fn formats_for_a_jpeg_lists_pdf_first_and_hides_jpg() { /* stdout dòng đầu "convert\tPDF", không có "JPG", có "tool\tNén ảnh" */ }
#[test] fn formats_for_mixed_selection_exits_1() { /* ảnh + file zip giả → exit 1, stderr có thông báo */ }
#[test] fn formats_without_files_lists_supported_targets() { /* stdout chứa PDF và AVIF */ }
#[test] fn doctor_reports_ffmpeg_and_data_dir_and_exits_0() { /* stdout chứa "ffmpeg:" và "thư mục dữ liệu:"; exit 0 dù thiếu ffmpeg (đặt PATH rỗng) */ }
#[test] fn doctor_exits_1_when_data_dir_is_not_writable() { /* MORPHIX_DATA_DIR trỏ vào một file thường */ }
```

- [ ] **Step 2: Run to verify fail**

Run: `cd morphix && cargo test -p morphix-cli formats doctor`
Expected: FAIL.

- [ ] **Step 3: Implement.** `doctor` kiểm ghi được bằng cách thử tạo và xoá một file tạm trong thư mục dữ liệu.

- [ ] **Step 4: Run to verify pass**

Run: `cd morphix && cargo test --workspace`
Expected: PASS (toàn bộ).

- [ ] **Step 5: Commit**

```bash
git add morphix/crates
git commit -m "feat(morphix-cli): formats and doctor commands"
```

---

### Task 12: Kiểm hoàn thành đợt 1a

**Files:** không tạo file mới; sửa lỗi phát hiện được ở đúng file chịu trách nhiệm.

- [ ] **Step 1: Chạy kiểm tra tĩnh**

Run: `cd morphix && cargo fmt --check && cargo clippy --workspace --all-targets -- -D warnings`
Expected: không lỗi, không cảnh báo. Sửa mọi cảnh báo ở nguồn, không dùng `#[allow]` trừ khi có comment giải thích tại sao.

- [ ] **Step 2: Chạy toàn bộ test**

Run: `cd morphix && cargo test --workspace`
Expected: tất cả PASS (khoảng 55 test: lõi và CLI).

- [ ] **Step 3: Thử thật bằng tay một vòng**

Run:
```bash
cd morphix && cargo build --release -p morphix-cli
mkdir -p /tmp/morphix-demo && cd /tmp/morphix-demo
# đặt vào đây 3 ảnh thật (một ảnh chụp điện thoại có hướng xoay, một PNG trong suốt)
MORPHIX_DATA_DIR=/tmp/morphix-demo/data /path/to/morphix/target/release/morphix convert *.jpg *.png --to pdf --merge
```
Expected: ra đúng một file `<tên ảnh đầu>-gop.pdf`, mở được bằng trình xem PDF, 3 trang đứng đúng chiều, PNG trong suốt nền trắng; ảnh gốc còn nguyên; `morphix doctor` chạy được; `data/morphix.log` tồn tại.

- [ ] **Step 4: Kiểm log và git**

Run: `cat /tmp/morphix-demo/data/morphix.log` rồi `git status --short morphix`
Expected: log không có lỗi bất ngờ (chỉ các lỗi chủ ý từ test thủ công); `git status` sạch cho `morphix/`.

- [ ] **Step 5: Commit nếu có sửa**

```bash
git add morphix
git commit -m "chore(morphix): pass fmt, clippy and full test suite for core and cli"
```

---

## Sau kế hoạch này

- **Kế hoạch 1b (giao diện Tauri):** khay hệ thống, cửa sổ vòng tròn (SVG theo wireframe), cửa sổ Khay, tiến trình nền, IPC `morphix menu`, gom lô 300 ms, lưu lịch sử JSON. Cần cài `libwebkit2gtk-4.1-dev` trên máy Linux dùng để làm (máy này chưa có). Các mục **[Chưa kiểm]** của spec (vị trí con trỏ trên Wayland, API `interprocess`, extension AppIndicator của GNOME) làm thử nghiệm nhỏ trước.
- **Kế hoạch 1c (tích hợp hệ điều hành):** mục menu `HKCU` với `MultiSelectModel=Player` (kiểm thực tế cách nhận danh sách file), Nautilus scripts, Dolphin service menu, `install-integration` và `uninstall-integration`, khởi động cùng máy.
