use std::{
    fs::File,
    io::{Read, Write},
    path::{Path, PathBuf},
};

use flate2::{read::GzDecoder, write::GzEncoder, Compression};
use morphix_core::{
    actions_for,
    error::MorphixError,
    kind::ArchiveFormat,
    ops::archive::{create_archive, extract},
    Action, ActionKind, CancelToken, FileKind, ImgFormat,
};
use tempfile::{tempdir, TempDir};

/// Ghi file zip với các entry (đường dẫn nguyên văn, kể cả đường dẫn độc hại).
fn write_zip(path: &Path, entries: &[(&str, &[u8])]) {
    let mut zip = zip::ZipWriter::new(File::create(path).expect("tạo zip"));
    let options: zip::write::SimpleFileOptions = zip::write::SimpleFileOptions::default();
    for (name, bytes) in entries {
        zip.start_file(*name, options).expect("bắt đầu entry zip");
        zip.write_all(bytes).expect("ghi entry zip");
    }
    zip.finish().expect("kết thúc zip");
}

/// Ghi tar.gz với một entry có tên đặt thẳng vào trường name của header, để kiểm đường dẫn `..`
/// mà thư viện tar không chịu ghi qua API thường.
fn write_tar_gz_raw(path: &Path, name: &str, link: bool) {
    let mut header = tar::Header::new_gnu();
    let bytes: &[u8] = name.as_bytes();
    header.as_old_mut().name[..bytes.len()].copy_from_slice(bytes);
    header.set_mode(0o644);
    if link {
        header.set_entry_type(tar::EntryType::Symlink);
        header.set_size(0);
        header.set_link_name("/etc").expect("đặt link");
    } else {
        header.set_entry_type(tar::EntryType::Regular);
        header.set_size(3);
    }
    header.set_cksum();
    let gz = GzEncoder::new(
        File::create(path).expect("tạo tar.gz"),
        Compression::default(),
    );
    let mut builder = tar::Builder::new(gz);
    builder.append(&header, &b"bad"[..]).expect("ghi entry tar");
    builder
        .into_inner()
        .expect("kết thúc tar")
        .finish()
        .expect("kết thúc gzip");
}

fn write_tar_gz(path: &Path, entries: &[(&str, &[u8])]) {
    let gz = GzEncoder::new(
        File::create(path).expect("tạo tar.gz"),
        Compression::default(),
    );
    let mut builder = tar::Builder::new(gz);
    for (name, bytes) in entries {
        let mut header = tar::Header::new_gnu();
        header.set_size(bytes.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        builder
            .append_data(&mut header, name, *bytes)
            .expect("ghi entry tar");
    }
    builder
        .into_inner()
        .expect("kết thúc tar")
        .finish()
        .expect("kết thúc gzip");
}

/// Thư mục gốc có sẵn `out/` làm đích; trả (thư mục gốc, đích).
fn layout() -> (TempDir, PathBuf) {
    let root = tempdir().expect("thư mục tạm");
    let out = root.path().join("out");
    std::fs::create_dir(&out).expect("tạo đích");
    (root, out)
}

fn entry_count(dir: &Path) -> usize {
    std::fs::read_dir(dir).expect("đọc thư mục").count()
}

#[test]
fn zip_extracts_nested_entry_into_stem_folder() {
    let (root, out) = layout();
    let archive = root.path().join("bundle.zip");
    write_zip(&archive, &[("a/b.txt", b"xin chao")]);

    let dest = extract(
        &archive,
        ArchiveFormat::Zip,
        Some(&out),
        &CancelToken::new(),
    )
    .expect("giải nén zip");

    assert_eq!(dest, out.join("bundle"));
    assert_eq!(
        std::fs::read(dest.join("a/b.txt")).expect("đọc file giải nén"),
        b"xin chao"
    );
    assert_eq!(entry_count(&out), 1, "không được còn thư mục tạm");
}

#[test]
fn existing_folder_with_same_stem_gets_number_suffix() {
    let (root, out) = layout();
    let archive = root.path().join("bundle.zip");
    write_zip(&archive, &[("a.txt", b"moi")]);
    std::fs::create_dir(out.join("bundle")).expect("tạo thư mục trùng");
    std::fs::write(out.join("bundle/cu.txt"), b"cu").expect("ghi file cũ");

    let dest = extract(
        &archive,
        ArchiveFormat::Zip,
        Some(&out),
        &CancelToken::new(),
    )
    .expect("giải nén zip");

    assert_eq!(dest, out.join("bundle (1)"));
    assert_eq!(std::fs::read(dest.join("a.txt")).expect("đọc"), b"moi");
    assert_eq!(
        std::fs::read(out.join("bundle/cu.txt")).expect("đọc thư mục cũ"),
        b"cu",
        "thư mục đã có không được đụng tới"
    );
}

#[test]
fn zip_parent_traversal_is_rejected_and_nothing_written_outside() {
    let (root, out) = layout();
    let archive = root.path().join("evil.zip");
    write_zip(&archive, &[("ok.txt", b"ok"), ("../evil.txt", b"hai")]);

    let err = extract(
        &archive,
        ArchiveFormat::Zip,
        Some(&out),
        &CancelToken::new(),
    )
    .expect_err("phải từ chối ../");

    assert_corrupt_naming(&err, "../evil.txt");
    assert!(!root.path().join("evil.txt").exists(), "ghi ra ngoài đích");
    assert_eq!(entry_count(&out), 0, "thư mục tạm và file dở phải bị xoá");
}

#[test]
fn zip_absolute_entry_is_rejected() {
    let (root, out) = layout();
    let archive = root.path().join("abs.zip");
    // WHY: đường dẫn tuyệt đối trỏ vào thư mục gốc của test, để nếu lỗi lọt qua thì file được
    // ghi vào đó và test thấy ngay, mà không ghi gì ra ngoài thư mục tạm.
    let escaped: PathBuf = root.path().join("escaped.txt");
    let name: String = escaped.to_string_lossy().into_owned();
    write_zip(&archive, &[(name.as_str(), b"x")]);

    let err = extract(
        &archive,
        ArchiveFormat::Zip,
        Some(&out),
        &CancelToken::new(),
    )
    .expect_err("phải từ chối đường dẫn tuyệt đối");

    assert_corrupt_naming(&err, &name);
    assert!(!escaped.exists(), "ghi ra đường dẫn tuyệt đối");
    assert_eq!(entry_count(&out), 0);
}

#[test]
fn tar_gz_extracts_file() {
    let (root, out) = layout();
    let archive = root.path().join("pack.tar.gz");
    write_tar_gz(&archive, &[("docs/readme.txt", b"tai lieu")]);

    let dest = extract(
        &archive,
        ArchiveFormat::TarGz,
        Some(&out),
        &CancelToken::new(),
    )
    .expect("giải nén tar.gz");

    // WHY: tên `pack.tar.gz` bỏ luôn đuôi `.tar` để thư mục đích là `pack`, không phải `pack.tar`.
    assert_eq!(dest, out.join("pack"));
    assert_eq!(
        std::fs::read(dest.join("docs/readme.txt")).expect("đọc file"),
        "tai lieu".as_bytes()
    );
}

#[test]
fn tar_gz_parent_traversal_is_rejected() {
    let (root, out) = layout();
    let archive = root.path().join("evil.tar.gz");
    write_tar_gz_raw(&archive, "../evil.txt", false);

    let err = extract(
        &archive,
        ArchiveFormat::TarGz,
        Some(&out),
        &CancelToken::new(),
    )
    .expect_err("phải từ chối ..");

    assert_corrupt_naming(&err, "../evil.txt");
    assert!(!root.path().join("evil.txt").exists());
    assert_eq!(entry_count(&out), 0);
}

#[test]
fn tar_gz_symlink_entry_is_rejected() {
    let (root, out) = layout();
    let archive = root.path().join("link.tar.gz");
    write_tar_gz_raw(&archive, "lien", true);

    let err = extract(
        &archive,
        ArchiveFormat::TarGz,
        Some(&out),
        &CancelToken::new(),
    )
    .expect_err("phải từ chối liên kết mềm");

    assert_corrupt_naming(&err, "lien");
    assert_eq!(entry_count(&out), 0);
}

#[test]
fn sevenz_extracts_file() {
    let (root, out) = layout();
    let src = root.path().join("src");
    std::fs::create_dir_all(src.join("a")).expect("tạo nguồn");
    std::fs::write(src.join("a/b.txt"), b"bay").expect("ghi nguồn");
    // WHY: tạo fixture 7z ngay trong test bằng chính crate sevenz-rust2, không commit file nhị phân.
    let archive = root.path().join("pack.7z");
    sevenz_rust2::compress_to_path(&src, &archive).expect("tạo 7z");

    let dest = extract(
        &archive,
        ArchiveFormat::SevenZ,
        Some(&out),
        &CancelToken::new(),
    )
    .expect("giải nén 7z");

    assert_eq!(dest, out.join("pack"));
    assert_eq!(std::fs::read(dest.join("a/b.txt")).expect("đọc"), b"bay");
}

#[test]
fn rar_is_unsupported_for_now() {
    let (root, out) = layout();
    let archive = root.path().join("x.rar");
    std::fs::write(&archive, b"Rar!\x1a\x07\x00").expect("ghi rar giả");

    let err = extract(
        &archive,
        ArchiveFormat::Rar,
        Some(&out),
        &CancelToken::new(),
    )
    .expect_err("RAR chưa làm");

    assert!(matches!(err, MorphixError::Unsupported { .. }), "{err:?}");
    assert_eq!(entry_count(&out), 0);
}

#[test]
fn cancelled_before_start_leaves_no_folder() {
    let (root, out) = layout();
    let archive = root.path().join("bundle.zip");
    write_zip(&archive, &[("a.txt", b"a")]);
    let cancel = CancelToken::new();
    cancel.cancel();

    let err = extract(&archive, ArchiveFormat::Zip, Some(&out), &cancel).expect_err("phải báo huỷ");

    assert!(matches!(err, MorphixError::Cancelled), "{err:?}");
    assert_eq!(entry_count(&out), 0, "không được còn thư mục đích hay tạm");
}

#[test]
fn source_archive_is_never_modified() {
    let (root, out) = layout();
    let archive = root.path().join("bundle.zip");
    write_zip(&archive, &[("a.txt", b"a")]);
    let before: Vec<u8> = read_all(&archive);

    extract(
        &archive,
        ArchiveFormat::Zip,
        Some(&out),
        &CancelToken::new(),
    )
    .expect("giải nén zip");

    assert_eq!(read_all(&archive), before, "file nén gốc bị đổi");
}

fn read_all(path: &Path) -> Vec<u8> {
    let mut bytes: Vec<u8> = Vec::new();
    File::open(path)
        .expect("mở file")
        .read_to_end(&mut bytes)
        .expect("đọc file");
    bytes
}

/// Lỗi phải là `Corrupt` và lý do phải nêu đúng tên entry để người dùng biết file nào.
fn assert_corrupt_naming(err: &MorphixError, entry: &str) {
    match err {
        MorphixError::Corrupt { reason, .. } => {
            assert!(reason.contains(entry), "lý do thiếu tên entry: {reason}");
        }
        other => panic!("cần Corrupt, được {other:?}"),
    }
}

/// Ghi file nguồn `name` với nội dung `bytes` trong `dir`, trả đường dẫn.
fn make_file(dir: &Path, name: &str, bytes: &[u8]) -> PathBuf {
    std::fs::create_dir_all(dir).expect("tạo thư mục nguồn");
    let path: PathBuf = dir.join(name);
    std::fs::write(&path, bytes).expect("ghi file nguồn");
    path
}

/// `src/a.txt` và `src/b.txt` làm đầu vào, `out/` làm đích. Trả (thư mục gốc, đích, đầu vào).
fn two_sources() -> (TempDir, PathBuf, Vec<PathBuf>) {
    let (root, out) = layout();
    let src: PathBuf = root.path().join("src");
    let a: PathBuf = make_file(&src, "a.txt", b"alpha");
    let b: PathBuf = make_file(&src, "b.txt", b"beta");
    (root, out, vec![a, b])
}

fn archive_labels(actions: &[Action]) -> Vec<&str> {
    actions
        .iter()
        .filter(|a| matches!(a.kind, ActionKind::CreateArchive(_)))
        .map(|a| a.label.as_str())
        .collect()
}

#[test]
fn zip_of_two_files_reads_back_both_entries() {
    let (_root, out, inputs) = two_sources();
    let created: PathBuf =
        create_archive(&inputs, ArchiveFormat::Zip, Some(&out), &CancelToken::new())
            .expect("nén zip");

    assert_eq!(
        created.file_name().and_then(|n| n.to_str()),
        Some("a-nen.zip")
    );
    let mut zip = zip::ZipArchive::new(File::open(&created).expect("mở zip")).expect("đọc zip");
    assert_eq!(zip.len(), 2);
    let mut alpha: Vec<u8> = Vec::new();
    zip.by_name("a.txt")
        .expect("có a.txt")
        .read_to_end(&mut alpha)
        .expect("đọc a.txt");
    let mut beta: Vec<u8> = Vec::new();
    zip.by_name("b.txt")
        .expect("có b.txt")
        .read_to_end(&mut beta)
        .expect("đọc b.txt");
    assert_eq!(alpha, b"alpha");
    assert_eq!(beta, b"beta");
}

#[test]
fn tar_gz_of_two_files_reads_back_both_entries() {
    let (_root, out, inputs) = two_sources();
    let created: PathBuf = create_archive(
        &inputs,
        ArchiveFormat::TarGz,
        Some(&out),
        &CancelToken::new(),
    )
    .expect("nén tar.gz");

    assert_eq!(
        created.file_name().and_then(|n| n.to_str()),
        Some("a-nen.tar.gz")
    );
    let mut tar = tar::Archive::new(GzDecoder::new(File::open(&created).expect("mở tar.gz")));
    let mut found: Vec<(String, Vec<u8>)> = Vec::new();
    for entry in tar.entries().expect("đọc mục lục tar") {
        let mut entry = entry.expect("đọc một mục tar");
        let name: String = entry
            .path()
            .expect("đường dẫn")
            .to_string_lossy()
            .into_owned();
        let mut bytes: Vec<u8> = Vec::new();
        entry.read_to_end(&mut bytes).expect("đọc nội dung");
        found.push((name, bytes));
    }
    found.sort();
    assert_eq!(
        found,
        vec![
            ("a.txt".to_string(), b"alpha".to_vec()),
            ("b.txt".to_string(), b"beta".to_vec()),
        ]
    );
}

#[test]
fn sevenz_of_two_files_reads_back_both_entries() {
    let (root, out, inputs) = two_sources();
    let created: PathBuf = create_archive(
        &inputs,
        ArchiveFormat::SevenZ,
        Some(&out),
        &CancelToken::new(),
    )
    .expect("nén 7z");

    assert_eq!(
        created.file_name().and_then(|n| n.to_str()),
        Some("a-nen.7z")
    );
    let back: PathBuf = root.path().join("back");
    let folder: PathBuf = extract(
        &created,
        ArchiveFormat::SevenZ,
        Some(&back),
        &CancelToken::new(),
    )
    .expect("giải nén 7z");
    assert_eq!(read_all(&folder.join("a.txt")), b"alpha");
    assert_eq!(read_all(&folder.join("b.txt")), b"beta");
}

#[test]
fn round_trip_gives_identical_bytes_for_every_format() {
    let (root, out, inputs) = two_sources();
    for (format, label) in [
        (ArchiveFormat::Zip, "zip"),
        (ArchiveFormat::TarGz, "tgz"),
        (ArchiveFormat::SevenZ, "7z"),
    ] {
        let created: PathBuf =
            create_archive(&inputs, format, Some(&out), &CancelToken::new()).expect("nén");
        let back: PathBuf = root.path().join(label);
        let folder: PathBuf =
            extract(&created, format, Some(&back), &CancelToken::new()).expect("giải nén");
        assert_eq!(
            read_all(&folder.join("a.txt")),
            read_all(&inputs[0]),
            "{label}"
        );
        assert_eq!(
            read_all(&folder.join("b.txt")),
            read_all(&inputs[1]),
            "{label}"
        );
    }
}

#[test]
fn source_inputs_are_never_modified() {
    let (_root, out, inputs) = two_sources();
    let before: Vec<Vec<u8>> = inputs.iter().map(|p| read_all(p)).collect();

    create_archive(&inputs, ArchiveFormat::Zip, Some(&out), &CancelToken::new()).expect("nén zip");

    let after: Vec<Vec<u8>> = inputs.iter().map(|p| read_all(p)).collect();
    assert_eq!(after, before, "file nguồn bị đổi");
}

#[test]
fn same_file_name_from_two_folders_is_rejected_naming_the_duplicate() {
    let (root, out) = layout();
    let a: PathBuf = make_file(&root.path().join("s1"), "a.txt", b"one");
    let b: PathBuf = make_file(&root.path().join("s2"), "a.txt", b"two");

    let err = create_archive(&[a, b], ArchiveFormat::Zip, Some(&out), &CancelToken::new())
        .expect_err("phải từ chối tên trùng");

    match &err {
        MorphixError::InvalidInput(reason) => {
            assert!(reason.contains("a.txt"), "thiếu tên trùng: {reason}");
        }
        other => panic!("cần InvalidInput, được {other:?}"),
    }
    assert_eq!(entry_count(&out), 0, "không được còn file nào");
}

#[test]
fn directory_input_is_rejected() {
    let (root, out) = layout();
    let dir: PathBuf = root.path().join("thu-muc");
    std::fs::create_dir(&dir).expect("tạo thư mục");

    let err = create_archive(&[dir], ArchiveFormat::Zip, Some(&out), &CancelToken::new())
        .expect_err("thư mục không được nén");

    assert!(matches!(err, MorphixError::InvalidInput(_)), "{err:?}");
    assert_eq!(entry_count(&out), 0);
}

#[test]
fn existing_archive_name_gets_number_suffix() {
    let (_root, out, inputs) = two_sources();
    std::fs::write(out.join("a-nen.zip"), b"cu").expect("tạo file trùng");

    let created: PathBuf =
        create_archive(&inputs, ArchiveFormat::Zip, Some(&out), &CancelToken::new())
            .expect("nén zip");

    assert_eq!(
        created.file_name().and_then(|n| n.to_str()),
        Some("a-nen (1).zip")
    );
    assert_eq!(read_all(&out.join("a-nen.zip")), b"cu", "file có sẵn bị đè");
}

#[test]
fn tar_gz_collision_keeps_double_extension() {
    let (_root, out, inputs) = two_sources();
    std::fs::write(out.join("a-nen.tar.gz"), b"cu").expect("tạo file trùng");

    let created: PathBuf = create_archive(
        &inputs,
        ArchiveFormat::TarGz,
        Some(&out),
        &CancelToken::new(),
    )
    .expect("nén tar.gz");

    assert_eq!(
        created.file_name().and_then(|n| n.to_str()),
        Some("a-nen (1).tar.gz")
    );
}

#[test]
fn cancelled_before_start_leaves_no_output_or_temp() {
    let (_root, out, inputs) = two_sources();
    let cancel = CancelToken::new();
    cancel.cancel();

    let err =
        create_archive(&inputs, ArchiveFormat::Zip, Some(&out), &cancel).expect_err("phải báo huỷ");

    assert!(matches!(err, MorphixError::Cancelled), "{err:?}");
    assert_eq!(
        entry_count(&out),
        0,
        "không được còn file đích hay file tạm"
    );
}

#[test]
fn rar_is_not_a_create_target() {
    let (_root, out, inputs) = two_sources();

    let err = create_archive(&inputs, ArchiveFormat::Rar, Some(&out), &CancelToken::new())
        .expect_err("RAR chỉ đọc");

    assert!(matches!(err, MorphixError::Unsupported { .. }), "{err:?}");
    assert_eq!(entry_count(&out), 0);
}

#[test]
fn single_jpg_offers_three_archive_targets() {
    let actions: Vec<Action> = actions_for(&[FileKind::Image(ImgFormat::Jpeg)]);
    assert_eq!(
        archive_labels(&actions),
        ["Nén thành ZIP", "Nén thành TAR.GZ", "Nén thành 7Z"]
    );
}

#[test]
fn archive_only_selection_offers_extract_and_no_archive_target() {
    // WHY: chọn toàn file nén thì không đề nghị nén lại, chỉ đề nghị giải nén.
    let actions: Vec<Action> = actions_for(&[
        FileKind::Archive(ArchiveFormat::Zip),
        FileKind::Archive(ArchiveFormat::SevenZ),
    ]);
    assert!(archive_labels(&actions).is_empty(), "{actions:?}");
    let labels: Vec<&str> = actions.iter().map(|a| a.label.as_str()).collect();
    assert_eq!(labels, ["Giải nén"]);
}

#[test]
fn mixed_kinds_offer_archive_targets() {
    let actions: Vec<Action> = actions_for(&[
        FileKind::Image(ImgFormat::Png),
        FileKind::Archive(ArchiveFormat::Zip),
    ]);
    assert_eq!(archive_labels(&actions).len(), 3);
}
