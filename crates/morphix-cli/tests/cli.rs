use std::io::Write;
use std::path::{Path, PathBuf};

use assert_cmd::{assert::Assert, cargo::cargo_bin_cmd};
use image::{codecs::jpeg::JpegEncoder, Rgb, RgbImage};
use pdf_writer::{Content, Finish, Name, Pdf, Rect, Ref, Str};
use predicates::{prelude::PredicateBooleanExt, str::contains};
use tempfile::{tempdir, TempDir};

/// Thư mục làm việc tạm kèm thư mục dữ liệu riêng, để test không đụng dữ liệu thật của người dùng.
struct Sandbox {
    work: TempDir,
    data: TempDir,
}

impl Sandbox {
    fn new() -> Self {
        Sandbox {
            work: tempdir().unwrap(),
            data: tempdir().unwrap(),
        }
    }

    fn path(&self, name: &str) -> PathBuf {
        self.work.path().join(name)
    }

    fn run(&self, args: &[&str]) -> Assert {
        cargo_bin_cmd!("morphix")
            .current_dir(self.work.path())
            .env("MORPHIX_DATA_DIR", self.data.path())
            .args(args)
            .assert()
    }

    fn jpeg(&self, name: &str, w: u32, h: u32, quality: u8, noisy: bool) -> &Self {
        let mut state: u32 = 0x9E37_79B9;
        let img = RgbImage::from_fn(w, h, |x, y| {
            if noisy {
                state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                Rgb([(state >> 24) as u8, (state >> 16) as u8, (state >> 8) as u8])
            } else {
                Rgb([(x * 7 % 256) as u8, (y * 11 % 256) as u8, 128])
            }
        });
        let mut buf = Vec::new();
        JpegEncoder::new_with_quality(&mut buf, quality)
            .encode_image(&img)
            .unwrap();
        std::fs::write(self.path(name), buf).unwrap();
        self
    }

    fn png(&self, name: &str, w: u32, h: u32) -> &Self {
        RgbImage::from_pixel(w, h, Rgb([10, 120, 200]))
            .save(self.path(name))
            .unwrap();
        self
    }

    /// Như `run`, nhưng dùng ffmpeg giả ở `ffmpeg` qua `MORPHIX_FFMPEG`.
    #[cfg(unix)]
    fn cmd_with_ffmpeg(&self, ffmpeg: &Path) -> assert_cmd::Command {
        let mut cmd = cargo_bin_cmd!("morphix");
        cmd.current_dir(self.work.path())
            .env("MORPHIX_DATA_DIR", self.data.path())
            .env("MORPHIX_FFMPEG", ffmpeg);
        cmd
    }

    #[cfg(unix)]
    fn run_with_ffmpeg(&self, ffmpeg: &Path, args: &[&str]) -> Assert {
        self.cmd_with_ffmpeg(ffmpeg).args(args).assert()
    }

    fn empty(&self, name: &str) -> &Self {
        std::fs::write(self.path(name), b"").unwrap();
        self
    }
}

fn page_count(pdf: &Path) -> usize {
    lopdf::Document::load(pdf).unwrap().get_pages().len()
}

#[test]
fn merge_three_images_to_one_pdf() {
    let s = Sandbox::new();
    s.jpeg("a.jpg", 40, 30, 90, false)
        .jpeg("b.jpg", 30, 40, 90, false)
        .png("c.png", 20, 20);
    s.run(&[
        "convert", "a.jpg", "b.jpg", "c.png", "--to", "pdf", "--merge",
    ])
    .success()
    .stdout(contains("a-gop.pdf"));
    assert_eq!(page_count(&s.path("a-gop.pdf")), 3);
}

#[test]
fn pdf_without_merge_makes_one_pdf_per_image() {
    let s = Sandbox::new();
    s.jpeg("a.jpg", 40, 30, 90, false).png("b.png", 20, 20);
    s.run(&["convert", "a.jpg", "b.png", "--to", "pdf"])
        .success();
    assert_eq!(page_count(&s.path("a.pdf")), 1);
    assert_eq!(page_count(&s.path("b.pdf")), 1);
}

#[test]
fn partial_failure_exits_3_and_still_converts_the_rest() {
    let s = Sandbox::new();
    s.png("good.png", 8, 8).empty("zero.png");
    s.run(&["convert", "good.png", "zero.png", "--to", "webp"])
        .code(3)
        .stderr(contains("zero.png"));
    assert!(s.path("good.webp").exists());
}

#[test]
fn all_failures_exit_1_with_cause_on_stderr() {
    let s = Sandbox::new();
    s.empty("zero.png");
    s.run(&["convert", "zero.png", "--to", "webp"])
        .code(1)
        .stderr(contains("zero.png"));
}

#[test]
fn usage_errors_exit_2() {
    let s = Sandbox::new();
    s.png("a.png", 8, 8);
    s.run(&["convert", "a.png"]).code(2);
    s.run(&["convert", "a.png", "--to", "png", "--merge"])
        .code(2);
    s.run(&["resize", "a.png"]).code(2);
}

#[test]
fn json_output_is_a_single_parsable_object() {
    let s = Sandbox::new();
    s.png("a.png", 8, 8).empty("zero.png");
    let assert = s
        .run(&["--json", "convert", "a.png", "zero.png", "--to", "webp"])
        .code(3);
    let value: serde_json::Value = serde_json::from_slice(&assert.get_output().stdout).unwrap();
    assert_eq!(
        (value["ok"].as_u64(), value["failed"].as_u64()),
        (Some(1), Some(1))
    );
    let results = value["results"].as_array().unwrap();
    assert!(results[0]["output"].as_str().unwrap().ends_with("a.webp"));
    assert!(results[0]["error"].is_null());
    assert!(results[1]["output"].is_null());
    assert!(results[1]["error"].as_str().unwrap().contains("zero.png"));
}

#[test]
fn dry_run_prints_plan_and_creates_nothing() {
    let s = Sandbox::new();
    s.jpeg("a.jpg", 16, 16, 90, false);
    s.run(&["--dry-run", "convert", "a.jpg", "--to", "png"])
        .success()
        .stdout(contains("a.png"));
    let names: Vec<_> = std::fs::read_dir(s.work.path())
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert_eq!(names.len(), 1, "{names:?}");
}

#[test]
fn strip_meta_compress_resize_produce_expected_names() {
    let s = Sandbox::new();
    s.jpeg("a.jpg", 100, 200, 90, false)
        .jpeg("noisy.jpg", 400, 300, 98, true);
    s.run(&["strip-meta", "a.jpg"]).success();
    assert!(s.path("a-sach.jpg").exists());
    s.run(&["--quality", "small", "compress", "noisy.jpg"])
        .success();
    assert!(s.path("noisy-nen.jpg").exists());
    s.run(&["resize", "a.jpg", "--percent", "50"]).success();
    assert!(s.path("a-50x100.jpg").exists());
}

#[test]
fn incompressible_file_reports_cannot_shrink() {
    let s = Sandbox::new();
    s.jpeg("small.jpg", 8, 8, 20, true);
    s.run(&["--quality", "high", "compress", "small.jpg"])
        .code(1)
        .stderr(contains("không nén thêm được"));
    assert!(!s.path("small-nen.jpg").exists());
}

#[test]
fn failures_are_written_to_the_log_file() {
    let s = Sandbox::new();
    s.empty("zero.png");
    s.run(&["convert", "zero.png", "--to", "webp"]).code(1);
    let log = std::fs::read_to_string(s.data.path().join("morphix.log")).unwrap();
    assert!(log.contains("zero.png"), "{log}");
}

fn stdout_of(assert: &Assert) -> String {
    String::from_utf8(assert.get_output().stdout.clone()).unwrap()
}

#[test]
fn formats_for_a_jpeg_lists_pdf_first_and_hides_jpg() {
    let s = Sandbox::new();
    s.jpeg("a.jpg", 16, 16, 90, false);
    let out = stdout_of(&s.run(&["formats", "a.jpg"]).success());
    assert_eq!(out.lines().next(), Some("convert\tPDF"));
    assert!(!out.contains("JPG"), "{out}");
    assert!(out.contains("tool\tNén ảnh"), "{out}");
}

/// Ghi zip có các entry đúng tên đã cho, để test giải nén và đọc lại qua CLI.
fn write_zip(path: &Path, entries: &[(&str, &[u8])]) {
    let mut zip = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
    let options: zip::write::SimpleFileOptions = zip::write::SimpleFileOptions::default();
    for (name, bytes) in entries {
        zip.start_file(*name, options).unwrap();
        zip.write_all(bytes).unwrap();
    }
    zip.finish().unwrap();
}

#[test]
fn formats_for_mixed_image_and_zip_lists_only_archive_targets() {
    // WHY: có file không phải file nén thì catalog đề nghị nén gộp; trước đây CLI giấu việc này.
    let s = Sandbox::new();
    s.png("a.png", 8, 8);
    write_zip(&s.path("b.zip"), &[("x.txt", b"x")]);
    let out = stdout_of(&s.run(&["formats", "a.png", "b.zip"]).success());
    assert_eq!(
        out,
        "convert\tNén thành ZIP\nconvert\tNén thành TAR.GZ\nconvert\tNén thành 7Z\n"
    );
}

#[test]
fn formats_for_rar_offers_nothing_and_exits_1() {
    // WHY: RAR chỉ đọc sau đợt 4, nên không đề nghị giải nén RAR.
    let s = Sandbox::new();
    std::fs::write(s.path("c.rar"), b"Rar!\x1A\x07\x00\xCF\x90").unwrap();
    s.run(&["formats", "c.rar"])
        .code(1)
        .stderr(contains("không có việc nào"));
}

#[test]
fn formats_for_a_zip_lists_extract_from_the_catalog() {
    let s = Sandbox::new();
    write_zip(&s.path("a.zip"), &[("b.txt", b"x")]);
    let out = stdout_of(&s.run(&["formats", "a.zip"]).success());
    assert_eq!(out, "convert\tGiải nén\n");
}

#[test]
fn archive_two_text_files_writes_nen_zip_next_to_first_input() {
    let s = Sandbox::new();
    std::fs::write(s.path("a.txt"), "xin chào\n").unwrap();
    std::fs::write(s.path("b.txt"), "dòng hai\n").unwrap();
    s.run(&["archive", "a.txt", "b.txt", "--to", "zip"])
        .success();
    let mut zip = zip::ZipArchive::new(std::fs::File::open(s.path("a-nen.zip")).unwrap()).unwrap();
    let mut names: Vec<String> = Vec::new();
    for index in 0..zip.len() {
        let entry = zip.by_index(index).unwrap();
        names.push(entry.name().unwrap().into_owned());
    }
    names.sort_unstable();
    assert_eq!(names, ["a.txt", "b.txt"]);
}

#[test]
fn archive_rar_target_is_usage_error_and_creates_nothing() {
    let s = Sandbox::new();
    std::fs::write(s.path("a.txt"), "x\n").unwrap();
    s.run(&["archive", "a.txt", "--to", "rar"])
        .code(2)
        .stderr(contains("RAR"));
    assert!(!s.path("a-nen.rar").exists());
}

#[test]
fn archive_without_inputs_is_usage_error() {
    let s = Sandbox::new();
    s.run(&["archive", "--to", "zip"]).code(2);
}

#[test]
fn extract_zip_writes_folder_next_to_archive_and_keeps_archive() {
    let s = Sandbox::new();
    write_zip(&s.path("a.zip"), &[("b.txt", b"noi dung")]);
    let before = std::fs::read(s.path("a.zip")).unwrap();
    s.run(&["extract", "a.zip"]).success();
    assert_eq!(std::fs::read(s.path("a/b.txt")).unwrap(), b"noi dung");
    assert_eq!(std::fs::read(s.path("a.zip")).unwrap(), before);
}

#[test]
fn extract_non_archive_exits_2_and_creates_nothing() {
    let s = Sandbox::new();
    s.jpeg("photo.jpg", 16, 16, 90, false);
    s.run(&["extract", "photo.jpg"])
        .code(2)
        .stderr(contains("không phải"));
    assert!(!s.path("photo").exists());
}

#[test]
fn extract_traversal_zip_exits_1_and_writes_nothing_outside() {
    let s = Sandbox::new();
    write_zip(
        &s.path("evil.zip"),
        &[("ok.txt", b"ok"), ("../escape.txt", b"hai")],
    );
    s.run(&["extract", "evil.zip"]).code(1);
    assert!(!s.path("escape.txt").exists(), "ghi ra ngoài thư mục đích");
    // WHY: chỉ còn file nén gốc; thư mục tạm và thư mục đích phải đã bị xoá.
    assert_eq!(std::fs::read_dir(s.work.path()).unwrap().count(), 1);
}

#[test]
fn formats_without_files_lists_supported_targets() {
    let s = Sandbox::new();
    s.run(&["formats"])
        .success()
        .stdout(contains("PDF"))
        .stdout(contains("AVIF"));
}

/// Exit doctor khi chưa cài tích hợp: Linux kiểm được và thấy `missing` (0); nơi khác không kiểm được gì (3).
const DOCTOR_EMPTY_EXIT_CODE: i32 = if cfg!(target_os = "linux") { 0 } else { 3 };

/// HOME và XDG nằm trong sandbox để doctor không đọc cấu hình tích hợp thật của người dùng.
fn doctor_isolated_from_user_integration(s: &Sandbox) -> assert_cmd::Command {
    let mut command = cargo_bin_cmd!("morphix");
    // WHY: phiên GNOME của máy chạy test không được đưa vào kết quả, nếu không doctor sẽ kiểm AppIndicator và thành `unknown`.
    command
        .env("HOME", s.path("home"))
        .env("XDG_DATA_HOME", s.path("xdg-data"))
        .env("XDG_CONFIG_HOME", s.path("xdg-config"))
        .env_remove("XDG_CURRENT_DESKTOP")
        .env_remove("DBUS_SESSION_BUS_ADDRESS");
    command
}

#[test]
fn doctor_reports_ffmpeg_and_data_dir_and_exits_3_without_integration_backend() {
    let s = Sandbox::new();
    // WHY: mục tích hợp chưa cài không phải lỗi; trên nền tảng không có backend thì exit 3 vì không kiểm được gì.
    let assert = doctor_isolated_from_user_integration(&s)
        .env("MORPHIX_DATA_DIR", s.data.path())
        .env("PATH", "") // chắc chắn không có ffmpeg
        .arg("doctor")
        .assert()
        .code(DOCTOR_EMPTY_EXIT_CODE);
    let out = stdout_of(&assert);
    assert!(out.contains("ffmpeg: không tìm thấy"), "{out}");
    assert!(
        out.contains("thư mục dữ liệu:") && out.contains("ghi được"),
        "{out}"
    );
}

#[cfg(unix)]
#[test]
fn doctor_shows_the_ffmpeg_it_finds() {
    use std::os::unix::fs::PermissionsExt;
    let s = Sandbox::new();
    let fake = s.path("ffmpeg");
    std::fs::write(&fake, "#!/bin/sh\n").unwrap();
    std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();
    let assert = doctor_isolated_from_user_integration(&s)
        .env("MORPHIX_DATA_DIR", s.data.path())
        .env("PATH", s.work.path())
        .arg("doctor")
        .assert()
        .code(DOCTOR_EMPTY_EXIT_CODE);
    assert!(stdout_of(&assert).contains(&fake.display().to_string()));
}

#[test]
fn doctor_exits_1_when_data_dir_is_not_writable() {
    let s = Sandbox::new();
    let not_a_dir = s.path("plain-file");
    std::fs::write(&not_a_dir, b"x").unwrap();
    let assert = cargo_bin_cmd!("morphix")
        .env("MORPHIX_DATA_DIR", &not_a_dir)
        .arg("doctor")
        .assert()
        .code(1);
    assert!(stdout_of(&assert).contains("KHÔNG ghi được"));
}

const SRT_SAMPLE: &str = "1\n00:00:01,000 --> 00:00:02,500\nXin chào\n\n2\n00:00:03,000 --> 00:00:04,000\nDòng hai\nDòng ba\n\n";

#[test]
fn convert_srt_to_vtt_writes_next_to_input_and_keeps_source() {
    let s = Sandbox::new();
    std::fs::write(s.path("sub.srt"), SRT_SAMPLE).unwrap();
    s.run(&["convert", "sub.srt", "--to", "vtt"])
        .success()
        .stdout(contains("sub.vtt"));
    let vtt = std::fs::read_to_string(s.path("sub.vtt")).unwrap();
    assert!(vtt.starts_with("WEBVTT"), "{vtt}");
    assert!(vtt.contains("00:00:01.000 --> 00:00:02.500"), "{vtt}");
    assert_eq!(
        std::fs::read_to_string(s.path("sub.srt")).unwrap(),
        SRT_SAMPLE
    );
}

#[test]
fn csv_cell_with_tab_to_tsv_exits_1_and_names_the_row() {
    let s = Sandbox::new();
    std::fs::write(s.path("a.csv"), "tên,ghi chú\nphở,\"x\ty\"\n").unwrap();
    s.run(&["convert", "a.csv", "--to", "tsv"])
        .code(1)
        .stderr(contains("dòng 2"));
    assert!(!s.path("a.tsv").exists());
}

#[test]
fn text_pair_outside_its_group_exits_2_with_vietnamese_reason() {
    let s = Sandbox::new();
    std::fs::write(s.path("a.txt"), "xin chào\n").unwrap();
    s.run(&["convert", "a.txt", "--to", "vtt"])
        .code(2)
        .stderr(contains("không chuyển được TXT sang VTT"));
}

#[test]
fn convert_text_without_to_is_a_usage_error() {
    let s = Sandbox::new();
    std::fs::write(s.path("sub.srt"), SRT_SAMPLE).unwrap();
    s.run(&["convert", "sub.srt"]).code(2);
}

#[test]
fn text_target_with_merge_is_a_usage_error() {
    let s = Sandbox::new();
    std::fs::write(s.path("sub.srt"), SRT_SAMPLE).unwrap();
    s.run(&["convert", "sub.srt", "--to", "vtt", "--merge"])
        .code(2);
}

#[test]
fn formats_for_srt_lists_the_text_targets_from_the_catalog() {
    let s = Sandbox::new();
    std::fs::write(s.path("sub.srt"), SRT_SAMPLE).unwrap();
    let out = stdout_of(&s.run(&["formats", "sub.srt"]).success());
    // WHY: SRT sang SRT là việc vô nghĩa, catalog bỏ đích trùng nguồn như với ảnh.
    // WHY: file phụ đề không phải file nén nên catalog cũng đề nghị ba đích nén (việc này trước bị CLI giấu).
    assert_eq!(
        out,
        "convert\tVTT\nconvert\tTXT\nconvert\tNén thành ZIP\nconvert\tNén thành TAR.GZ\nconvert\tNén thành 7Z\n"
    );
}

#[test]
fn image_to_text_target_is_unsupported_exits_2_and_creates_nothing() {
    let s = Sandbox::new();
    s.jpeg("photo.jpg", 16, 16, 90, false);
    s.run(&["convert", "photo.jpg", "--to", "srt"]).code(2);
    assert!(!s.path("photo.srt").exists());
}

#[test]
fn srt_to_csv_is_unsupported_exits_2_and_creates_nothing() {
    let s = Sandbox::new();
    std::fs::write(s.path("sub.srt"), SRT_SAMPLE).unwrap();
    s.run(&["convert", "sub.srt", "--to", "csv"]).code(2);
    assert!(!s.path("sub.csv").exists());
}

/// Đầu MP4 tối thiểu để `detect` nhận đúng video; nội dung không cần chạy được thật.
const MP4_HEAD: &[u8] = b"\x00\x00\x00\x18ftypisom\x00\x00\x02\x00";
/// Đầu ID3 của MP3; `detect` nhận đúng âm thanh.
const MP3_HEAD: &[u8] = b"ID3\x04\x00\x00\x00\x00\x00\x00";

/// ffmpeg giả cho test, chỉ qua `MORPHIX_FFMPEG`. Ghi argv vào `argv.log` cạnh nó, trả lời `-encoders`,
/// báo luồng âm thanh theo `FAKE_FFMPEG_NO_AUDIO`, và tạo file đầu ra có nội dung giả.
/// WHY: test không phụ thuộc ffmpeg thật trên máy; log argv cho phép kiểm đúng tham số đã truyền.
#[cfg(unix)]
fn fake_ffmpeg(dir: &Path) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let path = dir.join("ffmpeg");
    let script = r#"#!/bin/sh
echo "$*" >> "$(dirname "$0")/argv.log"
case "$*" in
  *-encoders*)
    printf 'Encoders:\n V..... = Video\n A..... = Audio\n ------\n V....D libx264 H.264\n A....D aac AAC\n A....D libopus Opus\n V....D libvpx-vp9 VP9\n A....D libmp3lame MP3\n A....D flac FLAC\n A....D libvorbis Vorbis\n A....D pcm_s16le PCM\n V....D gif GIF\n'
    exit 0 ;;
  "-hide_banner -nostdin -i "*)
    echo "  Stream #0:0: Video: h264" >&2
    if [ -z "$FAKE_FFMPEG_NO_AUDIO" ]; then echo "  Stream #0:1: Audio: aac" >&2; fi
    exit 1 ;;
esac
for last; do :; done
printf 'fake' > "$last"
"#;
    std::fs::write(&path, script).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    path
}

#[cfg(unix)]
fn argv_log(dir: &Path) -> String {
    std::fs::read_to_string(dir.join("argv.log")).unwrap_or_default()
}

#[cfg(unix)]
#[test]
fn cut_passes_seconds_as_timestamps_and_writes_output() {
    let s = Sandbox::new();
    std::fs::write(s.path("a.mp4"), MP4_HEAD).unwrap();
    let fake = fake_ffmpeg(s.work.path());
    s.run_with_ffmpeg(&fake, &["cut", "a.mp4", "--from", "2", "--to", "5"])
        .success();
    let log = argv_log(s.work.path());
    assert!(log.contains("-ss 00:00:02.000"), "{log}");
    assert!(log.contains("-to 00:00:05.000"), "{log}");
    assert!(s.path("a-cat.mp4").exists());
}

#[test]
fn cut_end_not_after_start_is_usage_error_and_creates_nothing() {
    let s = Sandbox::new();
    std::fs::write(s.path("a.mp4"), MP4_HEAD).unwrap();
    s.run(&["cut", "a.mp4", "--from", "5", "--to", "2"]).code(2);
    assert!(!s.path("a-cat.mp4").exists());
}

#[test]
fn speed_factor_zero_is_usage_error() {
    let s = Sandbox::new();
    std::fs::write(s.path("a.mp4"), MP4_HEAD).unwrap();
    s.run(&["speed", "a.mp4", "--factor", "0"]).code(2);
}

#[test]
fn audio_to_video_is_unsupported_exits_2_and_creates_nothing() {
    let s = Sandbox::new();
    std::fs::write(s.path("a.mp3"), MP3_HEAD).unwrap();
    s.run(&["convert", "a.mp3", "--to", "mp4"]).code(2);
    assert!(!s.path("a.mp4").exists());
}

#[cfg(unix)]
#[test]
fn video_to_mkv_runs_convert_media_with_fake_ffmpeg() {
    let s = Sandbox::new();
    std::fs::write(s.path("a.mp4"), MP4_HEAD).unwrap();
    let fake = fake_ffmpeg(s.work.path());
    s.run_with_ffmpeg(&fake, &["convert", "a.mp4", "--to", "mkv"])
        .success();
    assert!(s.path("a.mkv").exists());
}

#[cfg(unix)]
#[test]
fn extract_audio_without_audio_stream_exits_1_and_creates_nothing() {
    let s = Sandbox::new();
    std::fs::write(s.path("a.mp4"), MP4_HEAD).unwrap();
    let fake = fake_ffmpeg(s.work.path());
    s.cmd_with_ffmpeg(&fake)
        .env("FAKE_FFMPEG_NO_AUDIO", "1")
        .args(["extract-audio", "a.mp4", "--to", "mp3"])
        .assert()
        .code(1)
        .stderr(contains("không có âm thanh"));
    assert!(!s.path("a.mp3").exists());
}

#[test]
fn media_command_without_ffmpeg_exits_1_and_suggests_apt() {
    let s = Sandbox::new();
    std::fs::write(s.path("a.mp4"), MP4_HEAD).unwrap();
    cargo_bin_cmd!("morphix")
        .current_dir(s.work.path())
        .env("MORPHIX_DATA_DIR", s.data.path())
        .env_remove("MORPHIX_FFMPEG")
        .env("PATH", s.work.path())
        .args(["cut", "a.mp4", "--from", "2", "--to", "5"])
        .assert()
        .code(1)
        .stderr(contains("chưa có ffmpeg").and(contains("sudo").not()));
}

#[test]
fn formats_for_mp4_lists_media_targets_from_the_catalog() {
    let s = Sandbox::new();
    std::fs::write(s.path("a.mp4"), MP4_HEAD).unwrap();
    let out = stdout_of(&s.run(&["formats", "a.mp4"]).success());
    assert!(out.contains("convert\tMP3\n"), "{out}");
    assert!(out.contains("convert\tWebM\n"), "{out}");
    assert!(!out.contains("MP4\n"), "{out}");
}

/// PDF một trang; `text` là None thì trang trống (không có lớp chữ). ASCII nên không cần ToUnicode.
fn pdf_with_text(text: Option<&str>) -> Vec<u8> {
    let mut pdf = Pdf::new();
    pdf.catalog(Ref::new(1)).pages(Ref::new(2));
    pdf.pages(Ref::new(2)).kids([Ref::new(3)]).count(1);
    let mut page = pdf.page(Ref::new(3));
    page.parent(Ref::new(2))
        .media_box(Rect::new(0.0, 0.0, 612.0, 792.0))
        .contents(Ref::new(5));
    page.resources().fonts().pair(Name(b"F1"), Ref::new(4));
    page.finish();
    pdf.type1_font(Ref::new(4)).base_font(Name(b"Helvetica"));
    let mut content = Content::new();
    if let Some(text) = text {
        content
            .begin_text()
            .set_font(Name(b"F1"), 12.0)
            .next_line(72.0, 700.0)
            .show(Str(text.as_bytes()))
            .end_text();
    }
    pdf.stream(Ref::new(5), &content.finish());
    pdf.finish()
}

#[test]
fn convert_pdf_to_txt_writes_next_to_input_and_keeps_source() {
    let s = Sandbox::new();
    let bytes = pdf_with_text(Some("Hello morphix"));
    std::fs::write(s.path("sach.pdf"), &bytes).unwrap();
    s.run(&["convert", "sach.pdf", "--to", "txt"])
        .success()
        .stdout(contains("sach.txt"));
    let text = std::fs::read_to_string(s.path("sach.txt")).unwrap();
    assert!(text.contains("Hello morphix"), "{text}");
    assert_eq!(std::fs::read(s.path("sach.pdf")).unwrap(), bytes);
}

#[test]
fn pdf_without_text_layer_exits_1_with_vietnamese_reason_and_creates_nothing() {
    let s = Sandbox::new();
    std::fs::write(s.path("scan.pdf"), pdf_with_text(None)).unwrap();
    s.run(&["convert", "scan.pdf", "--to", "txt"])
        .code(1)
        .stderr(contains("cần OCR"));
    assert!(!s.path("scan.txt").exists());
}

#[test]
fn pdf_to_md_is_unsupported_exits_2_and_creates_nothing() {
    let s = Sandbox::new();
    std::fs::write(s.path("sach.pdf"), pdf_with_text(Some("Hello"))).unwrap();
    s.run(&["convert", "sach.pdf", "--to", "md"]).code(2);
    assert!(!s.path("sach.md").exists());
}

#[test]
fn formats_for_pdf_lists_txt_and_archive_targets() {
    // WHY: PDF là file không nén nên catalog đề nghị cả ba đích nén (trước đây CLI giấu chúng).
    let s = Sandbox::new();
    std::fs::write(s.path("sach.pdf"), pdf_with_text(Some("Hello"))).unwrap();
    let out = stdout_of(&s.run(&["formats", "sach.pdf"]).success());
    assert_eq!(
        out,
        "convert\tTXT\nconvert\tNén thành ZIP\nconvert\tNén thành TAR.GZ\nconvert\tNén thành 7Z\n"
    );
}
