use std::fs::File;
use std::path::{Path, PathBuf};

use crate::{
    cancel::CancelToken,
    catalog::text_pair_supported,
    error::{MorphixError, Result},
    kind::{detect, ArchiveFormat, FileKind, ImgFormat, MediaFormat, TextFormat},
    naming::{desired_output, extension_of, natural_sort, write_output},
    ops::{
        archive::{create_archive, extract, planned_folder, SUFFIX_ARCHIVE},
        compress::{
            compress_image, display_dimensions, resize_image, resize_suffix, target_dimensions,
            ResizeSpec, SUFFIX_COMPRESSED,
        },
        docx,
        html::{self, markdown_to_html},
        image_io::convert_image,
        image_pdf::{images_to_pdf, pdf_destination},
        markdown::{self, cues_to_markdown, escape_plain, markdown_to_plain, table_to_markdown},
        media::{
            change_speed, compress_media, convert_media_reporting, cut_media, extract_audio,
            SUFFIX_CUT, SUFFIX_SPEED,
        },
        pdf_text::{pdf_to_markdown, pdf_to_text},
        strip_meta::{strip_metadata, SUFFIX_STRIPPED},
        text::{
            read_cues, read_table, read_utf8, strip_timestamps, write_cues, write_plain,
            write_table, write_text,
        },
    },
    paths::find_ffmpeg,
    quality::Quality,
};

/// Việc cần làm, kèm tham số. `ActionKind` trong `catalog` là danh sách việc chưa có tham số.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Operation {
    ConvertImage {
        to: ImgFormat,
    },
    /// Gộp mọi đầu vào thành **một** PDF; là việc duy nhất tạo một đầu ra từ nhiều đầu vào.
    MergeImagesToPdf,
    CompressImage,
    StripMetadata,
    ResizeImage(ResizeSpec),
    /// Chuyển văn bản, phụ đề hoặc bảng sang định dạng cùng nhóm (xem `catalog::text_targets`).
    ConvertText {
        to: TextFormat,
    },
    /// Chuyển âm thanh hoặc video sang đích cùng loại (xem `catalog::actions_for`).
    ConvertMedia {
        to: MediaFormat,
    },
    /// Nén lại bằng encoder của chính định dạng gốc; kết quả không nhỏ hơn gốc thì báo `NotSmaller`.
    CompressMedia,
    /// Cắt đoạn từ `start_ms` đến `end_ms` (mili-giây), giữ định dạng gốc.
    CutMedia {
        start_ms: u64,
        end_ms: u64,
    },
    /// Đổi tốc độ theo phần nghìn: 1500 là nhanh 1,5 lần.
    ChangeSpeed {
        factor_milli: u32,
    },
    /// Tách âm thanh của video sang định dạng âm thanh `to`.
    ExtractAudio {
        to: MediaFormat,
    },
    /// Lớp chữ của PDF ra file TXT cạnh file gốc (không OCR).
    PdfToText,
    /// Lớp chữ của PDF ra file MD, mỗi trang một mục `## Trang N` (không OCR).
    PdfToMarkdown,
    /// Gộp mọi đầu vào thành **một** file nén; tên theo đầu vào đầu tiên (xem `ops::archive`).
    CreateArchive {
        format: ArchiveFormat,
    },
    /// Giải nén từng file nén vào thư mục riêng cạnh file (xem `ops::archive::extract`).
    ExtractArchive,
}

#[derive(Debug, Clone)]
pub struct Job {
    pub inputs: Vec<PathBuf>,
    pub operation: Operation,
    pub quality: Quality,
    pub out_dir: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Progress {
    pub done: usize,
    pub total: usize,
}

#[derive(Debug)]
pub struct ItemReport {
    pub inputs: Vec<PathBuf>,
    pub result: Result<PathBuf>,
    /// Điều người dùng nên biết dù việc đã xong, ví dụ ảnh convert ra không nhỏ hơn bản gốc.
    pub warnings: Vec<String>,
}

#[derive(Debug)]
pub struct JobReport {
    pub items: Vec<ItemReport>,
}

impl JobReport {
    pub fn succeeded(&self) -> usize {
        self.items.iter().filter(|item| item.result.is_ok()).count()
    }

    pub fn failed(&self) -> usize {
        self.items.len() - self.succeeded()
    }
}

/// Số mục `run_job` sẽ báo trong `Progress::total`. Giao diện dùng để vẽ tiến trình từ đầu cho đúng.
pub fn item_count(job: &Job) -> usize {
    item_groups(job).len()
}

/// Chia đầu vào thành các mục độc lập: gộp PDF là một mục chứa tất cả, các việc khác mỗi file một mục.
fn item_groups(job: &Job) -> Vec<Vec<PathBuf>> {
    match job.operation {
        Operation::MergeImagesToPdf | Operation::CreateArchive { .. } => vec![job.inputs.clone()],
        _ => job.inputs.iter().map(|input| vec![input.clone()]).collect(),
    }
}

/// Chạy một mục. `warnings` nhận cảnh báo mà chính thao tác biết (media dùng thông tin luồng nguồn đã đọc).
fn run_item(
    job: &Job,
    inputs: &[PathBuf],
    cancel: &CancelToken,
    warnings: &mut Vec<String>,
) -> Result<PathBuf> {
    let out_dir = job.out_dir.as_deref();
    match &job.operation {
        Operation::MergeImagesToPdf => images_to_pdf(inputs, out_dir, cancel),
        Operation::ConvertImage { to } => convert_image(&inputs[0], *to, job.quality, out_dir),
        Operation::CompressImage => compress_image(&inputs[0], job.quality, out_dir),
        Operation::StripMetadata => strip_metadata(&inputs[0], out_dir),
        Operation::ResizeImage(spec) => resize_image(&inputs[0], *spec, job.quality, out_dir),
        Operation::ConvertText { to } => convert_text(&inputs[0], *to, out_dir, warnings),
        Operation::ConvertMedia { to } => convert_media_reporting(
            &require_ffmpeg()?,
            &inputs[0],
            *to,
            out_dir,
            cancel,
            warnings,
        ),
        Operation::CompressMedia => compress_media(&require_ffmpeg()?, &inputs[0], out_dir, cancel),
        Operation::CutMedia { start_ms, end_ms } => cut_media(
            &require_ffmpeg()?,
            &inputs[0],
            *start_ms,
            *end_ms,
            out_dir,
            cancel,
        ),
        Operation::ChangeSpeed { factor_milli } => change_speed(
            &require_ffmpeg()?,
            &inputs[0],
            *factor_milli,
            out_dir,
            cancel,
        ),
        Operation::ExtractAudio { to } => {
            extract_audio(&require_ffmpeg()?, &inputs[0], *to, out_dir, cancel)
        }
        Operation::PdfToText => pdf_to_txt(&inputs[0], out_dir),
        Operation::PdfToMarkdown => {
            let desired: PathBuf = desired_output(&inputs[0], "", "md", out_dir);
            write_output(&desired, |out: &mut File| pdf_to_markdown(&inputs[0], out))
        }
        Operation::CreateArchive { format } => create_archive(inputs, *format, out_dir, cancel),
        Operation::ExtractArchive => extract_archive(&inputs[0], out_dir, cancel),
    }
}

/// Giải nén một file nén; định dạng lấy từ nội dung file. Không phải file nén thì báo lỗi trước khi giải nén.
fn extract_archive(input: &Path, out_dir: Option<&Path>, cancel: &CancelToken) -> Result<PathBuf> {
    match detect(input)? {
        FileKind::Archive(format) => extract(input, format, out_dir, cancel),
        _ => Err(MorphixError::Unsupported {
            path: input.to_path_buf(),
        }),
    }
}

/// Lớp chữ của một PDF ra TXT. Ghi qua file tạm rồi đổi tên: trang không có chữ làm lỗi thì file tạm
/// bị xoá, nên không còn file TXT cụt. Lỗi đọc PDF đã mang đường dẫn file gốc từ `pdf_to_text`.
fn pdf_to_txt(input: &Path, out_dir: Option<&Path>) -> Result<PathBuf> {
    let desired: PathBuf = desired_output(input, "", "txt", out_dir);
    write_output(&desired, |out: &mut File| pdf_to_text(input, out))
}

/// Đường dẫn ffmpeg trong PATH. Thiếu thì nói rõ cách cài, thay vì để việc chạy hỏng khó hiểu.
pub fn require_ffmpeg() -> Result<PathBuf> {
    find_ffmpeg().ok_or_else(|| {
        MorphixError::InvalidInput(
            "chưa có ffmpeg: tải bản ffmpeg dựng sẵn rồi đặt ffmpeg (ffmpeg.exe trên Windows) vào một \
             thư mục trong PATH của bạn, ví dụ ~/.local/bin, hoặc cài ffmpeg qua trình quản lý gói"
                .to_string(),
        )
    })
}

/// Chuyển một file văn bản. Kiểm cặp định dạng và đọc loại file trước khi tạo file tạm, nên việc
/// không hợp lệ không để lại gì trong thư mục.
fn convert_text(
    input: &Path,
    to: TextFormat,
    out_dir: Option<&Path>,
    warnings: &mut Vec<String>,
) -> Result<PathBuf> {
    let from: TextFormat = match detect(input)? {
        FileKind::Text(format) => format,
        _ => {
            return Err(MorphixError::Unsupported {
                path: input.to_path_buf(),
            })
        }
    };
    if !text_pair_supported(from, to) {
        return Err(MorphixError::InvalidInput(format!(
            "không chuyển được {} sang {}: {}",
            from.extension().to_uppercase(),
            to.extension().to_uppercase(),
            // WHY: TXT và MD không có mốc thời gian, nên không có gì để tạo ra phụ đề.
            if matches!(from, TextFormat::Txt | TextFormat::Markdown)
                && matches!(to, TextFormat::Srt | TextFormat::Vtt)
            {
                "văn bản thuần không có mốc thời gian để tạo phụ đề"
            } else {
                "khác nhóm định dạng (văn bản, phụ đề, bảng, tài liệu)"
            }
        )));
    }
    let desired: PathBuf = desired_output(input, "", to.extension(), out_dir);
    write_output(&desired, |out: &mut File| {
        write_converted(input, from, to, out, warnings)
    })
    .map_err(|error| attach_output_path(error, &desired))
}

/// Đọc đầu vào theo nhóm rồi ghi theo định dạng đích.
fn write_converted(
    input: &Path,
    from: TextFormat,
    to: TextFormat,
    out: &mut File,
    warnings: &mut Vec<String>,
) -> Result<()> {
    // WHY: ra MD thì thêm ký hiệu hợp với nguồn, từ MD ra thì bỏ ký hiệu; cùng định dạng thì chép nguyên.
    match (from, to) {
        (TextFormat::Markdown, TextFormat::Txt) => {
            write_text(&markdown_to_plain(&read_utf8(input)?), out)
        }
        (TextFormat::Txt, TextFormat::Markdown) => {
            write_text(&escape_plain(&read_utf8(input)?), out)
        }
        (TextFormat::Markdown, TextFormat::Html | TextFormat::Docx)
        | (TextFormat::Html | TextFormat::Docx, _) => {
            write_document(input, from, to, out, warnings)
        }
        (TextFormat::Txt | TextFormat::Markdown, _) => write_text(&read_utf8(input)?, out),
        (TextFormat::Srt | TextFormat::Vtt, _) => {
            let cues = read_cues(input)?;
            match to {
                TextFormat::Txt => write_text(&strip_timestamps(&cues), out),
                TextFormat::Markdown => write_text(&cues_to_markdown(&cues), out),
                _ => write_cues(&cues, to, out),
            }
        }
        (TextFormat::Csv | TextFormat::Tsv, TextFormat::Markdown) => {
            write_text(&table_to_markdown(&read_table(input, from)?), out)
        }
        (TextFormat::Csv | TextFormat::Tsv, _) => write_table(&read_table(input, from)?, to, out),
    }
}

/// Tài liệu có định dạng (MD, HTML, DOCX): đọc nguồn thành cây tài liệu rồi ghi thẳng sang đích, không đi
/// qua Markdown. WHY: DOCX ↔ HTML giữ được gạch chân, ô gộp, ảnh và chú thích mà Markdown không chứa.
fn write_document(
    input: &Path,
    from: TextFormat,
    to: TextFormat,
    out: &mut File,
    warnings: &mut Vec<String>,
) -> Result<()> {
    let name = input.file_name().unwrap_or_default().to_string_lossy();
    let title = input.file_stem().unwrap_or_default().to_string_lossy();
    let doc = match from {
        TextFormat::Html => html::read(&read_utf8(input)?, input.parent()),
        TextFormat::Docx => docx::read(input)?,
        _ => markdown::read(&read_utf8(input)?, input.parent()),
    };
    let dropped = match to {
        TextFormat::Txt => {
            let (text, dropped) = write_plain(&doc);
            write_text(&text, out)?;
            dropped
        }
        // WHY: MD → HTML giữ đường `push_html` như D2: nó giữ mọi thứ Markdown có, không cần qua cây.
        TextFormat::Html if from == TextFormat::Markdown => {
            write_text(&markdown_to_html(&read_utf8(input)?, &title), out)?;
            doc.dropped
        }
        TextFormat::Html => {
            let (page, dropped) = html::write(&doc, &title);
            write_text(&page, out)?;
            dropped
        }
        TextFormat::Docx => docx::write(&doc, out)?,
        TextFormat::Markdown => {
            let (markdown, dropped) = markdown::write(&doc);
            write_text(&markdown, out)?;
            dropped
        }
        TextFormat::Srt | TextFormat::Vtt | TextFormat::Csv | TextFormat::Tsv => {
            return Err(MorphixError::InvalidInput(format!(
                "không chuyển được tài liệu sang {}",
                to.extension().to_uppercase()
            )))
        }
    };
    if dropped.images > 0 {
        warnings.push(format!(
            "{name}: bỏ {} ảnh (chỉ giữ chữ thay thế)",
            dropped.images
        ));
    }
    let mut left_out: Vec<String> = Vec::new();
    if dropped.notes > 0 {
        left_out.push(format!("{} chú thích/ghi chú", dropped.notes));
    }
    if dropped.headers {
        left_out.push("đầu/chân trang".to_string());
    }
    if dropped.md_downgraded {
        left_out.push("định dạng MD không có (gạch chân, trên/dưới, ô gộp)".to_string());
    }
    if !left_out.is_empty() {
        warnings.push(format!(
            "{name}: bỏ {} (chỉ giữ nội dung chính)",
            left_out.join(" và ")
        ));
    }
    Ok(())
}

/// Lỗi ghi từ `ops::text` không biết đường dẫn file đích nên để trống; gắn đường dẫn thật để người
/// dùng biết file nào không ghi được. Lỗi đã có đường dẫn (đọc file, tạo file tạm) giữ nguyên.
fn attach_output_path(error: MorphixError, output: &Path) -> MorphixError {
    match error {
        MorphixError::Io { path, source } if path.as_os_str().is_empty() => MorphixError::Io {
            path: output.to_path_buf(),
            source,
        },
        other => other,
    }
}

/// Cảnh báo khi ảnh đổi sang WebP/AVIF/JPEG ra không nhỏ hơn bản gốc. WHY chỉ ba đích này: người dùng
/// chọn chúng để nhỏ đi; PNG/TIFF/BMP/GIF to hơn bản gốc là bình thường nên không cảnh báo.
fn size_warning(input: &Path, input_len: u64, output_len: u64, to: ImgFormat) -> Option<String> {
    if !matches!(to, ImgFormat::WebP | ImgFormat::Avif | ImgFormat::Jpeg) {
        return None;
    }
    size_message(input, input_len, output_len)
}

/// Thông điệp khi file ra không nhỏ hơn file gốc; dùng chung cho ảnh và media. `None` khi file ra nhỏ hơn.
pub(crate) fn size_message(input: &Path, input_len: u64, output_len: u64) -> Option<String> {
    if output_len < input_len {
        return None;
    }
    let name = input
        .file_name()
        .unwrap_or(input.as_os_str())
        .to_string_lossy();
    Some(if output_len == input_len {
        format!("{name}: file mới không nhỏ hơn file gốc")
    } else {
        // WHY checked_div: file gốc 0 byte không có phần trăm để nói, chỉ báo là lớn hơn.
        match ((output_len - input_len) * 100).checked_div(input_len) {
            Some(percent) => format!("{name}: file mới lớn hơn file gốc {percent}%"),
            None => format!("{name}: file mới lớn hơn file gốc"),
        }
    })
}

/// Cảnh báo của một mục đã chạy xong. Không đọc được kích thước thì chỉ ghi log: file đã ghi xong,
/// không đáng biến cả việc thành lỗi chỉ vì thiếu một lời nhắc.
fn item_warnings(job: &Job, inputs: &[PathBuf], result: &Result<PathBuf>) -> Vec<String> {
    let Ok(output) = result else {
        return Vec::new();
    };
    let sizes = |input_len: std::io::Result<u64>| {
        input_len.and_then(|i| std::fs::metadata(output).map(|o| (i, o.len())))
    };
    let warning = match &job.operation {
        Operation::ConvertImage { to } => {
            let input = &inputs[0];
            // WHY: đổi định dạng chỉ giải mã khung đầu; GIF động mất chuyển động mà không ai biết.
            let animated = matches!(detect(input), Ok(FileKind::Image(ImgFormat::Gif)))
                .then(|| crate::ops::image_io::is_animated_gif(input));
            let mut warnings: Vec<String> = match animated.unwrap_or(Ok(false)) {
                Ok(true) => vec![format!(
                    "{}: GIF động, chỉ giữ khung đầu",
                    input.file_name().unwrap_or_default().to_string_lossy()
                )],
                Ok(false) => Vec::new(),
                Err(error) => {
                    tracing::warn!(%error, ?input, "không kiểm được GIF động");
                    Vec::new()
                }
            };
            match sizes(std::fs::metadata(input).map(|m| m.len())) {
                Ok((i, o)) => warnings.extend(size_warning(input, i, o, *to)),
                Err(error) => {
                    tracing::warn!(%error, ?output, "không đọc được kích thước để so sánh")
                }
            }
            return warnings;
        }
        Operation::MergeImagesToPdf => {
            let total = inputs.iter().try_fold(0u64, |sum, input| {
                std::fs::metadata(input).map(|m| sum + m.len())
            });
            sizes(total).map(|(i, o)| merge_size_warning(output, i, o))
        }
        _ => return Vec::new(),
    };
    match warning {
        Ok(warning) => warning.into_iter().collect(),
        Err(error) => {
            tracing::warn!(%error, ?output, "không đọc được kích thước để so sánh");
            Vec::new()
        }
    }
}

/// Cảnh báo khi PDF gộp to hơn tổng ảnh gốc. WHY dung sai 5% + 2 KB: khung PDF (trang, từ điển) luôn
/// cộng thêm vài trăm byte; chỉ phần phình thật (vd. ảnh PNG/WebP nở thành điểm ảnh) mới đáng nhắc.
/// shortcut: dung sai cố định, nâng cấp nếu người dùng ghép nhiều ảnh rất nhỏ.
fn merge_size_warning(pdf: &Path, input_total: u64, output_len: u64) -> Option<String> {
    if output_len > input_total + input_total / 20 + 2048 {
        size_message(pdf, input_total, output_len)
    } else {
        None
    }
}

/// Chạy tuần tự từng mục. Một mục lỗi **không chặn** các mục sau; huỷ thì mọi mục chưa chạy trả
/// `Cancelled`. Mỗi lỗi được ghi log kèm đường dẫn và nguyên nhân (không nuốt lỗi im lặng).
pub fn run_job(job: &Job, cancel: &CancelToken, on_progress: &dyn Fn(Progress)) -> JobReport {
    let groups = item_groups(job);
    let total = groups.len();
    let mut items = Vec::with_capacity(total);
    for (index, inputs) in groups.into_iter().enumerate() {
        let mut warnings: Vec<String> = Vec::new();
        let result = if cancel.is_cancelled() {
            Err(MorphixError::Cancelled)
        } else {
            run_item(job, &inputs, cancel, &mut warnings)
        };
        if let Err(error) = &result {
            if !matches!(error, MorphixError::Cancelled) {
                tracing::error!(inputs = ?inputs, %error, "việc thất bại");
            }
        }
        // WHY: việc lỗi thì mọi cảnh báo thu được trước đó không còn ý nghĩa; chỉ báo cho việc đã ghi file.
        if result.is_ok() {
            warnings.extend(item_warnings(job, &inputs, &result));
        } else {
            warnings.clear();
        }
        items.push(ItemReport {
            inputs,
            result,
            warnings,
        });
        on_progress(Progress {
            done: index + 1,
            total,
        });
    }
    JobReport { items }
}

/// Đuôi đầu ra của việc media giữ định dạng gốc: các việc này ghi theo `MediaFormat::extension`
/// (`.MP4` thành `.mp4`), nên dry-run cũng phải vậy. Không đọc được loại thì lùi về đuôi gốc.
fn media_extension(input: &Path) -> String {
    match detect(input) {
        Ok(FileKind::Media(format)) => format.extension().to_string(),
        _ => kept_extension(input),
    }
}

/// Đuôi file đầu ra khi giữ nguyên định dạng: đuôi gốc, hoặc đuôi theo nội dung nếu file không có đuôi.
fn kept_extension(input: &Path) -> String {
    let fallback = match detect(input) {
        Ok(FileKind::Image(format)) => format.extension(),
        _ => "bin",
    };
    extension_of(input, fallback)
}

fn planned_output(job: &Job, inputs: &[PathBuf]) -> PathBuf {
    let out_dir = job.out_dir.as_deref();
    let input = &inputs[0];
    match &job.operation {
        Operation::MergeImagesToPdf => {
            let mut sorted = inputs.to_vec();
            natural_sort(&mut sorted);
            pdf_destination(&sorted, out_dir)
        }
        Operation::ConvertImage { to } => desired_output(input, "", to.extension(), out_dir),
        Operation::CompressImage => {
            desired_output(input, SUFFIX_COMPRESSED, &kept_extension(input), out_dir)
        }
        Operation::StripMetadata => {
            desired_output(input, SUFFIX_STRIPPED, &kept_extension(input), out_dir)
        }
        Operation::ConvertText { to } => desired_output(input, "", to.extension(), out_dir),
        Operation::PdfToText => desired_output(input, "", "txt", out_dir),
        Operation::PdfToMarkdown => desired_output(input, "", "md", out_dir),
        Operation::CreateArchive { format } => {
            desired_output(input, SUFFIX_ARCHIVE, format.extension(), out_dir)
        }
        // WHY: dry-run không đọc được loại file thì chỉ in đường dẫn file; lỗi thật sẽ báo khi chạy.
        Operation::ExtractArchive => match detect(input) {
            Ok(FileKind::Archive(format)) => planned_folder(input, format, out_dir),
            _ => input.to_path_buf(),
        },
        Operation::ConvertMedia { to } | Operation::ExtractAudio { to } => {
            desired_output(input, "", to.extension(), out_dir)
        }
        Operation::CompressMedia => {
            desired_output(input, SUFFIX_COMPRESSED, &media_extension(input), out_dir)
        }
        Operation::CutMedia { .. } => {
            desired_output(input, SUFFIX_CUT, &media_extension(input), out_dir)
        }
        Operation::ChangeSpeed { .. } => {
            desired_output(input, SUFFIX_SPEED, &media_extension(input), out_dir)
        }
        Operation::ResizeImage(spec) => {
            let suffix = display_dimensions(input)
                .and_then(|(width, height)| target_dimensions(width, height, *spec))
                .map(|(width, height)| resize_suffix(width, height))
                .unwrap_or_else(|error| {
                    // Không đoán được kích thước (file hỏng, yêu cầu sai): để việc chạy thật báo lỗi chính thức.
                    tracing::warn!(input = %input.display(), %error, "không đoán được tên đầu ra khi đổi kích thước");
                    "-resized".to_string()
                });
            desired_output(input, &suffix, &kept_extension(input), out_dir)
        }
    }
}

/// Tên đầu ra dự kiến cho từng mục (cho `--dry-run`). Chỉ đọc phần đầu file; không xử lý trùng tên
/// (tên thật có thể thêm ` (1)`).
pub fn planned_outputs(job: &Job) -> Vec<(Vec<PathBuf>, PathBuf)> {
    item_groups(job)
        .into_iter()
        .filter(|inputs| !inputs.is_empty())
        .map(|inputs| {
            let output = planned_output(job, &inputs);
            (inputs, output)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn size_warning_only_for_shrinking_targets_that_did_not_shrink() {
        let input = Path::new("/x/a.png");
        assert_eq!(size_warning(input, 1000, 500, ImgFormat::WebP), None);
        let same = size_warning(input, 1000, 1000, ImgFormat::WebP).unwrap();
        assert!(same.starts_with("a.png: ") && same.contains("không nhỏ hơn"));
        let bigger = size_warning(input, 1000, 2000, ImgFormat::Avif).unwrap();
        assert!(bigger.contains("lớn hơn file gốc 100%"), "{bigger}");
        assert_eq!(size_warning(input, 1000, 2000, ImgFormat::Png), None);
    }

    #[test]
    fn merge_size_warning_tolerates_the_pdf_frame_but_not_real_growth() {
        let pdf = Path::new("/x/a-gop.pdf");
        assert_eq!(merge_size_warning(pdf, 100_000, 100_500), None);
        assert_eq!(merge_size_warning(pdf, 100_000, 50_000), None);
        let grown = merge_size_warning(pdf, 100_000, 120_000).unwrap();
        assert!(
            grown.starts_with("a-gop.pdf: ") && grown.contains("lớn hơn file gốc 20%"),
            "{grown}"
        );
    }

    #[test]
    fn size_message_is_format_independent() {
        let input = Path::new("/x/a.mp3");
        assert!(size_message(input, 1000, 2000)
            .unwrap()
            .contains("lớn hơn file gốc 100%"));
        assert_eq!(size_message(input, 1000, 500), None);
    }

    #[test]
    fn size_warning_with_empty_input_has_no_percent() {
        let warning = size_warning(Path::new("a.png"), 0, 10, ImgFormat::Jpeg).unwrap();
        assert!(!warning.contains('%'), "{warning}");
    }

    #[test]
    fn write_error_without_path_gets_output_path() {
        let output = Path::new("/x/a.vtt");
        let error = MorphixError::Io {
            path: PathBuf::new(),
            source: std::io::Error::other("đầy đĩa"),
        };
        match attach_output_path(error, output) {
            MorphixError::Io { path, .. } => assert_eq!(path, output),
            other => panic!("cần Io có đường dẫn, được {other:?}"),
        }
    }

    #[test]
    fn error_with_path_is_kept() {
        let error = MorphixError::Io {
            path: PathBuf::from("/x/nguon.srt"),
            source: std::io::Error::other("hỏng"),
        };
        match attach_output_path(error, Path::new("/x/a.vtt")) {
            MorphixError::Io { path, .. } => assert_eq!(path, PathBuf::from("/x/nguon.srt")),
            other => panic!("cần Io giữ đường dẫn cũ, được {other:?}"),
        }
    }
}
