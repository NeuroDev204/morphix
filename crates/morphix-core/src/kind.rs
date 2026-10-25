use std::{fs::File, io::Read, path::Path};

use crate::error::{MorphixError, Result};

/// Định dạng ảnh Morphix đọc hoặc ghi được. Thứ tự trong `ALL_ORDERED` là thứ tự các múi
/// của vòng tròn, nên đổi thứ tự là đổi giao diện.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ImgFormat {
    Jpeg,
    Png,
    WebP,
    Avif,
    Gif,
    Tiff,
    Bmp,
}

impl ImgFormat {
    pub const ALL_ORDERED: [ImgFormat; 7] = [
        ImgFormat::Jpeg,
        ImgFormat::Png,
        ImgFormat::WebP,
        ImgFormat::Avif,
        ImgFormat::Gif,
        ImgFormat::Tiff,
        ImgFormat::Bmp,
    ];

    pub fn extension(self) -> &'static str {
        match self {
            ImgFormat::Jpeg => "jpg",
            ImgFormat::Png => "png",
            ImgFormat::WebP => "webp",
            ImgFormat::Avif => "avif",
            ImgFormat::Gif => "gif",
            ImgFormat::Tiff => "tiff",
            ImgFormat::Bmp => "bmp",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            ImgFormat::Jpeg => "JPG",
            ImgFormat::Png => "PNG",
            ImgFormat::WebP => "WebP",
            ImgFormat::Avif => "AVIF",
            ImgFormat::Gif => "GIF",
            ImgFormat::Tiff => "TIFF",
            ImgFormat::Bmp => "BMP",
        }
    }

    pub fn from_extension(ext: &str) -> Option<ImgFormat> {
        match ext.to_ascii_lowercase().as_str() {
            "jpg" | "jpeg" => Some(ImgFormat::Jpeg),
            "png" => Some(ImgFormat::Png),
            "webp" => Some(ImgFormat::WebP),
            "avif" => Some(ImgFormat::Avif),
            "gif" => Some(ImgFormat::Gif),
            "tif" | "tiff" => Some(ImgFormat::Tiff),
            "bmp" => Some(ImgFormat::Bmp),
            _ => None,
        }
    }

    /// Chỉ ba định dạng này có thể nén lại hoặc xoá metadata mà không phải đổi định dạng.
    pub fn supports_metadata_tools(self) -> bool {
        matches!(self, ImgFormat::Jpeg | ImgFormat::Png | ImgFormat::WebP)
    }
}

/// Định dạng văn bản thuần, phụ đề và bảng. Không có chữ ký nhị phân nên nhận dạng bằng đuôi
/// rồi kiểm nội dung UTF-8.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TextFormat {
    Txt,
    Markdown,
    Srt,
    Vtt,
    Csv,
    Tsv,
}

impl TextFormat {
    pub fn extension(self) -> &'static str {
        match self {
            TextFormat::Txt => "txt",
            TextFormat::Markdown => "md",
            TextFormat::Srt => "srt",
            TextFormat::Vtt => "vtt",
            TextFormat::Csv => "csv",
            TextFormat::Tsv => "tsv",
        }
    }

    pub fn from_extension(ext: &str) -> Option<TextFormat> {
        match ext.to_ascii_lowercase().as_str() {
            "txt" => Some(TextFormat::Txt),
            "md" | "markdown" => Some(TextFormat::Markdown),
            "srt" => Some(TextFormat::Srt),
            "vtt" => Some(TextFormat::Vtt),
            "csv" => Some(TextFormat::Csv),
            "tsv" => Some(TextFormat::Tsv),
            _ => None,
        }
    }
}

/// Định dạng âm thanh và video. `Gif` là đầu ra video; khi đọc, GIF vẫn nhận là `Image(Gif)`
/// vì nhánh ảnh được xét trước.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MediaFormat {
    Mp3,
    M4a,
    Wav,
    Flac,
    Ogg,
    Opus,
    Aiff,
    Wma,
    Mp4,
    Mov,
    Mkv,
    WebM,
    Avi,
    Wmv,
    Gif,
}

impl MediaFormat {
    /// Mọi định dạng âm thanh và video; `from_extension` tra theo đuôi trong danh sách này.
    const ALL: [MediaFormat; 15] = [
        MediaFormat::Mp3,
        MediaFormat::M4a,
        MediaFormat::Wav,
        MediaFormat::Flac,
        MediaFormat::Ogg,
        MediaFormat::Opus,
        MediaFormat::Aiff,
        MediaFormat::Wma,
        MediaFormat::Mp4,
        MediaFormat::Mov,
        MediaFormat::Mkv,
        MediaFormat::WebM,
        MediaFormat::Avi,
        MediaFormat::Wmv,
        MediaFormat::Gif,
    ];

    /// Định dạng theo đuôi, dùng khi giao diện gửi lại mã `media:<đuôi>` của múi đã chọn.
    pub fn from_extension(ext: &str) -> Option<MediaFormat> {
        Self::ALL
            .into_iter()
            .find(|format| format.extension() == ext)
    }

    pub fn extension(self) -> &'static str {
        match self {
            MediaFormat::Mp3 => "mp3",
            MediaFormat::M4a => "m4a",
            MediaFormat::Wav => "wav",
            MediaFormat::Flac => "flac",
            MediaFormat::Ogg => "ogg",
            MediaFormat::Opus => "opus",
            MediaFormat::Aiff => "aiff",
            MediaFormat::Wma => "wma",
            MediaFormat::Mp4 => "mp4",
            MediaFormat::Mov => "mov",
            MediaFormat::Mkv => "mkv",
            MediaFormat::WebM => "webm",
            MediaFormat::Avi => "avi",
            MediaFormat::Wmv => "wmv",
            MediaFormat::Gif => "gif",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            MediaFormat::Mp3 => "MP3",
            MediaFormat::M4a => "M4A",
            MediaFormat::Wav => "WAV",
            MediaFormat::Flac => "FLAC",
            MediaFormat::Ogg => "OGG",
            MediaFormat::Opus => "Opus",
            MediaFormat::Aiff => "AIFF",
            MediaFormat::Wma => "WMA",
            MediaFormat::Mp4 => "MP4",
            MediaFormat::Mov => "MOV",
            MediaFormat::Mkv => "MKV",
            MediaFormat::WebM => "WebM",
            MediaFormat::Avi => "AVI",
            MediaFormat::Wmv => "WMV",
            MediaFormat::Gif => "GIF",
        }
    }

    pub fn is_audio(self) -> bool {
        matches!(
            self,
            MediaFormat::Mp3
                | MediaFormat::M4a
                | MediaFormat::Wav
                | MediaFormat::Flac
                | MediaFormat::Ogg
                | MediaFormat::Opus
                | MediaFormat::Aiff
                | MediaFormat::Wma
        )
    }
}

/// Định dạng file nén Morphix nhận dạng được. Đây là danh sách đóng: định dạng khác không được
/// đoán là file nén.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ArchiveFormat {
    Zip,
    TarGz,
    SevenZ,
    Rar,
}

impl ArchiveFormat {
    /// Đuôi file (không có dấu chấm). `TarGz` là đuôi kép `tar.gz`. RAR chỉ để khớp đủ, không dùng để tạo file.
    pub fn extension(self) -> &'static str {
        match self {
            ArchiveFormat::Zip => "zip",
            ArchiveFormat::TarGz => "tar.gz",
            ArchiveFormat::SevenZ => "7z",
            ArchiveFormat::Rar => "rar",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FileKind {
    Image(ImgFormat),
    Pdf,
    Media(MediaFormat),
    Archive(ArchiveFormat),
    Text(TextFormat),
    Unknown,
}

/// Số byte đầu đủ để `infer` nhận dạng mọi định dạng cần dùng.
const SNIFF_BYTES: u64 = 8192;

/// Nhận dạng theo nội dung chứ không theo đuôi: file đổi đuôi hoặc không có đuôi vẫn đúng.
pub fn detect(path: &Path) -> Result<FileKind> {
    let io_error = |source| MorphixError::Io {
        path: path.to_path_buf(),
        source,
    };
    let mut head = Vec::new();
    File::open(path)
        .map_err(io_error)?
        .take(SNIFF_BYTES)
        .read_to_end(&mut head)
        .map_err(io_error)?;

    let kind = sniff_binary(&head, path);
    if kind != FileKind::Unknown {
        return Ok(kind);
    }
    // Đuôi chỉ là gợi ý; nội dung phải là UTF-8 thì mới coi là văn bản.
    let format = path
        .extension()
        .and_then(|e| e.to_str())
        .and_then(TextFormat::from_extension);
    Ok(match format {
        Some(format) if is_utf8_text(&head) => FileKind::Text(format),
        _ => FileKind::Unknown,
    })
}

fn sniff_binary(head: &[u8], path: &Path) -> FileKind {
    let Some(found) = infer::get(head) else {
        return FileKind::Unknown;
    };
    match found.mime_type() {
        "image/jpeg" => FileKind::Image(ImgFormat::Jpeg),
        "image/png" => FileKind::Image(ImgFormat::Png),
        "image/webp" => FileKind::Image(ImgFormat::WebP),
        "image/avif" => FileKind::Image(ImgFormat::Avif),
        "image/gif" => FileKind::Image(ImgFormat::Gif),
        "image/tiff" => FileKind::Image(ImgFormat::Tiff),
        "image/bmp" => FileKind::Image(ImgFormat::Bmp),
        "application/pdf" => FileKind::Pdf,
        // WHY: bảng MIME tường minh thay vì nhận theo matcher_type. infer còn trả MIME cho
        // định dạng ngoài danh mục (MPEG, FLV, AAC...), và đoán chúng là sai.
        "audio/mpeg" => FileKind::Media(MediaFormat::Mp3),
        "audio/m4a" => FileKind::Media(MediaFormat::M4a),
        "audio/x-wav" => FileKind::Media(MediaFormat::Wav),
        "audio/x-flac" => FileKind::Media(MediaFormat::Flac),
        "audio/ogg" => FileKind::Media(MediaFormat::Ogg),
        // infer tự phân biệt Ogg Opus (OpusHead ở byte 28) với Ogg Vorbis.
        "audio/opus" => FileKind::Media(MediaFormat::Opus),
        "audio/x-aiff" => FileKind::Media(MediaFormat::Aiff),
        "video/mp4" => FileKind::Media(MediaFormat::Mp4),
        "video/quicktime" => FileKind::Media(MediaFormat::Mov),
        "video/x-matroska" => FileKind::Media(MediaFormat::Mkv),
        "video/webm" => FileKind::Media(MediaFormat::WebM),
        "video/x-msvideo" => FileKind::Media(MediaFormat::Avi),
        "video/x-ms-wmv" => FileKind::Media(asf_format(head)),
        // WHY: bảng MIME tường minh, cùng lý do với nhánh âm thanh. Các MIME này do infer 0.22.0
        // gán (map.rs: zip dòng 449, rar 467, gzip 473, 7z 491).
        "application/zip" => FileKind::Archive(ArchiveFormat::Zip),
        "application/vnd.rar" => FileKind::Archive(ArchiveFormat::Rar),
        "application/x-7z-compressed" => FileKind::Archive(ArchiveFormat::SevenZ),
        // WHY: gzip chỉ là lớp nén, muốn biết bên trong có tar phải giải nén khối đầu. Để không
        // kéo thêm bước giải nén vào đây, tên .tar.gz hoặc .tgz là dấu hiệu quyết định. Gzip đặt
        // tên khác thì là Unknown, không đoán.
        "application/gzip" if is_tar_gz_name(path) => FileKind::Archive(ArchiveFormat::TarGz),
        _ => FileKind::Unknown,
    }
}

fn is_tar_gz_name(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(|name| name.to_ascii_lowercase())
        .is_some_and(|name| name.ends_with(".tar.gz") || name.ends_with(".tgz"))
}

// GUID ASF_Video_Media {BC19EFC0-5B4D-11CF-A8FD-00805F5C442B}, lưu little-endian như trong file.
const ASF_VIDEO_STREAM_GUID: [u8; 16] = [
    0xC0, 0xEF, 0x19, 0xBC, 0x4D, 0x5B, 0xCF, 0x11, 0xA8, 0xFD, 0x00, 0x80, 0x5F, 0x5C, 0x44, 0x2B,
];

/// WMA và WMV dùng chung chữ ký ASF mà infer nhận ra. WHY: chỉ WMV có luồng video, nên tìm GUID
/// luồng video trong 8 KB đầu. Header ASF lớn hơn 8 KB có thể làm WMV bị coi là WMA.
fn asf_format(head: &[u8]) -> MediaFormat {
    if head.windows(16).any(|w| w == ASF_VIDEO_STREAM_GUID) {
        MediaFormat::Wmv
    } else {
        MediaFormat::Wma
    }
}

/// Kiểm 8 KB đầu có phải UTF-8 hợp lệ, bỏ BOM nếu có.
fn is_utf8_text(head: &[u8]) -> bool {
    const UTF8_BOM: &[u8] = &[0xEF, 0xBB, 0xBF];
    let body = head.strip_prefix(UTF8_BOM).unwrap_or(head);
    match std::str::from_utf8(body) {
        Ok(_) => true,
        // WHY: đọc cắt ở 8 KB nên có thể cắt giữa một ký tự nhiều byte; lỗi "hết dữ liệu"
        // ở cuối chỉ hợp lệ khi đã cắt. File ngắn hơn 8 KB mà kết thúc dở dang thì không hợp lệ.
        Err(e) => e.error_len().is_none() && head.len() as u64 == SNIFF_BYTES,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::MorphixError;
    use std::path::{Path, PathBuf};

    fn write_tmp(dir: &Path, name: &str, bytes: &[u8]) -> PathBuf {
        let path = dir.join(name);
        std::fs::write(&path, bytes).unwrap();
        path
    }

    // `infer` nhận dạng JPEG chỉ bằng 3 byte đầu, nên một đầu đủ dùng cho test nhận dạng.
    const JPEG_HEAD: &[u8] = &[
        0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, b'J', b'F', b'I', b'F', 0x00,
    ];

    #[test]
    fn detects_by_content_not_extension() {
        let d = tempfile::tempdir().unwrap();
        // JPEG thật đặt tên .png, PDF thật đặt tên .txt, zip thật đặt tên .jpg
        assert_eq!(
            detect(&write_tmp(d.path(), "a.png", JPEG_HEAD)).unwrap(),
            FileKind::Image(ImgFormat::Jpeg)
        );
        assert_eq!(
            detect(&write_tmp(d.path(), "b.txt", b"%PDF-1.4\n%....")).unwrap(),
            FileKind::Pdf
        );
        assert_eq!(
            detect(&write_tmp(d.path(), "c.jpg", b"PK\x03\x04rest")).unwrap(),
            FileKind::Archive(ArchiveFormat::Zip)
        );
    }

    #[test]
    fn empty_and_unknown_files_are_unknown() {
        let d = tempfile::tempdir().unwrap();
        assert_eq!(
            detect(&write_tmp(d.path(), "d.jpg", b"")).unwrap(),
            FileKind::Unknown
        );
        assert_eq!(
            detect(&write_tmp(d.path(), "e.bin", b"hello world")).unwrap(),
            FileKind::Unknown
        );
    }

    #[test]
    fn missing_file_is_io_error() {
        assert!(matches!(
            detect(Path::new("/no/such/file.jpg")),
            Err(MorphixError::Io { .. })
        ));
    }

    #[test]
    fn detects_text_subtitle_and_table_files() {
        let d = tempfile::tempdir().unwrap();
        // SRT có BOM UTF-8 đầu file, CSV có tiếng Việt
        let mut srt = vec![0xEF, 0xBB, 0xBF];
        srt.extend_from_slice("1\n00:00:01,000 --> 00:00:02,000\nXin chào\n\n".as_bytes());
        assert_eq!(
            detect(&write_tmp(d.path(), "a.srt", &srt)).unwrap(),
            FileKind::Text(TextFormat::Srt)
        );
        assert_eq!(
            detect(&write_tmp(
                d.path(),
                "b.csv",
                "tên,giá\nphở,\"1,5\"\n".as_bytes()
            ))
            .unwrap(),
            FileKind::Text(TextFormat::Csv)
        );
    }

    #[test]
    fn non_utf8_file_with_text_extension_is_unknown() {
        let d = tempfile::tempdir().unwrap();
        assert_eq!(
            detect(&write_tmp(
                d.path(),
                "f.bin",
                &[0xFF, 0xFE, 0x00, 0x80, 0xC3]
            ))
            .unwrap(),
            FileKind::Unknown
        );
        assert_eq!(
            detect(&write_tmp(
                d.path(),
                "g.txt",
                &[0xFF, 0xFE, 0x00, 0x80, 0xC3]
            ))
            .unwrap(),
            FileKind::Unknown
        );
    }

    #[test]
    fn utf8_char_cut_by_sniff_window_is_still_text() {
        let d = tempfile::tempdir().unwrap();
        // 8191 byte ASCII rồi một ký tự 2 byte: đọc 8 KB sẽ cắt giữa ký tự đó
        let mut body = vec![b'a'; (SNIFF_BYTES - 1) as usize];
        body.extend_from_slice("é".as_bytes());
        assert_eq!(
            detect(&write_tmp(d.path(), "i.txt", &body)).unwrap(),
            FileKind::Text(TextFormat::Txt)
        );
    }

    #[test]
    fn signed_file_with_txt_extension_keeps_its_kind() {
        let d = tempfile::tempdir().unwrap();
        let png: &[u8] = &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
        assert_eq!(
            detect(&write_tmp(d.path(), "h.txt", png)).unwrap(),
            FileKind::Image(ImgFormat::Png)
        );
    }

    #[test]
    fn text_format_extension_mapping() {
        assert_eq!(TextFormat::from_extension("MD"), Some(TextFormat::Markdown));
        assert_eq!(
            TextFormat::from_extension("markdown"),
            Some(TextFormat::Markdown)
        );
        assert_eq!(TextFormat::from_extension("TXT"), Some(TextFormat::Txt));
        assert_eq!(TextFormat::from_extension("vtt"), Some(TextFormat::Vtt));
        assert_eq!(TextFormat::from_extension("tsv"), Some(TextFormat::Tsv));
        assert_eq!(TextFormat::from_extension("doc"), None);
        assert_eq!(TextFormat::Markdown.extension(), "md");
    }

    #[test]
    fn extension_mapping_is_case_insensitive() {
        assert_eq!(ImgFormat::from_extension("JPEG"), Some(ImgFormat::Jpeg));
        assert_eq!(ImgFormat::from_extension("tif"), Some(ImgFormat::Tiff));
        assert_eq!(ImgFormat::from_extension("doc"), None);
    }

    // Đệm đến 512 byte: matcher MKV và WebM đòi ít nhất 256 byte để nhận dạng.
    fn padded(prefix: &[u8]) -> Vec<u8> {
        let mut head = prefix.to_vec();
        head.resize(512, 0);
        head
    }

    #[test]
    fn detects_audio_formats_from_headers() {
        let d = tempfile::tempdir().unwrap();
        let mut opus = b"OggS".to_vec();
        opus.resize(28, 0);
        opus.extend_from_slice(b"OpusHead");
        let cases: Vec<(&str, Vec<u8>, MediaFormat)> = vec![
            (
                "a.mp3",
                b"ID3\x04\x00\x00\x00\x00\x00\x00".to_vec(),
                MediaFormat::Mp3,
            ),
            (
                "b.m4a",
                b"\x00\x00\x00\x20ftypM4A \x00\x00\x00\x00".to_vec(),
                MediaFormat::M4a,
            ),
            (
                "c.wav",
                b"RIFF\x00\x00\x00\x00WAVEfmt ".to_vec(),
                MediaFormat::Wav,
            ),
            (
                "d.flac",
                b"fLaC\x00\x00\x00\x22".to_vec(),
                MediaFormat::Flac,
            ),
            ("e.ogg", b"OggS\x00\x02".to_vec(), MediaFormat::Ogg),
            ("f.opus", opus, MediaFormat::Opus),
            (
                "g.aiff",
                b"FORM\x00\x00\x00\x00AIFF".to_vec(),
                MediaFormat::Aiff,
            ),
        ];
        for (name, head, want) in cases {
            assert_eq!(
                detect(&write_tmp(d.path(), name, &padded(&head))).unwrap(),
                FileKind::Media(want),
                "{name}"
            );
        }
    }

    #[test]
    fn detects_video_formats_from_headers() {
        let d = tempfile::tempdir().unwrap();
        let cases: Vec<(&str, Vec<u8>, MediaFormat)> = vec![
            (
                "a.mp4",
                b"\x00\x00\x00\x18ftypisom\x00\x00\x02\x00".to_vec(),
                MediaFormat::Mp4,
            ),
            (
                "b.mov",
                b"\x00\x00\x00\x14ftypqt  \x00\x00\x00\x00".to_vec(),
                MediaFormat::Mov,
            ),
            (
                "c.mkv",
                b"\x1A\x45\xDF\xA3\x9F\x42\x82\x88matroska".to_vec(),
                MediaFormat::Mkv,
            ),
            (
                "d.webm",
                b"\x1A\x45\xDF\xA3\x9F\x42\x82\x84webm".to_vec(),
                MediaFormat::WebM,
            ),
            (
                "e.avi",
                b"RIFF\x00\x00\x00\x00AVI LIST".to_vec(),
                MediaFormat::Avi,
            ),
            (
                "f.wmv",
                // Chữ ký ASF, rồi GUID luồng video (little-endian) trong Stream Properties.
                [
                    &b"\x30\x26\xB2\x75\x8E\x66\xCF\x11\xA6\xD9\x00\xAA"[..],
                    &ASF_VIDEO_STREAM_GUID[..],
                ]
                .concat(),
                MediaFormat::Wmv,
            ),
        ];
        for (name, head, want) in cases {
            assert_eq!(
                detect(&write_tmp(d.path(), name, &padded(&head))).unwrap(),
                FileKind::Media(want),
                "{name}"
            );
        }
    }

    #[test]
    fn asf_without_video_stream_is_wma() {
        let d = tempfile::tempdir().unwrap();
        // Cùng chữ ký ASF với WMV nhưng chỉ có luồng âm thanh (GUID ASF_Audio_Media, little-endian).
        let mut head = b"\x30\x26\xB2\x75\x8E\x66\xCF\x11\xA6\xD9\x00\xAA".to_vec();
        head.extend_from_slice(&[
            0x40, 0x9E, 0x69, 0xF8, 0x4D, 0x5B, 0xCF, 0x11, 0xA8, 0xFD, 0x00, 0x80, 0x5F, 0x5C,
            0x44, 0x2B,
        ]);
        assert_eq!(
            detect(&write_tmp(d.path(), "k.wma", &padded(&head))).unwrap(),
            FileKind::Media(MediaFormat::Wma)
        );
    }

    #[test]
    fn media_mime_outside_the_table_is_unknown() {
        let d = tempfile::tempdir().unwrap();
        // ADTS AAC: infer nhận ra (audio/aac) nhưng định dạng này không nằm trong bảng, nên không đoán.
        assert_eq!(
            detect(&write_tmp(
                d.path(),
                "h.aac",
                &padded(&[0xFF, 0xF1, 0x50, 0x80])
            ))
            .unwrap(),
            FileKind::Unknown
        );
    }

    // LCG cố định để test lặp lại được; byte "ngẫu nhiên" không mang chữ ký định dạng nào.
    fn noise(len: usize) -> Vec<u8> {
        let mut seed: u32 = 0x1234_5678;
        (0..len)
            .map(|_| {
                seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12_345);
                (seed >> 16) as u8
            })
            .collect()
    }

    #[test]
    fn random_bytes_are_unknown_even_with_media_extension() {
        let d = tempfile::tempdir().unwrap();
        assert_eq!(
            detect(&write_tmp(d.path(), "i.mp4", &noise(512))).unwrap(),
            FileKind::Unknown
        );
    }

    #[test]
    fn random_bytes_named_as_archive_are_unknown() {
        let d = tempfile::tempdir().unwrap();
        assert_eq!(
            detect(&write_tmp(d.path(), "j.zip", &noise(512))).unwrap(),
            FileKind::Unknown
        );
    }

    #[test]
    fn detects_archive_formats_from_headers() {
        let d = tempfile::tempdir().unwrap();
        let cases: Vec<(&str, Vec<u8>, ArchiveFormat)> = vec![
            ("a.zip", b"PK\x03\x04\x14\x00".to_vec(), ArchiveFormat::Zip),
            (
                "b.7z",
                b"7z\xBC\xAF\x27\x1C\x00\x04".to_vec(),
                ArchiveFormat::SevenZ,
            ),
            (
                "c.rar",
                b"Rar!\x1A\x07\x00\xCF\x90".to_vec(),
                ArchiveFormat::Rar,
            ),
        ];
        for (name, head, want) in cases {
            assert_eq!(
                detect(&write_tmp(d.path(), name, &padded(&head))).unwrap(),
                FileKind::Archive(want),
                "{name}"
            );
        }
    }

    #[test]
    fn gzip_is_tar_gz_only_when_named_as_tar_gz() {
        let d = tempfile::tempdir().unwrap();
        // WHY: kiểm nội dung tar trong gzip cần giải nén, nên không làm ở đây. Tên .tar.gz hoặc
        // .tgz là dấu hiệu duy nhất; gzip có tên khác thì không đoán là tar.gz.
        let gz: &[u8] = &[0x1F, 0x8B, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00];
        assert_eq!(
            detect(&write_tmp(d.path(), "backup.tar.gz", gz)).unwrap(),
            FileKind::Archive(ArchiveFormat::TarGz)
        );
        assert_eq!(
            detect(&write_tmp(d.path(), "BACKUP.TGZ", gz)).unwrap(),
            FileKind::Archive(ArchiveFormat::TarGz)
        );
        assert_eq!(
            detect(&write_tmp(d.path(), "notes.gz", gz)).unwrap(),
            FileKind::Unknown
        );
    }

    #[test]
    fn gif_stays_an_image() {
        let d = tempfile::tempdir().unwrap();
        assert_eq!(
            detect(&write_tmp(d.path(), "j.gif", b"GIF89a\x01\x00\x01\x00")).unwrap(),
            FileKind::Image(ImgFormat::Gif)
        );
    }

    #[test]
    fn media_format_extension_label_and_audio_flag() {
        assert_eq!(MediaFormat::M4a.extension(), "m4a");
        assert_eq!(MediaFormat::WebM.label(), "WebM");
        assert_eq!(MediaFormat::Mp4.label(), "MP4");
        assert!(MediaFormat::Opus.is_audio());
        assert!(MediaFormat::Wma.is_audio());
        assert!(!MediaFormat::Mkv.is_audio());
        assert!(!MediaFormat::Gif.is_audio());
    }
}
