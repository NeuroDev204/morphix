use crate::kind::{ArchiveFormat, FileKind, ImgFormat, MediaFormat, TextFormat};
use crate::ops::media::encoder_for;

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

/// Định dạng đích cho một định dạng văn bản.
/// WHY: nhóm phụ đề chỉ xuất TXT (bỏ mốc thời gian), TXT không có mốc nên không sang SRT/VTT.
pub fn text_targets(from: TextFormat) -> &'static [TextFormat] {
    match from {
        TextFormat::Txt => &[TextFormat::Txt, TextFormat::Markdown],
        TextFormat::Markdown => &[
            TextFormat::Txt,
            TextFormat::Markdown,
            TextFormat::Html,
            TextFormat::Docx,
        ],
        TextFormat::Srt | TextFormat::Vtt => &[
            TextFormat::Srt,
            TextFormat::Vtt,
            TextFormat::Txt,
            TextFormat::Markdown,
        ],
        TextFormat::Csv | TextFormat::Tsv => {
            &[TextFormat::Csv, TextFormat::Tsv, TextFormat::Markdown]
        }
        // WHY: tài liệu có định dạng đổi qua lại qua Markdown; TXT ở cuối vì mất hết định dạng.
        TextFormat::Html => &[TextFormat::Markdown, TextFormat::Docx, TextFormat::Txt],
        TextFormat::Docx => &[TextFormat::Markdown, TextFormat::Html, TextFormat::Txt],
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
    /// Lớp chữ của một PDF ra MD, mỗi trang một mục. Chỉ có khi chọn đúng một PDF.
    PdfToMarkdown,
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
    // WHY: chỉ một PDF mới được đề nghị TXT/MD, để mỗi PDF cho đúng một file tên theo chính nó. Nhiều
    // PDF không có tên hợp lý cho một file gộp, nên không hiện việc. Không thêm PDF sang EPUB hay Office.
    if let [FileKind::Pdf] = kinds {
        return vec![
            action(ActionKind::PdfToText, "TXT", false),
            action(ActionKind::PdfToMarkdown, "MD", false),
        ];
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
    // WHY: nén mã hoá lại theo định dạng gốc; WMA/WMV không có bộ mã hoá nên việc nén luôn hỏng.
    if formats
        .iter()
        .all(|f: &MediaFormat| encoder_for(*f).is_some())
    {
        actions.push(action(ActionKind::CompressMedia, "Nén", true));
    }
    actions
}

/// Việc chuyển văn bản chỉ có khi mọi file cùng một nhóm; trộn nhóm thì không có việc nào.
/// Nhóm văn bản; chọn lẫn file khác nhóm thì không có việc chuyển nào.
fn text_group(format: TextFormat) -> u8 {
    match format {
        TextFormat::Txt | TextFormat::Markdown => 0,
        TextFormat::Srt | TextFormat::Vtt => 1,
        TextFormat::Csv | TextFormat::Tsv => 2,
        TextFormat::Html | TextFormat::Docx => 3,
    }
}

fn text_actions(formats: &[TextFormat]) -> Vec<Action> {
    if !formats
        .iter()
        .all(|f| text_group(*f) == text_group(formats[0]))
    {
        return Vec::new();
    }
    text_targets(formats[0])
        .iter()
        // WHY: chỉ đề nghị đích mà mọi file đã chọn đều chuyển được (MD sang DOCX được, TXT thì không).
        .filter(|target| formats.iter().all(|f| text_targets(*f).contains(target)))
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

/// Danh sách id công cụ đã được nối đầy đủ từ lõi xử lý đến giao diện. Công cụ chưa có giao diện
/// bị lọc khỏi menu để người dùng chỉ thấy tính năng đã sẵn sàng.
pub const WIRED_TOOL_IDS: [&str; 4] = ["compress", "strip", "resize", "compress-media"];

/// Id chuỗi ổn định của một việc. Vòng tròn, menu Explorer và IPC dùng chung để hai phía luôn sinh cùng id.
pub fn action_id(kind: ActionKind) -> String {
    match kind {
        ActionKind::ConvertTo(format) => format!("convert:{}", format.extension()),
        ActionKind::MergeToPdf => "merge".to_string(),
        ActionKind::Compress => "compress".to_string(),
        ActionKind::StripMetadata => "strip".to_string(),
        ActionKind::Resize => "resize".to_string(),
        ActionKind::ConvertText(format) => format!("text:{}", format.extension()),
        ActionKind::ConvertMedia(format) => format!("media:{}", format.extension()),
        ActionKind::CompressMedia => "compress-media".to_string(),
        ActionKind::CutMedia => "cut".to_string(),
        ActionKind::ChangeSpeed => "speed".to_string(),
        ActionKind::ExtractAudio(format) => format!("extract:{}", format.extension()),
        ActionKind::PdfToText => "pdf-to-text".to_string(),
        ActionKind::PdfToMarkdown => "pdf-to-markdown".to_string(),
        ActionKind::CreateArchive(format) => format!("archive:{}", format.extension()),
        ActionKind::ExtractArchive => "extract".to_string(),
    }
}

/// Các việc hiện trên menu: `actions_for` bỏ những việc chỉ có ở CLI và công cụ chưa nối giao diện.
pub fn menu_actions(kinds: &[FileKind]) -> Vec<Action> {
    actions_for(kinds)
        .into_iter()
        .filter(|action| {
            // WHY: cắt, đổi tốc độ và tách âm thanh chỉ có ở CLI trong bản này (spec mục 2a), nên không hiện.
            if matches!(
                action.kind,
                ActionKind::CutMedia | ActionKind::ChangeSpeed | ActionKind::ExtractAudio(_)
            ) {
                return false;
            }
            !action.is_tool || WIRED_TOOL_IDS.contains(&action_id(action.kind).as_str())
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
    fn action_id_matches_wire_ids() {
        assert_eq!(
            action_id(ActionKind::ConvertTo(ImgFormat::WebP)),
            "convert:webp"
        );
        assert_eq!(
            action_id(ActionKind::CreateArchive(ArchiveFormat::Zip)),
            "archive:zip"
        );
        assert_eq!(action_id(ActionKind::Compress), "compress");
    }

    #[test]
    fn menu_actions_hides_cli_only_and_unwired_tools() {
        let actions = menu_actions(&[FileKind::Media(MediaFormat::Mp4)]);
        assert!(!actions.is_empty());
        assert!(actions.iter().all(|a| !matches!(
            a.kind,
            ActionKind::CutMedia | ActionKind::ChangeSpeed | ActionKind::ExtractAudio(_)
        )));
        assert!(actions
            .iter()
            .filter(|a| a.is_tool)
            .all(|a| WIRED_TOOL_IDS.contains(&action_id(a.kind).as_str())));
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
    fn compress_is_offered_only_for_media_with_an_encoder() {
        assert!(labels(&actions_for(&[FileKind::Media(MediaFormat::Mp4)]), true).contains(&"Nén"));
        for format in [MediaFormat::Wmv, MediaFormat::Wma] {
            assert!(
                !labels(&actions_for(&[FileKind::Media(format)]), true).contains(&"Nén"),
                "{format:?}"
            );
        }
    }

    #[test]
    fn text_pair_supported_only_inside_its_group() {
        assert!(text_pair_supported(TextFormat::Srt, TextFormat::Vtt));
        assert!(text_pair_supported(TextFormat::Srt, TextFormat::Txt));
        assert!(text_pair_supported(TextFormat::Csv, TextFormat::Tsv));
        assert!(!text_pair_supported(TextFormat::Srt, TextFormat::Csv));
        assert!(!text_pair_supported(TextFormat::Txt, TextFormat::Srt));
        assert!(text_pair_supported(TextFormat::Srt, TextFormat::Markdown));
        assert!(text_pair_supported(TextFormat::Csv, TextFormat::Markdown));
        assert!(!text_pair_supported(TextFormat::Markdown, TextFormat::Csv));
    }

    /// Việc theo loại, bỏ việc nén (có test riêng trong `tests/archive.rs`).
    fn type_only(actions: Vec<Action>) -> Vec<Action> {
        actions
            .into_iter()
            .filter(|a| !matches!(a.kind, ActionKind::CreateArchive(_)))
            .collect()
    }

    #[test]
    fn single_pdf_offers_txt_and_md_and_several_pdfs_offer_nothing() {
        let single = type_only(actions_for(&[FileKind::Pdf]));
        let kinds: Vec<(ActionKind, &str)> =
            single.iter().map(|a| (a.kind, a.label.as_str())).collect();
        assert_eq!(
            kinds,
            [
                (ActionKind::PdfToText, "TXT"),
                (ActionKind::PdfToMarkdown, "MD")
            ]
        );
        assert_eq!(action_id(ActionKind::PdfToMarkdown), "pdf-to-markdown");
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
