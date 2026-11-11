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

/// Một nhóm loại file cho trình quản lý file chỉ lọc được theo loại (Dolphin, Thunar, thư mục Scripts),
/// không đọc được nội dung file như Explorer hay Nautilus có `nautilus-python`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MenuGroup {
    /// "image" | "audio" | "video" | "text" | "pdf" | "archive" | "any"; dùng làm tên file và id sinh ra.
    pub key: &'static str,
    /// Nhãn hiển thị tiếng Việt của nhóm.
    pub label: &'static str,
    /// Kiểu MIME của nhóm; nhóm `any` là `all/allfiles`.
    pub mime_types: Vec<&'static str>,
    /// Đuôi file không có dấu chấm (kể cả bí danh như `jpeg`); nhóm `any` để rỗng.
    pub extensions: Vec<&'static str>,
    /// Hợp các việc của mọi định dạng trong nhóm, bỏ trùng theo `action_id`; việc thường đứng trước công cụ.
    pub actions: Vec<Action>,
}

/// Hợp các việc trên menu của từng loại file, bỏ các việc nén thành file nén (đã có ở nhóm `any`).
/// `order` là thứ tự chuẩn của các việc chuyển đổi trong nhóm (cùng mảng đích mà `actions_for` dùng).
fn union_actions(kinds: &[FileKind], order: &[ActionKind]) -> Vec<Action> {
    let mut actions: Vec<Action> = Vec::new();
    for kind in kinds {
        for action in menu_actions(std::slice::from_ref(kind)) {
            let id = action_id(action.kind);
            if id.starts_with("archive:") || actions.iter().any(|a| action_id(a.kind) == id) {
                continue;
            }
            actions.push(action);
        }
    }
    // WHY sắp theo `order`: hợp theo thứ tự "thấy đầu tiên" đẩy đích phổ biến như MP4 xuống cuối, vì MP4 chỉ
    // hiện khi xét đầu vào khác MP4. Nhóm Công cụ luôn đứng sau nhóm Chuyển đổi, như vòng tròn và menu Explorer.
    actions.sort_by_key(|action| {
        let rank = order
            .iter()
            .position(|kind| *kind == action.kind)
            .unwrap_or(usize::MAX);
        (action.is_tool, rank)
    });
    actions
}

/// Các nhóm loại file cho menu tĩnh, sinh từ catalog để không có bảng việc thứ hai.
pub fn static_menu_groups() -> Vec<MenuGroup> {
    let images: Vec<FileKind> = ImgFormat::ALL_ORDERED.into_iter().map(FileKind::Image).collect();
    let image_order: Vec<ActionKind> = std::iter::once(ActionKind::MergeToPdf)
        .chain(ImgFormat::ALL_ORDERED.into_iter().map(ActionKind::ConvertTo))
        .collect();
    let convert_media =
        |targets: &[MediaFormat]| -> Vec<ActionKind> { targets.iter().copied().map(ActionKind::ConvertMedia).collect() };
    let text_order: Vec<ActionKind> = TextFormat::ALL.into_iter().map(ActionKind::ConvertText).collect();
    // WHY bỏ GIF: `detect` luôn nhận GIF là ảnh, nên `Media(Gif)` chỉ là đích, không bao giờ là đầu vào.
    let media = |audio: bool| -> Vec<FileKind> {
        MediaFormat::ALL
            .into_iter()
            .filter(|f| *f != MediaFormat::Gif && f.is_audio() == audio)
            .map(FileKind::Media)
            .collect()
    };
    let texts: Vec<FileKind> = TextFormat::ALL.into_iter().map(FileKind::Text).collect();
    // WHY bỏ RAR: Morphix chưa giải nén RAR (xem `type_actions`).
    let archives: Vec<FileKind> = [ArchiveFormat::Zip, ArchiveFormat::TarGz, ArchiveFormat::SevenZ]
        .into_iter()
        .map(FileKind::Archive)
        .collect();
    let to_archive_actions = menu_actions(&[FileKind::Unknown])
        .into_iter()
        .filter(|a| action_id(a.kind).starts_with("archive:"))
        .collect();
    vec![
        MenuGroup {
            key: "image",
            label: "Ảnh",
            mime_types: vec![
                "image/jpeg", "image/png", "image/webp", "image/avif", "image/gif", "image/tiff",
                "image/bmp",
            ],
            extensions: vec!["jpg", "jpeg", "png", "webp", "avif", "gif", "tiff", "tif", "bmp"],
            actions: union_actions(&images, &image_order),
        },
        MenuGroup {
            key: "audio",
            label: "Âm thanh",
            mime_types: vec![
                "audio/mpeg", "audio/mp4", "audio/x-wav", "audio/flac", "audio/ogg",
                "audio/x-opus+ogg", "audio/x-aiff", "audio/x-ms-wma",
            ],
            extensions: vec!["mp3", "m4a", "wav", "flac", "ogg", "opus", "aiff", "wma"],
            actions: union_actions(&media(true), &convert_media(AUDIO_TARGETS)),
        },
        MenuGroup {
            key: "video",
            label: "Video",
            mime_types: vec![
                "video/mp4", "video/quicktime", "video/x-matroska", "video/webm",
                "video/x-msvideo", "video/x-ms-wmv",
            ],
            extensions: vec!["mp4", "mov", "mkv", "webm", "avi", "wmv"],
            actions: union_actions(&media(false), &convert_media(VIDEO_TARGETS)),
        },
        MenuGroup {
            key: "text",
            label: "Văn bản",
            mime_types: vec![
                "text/plain", "text/markdown", "text/csv", "text/tab-separated-values",
                "text/html", "application/x-subrip", "text/vtt",
                "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
            ],
            extensions: vec![
                "txt", "md", "markdown", "srt", "vtt", "csv", "tsv", "html", "htm", "docx",
            ],
            actions: union_actions(&texts, &text_order),
        },
        MenuGroup {
            key: "pdf",
            label: "PDF",
            mime_types: vec!["application/pdf"],
            extensions: vec!["pdf"],
            actions: union_actions(&[FileKind::Pdf], &[]),
        },
        MenuGroup {
            key: "archive",
            label: "File nén",
            mime_types: vec![
                "application/zip", "application/gzip", "application/x-compressed-tar",
                "application/x-7z-compressed",
            ],
            extensions: vec!["zip", "tar.gz", "tgz", "7z"],
            actions: union_actions(&archives, &[]),
        },
        MenuGroup {
            key: "any",
            label: "Nén file",
            mime_types: vec!["all/allfiles"],
            extensions: Vec::new(),
            actions: to_archive_actions,
        },
    ]
}

/// Nhãn dùng trong menu chuột phải: có động từ, vì một mục chỉ ghi "JPG" hay "PDF" trong menu không nói được nó làm gì.
/// Vòng tròn cũ và Khay giữ nhãn ngắn (`Action::label`) vì ngữ cảnh của chúng đã rõ.
pub fn menu_label(action: &Action) -> String {
    match action.kind {
        ActionKind::MergeToPdf if action.label == "PDF gộp" => "Gộp thành PDF".to_string(),
        ActionKind::MergeToPdf
        | ActionKind::ConvertTo(_)
        | ActionKind::ConvertText(_)
        | ActionKind::ConvertMedia(_)
        | ActionKind::PdfToText
        | ActionKind::PdfToMarkdown => format!("Đổi sang {}", action.label),
        // Các việc còn lại đã có động từ ("Nén ảnh", "Giải nén", "Nén thành ZIP"...).
        _ => action.label.clone(),
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

    #[test]
    fn static_groups_cover_every_menu_action_exactly_like_the_catalog() {
        let groups = static_menu_groups();
        let keys: Vec<_> = groups.iter().map(|g| g.key).collect();
        assert_eq!(
            keys,
            ["image", "audio", "video", "text", "pdf", "archive", "any"]
        );
        let any = groups.iter().find(|g| g.key == "any").unwrap();
        assert_eq!(any.mime_types, ["all/allfiles"]);
        assert!(any.extensions.is_empty());
        assert!(!any.actions.is_empty());
        assert!(any
            .actions
            .iter()
            .all(|a| action_id(a.kind).starts_with("archive:")));
        for group in groups.iter().filter(|g| g.key != "any") {
            assert!(!group.actions.is_empty(), "{}", group.key);
            assert!(!group.mime_types.is_empty(), "{}", group.key);
            assert!(!group.extensions.is_empty(), "{}", group.key);
            assert!(group
                .actions
                .iter()
                .all(|a| !action_id(a.kind).starts_with("archive:")));
        }
        let image = &groups[0];
        for format in ImgFormat::ALL_ORDERED {
            for action in menu_actions(&[FileKind::Image(format)]) {
                let id = action_id(action.kind);
                assert!(
                    id.starts_with("archive:")
                        || image.actions.iter().any(|a| action_id(a.kind) == id),
                    "{id}"
                );
            }
        }
    }

    #[test]
    fn menu_labels_name_the_verb_so_a_bare_format_is_not_ambiguous() {
        let png = menu_actions(&[FileKind::Image(ImgFormat::Png)]);
        let label = |actions: &[Action], id: &str| {
            menu_label(actions.iter().find(|a| action_id(a.kind) == id).unwrap())
        };
        assert_eq!(label(&png, "merge"), "Đổi sang PDF");
        assert_eq!(label(&png, "convert:webp"), "Đổi sang WebP");
        assert_eq!(label(&png, "compress"), "Nén ảnh");
        assert_eq!(label(&png, "resize"), "Đổi kích thước");
        assert_eq!(label(&png, "archive:zip"), "Nén thành ZIP");
        let two = menu_actions(&[FileKind::Image(ImgFormat::Png), FileKind::Image(ImgFormat::Png)]);
        assert_eq!(label(&two, "merge"), "Gộp thành PDF");
        let pdf = menu_actions(&[FileKind::Pdf]);
        assert_eq!(label(&pdf, "pdf-to-text"), "Đổi sang TXT");
        assert_eq!(label(&pdf, "pdf-to-markdown"), "Đổi sang MD");
        let video = menu_actions(&[FileKind::Media(MediaFormat::Mov)]);
        assert_eq!(label(&video, "media:mp4"), "Đổi sang MP4");
        let zip = menu_actions(&[FileKind::Archive(ArchiveFormat::Zip)]);
        assert_eq!(label(&zip, "extract"), "Giải nén");
    }

    #[test]
    fn static_groups_follow_the_catalog_target_order() {
        let groups = static_menu_groups();
        let ids = |key: &str| -> Vec<String> {
            groups
                .iter()
                .find(|g| g.key == key)
                .unwrap()
                .actions
                .iter()
                .filter(|a| !a.is_tool)
                .map(|a| action_id(a.kind))
                .collect()
        };
        assert_eq!(
            ids("video"),
            ["media:mp4", "media:mkv", "media:webm", "media:mov", "media:avi", "media:gif", "media:mp3"]
        );
        assert_eq!(
            ids("audio"),
            ["media:mp3", "media:m4a", "media:wav", "media:flac", "media:ogg", "media:opus"]
        );
        assert_eq!(
            ids("image"),
            [
                "merge", "convert:jpg", "convert:png", "convert:webp", "convert:avif",
                "convert:gif", "convert:tiff", "convert:bmp"
            ]
        );
    }

    #[test]
    fn static_groups_have_no_duplicate_ids_and_keep_tools_last() {
        for group in static_menu_groups() {
            let ids: Vec<String> = group.actions.iter().map(|a| action_id(a.kind)).collect();
            let mut unique = ids.clone();
            unique.sort();
            unique.dedup();
            assert_eq!(ids.len(), unique.len(), "{} có id trùng", group.key);
            let first_tool = group.actions.iter().position(|a| a.is_tool);
            if let Some(index) = first_tool {
                assert!(group.actions[index..].iter().all(|a| a.is_tool), "{}", group.key);
            }
        }
    }

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
