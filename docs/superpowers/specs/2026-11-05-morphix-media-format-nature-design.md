# Morphix: convert video và âm thanh đúng bản chất định dạng

Status: thiết kế đã được người dùng duyệt trong hội thoại (2026-11-05); chờ duyệt bản viết này trước khi lập plan.

Đây là dự án con **C** trong lần thiết kế lại Morphix. Thứ tự: B (ảnh, đã làm) → A (menu Windows, đã làm, chờ kiểm tay) → C → D (PDF/tài liệu).

## Mục tiêu

Convert video và âm thanh theo đúng tác dụng của định dạng đích: chỉ đổi container thì không mã hoá lại; không làm file phình mà chất lượng không tăng; báo rõ khi kết quả không như người dùng chờ đợi.

## Vấn đề hiện tại

- `crates/morphix-core/src/ops/media.rs`: `convert_media` luôn **mã hoá lại** bằng argv cố định của đích (`encoder_for`), không xét codec nguồn. MP4 (H.264/AAC) sang MKV hay MOV bị mã hoá lại H.264 CRF 23: chậm, mất chất lượng, có thể to hơn.
- Âm thanh lossy sang lossy dùng bitrate cố định (MP3/M4A 192k, Opus 128k), nên MP3 128k sang M4A ra 192k: to hơn mà không hay hơn.
- Âm thanh lossy sang WAV/FLAC/AIFF làm file to gấp nhiều lần mà không lấy lại được chất lượng, và không có lời nhắc nào.
- `ItemReport.warnings` (dự án B) chỉ được điền cho `ConvertImage`.
- App chỉ đóng gói và tìm `ffmpeg` (`paths::find_ffmpeg`), không có `ffprobe`. Thông tin luồng hiện được đọc từ stderr của `ffmpeg -i` (`has_audio_stream`).

## Quyết định của người dùng

- **Remux khi được**: luồng nào có codec hợp lệ trong container đích thì chép nguyên, không mã hoá lại.
- **Không vượt bitrate nguồn** khi âm thanh lossy phải mã hoá sang lossy.
- **Cảnh báo lossy sang lossless**: vẫn làm, nhưng báo rõ chất lượng không tăng mà file lớn hơn nhiều.
- **Cảnh báo file to hơn** cho đích lossy, dùng lại cơ chế của dự án B. Vẫn giữ file ra, không tự xoá.

## Thiết kế

### 1. Đọc thông tin nguồn

- Hàm thuần `parse_streams(stderr: &str) -> MediaInfo` đọc các dòng `Stream #...` mà `ffmpeg -i` in ra:
  - codec video: từ đầu tiên sau `Video: ` (ví dụ `h264`, `hevc`, `vp9`), của luồng video đầu tiên không phải ảnh bìa (`attached pic`);
  - codec audio: từ đầu tiên sau `Audio: ` của luồng audio đầu tiên;
  - bitrate audio: số `N kb/s` trên dòng luồng audio đó, nếu có.
- Hàm `probe(ffmpeg, input) -> Result<MediaInfo>` chạy `ffmpeg -hide_banner -nostdin -i <input>` như `has_audio_stream` và gọi `parse_streams`. Không có dòng luồng nào thì trả `Corrupt` như `has_audio_stream` đang làm.
- Không đọc được một trường thì để `None`; quyết định ở mục 2 khi đó rơi về mã hoá lại như hiện nay. WHY: chữ của ffmpeg có thể đổi giữa các phiên bản; đọc sai không được làm hỏng việc, chỉ mất phần tối ưu.

### 2. Chép hay mã hoá theo từng luồng

| Đích | Video chép được | Audio chép được |
|---|---|---|
| MP4, MOV | h264, hevc | aac, mp3 |
| MKV | h264, hevc, vp9, av1, mpeg4 | aac, mp3, opus, vorbis, flac, ac3 |
| WebM | vp8, vp9, av1 | opus, vorbis |
| AVI | h264, mpeg4 | mp3 |
| MP3 | — | mp3 |
| M4A | — | aac |
| Opus | — | opus |
| Ogg | — | vorbis, opus |
| GIF, WAV, FLAC, AIFF | luôn mã hoá | luôn mã hoá |

- Mỗi luồng (video, audio) quyết định riêng: codec có trong bảng của đích thì `-c:v copy` / `-c:a copy`, còn lại dùng phần encoder tương ứng của `encoder_for` như hiện nay.
- Chọn luồng (`-vn`/`-an`/`-sn`/`-dn` của `stream_selection`) giữ nguyên. Phụ đề vẫn bị bỏ.
- Giữ `-movflags +faststart` cho MP4/MOV cả khi chép luồng.
- Không có thông tin nguồn (mục 1 trả `None`) thì mã hoá như hiện nay.

### 3. Bitrate âm thanh

- Khi luồng audio phải mã hoá, nguồn là codec lossy (mp3, aac, opus, vorbis, ac3, wmav2) và đích dùng `-b:a` cố định (MP3, M4A, Opus, MP4, MOV, MKV, WebM, AVI): dùng `min(bitrate nguồn, bitrate mặc định của đích)`.
- Ogg (Vorbis `-q:a 5`) và các đích lossless không đổi.
- Không biết bitrate nguồn thì dùng mặc định của đích.

### 4. Cảnh báo

Mở rộng `item_warnings` trong `crates/morphix-core/src/job.rs` cho `Operation::ConvertMedia { to }`:
- Nguồn có audio lossy và đích là WAV, FLAC hoặc AIFF: cảnh báo `"{tên}: âm thanh gốc đã nén mất dữ liệu; file mới lớn hơn nhưng chất lượng không tăng"`. Có cảnh báo này thì không thêm cảnh báo kích thước.
- Đích lossy mà file ra không nhỏ hơn nguồn: cảnh báo kích thước cùng dạng với ảnh (`size_warning`).
- Mọi luồng đều được chép (remux hoàn toàn): không cảnh báo kích thước, vì giữ gần nguyên kích thước là đúng bản chất.
- Thông tin "đã remux hay chưa" và "nguồn lossy" do `convert_media` trả cùng đường dẫn đầu ra, không đọc lại file.
- CLI và GUI hiển thị cảnh báo qua đường đã có của dự án B.

## Không đổi

- `compress_media` (nén), `cut_media`, `change_speed`, `extract_audio`.
- CRF, preset và `-pix_fmt yuv420p` khi phải mã hoá video.
- Danh sách đích trong `catalog` (`VIDEO_TARGETS`, `AUDIO_TARGETS`).

## Kiểm thử

- `parse_streams` với mẫu stderr thật: MP4 H.264 + AAC 128 kb/s; MKV H.264 + Opus không có bitrate trên dòng luồng; MP3 có ảnh bìa (`attached pic`) phải không bị coi là video; WAV `pcm_s16le`; chuỗi không có dòng luồng nào.
- Hàm dựng argv thuần cho các ca: chép cả hai luồng (MP4 sang MKV); chép video, mã hoá audio (MKV Opus sang MP4); mã hoá cả hai (MP4 sang WebM); đích audio chép (MP4 có audio mp3 sang MP3); bitrate bị giới hạn (MP3 128k sang M4A dùng `128k`); thiếu thông tin nguồn (giống argv hiện tại).
- Cảnh báo: MP3 sang WAV có cảnh báo lossless; remux hoàn toàn không có cảnh báo kích thước.
- Test chạy ffmpeg thật tự bỏ qua khi máy không có ffmpeg, theo mẫu `avif_source_converts_back`: MP4 sang MKV chạy xong và codec video của file ra vẫn là codec nguồn.
- Test media hiện có vẫn đạt.

## Ngoài phạm vi

- Giữ phụ đề khi remux.
- Giới hạn bitrate video, hay đổi CRF theo nguồn.
- Dùng `ffprobe`.
- Đổi danh sách đích.

## Rủi ro đã biết

- Đọc stderr của ffmpeg phụ thuộc chữ in ra; phiên bản ffmpeg khác có thể đổi định dạng. Hậu quả chỉ là mất phần remux/giới hạn bitrate, không hỏng việc.
- Chép luồng giữ nguyên mọi đặc tính của nguồn, kể cả những thứ thiết bị cũ không phát được (ví dụ H.264 yuv444, HEVC trong MOV trên máy không hỗ trợ). Đây là đánh đổi của remux: không mất chất lượng, nhưng không "chuẩn hoá" lại file.
- Một số tổ hợp hợp lệ trên giấy vẫn có thể bị muxer từ chối (ví dụ timestamp lạ). Khi ffmpeg báo lỗi lúc chép luồng, việc báo lỗi rõ ràng như hiện nay; không tự thử lại bằng mã hoá trong dự án này.
