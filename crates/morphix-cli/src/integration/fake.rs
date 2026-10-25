//! Backend giả lập bằng file trong một thư mục gốc được inject. Test manager và helper file dùng nó,
//! không chạm HOME, XDG hay registry thật. Mỗi thành phần ghi hai file để test rollback được file đầu.

use std::{
    io,
    path::{Path, PathBuf},
};

use super::{
    files::{
        classify, publish_file, read_existing, remove_dir_if_empty, remove_file_if_owned,
        FileOwnership, Transaction, MANAGED_MARKER,
    },
    BinaryPaths, ComponentId, ComponentStatus, IntegrationBackend, IntegrationComponentReport,
};

const FAKE_FILE_MODE: u32 = 0o755;

pub struct FakeBackend {
    root: PathBuf,
    fail_after_first_file: Vec<ComponentId>,
}

impl FakeBackend {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        FakeBackend {
            root: root.into(),
            fail_after_first_file: Vec::new(),
        }
    }

    /// Lệnh cài của `id` lỗi sau khi đã ghi file đầu tiên, để kiểm rollback.
    pub fn failing_after_first_file(mut self, id: ComponentId) -> Self {
        self.fail_after_first_file.push(id);
        self
    }

    pub fn entry_path(&self, id: ComponentId) -> PathBuf {
        self.root.join(id.as_str()).join("entry.desktop")
    }

    fn companion_path(&self, id: ComponentId) -> PathBuf {
        self.root.join(id.as_str()).join("companion.txt")
    }

    fn desired_content(morphix: &Path) -> Vec<u8> {
        format!("# {MANAGED_MARKER}\ntarget={}\n", morphix.display()).into_bytes()
    }

    fn write_pair(&self, id: ComponentId, desired: &[u8], tx: &mut Transaction) -> io::Result<()> {
        publish_file(&self.entry_path(id), desired, FAKE_FILE_MODE, tx)?;
        if self.fail_after_first_file.contains(&id) {
            return Err(io::Error::other("lỗi giả lập sau file đầu tiên"));
        }
        publish_file(&self.companion_path(id), desired, FAKE_FILE_MODE, tx)
    }
}

fn report(
    id: ComponentId,
    status: ComponentStatus,
    changed: bool,
    path: &Path,
    message: &str,
) -> IntegrationComponentReport {
    IntegrationComponentReport {
        id,
        status,
        changed,
        path: Some(path.to_path_buf()),
        message: message.to_string(),
    }
}

fn io_failure(id: ComponentId, path: &Path, error: &io::Error) -> IntegrationComponentReport {
    tracing::error!(%error, path = %path.display(), "thao tác file giả lập lỗi");
    report(
        id,
        ComponentStatus::Failed,
        false,
        path,
        &format!("không ghi/đọc được {}: {error}", path.display()),
    )
}

fn recorded_target(bytes: &[u8]) -> Option<String> {
    String::from_utf8_lossy(bytes)
        .lines()
        .find_map(|line| line.strip_prefix("target=").map(str::to_string))
}

impl IntegrationBackend for FakeBackend {
    fn platform(&self) -> &'static str {
        "fake"
    }

    fn install(&self, id: ComponentId, binaries: &BinaryPaths) -> IntegrationComponentReport {
        let entry = self.entry_path(id);
        let existing = match read_existing(&entry) {
            Ok(existing) => existing,
            Err(error) => return io_failure(id, &entry, &error),
        };
        let desired = Self::desired_content(&binaries.morphix);
        match classify(existing.as_ref()) {
            FileOwnership::Foreign => {
                return report(
                    id,
                    ComponentStatus::Conflict,
                    false,
                    &entry,
                    "đã có file không thuộc Morphix tại đường dẫn này; giữ nguyên",
                )
            }
            FileOwnership::Owned if existing.as_ref().is_some_and(|f| f.bytes == desired) => {
                return report(
                    id,
                    ComponentStatus::AlreadyInstalled,
                    false,
                    &entry,
                    "đã cài sẵn",
                )
            }
            _ => {}
        }
        let mut tx = Transaction::new();
        match self.write_pair(id, &desired, &mut tx) {
            Ok(()) => {
                tx.commit();
                report(id, ComponentStatus::Installed, true, &entry, "đã cài")
            }
            // WHY không commit: `tx` rollback các file và thư mục do lần cài này tạo khi rời phạm vi.
            Err(error) => io_failure(id, &entry, &error),
        }
    }

    fn uninstall(&self, id: ComponentId) -> IntegrationComponentReport {
        let entry = self.entry_path(id);
        let mut tx = Transaction::new();
        let result = remove_file_if_owned(&entry, &mut tx).and_then(|ownership| {
            if ownership == FileOwnership::Owned {
                remove_file_if_owned(&self.companion_path(id), &mut tx)?;
                remove_dir_if_empty(&self.root.join(id.as_str()))?;
            }
            Ok(ownership)
        });
        match result {
            Ok(FileOwnership::Owned) => {
                tx.commit();
                report(id, ComponentStatus::Removed, true, &entry, "đã gỡ")
            }
            Ok(FileOwnership::Absent) => report(
                id,
                ComponentStatus::AlreadyAbsent,
                false,
                &entry,
                "chưa cài",
            ),
            Ok(FileOwnership::Foreign) => report(
                id,
                ComponentStatus::Conflict,
                false,
                &entry,
                "file không thuộc Morphix; giữ nguyên",
            ),
            Err(error) => io_failure(id, &entry, &error),
        }
    }

    fn inspect(&self, id: ComponentId, binaries: &BinaryPaths) -> IntegrationComponentReport {
        let entry = self.entry_path(id);
        let existing = match read_existing(&entry) {
            Ok(existing) => existing,
            Err(error) => return io_failure(id, &entry, &error),
        };
        match classify(existing.as_ref()) {
            FileOwnership::Absent => {
                report(id, ComponentStatus::Missing, false, &entry, "chưa cài")
            }
            FileOwnership::Foreign => report(
                id,
                ComponentStatus::Conflict,
                false,
                &entry,
                "file không thuộc Morphix",
            ),
            FileOwnership::Owned => {
                let bytes = existing.map(|f| f.bytes).unwrap_or_default();
                let current = binaries.morphix.display().to_string();
                if recorded_target(&bytes).as_deref() == Some(current.as_str()) {
                    report(
                        id,
                        ComponentStatus::Installed,
                        false,
                        &entry,
                        "đang hoạt động",
                    )
                } else {
                    report(
                        id,
                        ComponentStatus::Stale,
                        false,
                        &entry,
                        "morphix đã được di chuyển; chạy lại install-integration",
                    )
                }
            }
        }
    }
}
