use std::collections::HashSet;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

use morphix_core::error::MorphixError;
use morphix_core::kind::MediaFormat;
use morphix_core::ops::media::{available_encoders, convert_args, encoder_for, target_available};
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
        let fake = Fake::new(FULL_ENCODERS, WITH_AUDIO, "printf encoded > \"$out\"");
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
