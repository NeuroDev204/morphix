//! Bảng encoder cho đích âm thanh và video, dựng argv cho ffmpeg, kiểm ffmpeg đang cài có encoder
//! cần thiết hay không, và các thao tác chuyển, nén, cắt, đổi tốc độ, tách âm thanh.

use std::collections::HashSet;
use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::cancel::CancelToken;
use crate::error::{MorphixError, Result};
use crate::kind::{detect, FileKind, MediaFormat};
use crate::naming::{desired_output, write_output_path};
use crate::ops::compress::SUFFIX_COMPRESSED;
use crate::ops::ffmpeg::run_ffmpeg;

/// Hậu tố của file đã cắt, đổi tốc độ. WHY: khác với hậu tố ảnh để người dùng phân biệt được file.
pub const SUFFIX_CUT: &str = "-cat";
pub const SUFFIX_SPEED: &str = "-speed";

// WHY: `atempo` chỉ nhận hệ số từ 0.5 đến 2.0 cho mỗi bộ lọc; ngoài khoảng đó phải xếp nhiều bộ lọc.
const ATEMPO_MIN: f64 = 0.5;
const ATEMPO_MAX: f64 = 2.0;

// WHY: R1 đã chốt ffmpeg GPL có libx264, nên H.264 là codec video cho MP4 và MOV.
// Mình chọn: preset medium, crf 23 (mức cân bằng chất lượng và thời gian mã hoá của x264).
// WHY: yuv420p để trình duyệt và thiết bị cũ đọc được; nhiều nguồn ra yuv444 sẽ không phát được.
const H264_AAC: &[&str] = &[
    "-c:v",
    "libx264",
    "-preset",
    "medium",
    "-crf",
    "23",
    "-pix_fmt",
    "yuv420p",
    "-c:a",
    "aac",
    "-b:a",
    "128k",
    "-movflags",
    "+faststart",
];

// Mình chọn: MKV dùng Opus thay AAC, vì Opus là âm thanh gốc của container MKV hiện đại.
const H264_OPUS: &[&str] = &[
    "-c:v", "libx264", "-preset", "medium", "-crf", "23", "-pix_fmt", "yuv420p", "-c:a", "libopus",
    "-b:a", "128k",
];

// WHY: WebM chỉ dùng VP9 và Opus để trình duyệt đọc được.
// Mình chọn: crf 32 với -b:v 0 (chất lượng cố định, không giới hạn bitrate), -row-mt 1 để mã hoá đa luồng.
const VP9_OPUS: &[&str] = &[
    "-c:v",
    "libvpx-vp9",
    "-crf",
    "32",
    "-b:v",
    "0",
    "-row-mt",
    "1",
    "-c:a",
    "libopus",
    "-b:a",
    "128k",
];

// Mình chọn: AVI giữ H.264 và âm thanh MP3 192k, vì AVI thường được mở trên máy cũ chỉ đọc được MP3.
const H264_MP3_AVI: &[&str] = &[
    "-c:v",
    "libx264",
    "-preset",
    "medium",
    "-crf",
    "23",
    "-pix_fmt",
    "yuv420p",
    "-c:a",
    "libmp3lame",
    "-b:a",
    "192k",
];

// WHY: GIF không có codec; ffmpeg chọn muxer GIF theo đuôi file, nên không có cờ -c:v.
// Mình chọn: 10 khung/giây, rộng 480 px để file không quá nặng; lanczos cho chất lượng khi thu nhỏ.
const GIF_FILTER: &[&str] = &["-vf", "fps=10,scale=480:-1:flags=lanczos"];

const MP3: &[&str] = &["-c:a", "libmp3lame", "-b:a", "192k"];
const M4A: &[&str] = &["-c:a", "aac", "-b:a", "192k"];
const WAV: &[&str] = &["-c:a", "pcm_s16le"];
const FLAC: &[&str] = &["-c:a", "flac"];
// Mình chọn: Ogg dùng Vorbis theo thang chất lượng -q:a 5, vì Vorbis không dùng bitrate cố định như AAC.
const OGG: &[&str] = &["-c:a", "libvorbis", "-q:a", "5"];
const OPUS: &[&str] = &["-c:a", "libopus", "-b:a", "128k"];
const AIFF: &[&str] = &["-c:a", "pcm_s16be"];

/// Argv encoder cho đích. `None` nghĩa là không có đầu ra cho định dạng này.
/// WHY: WMA và WMV chỉ là đầu vào theo quyết định trong spec, không có encoder để ghi.
pub fn encoder_for(target: MediaFormat) -> Option<&'static [&'static str]> {
    match target {
        MediaFormat::Mp4 | MediaFormat::Mov => Some(H264_AAC),
        MediaFormat::Mkv => Some(H264_OPUS),
        MediaFormat::WebM => Some(VP9_OPUS),
        MediaFormat::Avi => Some(H264_MP3_AVI),
        MediaFormat::Gif => Some(GIF_FILTER),
        MediaFormat::Mp3 => Some(MP3),
        MediaFormat::M4a => Some(M4A),
        MediaFormat::Wav => Some(WAV),
        MediaFormat::Flac => Some(FLAC),
        MediaFormat::Ogg => Some(OGG),
        MediaFormat::Opus => Some(OPUS),
        MediaFormat::Aiff => Some(AIFF),
        MediaFormat::Wma | MediaFormat::Wmv => None,
    }
}

/// Dựng argv đầy đủ cho ffmpeg, không qua shell. `-n` để ffmpeg không bao giờ ghi đè: bên gọi
/// chọn tên đầu ra tạm mới, và đuôi file phải đúng để ffmpeg chọn muxer.
/// Trả `None` khi đích không có encoder.
pub fn convert_args(input: &Path, target: MediaFormat, output: &Path) -> Option<Vec<OsString>> {
    let encoder: &[&str] = encoder_for(target)?;
    let mut args: Vec<OsString> = ["-hide_banner", "-nostdin", "-n", "-i"]
        .into_iter()
        .map(OsString::from)
        .collect();
    args.push(OsString::from(input));
    args.extend(encoder.iter().map(|arg: &&str| OsString::from(arg)));
    args.push(OsString::from(output));
    Some(args)
}

/// Liệt kê tên encoder mà ffmpeg đang cài báo ra. Lỗi chạy ffmpeg được trả lên kèm đường dẫn,
/// không trả tập rỗng để tránh báo "không có encoder" khi thật ra ffmpeg hỏng.
pub fn available_encoders(ffmpeg: &Path) -> Result<HashSet<String>> {
    let output: std::process::Output = Command::new(ffmpeg)
        .args(["-hide_banner", "-encoders"])
        .stdin(Stdio::null())
        .output()
        .map_err(|source: io::Error| MorphixError::Io {
            path: ffmpeg.to_path_buf(),
            source,
        })?;
    if !output.status.success() {
        return Err(MorphixError::Io {
            path: ffmpeg.to_path_buf(),
            source: io::Error::other(format!("ffmpeg -encoders thoát với {}", output.status)),
        });
    }
    let text: std::borrow::Cow<'_, str> = String::from_utf8_lossy(&output.stdout);
    parse_encoders(&text).ok_or_else(|| MorphixError::Corrupt {
        path: ffmpeg.to_path_buf(),
        reason: "không đọc được danh sách encoder của ffmpeg".to_string(),
    })
}

/// Đọc tên encoder từ cột thứ hai, sau dòng `------` ngăn tiêu đề. Thiếu dòng ngăn thì `None`.
fn parse_encoders(text: &str) -> Option<HashSet<String>> {
    let mut lines = text
        .lines()
        .skip_while(|line: &&str| !line.trim_start().starts_with("------"));
    lines.next()?;
    Some(
        lines
            .filter_map(|line: &str| line.split_whitespace().nth(1))
            .map(str::to_string)
            .collect(),
    )
}

/// Codec `-c:v`/`-c:a` của đích mà `encoders` không có. Đích không có encoder thì trả rỗng.
pub fn missing_encoders(target: MediaFormat, encoders: &HashSet<String>) -> Vec<&'static str> {
    let Some(args) = encoder_for(target) else {
        return Vec::new();
    };
    args.windows(2)
        .filter(|pair: &&[&str]| matches!(pair[0], "-c:v" | "-c:a") && !encoders.contains(pair[1]))
        .map(|pair: &[&str]| pair[1])
        .collect()
}

/// Đích dùng được khi có encoder và mọi codec `-c:v`/`-c:a` của nó đều có trong `encoders`.
/// WHY: ffmpeg bản Ubuntu có thể thiếu libx264; kiểm trước để báo rõ thay vì để ffmpeg hỏng giữa chừng.
pub fn target_available(target: MediaFormat, encoders: &HashSet<String>) -> bool {
    encoder_for(target).is_some() && missing_encoders(target, encoders).is_empty()
}

/// Dừng sớm với đích không có encoder (không tốn lần gọi ffmpeg), rồi kiểm ffmpeg có đủ codec không.
/// Lỗi nêu tên encoder còn thiếu để người dùng biết phải cài bản ffmpeg nào.
pub fn ensure_encoders(target: MediaFormat, ffmpeg: &Path) -> Result<()> {
    if encoder_for(target).is_none() {
        return Err(MorphixError::InvalidInput(format!(
            "không ghi được {}: Morphix chưa có bộ mã hoá cho định dạng này",
            target.label()
        )));
    }
    let encoders: HashSet<String> = available_encoders(ffmpeg)?;
    if target_available(target, &encoders) {
        return Ok(());
    }
    Err(MorphixError::InvalidInput(format!(
        "không ghi được {}: ffmpeg đang cài thiếu encoder {}",
        target.label(),
        missing_encoders(target, &encoders).join(", ")
    )))
}

/// Kiểm file có luồng âm thanh không, dựa vào dòng `Stream #... Audio:` mà `ffmpeg -i` in ra.
/// WHY: `ffmpeg -i` không có đầu ra nên luôn thoát khác 0; chỉ đọc stderr. Không có dòng luồng nào
/// nghĩa là ffmpeg không đọc được file, nên báo `Corrupt` chứ không kết luận "không có âm thanh".
pub fn has_audio_stream(ffmpeg: &Path, input: &Path) -> Result<bool> {
    let output: std::process::Output = Command::new(ffmpeg)
        .args(["-hide_banner", "-nostdin", "-i"])
        .arg(input)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .map_err(|source: io::Error| MorphixError::Io {
            path: ffmpeg.to_path_buf(),
            source,
        })?;
    let text: std::borrow::Cow<'_, str> = String::from_utf8_lossy(&output.stderr);
    let streams: Vec<&str> = text
        .lines()
        .filter(|line: &&str| line.contains("Stream #"))
        .collect();
    if streams.is_empty() {
        let reason: String = text
            .lines()
            .rev()
            .find(|line: &&str| !line.trim().is_empty())
            .unwrap_or("ffmpeg không in thông tin luồng nào")
            .to_string();
        return Err(MorphixError::Corrupt {
            path: input.to_path_buf(),
            reason,
        });
    }
    Ok(streams.iter().any(|line: &&str| line.contains(": Audio:")))
}

/// Định dạng media của file, theo nội dung. File không phải âm thanh hay video thì `Unsupported`.
pub fn require_media(input: &Path) -> Result<MediaFormat> {
    match detect(input)? {
        FileKind::Media(format) => Ok(format),
        _ => Err(MorphixError::Unsupported {
            path: input.to_path_buf(),
        }),
    }
}

/// Hệ số `atempo` cho tốc độ `factor_milli` phần nghìn. Hệ số ngoài 0.5..2.0 được chia thành nhiều
/// bộ lọc có tích bằng hệ số đó, ví dụ 4.0 thành `atempo=2.0,atempo=2.0`.
pub fn atempo_filter(factor_milli: u32) -> String {
    let mut rest: f64 = f64::from(factor_milli) / 1000.0;
    let mut parts: Vec<String> = Vec::new();
    while rest > ATEMPO_MAX {
        parts.push(format_factor(ATEMPO_MAX));
        rest /= ATEMPO_MAX;
    }
    while rest < ATEMPO_MIN {
        parts.push(format_factor(ATEMPO_MIN));
        rest /= ATEMPO_MIN;
    }
    parts.push(format_factor(rest));
    parts
        .iter()
        .map(|part: &String| format!("atempo={part}"))
        .collect::<Vec<String>>()
        .join(",")
}

/// Số thập phân ít nhất một chữ số lẻ, bỏ số 0 thừa: 2.0, 1.5, 0.25.
fn format_factor(value: f64) -> String {
    let text: String = format!("{value:.3}");
    let trimmed: &str = text.trim_end_matches('0');
    if trimmed.ends_with('.') {
        format!("{trimmed}0")
    } else {
        trimmed.to_string()
    }
}

/// Mốc thời gian `HH:MM:SS.mmm` cho `-ss` và `-to` của ffmpeg.
fn format_timestamp(ms: u64) -> String {
    let hours: u64 = ms / 3_600_000;
    let minutes: u64 = (ms / 60_000) % 60;
    let seconds: u64 = (ms / 1000) % 60;
    let millis: u64 = ms % 1000;
    format!("{hours:02}:{minutes:02}:{seconds:02}.{millis:03}")
}

/// Argv encoder với tuỳ chọn đầu ra chèn ngay trước đường dẫn đầu ra (sau `-i`).
/// WHY: `-ss`, `-to`, `-vn` và bộ lọc là tuỳ chọn đầu ra; đặt sau `-i` để cắt đúng khung hình.
fn output_args(
    input: &Path,
    target: MediaFormat,
    output: &Path,
    options: &[String],
) -> Result<Vec<OsString>> {
    let mut args: Vec<OsString> = convert_args(input, target, output).ok_or_else(|| {
        MorphixError::InvalidInput(format!(
            "không ghi được {}: Morphix chưa có bộ mã hoá cho định dạng này",
            target.label()
        ))
    })?;
    // Đường dẫn đầu ra luôn là phần tử cuối nên `len - 1` không bao giờ âm.
    let tail: Vec<OsString> = args.split_off(args.len() - 1);
    args.extend(options.iter().map(OsString::from));
    args.extend(tail);
    Ok(args)
}

/// Lỗi của ffmpeg mang đường dẫn chính ffmpeg; đổi sang file đầu vào để người dùng biết file nào hỏng.
fn with_input_path(error: MorphixError, input: &Path) -> MorphixError {
    match error {
        MorphixError::Corrupt { reason, .. } => MorphixError::Corrupt {
            path: input.to_path_buf(),
            reason,
        },
        other => other,
    }
}

/// Chạy ffmpeg ghi vào file tạm rồi đặt thành `desired`. `accept` kiểm file tạm trước khi đặt tên;
/// lỗi từ `accept` hoặc ffmpeg thì file tạm bị xoá cùng thư mục tạm.
fn encode_to(
    ffmpeg: &Path,
    input: &Path,
    target: MediaFormat,
    desired: &Path,
    options: &[String],
    cancel: &CancelToken,
    accept: impl FnOnce(&Path) -> Result<()>,
) -> Result<PathBuf> {
    write_output_path(desired, |temp: &Path| {
        let args: Vec<OsString> = output_args(input, target, temp, options)?;
        // WHY: tiến trình chưa được báo lên giao diện; việc này nằm ở tác vụ sau, nên bỏ qua mốc ở đây.
        run_ffmpeg(ffmpeg, &args, cancel, &mut |_: u64| {})
            .map_err(|error: MorphixError| with_input_path(error, input))?;
        accept(temp)
    })
}

/// Chuyển âm thanh hoặc video sang đích `to`, dùng encoder của đích.
pub fn convert_media(
    ffmpeg: &Path,
    input: &Path,
    to: MediaFormat,
    out_dir: Option<&Path>,
    cancel: &CancelToken,
) -> Result<PathBuf> {
    require_media(input)?;
    ensure_encoders(to, ffmpeg)?;
    let desired: PathBuf = desired_output(input, "", to.extension(), out_dir);
    encode_to(ffmpeg, input, to, &desired, &[], cancel, |_: &Path| Ok(()))
}

/// Nén lại theo định dạng gốc, dùng encoder của chính nó. WHY: nén không bao giờ được ra file lớn hơn
/// hoặc bằng gốc; trường hợp đó trả `NotSmaller` và xoá file tạm, thay vì để người dùng có file to hơn.
pub fn compress_media(
    ffmpeg: &Path,
    input: &Path,
    out_dir: Option<&Path>,
    cancel: &CancelToken,
) -> Result<PathBuf> {
    let format: MediaFormat = require_media(input)?;
    ensure_encoders(format, ffmpeg)?;
    let desired: PathBuf = desired_output(input, SUFFIX_COMPRESSED, format.extension(), out_dir);
    encode_to(
        ffmpeg,
        input,
        format,
        &desired,
        &[],
        cancel,
        |temp: &Path| ensure_smaller(input, temp),
    )
}

fn ensure_smaller(input: &Path, output: &Path) -> Result<()> {
    let size = |path: &Path| -> Result<u64> {
        std::fs::metadata(path)
            .map(|meta: std::fs::Metadata| meta.len())
            .map_err(|source: io::Error| MorphixError::Io {
                path: path.to_path_buf(),
                source,
            })
    };
    if size(output)? >= size(input)? {
        return Err(MorphixError::NotSmaller {
            path: input.to_path_buf(),
        });
    }
    Ok(())
}

/// Cắt đoạn từ `start_ms` đến `end_ms` (mili-giây), giữ định dạng gốc.
pub fn cut_media(
    ffmpeg: &Path,
    input: &Path,
    start_ms: u64,
    end_ms: u64,
    out_dir: Option<&Path>,
    cancel: &CancelToken,
) -> Result<PathBuf> {
    if end_ms <= start_ms {
        return Err(MorphixError::InvalidInput(
            "thời điểm kết thúc phải sau thời điểm bắt đầu".to_string(),
        ));
    }
    let format: MediaFormat = require_media(input)?;
    ensure_encoders(format, ffmpeg)?;
    let options: Vec<String> = vec![
        "-ss".to_string(),
        format_timestamp(start_ms),
        "-to".to_string(),
        format_timestamp(end_ms),
    ];
    let desired: PathBuf = desired_output(input, SUFFIX_CUT, format.extension(), out_dir);
    encode_to(
        ffmpeg,
        input,
        format,
        &desired,
        &options,
        cancel,
        |_: &Path| Ok(()),
    )
}

/// Đổi tốc độ theo `factor_milli` phần nghìn (1500 là nhanh 1,5 lần). Video đổi khung hình bằng
/// `setpts`; âm thanh đổi bằng chuỗi `atempo`, và chỉ khi file thật sự có âm thanh.
pub fn change_speed(
    ffmpeg: &Path,
    input: &Path,
    factor_milli: u32,
    out_dir: Option<&Path>,
    cancel: &CancelToken,
) -> Result<PathBuf> {
    if factor_milli == 0 {
        return Err(MorphixError::InvalidInput(
            "hệ số tốc độ phải lớn hơn 0".to_string(),
        ));
    }
    let format: MediaFormat = require_media(input)?;
    ensure_encoders(format, ffmpeg)?;
    let is_video: bool = !format.is_audio();
    let has_audio: bool = !is_video || has_audio_stream(ffmpeg, input)?;
    let mut options: Vec<String> = Vec::new();
    if is_video {
        // WHY: setpts chia mốc khung hình cho hệ số; chia PTS là nhanh lên, nhân là chậm lại.
        let factor: String = format_factor(f64::from(factor_milli) / 1000.0);
        options.push("-filter:v".to_string());
        options.push(format!("setpts=PTS/{factor}"));
    }
    if has_audio {
        options.push("-filter:a".to_string());
        options.push(atempo_filter(factor_milli));
    }
    let desired: PathBuf = desired_output(input, SUFFIX_SPEED, format.extension(), out_dir);
    encode_to(
        ffmpeg,
        input,
        format,
        &desired,
        &options,
        cancel,
        |_: &Path| Ok(()),
    )
}

/// Tách âm thanh của video sang định dạng âm thanh `to`. Video không có âm thanh thì lỗi rõ và không
/// tạo file, vì ffmpeg sẽ tạo file rỗng hoặc hỏng nếu cố ghi luồng âm thanh không tồn tại.
pub fn extract_audio(
    ffmpeg: &Path,
    input: &Path,
    to: MediaFormat,
    out_dir: Option<&Path>,
    cancel: &CancelToken,
) -> Result<PathBuf> {
    if !to.is_audio() {
        return Err(MorphixError::InvalidInput(format!(
            "không tách được sang {}: chỉ tách sang định dạng âm thanh",
            to.label()
        )));
    }
    require_media(input)?;
    ensure_encoders(to, ffmpeg)?;
    if !has_audio_stream(ffmpeg, input)? {
        return Err(MorphixError::InvalidInput(format!(
            "{} không có âm thanh để tách",
            input.display()
        )));
    }
    let desired: PathBuf = desired_output(input, "", to.extension(), out_dir);
    encode_to(
        ffmpeg,
        input,
        to,
        &desired,
        &["-vn".to_string()],
        cancel,
        |_: &Path| Ok(()),
    )
}
