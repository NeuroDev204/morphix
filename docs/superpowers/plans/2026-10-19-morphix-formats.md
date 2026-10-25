# Morphix: mở rộng định dạng (đợt 2b, 2, 3, 4) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Thêm chuyển đổi và công cụ cho văn bản thuần, phụ đề, CSV/TSV (đợt 2b), âm thanh và video qua ffmpeg (đợt 2), PDF sang TXT (đợt 3), và file nén ZIP, TAR.GZ, 7Z (đợt 4), theo đúng phạm vi đã duyệt trong spec.

**Architecture:** Mọi việc vẫn đi qua `morphix-core`: nhận dạng loại trong `kind.rs`, việc có thể làm trong `catalog.rs`, thực thi trong `job.rs` qua `Operation`, và mỗi nhóm định dạng một module trong `ops/`. Văn bản và CSV làm bằng Rust thuần. Âm thanh và video chạy `ffmpeg` ngoài qua một lớp chạy duy nhất có huỷ và báo tiến trình. Giao diện và CLI không có logic riêng: chúng đọc danh sách việc từ `actions_for`.

**Tech Stack:** Rust (edition 2021) như workspace hiện có. Crate mới cho CSV (`csv`), nén (`zip`, `tar`, `flate2`, `sevenz-rust2`, đã liệt kê trong spec gốc mục 7), PDF (`pdfium-render`, đã có trong spec gốc mục 7). Chương trình ngoài: `ffmpeg`, lấy qua `find_ffmpeg()` đã có trong `paths.rs`.

**Spec:** `morphix/docs/superpowers/specs/2026-10-19-morphix-formats-design.md` (bổ sung), và `morphix/docs/superpowers/specs/2026-10-19-morphix-design.md` (gốc: mục 2, 5.4, 7, 9).

## Global Constraints

- Offline tuyệt đối; không tải về bất kỳ thứ gì khi chạy.
- Không bao giờ ghi đè file gốc. Đầu ra cạnh file gốc, hậu tố theo spec gốc mục 5.4; trùng tên thì thêm ` (1)`, ` (2)`.
- Ghi vào file tạm cùng thư mục rồi đổi tên; huỷ hoặc lỗi thì xoá file tạm.
- Mọi lỗi hiện cho người dùng bằng câu nói được nguyên nhân (tiếng Việt), và ghi `tracing`. Không nuốt lỗi.
- Lỗi một file trong lô không làm hỏng cả lô; kết quả báo từng file.
- Không cần quyền quản trị để cài hoặc dùng.
- Đuôi định dạng và chữ hiển thị đi qua một nguồn duy nhất: bảng trong `catalog.rs` và `ui/src/strings.ts`.
- Mỗi thư viện hoặc chương trình đi kèm phải có Ruling ghi giấy phép trước khi phát hành.
- `morphix-cli` không phụ thuộc `morphix-gui`, Tauri, GTK hay WebKit.

## Review Focus

1. **CSV có tiếng Việt, dấu ngoặc kép, ô có xuống dòng**: chuyển qua lại không mất ký tự (Task 2).
2. **SRT có nhiều dòng trong một đoạn, BOM đầu file, thiếu dòng trống cuối**: parse và ghi lại đúng (Task 2).
3. **ZIP hoặc TAR có đường dẫn `../` hoặc tuyệt đối**: từ chối giải nén file đó, không ghi ra ngoài thư mục đích (Task 13).
4. **Video không có âm thanh khi tách âm thanh**: báo lỗi rõ, không tạo file rỗng (Task 8).
5. **Huỷ giữa chừng ffmpeg hoặc giải nén**: không để file tạm, và thư mục giải nén đã tạo được xoá (Task 6, Task 13).

## Quyết định đã chốt trong plan (Mình chọn, cần người dùng xác nhận)

- **TSV:** ô chứa tab hoặc xuống dòng không biểu diễn được, nên lỗi kèm số dòng. CSV chứa xuống dòng trong ô vẫn chuyển sang CSV được.
- **CSV:** dấu phân cách là dấu phẩy. File dùng dấu chấm phẩy báo lỗi kèm gợi ý. Đầu ra CSV có BOM UTF-8 để Excel đọc tiếng Việt đúng.
- **Giải nén:** thư mục đích đặt theo tên file nén (bỏ đuôi). Nếu đã tồn tại thì thêm ` (1)`. Giải nén vào thư mục tạm cùng cấp rồi đổi tên khi xong; lỗi thì xoá thư mục tạm.
- **RAR:** chỉ đọc, làm **sau** đợt 4 khi đã kiểm giấy phép. Không nằm trong đợt 4.
- **AVI:** là đầu ra của video. Một bảng định nghĩa dùng chung cho menu và CLI.
- **Cắt đoạn, đổi tốc độ, tách âm thanh (đợt 2):** chỉ có trong CLI ở bản này. Giao diện vòng tròn chỉ hiện chuyển định dạng và nén.

## Điểm chặn

- **R1 (đã chốt: ffmpeg GPL có `libx264`):** các task đánh dấu **[Chờ R1]** được mở. Encoder H.264 lấy `libx264`; WebM dùng `libvpx-vp9` và `libopus`. Trên Linux, bản đóng gói và bản hệ thống cùng cấu hình.

## File structure

```
morphix/crates/morphix-core/src/
  kind.rs                 thêm TextFormat, MediaFormat, ArchiveFormat và nhận dạng văn bản
  catalog.rs              thêm ActionKind cho văn bản, media, archive
  job.rs                  thêm Operation cho từng nhóm
  ops/text.rs             SRT, VTT, TXT, MD, CSV, TSV: đọc, ghi, chuyển (đợt 2b)
  ops/ffmpeg.rs           chạy ffmpeg: argv, tiến trình, huỷ (đợt 2)
  ops/media.rs            bảng encoder, dựng argv, chuyển, nén, cắt, đổi tốc độ, tách âm thanh (đợt 2)
  ops/pdf_text.rs         PDF sang TXT qua pdfium (đợt 3)
  ops/archive.rs          giải nén và nén ZIP, TAR.GZ, 7Z (đợt 4)
morphix/crates/morphix-core/tests/
  text.rs  ffmpeg.rs  media.rs  pdf_text.rs  archive.rs
morphix/crates/morphix-cli/src/main.rs      convert nhận định dạng mới; lệnh formats liệt kê
morphix/crates/morphix-gui/src/state.rs     kind_label cho văn bản, media, archive
```

Thứ tự thực hiện: đợt 2b trước (không bị chặn), rồi đợt 2, đợt 3, đợt 4.

---

## Đợt 2b: văn bản, phụ đề, CSV (không phụ thuộc ffmpeg)

### Task 1: Nhận dạng văn bản, phụ đề, bảng

**Files:**
- Modify: `crates/morphix-core/src/kind.rs`
- Test: `crates/morphix-core/src/kind.rs` (module `tests`, đã có)

**Interfaces:**
- Produces:
  - `#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)] pub enum TextFormat { Txt, Markdown, Srt, Vtt, Csv, Tsv }`, với `fn extension(self) -> &'static str` và `fn from_extension(ext: &str) -> Option<TextFormat>`.
  - `FileKind::Text(TextFormat)` thêm vào enum hiện có.

- [ ] **Step 1: Viết test đỏ**

Thêm vào module `tests` của `kind.rs`:
- file `.srt` UTF-8 có BOM trả `FileKind::Text(TextFormat::Srt)`;
- file `.csv` có tiếng Việt trả `FileKind::Text(TextFormat::Csv)`;
- file `.bin` không phải UTF-8 trả `FileKind::Unknown`;
- file PNG đặt đuôi `.txt` vẫn trả `FileKind::Image(ImgFormat::Png)`.

Run: `cargo test -p morphix-core kind`. Expected: FAIL vì chưa có `TextFormat`.

- [ ] **Step 2: Triển khai trong `detect`**

Văn bản không có chữ ký nhị phân, nên sau khi `infer` trả `Unknown`, nếu đuôi thuộc `TextFormat::from_extension` và 8 KB đầu là UTF-8 hợp lệ (bỏ BOM nếu có), trả `Text(format)`. **WHY:** đuôi không đủ tin cậy nên vẫn kiểm nội dung; file nhị phân có đuôi `.txt` không được coi là văn bản.

- [ ] **Step 3: Chạy test để xác nhận đạt**

Run: `cargo test -p morphix-core kind`. Expected: pass.

- [ ] **Step 4: Commit**

```bash
git add morphix/crates/morphix-core/src/kind.rs
git commit -m "feat(morphix-core): detect text, subtitle and table files"
```

### Task 2: Đọc và ghi văn bản, phụ đề, CSV/TSV

**Files:**
- Create: `crates/morphix-core/src/ops/text.rs`
- Modify: `crates/morphix-core/src/ops/mod.rs` (thêm `pub mod text;`)
- Test: `crates/morphix-core/tests/text.rs`

**Interfaces:**
- Consumes: `kind::TextFormat`, `error::{MorphixError, Result}`.
- Produces:
  - `pub struct Cue { pub start_ms: u64, pub end_ms: u64, pub text: String }`
  - `pub fn read_cues(path: &Path) -> Result<Vec<Cue>>`: SRT và VTT; VTT bỏ header `WEBVTT` và khối `NOTE`.
  - `pub fn write_cues(cues: &[Cue], format: TextFormat, out: &mut impl Write) -> Result<()>`: chỉ `Srt` và `Vtt`.
  - `pub fn strip_timestamps(cues: &[Cue]) -> String`: nội dung cho TXT.
  - `pub fn read_table(path: &Path, format: TextFormat) -> Result<Vec<Vec<String>>>`: `Csv` và `Tsv`.
  - `pub fn write_table(rows: &[Vec<String>], format: TextFormat, out: &mut impl Write) -> Result<()>`: `Csv` ghi BOM ở đầu; `Tsv` lỗi `InvalidInput` kèm số dòng nếu ô chứa tab hoặc xuống dòng.

- [ ] **Step 1: Viết test đỏ** (`tests/text.rs`)

- SRT có BOM và đoạn nhiều dòng: đọc ra đúng `start_ms`, `text` giữ xuống dòng giữa hai dòng; ghi lại SRT không đổi nội dung.
- SRT sang VTT giữ mốc thời gian, dấu `,` trong mốc đổi thành `.`.
- `strip_timestamps` chỉ còn văn bản, không có số thứ tự và mốc.
- CSV `"Nguyễn, Văn A","ghi chú ""có ngoặc"""` và một ô có xuống dòng: đọc ra đúng; ghi lại rồi đọc lại cho cùng bảng.
- CSV có dấu chấm phẩy (`a;b`) trả lỗi có chữ "dấu phẩy".
- TSV có ô chứa tab trả lỗi có số dòng.
- CSV ghi ra có BOM `EF BB BF` ở đầu.

Run: `cargo test -p morphix-core --test text`. Expected: FAIL vì chưa có module.

- [ ] **Step 2: Triển khai** các chữ ký ở trên. Dùng crate `csv` cho CSV; kiểm phiên bản trên crates.io trước khi thêm và ghi vào Ruling. SRT và VTT tự parse vì định dạng đơn giản. File không phải UTF-8 trả `MorphixError::Corrupt` kèm lý do.

- [ ] **Step 3: Chạy test để xác nhận đạt**

Run: `cargo test -p morphix-core --test text`. Expected: pass.

- [ ] **Step 4: Commit**

```bash
git add morphix/crates/morphix-core/src/ops/text.rs morphix/crates/morphix-core/src/ops/mod.rs morphix/crates/morphix-core/tests/text.rs morphix/crates/morphix-core/Cargo.toml morphix/Cargo.lock
git commit -m "feat(morphix-core): read and write subtitles, csv and tsv"
```

### Task 3: Chuyển văn bản trong job và catalog

**Files:**
- Modify: `crates/morphix-core/src/catalog.rs`, `crates/morphix-core/src/job.rs`
- Test: `crates/morphix-core/tests/text.rs` (thêm test job)

**Interfaces:**
- Consumes: `ops::text::*` (Task 2), `kind::{FileKind, TextFormat}` (Task 1), `naming::write_output`.
- Produces:
  - `ActionKind::ConvertText(TextFormat)`, `Operation::ConvertText { to: TextFormat }`.
  - `actions_for` thêm việc văn bản khi **mọi** file thuộc cùng một nhóm: nhóm văn bản (TXT, MD), nhóm phụ đề (SRT, VTT, TXT), nhóm bảng (CSV, TSV). Không trộn nhóm.

- [ ] **Step 1: Viết test đỏ**: `actions_for` với một file SRT trả đúng SRT→VTT và SRT→TXT; tập lẫn SRT và CSV trả rỗng; job `ConvertText { to: Txt }` trên file SRT ra file `.txt` cạnh file gốc, tên theo `naming::desired_output`, file gốc không đổi.

- [ ] **Step 2: Chạy test đỏ** và xác nhận lỗi vì thiếu biến thể.

- [ ] **Step 3: Triển khai** nhánh `Operation::ConvertText` trong `run_item`, gọi `ops::text` rồi `write_output`. Thêm vào `planned_outputs` cùng quy tắc đặt tên.

- [ ] **Step 4: Chạy test** `cargo test -p morphix-core`. Expected: pass.

- [ ] **Step 5: Commit**

```bash
git add morphix/crates/morphix-core/src/catalog.rs morphix/crates/morphix-core/src/job.rs morphix/crates/morphix-core/tests/text.rs
git commit -m "feat(morphix-core): run text conversions through jobs and catalog"
```

### Task 4: CLI và giao diện nhận định dạng văn bản

**Files:**
- Modify: `crates/morphix-cli/src/main.rs`, `crates/morphix-gui/src/state.rs` (`kind_label`), `crates/morphix-gui/ui/src/strings.ts`
- Test: `crates/morphix-cli/tests/cli.rs`, `crates/morphix-gui/tests/state.rs`

**Interfaces:**
- Consumes: `actions_for`, `Operation::ConvertText`.
- Produces: `morphix convert <file> --to txt|md|srt|vtt|csv|tsv`; `morphix formats` liệt kê nhóm văn bản.

- [ ] **Step 1: Viết test đỏ**: `morphix convert sub.srt --to vtt` ra `sub.vtt`; `morphix convert a.csv --to tsv` thoát mã 1 khi có tab trong ô; `kind_label` của `FileKind::Text(Srt)` trả nhãn tiếng Việt, không phải "other".

- [ ] **Step 2: Chạy test đỏ** và xác nhận lỗi.

- [ ] **Step 3: Triển khai.** Thêm nhãn vào `strings.ts`; `kind_label` ánh xạ `Text` theo định dạng.

- [ ] **Step 4: Chạy** `cargo test --workspace` và `cd crates/morphix-gui/ui && npx vitest run && npm run build`. Expected: pass.

- [ ] **Step 5: Commit**

```bash
git add morphix/crates/morphix-cli morphix/crates/morphix-gui
git commit -m "feat(morphix): expose text, subtitle and table conversions in cli and gui"
```

---

## Đợt 2: âm thanh và video qua ffmpeg

### Task 5: Nhận dạng âm thanh và video theo định dạng cụ thể

**Files:**
- Modify: `crates/morphix-core/src/kind.rs`
- Test: `crates/morphix-core/src/kind.rs` (module `tests`)

**Interfaces:**
- Produces:
  - `#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)] pub enum MediaFormat { Mp3, M4a, Wav, Flac, Ogg, Opus, Aiff, Wma, Mp4, Mov, Mkv, WebM, Avi, Wmv, Gif }`, với `extension()`, `label()`, `is_audio()`.
  - `FileKind::Media(MediaFormat)` thay thế `Video` và `Audio` của đợt 1. Mọi nơi dùng hai biến thể cũ được sửa trong cùng commit.

- [ ] **Step 1: Viết test đỏ**: header `ftyp` của MP4 trả `Media(Mp4)`; header WAV trả `Media(Wav)`; MIME audio không có trong bảng trả `Unknown`.

- [ ] **Step 2: Chạy test đỏ** và xác nhận lỗi.

- [ ] **Step 3: Triển khai** ánh xạ MIME của `infer` sang `MediaFormat`. MIME không có trong bảng trả `Unknown`, không đoán.

- [ ] **Step 4: Chạy** `cargo test -p morphix-core kind`. Expected: pass.

- [ ] **Step 5: Commit**

```bash
git add morphix/crates/morphix-core/src/kind.rs
git commit -m "feat(morphix-core): identify audio and video formats"
```

### Task 6: Lớp chạy ffmpeg có huỷ và tiến trình

**Files:**
- Create: `crates/morphix-core/src/ops/ffmpeg.rs`
- Modify: `crates/morphix-core/src/ops/mod.rs`
- Test: `crates/morphix-core/tests/ffmpeg.rs`

**Interfaces:**
- Consumes: `paths::find_ffmpeg`, `cancel::CancelToken`, `error::{MorphixError, Result}`.
- Produces:
  - `pub fn run_ffmpeg(ffmpeg: &Path, args: &[OsString], cancel: &CancelToken, on_time_ms: &mut dyn FnMut(u64)) -> Result<()>`
  - `pub fn parse_time_ms(stderr_line: &str) -> Option<u64>`: đọc `time=HH:MM:SS.xx` của ffmpeg.

- [ ] **Step 1: Viết test đỏ** dùng một script giả thay ffmpeg, đặt trong thư mục tạm (chỉ unix; test Windows đánh dấu pending):
  - script in `time=00:00:02.50` rồi thoát 0: `run_ffmpeg` gọi `on_time_ms(2500)` và trả `Ok`;
  - script thoát 1 và in `Invalid data`: trả `Corrupt` có dòng lỗi cuối;
  - huỷ ngay khi bắt đầu: trả `Cancelled`, tiến trình con được dừng và thu hồi (không zombie);
  - `parse_time_ms("time=00:01:02.50")` trả `Some(62500)`.

- [ ] **Step 2: Chạy test đỏ** và xác nhận lỗi.

- [ ] **Step 3: Triển khai.** Spawn bằng `Command` với argv, không dùng shell. Đọc stderr trên luồng riêng để không treo khi bộ đệm đầy. Vòng `try_wait` kiểm `CancelToken` mỗi 50 ms; huỷ thì `kill` rồi `wait`. **WHY:** ffmpeg ghi tiến trình ra stderr; đọc đồng bộ sẽ treo khi bộ đệm đầy.

- [ ] **Step 4: Chạy** `cargo test -p morphix-core --test ffmpeg`. Expected: pass.

- [ ] **Step 5: Commit**

```bash
git add morphix/crates/morphix-core/src/ops/ffmpeg.rs morphix/crates/morphix-core/src/ops/mod.rs morphix/crates/morphix-core/tests/ffmpeg.rs
git commit -m "feat(morphix-core): run ffmpeg with progress and cancel"
```

### Task 7: Bảng encoder và dựng argv

**Files:**
- Create: `crates/morphix-core/src/ops/media.rs` (chỉ bảng và hàm dựng argv)
- Test: `crates/morphix-core/tests/media.rs`

**Interfaces:**
- Consumes: `kind::MediaFormat`.
- Produces:
  - `pub fn encoder_for(target: MediaFormat) -> Option<&'static [&'static str]>`: argv encoder. Giá trị theo bảng đã chốt (R1: ffmpeg GPL có libx264); WMA và WMV không có encoder.
  - `pub fn convert_args(input: &Path, target: MediaFormat, output: &Path) -> Option<Vec<OsString>>`: `None` nếu không có encoder cho đích.

- [ ] **Step 1: Viết test đỏ**:
  - WAV sang FLAC: argv có `-c:a flac`. Test này không phụ thuộc R1.
  - MP4: argv có `libx264` và `+faststart`; WMV và WMA: `encoder_for` và `convert_args` trả `None`.

- [ ] **Step 2–4: Chạy đỏ, triển khai, chạy đạt.**

- [ ] **Step 5: Commit** `feat(morphix-core): map media targets to ffmpeg encoders`

### Task 8: Operations cho âm thanh và video

**Files:**
- Modify: `crates/morphix-core/src/job.rs`, `crates/morphix-core/src/catalog.rs`, `crates/morphix-core/src/ops/media.rs`
- Test: `crates/morphix-core/tests/media.rs`

**Interfaces:**
- Consumes: `run_ffmpeg`, `convert_args`, `MediaFormat`.
- Produces:
  - `Operation::ConvertMedia { to: MediaFormat }`, `Operation::CompressMedia`, `Operation::CutMedia { start_ms: u64, end_ms: u64 }`, `Operation::ChangeSpeed { factor_milli: u32 }`, `Operation::ExtractAudio { to: MediaFormat }`.
  - `ActionKind` tương ứng. `actions_for` cho video liệt kê: MP4, MKV, WebM, MOV, AVI, GIF, MP3; cho âm thanh: MP3, M4A, WAV, FLAC, OGG, Opus.

- [ ] **Step 1: Viết test đỏ** với script ffmpeg giả ghi lại argv:
  - tách âm thanh từ file không có luồng âm thanh: lỗi "không có âm thanh để tách", không tạo file;
  - cắt đoạn 2 s đến 5 s: argv có `-ss` và `-to` đúng giá trị;
  - đổi tốc độ `factor_milli = 1500`: argv có `atempo=1.5` khi có âm thanh;
  - đích không có encoder: lỗi rõ, không gọi ffmpeg.

- [ ] **Step 2–4: Chạy đỏ, triển khai, chạy đạt.**

- [ ] **Step 5: Commit** `feat(morphix-core): convert, compress, cut, speed and extract media`

### Task 9: CLI và giao diện cho âm thanh và video

**Files:**
- Modify: `crates/morphix-cli/src/main.rs`, `crates/morphix-gui/src/state.rs`, `crates/morphix-gui/ui/src/strings.ts`
- Test: `crates/morphix-cli/tests/cli.rs`, `crates/morphix-gui/tests/state.rs`

**Interfaces:**
- Consumes: `actions_for`, các `Operation` của Task 8.
- Produces: `morphix convert <file> --to mp4 …`, `morphix cut <file> --from 2.0 --to 5.0`, `morphix speed <file> --factor 1.5`, `morphix extract-audio <file> --to mp3`. Giao diện chỉ hiện chuyển định dạng và nén.

- [ ] **Step 1: Viết test đỏ**: CLI trả mã 2 khi thiếu `--to`; giao diện không hiện nhãn "Cắt đoạn".
- [ ] **Step 2–4: Chạy đỏ, triển khai, chạy đạt.**
- [ ] **Step 5: Commit** `feat(morphix): media commands in cli and gui menu`

---

## Đợt 3: PDF sang TXT

### Task 10: Đọc lớp văn bản PDF

**Files:**
- Create: `crates/morphix-core/src/ops/pdf_text.rs`
- Test: `crates/morphix-core/tests/pdf_text.rs`

**Interfaces:**
- Consumes: `pdfium-render` (đã có trong spec gốc mục 7; kiểm phiên bản và giấy phép trước khi thêm, ghi Ruling), `MorphixError`.
- Produces: `pub fn pdf_to_text(path: &Path, out: &mut impl Write) -> Result<()>`. Nếu một trang không có lớp văn bản, trả `Corrupt` với thông điệp "trang N không có chữ có thể chọn, cần OCR, chưa hỗ trợ"; không tạo file rỗng.

- [ ] **Step 1: Viết test đỏ**: PDF có chữ tiếng Việt ra đúng chuỗi; PDF một trang trống báo lỗi có "trang 1".

- [ ] **Step 2–4: Đỏ, triển khai, đạt.** Test cần thư viện PDFium đi kèm; nếu thiếu thư viện thì đánh dấu `#[ignore]` kèm lý do, không giả PASS.

- [ ] **Step 5: Commit** `feat(morphix-core): extract text layer from pdf`

### Task 11: PDF sang TXT trong job và catalog

**Files:**
- Modify: `crates/morphix-core/src/catalog.rs`, `crates/morphix-core/src/job.rs`, `crates/morphix-cli/src/main.rs`
- Test: `crates/morphix-core/tests/pdf_text.rs`

**Interfaces:**
- Produces: `ActionKind::PdfToText`, `Operation::PdfToText`. `actions_for` thêm việc này khi file là PDF. **Mình chọn:** không thêm PDF sang MD, EPUB hay Office.

- [ ] **Step 1–5:** test đỏ (PDF ra `.txt` cạnh file gốc, file gốc không đổi), triển khai, đạt, commit `feat(morphix-core): pdf to text conversion`.

---

## Đợt 4: file nén

### Task 12: Nhận dạng file nén

**Files:**
- Modify: `crates/morphix-core/src/kind.rs`
- Test: `crates/morphix-core/src/kind.rs`

**Interfaces:**
- Produces: `#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)] pub enum ArchiveFormat { Zip, TarGz, SevenZ, Rar }`; `FileKind::Archive(ArchiveFormat)`.

- [ ] **Step 1–5:** test đỏ (ZIP, gzip chứa tar nhận là `TarGz`, 7z nhận là `SevenZ`, RAR nhận là `Rar`), triển khai qua MIME của `infer`, đạt, commit `feat(morphix-core): identify archive formats`.

### Task 13: Giải nén an toàn

**Files:**
- Create: `crates/morphix-core/src/ops/archive.rs`
- Test: `crates/morphix-core/tests/archive.rs`

**Interfaces:**
- Consumes: `zip`, `tar`, `flate2`, `sevenz-rust2` (kiểm phiên bản, ghi Ruling), `naming`.
- Produces: `pub fn extract(archive: &Path, format: ArchiveFormat, out_dir: Option<&Path>, cancel: &CancelToken) -> Result<PathBuf>`, trả thư mục đích.

- [ ] **Step 1: Viết test đỏ**:
  - ZIP có `a/b.txt`: giải nén ra `<tên file>/a/b.txt`; đã có thư mục cùng tên thì dùng `<tên> (1)`;
  - ZIP có entry `../evil.txt` hoặc đường dẫn tuyệt đối: lỗi, không tạo file ngoài thư mục đích, thư mục tạm bị xoá;
  - huỷ giữa chừng: không còn thư mục tạm.

- [ ] **Step 2–4:** đỏ, triển khai (chuẩn hoá mọi đường dẫn entry và kiểm nó nằm dưới thư mục đích trước khi ghi), đạt.

- [ ] **Step 5: Commit** `feat(morphix-core): extract zip, tar.gz and 7z safely`

### Task 14: Nén thành ZIP, TAR.GZ, 7Z

**Files:**
- Modify: `crates/morphix-core/src/ops/archive.rs`, `crates/morphix-core/src/job.rs`, `crates/morphix-core/src/catalog.rs`
- Test: `crates/morphix-core/tests/archive.rs`

**Interfaces:**
- Produces: `Operation::CreateArchive { format: ArchiveFormat }` (gộp nhiều file thành một file nén, tên theo file đầu tiên và `-nen` như spec gốc mục 5.4) và `Operation::ExtractArchive`.

- [ ] **Step 1–5:** test đỏ (ZIP tạo ra đọc lại được; file gốc không đổi; tên đầu ra đúng), triển khai, đạt, commit `feat(morphix-core): create zip, tar.gz and 7z archives`.

### Task 15: CLI và giao diện cho file nén

**Files:**
- Modify: `crates/morphix-cli/src/main.rs`, `crates/morphix-gui/src/state.rs`, `crates/morphix-gui/ui/src/strings.ts`
- Test: `crates/morphix-cli/tests/cli.rs`, `crates/morphix-gui/tests/state.rs`

**Interfaces:**
- Produces: `morphix extract <file>`, `morphix archive <files…> --to zip|tar.gz|7z`. Menu chuột phải hiện "Giải nén" khi file là file nén.

- [ ] **Step 1–5:** test đỏ, triển khai, đạt, commit `feat(morphix): archive commands in cli and gui menu`.

### Task 16: Kiểm tra cuối các đợt 2b, 2, 3, 4

**Files:** không sửa mã; ghi kết quả vào plan và Ruling.

- [ ] **Step 1:** `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, `cd crates/morphix-gui/ui && npx vitest run && npm run build`. Expected: tất cả pass, trừ các test đánh dấu ignored có lý do.
- [ ] **Step 2:** chạy thật trên Linux: SRT sang VTT, CSV có tiếng Việt sang TSV và CSV, PDF sang TXT, ZIP giải nén, và khi R1 đã chốt thì MP4 sang WebM. Ghi lại file đầu ra và kích thước.
- [ ] **Step 3:** ghi vào Ruling giấy phép từng thư viện mới, và các mục chưa chạy trên Windows.
- [ ] **Step 4: Commit** `docs(morphix): record formats rulings and checks`.

---

## Đóng gói Linux (`.deb` tự chứa)

### Task 17: Gói `.deb` có ffmpeg GPL và thư viện đi kèm [Đã chốt: `.deb`, sudo, ffmpeg GPL]

**Files:**
- Create: `morphix/crates/morphix-gui/packaging/deb/` (cấu hình bổ sung cho Tauri bundler)
- Create: `morphix/scripts/build-ffmpeg-sidecar.sh` (tải hoặc sao chép binary ffmpeg và các `.so` của nó vào thư mục đóng gói)
- Test: `morphix/scripts/check-deb.sh` (kiểm bằng `dpkg-deb -c` và `ldd`)

**Interfaces:**
- Consumes: `tauri build` (bundler `deb`), `find_ffmpeg()` đã có trong `paths.rs`.
- Produces: một file `.deb` đặt `morphix`, `morphix-gui`, `ffmpeg` và các thư viện đi kèm trong `/usr/lib/morphix/` và `/usr/bin/morphix`. Binary đi kèm tìm thư viện qua RPATH `$ORIGIN`.

- [ ] **Step 1: Kiểm tra trước khi đóng gói.** `dpkg-deb -c` liệt kê đúng các file; `ldd` của `ffmpeg` đóng gói chỉ trỏ vào `/usr/lib/morphix/` hoặc thư viện hệ thống có trong `Depends`.
- [ ] **Step 2: Khai báo phụ thuộc.** `Depends` chỉ gồm thư viện hệ thống (WebKitGTK 4.1, AppIndicator) mà Tauri đã khai báo. Không đóng gói WebKitGTK.
- [ ] **Step 3: Build.** Build trên Ubuntu 22.04 (glibc thấp nhất cần hỗ trợ). Ghi phiên bản glibc tối đa vào Ruling.
- [ ] **Step 4: Kiểm trên máy sạch.** Cài `.deb` trong container Ubuntu 22.04 và 24.04 không có ffmpeg: `morphix formats` và `morphix convert` chạy được; `morphix doctor` thấy ffmpeg đi kèm.
- [ ] **Step 5: Ghi giấy phép** của từng thư viện đóng gói vào Ruling trước khi phát hành.

**Đã chốt (người dùng duyệt):**
- Gói `.deb`, cần sudo để cài. Spec mở rộng mục 2a ghi thay đổi này.
- ffmpeg đóng gói là bản GPL có `libx264`; nguồn là gói `ffmpeg` 7:6.1.1-3ubuntu5 của Ubuntu. Phải cung cấp mã nguồn tương ứng khi phân phối.

## Ruling ledger

- **Ruling:** TSV từ chối ô có tab hoặc xuống dòng, lỗi kèm số dòng, không cắt âm thầm. **Mình chọn**.
- **Ruling:** CSV dùng dấu phẩy; BOM UTF-8 ở đầu CSV đầu ra. **Mình chọn**.
- **Ruling:** giải nén vào thư mục theo tên file, làm tạm rồi đổi tên. **Mình chọn**.
- **Ruling:** RAR đọc để sau đợt 4, chờ kiểm giấy phép. **Mình chọn**.
- **Ruling (chờ R1):** bản ffmpeg GPL hay LGPL và bộ encoder. Phần encoder của đợt 2 bị chặn đến khi người dùng quyết định.
- **Ruling:** cắt đoạn, đổi tốc độ, tách âm thanh chỉ có trong CLI ở đợt 2. **Mình chọn**.
- **Ruling:** PDF sang TXT đọc lớp văn bản qua `pdfium-render` 0.9.4 (giấy phép MIT OR Apache-2.0, kiểm trên crates.io), tính năng `pdfium_latest` = PDFium `chromium/7881` (VERSION ghi 151.0.7881.0 trong bản tải). Tải từ bblanchon/pdfium-binaries, release tag `chromium/7881`, tệp `pdfium-linux-x64.tgz`, sha256 `1470e21b8b4a3b4ad7f85684e2da11d94f3b69a86d81dee11b9b6709d927ac1d`; `libpdfium.so` sha256 `f728930966f503652b92acc89b9374a2eeca00ce42e26dccd3e4b5c5161b2d64`. Giấy phép PDFium là BSD kiểu 3 điều khoản (The PDFium Authors); bản dựng pdfium-binaries là MIT (Benoit Blanchon); các thư viện con (FreeType, ICU, libpng, libjpeg-turbo, zlib, OpenJPEG, LCMS, v.v.) đi kèm văn bản giấy phép trong `licenses/` của bản tải. Chỉ dùng cho phát triển và kiểm thử, đặt ở `morphix/third_party/pdfium/` (đã thêm vào `.gitignore`), không commit. Khi đóng gói phải kèm thư viện và toàn bộ văn bản giấy phép này. **Mình chọn.**
