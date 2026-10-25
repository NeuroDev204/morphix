use super::CaptureError;
use gtk::{gdk, glib, prelude::*};
use std::{cell::Cell, path::PathBuf, rc::Rc, time::Duration};

pub const MAX_SNAPSHOT_BYTES: usize = 10 * 1024 * 1024;
pub const SENSITIVE_TARGET: &str = "x-kde-passwordManagerHint";
const COPIED_FILES: &str = "x-special/gnome-copied-files";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClipboardEntry {
    pub target: String,
    pub data_type: String,
    pub format: i32,
    pub data: Vec<u8>,
}
#[derive(Debug)]
pub struct Snapshot {
    pub entries: Vec<ClipboardEntry>,
}

pub trait ClipboardAccess {
    fn targets(&mut self) -> Result<Vec<String>, CaptureError>;
    fn read(&mut self, target: &str) -> Result<ClipboardEntry, CaptureError>;
    fn write_all(&mut self, entries: &[ClipboardEntry]) -> Result<(), CaptureError>;
    fn clear(&mut self) -> Result<(), CaptureError>;
    fn owner_changes(&mut self) -> u64;
}

pub fn take_snapshot(cb: &mut dyn ClipboardAccess) -> Result<Snapshot, CaptureError> {
    let owner_before = cb.owner_changes();
    let targets = cb
        .targets()
        .map_err(|e| CaptureError::ClipboardUnsafe(e.to_string()))?;
    if targets.iter().any(|t| t == SENSITIVE_TARGET) {
        return Err(CaptureError::ClipboardUnsafe(
            "Nội dung được đánh dấu nhạy cảm.".into(),
        ));
    }
    let mut entries = Vec::new();
    let mut size = 0usize;
    for target in targets {
        if ["TARGETS", "TIMESTAMP", "MULTIPLE", "SAVE_TARGETS"].contains(&target.as_str()) {
            continue;
        }
        let entry = cb
            .read(&target)
            .map_err(|e| CaptureError::ClipboardUnsafe(e.to_string()))?;
        size = size.saturating_add(entry.data.len());
        if size > MAX_SNAPSHOT_BYTES {
            return Err(CaptureError::ClipboardUnsafe(
                "Nội dung vượt quá 10 MiB.".into(),
            ));
        }
        entries.push(entry);
    }
    if cb.owner_changes() != owner_before {
        return Err(CaptureError::ClipboardUnsafe(
            "Clipboard thay đổi trong lúc lưu; hãy thử lại.".into(),
        ));
    }
    Ok(Snapshot { entries })
}

pub fn restore(cb: &mut dyn ClipboardAccess, snapshot: &Snapshot) -> Result<(), CaptureError> {
    if snapshot.entries.is_empty() {
        cb.clear()
    } else {
        cb.write_all(&snapshot.entries)
    }
}

pub fn parse_copied_files(data: &[u8]) -> Result<Vec<PathBuf>, CaptureError> {
    let text = std::str::from_utf8(data)
        .map_err(|e| CaptureError::Other(format!("Danh sách file không phải UTF-8: {e}")))?;
    let mut lines = text.lines();
    if !matches!(lines.next(), Some("copy" | "cut")) {
        return Err(CaptureError::Other(
            "Danh sách file thiếu hành động copy/cut ở dòng 1.".into(),
        ));
    }
    let mut paths = Vec::new();
    for (index, line) in lines.enumerate() {
        let path = url::Url::parse(line)
            .ok()
            .filter(|url| url.scheme() == "file")
            .and_then(|url| url.to_file_path().ok())
            .filter(|path| path.is_absolute());
        match path {
            Some(path) => paths.push(path),
            None => {
                return Err(CaptureError::Other(format!(
                    "URI file cục bộ không hợp lệ ở dòng {}.",
                    index + 2
                )))
            }
        }
    }
    if paths.is_empty() {
        Err(CaptureError::NoFiles)
    } else {
        Ok(paths)
    }
}

pub fn capture_selection_via_clipboard(
    cb: &mut dyn ClipboardAccess,
    copy: &mut dyn FnMut() -> Result<(), CaptureError>,
    sleep: &mut dyn FnMut(Duration),
    wait: Duration,
) -> Result<Vec<PathBuf>, CaptureError> {
    let snapshot = take_snapshot(cb)?;
    let before = cb.owner_changes();
    let result = (|| {
        copy()?;
        let mut elapsed = Duration::ZERO;
        while cb.owner_changes() == before {
            if elapsed >= wait {
                return Err(CaptureError::NoFiles);
            }
            let step = Duration::from_millis(20).min(wait - elapsed);
            sleep(step);
            elapsed += step;
        }
        parse_copied_files(&cb.read(COPIED_FILES)?.data)
    })();
    // WHY: lỗi bấm phím, hết hạn và lỗi phân tích cũng phải trả clipboard trước khi báo lỗi.
    let restored = restore(cb, &snapshot);
    if let Err(error) = &result {
        tracing::warn!(%error, "không đọc được vùng chọn desktop");
    }
    restored?;
    result
}

pub struct GtkClipboard {
    clipboard: gtk::Clipboard,
    changes: Rc<Cell<u64>>,
    signal: Option<glib::SignalHandlerId>,
    session: super::x11::X11Session,
}
impl GtkClipboard {
    pub fn new() -> Result<Self, CaptureError> {
        let clipboard = gtk::Clipboard::get(&gdk::SELECTION_CLIPBOARD);
        let changes = Rc::new(Cell::new(0u64));
        let counter = changes.clone();
        // WHY: gtk-rs chưa sinh wrapper owner-change; chỉ cần tín hiệu, không giải mã GdkEvent.
        let signal = clipboard.connect_local("owner-change", false, move |_| {
            counter.set(counter.get().wrapping_add(1));
            None
        });
        Ok(Self {
            clipboard,
            changes,
            signal: Some(signal),
            session: super::x11::X11Session::connect()?,
        })
    }
}
impl Drop for GtkClipboard {
    fn drop(&mut self) {
        if let Some(signal) = self.signal.take() {
            self.clipboard.disconnect(signal);
        }
    }
}

pub fn gtk_sleep(duration: Duration) {
    let context = glib::MainContext::default();
    let deadline = std::time::Instant::now() + duration;
    while std::time::Instant::now() < deadline {
        while context.pending() {
            context.iteration(false);
        }
        std::thread::sleep(Duration::from_millis(1));
    }
}
impl ClipboardAccess for GtkClipboard {
    fn targets(&mut self) -> Result<Vec<String>, CaptureError> {
        if self.session.selection_owner()? == x11rb::NONE {
            return Ok(Vec::new());
        }
        self.clipboard
            .wait_for_targets()
            .map(|targets| {
                targets
                    .into_iter()
                    .map(|atom| atom.name().to_string())
                    .collect()
            })
            .ok_or_else(|| {
                CaptureError::ClipboardUnsafe("Không đọc được các định dạng clipboard.".into())
            })
    }
    fn read(&mut self, target: &str) -> Result<ClipboardEntry, CaptureError> {
        let contents = self
            .clipboard
            .wait_for_contents(&gdk::Atom::intern(target))
            .filter(|data| data.length() >= 0)
            .ok_or_else(|| {
                CaptureError::ClipboardUnsafe(format!("Không đọc được định dạng {target}."))
            })?;
        let format = contents.format();
        if ![8, 16, 32].contains(&format) {
            return Err(CaptureError::ClipboardUnsafe(format!(
                "Định dạng {target} có số bit không hợp lệ."
            )));
        }
        Ok(ClipboardEntry {
            target: target.into(),
            data_type: contents.data_type().name().to_string(),
            format,
            data: contents.data(),
        })
    }
    fn write_all(&mut self, entries: &[ClipboardEntry]) -> Result<(), CaptureError> {
        let targets: Vec<_> = entries
            .iter()
            .enumerate()
            .map(|(index, entry)| {
                gtk::TargetEntry::new(&entry.target, gtk::TargetFlags::empty(), index as u32)
            })
            .collect();
        let saved = entries.to_vec();
        if !self
            .clipboard
            .set_with_data(&targets, move |_, selection, index| {
                if let Some(entry) = saved.get(index as usize) {
                    selection.set(
                        &gdk::Atom::intern(&entry.data_type),
                        entry.format,
                        &entry.data,
                    );
                }
            })
        {
            return Err(CaptureError::ClipboardUnsafe(
                "Không trả lại được clipboard.".into(),
            ));
        }
        Ok(())
    }
    fn clear(&mut self) -> Result<(), CaptureError> {
        // WHY: gtk_clipboard_clear chỉ xoá khi GTK đang sở hữu; chiếm quyền trước để xoá cả clipboard của DING.
        self.write_all(&[ClipboardEntry {
            target: "UTF8_STRING".into(),
            data_type: "UTF8_STRING".into(),
            format: 8,
            data: Vec::new(),
        }])?;
        self.clipboard.clear();
        Ok(())
    }
    fn owner_changes(&mut self) -> u64 {
        let context = glib::MainContext::default();
        while context.pending() {
            context.iteration(false);
        }
        self.changes.get()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct FakeClipboard {
        entries: Vec<ClipboardEntry>,
        incoming: Option<Vec<u8>>,
        polls: u64,
        unreadable: bool,
        restored: bool,
        change_after: u64,
        restore_fails: bool,
    }
    impl ClipboardAccess for FakeClipboard {
        fn targets(&mut self) -> Result<Vec<String>, CaptureError> {
            Ok(self.entries.iter().map(|e| e.target.clone()).collect())
        }
        fn read(&mut self, target: &str) -> Result<ClipboardEntry, CaptureError> {
            if self.unreadable {
                return Err(CaptureError::Other("không đọc được".into()));
            }
            if self.polls > 3 && target == COPIED_FILES {
                if let Some(data) = &self.incoming {
                    return Ok(entry(target, data));
                }
            }
            self.entries
                .iter()
                .find(|e| e.target == target)
                .cloned()
                .ok_or(CaptureError::NoFiles)
        }
        fn write_all(&mut self, entries: &[ClipboardEntry]) -> Result<(), CaptureError> {
            if self.restore_fails {
                return Err(CaptureError::ClipboardUnsafe("test restore failed".into()));
            }
            self.entries = entries.to_vec();
            self.restored = true;
            Ok(())
        }
        fn clear(&mut self) -> Result<(), CaptureError> {
            self.entries.clear();
            self.restored = true;
            Ok(())
        }
        fn owner_changes(&mut self) -> u64 {
            self.polls += 1;
            if self.incoming.is_some() && self.polls > self.change_after {
                1
            } else {
                0
            }
        }
    }
    fn entry(target: &str, data: &[u8]) -> ClipboardEntry {
        ClipboardEntry {
            target: target.into(),
            data_type: target.into(),
            format: 8,
            data: data.to_vec(),
        }
    }
    fn fake(entries: Vec<ClipboardEntry>, incoming: Option<&[u8]>) -> FakeClipboard {
        FakeClipboard {
            entries,
            incoming: incoming.map(Vec::from),
            polls: 0,
            unreadable: false,
            restored: false,
            change_after: 3,
            restore_fails: false,
        }
    }
    #[test]
    fn clipboard_changes_while_snapshotting_aborts_before_copy() {
        let mut cb = fake(
            vec![entry("text/plain", b"user data")],
            Some(b"copy\nfile:///tmp/a"),
        );
        cb.change_after = 1;
        assert!(matches!(
            capture_selection_via_clipboard(
                &mut cb,
                &mut || panic!("không được copy"),
                &mut |_| {},
                Duration::ZERO
            ),
            Err(CaptureError::ClipboardUnsafe(_))
        ));
        assert!(!cb.restored);
    }
    #[test]
    fn restore_failure_is_reported_instead_of_success() {
        let mut cb = fake(
            vec![entry("text/plain", b"user data")],
            Some(b"copy\nfile:///tmp/a"),
        );
        cb.restore_fails = true;
        let error =
            capture_selection_via_clipboard(&mut cb, &mut || Ok(()), &mut |_| {}, Duration::ZERO)
                .unwrap_err();
        assert!(error.to_string().contains("test restore failed"));
    }
    const FILES: &[u8] = b"copy\nfile:///tmp/a.jpg\n";
    #[test]
    fn empty_and_image_clipboard_are_restored_byte_for_byte() {
        for original in [
            vec![],
            vec![
                entry("image/png", b"\x89PNG\0\xff"),
                ClipboardEntry {
                    target: "custom".into(),
                    data_type: "INTEGER".into(),
                    format: 32,
                    data: vec![1, 0, 0, 0],
                },
            ],
        ] {
            let mut cb = fake(original.clone(), Some(FILES));
            let paths = capture_selection_via_clipboard(
                &mut cb,
                &mut || Ok(()),
                &mut |_| {},
                Duration::from_secs(1),
            )
            .unwrap();
            assert_eq!(paths, [PathBuf::from("/tmp/a.jpg")]);
            assert!(cb.restored);
            assert_eq!(cb.entries, original);
        }
    }
    #[test]
    fn unsafe_clipboards_never_trigger_copy() {
        let mut sensitive = fake(vec![entry(SENSITIVE_TARGET, b"1")], None);
        let mut oversized = fake(
            vec![entry("image/png", &vec![0; MAX_SNAPSHOT_BYTES + 1])],
            None,
        );
        let mut unreadable = fake(vec![entry("text/plain", b"text")], None);
        unreadable.unreadable = true;
        for cb in [&mut sensitive, &mut oversized, &mut unreadable] {
            let original = cb.entries.clone();
            let result = capture_selection_via_clipboard(
                cb,
                &mut || panic!("không được bấm Ctrl+C"),
                &mut |_| {},
                Duration::ZERO,
            );
            assert!(matches!(result, Err(CaptureError::ClipboardUnsafe(_))));
            assert_eq!(cb.entries, original);
            assert!(!cb.restored);
        }
    }
    #[test]
    fn stale_copied_files_without_owner_change_is_no_files() {
        let original = vec![entry(COPIED_FILES, FILES)];
        let mut cb = fake(original.clone(), None);
        assert_eq!(
            capture_selection_via_clipboard(
                &mut cb,
                &mut || Ok(()),
                &mut |_| {},
                Duration::from_millis(40)
            ),
            Err(CaptureError::NoFiles)
        );
        assert_eq!(cb.entries, original);
        assert!(cb.restored);
    }
    #[test]
    fn restore_runs_even_when_parse_or_copy_fails() {
        for incoming in [b"bad".as_slice(), FILES] {
            let original = vec![entry("text/plain", b"user data")];
            let mut cb = fake(original.clone(), Some(incoming));
            let fail_copy = incoming == FILES;
            let result = capture_selection_via_clipboard(
                &mut cb,
                &mut || {
                    if fail_copy {
                        Err(CaptureError::KeysFailed("test".into()))
                    } else {
                        Ok(())
                    }
                },
                &mut |_| {},
                Duration::ZERO,
            );
            assert!(result.is_err());
            assert!(cb.restored);
            assert_eq!(cb.entries, original);
        }
    }
    #[test]
    fn parse_decodes_vietnamese_and_spaces_and_rejects_bad_uri() {
        assert_eq!(
            parse_copied_files(
                b"cut\nfile:///home/user/Documents/Ti%E1%BA%BFng%20Anh/b%C3%A0i.docx"
            )
            .unwrap(),
            [PathBuf::from("/home/user/Documents/Tiếng Anh/bài.docx")]
        );
        for input in [
            "copy\nhttps://example.com/a",
            "copy\n",
            "copy\nfile://remote/tmp/a",
            "copy\nfile:///tmp/a\n\n",
        ] {
            assert!(parse_copied_files(input.as_bytes()).is_err(), "{input}");
        }
        assert!(parse_copied_files(b"copy\nhttps://example.com")
            .unwrap_err()
            .to_string()
            .contains("2"));
    }
}
