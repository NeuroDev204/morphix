use std::{
    fs::OpenOptions,
    path::PathBuf,
    process::{Child, ExitCode, Stdio},
    sync::Mutex,
    thread,
    time::Duration,
};

use clap::{Parser, Subcommand, ValueEnum};
use morphix_cli::{
    integration::{self, BinaryPaths, IntegrationOptions},
    output::{self, DoctorReport},
    self_uninstall::{is_user_install, remove_user_install},
    uri::file_uris_to_paths,
};
use morphix_core::{
    actions_for,
    catalog::text_pair_supported,
    check_dir_writable, data_dir, detect, find_ffmpeg,
    kind::{ArchiveFormat, FileKind, MediaFormat, TextFormat},
    planned_outputs, run_job, ActionKind, CancelToken, ImgFormat, ItemReport, Job, MorphixError,
    Operation, Quality, ResizeSpec,
};
use morphix_ipc::{send_request, socket_name, MenuRequest};

/// Số lần thử kết nối lại sau khi khởi động GUI. WHY: GUI cần vài trăm ms để dựng Tauri và mở socket.
/// WHY khoảng 15 giây: lần khởi động nguội của GUI (WebKit, máy không có GPU) có thể mất vài giây trước khi
/// mở socket; 2 giây như trước khiến menu báo lỗi dù GUI vẫn đang khởi động.
const CONNECT_ATTEMPTS: u32 = 150;
const CONNECT_RETRY_INTERVAL: Duration = Duration::from_millis(100);

#[derive(Parser)]
#[command(
    name = "morphix",
    version,
    about = "Đổi định dạng, nén, đổi cỡ và xoá metadata cho ảnh, ngay trên máy."
)]
struct Cli {
    /// Thư mục lưu kết quả (mặc định: cạnh file gốc).
    #[arg(short, long, global = true)]
    out_dir: Option<PathBuf>,
    /// Mức chất lượng cho định dạng có mất mát (JPG, WebP, AVIF).
    #[arg(long, global = true, value_enum, default_value_t = QualityArg::Balanced)]
    quality: QualityArg,
    /// In kết quả dạng JSON (một object duy nhất trên stdout).
    #[arg(long, global = true)]
    json: bool,
    /// Chỉ in tên file sẽ tạo, không tạo gì.
    #[arg(long, global = true)]
    dry_run: bool,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Đổi định dạng ảnh. Mã hoá lại nên siêu dữ liệu bị bỏ (dùng `strip-meta` để xoá mà không mã hoá lại);
    /// GIF động chỉ lấy khung đầu. `--to pdf` tạo mỗi ảnh một PDF; thêm `--merge` để gộp thành một PDF.
    /// Văn bản và phụ đề: `--to txt|md|srt|vtt|csv|tsv`, chỉ trong cùng nhóm (xem `formats`).
    /// PDF: `--to txt` lấy lớp chữ có sẵn (không OCR).
    Convert {
        #[arg(required = true)]
        files: Vec<PathBuf>,
        #[arg(long, value_enum)]
        to: Target,
        /// Gộp mọi ảnh thành một PDF (chỉ với `--to pdf`).
        #[arg(long)]
        merge: bool,
    },
    /// Cắt đoạn âm thanh hoặc video, giữ định dạng gốc; thời điểm tính bằng giây, có thể là số thập phân.
    Cut {
        file: PathBuf,
        #[arg(long)]
        from: f64,
        #[arg(long)]
        to: f64,
    },
    /// Đổi tốc độ âm thanh hoặc video: 1.5 là nhanh 1,5 lần, 0.5 là chậm một nửa.
    Speed {
        file: PathBuf,
        #[arg(long)]
        factor: f64,
    },
    /// Tách âm thanh của video sang định dạng âm thanh.
    ExtractAudio {
        file: PathBuf,
        #[arg(long, value_enum)]
        to: AudioTarget,
    },
    /// Giải nén ZIP, TAR.GZ hoặc 7Z vào thư mục cùng tên (bỏ đuôi) cạnh file; RAR chưa giải nén được.
    Extract { file: PathBuf },
    /// Gộp các file thành một file nén `<tên file đầu>-nen.<đuôi>` cạnh file đầu; RAR chỉ đọc nên không nén được.
    Archive {
        #[arg(required = true)]
        files: Vec<PathBuf>,
        #[arg(long, value_enum)]
        to: ArchiveTarget,
    },
    /// Nén ảnh JPG, PNG, WebP, giữ nguyên định dạng.
    Compress {
        #[arg(required = true)]
        files: Vec<PathBuf>,
    },
    /// Xoá metadata (EXIF, GPS, XMP...) mà không mã hoá lại ảnh.
    StripMeta {
        #[arg(required = true)]
        files: Vec<PathBuf>,
    },
    /// Mở Menu nhanh cho các file đã cho; GUI chưa chạy thì tự khởi động.
    Menu {
        /// File đặt trực tiếp sau `menu`.
        #[arg(required_unless_present_any = ["newline_list", "uris"], conflicts_with = "uris")]
        files: Vec<PathBuf>,
        /// Nhiều đường dẫn phân cách bằng xuống dòng, cho tích hợp chỉ cung cấp kiểu danh sách này.
        #[arg(long, conflicts_with = "uris")]
        newline_list: Option<String>,
        /// URI `file:` từ Dolphin (`%U`); không dùng cùng file đặt trực tiếp hoặc `--newline-list`.
        #[arg(long, num_args = 1.., conflicts_with_all = ["files", "newline_list"])]
        uris: Vec<String>,
    },
    /// Mở Khay qua morphix-gui, dùng khi GNOME thiếu AppIndicator; không chờ GUI thoát và không gửi file.
    Tray {
        #[arg(long)]
        selection: bool,
        #[arg(long, conflicts_with = "selection")]
        unsupported_selection: bool,
    },
    /// Liệt kê việc làm được với các file đã cho (không có file thì liệt kê định dạng đầu ra hỗ trợ).
    Formats { files: Vec<PathBuf> },
    /// Kiểm môi trường: ffmpeg, thư mục dữ liệu, log và trạng thái tích hợp (chỉ đọc).
    Doctor,
    /// Cài menu chuột phải, mục Scripts/Dolphin và khởi động cùng máy, chỉ cho người dùng hiện tại.
    InstallIntegration {
        /// Không đăng ký khởi động cùng máy (chỉ dùng khi cài).
        #[arg(long)]
        no_autostart: bool,
    },
    /// Gỡ các mục tích hợp do Morphix tạo; file hoặc khoá của ứng dụng khác và dữ liệu Morphix được giữ nguyên.
    UninstallIntegration,
    /// Gỡ Morphix đã cài bằng lệnh curl; giữ dữ liệu người dùng.
    Uninstall,
    /// Đổi kích thước ảnh theo phần trăm hoặc cạnh dài nhất; không phóng to.
    Resize {
        #[arg(required = true)]
        files: Vec<PathBuf>,
        /// Phần trăm kích thước gốc, từ 1 đến 100.
        #[arg(
            long,
            required_unless_present = "max_edge",
            conflicts_with = "max_edge"
        )]
        percent: Option<u8>,
        /// Cạnh dài nhất tối đa, tính bằng điểm ảnh.
        #[arg(long)]
        max_edge: Option<u32>,
    },
}

#[derive(Clone, Copy, ValueEnum)]
enum QualityArg {
    Small,
    Balanced,
    High,
}

#[derive(Clone, Copy, ValueEnum)]
enum Target {
    #[value(alias = "jpeg")]
    Jpg,
    Png,
    Webp,
    Avif,
    Gif,
    #[value(alias = "tif")]
    Tiff,
    Bmp,
    Pdf,
    Txt,
    Md,
    Srt,
    Vtt,
    Csv,
    Tsv,
    Mp4,
    Mkv,
    Webm,
    Mov,
    Avi,
    Mp3,
    M4a,
    Wav,
    Flac,
    Ogg,
    Opus,
}

/// Đích của `archive`. RAR có mặt để báo lỗi rõ ràng thay vì chỉ "giá trị không hợp lệ".
#[derive(Clone, Copy, ValueEnum)]
enum ArchiveTarget {
    Zip,
    #[value(name = "tar.gz")]
    TarGz,
    #[value(name = "7z")]
    SevenZ,
    Rar,
}

/// Đích của `extract-audio`: chỉ định dạng âm thanh.
#[derive(Clone, Copy, ValueEnum)]
enum AudioTarget {
    Mp3,
    M4a,
    Wav,
    Flac,
    Ogg,
    Opus,
}

impl AudioTarget {
    fn media_format(self) -> MediaFormat {
        match self {
            AudioTarget::Mp3 => MediaFormat::Mp3,
            AudioTarget::M4a => MediaFormat::M4a,
            AudioTarget::Wav => MediaFormat::Wav,
            AudioTarget::Flac => MediaFormat::Flac,
            AudioTarget::Ogg => MediaFormat::Ogg,
            AudioTarget::Opus => MediaFormat::Opus,
        }
    }
}

impl Target {
    /// Đích âm thanh hoặc video; `gif` là đích của cả ảnh lẫn video nên `build_jobs` chọn theo đầu vào.
    fn media_format(self) -> Option<MediaFormat> {
        match self {
            Target::Mp4 => Some(MediaFormat::Mp4),
            Target::Mkv => Some(MediaFormat::Mkv),
            Target::Webm => Some(MediaFormat::WebM),
            Target::Mov => Some(MediaFormat::Mov),
            Target::Avi => Some(MediaFormat::Avi),
            Target::Gif => Some(MediaFormat::Gif),
            Target::Mp3 => Some(MediaFormat::Mp3),
            Target::M4a => Some(MediaFormat::M4a),
            Target::Wav => Some(MediaFormat::Wav),
            Target::Flac => Some(MediaFormat::Flac),
            Target::Ogg => Some(MediaFormat::Ogg),
            Target::Opus => Some(MediaFormat::Opus),
            _ => None,
        }
    }

    /// Đích văn bản; `None` cho đích ảnh và PDF.
    fn text_format(self) -> Option<TextFormat> {
        match self {
            Target::Txt => Some(TextFormat::Txt),
            Target::Md => Some(TextFormat::Markdown),
            Target::Srt => Some(TextFormat::Srt),
            Target::Vtt => Some(TextFormat::Vtt),
            Target::Csv => Some(TextFormat::Csv),
            Target::Tsv => Some(TextFormat::Tsv),
            _ => None,
        }
    }

    fn image_format(self) -> Option<ImgFormat> {
        match self {
            Target::Jpg => Some(ImgFormat::Jpeg),
            Target::Png => Some(ImgFormat::Png),
            Target::Webp => Some(ImgFormat::WebP),
            Target::Avif => Some(ImgFormat::Avif),
            Target::Gif => Some(ImgFormat::Gif),
            Target::Tiff => Some(ImgFormat::Tiff),
            Target::Bmp => Some(ImgFormat::Bmp),
            _ => None,
        }
    }
}

fn quality_of(arg: QualityArg) -> Quality {
    match arg {
        QualityArg::Small => Quality::Small,
        QualityArg::Balanced => Quality::Balanced,
        QualityArg::High => Quality::High,
    }
}

/// Dựng các `Job` từ tham số. Lỗi ở đây là lỗi dùng sai lệnh (mã thoát 2).
fn build_jobs(cli: &Cli) -> Result<Vec<Job>, String> {
    let job = |inputs: Vec<PathBuf>, operation: Operation| Job {
        inputs,
        operation,
        quality: quality_of(cli.quality),
        out_dir: cli.out_dir.clone(),
    };
    match &cli.command {
        Command::Formats { .. }
        | Command::Doctor
        | Command::Tray { .. }
        | Command::Menu { .. }
        | Command::InstallIntegration { .. }
        | Command::UninstallIntegration
        | Command::Uninstall => Err("lệnh này không tạo việc xử lý file".to_string()),
        Command::Convert { files, to, merge } => {
            // WHY: `--merge` chỉ có nghĩa với PDF; nhánh văn bản phải báo lỗi chứ không bỏ cờ âm thầm.
            if let Some(format) = to.text_format() {
                if *merge {
                    return Err("--merge chỉ dùng với --to pdf".to_string());
                }
                return Ok(files
                    .iter()
                    .map(|file| {
                        // WHY: PDF sang TXT đọc lớp chữ của PDF, không đi qua bộ chuyển văn bản; chọn theo đầu vào như GIF.
                        let operation = if format == TextFormat::Txt && is_pdf_input(file) {
                            Operation::PdfToText
                        } else {
                            Operation::ConvertText { to: format }
                        };
                        job(vec![file.clone()], operation)
                    })
                    .collect());
            }
            if let Some(format) = to.media_format() {
                if *merge {
                    return Err("--merge chỉ dùng với --to pdf".to_string());
                }
                let gif_target = matches!(to, Target::Gif);
                return Ok(files
                    .iter()
                    .map(|file| {
                        // WHY: ảnh đích GIF vẫn đi qua bộ mã hoá ảnh; file không đọc được thì coi là ảnh để job báo đúng lỗi đọc.
                        let as_image = gif_target && !is_media_input(file);
                        let operation = if as_image {
                            Operation::ConvertImage { to: ImgFormat::Gif }
                        } else {
                            Operation::ConvertMedia { to: format }
                        };
                        job(vec![file.clone()], operation)
                    })
                    .collect());
            }
            match to.image_format() {
                None if *merge => Ok(vec![job(files.clone(), Operation::MergeImagesToPdf)]),
                None => Ok(files
                    .iter()
                    .map(|f| job(vec![f.clone()], Operation::MergeImagesToPdf))
                    .collect()),
                Some(_) if *merge => Err("--merge chỉ dùng với --to pdf".to_string()),
                Some(format) => Ok(vec![job(
                    files.clone(),
                    Operation::ConvertImage { to: format },
                )]),
            }
        }
        Command::Cut { file, from, to } => {
            let start_ms = seconds_to_ms(*from, "--from")?;
            let end_ms = seconds_to_ms(*to, "--to")?;
            if end_ms <= start_ms {
                return Err("--to phải lớn hơn --from".to_string());
            }
            Ok(vec![job(
                vec![file.clone()],
                Operation::CutMedia { start_ms, end_ms },
            )])
        }
        Command::Speed { file, factor } => Ok(vec![job(
            vec![file.clone()],
            Operation::ChangeSpeed {
                factor_milli: factor_to_milli(*factor)?,
            },
        )]),
        Command::ExtractAudio { file, to } => Ok(vec![job(
            vec![file.clone()],
            Operation::ExtractAudio {
                to: to.media_format(),
            },
        )]),
        Command::Extract { file } => Ok(vec![job(vec![file.clone()], Operation::ExtractArchive)]),
        Command::Archive { files, to } => {
            let format = match to {
                ArchiveTarget::Zip => ArchiveFormat::Zip,
                ArchiveTarget::TarGz => ArchiveFormat::TarGz,
                ArchiveTarget::SevenZ => ArchiveFormat::SevenZ,
                // WHY: RAR chỉ đọc (xem `ops::archive`), không có bộ nén; đây là dùng sai lệnh nên exit 2.
                ArchiveTarget::Rar => {
                    return Err(
                        "RAR chỉ đọc được, không nén thành RAR; chọn zip, tar.gz hoặc 7z"
                            .to_string(),
                    )
                }
            };
            Ok(vec![job(
                files.clone(),
                Operation::CreateArchive { format },
            )])
        }
        Command::Compress { files } => Ok(vec![job(files.clone(), Operation::CompressImage)]),
        Command::StripMeta { files } => Ok(vec![job(files.clone(), Operation::StripMetadata)]),
        Command::Resize {
            files,
            percent,
            max_edge,
        } => {
            let spec = match (percent, max_edge) {
                (Some(percent), _) => ResizeSpec::Percent(*percent),
                (None, Some(max_edge)) => ResizeSpec::MaxEdge(*max_edge),
                (None, None) => return Err("cần --percent hoặc --max-edge".to_string()),
            };
            Ok(vec![job(files.clone(), Operation::ResizeImage(spec))])
        }
    }
}

/// Giây thập phân sang mili-giây. WHY: số âm, NaN và vô cùng là dùng sai lệnh nên báo ngay (exit 2).
fn seconds_to_ms(seconds: f64, flag: &str) -> Result<u64, String> {
    if !seconds.is_finite() || seconds < 0.0 {
        return Err(format!("{flag} phải là số giây không âm"));
    }
    Ok((seconds * 1000.0).round() as u64)
}

/// Hệ số tốc độ sang phần nghìn, theo `Operation::ChangeSpeed`. Hệ số về 0 sau làm tròn cũng bị loại.
fn factor_to_milli(factor: f64) -> Result<u32, String> {
    let milli = (factor * 1000.0).round();
    if !factor.is_finite() || milli < 1.0 || milli > f64::from(u32::MAX) {
        return Err("--factor phải lớn hơn 0".to_string());
    }
    Ok(milli as u32)
}

/// File có phải âm thanh hoặc video không. Không đọc được thì coi là không, để job báo đúng lỗi đọc.
fn is_media_input(path: &std::path::Path) -> bool {
    matches!(detect(path), Ok(FileKind::Media(_)))
}

/// File có phải PDF không. Không đọc được thì coi là không, để job báo đúng lỗi đọc.
fn is_pdf_input(path: &std::path::Path) -> bool {
    matches!(detect(path), Ok(FileKind::Pdf))
}

/// Kiểm mọi đầu vào trước khi chạy việc nào, nên cặp không hỗ trợ không để lại file nào (spec mục 5).
/// WHY: đó là dùng sai lệnh (exit 2), khác lỗi dữ liệu của một cặp hợp lệ (exit 1).
fn check_pairs(jobs: &[Job]) -> Result<(), String> {
    for job in jobs {
        for input in &job.inputs {
            // WHY: file không đọc được không phải cặp sai; bỏ qua để job báo đúng lỗi đọc file.
            let Ok(kind) = detect(input) else { continue };
            if let Some(reason) =
                unsupported_reason(&input.display().to_string(), &kind, &job.operation)
            {
                return Err(reason);
            }
        }
    }
    Ok(())
}

/// Lý do không chuyển được, `None` nếu cặp hợp lệ.
fn unsupported_reason(name: &str, kind: &FileKind, operation: &Operation) -> Option<String> {
    match (kind, operation) {
        // WHY: file không nhận dạng được là lỗi dữ liệu (file rỗng, hỏng); job báo lỗi đó với exit 1.
        (FileKind::Unknown, _) => None,
        // WHY: đích có được hay không do catalog quyết định, để CLI và vòng tròn không lệch nhau.
        (FileKind::Media(_), Operation::ConvertMedia { to }) => {
            let offered = actions_for(std::slice::from_ref(kind))
                .iter()
                .any(|action| action.kind == ActionKind::ConvertMedia(*to));
            if offered {
                None
            } else {
                Some(format!(
                    "{name}: không chuyển được sang {}: cặp này chưa được hỗ trợ",
                    to.label()
                ))
            }
        }
        (_, Operation::ConvertMedia { to }) => Some(format!(
            "{name}: không chuyển được sang {}: chỉ âm thanh và video mới chuyển được sang đích này",
            to.label()
        )),
        (FileKind::Media(_), Operation::CompressMedia)
        | (FileKind::Media(_), Operation::CutMedia { .. })
        | (FileKind::Media(_), Operation::ChangeSpeed { .. })
        | (FileKind::Media(_), Operation::ExtractAudio { .. }) => None,
        (_, Operation::CompressMedia)
        | (_, Operation::CutMedia { .. })
        | (_, Operation::ChangeSpeed { .. })
        | (_, Operation::ExtractAudio { .. }) => Some(format!(
            "{name}: chỉ âm thanh và video mới dùng được với lệnh này"
        )),
        // WHY: build_jobs chỉ tạo PdfToText cho PDF; khớp với catalog để CLI và vòng tròn không lệch nhau.
        (FileKind::Pdf, Operation::PdfToText) => None,
        // WHY: mọi file nhận dạng được đều gộp được; file không nhận dạng được đã bỏ qua ở trên.
        (_, Operation::CreateArchive { .. }) => None,
        (FileKind::Archive(ArchiveFormat::Rar), Operation::ExtractArchive) => Some(
            // WHY: lời từ lõi (Unsupported) giữ nguyên, để CLI và GUI cùng một câu; câu này còn chung chung (xem gap trong báo cáo).
            MorphixError::Unsupported {
                path: PathBuf::from(name),
            }
            .to_string(),
        ),
        (FileKind::Archive(_), Operation::ExtractArchive) => None,
        (_, Operation::ExtractArchive) => Some(format!("{name}: không phải file nén nên không giải nén được")),
        (_, Operation::PdfToText) => Some(format!(
            "{name}: không chuyển được sang TXT: chỉ PDF mới chuyển được sang TXT bằng việc này"
        )),
        (FileKind::Text(from), Operation::ConvertText { to }) => {
            if text_pair_supported(*from, *to) {
                None
            } else {
                Some(format!(
                    "{name}: không chuyển được {} sang {}: cặp này chưa được hỗ trợ",
                    from.extension().to_uppercase(),
                    to.extension().to_uppercase()
                ))
            }
        }
        (_, Operation::ConvertText { to }) => Some(format!(
            "{name}: không chuyển được sang {}: chỉ văn bản, phụ đề và bảng mới chuyển được sang đích này",
            to.extension().to_uppercase()
        )),
        (FileKind::Image(_), _) => None,
        (_, _) => Some(format!("{name}: chỉ ảnh mới dùng được với lệnh này")),
    }
}

/// Ghi log vào `data_dir()/morphix.log`. Không mở được file thì chạy tiếp và báo một lần, vì việc
/// chuyển đổi không nên hỏng chỉ vì thiếu log.
/// Dry-run và doctor không được tạo `morphix.log`, nên chỉ ghi stderr; các lệnh khác ghi file như cũ.
fn init_logging(log_to_file: bool) {
    if !log_to_file {
        tracing_subscriber::fmt()
            .with_writer(std::io::stderr)
            .with_ansi(false)
            .init();
        return;
    }
    let dir = data_dir();
    let opened = std::fs::create_dir_all(&dir).and_then(|()| {
        OpenOptions::new()
            .create(true)
            .append(true)
            .open(dir.join("morphix.log"))
    });
    match opened {
        Ok(file) => tracing_subscriber::fmt()
            .with_writer(Mutex::new(file))
            .with_ansi(false)
            .init(),
        Err(error) => eprintln!("cảnh báo: không ghi được log ở {}: {error}", dir.display()),
    }
}

/// `0` mọi mục ok, `1` mọi mục lỗi, `3` ok lẫn lỗi.
fn exit_code(items: &[ItemReport]) -> ExitCode {
    let failed = items.iter().filter(|item| item.result.is_err()).count();
    match (failed, items.len() - failed) {
        (0, _) => ExitCode::SUCCESS,
        (_, 0) => ExitCode::from(1),
        _ => ExitCode::from(3),
    }
}

/// `convert<TAB>nhãn` hoặc `tool<TAB>nhãn` cho mỗi việc; không việc nào thì thoát 1.
fn run_formats(files: &[PathBuf], as_json: bool) -> ExitCode {
    if files.is_empty() {
        let labels = std::iter::once("PDF").chain(ImgFormat::ALL_ORDERED.iter().map(|f| f.label()));
        for label in labels {
            println!("{label}");
        }
        return ExitCode::SUCCESS;
    }
    let mut kinds = Vec::with_capacity(files.len());
    for file in files {
        match detect(file) {
            Ok(kind) => kinds.push(kind),
            Err(error) => {
                tracing::error!(%error, "không nhận dạng được file");
                eprintln!("lỗi  {error}");
                return ExitCode::from(1);
            }
        }
    }
    let actions = actions_for(&kinds);
    if actions.is_empty() {
        eprintln!("không có việc nào làm được với tập file này");
        return ExitCode::from(1);
    }
    if as_json {
        let items: Vec<_> = actions
            .iter()
            .map(|a| serde_json::json!({ "group": if a.is_tool { "tool" } else { "convert" }, "label": a.label }))
            .collect();
        println!("{}", serde_json::json!({ "actions": items }));
    } else {
        for action in &actions {
            println!(
                "{}\t{}",
                if action.is_tool { "tool" } else { "convert" },
                action.label
            );
        }
    }
    ExitCode::SUCCESS
}

/// Thiếu ffmpeg không phải lỗi (ảnh không cần). Thư mục dữ liệu không ghi được là lỗi, và lỗi tích hợp
/// (`1`) thắng kết luận chưa đủ dữ liệu (`3`); mọi mục áp dụng khoẻ thì `0`.
fn run_doctor(as_json: bool) -> ExitCode {
    let dir = data_dir();
    let writable = check_dir_writable(&dir);
    if let Err(error) = &writable {
        tracing::error!(dir = %dir.display(), %error, "thư mục dữ liệu không ghi được");
    }
    let (integration, integration_error) = match BinaryPaths::resolve() {
        Ok(binaries) => (
            Some(integration::inspect(
                integration::default_backend().as_ref(),
                &binaries,
            )),
            None,
        ),
        Err(message) => (None, Some(message)),
    };
    let integration_code = match &integration {
        Some(report) => integration::doctor_exit_code(&report.components),
        None => 3,
    };
    let report = DoctorReport {
        ffmpeg: find_ffmpeg().map(|path| path.display().to_string()),
        data_dir: dir.display().to_string(),
        data_dir_writable: writable.is_ok(),
        data_dir_error: writable.as_ref().err().map(ToString::to_string),
        log: dir.join("morphix.log").display().to_string(),
        integration,
        integration_error,
    };
    if let Err(error) = output::print_doctor(&report, as_json) {
        tracing::error!(%error, "không mã hoá được báo cáo doctor");
        eprintln!("lỗi: không in được báo cáo doctor ({error})");
        return ExitCode::from(1);
    }
    if !report.data_dir_writable || integration_code == 1 {
        ExitCode::from(1)
    } else if integration_code == 3 {
        ExitCode::from(3)
    } else {
        ExitCode::SUCCESS
    }
}

/// Chạy install hoặc uninstall với backend của nền tảng; lỗi không xác định được đường dẫn binary là không thể tiếp tục.
fn run_integration_change(options: &IntegrationOptions, as_json: bool, install: bool) -> ExitCode {
    let binaries = match BinaryPaths::resolve() {
        Ok(binaries) => binaries,
        Err(message) => {
            eprintln!("lỗi: {message}");
            return ExitCode::from(1);
        }
    };
    let backend = integration::default_backend();
    let report = if install {
        integration::install(backend.as_ref(), &binaries, options)
    } else {
        integration::uninstall(backend.as_ref(), &binaries, options)
    };
    if let Err(error) = output::print_integration(&report, as_json) {
        tracing::error!(%error, "không mã hoá được báo cáo tích hợp");
        eprintln!("lỗi: không in được báo cáo tích hợp ({error})");
        return ExitCode::from(1);
    }
    ExitCode::from(integration::exit_code_for(report.overall))
}

/// Gỡ bản cài: gói `.deb` chỉ gỡ tích hợp per-user và nhắc `apt remove`; bản curl gỡ tích hợp, tắt GUI rồi
/// xoá file trong prefix. WHY kiểm marker trước: chạy từ chỗ lạ thì không được đụng vào gì.
fn run_uninstall() -> ExitCode {
    let binaries = match BinaryPaths::resolve() {
        Ok(binaries) => binaries,
        Err(message) => {
            eprintln!("lỗi: {message}");
            return ExitCode::from(1);
        }
    };
    let package = integration::linux::install_mode(&binaries.morphix)
        == integration::linux::InstallMode::Package;
    let prefix = binaries
        .morphix
        .parent()
        .and_then(std::path::Path::parent)
        .map(std::path::Path::to_path_buf);
    let prefix = match prefix {
        Some(prefix) if package || is_user_install(&prefix) => prefix,
        _ => {
            eprintln!(
                "lỗi: {} không thuộc bản cài Morphix bằng lệnh curl (thiếu lib/morphix/.morphix-install); không gỡ gì",
                binaries.morphix.display()
            );
            return ExitCode::from(1);
        }
    };
    let options = IntegrationOptions::default();
    let report =
        integration::uninstall(integration::default_backend().as_ref(), &binaries, &options);
    if let Err(error) = output::print_integration(&report, false) {
        eprintln!("lỗi: không in được báo cáo tích hợp ({error})");
        return ExitCode::from(1);
    }
    if package {
        println!(
            "Morphix được cài bằng gói .deb: chạy `sudo apt remove morphix` để gỡ chương trình."
        );
        return ExitCode::from(integration::exit_code_for(report.overall));
    }
    stop_gui();
    match remove_user_install(&prefix) {
        Ok(removed) => {
            for path in removed {
                println!("đã xoá {}", path.display());
            }
            println!("Đã giữ dữ liệu tại {}", data_dir().display());
            ExitCode::from(integration::exit_code_for(report.overall))
        }
        Err(message) => {
            eprintln!("lỗi: {message}");
            ExitCode::from(1)
        }
    }
}

/// Tắt `morphix-gui` trước khi xoá file của nó. pkill thoát 1 khi không có tiến trình nào, không phải lỗi.
fn stop_gui() {
    match std::process::Command::new("pkill")
        .args(["-x", "morphix-gui"])
        .status()
    {
        Ok(status) if matches!(status.code(), Some(0 | 1)) => {}
        Ok(status) => {
            eprintln!("cảnh báo: pkill morphix-gui thoát {status}; Morphix có thể vẫn đang chạy")
        }
        Err(error) => eprintln!(
            "cảnh báo: không chạy được pkill ({error}); hãy tự thoát Morphix nếu đang chạy"
        ),
    }
}

/// Gộp đường dẫn positional trước, rồi tới từng dòng của `--newline-list` (CRLF được chuẩn hoá).
/// Dòng rỗng bị bỏ; không còn đường dẫn nào thì từ chối để GUI không mở cửa sổ rỗng.
fn collect_menu_paths(
    files: &[PathBuf],
    newline_list: Option<&str>,
) -> Result<Vec<PathBuf>, String> {
    let mut paths = files.to_vec();
    if let Some(list) = newline_list {
        paths.extend(
            list.lines()
                .filter(|line| !line.is_empty())
                .map(PathBuf::from),
        );
    }
    if paths.is_empty() {
        return Err("không có đường dẫn file nào".to_string());
    }
    // WHY absolute: GUI đang chạy có thư mục làm việc khác, nên đường dẫn tương đối chỉ đúng ở đây.
    paths
        .into_iter()
        .map(|path| {
            std::path::absolute(&path).map_err(|error| {
                format!("không xác định được đường dẫn {}: {error}", path.display())
            })
        })
        .collect()
}

/// Chạy `morphix-gui` với `args` và trả `Child` cho người gọi quyết định có chờ hay không.
/// WHY null stdio: GUI tiếp tục chạy sau khi CLI thoát; giữ pipe mở sẽ làm `$(morphix menu ...)` treo.
fn spawn_gui(args: &[&str]) -> Result<Child, String> {
    let binary = integration::gui_binary_path()?;
    // WHY absolute: GUI có thể chạy với thư mục làm việc khác, và đường dẫn tương đối sẽ trỏ sai ở đó.
    let binary = std::path::absolute(&binary)
        .map_err(|error| format!("không xác định được {}: {error}", binary.display()))?;
    if !binary.is_file() {
        return Err(format!(
            "không tìm thấy morphix-gui tại {}",
            binary.display()
        ));
    }
    std::process::Command::new(&binary)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| format!("không chạy được {}: {error}", binary.display()))
}

/// Khởi động GUI không chờ, rồi thử kết nối lại. Lỗi trả về là nguyên nhân cuối cùng để người dùng biết vì sao.
fn start_gui_and_send(socket: &str, request: &MenuRequest) -> Result<(), String> {
    let _gui = spawn_gui(&[])?;

    connect_with_retries(
        CONNECT_ATTEMPTS,
        CONNECT_RETRY_INTERVAL,
        || send_request(socket, request).map_err(|error| error.to_string()),
        thread::sleep,
    )
}

/// Thử gửi tối đa `attempts` lần, mỗi lần cách `interval`; trả lỗi cuối cùng nếu hết lượt.
fn connect_with_retries(
    attempts: u32,
    interval: Duration,
    mut send: impl FnMut() -> Result<(), String>,
    mut sleep: impl FnMut(Duration),
) -> Result<(), String> {
    let mut last_error = String::new();
    for _ in 0..attempts {
        sleep(interval);
        match send() {
            Ok(()) => return Ok(()),
            Err(error) => last_error = error,
        }
    }
    Err(format!(
        "không kết nối được tới Morphix sau {attempts} lần thử: {last_error}"
    ))
}

/// Gửi file tới GUI đang chạy; không có thì khởi động và gửi lại. Trả `Ok` ngay khi server nhận yêu cầu.
fn run_menu(files: &[PathBuf], newline_list: Option<&str>, uris: &[String]) -> ExitCode {
    let paths = if uris.is_empty() {
        collect_menu_paths(files, newline_list)
    } else {
        file_uris_to_paths(uris)
    };
    let request = match paths {
        Ok(paths) => MenuRequest { paths },
        Err(message) => {
            eprintln!("lỗi: {message}");
            return ExitCode::from(2);
        }
    };
    let socket = socket_name();
    let first_error = match send_request(&socket, &request) {
        Ok(()) => return ExitCode::SUCCESS,
        // WHY: tên file không mã hoá được là lỗi của yêu cầu; khởi động GUI cũng không giúp gì.
        Err(error) if error.kind() == std::io::ErrorKind::InvalidData => {
            eprintln!("lỗi: {error}");
            return ExitCode::from(2);
        }
        Err(error) => error,
    };
    tracing::info!(error = %first_error, "không kết nối được GUI đang chạy, thử khởi động");
    match start_gui_and_send(&socket, &request) {
        Ok(()) => ExitCode::SUCCESS,
        Err(reason) => {
            tracing::error!(%reason, "không khởi động được Morphix");
            eprintln!("lỗi: không khởi động được Morphix ({reason})");
            ExitCode::from(1)
        }
    }
}

/// `morphix tray`: nhờ GUI mở Khay rồi thoát. Không mở socket và không gửi file; GUI đang chạy sẽ tự đưa Khay lên trước.
fn run_tray(selection: bool, unsupported_selection: bool) -> ExitCode {
    match spawn_gui(&tray_gui_args(selection, unsupported_selection)) {
        Ok(_gui) => ExitCode::SUCCESS,
        Err(reason) => {
            tracing::error!(%reason, "không mở được Khay");
            eprintln!("lỗi: không mở được Khay ({reason})");
            ExitCode::from(1)
        }
    }
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let logs_to_file = !cli.dry_run && !matches!(cli.command, Command::Doctor);
    init_logging(logs_to_file);
    match &cli.command {
        Command::Formats { files } => return run_formats(files, cli.json),
        Command::Doctor => return run_doctor(cli.json),
        Command::Tray {
            selection,
            unsupported_selection,
        } => return run_tray(*selection, *unsupported_selection),
        Command::InstallIntegration { no_autostart } => {
            let options = IntegrationOptions {
                dry_run: cli.dry_run,
                install_autostart: !no_autostart,
            };
            return run_integration_change(&options, cli.json, true);
        }
        Command::Uninstall => {
            // WHY: lệnh này xoá file thật; cờ toàn cục không áp dụng được thì báo lỗi thay vì bỏ qua âm thầm.
            for (set, flag) in [(cli.dry_run, "--dry-run"), (cli.json, "--json")] {
                if set {
                    eprintln!("lỗi: {flag} không dùng được với uninstall");
                    return ExitCode::from(2);
                }
            }
            return run_uninstall();
        }
        Command::UninstallIntegration => {
            let options = IntegrationOptions {
                dry_run: cli.dry_run,
                install_autostart: true,
            };
            return run_integration_change(&options, cli.json, false);
        }
        Command::Menu {
            files,
            newline_list,
            uris,
        } => return run_menu(files, newline_list.as_deref(), uris),
        _ => {}
    }
    let jobs = match build_jobs(&cli) {
        Ok(jobs) => jobs,
        Err(message) => {
            eprintln!("lỗi: {message}");
            return ExitCode::from(2);
        }
    };
    if let Err(message) = check_pairs(&jobs) {
        eprintln!("lỗi: {message}");
        return ExitCode::from(2);
    }
    if cli.dry_run {
        let plan: Vec<_> = jobs.iter().flat_map(planned_outputs).collect();
        output::print_plan(&plan, cli.json);
        return ExitCode::SUCCESS;
    }
    let cancel = CancelToken::new();
    let items: Vec<ItemReport> = jobs
        .iter()
        .flat_map(|job| run_job(job, &cancel, &|_| {}).items)
        .collect();
    output::print_results(&items, cli.json);
    exit_code(&items)
}

fn tray_gui_args(selection: bool, unsupported_selection: bool) -> Vec<&'static str> {
    if selection {
        return vec!["--capture-selection"];
    }
    if unsupported_selection {
        return vec!["--show-khay", "--unsupported-selection"];
    }
    vec!["--show-khay"]
}

#[cfg(test)]
mod gui_start_tests {
    use super::connect_with_retries;
    use std::time::Duration;

    #[test]
    fn succeeds_on_a_late_attempt_without_giving_up() {
        // WHY: GUI khởi động nguội (WebKit, không GPU) có thể mất vài giây trước khi mở socket.
        let mut calls = 0;
        let result = connect_with_retries(
            20,
            Duration::from_millis(100),
            || {
                calls += 1;
                if calls < 15 {
                    Err("chưa có socket".to_string())
                } else {
                    Ok(())
                }
            },
            |_| {},
        );
        assert!(result.is_ok());
        assert_eq!(calls, 15);
    }

    #[test]
    fn gives_up_after_the_budget_with_the_last_error() {
        let mut slept = Duration::ZERO;
        let result = connect_with_retries(
            3,
            Duration::from_millis(100),
            || Err("không có socket".to_string()),
            |d| slept += d,
        );
        let message = result.unwrap_err();
        assert!(message.contains("sau 3 lần thử"), "{message}");
        assert!(message.ends_with("không có socket"), "{message}");
        assert_eq!(slept, Duration::from_millis(300));
    }
}
