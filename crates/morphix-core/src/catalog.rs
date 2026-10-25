use crate::kind::{ArchiveFormat, FileKind, ImgFormat, MediaFormat, TextFormat};

/// Các đích nén, theo thứ tự hiện trên vòng tròn. Nhãn nằm ở đây để giữ một nguồn chữ hiển thị.
const ARCHIVE_TARGETS: &[(ArchiveFormat, &str)] = &[
    (ArchiveFormat::Zip, "Nén thành ZIP"),
    (ArchiveFormat::TarGz, "Nén thành TAR.GZ"),
    (ArchiveFormat::SevenZ, "Nén thành 7Z"),
];

/// Đích khi đầu vào là video. WHY: GIF là đích của video để làm ảnh động ngắn; MP3 để lấy âm thanh.
const VIDEO_TARGETS: &[MediaFormat] = &[
    MediaFormat::Mp4,
    MediaFormat::Mkv,
    MediaFormat::WebM,
    MediaFormat::Mov,
    MediaFormat::Avi,
    MediaFormat::Gif,
    MediaFormat::Mp3,
];

/// Đích khi đầu vào là âm thanh.
const AUDIO_TARGETS: &[MediaFormat] = &[
    MediaFormat::Mp3,
    MediaFormat::M4a,
    MediaFormat::Wav,
    MediaFormat::Flac,
    MediaFormat::Ogg,
    MediaFormat::Opus,
];

/// Định dạng đích cho một định dạng văn bản. Mọi định dạng cùng nhóm có cùng danh sách, nên so
/// danh sách là đủ để biết hai file có cùng nhóm hay không.
/// WHY: nhóm phụ đề chỉ xuất TXT (bỏ mốc thời gian), TXT không có mốc nên không sang SRT/VTT.
pub fn text_targets(from: TextFormat) -> &'static [TextFormat] {
    match from {
        TextFormat::Txt | TextFormat::Markdown => &[TextFormat::Txt, TextFormat::Markdown],
        TextFormat::Srt | TextFormat::Vtt => &[TextFormat::Srt, TextFormat::Vtt, TextFormat::Txt],
        TextFormat::Csv | TextFormat::Tsv => &[TextFormat::Csv, TextFormat::Tsv],
    }
}

/// Cặp văn bản có được chuyển không. `convert_text` và CLI cùng gọi hàm này để không lệch nhau.
pub fn text_pair_supported(from: TextFormat, to: TextFormat) -> bool {
    text_targets(from).contains(&to)
}

/// Việc có thể làm với một tập file, chưa kèm tham số (kích thước mới, chất lượng...). Tham số do
/// giao diện hoặc CLI hỏi rồi đưa vào `Operation` khi chạy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionKind {
    ConvertTo(ImgFormat),
    MergeToPdf,
    Compress,
    StripMetadata,
    Resize,
    ConvertText(TextFormat),
    ConvertMedia(MediaFormat),
    CompressMedia,
    /// Các việc dưới đây đã có `Operation` nhưng `actions_for` không liệt kê: theo spec, cắt, đổi tốc độ
    /// và tách âm thanh chỉ có ở CLI (đợt 2).
    CutMedia,
    ChangeSpeed,
    ExtractAudio(MediaFormat),
    /// Lớp chữ của một PDF ra TXT (không OCR). Chỉ có khi chọn đúng một PDF.
    PdfToText,
    /// Gộp các file đã chọn thành một file nén định dạng này.
    CreateArchive(ArchiveFormat),
    /// Giải nén file nén đã chọn vào một thư mục cạnh file (chỉ ZIP, TAR.GZ, 7Z).
    ExtractArchive,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Action {
    pub kind: ActionKind,
    pub label: String,
    /// `true` cho nhóm Công cụ (giữ Alt), `false` cho nhóm Chuyển đổi.
    pub is_tool: bool,
}

fn action(kind: ActionKind, label: impl Into<String>, is_tool: bool) -> Action {
    Action {
        kind,
        label: label.into(),
        is_tool,
    }
}

/// Nguồn duy nhất cho các múi của vòng tròn, lệnh `formats` và Khay. Việc đầu tiên là việc gợi ý.
/// Chỉ trả những việc hợp lệ cho **mọi** file được chọn; tập rỗng hoặc lẫn loại thì không có việc nào.
/// Đợt 1 chỉ có ảnh.
pub fn actions_for(kinds: &[FileKind]) -> Vec<Action> {
    let mut actions: Vec<Action> = type_actions(kinds);
    // WHY: chỉ cần một file không phải file nén là có gì đó để gộp; chọn toàn file nén thì không đề nghị nén lại.
    if kinds
        .iter()
        .any(|kind| !matches!(kind, FileKind::Archive(_)))
    {
        actions.extend(
            ARCHIVE_TARGETS
                .iter()
                .map(|(format, label)| action(ActionKind::CreateArchive(*format), *label, false)),
        );
    }
    actions
}

/// Việc theo loại file. Nén không nằm ở đây vì áp dụng cho mọi loại, kể cả file lẫn loại.
fn type_actions(kinds: &[FileKind]) -> Vec<Action> {
    // WHY: RAR chỉ đọc sau đợt 4 khi giấy phép đã kiểm, nên không đề nghị giải nén RAR.
    if !kinds.is_empty()
        && kinds
            .iter()
            .all(|kind| matches!(kind, FileKind::Archive(format) if *format != ArchiveFormat::Rar))
    {
        return vec![action(ActionKind::ExtractArchive, "Giải nén", false)];
    }
    if let Some(texts) = text_formats(kinds) {
        return text_actions(&texts);
    }
    if let Some(media) = media_formats(kinds) {
        return media_actions(&media);
    }
    // WHY: chỉ một PDF mới được đề nghị TXT, để mỗi PDF cho đúng một TXT tên theo chính nó. Nhiều PDF
    // không có tên hợp lý cho một file gộp, nên không hiện việc. Không thêm PDF sang MD, EPUB hay Office.
    if let [FileKind::Pdf] = kinds {
        return vec![action(ActionKind::PdfToText, "TXT", false)];
    }
    let formats: Option<Vec<ImgFormat>> = kinds
        .iter()
        .map(|kind| match kind {
            FileKind::Image(format) => Some(*format),
            _ => None,
        })
        .collect();
    let Some(formats) = formats.filter(|f| !f.is_empty()) else {
        return Vec::new();
    };

    let pdf_label = if formats.len() > 1 {
        "PDF gộp"
    } else {
        "PDF"
    };
    let mut actions = vec![action(ActionKind::MergeToPdf, pdf_label, false)];
    for target in ImgFormat::ALL_ORDERED {
        // Chọn toàn ảnh JPG thì "đổi sang JPG" là việc vô nghĩa.
        if !formats.iter().all(|f| *f == target) {
            actions.push(action(ActionKind::ConvertTo(target), target.label(), false));
        }
    }
    if formats.iter().all(|f| f.supports_metadata_tools()) {
        actions.push(action(ActionKind::Compress, "Nén ảnh", true));
        actions.push(action(ActionKind::StripMetadata, "Xoá metadata", true));
    }
    actions.push(action(ActionKind::Resize, "Đổi kích thước", true));
    actions
}

/// Các định dạng văn bản của tập file, nếu **mọi** file đều là văn bản.
fn text_formats(kinds: &[FileKind]) -> Option<Vec<TextFormat>> {
    kinds
        .iter()
        .map(|kind| match kind {
            FileKind::Text(format) => Some(*format),
            _ => None,
        })
        .collect::<Option<Vec<TextFormat>>>()
        .filter(|f| !f.is_empty())
}

/// Các định dạng âm thanh hoặc video của tập file, nếu **mọi** file đều là media.
fn media_formats(kinds: &[FileKind]) -> Option<Vec<MediaFormat>> {
    kinds
        .iter()
        .map(|kind: &FileKind| match kind {
            FileKind::Media(format) => Some(*format),
            _ => None,
        })
        .collect::<Option<Vec<MediaFormat>>>()
        .filter(|f: &Vec<MediaFormat>| !f.is_empty())
}

/// Việc của âm thanh và video. Đích theo nhóm của đầu vào; trộn video với âm thanh thì không có việc nào.
/// Đích trùng định dạng gốc bị ẩn, như ảnh và văn bản, vì khi đó việc đúng là nén.
fn media_actions(formats: &[MediaFormat]) -> Vec<Action> {
    let all_video: bool = formats.iter().all(|f: &MediaFormat| !f.is_audio());
    let all_audio: bool = formats.iter().all(|f: &MediaFormat| f.is_audio());
    let targets: &[MediaFormat] = if all_video {
        VIDEO_TARGETS
    } else if all_audio {
        AUDIO_TARGETS
    } else {
        return Vec::new();
    };
    let mut actions: Vec<Action> = targets
        .iter()
        .filter(|target: &&MediaFormat| !formats.iter().all(|f: &MediaFormat| f == *target))
        .map(|target: &MediaFormat| {
            action(ActionKind::ConvertMedia(*target), target.label(), false)
        })
        .collect();
    actions.push(action(ActionKind::CompressMedia, "Nén", true));
    actions
}

/// Việc chuyển văn bản chỉ có khi mọi file cùng một nhóm; trộn nhóm thì không có việc nào.
fn text_actions(formats: &[TextFormat]) -> Vec<Action> {
    let targets = text_targets(formats[0]);
    if !formats.iter().all(|f| text_targets(*f) == targets) {
        return Vec::new();
    }
    targets
        .iter()
        // Chọn toàn file TXT thì "đổi sang TXT" là việc vô nghĩa, giống quy tắc của ảnh.
        .filter(|target| !formats.iter().all(|f| f == *target))
        .map(|target| {
            action(
                ActionKind::ConvertText(*target),
                target.extension().to_uppercase(),
                false,
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kind::{ArchiveFormat, MediaFormat};

    fn labels(actions: &[Action], tool: bool) -> Vec<&str> {
        actions
            .iter()
            .filter(|a| a.is_tool == tool && !matches!(a.kind, ActionKind::CreateArchive(_)))
            .map(|a| a.label.as_str())
            .collect()
    }

    #[test]
    fn single_jpeg_gets_pdf_first_then_other_formats_and_three_tools() {
        let a = actions_for(&[FileKind::Image(ImgFormat::Jpeg)]);
        assert_eq!(
            labels(&a, false),
            ["PDF", "PNG", "WebP", "AVIF", "GIF", "TIFF", "BMP"]
        );
        assert_eq!(
            labels(&a, true),
            ["Nén ảnh", "Xoá metadata", "Đổi kích thước"]
        );
        assert_eq!(a[0].kind, ActionKind::MergeToPdf);
    }

    #[test]
    fn several_images_get_merge_label() {
        let a = actions_for(&[
            FileKind::Image(ImgFormat::Png),
            FileKind::Image(ImgFormat::Jpeg),
        ]);
        assert_eq!(a[0].label, "PDF gộp");
        assert_eq!(labels(&a, false).len(), 8); // trộn định dạng thì không bỏ định dạng nào
    }

    #[test]
    fn all_already_in_target_format_hides_that_format() {
        let a = actions_for(&[
            FileKind::Image(ImgFormat::Jpeg),
            FileKind::Image(ImgFormat::Jpeg),
        ]);
        assert!(!labels(&a, false).contains(&"JPG"));
    }

    #[test]
    fn gif_only_has_resize_tool() {
        assert_eq!(
            labels(&actions_for(&[FileKind::Image(ImgFormat::Gif)]), true),
            ["Đổi kích thước"]
        );
    }

    #[test]
    fn text_pair_supported_only_inside_its_group() {
        assert!(text_pair_supported(TextFormat::Srt, TextFormat::Vtt));
        assert!(text_pair_supported(TextFormat::Srt, TextFormat::Txt));
        assert!(text_pair_supported(TextFormat::Csv, TextFormat::Tsv));
        assert!(!text_pair_supported(TextFormat::Srt, TextFormat::Csv));
        assert!(!text_pair_supported(TextFormat::Txt, TextFormat::Srt));
    }

    /// Việc theo loại, bỏ việc nén (có test riêng trong `tests/archive.rs`).
    fn type_only(actions: Vec<Action>) -> Vec<Action> {
        actions
            .into_iter()
            .filter(|a| !matches!(a.kind, ActionKind::CreateArchive(_)))
            .collect()
    }

    #[test]
    fn single_pdf_offers_only_txt_and_several_pdfs_offer_nothing() {
        let single = type_only(actions_for(&[FileKind::Pdf]));
        assert_eq!(single.len(), 1);
        assert_eq!(single[0].kind, ActionKind::PdfToText);
        assert_eq!(single[0].label, "TXT");
        assert!(type_only(actions_for(&[FileKind::Pdf, FileKind::Pdf])).is_empty());
    }

    #[test]
    fn mixed_or_empty_or_unsupported_selection_yields_no_type_action() {
        assert!(actions_for(&[]).is_empty());
        assert!(type_only(actions_for(&[
            FileKind::Image(ImgFormat::Png),
            FileKind::Pdf
        ]))
        .is_empty());
        assert_eq!(
            actions_for(&[FileKind::Archive(ArchiveFormat::Zip)])
                .iter()
                .map(|a| a.kind)
                .collect::<Vec<_>>(),
            [ActionKind::ExtractArchive]
        );
        // WHY: RAR chỉ đọc sau đợt 4, nên không đề nghị giải nén.
        assert!(actions_for(&[FileKind::Archive(ArchiveFormat::Rar)]).is_empty());
        assert!(type_only(actions_for(&[
            FileKind::Media(MediaFormat::Mp4),
            FileKind::Media(MediaFormat::Mp3)
        ]))
        .is_empty());
    }
}
