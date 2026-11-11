//! Cài, gỡ và kiểm một thành phần gồm nhiều file do Morphix sinh ra (service menu Dolphin, cây Scripts).
//! Mỗi thao tác là một giao dịch: lỗi giữa chừng thì mọi file đã ghi hoặc đã xoá được hoàn lại.

use std::{
    fs, io,
    path::{Path, PathBuf},
};

use super::{
    files::{classify, read_existing, remove_file_if_owned, FileOwnership, Transaction},
    linux::{
        io_failure, is_same, remove_created_dirs, report, write_with_created_dirs, DesiredFile,
        CREATED_DIR_MARKER,
    },
    ComponentId, ComponentStatus, IntegrationComponentReport,
};

/// Ghi chú loại menu, hiện trong báo cáo của `doctor`.
pub(super) const STATIC_TIER: &str = "menu tĩnh (theo loại file)";

/// Nơi tìm file Morphix đã sinh ở bản trước mà bản này không còn sinh.
pub(super) enum Scan {
    /// File phẳng tên `<prefix>…<suffix>` trong một thư mục.
    Flat {
        dir: PathBuf,
        prefix: &'static str,
        suffix: &'static str,
    },
    /// Mọi file dưới một cây thư mục.
    Tree(PathBuf),
}

pub(super) struct FileSet {
    pub id: ComponentId,
    /// Thư mục dùng làm `path` trong báo cáo.
    pub dir: PathBuf,
    pub files: Vec<DesiredFile>,
    /// File kiểu một mục của bản cũ; chỉ xoá khi có marker của Morphix.
    pub legacy: Vec<PathBuf>,
    pub scan: Scan,
}

fn is_owned(path: &Path) -> io::Result<bool> {
    Ok(classify(read_existing(path)?.as_ref()) == FileOwnership::Owned)
}

fn walk_files(dir: &Path, out: &mut Vec<PathBuf>) -> io::Result<()> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    for entry in entries {
        let path = entry?.path();
        if path.is_dir() {
            walk_files(&path, out)?;
        } else {
            out.push(path);
        }
    }
    Ok(())
}

/// File mang marker của Morphix nằm trong vùng quét nhưng không còn nằm trong danh sách mong muốn,
/// cộng các file kiểu cũ còn marker. Cả hai đều là thứ bản này phải dọn.
fn leftovers(set: &FileSet) -> io::Result<Vec<PathBuf>> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    match &set.scan {
        Scan::Flat {
            dir,
            prefix,
            suffix,
        } => {
            let mut all = Vec::new();
            walk_files(dir, &mut all)?;
            candidates.extend(all.into_iter().filter(|path| {
                path.parent() == Some(dir.as_path())
                    && path
                        .file_name()
                        .and_then(|name| name.to_str())
                        .is_some_and(|name| name.starts_with(prefix) && name.ends_with(suffix))
            }));
        }
        Scan::Tree(root) => walk_files(root, &mut candidates)?,
    }
    let mut found = Vec::new();
    for path in candidates {
        let is_dir_marker = path
            .file_name()
            .is_some_and(|name| name == CREATED_DIR_MARKER);
        if is_dir_marker || set.files.iter().any(|file| file.path == path) {
            continue;
        }
        if is_owned(&path)? {
            found.push(path);
        }
    }
    for path in &set.legacy {
        if is_owned(path)? {
            found.push(path.clone());
        }
    }
    Ok(found)
}

pub(super) fn install(set: &FileSet) -> IntegrationComponentReport {
    let id = set.id;
    let mut current = Vec::with_capacity(set.files.len());
    for file in &set.files {
        let existing = match read_existing(&file.path) {
            Ok(existing) => existing,
            Err(error) => return io_failure(id, &file.path, &error),
        };
        if classify(existing.as_ref()) == FileOwnership::Foreign {
            // WHY dừng cả thành phần: ghi một nửa sẽ cho menu thiếu mục mà doctor khó giải thích.
            return report(
                id,
                ComponentStatus::Conflict,
                false,
                Some(&file.path),
                "đã có file không thuộc Morphix tại đường dẫn này; giữ nguyên",
            );
        }
        current.push(existing);
    }
    let stale = match leftovers(set) {
        Ok(stale) => stale,
        Err(error) => return io_failure(id, &set.dir, &error),
    };
    let up_to_date = stale.is_empty()
        && set
            .files
            .iter()
            .zip(&current)
            .all(|(file, now)| now.as_ref().is_some_and(|now| is_same(now, file)));
    if up_to_date {
        return report(
            id,
            ComponentStatus::AlreadyInstalled,
            false,
            Some(&set.dir),
            &format!("đã cài sẵn · {STATIC_TIER}"),
        );
    }
    let mut tx = Transaction::new();
    match apply_install(set, &current, &stale, &mut tx) {
        Ok(()) => {
            tx.commit();
            report(
                id,
                ComponentStatus::Installed,
                true,
                Some(&set.dir),
                &format!("đã cài {} file · {STATIC_TIER}", set.files.len()),
            )
        }
        // WHY không commit: `tx` đảo ngược các bước của lần cài này khi rời phạm vi.
        Err(error) => io_failure(id, &set.dir, &error),
    }
}

fn apply_install(
    set: &FileSet,
    current: &[Option<super::files::ExistingFile>],
    stale: &[PathBuf],
    tx: &mut Transaction,
) -> io::Result<()> {
    for (file, now) in set.files.iter().zip(current) {
        if now.as_ref().is_some_and(|now| is_same(now, file)) {
            continue;
        }
        write_with_created_dirs(file, tx)?;
    }
    for path in stale {
        remove_file_if_owned(path, tx)?;
    }
    for dir in stale.iter().filter_map(|path| path.parent()) {
        remove_created_dirs(dir)?;
    }
    Ok(())
}

pub(super) fn uninstall(set: &FileSet) -> IntegrationComponentReport {
    let id = set.id;
    let mut tx = Transaction::new();
    let mut removed: Vec<PathBuf> = Vec::new();
    let mut foreign = false;
    for file in &set.files {
        match remove_file_if_owned(&file.path, &mut tx) {
            Ok(FileOwnership::Owned) => removed.push(file.path.clone()),
            Ok(FileOwnership::Foreign) => foreign = true,
            Ok(FileOwnership::Absent) => {}
            Err(error) => return io_failure(id, &file.path, &error),
        }
    }
    let stale = match leftovers(set) {
        Ok(stale) => stale,
        Err(error) => return io_failure(id, &set.dir, &error),
    };
    for path in &stale {
        match remove_file_if_owned(path, &mut tx) {
            Ok(FileOwnership::Owned) => removed.push(path.clone()),
            Ok(_) => {}
            Err(error) => return io_failure(id, path, &error),
        }
    }
    // WHY lỗi dọn thư mục làm rollback file: `tx` còn chưa commit nên file được khôi phục.
    for dir in removed.iter().filter_map(|path| path.parent()) {
        if let Err(error) = remove_created_dirs(dir) {
            return io_failure(id, dir, &error);
        }
    }
    tx.commit();
    if foreign {
        return report(
            id,
            ComponentStatus::Conflict,
            !removed.is_empty(),
            Some(&set.dir),
            "có file không thuộc Morphix; giữ nguyên",
        );
    }
    if removed.is_empty() {
        return report(
            id,
            ComponentStatus::AlreadyAbsent,
            false,
            Some(&set.dir),
            "chưa cài",
        );
    }
    report(
        id,
        ComponentStatus::Removed,
        true,
        Some(&set.dir),
        &format!("đã gỡ {} file", removed.len()),
    )
}

pub(super) fn inspect(set: &FileSet) -> IntegrationComponentReport {
    let id = set.id;
    let (mut owned, mut same) = (0, 0);
    for file in &set.files {
        let existing = match read_existing(&file.path) {
            Ok(existing) => existing,
            Err(error) => return io_failure(id, &file.path, &error),
        };
        match classify(existing.as_ref()) {
            FileOwnership::Absent => {}
            FileOwnership::Foreign => {
                return report(
                    id,
                    ComponentStatus::Conflict,
                    false,
                    Some(&file.path),
                    "file không thuộc Morphix",
                )
            }
            FileOwnership::Owned => {
                owned += 1;
                if existing.as_ref().is_some_and(|now| is_same(now, file)) {
                    same += 1;
                }
            }
        }
    }
    if owned == 0 {
        return report(id, ComponentStatus::Missing, false, Some(&set.dir), "chưa cài");
    }
    let stale = match leftovers(set) {
        Ok(stale) => stale,
        Err(error) => return io_failure(id, &set.dir, &error),
    };
    if same == set.files.len() && stale.is_empty() {
        return report(
            id,
            ComponentStatus::Installed,
            false,
            Some(&set.dir),
            &format!("đang hoạt động · {STATIC_TIER}"),
        );
    }
    report(
        id,
        ComponentStatus::Stale,
        false,
        Some(&set.dir),
        "thiếu file, còn file cũ hoặc morphix đã được di chuyển hoặc cập nhật; chạy lại install-integration",
    )
}
