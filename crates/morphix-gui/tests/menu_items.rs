use morphix_core::kind::{ArchiveFormat, MediaFormat, TextFormat};
use morphix_core::{FileKind, ImgFormat, Operation, Quality, ResizeSpec};
use morphix_gui::menu_items::{operation_for, quality_for, wheel_items, WheelItem, WIRED_TOOL_IDS};
use morphix_gui::settings::{QualityChoice, Settings};
use std::path::PathBuf;

#[test]
fn single_jpeg_wheel_starts_with_pdf_and_marks_it_suggested() {
    let items = wheel_items(&[FileKind::Image(ImgFormat::Jpeg)]);
    assert_eq!(items[0].label, "PDF");
    assert!(items[0].is_suggested);
    assert!(items.iter().skip(1).all(|i| !i.is_suggested));
}

#[test]
fn several_images_wheel_says_pdf_gop() {
    let items = wheel_items(&[
        FileKind::Image(ImgFormat::Png),
        FileKind::Image(ImgFormat::Jpeg),
    ]);
    assert_eq!(items[0].label, "PDF gộp");
    assert_eq!(items[0].id, "merge");
}

#[test]
fn mp4_wheel_offers_convert_targets_and_compress_only() {
    let items = wheel_items(&[FileKind::Media(MediaFormat::Mp4)]);
    let ids: Vec<&str> = items.iter().map(|i| i.id.as_str()).collect();
    assert!(ids.contains(&"media:webm"));
    assert!(ids.contains(&"media:mp3"));
    assert!(ids.contains(&"compress-media"));
    // WHY: cắt, đổi tốc độ và tách âm thanh chỉ có ở CLI trong bản này (spec mục 2a).
    assert!(ids
        .iter()
        .all(|id| !matches!(*id, "cut" | "speed") && !id.starts_with("extract:")));
}

#[test]
fn media_ids_map_to_convert_and_compress_operations() {
    let s = Settings::default();
    let inputs = vec![PathBuf::from("a.mp4")];
    assert_eq!(
        operation_for("media:webm", &inputs, &s).unwrap(),
        Operation::ConvertMedia {
            to: MediaFormat::WebM
        }
    );
    assert_eq!(
        operation_for("compress-media", &inputs, &s).unwrap(),
        Operation::CompressMedia
    );
    assert!(operation_for("media:nope", &inputs, &s).is_err());
}

#[test]
fn tools_without_a_screen_are_not_offered() {
    let items = wheel_items(&[FileKind::Image(ImgFormat::Jpeg)]);
    assert!(items
        .iter()
        .all(|i| !i.is_tool || WIRED_TOOL_IDS.contains(&i.id.as_str())));
    assert!(items.iter().any(|i| i.id == "strip"));
}

#[test]
fn operation_for_maps_ids_and_uses_settings_percent() {
    let s = Settings {
        default_resize_percent: 25,
        ..Settings::default()
    };
    let inputs = vec![PathBuf::from("a.jpg")];
    assert_eq!(
        operation_for("convert:png", &inputs, &s).unwrap(),
        Operation::ConvertImage { to: ImgFormat::Png }
    );
    assert_eq!(
        operation_for("resize", &inputs, &s).unwrap(),
        Operation::ResizeImage(ResizeSpec::Percent(25))
    );
    assert!(operation_for("cat-anh", &inputs, &s).is_err());
}

#[test]
fn operation_for_maps_merge_compress_and_strip() {
    let s = Settings::default();
    let inputs = vec![PathBuf::from("a.jpg")];
    assert_eq!(
        operation_for("merge", &inputs, &s).unwrap(),
        Operation::MergeImagesToPdf
    );
    assert_eq!(
        operation_for("compress", &inputs, &s).unwrap(),
        Operation::CompressImage
    );
    assert_eq!(
        operation_for("strip", &inputs, &s).unwrap(),
        Operation::StripMetadata
    );
}

#[test]
fn operation_for_rejects_invalid_convert_target() {
    let s = Settings::default();
    let inputs = vec![PathBuf::from("a.jpg")];
    assert!(operation_for("convert:nonexistent", &inputs, &s).is_err());
    assert!(operation_for("convert:", &inputs, &s).is_err());
}

#[test]
fn quality_for_reflects_settings_choice() {
    let s_small = Settings {
        quality: QualityChoice::Small,
        ..Settings::default()
    };
    assert_eq!(quality_for(&s_small), Quality::Small);

    let s_balanced = Settings {
        quality: QualityChoice::Balanced,
        ..Settings::default()
    };
    assert_eq!(quality_for(&s_balanced), Quality::Balanced);

    let s_high = Settings {
        quality: QualityChoice::High,
        ..Settings::default()
    };
    assert_eq!(quality_for(&s_high), Quality::High);
}

#[test]
fn subtitle_selection_has_no_wheel_yet() {
    // WHY: giao diện chưa có luồng chuyển văn bản, nên vòng tròn chỉ còn múi nén (không phải file nén nào cũng nén được gộp).
    let items = wheel_items(&[FileKind::Text(TextFormat::Srt)]);
    assert!(
        items.iter().all(|i| i.id.starts_with("archive:")),
        "{items:?}"
    );
}

#[test]
fn single_pdf_wheel_offers_txt_as_the_suggested_convert() {
    // WHY: file không phải file nén luôn có múi nén; bài test này chỉ xét việc theo loại file.
    let items: Vec<WheelItem> = wheel_items(&[FileKind::Pdf])
        .into_iter()
        .filter(|i| !i.id.starts_with("archive:"))
        .collect();
    assert_eq!(items.len(), 1, "{items:?}");
    assert_eq!(items[0].label, "TXT");
    assert_eq!(items[0].id, "pdf-to-text");
    assert!(!items[0].is_tool);
    assert!(items[0].is_suggested);
}

#[test]
fn several_pdfs_wheel_offers_nothing() {
    // WHY: mỗi PDF cho một TXT tên theo chính nó; nhiều PDF không có việc gộp văn bản nên không hiện múi.
    assert!(wheel_items(&[FileKind::Pdf, FileKind::Pdf])
        .iter()
        .all(|i| i.id.starts_with("archive:")));
}

#[test]
fn pdf_to_text_id_maps_to_operation() {
    let s = Settings::default();
    let inputs = vec![PathBuf::from("a.pdf")];
    assert_eq!(
        operation_for("pdf-to-text", &inputs, &s).unwrap(),
        Operation::PdfToText
    );
}

#[test]
fn jpeg_wheel_offers_archive_targets() {
    let items = wheel_items(&[FileKind::Image(ImgFormat::Jpeg)]);
    let ids: Vec<&str> = items.iter().map(|i| i.id.as_str()).collect();
    for id in ["archive:zip", "archive:tar.gz", "archive:7z"] {
        assert!(ids.contains(&id), "{ids:?}");
    }
}

#[test]
fn zip_wheel_offers_extract_as_the_suggested_action() {
    let items = wheel_items(&[FileKind::Archive(ArchiveFormat::Zip)]);
    assert_eq!(items.len(), 1, "{items:?}");
    assert_eq!(items[0].id, "extract");
    assert_eq!(items[0].label, "Giải nén");
    assert!(items[0].is_suggested);
}

#[test]
fn rar_wheel_offers_nothing_until_rar_reading_is_licensed() {
    assert!(wheel_items(&[FileKind::Archive(ArchiveFormat::Rar)]).is_empty());
}

#[test]
fn archive_ids_map_to_create_and_extract_operations() {
    let s = Settings::default();
    let inputs = vec![PathBuf::from("a.zip")];
    assert_eq!(
        operation_for("archive:zip", &inputs, &s).unwrap(),
        Operation::CreateArchive {
            format: ArchiveFormat::Zip
        }
    );
    assert_eq!(
        operation_for("archive:tar.gz", &inputs, &s).unwrap(),
        Operation::CreateArchive {
            format: ArchiveFormat::TarGz
        }
    );
    assert_eq!(
        operation_for("archive:7z", &inputs, &s).unwrap(),
        Operation::CreateArchive {
            format: ArchiveFormat::SevenZ
        }
    );
    assert_eq!(
        operation_for("extract", &inputs, &s).unwrap(),
        Operation::ExtractArchive
    );
    assert!(operation_for("archive:rar", &inputs, &s).is_err());
}

#[test]
fn every_wheel_item_maps_to_an_operation() {
    // WHY: id không ánh xạ được sẽ lỗi lúc bấm; test này bắt lỗi đó ngay khi thêm việc mới vào catalog.
    let s = Settings::default();
    let inputs = vec![PathBuf::from("a.jpg")];
    let selections = [
        vec![FileKind::Image(ImgFormat::Jpeg)],
        vec![FileKind::Archive(ArchiveFormat::Zip)],
    ];
    for kinds in &selections {
        for item in wheel_items(kinds) {
            assert!(
                operation_for(&item.id, &inputs, &s).is_ok(),
                "không ánh xạ được {}",
                item.id
            );
        }
    }
}
