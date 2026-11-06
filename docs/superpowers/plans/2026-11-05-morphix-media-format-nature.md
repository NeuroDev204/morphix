# Morphix: video và âm thanh đúng bản chất định dạng — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Convert media chép nguyên luồng khi container đích nhận được codec nguồn, không đẩy bitrate âm thanh lên cao hơn nguồn, và cảnh báo khi lossy sang lossless hoặc khi file ra không nhỏ hơn.

**Architecture:** `probe` đọc codec/bitrate từ stderr của `ffmpeg -i` qua hàm thuần `parse_streams`. `convert_args_with` dựng argv theo từng luồng (chép hoặc mã hoá) từ `MediaInfo`; `convert_args` cũ gọi nó với `MediaInfo::default()` nên mọi đường khác (nén, cắt, tốc độ, tách âm thanh) giữ nguyên argv. Cảnh báo media được tính bằng hàm thuần `media_warnings` và đưa vào `ItemReport.warnings` qua `run_item`.

**Tech Stack:** Rust 2021, ffmpeg (đã có trên máy: 9.0.2), `tempfile`.

**Spec:** `docs/superpowers/specs/2026-11-05-morphix-media-format-nature-design.md`

## Global Constraints

- Không ghi đè hay mất file nguồn; đầu ra đi qua `write_output_path`/`desired_output` như hiện nay.
- Lỗi và cảnh báo phải hiện cho người dùng. Lỗi `probe` trong lúc convert **không** làm hỏng việc: ghi `tracing::warn!` kèm đường dẫn và nguyên nhân, rồi dùng `MediaInfo::default()` (mã hoá như hiện nay). Nếu file thật sự hỏng, bước mã hoá sẽ báo lỗi như cũ.
- `convert_media`, `compress_media`, `cut_media`, `change_speed`, `extract_audio` giữ nguyên chữ ký và argv hiện có. Các test `#[cfg(unix)]` trong `crates/morphix-core/tests/media.rs` (ffmpeg giả, không chạy được trên Windows) phải tiếp tục đúng: đọc chúng khi sửa và ghi ruling nếu một test giả định ffmpeg chỉ được gọi một lần.
- Comment là ghi chú `WHY` tiếng Việt; chuỗi người dùng thấy bằng tiếng Việt.
- Máy chưa có danh tính git: commit thất bại vì thiếu `user.name`/`user.email` thì bỏ qua và ghi ruling.
- Chỉ `rustfmt` các file đã động tới. Trước Task 1, ghi danh sách test đang fail của `cargo test --workspace --no-fail-fast` làm mốc.

## Review Focus

- File có nhiều luồng audio hoặc video (ví dụ MKV hai ngôn ngữ): chỉ luồng đầu tiên được đọc codec; ffmpeg mặc định cũng chỉ lấy một luồng mỗi loại, nên quyết định chép phải dựa đúng luồng đó — Task 1 (test mẫu hai dòng `Audio:`, lấy dòng đầu).
- Ảnh bìa trong MP3/M4A (`Video: png ... (attached pic)`) không được coi là luồng video; MP3 có bìa sang M4A phải chép/mã hoá audio bình thường — Task 1, Task 2.
- Bitrate nguồn lạ (`0 kb/s`, thiếu `kb/s`) không được sinh `-b:a 0k` — Task 2.
- Video H.264 yuv444 (thấy thật trong mẫu MKV) sang MP4 được chép nguyên; đây là đánh đổi spec đã nêu, không phải lỗi — ghi trong Rủi ro của spec, không test.
- Lỗi `probe` (ffmpeg không in dòng luồng) không làm việc convert thất bại sớm hơn hiện nay — Task 3.

## Mẫu stderr thật (ffmpeg 9.0.2, dùng nguyên văn trong test)

```text
MP4  Stream #0:0[0x1](und): Video: h264 (High) (avc1 / 0x31637661), yuv420p(progressive), 160x120 [SAR 1:1 DAR 4:3], 35 kb/s, 10 fps, 10 tbr, 10240 tbn (default)
     Stream #0:1[0x2](und): Audio: aac (LC) (mp4a / 0x6134706D), 44100 Hz, mono, fltp, 116 kb/s (default)
MKV  Stream #0:0: Video: h264 (High 4:4:4 Predictive), yuv444p(tv, progressive), 160x120 [SAR 1:1 DAR 4:3], 10 fps, 10 tbr, 1k tbn
     Stream #0:1: Audio: opus, 48000 Hz, mono, fltp
WAV  Stream #0:0: Audio: pcm_s16le ([1][0][0][0] / 0x0001), 44100 Hz, mono, s16, 705 kb/s
MP3  Stream #0:0: Audio: mp3 (mp3float), 44100 Hz, mono, fltp, 128 kb/s, start 0.025057
     Stream #0:1: Video: png, rgb24(pc, gbr/unknown/unknown), 64x64 [SAR 1:1 DAR 1:1], 90k tbr, 90k tbn, start 0.025056 (attached pic)
```

---

### Task 1: Đọc codec và bitrate nguồn

**Files:**
- Modify: `crates/morphix-core/src/ops/media.rs` (thêm `MediaInfo`, `parse_streams`, `probe`; `has_audio_stream` dùng `probe`)
- Test: `crates/morphix-core/tests/media.rs` (phần không gate `unix`)

**Interfaces:**
- Produces: `#[derive(Debug, Clone, Default, PartialEq, Eq)] pub struct MediaInfo { pub video_codec: Option<String>, pub audio_codec: Option<String>, pub audio_kbps: Option<u32> }`.
- Produces: `pub fn parse_streams(stderr: &str) -> Option<MediaInfo>` — `None` khi không có dòng nào chứa `Stream #`. Codec là từ đầu tiên sau `Video: `/`Audio: ` với dấu `,` ở cuối bị cắt. Dòng video có `(attached pic)` bị bỏ qua. Chỉ lấy luồng đầu tiên mỗi loại. `audio_kbps` là số đứng ngay trước ` kb/s` trên dòng audio đó; `0` hoặc không có thì `None`.
- Produces: `pub fn probe(ffmpeg: &Path, input: &Path) -> Result<MediaInfo>` — chạy đúng lệnh `has_audio_stream` đang chạy; `parse_streams` trả `None` thì `Corrupt` với dòng stderr cuối không rỗng, như hiện nay.
- `has_audio_stream` trở thành `Ok(probe(ffmpeg, input)?.audio_codec.is_some())`.

- [ ] **Step 1: Viết test thất bại** (mẫu ở trên làm hằng `&str`): `parse_mp4_h264_aac` → `video_codec = Some("h264")`, `audio_codec = Some("aac")`, `audio_kbps = Some(116)`; `parse_mkv_opus_without_bitrate` → `Some("h264")`, `Some("opus")`, `None`; `parse_mp3_ignores_attached_cover` → `video_codec = None`, `audio_codec = Some("mp3")`, `audio_kbps = Some(128)`; `parse_wav_pcm` → `audio_codec = Some("pcm_s16le")`; `parse_takes_the_first_audio_stream` (dòng MP4 audio rồi thêm một dòng `Stream #0:2: Audio: ac3, 48000 Hz, 5.1, fltp, 384 kb/s` → `aac`, `116`); `parse_without_stream_lines_is_none` (`"a.xyz: Invalid data found when processing input"` → `None`).
- [ ] **Step 2: Chạy.** `cargo test -p morphix-core --test media -- parse_` → FAIL (chưa có `parse_streams`).
- [ ] **Step 3: Cài đặt** theo Interfaces.
- [ ] **Step 4: Chạy.** `cargo test -p morphix-core --test media` → PASS (test `unix` không chạy trên Windows; đọc lại các test gọi `has_audio_stream` để chắc thông điệp `Corrupt` không đổi).
- [ ] **Step 5: Commit** `feat(morphix-core): đọc codec và bitrate nguồn từ ffmpeg -i`.

---

### Task 2: Argv chép hay mã hoá theo từng luồng

**Files:**
- Modify: `crates/morphix-core/src/ops/media.rs` (`convert_args`, `output_args`, `encode_to`; thêm `convert_args_with`, bảng chép)
- Test: `crates/morphix-core/tests/media.rs`

**Interfaces:**
- Consumes: `MediaInfo` (Task 1).
- Produces: `pub fn convert_args_with(input: &Path, target: MediaFormat, output: &Path, info: &MediaInfo) -> Option<Vec<OsString>>`. `convert_args(input, target, output)` chỉ còn gọi `convert_args_with(..., &MediaInfo::default())`.
- Produces: `pub fn fully_remuxed(target: MediaFormat, info: &MediaInfo) -> bool` — `true` khi mọi luồng đích giữ lại đều được chép: đích video (không phải GIF) cần `video_codec` chép được và `audio_codec` là `None` hoặc chép được; đích audio cần `audio_codec` chép được; GIF, WAV, FLAC, AIFF luôn `false`; `MediaInfo::default()` luôn `false`.
- Produces: `pub fn is_lossy_audio(codec: &str) -> bool` — đúng cho `mp3`, `aac`, `opus`, `vorbis`, `ac3`, `wmav2`.
- `output_args` và `encode_to` thêm tham số `info: &MediaInfo` (đặt ngay sau `target`); `compress_media`, `cut_media`, `change_speed`, `extract_audio` truyền `&MediaInfo::default()`.
- Bảng codec chép được: đúng bảng ở spec mục 2.
- Thuật toán dựng argv (không suy được từ chữ ký): tách slice của `encoder_for(target)` thành ba phần theo tên tuỳ chọn, mỗi tuỳ chọn đi kèm đúng một giá trị — **video** `{-c:v, -preset, -crf, -pix_fmt, -b:v, -row-mt, -vf}`, **audio** `{-c:a, -b:a, -q:a}`, **khác** (ví dụ `-movflags +faststart`). Phần video thay bằng `-c:v copy` khi `video_codec` chép được cho đích; phần audio thay bằng `-c:a copy` khi `audio_codec` chép được. Khi audio phải mã hoá, nguồn `is_lossy_audio` và có `audio_kbps`, giá trị `-b:a` mặc định (dạng `Nk`) đổi thành `min(N, audio_kbps)k`. Thứ tự đầu ra: `stream_selection`, video, audio, khác. Không có phần video/audio nào (ví dụ GIF chỉ có `-vf`) thì giữ nguyên như hiện tại.

- [ ] **Step 1: Viết test thất bại** (dùng `argv_strings` đã có; `MediaInfo` dựng tay):
  - `mp4_to_mkv_copies_both_streams`: info `h264`/`aac`/`116` → argv có `-c:v copy` và `-c:a copy`, không có `libx264`; `fully_remuxed(Mkv, &info)`.
  - `mkv_opus_to_mp4_copies_video_and_encodes_audio`: `h264`/`opus`/`None` → `-c:v copy`, `-c:a aac`, `-b:a 128k`, còn `+faststart`; `!fully_remuxed`.
  - `mp4_to_webm_encodes_both_and_caps_audio_bitrate`: `h264`/`aac`/`116` → `libvpx-vp9`, `libopus`, `-b:a 116k`.
  - `video_with_mp3_audio_to_mp3_copies_audio`: `h264`/`mp3`/`128` sang `Mp3` → `-vn` và `-c:a copy`.
  - `mp3_128k_to_m4a_keeps_128k`: audio `mp3`/`128` sang `M4a` → `-c:a aac`, `-b:a 128k`.
  - `zero_or_missing_bitrate_keeps_the_default`: `audio_kbps = Some(0)` hoặc `None`, MP3 sang M4A → `-b:a 192k`.
  - `unknown_source_matches_the_old_argv`: với mọi `MediaFormat` có encoder, `convert_args_with(.., &MediaInfo::default()) == convert_args(..)`; test argv hiện có (`mp4_argv_uses_libx264_and_faststart`...) vẫn đạt.
  - `lossless_and_gif_targets_are_never_remuxed`: `fully_remuxed` cho `Wav`, `Flac`, `Aiff`, `Gif` là `false` kể cả khi codec trùng.
- [ ] **Step 2: Chạy.** `cargo test -p morphix-core --test media -- copies encodes keeps remuxed old_argv` → FAIL.
- [ ] **Step 3: Cài đặt** theo Interfaces.
- [ ] **Step 4: Chạy.** `cargo test -p morphix-core --test media` → PASS.
- [ ] **Step 5: Commit** `feat(morphix-core): chép nguyên luồng media khi container đích nhận được`.

---

### Task 3: Convert dùng `probe`, cảnh báo, và kiểm tra toàn bộ

**Files:**
- Modify: `crates/morphix-core/src/ops/media.rs` (`convert_media`, thêm `convert_media_reporting`, `media_warnings`)
- Modify: `crates/morphix-core/src/job.rs` (`size_warning` tách phần thông điệp; `run_item` nhận `warnings`; `run_job`)
- Test: `crates/morphix-core/tests/media.rs`, `#[cfg(test)]` trong `job.rs`

**Interfaces:**
- Consumes: `probe`, `convert_args_with`, `fully_remuxed`, `is_lossy_audio` (Task 1–2).
- Produces trong `job.rs`: `pub(crate) fn size_message(input: &Path, input_len: u64, output_len: u64) -> Option<String>` — phần không phụ thuộc định dạng của `size_warning` hiện tại (`None` khi nhỏ hơn; cùng chuỗi "không nhỏ hơn" / "lớn hơn file gốc X%"). `size_warning` chỉ còn kiểm đích ảnh rồi gọi `size_message`.
- Produces trong `media.rs`: `pub fn media_warnings(input: &Path, target: MediaFormat, info: &MediaInfo, input_len: u64, output_len: u64) -> Vec<String>`:
  - đích `Wav`/`Flac`/`Aiff` và `info.audio_codec` lossy → đúng một cảnh báo `"{tên}: âm thanh gốc đã nén mất dữ liệu; file mới lớn hơn nhưng chất lượng không tăng"` (tên = tên file nguồn), không thêm cảnh báo kích thước;
  - đích lossy (mọi đích có encoder trừ `Wav`, `Flac`, `Aiff`, `Gif`), không `fully_remuxed` → `size_message(...)` nếu có;
  - còn lại → rỗng.
- Produces: `pub fn convert_media_reporting(ffmpeg: &Path, input: &Path, to: MediaFormat, out_dir: Option<&Path>, cancel: &CancelToken, warnings: &mut Vec<String>) -> Result<PathBuf>` — gọi `probe` (lỗi → `tracing::warn!` rồi `MediaInfo::default()`), mã hoá bằng `encode_to(..., &info, ...)`, trong `accept` đo kích thước nguồn và file tạm rồi đẩy `media_warnings(...)` vào `warnings`. Đọc kích thước lỗi → `tracing::warn!`, không cảnh báo, không lỗi. `convert_media` giữ chữ ký, gọi `convert_media_reporting` với một `Vec` bỏ đi.
- `run_item(job, inputs, cancel, warnings: &mut Vec<String>)`: nhánh `ConvertMedia` gọi `convert_media_reporting`. `run_job` tạo `warnings`, gọi `run_item`, nối thêm `item_warnings(...)` (ảnh); kết quả `Err` thì xoá `warnings`.

- [ ] **Step 1: Viết test thất bại.**
  - `job.rs`: `size_message_is_format_independent` (`1000, 2000` → chứa `"100%"`; `1000, 500` → `None`); hai test `size_warning_*` hiện có vẫn đạt.
  - `media.rs` (thuần): `mp3_to_wav_warns_that_quality_does_not_improve` (info audio `mp3`, đích `Wav`, `100, 1000` → đúng 1 cảnh báo chứa `"chất lượng không tăng"`); `full_remux_has_no_size_warning` (info `h264`/`aac` sang `Mkv`, `100, 120` → rỗng); `reencode_that_grows_warns_about_size` (info `h264`/`aac` sang `WebM`, `100, 120` → chứa `"lớn hơn file gốc 20%"`); `gif_target_never_warns` (`100, 500` → rỗng).
  - Chạy ffmpeg thật, tự bỏ qua khi `find_ffmpeg()` là `None` (theo mẫu `avif_source_converts_back`; file nguồn tạo bằng `ffmpeg -f lavfi -i testsrc=size=160x120:rate=10:duration=1 -f lavfi -i sine=frequency=440:duration=1 -c:v libx264 -pix_fmt yuv420p -c:a aac -b:a 128k`, và `-f lavfi -i sine=frequency=440:duration=1 -c:a libmp3lame -b:a 128k` cho MP3; mọi nguồn `lavfi` phải có `duration` để ffmpeg không chạy vô hạn):
    - `real_mp4_to_mkv_keeps_the_video_codec_and_does_not_warn`: `run_job` `ConvertMedia { to: Mkv }` → `Ok`, `probe` file ra có `video_codec = Some("h264")`, `warnings` rỗng.
    - `real_mp3_to_wav_warns_about_lossless`: `run_job` `ConvertMedia { to: Wav }` → `Ok`, `warnings.len() == 1`.
- [ ] **Step 2: Chạy.** `cargo test -p morphix-core -- size_message media_warn quality remux grows gif_target real_` → FAIL.
- [ ] **Step 3: Cài đặt** theo Interfaces.
- [ ] **Step 4: Chạy toàn bộ cổng.**
  ```
  cargo clippy --workspace --all-targets -- -D warnings
  cargo test --workspace --no-fail-fast
  ```
  Expected: clippy 0 lỗi; test Rust không có lỗi mới so với mốc ghi trước Task 1; các test mới (kể cả `real_`) đạt trên máy có ffmpeg.
- [ ] **Step 5: Commit** `feat(morphix-core): convert media dùng thông tin nguồn và cảnh báo khi không có lợi`.
