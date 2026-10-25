//! Giải nén ZIP, TAR.GZ và 7Z. Mỗi entry được kiểm đường dẫn trước khi ghi; file được ghi vào thư mục
//! tạm cạnh đích rồi mới đổi tên, nên huỷ hoặc lỗi không để lại thư mục dở.

use std::{
    fs::File,
    io::{self, BufReader, Read, Write},
    path::{Component, Path, PathBuf},
};

use flate2::{read::GzDecoder, write::GzEncoder, Compression};

use crate::{
    cancel::CancelToken,
    error::{MorphixError, Result},
    kind::ArchiveFormat,
    naming::{desired_output, write_output},
};

/// Giới hạn số lần thử ` (n)` cho tên thư mục đích, như `naming.rs`.
const MAX_FOLDER_SUFFIX: u32 = 9999;

/// Hậu tố của file nén tạo ra: `bundle.jpg` thành `bundle-nen.zip`.
pub const SUFFIX_ARCHIVE: &str = "-nen";

/// Hàm ghi một định dạng: nhận đầu vào, tên entry, đích (cho thông báo lỗi), file tạm và cờ huỷ.
type ArchiveWriter = fn(&[PathBuf], &[String], &Path, &mut File, &CancelToken) -> Result<()>;

/// Gộp `inputs` thành **một** file nén `format` cạnh đầu vào đầu tiên (hoặc trong `out_dir`), tên
/// `<stem>-nen.<đuôi>`. Mỗi entry lưu theo tên file, không kèm thư mục nguồn nên không lộ đường dẫn tuyệt đối.
/// Chỉ nhận file thường; file nguồn chỉ được đọc.
pub fn create_archive(
    inputs: &[PathBuf],
    format: ArchiveFormat,
    out_dir: Option<&Path>,
    cancel: &CancelToken,
) -> Result<PathBuf> {
    let writer: ArchiveWriter = match format {
        ArchiveFormat::Zip => write_zip,
        ArchiveFormat::TarGz => write_tar_gz,
        ArchiveFormat::SevenZ => write_7z,
        // WHY: RAR chỉ đọc (xem `extract`), không có thư viện tạo RAR được phép dùng.
        ArchiveFormat::Rar => {
            return Err(MorphixError::Unsupported {
                path: inputs.first().cloned().unwrap_or_default(),
            })
        }
    };
    cancel.check()?;
    let names: Vec<String> = entry_names(inputs)?;
    let first: &PathBuf = &inputs[0];
    let desired: PathBuf = desired_output(first, SUFFIX_ARCHIVE, format.extension(), out_dir);
    // WHY: kiểm huỷ cuối cùng trước khi đổi tên, vì `write_output` sẽ giữ file nếu closure trả Ok.
    write_output(&desired, |out: &mut File| {
        writer(inputs, &names, &desired, out, cancel)?;
        cancel.check()
    })
}

/// Tên entry của từng đầu vào. Từ chối thư mục và trùng tên: nếu không, một entry sẽ âm thầm đè entry kia.
fn entry_names(inputs: &[PathBuf]) -> Result<Vec<String>> {
    if inputs.is_empty() {
        return Err(MorphixError::InvalidInput(
            "chưa chọn file nào để nén".to_string(),
        ));
    }
    let mut names: Vec<String> = Vec::with_capacity(inputs.len());
    for input in inputs {
        let meta: std::fs::Metadata =
            std::fs::metadata(input).map_err(|source| io_err(input, source))?;
        if !meta.is_file() {
            return Err(MorphixError::InvalidInput(format!(
                "{} không phải file thường, chưa nén được thư mục",
                input.display()
            )));
        }
        let name: String = input
            .file_name()
            .and_then(|n| n.to_str())
            .map(str::to_string)
            .ok_or_else(|| {
                MorphixError::InvalidInput(format!("tên file không đọc được: {}", input.display()))
            })?;
        if names.contains(&name) {
            return Err(MorphixError::InvalidInput(format!(
                "hai file cùng tên {name} trong lần nén, không gộp được vì file nén không chứa hai mục trùng tên"
            )));
        }
        names.push(name);
    }
    Ok(names)
}

fn write_zip(
    inputs: &[PathBuf],
    names: &[String],
    dest: &Path,
    out: &mut File,
    cancel: &CancelToken,
) -> Result<()> {
    let mut zip = zip::ZipWriter::new(out);
    let options: zip::write::SimpleFileOptions = zip::write::SimpleFileOptions::default();
    for (input, name) in inputs.iter().zip(names) {
        cancel.check()?;
        zip.start_file(name.as_str(), options)
            .map_err(|e| write_failed(dest, e))?;
        copy_file(input, &mut zip, dest)?;
    }
    zip.finish().map_err(|e| write_failed(dest, e))?;
    Ok(())
}

fn write_tar_gz(
    inputs: &[PathBuf],
    names: &[String],
    dest: &Path,
    out: &mut File,
    cancel: &CancelToken,
) -> Result<()> {
    let mut builder = tar::Builder::new(GzEncoder::new(out, Compression::default()));
    for (input, name) in inputs.iter().zip(names) {
        cancel.check()?;
        builder
            .append_path_with_name(input, name)
            .map_err(|source| io_err(input, source))?;
    }
    let gz = builder
        .into_inner()
        .map_err(|source| io_err(dest, source))?;
    gz.finish().map_err(|source| io_err(dest, source))?;
    Ok(())
}

fn write_7z(
    inputs: &[PathBuf],
    names: &[String],
    dest: &Path,
    out: &mut File,
    cancel: &CancelToken,
) -> Result<()> {
    let mut writer = sevenz_rust2::ArchiveWriter::new(out).map_err(|e| write_failed(dest, e))?;
    for (input, name) in inputs.iter().zip(names) {
        cancel.check()?;
        let entry = sevenz_rust2::ArchiveEntry::from_path(input, name.clone());
        let source: File = File::open(input).map_err(|source| io_err(input, source))?;
        writer
            .push_archive_entry(entry, Some(source))
            .map_err(|e| write_failed(dest, e))?;
    }
    writer.finish().map_err(|source| io_err(dest, source))?;
    Ok(())
}

/// Chép nội dung một file nguồn vào `sink`. Lỗi mở file nguồn gắn tên nguồn; lỗi ghi gắn đích.
fn copy_file(input: &Path, sink: &mut dyn Write, dest: &Path) -> Result<()> {
    let mut source: File = File::open(input).map_err(|source| io_err(input, source))?;
    io::copy(&mut source, sink).map_err(|source| io_err(dest, source))?;
    Ok(())
}

fn write_failed(
    dest: &Path,
    source: impl Into<Box<dyn std::error::Error + Send + Sync>>,
) -> MorphixError {
    MorphixError::Io {
        path: dest.to_path_buf(),
        source: io::Error::other(source),
    }
}

/// Giải nén `archive` vào một thư mục mới đặt tên theo file (bỏ đuôi), trong `out_dir` hoặc cạnh file.
/// Trả về đường dẫn thư mục đích.
pub fn extract(
    archive: &Path,
    format: ArchiveFormat,
    out_dir: Option<&Path>,
    cancel: &CancelToken,
) -> Result<PathBuf> {
    let unpack: fn(&Path, &Path, &CancelToken) -> Result<()> = match format {
        ArchiveFormat::Zip => unpack_zip,
        ArchiveFormat::TarGz => unpack_tar_gz,
        ArchiveFormat::SevenZ => unpack_7z,
        // WHY: RAR chỉ đọc và làm sau đợt 4, khi giấy phép đã được kiểm.
        ArchiveFormat::Rar => {
            return Err(MorphixError::Unsupported {
                path: archive.to_path_buf(),
            })
        }
    };
    cancel.check()?;

    let name: String = folder_name(archive, format)?;
    let base: &Path = base_dir(archive, out_dir);
    std::fs::create_dir_all(base).map_err(|source| io_err(base, source))?;
    // WHY: thư mục tạm nằm cùng cấp với đích để đổi tên là thao tác trên cùng ổ đĩa. TempDir tự xoá
    // khi hàm trả lỗi hoặc bị huỷ, nên không cần dọn tay ở từng nhánh.
    let work = tempfile::Builder::new()
        .prefix(".morphix-")
        .tempdir_in(base)
        .map_err(|source| io_err(base, source))?;
    unpack(archive, work.path(), cancel)?;

    cancel.check()?;
    let dest: PathBuf = free_folder(base, &name)?;
    let kept: PathBuf = work.keep();
    match std::fs::rename(&kept, &dest) {
        Ok(()) => Ok(dest),
        Err(source) => {
            if let Err(cleanup) = std::fs::remove_dir_all(&kept) {
                tracing::warn!(path = %kept.display(), %cleanup, "không xoá được thư mục giải nén dở");
            }
            Err(io_err(&dest, source))
        }
    }
}

/// Thư mục cha của thư mục giải nén: `out_dir`, hoặc thư mục chứa file nén; không có thì thư mục hiện tại.
fn base_dir<'a>(archive: &'a Path, out_dir: Option<&'a Path>) -> &'a Path {
    match out_dir.or_else(|| archive.parent()) {
        Some(dir) if !dir.as_os_str().is_empty() => dir,
        _ => Path::new("."),
    }
}

/// Thư mục `extract` sẽ tạo nếu không trùng tên, cho `--dry-run`. Không xử lý trùng tên (tên thật có thể thêm ` (n)`).
pub fn planned_folder(archive: &Path, format: ArchiveFormat, out_dir: Option<&Path>) -> PathBuf {
    match folder_name(archive, format) {
        Ok(name) => base_dir(archive, out_dir).join(name),
        Err(error) => {
            tracing::warn!(path = %archive.display(), %error, "không đoán được thư mục giải nén: {error}");
            archive.to_path_buf()
        }
    }
}

/// Tên thư mục đích: tên file, bỏ đuôi. Với tar.gz bỏ thêm `.tar` để không ra `bundle.tar`.
fn folder_name(archive: &Path, format: ArchiveFormat) -> Result<String> {
    let mut name: String = archive
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    if format == ArchiveFormat::TarGz && name.ends_with(".tar") {
        name.truncate(name.len() - ".tar".len());
    }
    if name.is_empty() {
        return Err(MorphixError::InvalidInput(format!(
            "không đặt được tên thư mục cho {}",
            archive.display()
        )));
    }
    Ok(name)
}

/// Thư mục `name` nếu còn trống tên, không thì `name (1)`, `name (2)`... Không đụng vào thư mục đã có.
fn free_folder(base: &Path, name: &str) -> Result<PathBuf> {
    for attempt in 0..=MAX_FOLDER_SUFFIX {
        let candidate: PathBuf = if attempt == 0 {
            base.join(name)
        } else {
            base.join(format!("{name} ({attempt})"))
        };
        match std::fs::symlink_metadata(&candidate) {
            Err(source) if source.kind() == io::ErrorKind::NotFound => return Ok(candidate),
            Ok(_) => {}
            Err(source) => return Err(io_err(&candidate, source)),
        }
    }
    Err(MorphixError::InvalidInput(format!(
        "quá nhiều thư mục trùng tên với {name}"
    )))
}

fn unpack_zip(archive: &Path, root: &Path, cancel: &CancelToken) -> Result<()> {
    let file: File = File::open(archive).map_err(|source| io_err(archive, source))?;
    let mut zip = zip::ZipArchive::new(BufReader::new(file))
        .map_err(|e| corrupt(archive, format!("không đọc được mục lục file nén: {e}")))?;
    for index in 0..zip.len() {
        cancel.check()?;
        let mut entry = zip
            .by_index(index)
            .map_err(|e| corrupt(archive, format!("không đọc được mục {index}: {e}")))?;
        let name: PathBuf = PathBuf::from(
            entry
                .name()
                .map_err(|e| corrupt(archive, format!("tên mục không đọc được: {e}")))?
                .as_ref(),
        );
        let is_dir: bool = entry.is_dir();
        write_entry(root, archive, &name, is_dir, &mut entry)?;
    }
    Ok(())
}

fn unpack_tar_gz(archive: &Path, root: &Path, cancel: &CancelToken) -> Result<()> {
    let file: File = File::open(archive).map_err(|source| io_err(archive, source))?;
    let mut tar = tar::Archive::new(GzDecoder::new(BufReader::new(file)));
    let entries = tar
        .entries()
        .map_err(|e| corrupt(archive, format!("không đọc được mục lục file nén: {e}")))?;
    for entry in entries {
        cancel.check()?;
        let mut entry =
            entry.map_err(|e| corrupt(archive, format!("không đọc được một mục: {e}")))?;
        let kind = entry.header().entry_type();
        // WHY: pax global chỉ chứa siêu dữ liệu, không có nội dung để ghi.
        if kind.is_pax_global_extensions() {
            continue;
        }
        let name: PathBuf = entry
            .path()
            .map_err(|e| corrupt(archive, format!("đường dẫn mục không đọc được: {e}")))?
            .into_owned();
        // WHY: liên kết mềm hoặc cứng có thể trỏ ra ngoài thư mục đích, rồi entry sau ghi xuyên qua nó.
        // Chỉ nhận file và thư mục thường nên liên kết và thiết bị bị từ chối luôn.
        if !(kind.is_file() || kind.is_dir()) {
            return Err(corrupt(
                archive,
                format!(
                    "loại mục không được giải nén (liên kết hoặc thiết bị): {}",
                    name.display()
                ),
            ));
        }
        write_entry(root, archive, &name, kind.is_dir(), &mut entry)?;
    }
    Ok(())
}

fn unpack_7z(archive: &Path, root: &Path, cancel: &CancelToken) -> Result<()> {
    let file: File = File::open(archive).map_err(|source| io_err(archive, source))?;
    let mut reader = sevenz_rust2::ArchiveReader::new(file, sevenz_rust2::Password::empty())
        .map_err(|e| corrupt(archive, format!("không đọc được mục lục file nén: {e}")))?;
    // WHY: callback của thư viện chỉ trả lỗi của chính nó. Giữ lỗi Morphix (huỷ, đường dẫn xấu) ở đây
    // rồi trả về sau, để người dùng thấy đúng nguyên nhân thay vì một lỗi chung.
    let mut failure: Option<MorphixError> = None;
    let result = reader.for_each_entries(|entry, data| {
        let step: Result<()> = cancel.check().and_then(|()| {
            write_entry(
                root,
                archive,
                Path::new(entry.name()),
                entry.is_directory(),
                data,
            )
        });
        match step {
            Ok(()) => Ok(true),
            Err(error) => {
                failure = Some(error);
                Ok(false)
            }
        }
    });
    if let Some(error) = failure {
        return Err(error);
    }
    result.map_err(|e| corrupt(archive, format!("không giải nén được file 7z: {e}")))
}

/// Ghi một entry vào `root`, sau khi đường dẫn đã được kiểm.
fn write_entry(
    root: &Path,
    archive: &Path,
    name: &Path,
    is_dir: bool,
    data: &mut dyn Read,
) -> Result<()> {
    let Some(target) = entry_target(root, archive, name)? else {
        return Ok(());
    };
    if is_dir {
        return std::fs::create_dir_all(&target).map_err(|source| io_err(&target, source));
    }
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent).map_err(|source| io_err(parent, source))?;
    }
    let mut out = File::create(&target).map_err(|source| io_err(&target, source))?;
    io::copy(data, &mut out).map_err(|source| io_err(&target, source))?;
    Ok(())
}

/// Đường dẫn đích của entry trong `root`. `None` nếu entry là gốc (`./`) và không có gì để ghi.
/// Đường dẫn không an toàn thì trả `Corrupt` kèm tên entry.
fn entry_target(root: &Path, archive: &Path, name: &Path) -> Result<Option<PathBuf>> {
    let relative: PathBuf = safe_relative(name).ok_or_else(|| {
        corrupt(
            archive,
            format!("đường dẫn không an toàn trong file nén: {}", name.display()),
        )
    })?;
    if relative.as_os_str().is_empty() {
        return Ok(None);
    }
    let target: PathBuf = root.join(&relative);
    // WHY: kiểm lại sau khi nối. Với các bước ở trên, đích luôn nằm trong root; đây là lớp phòng thủ
    // thứ hai, không tin riêng việc thư viện nén đã làm sạch đường dẫn.
    if !target.starts_with(root) {
        return Err(corrupt(
            archive,
            format!("đường dẫn thoát khỏi thư mục đích: {}", name.display()),
        ));
    }
    Ok(Some(target))
}

/// Chỉ giữ các thành phần thường. `None` nếu có thành phần gốc, ổ đĩa (Windows) hoặc `..`.
fn safe_relative(name: &Path) -> Option<PathBuf> {
    let mut relative = PathBuf::new();
    for component in name.components() {
        match component {
            Component::Normal(part) => relative.push(part),
            Component::CurDir => {}
            // WHY: RootDir và Prefix làm đường nối bỏ qua thư mục đích; ParentDir đi lên khỏi nó.
            Component::RootDir | Component::Prefix(_) | Component::ParentDir => return None,
        }
    }
    Some(relative)
}

fn corrupt(archive: &Path, reason: String) -> MorphixError {
    MorphixError::Corrupt {
        path: archive.to_path_buf(),
        reason,
    }
}

fn io_err(path: &Path, source: io::Error) -> MorphixError {
    MorphixError::Io {
        path: path.to_path_buf(),
        source,
    }
}
