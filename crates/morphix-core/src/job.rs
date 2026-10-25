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
        image_io::convert_image,
        image_pdf::{images_to_pdf, pdf_destination},
        media::{
            change_speed, compress_media, convert_media, cut_media, extract_audio, SUFFIX_CUT,
            SUFFIX_SPEED,
        },
        pdf_text::pdf_to_text,
        strip_meta::{strip_metadata, SUFFIX_STRIPPED},
        text::{
            read_cues, read_table, read_utf8, strip_timestamps, write_cues, write_table, write_text,
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

/// Chia đầu vào thành các mục độc lập: gộp PDF là một mục chứa tất cả, các việc khác mỗi file một mục.
fn item_groups(job: &Job) -> Vec<Vec<PathBuf>> {
    match job.operation {
        Operation::MergeImagesToPdf | Operation::CreateArchive { .. } => vec![job.inputs.clone()],
        _ => job.inputs.iter().map(|input| vec![input.clone()]).collect(),
    }
}

fn run_item(job: &Job, inputs: &[PathBuf], cancel: &CancelToken) -> Result<PathBuf> {
    let out_dir = job.out_dir.as_deref();
    match &job.operation {
        Operation::MergeImagesToPdf => images_to_pdf(inputs, out_dir, cancel),
        Operation::ConvertImage { to } => convert_image(&inputs[0], *to, job.quality, out_dir),
        Operation::CompressImage => compress_image(&inputs[0], job.quality, out_dir),
        Operation::StripMetadata => strip_metadata(&inputs[0], out_dir),
        Operation::ResizeImage(spec) => resize_image(&inputs[0], *spec, job.quality, out_dir),
        Operation::ConvertText { to } => convert_text(&inputs[0], *to, out_dir),
        Operation::ConvertMedia { to } => {
            convert_media(&require_ffmpeg()?, &inputs[0], *to, out_dir, cancel)
        }
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
            "chưa có ffmpeg: cài bằng `sudo apt install ffmpeg` trên Linux, hoặc tải bản ffmpeg \
             cho Windows rồi thêm thư mục chứa ffmpeg.exe vào PATH"
                .to_string(),
        )
    })
}

/// Chuyển một file văn bản. Kiểm cặp định dạng và đọc loại file trước khi tạo file tạm, nên việc
/// không hợp lệ không để lại gì trong thư mục.
fn convert_text(input: &Path, to: TextFormat, out_dir: Option<&Path>) -> Result<PathBuf> {
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
            if matches!(from, TextFormat::Txt | TextFormat::Markdown) {
                "văn bản thuần không có mốc thời gian để tạo phụ đề"
            } else {
                "khác nhóm định dạng (văn bản, phụ đề, bảng)"
            }
        )));
    }
    let desired: PathBuf = desired_output(input, "", to.extension(), out_dir);
    write_output(&desired, |out: &mut File| {
        write_converted(input, from, to, out)
    })
    .map_err(|error| attach_output_path(error, &desired))
}

/// Đọc đầu vào theo nhóm rồi ghi theo định dạng đích.
fn write_converted(input: &Path, from: TextFormat, to: TextFormat, out: &mut File) -> Result<()> {
    match from {
        TextFormat::Txt | TextFormat::Markdown => write_text(&read_utf8(input)?, out),
        TextFormat::Srt | TextFormat::Vtt => {
            let cues = read_cues(input)?;
            if to == TextFormat::Txt {
                write_text(&strip_timestamps(&cues), out)
            } else {
                write_cues(&cues, to, out)
            }
        }
        TextFormat::Csv | TextFormat::Tsv => write_table(&read_table(input, from)?, to, out),
    }
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

/// Chạy tuần tự từng mục. Một mục lỗi **không chặn** các mục sau; huỷ thì mọi mục chưa chạy trả
/// `Cancelled`. Mỗi lỗi được ghi log kèm đường dẫn và nguyên nhân (không nuốt lỗi im lặng).
pub fn run_job(job: &Job, cancel: &CancelToken, on_progress: &dyn Fn(Progress)) -> JobReport {
    let groups = item_groups(job);
    let total = groups.len();
    let mut items = Vec::with_capacity(total);
    for (index, inputs) in groups.into_iter().enumerate() {
        let result = if cancel.is_cancelled() {
            Err(MorphixError::Cancelled)
        } else {
            run_item(job, &inputs, cancel)
        };
        if let Err(error) = &result {
            if !matches!(error, MorphixError::Cancelled) {
                tracing::error!(inputs = ?inputs, %error, "việc thất bại");
            }
        }
        items.push(ItemReport { inputs, result });
        on_progress(Progress {
            done: index + 1,
            total,
        });
    }
    JobReport { items }
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
            desired_output(input, SUFFIX_COMPRESSED, &kept_extension(input), out_dir)
        }
        Operation::CutMedia { .. } => {
            desired_output(input, SUFFIX_CUT, &kept_extension(input), out_dir)
        }
        Operation::ChangeSpeed { .. } => {
            desired_output(input, SUFFIX_SPEED, &kept_extension(input), out_dir)
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
