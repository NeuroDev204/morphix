// WHY: các test dùng script sh giả là cfg(unix), nên trên Windows import và SAMPLE_ENCODERS của chúng thừa.
#![cfg_attr(not(unix), allow(unused_imports, dead_code))]

use std::collections::HashSet;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

use morphix_core::error::MorphixError;
use morphix_core::kind::MediaFormat;
use morphix_core::ops::media::{
    available_encoders, convert_args, convert_args_with, encoder_for, fully_remuxed,
    media_warnings, parse_streams, probe, target_available, MediaInfo,
};
use morphix_core::{run_job, CancelToken, Job, Operation, Quality};
use tempfile::{tempdir, TempDir};

/// Đầu ra thật của `ffmpeg -hide_banner -encoders` (rút gọn), gồm cả phần tiêu đề trước dòng `------`.
const SAMPLE_ENCODERS: &str = "Encoders:
 V..... = Video
 A..... = Audio
 S..... = Subtitle
 .F.... = Frame-level multithreading
 ..S... = Slice-level multithreading
 ...X.. = Codec is experimental
 ....B. = Supports draw_horiz_band
 .....D = Supports direct rendering method 1
 ------
 V....D libx264              libx264 H.264 / AVC / MPEG-4 AVC (codec h264)
 V....D libvpx-vp9           libvpx VP9 (codec vp9)
 A....D libopus              libopus Opus (codec opus)
 V....D h264_vaapi           H.264/AVC (VAAPI) (codec h264)
 A....D aac                  AAC (Advanced Audio Coding)
";

fn argv_strings(args: &[OsString]) -> Vec<String> {
    args.iter()
        .map(|a| a.to_string_lossy().into_owned())
        .collect()
}

fn set_of(names: &[&str]) -> HashSet<String> {
    names.iter().map(|n| n.to_string()).collect()
}

/// Ghi một script sh giả đóng vai ffmpeg, in ra nội dung `sample`. Test không chạy ffmpeg thật.
#[cfg(unix)]
fn fake_ffmpeg_printing(dir: &TempDir, sample: &str) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;

    let sample_path: PathBuf = dir.path().join("sample.txt");
    std::fs::write(&sample_path, sample).expect("ghi mẫu encoder");
    let script: PathBuf = dir.path().join("fake-ffmpeg.sh");
    let body: String = format!("#!/bin/sh\ncat '{}'\n", sample_path.display());
    std::fs::write(&script, body).expect("ghi script giả");
    // WHY: script phải có quyền thực thi vì available_encoders chạy nó trực tiếp như ffmpeg.
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755))
        .expect("cấp quyền thực thi");
    script
}

// Mẫu stderr thật của `ffmpeg -i` (ffmpeg 9.0.2).
const MP4_VIDEO: &str = "  Stream #0:0[0x1](und): Video: h264 (High) (avc1 / 0x31637661), yuv420p(progressive), 160x120 [SAR 1:1 DAR 4:3], 35 kb/s, 10 fps, 10 tbr, 10240 tbn (default)";
const MP4_AUDIO: &str = "  Stream #0:1[0x2](und): Audio: aac (LC) (mp4a / 0x6134706D), 44100 Hz, mono, fltp, 116 kb/s (default)";
const MKV_VIDEO: &str = "  Stream #0:0: Video: h264 (High 4:4:4 Predictive), yuv444p(tv, progressive), 160x120 [SAR 1:1 DAR 4:3], 10 fps, 10 tbr, 1k tbn";
const MKV_AUDIO: &str = "  Stream #0:1: Audio: opus, 48000 Hz, mono, fltp";
const WAV_AUDIO: &str =
    "  Stream #0:0: Audio: pcm_s16le ([1][0][0][0] / 0x0001), 44100 Hz, mono, s16, 705 kb/s";
const MP3_AUDIO: &str =
    "  Stream #0:0: Audio: mp3 (mp3float), 44100 Hz, mono, fltp, 128 kb/s, start 0.025057";
const MP3_COVER: &str = "  Stream #0:1: Video: png, rgb24(pc, gbr/unknown/unknown), 64x64 [SAR 1:1 DAR 1:1], 90k tbr, 90k tbn, start 0.025056 (attached pic)";

fn stderr_of(lines: &[&str]) -> String {
    lines.join("\n")
}

#[test]
fn parse_mp4_h264_aac() {
    let info = parse_streams(&stderr_of(&[MP4_VIDEO, MP4_AUDIO])).unwrap();
    assert_eq!(info.video_codec.as_deref(), Some("h264"));
    assert_eq!(info.audio_codec.as_deref(), Some("aac"));
    assert_eq!(info.audio_kbps, Some(116));
}

#[test]
fn parse_mkv_opus_without_bitrate() {
    let info = parse_streams(&stderr_of(&[MKV_VIDEO, MKV_AUDIO])).unwrap();
    assert_eq!(info.video_codec.as_deref(), Some("h264"));
    assert_eq!(info.audio_codec.as_deref(), Some("opus"));
    assert_eq!(info.audio_kbps, None);
}

#[test]
fn parse_mp3_ignores_attached_cover() {
    let info = parse_streams(&stderr_of(&[MP3_AUDIO, MP3_COVER])).unwrap();
    assert_eq!(info.video_codec, None);
    assert_eq!(info.audio_codec.as_deref(), Some("mp3"));
    assert_eq!(info.audio_kbps, Some(128));
}

#[test]
fn parse_wav_pcm() {
    let info = parse_streams(&stderr_of(&[WAV_AUDIO])).unwrap();
    assert_eq!(info.audio_codec.as_deref(), Some("pcm_s16le"));
}

#[test]
fn parse_takes_the_first_audio_stream() {
    let second = "  Stream #0:2: Audio: ac3, 48000 Hz, 5.1, fltp, 384 kb/s";
    let info = parse_streams(&stderr_of(&[MP4_VIDEO, MP4_AUDIO, second])).unwrap();
    assert_eq!(info.audio_codec.as_deref(), Some("aac"));
    assert_eq!(info.audio_kbps, Some(116));
}

#[test]
fn parse_without_stream_lines_is_none() {
    assert_eq!(
        parse_streams("a.xyz: Invalid data found when processing input"),
        None
    );
}

fn info(video: Option<&str>, audio: Option<&str>, kbps: Option<u32>) -> MediaInfo {
    MediaInfo {
        video_codec: video.map(str::to_string),
        audio_codec: audio.map(str::to_string),
        audio_kbps: kbps,
    }
}

fn args_with(target: MediaFormat, info: &MediaInfo) -> Vec<String> {
    let args = convert_args_with(Path::new("in"), target, Path::new("out"), info)
        .expect("đích có encoder");
    argv_strings(&args)
}

fn has_pair(argv: &[String], flag: &str, value: &str) -> bool {
    argv.windows(2)
        .any(|pair| pair[0] == flag && pair[1] == value)
}

#[test]
fn mp4_to_mkv_copies_both_streams() {
    let source = info(Some("h264"), Some("aac"), Some(116));
    let argv = args_with(MediaFormat::Mkv, &source);
    assert!(has_pair(&argv, "-c:v", "copy"), "{argv:?}");
    assert!(has_pair(&argv, "-c:a", "copy"), "{argv:?}");
    assert!(!argv.contains(&"libx264".to_string()));
    assert!(fully_remuxed(MediaFormat::Mkv, &source));
}

#[test]
fn mkv_opus_to_mp4_copies_video_and_encodes_audio() {
    let source = info(Some("h264"), Some("opus"), None);
    let argv = args_with(MediaFormat::Mp4, &source);
    assert!(has_pair(&argv, "-c:v", "copy"), "{argv:?}");
    assert!(has_pair(&argv, "-c:a", "aac"), "{argv:?}");
    assert!(has_pair(&argv, "-b:a", "128k"), "{argv:?}");
    assert!(has_pair(&argv, "-movflags", "+faststart"), "{argv:?}");
    assert!(!fully_remuxed(MediaFormat::Mp4, &source));
}

#[test]
fn mp4_to_webm_encodes_both_and_caps_audio_bitrate() {
    let source = info(Some("h264"), Some("aac"), Some(116));
    let argv = args_with(MediaFormat::WebM, &source);
    assert!(has_pair(&argv, "-c:v", "libvpx-vp9"), "{argv:?}");
    assert!(has_pair(&argv, "-c:a", "libopus"), "{argv:?}");
    assert!(has_pair(&argv, "-b:a", "116k"), "{argv:?}");
}

#[test]
fn video_with_mp3_audio_to_mp3_copies_audio() {
    let source = info(Some("h264"), Some("mp3"), Some(128));
    let argv = args_with(MediaFormat::Mp3, &source);
    assert!(argv.contains(&"-vn".to_string()), "{argv:?}");
    assert!(has_pair(&argv, "-c:a", "copy"), "{argv:?}");
}

#[test]
fn mp3_128k_to_m4a_keeps_128k() {
    let argv = args_with(MediaFormat::M4a, &info(None, Some("mp3"), Some(128)));
    assert!(has_pair(&argv, "-c:a", "aac"), "{argv:?}");
    assert!(has_pair(&argv, "-b:a", "128k"), "{argv:?}");
}

#[test]
fn zero_or_missing_bitrate_keeps_the_default() {
    for kbps in [Some(0), None] {
        let argv = args_with(MediaFormat::M4a, &info(None, Some("mp3"), kbps));
        assert!(has_pair(&argv, "-b:a", "192k"), "{kbps:?}: {argv:?}");
    }
}

#[test]
fn unknown_source_matches_the_old_argv() {
    for target in [
        MediaFormat::Mp3,
        MediaFormat::M4a,
        MediaFormat::Wav,
        MediaFormat::Flac,
        MediaFormat::Ogg,
        MediaFormat::Opus,
        MediaFormat::Aiff,
        MediaFormat::Mp4,
        MediaFormat::Mov,
        MediaFormat::Mkv,
        MediaFormat::WebM,
        MediaFormat::Avi,
        MediaFormat::Gif,
    ] {
        assert_eq!(
            convert_args_with(
                Path::new("in"),
                target,
                Path::new("out"),
                &MediaInfo::default()
            ),
            convert_args(Path::new("in"), target, Path::new("out")),
            "{target:?}"
        );
    }
}

#[test]
fn lossless_and_gif_targets_are_never_remuxed() {
    let same = info(Some("h264"), Some("pcm_s16le"), None);
    for target in [
        MediaFormat::Wav,
        MediaFormat::Flac,
        MediaFormat::Aiff,
        MediaFormat::Gif,
    ] {
        assert!(!fully_remuxed(target, &same), "{target:?}");
    }
    assert!(!fully_remuxed(MediaFormat::Mkv, &MediaInfo::default()));
}

fn warn(source: &MediaInfo, target: MediaFormat, input_len: u64, output_len: u64) -> Vec<String> {
    media_warnings(Path::new("/x/a.mp3"), target, source, input_len, output_len)
}

#[test]
fn mp3_to_wav_warns_that_quality_does_not_improve() {
    let warnings = warn(
        &info(None, Some("mp3"), Some(128)),
        MediaFormat::Wav,
        100,
        1000,
    );
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    assert!(
        warnings[0].contains("chất lượng không tăng"),
        "{warnings:?}"
    );
}

#[test]
fn full_remux_has_no_size_warning() {
    let source = info(Some("h264"), Some("aac"), Some(116));
    assert!(warn(&source, MediaFormat::Mkv, 100, 120).is_empty());
}

#[test]
fn reencode_that_grows_warns_about_size() {
    let source = info(Some("h264"), Some("aac"), Some(116));
    let warnings = warn(&source, MediaFormat::WebM, 100, 120);
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    assert!(warnings[0].contains("lớn hơn file gốc 20%"), "{warnings:?}");
}

#[test]
fn gif_target_never_warns() {
    let source = info(Some("h264"), Some("aac"), Some(116));
    assert!(warn(&source, MediaFormat::Gif, 100, 500).is_empty());
}

fn make_with_ffmpeg(ffmpeg: &Path, dir: &Path, name: &str, args: &[&str]) -> PathBuf {
    let path = dir.join(name);
    let status = std::process::Command::new(ffmpeg)
        .args(["-hide_banner", "-loglevel", "error", "-nostdin", "-y"])
        .args(args)
        .arg(&path)
        .status()
        .expect("chạy được ffmpeg");
    assert!(status.success(), "ffmpeg không tạo được {name}");
    path
}

fn convert_job(input: PathBuf, to: MediaFormat) -> Job {
    Job {
        inputs: vec![input],
        operation: Operation::ConvertMedia { to },
        quality: Quality::Balanced,
        out_dir: None,
    }
}

#[test]
fn real_mp4_to_mkv_keeps_the_video_codec_and_does_not_warn() {
    let Some(ffmpeg) = morphix_core::find_ffmpeg() else {
        eprintln!("bỏ qua: máy test không có ffmpeg");
        return;
    };
    let dir = tempdir().unwrap();
    let source = make_with_ffmpeg(
        &ffmpeg,
        dir.path(),
        "a.mp4",
        &[
            "-f",
            "lavfi",
            "-i",
            "testsrc=size=160x120:rate=10:duration=1",
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=440:duration=1",
            "-c:v",
            "libx264",
            "-pix_fmt",
            "yuv420p",
            "-c:a",
            "aac",
            "-b:a",
            "128k",
        ],
    );
    let report = run_job(
        &convert_job(source, MediaFormat::Mkv),
        &CancelToken::new(),
        &|_| {},
    );
    let item = &report.items[0];
    let output = item.result.as_ref().expect("MP4 sang MKV chạy được");
    let converted = probe(&ffmpeg, output).unwrap();
    assert_eq!(converted.video_codec.as_deref(), Some("h264"));
    assert!(item.warnings.is_empty(), "{:?}", item.warnings);
}

#[test]
fn real_mp3_to_wav_warns_about_lossless() {
    let Some(ffmpeg) = morphix_core::find_ffmpeg() else {
        eprintln!("bỏ qua: máy test không có ffmpeg");
        return;
    };
    let dir = tempdir().unwrap();
    let source = make_with_ffmpeg(
        &ffmpeg,
        dir.path(),
        "a.mp3",
        &[
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=440:duration=1",
            "-c:a",
            "libmp3lame",
            "-b:a",
            "128k",
        ],
    );
    let report = run_job(
        &convert_job(source, MediaFormat::Wav),
        &CancelToken::new(),
        &|_| {},
    );
    let item = &report.items[0];
    assert!(item.result.is_ok(), "{:?}", item.result);
    assert_eq!(item.warnings.len(), 1, "{:?}", item.warnings);
}

#[test]
fn wav_to_flac_argv_has_c_a_flac() {
    let args: Vec<OsString> = convert_args(
        Path::new("/tmp/in.wav"),
        MediaFormat::Flac,
        Path::new("/tmp/out.flac"),
    )
    .expect("FLAC có encoder");
    let argv: Vec<String> = argv_strings(&args);
    let pos: usize = argv.iter().position(|a| a == "-c:a").expect("argv có -c:a");
    assert_eq!(argv[pos + 1], "flac");
}

#[test]
fn mp4_argv_uses_libx264_and_faststart() {
    let args: Vec<OsString> = convert_args(
        Path::new("/tmp/in.mkv"),
        MediaFormat::Mp4,
        Path::new("/tmp/out.mp4"),
    )
    .expect("MP4 có encoder");
    let argv: Vec<String> = argv_strings(&args);
    assert!(argv.contains(&"libx264".to_string()));
    assert!(argv.contains(&"-movflags".to_string()));
    assert!(argv.contains(&"+faststart".to_string()));
}

#[test]
fn wmv_and_wma_have_no_encoder() {
    for target in [MediaFormat::Wmv, MediaFormat::Wma] {
        assert_eq!(encoder_for(target), None, "{target:?} không có encoder");
        assert!(
            convert_args(Path::new("/tmp/in.mp4"), target, Path::new("/tmp/out")).is_none(),
            "{target:?} không dựng được argv"
        );
    }
}

#[test]
fn gif_uses_video_filter_without_codec() {
    let args: Vec<OsString> = convert_args(
        Path::new("/tmp/in.mp4"),
        MediaFormat::Gif,
        Path::new("/tmp/out.gif"),
    )
    .expect("GIF có encoder");
    let argv: Vec<String> = argv_strings(&args);
    assert!(argv.contains(&"-vf".to_string()));
    assert!(!argv.contains(&"-c:v".to_string()));
}

#[test]
fn argv_order_and_awkward_paths_pass_through_unchanged() {
    let input: PathBuf = PathBuf::from("/home/a/Nhạc thử/it's here.wav");
    let output: PathBuf = PathBuf::from("/home/a/Nhạc thử/it's here.flac");
    let args: Vec<OsString> =
        convert_args(&input, MediaFormat::Flac, &output).expect("FLAC có encoder");
    assert_eq!(
        argv_strings(&args)[..4],
        ["-hide_banner", "-nostdin", "-n", "-i"]
    );
    assert_eq!(args[4], OsString::from(&input));
    assert_eq!(args.last(), Some(&OsString::from(&output)));
}

#[cfg(unix)]
#[test]
fn available_encoders_reads_sample_list() {
    let dir: TempDir = tempdir().expect("thư mục tạm");
    let ffmpeg: PathBuf = fake_ffmpeg_printing(&dir, SAMPLE_ENCODERS);
    let found: HashSet<String> = available_encoders(&ffmpeg).expect("đọc được danh sách");
    assert_eq!(
        found,
        set_of(&["libx264", "libvpx-vp9", "libopus", "h264_vaapi", "aac"])
    );
}

#[test]
fn available_encoders_reports_missing_ffmpeg_path() {
    let missing: PathBuf = PathBuf::from("/nonexistent/morphix/ffmpeg");
    match available_encoders(&missing) {
        Err(MorphixError::Io { path, .. }) => assert_eq!(path, missing),
        other => panic!("cần lỗi Io kèm đường dẫn ffmpeg, được {other:?}"),
    }
}

#[test]
fn target_available_mp4_needs_libx264_and_aac() {
    assert!(!target_available(
        MediaFormat::Mp4,
        &set_of(&["aac", "libopus"])
    ));
    assert!(target_available(
        MediaFormat::Mp4,
        &set_of(&["libx264", "aac"])
    ));
    assert!(!target_available(
        MediaFormat::Wmv,
        &set_of(&["libx264", "aac"])
    ));
    assert!(target_available(MediaFormat::Gif, &set_of(&[])));
}

#[test]
fn video_input_offers_video_targets_and_compress() {
    use morphix_core::catalog::{actions_for, ActionKind};
    use morphix_core::kind::FileKind;

    let actions = actions_for(&[FileKind::Media(MediaFormat::Mp4)]);
    let kinds: Vec<ActionKind> = actions.iter().map(|a| a.kind).collect();
    assert!(kinds.contains(&ActionKind::ConvertMedia(MediaFormat::Mkv)));
    assert!(kinds.contains(&ActionKind::ConvertMedia(MediaFormat::Mp3)));
    assert!(kinds.contains(&ActionKind::CompressMedia));
    // WHY: đích trùng định dạng gốc bị ẩn, như ảnh và văn bản; nén là việc riêng.
    assert!(!kinds.contains(&ActionKind::ConvertMedia(MediaFormat::Mp4)));
}

#[test]
fn audio_input_offers_audio_targets_only() {
    use morphix_core::catalog::{actions_for, ActionKind};
    use morphix_core::kind::FileKind;

    let actions = actions_for(&[FileKind::Media(MediaFormat::Mp3)]);
    let kinds: Vec<ActionKind> = actions.iter().map(|a| a.kind).collect();
    assert!(kinds.contains(&ActionKind::ConvertMedia(MediaFormat::Opus)));
    assert!(!kinds.contains(&ActionKind::ConvertMedia(MediaFormat::Mp4)));
    assert!(!kinds.contains(&ActionKind::ConvertMedia(MediaFormat::Mp3)));
}

#[cfg(unix)]
mod ffmpeg_ops {
    use std::path::{Path, PathBuf};
    use std::sync::{Mutex, MutexGuard, PoisonError};
    use std::thread;
    use std::time::Duration;

    use morphix_core::error::MorphixError;
    use morphix_core::kind::MediaFormat;
    use morphix_core::ops::media::{
        atempo_filter, change_speed, compress_media, convert_media, cut_media, extract_audio,
        has_audio_stream,
    };
    use morphix_core::CancelToken;
    use tempfile::{tempdir, TempDir};

    use super::user_files;

    const FULL_ENCODERS: &str = "Encoders:\n ------\n V....D libx264 x\n V....D libvpx-vp9 x\n A....D libopus x\n A....D aac x\n A....D libmp3lame x\n A....D libvorbis x\n A....D flac x\n A....D pcm_s16le x\n A....D pcm_s16be x\n";
    const WITH_AUDIO: &str = "  Stream #0:0(und): Video: h264 (High), yuv420p\n  Stream #0:1(und): Audio: aac (LC), 48000 Hz\n";
    const VIDEO_ONLY: &str = "  Stream #0:0(und): Video: h264 (High), yuv420p\n";
    const NO_STREAMS: &str = "Invalid data found when processing input\n";
    /// Nguồn mà MP4 không nhận nguyên codec, nên buộc phải mã hoá lại cả hai luồng.
    const VP9_OPUS: &str =
        "  Stream #0:0(und): Video: vp9\n  Stream #0:1(und): Audio: opus, 48000 Hz\n";
    const MP4_HEAD: &[u8] = b"\x00\x00\x00\x18ftypisom\x00\x00\x02\x00";
    const MKV_HEAD: &[u8] = b"\x1A\x45\xDF\xA3\x9F\x42\x82\x88matroska";
    const MP3_HEAD: &[u8] = b"ID3\x04\x00\x00\x00\x00\x00\x00";
    /// Chuỗi các test chạy ffmpeg giả. WHY: ghi script rồi exec ngay trong lúc test khác fork
    /// có thể gặp ETXTBSY, cùng lý do Task 6 chạy script qua /bin/sh.
    static FFMPEG_LOCK: Mutex<()> = Mutex::new(());

    fn serial() -> MutexGuard<'static, ()> {
        FFMPEG_LOCK.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Script giả đóng vai ffmpeg. Mỗi lần gọi ghi argv vào `log`, một đối số một dòng, sau dòng `---`.
    struct Fake {
        dir: TempDir,
        ffmpeg: PathBuf,
        log: PathBuf,
    }

    impl Fake {
        /// `body` là lệnh chạy khi ffmpeg ghi đầu ra; đầu ra là đối số cuối, lưu ở `$out`.
        fn new(encoders: &str, streams: &str, body: &str) -> Self {
            use std::os::unix::fs::PermissionsExt;
            let dir: TempDir = tempdir().expect("thư mục tạm");
            let enc_path: PathBuf = dir.path().join("encoders.txt");
            let streams_path: PathBuf = dir.path().join("streams.txt");
            let log: PathBuf = dir.path().join("argv.log");
            std::fs::write(&enc_path, encoders).expect("ghi encoders");
            std::fs::write(&streams_path, streams).expect("ghi streams");
            let script: String = format!(
                "#!/bin/sh\n\
                 printf -- '---\\n' >> '{log}'\n\
                 for arg in \"$@\"; do printf '%s\\n' \"$arg\" >> '{log}'; done\n\
                 if [ \"$1\" = \"-hide_banner\" ] && [ \"$2\" = \"-encoders\" ]; then cat '{enc}'; exit 0; fi\n\
                 if [ $# -eq 4 ]; then cat '{streams}' >&2; exit 1; fi\n\
                 for out; do :; done\n\
                 {body}\n",
                log = log.display(),
                enc = enc_path.display(),
                streams = streams_path.display(),
            );
            let ffmpeg: PathBuf = dir.path().join("fake-ffmpeg");
            std::fs::write(&ffmpeg, script).expect("ghi script giả");
            std::fs::set_permissions(&ffmpeg, std::fs::Permissions::from_mode(0o755))
                .expect("cấp quyền thực thi");
            Self { dir, ffmpeg, log }
        }

        fn dir(&self) -> &Path {
            self.dir.path()
        }

        /// File đầu vào có đúng đầu file để nhận dạng, đệm 0 tới 512 byte như fixture của `kind.rs`.
        fn input(&self, name: &str, head: &[u8]) -> PathBuf {
            let path: PathBuf = self.dir().join(name);
            let mut bytes: Vec<u8> = head.to_vec();
            bytes.resize(512, 0);
            std::fs::write(&path, bytes).expect("ghi file đầu vào");
            path
        }

        /// Mọi lần gọi ffmpeg giả, mỗi lần là danh sách đối số.
        fn calls(&self) -> Vec<Vec<String>> {
            let text: String = std::fs::read_to_string(&self.log).unwrap_or_default();
            text.split("---\n")
                .filter(|chunk: &&str| !chunk.is_empty())
                .map(|chunk: &str| chunk.lines().map(str::to_string).collect())
                .collect()
        }

        /// Lần gọi chuyển đổi, nhận ra qua cờ `-n`. Không tính lần kiểm encoder hay thăm luồng.
        fn conversions(&self) -> Vec<Vec<String>> {
            self.calls()
                .into_iter()
                .filter(|call: &Vec<String>| call.iter().any(|a: &String| a == "-n"))
                .collect()
        }
    }

    /// Giá trị đứng ngay sau `flag` trong argv của một lần gọi.
    fn value_after<'a>(call: &'a [String], flag: &str) -> &'a str {
        let pos: usize = call
            .iter()
            .position(|a: &String| a == flag)
            .unwrap_or_else(|| panic!("argv thiếu {flag}: {call:?}"));
        &call[pos + 1]
    }

    fn encoders_without_x264() -> String {
        FULL_ENCODERS.replace("libx264", "libx265")
    }

    #[test]
    fn convert_refuses_missing_encoder_without_running_conversion() {
        let _lock = serial();
        let fake = Fake::new(&encoders_without_x264(), WITH_AUDIO, "printf x > \"$out\"");
        let input: PathBuf = fake.input("clip.mkv", MKV_HEAD);
        let result = convert_media(
            &fake.ffmpeg,
            &input,
            MediaFormat::Mp4,
            None,
            &CancelToken::new(),
        );
        match result {
            Err(MorphixError::InvalidInput(msg)) => assert!(msg.contains("libx264"), "{msg}"),
            other => panic!("cần InvalidInput nêu libx264, được {other:?}"),
        }
        assert!(fake.conversions().is_empty(), "không được chạy chuyển đổi");
        assert_eq!(user_files(fake.dir()), ["clip.mkv"]);
    }

    #[test]
    fn convert_mp4_runs_exact_encoder_argv_and_writes_output() {
        let _lock = serial();
        // WHY VP9_OPUS: MP4 không nhận nguyên VP9 hay Opus, nên argv vẫn là encoder đầy đủ như trước khi có remux.
        let fake = Fake::new(FULL_ENCODERS, VP9_OPUS, "printf encoded > \"$out\"");
        let input: PathBuf = fake.input("clip.mkv", MKV_HEAD);
        let output: PathBuf = convert_media(
            &fake.ffmpeg,
            &input,
            MediaFormat::Mp4,
            None,
            &CancelToken::new(),
        )
        .expect("chuyển sang MP4 phải thành công");
        assert_eq!(output, fake.dir().join("clip.mp4"));
        assert_eq!(std::fs::read(&output).expect("đọc đầu ra"), b"encoded");

        let conversions: Vec<Vec<String>> = fake.conversions();
        assert_eq!(conversions.len(), 1);
        let argv: &[String] = &conversions[0];
        let input_arg: String = input.display().to_string();
        let expected_prefix: Vec<&str> = vec![
            "-hide_banner",
            "-nostdin",
            "-n",
            "-i",
            input_arg.as_str(),
            "-sn",
            "-dn",
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
        assert_eq!(argv[..argv.len() - 1], expected_prefix[..]);
        assert!(argv.last().expect("có đường dẫn đầu ra").ends_with(".mp4"));
    }

    #[test]
    fn convert_h264_aac_source_to_mp4_copies_both_streams() {
        let _lock = serial();
        let fake = Fake::new(FULL_ENCODERS, WITH_AUDIO, "printf copied > \"$out\"");
        let input: PathBuf = fake.input("clip.mkv", MKV_HEAD);
        convert_media(
            &fake.ffmpeg,
            &input,
            MediaFormat::Mp4,
            None,
            &CancelToken::new(),
        )
        .expect("chuyển sang MP4 phải thành công");

        let conversions: Vec<Vec<String>> = fake.conversions();
        assert_eq!(conversions.len(), 1);
        let argv: &[String] = &conversions[0];
        let input_arg: String = input.display().to_string();
        let expected_prefix: Vec<&str> = vec![
            "-hide_banner",
            "-nostdin",
            "-n",
            "-i",
            input_arg.as_str(),
            "-sn",
            "-dn",
            "-c:v",
            "copy",
            "-c:a",
            "copy",
            "-movflags",
            "+faststart",
        ];
        assert_eq!(argv[..argv.len() - 1], expected_prefix[..]);
    }

    #[test]
    fn extract_audio_from_video_without_audio_stream_creates_no_file() {
        let _lock = serial();
        let fake = Fake::new(FULL_ENCODERS, VIDEO_ONLY, "printf x > \"$out\"");
        let input: PathBuf = fake.input("clip.mp4", MP4_HEAD);
        match extract_audio(
            &fake.ffmpeg,
            &input,
            MediaFormat::Mp3,
            None,
            &CancelToken::new(),
        ) {
            Err(MorphixError::InvalidInput(msg)) => {
                assert!(msg.contains("không có âm thanh"), "{msg}")
            }
            other => panic!("cần InvalidInput 'không có âm thanh', được {other:?}"),
        }
        assert!(fake.conversions().is_empty());
        assert_eq!(user_files(fake.dir()), ["clip.mp4"]);
    }

    #[test]
    fn extract_audio_with_audio_stream_drops_video() {
        let _lock = serial();
        let fake = Fake::new(FULL_ENCODERS, WITH_AUDIO, "printf audio > \"$out\"");
        let input: PathBuf = fake.input("clip.mp4", MP4_HEAD);
        let output: PathBuf = extract_audio(
            &fake.ffmpeg,
            &input,
            MediaFormat::Mp3,
            None,
            &CancelToken::new(),
        )
        .expect("video có âm thanh phải tách được");
        assert_eq!(output, fake.dir().join("clip.mp3"));
        assert_eq!(std::fs::read(&output).expect("đọc đầu ra"), b"audio");

        let conversions: Vec<Vec<String>> = fake.conversions();
        let argv: &[String] = &conversions[0];
        assert_eq!(value_after(argv, "-c:a"), "libmp3lame");
        assert_eq!(argv[argv.len() - 2], "-vn");
    }

    #[test]
    fn extract_audio_rejects_video_target_before_calling_ffmpeg() {
        let _lock = serial();
        let fake = Fake::new(FULL_ENCODERS, WITH_AUDIO, "printf x > \"$out\"");
        let input: PathBuf = fake.input("clip.mp4", MP4_HEAD);
        let result = extract_audio(
            &fake.ffmpeg,
            &input,
            MediaFormat::Mp4,
            None,
            &CancelToken::new(),
        );
        assert!(matches!(result, Err(MorphixError::InvalidInput(_))));
        assert!(fake.calls().is_empty());
    }

    #[test]
    fn cut_passes_start_and_end_as_output_options() {
        let _lock = serial();
        let fake = Fake::new(FULL_ENCODERS, WITH_AUDIO, "printf cut > \"$out\"");
        let input: PathBuf = fake.input("clip.mp4", MP4_HEAD);
        let output: PathBuf =
            cut_media(&fake.ffmpeg, &input, 2000, 5000, None, &CancelToken::new())
                .expect("cắt 2 s đến 5 s phải thành công");
        assert_eq!(output, fake.dir().join("clip-cat.mp4"));

        let conversions: Vec<Vec<String>> = fake.conversions();
        let argv: &[String] = &conversions[0];
        assert_eq!(value_after(argv, "-ss"), "00:00:02.000");
        assert_eq!(value_after(argv, "-to"), "00:00:05.000");
    }

    #[test]
    fn cut_rejects_end_not_after_start() {
        let _lock = serial();
        let fake = Fake::new(FULL_ENCODERS, WITH_AUDIO, "printf x > \"$out\"");
        let input: PathBuf = fake.input("clip.mp4", MP4_HEAD);
        let result = cut_media(&fake.ffmpeg, &input, 5000, 5000, None, &CancelToken::new());
        assert!(matches!(result, Err(MorphixError::InvalidInput(_))));
        assert!(fake.calls().is_empty());
    }

    #[test]
    fn atempo_chains_only_outside_single_filter_range() {
        assert_eq!(atempo_filter(4000), "atempo=2.0,atempo=2.0");
        assert_eq!(atempo_filter(1500), "atempo=1.5");
        assert_eq!(atempo_filter(250), "atempo=0.5,atempo=0.5");
        assert_eq!(atempo_filter(1000), "atempo=1.0");
    }

    #[test]
    fn speed_4000_applies_atempo_chain_and_setpts_to_video() {
        let _lock = serial();
        let fake = Fake::new(FULL_ENCODERS, WITH_AUDIO, "printf fast > \"$out\"");
        let input: PathBuf = fake.input("clip.mp4", MP4_HEAD);
        let output: PathBuf = change_speed(&fake.ffmpeg, &input, 4000, None, &CancelToken::new())
            .expect("đổi tốc độ 4x phải thành công");
        assert_eq!(output, fake.dir().join("clip-speed.mp4"));

        let conversions: Vec<Vec<String>> = fake.conversions();
        let argv: &[String] = &conversions[0];
        assert_eq!(value_after(argv, "-filter:a"), "atempo=2.0,atempo=2.0");
        assert_eq!(value_after(argv, "-filter:v"), "setpts=PTS/4.0");
    }

    #[test]
    fn speed_video_without_audio_stream_has_no_audio_filter() {
        let _lock = serial();
        let fake = Fake::new(FULL_ENCODERS, VIDEO_ONLY, "printf x > \"$out\"");
        let input: PathBuf = fake.input("clip.mp4", MP4_HEAD);
        change_speed(&fake.ffmpeg, &input, 1500, None, &CancelToken::new())
            .expect("video không có âm thanh vẫn đổi tốc độ được");
        let conversions: Vec<Vec<String>> = fake.conversions();
        let argv: &[String] = &conversions[0];
        assert!(!argv.contains(&"-filter:a".to_string()));
        assert_eq!(value_after(argv, "-filter:v"), "setpts=PTS/1.5");
    }

    #[test]
    fn speed_zero_factor_is_invalid_input() {
        let _lock = serial();
        let fake = Fake::new(FULL_ENCODERS, WITH_AUDIO, "printf x > \"$out\"");
        let input: PathBuf = fake.input("clip.mp4", MP4_HEAD);
        let result = change_speed(&fake.ffmpeg, &input, 0, None, &CancelToken::new());
        assert!(matches!(result, Err(MorphixError::InvalidInput(_))));
        assert!(fake.calls().is_empty());
    }

    #[test]
    fn compress_keeps_format_when_output_is_smaller() {
        let _lock = serial();
        let fake = Fake::new(FULL_ENCODERS, WITH_AUDIO, "printf x > \"$out\"");
        let input: PathBuf = fake.input("clip.mp4", MP4_HEAD);
        let output: PathBuf = compress_media(&fake.ffmpeg, &input, None, &CancelToken::new())
            .expect("đầu ra nhỏ hơn phải được giữ");
        assert_eq!(output, fake.dir().join("clip-nen.mp4"));
        let conversions: Vec<Vec<String>> = fake.conversions();
        assert_eq!(value_after(&conversions[0], "-c:v"), "libx264");
    }

    #[test]
    fn compress_not_smaller_is_refused_and_leaves_no_file() {
        let _lock = serial();
        let fake = Fake::new(
            FULL_ENCODERS,
            WITH_AUDIO,
            "head -c 1000 /dev/zero > \"$out\"",
        );
        let input: PathBuf = fake.input("clip.mp4", MP4_HEAD);
        let result = compress_media(&fake.ffmpeg, &input, None, &CancelToken::new());
        assert!(matches!(result, Err(MorphixError::NotSmaller { .. })));
        assert_eq!(user_files(fake.dir()), ["clip.mp4"]);
    }

    #[test]
    fn cancel_mid_run_removes_temp_output_and_keeps_input() {
        let _lock = serial();
        // WHY: `exec` thay sh bằng sleep, để huỷ giết đúng tiến trình và đường ống stderr đóng.
        let fake = Fake::new(FULL_ENCODERS, WITH_AUDIO, "exec sleep 5");
        let input: PathBuf = fake.input("clip.mp4", MP4_HEAD);
        let before: Vec<u8> = std::fs::read(&input).expect("đọc đầu vào");
        let cancel = CancelToken::new();
        let trigger: CancelToken = cancel.clone();
        let timer = thread::spawn(move || {
            thread::sleep(Duration::from_millis(200));
            trigger.cancel();
        });
        let result = cut_media(&fake.ffmpeg, &input, 0, 1000, None, &cancel);
        timer.join().expect("luồng huỷ");
        assert!(matches!(result, Err(MorphixError::Cancelled)), "{result:?}");
        assert_eq!(user_files(fake.dir()), ["clip.mp4"]);
        assert_eq!(std::fs::read(&input).expect("đọc đầu vào"), before);
    }

    #[test]
    fn corrupt_probe_reports_input_path() {
        let _lock = serial();
        let fake = Fake::new(FULL_ENCODERS, NO_STREAMS, "printf x > \"$out\"");
        let input: PathBuf = fake.input("clip.mp4", MP4_HEAD);
        match has_audio_stream(&fake.ffmpeg, &input) {
            Err(MorphixError::Corrupt { path, .. }) => assert_eq!(path, input),
            other => panic!("cần Corrupt kèm đường dẫn đầu vào, được {other:?}"),
        }
    }

    #[test]
    fn mp3_converts_to_wav_with_matching_name() {
        let _lock = serial();
        let fake = Fake::new(FULL_ENCODERS, WITH_AUDIO, "printf x > \"$out\"");
        let input: PathBuf = fake.input("song.mp3", MP3_HEAD);
        let output: PathBuf = convert_media(
            &fake.ffmpeg,
            &input,
            MediaFormat::Wav,
            None,
            &CancelToken::new(),
        )
        .expect("MP3 sang WAV phải thành công");
        assert_eq!(output, fake.dir().join("song.wav"));
    }

    #[test]
    fn every_operation_leaves_input_unchanged() {
        let _lock = serial();
        let fake = Fake::new(FULL_ENCODERS, WITH_AUDIO, "printf x > \"$out\"");
        let input: PathBuf = fake.input("clip.mp4", MP4_HEAD);
        let before: Vec<u8> = std::fs::read(&input).expect("đọc đầu vào");
        let cancel = CancelToken::new();
        let ffmpeg: &Path = &fake.ffmpeg;
        convert_media(ffmpeg, &input, MediaFormat::Mkv, None, &cancel).expect("chuyển MKV");
        compress_media(ffmpeg, &input, None, &cancel).expect("nén");
        cut_media(ffmpeg, &input, 0, 1000, None, &cancel).expect("cắt");
        change_speed(ffmpeg, &input, 2000, None, &cancel).expect("đổi tốc độ");
        extract_audio(ffmpeg, &input, MediaFormat::Mp3, None, &cancel).expect("tách âm thanh");
        assert_eq!(std::fs::read(&input).expect("đọc đầu vào"), before);
    }
}

/// Các file người dùng còn trong thư mục, bỏ qua file phụ của script giả.
#[cfg(unix)]
fn user_files(dir: &Path) -> Vec<String> {
    const FAKE_FILES: [&str; 4] = ["argv.log", "encoders.txt", "fake-ffmpeg", "streams.txt"];
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .expect("đọc thư mục")
        .map(|entry| {
            entry
                .expect("đọc mục thư mục")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .filter(|name: &String| !FAKE_FILES.contains(&name.as_str()))
        .collect();
    names.sort();
    names
}

#[test]
fn argv_selects_only_streams_the_target_can_hold() {
    let flags = |target: MediaFormat, out: &str| -> Vec<String> {
        argv_strings(
            &convert_args(Path::new("/tmp/in.mkv"), target, Path::new(out)).expect("có encoder"),
        )
    };
    // WHY: phụ đề ảnh làm hỏng MP4; bìa album thành video trong OGG; video sang MP3 phải bỏ hình.
    let mp4 = flags(MediaFormat::Mp4, "/tmp/o.mp4");
    assert!(mp4.contains(&"-sn".to_string()) && !mp4.contains(&"-vn".to_string()));
    for (target, out) in [
        (MediaFormat::Mp3, "/tmp/o.mp3"),
        (MediaFormat::Ogg, "/tmp/o.ogg"),
    ] {
        let argv = flags(target, out);
        assert!(
            argv.contains(&"-vn".to_string()) && argv.contains(&"-sn".to_string()),
            "{target:?}"
        );
    }
    assert!(flags(MediaFormat::Gif, "/tmp/o.gif").contains(&"-an".to_string()));
}
