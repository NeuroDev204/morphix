use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use morphix_core::{run_job, CancelToken, Job, JobReport, MorphixError, Operation, Progress};
use serde::{Deserialize, Serialize};

/// Trạng thái của một tác vụ đang chạy hoặc đã kết thúc.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum JobState {
    Running,
    Finished { ok: usize, failed: usize },
    Cancelled,
}

/// Ảnh chụp nhanh tiến trình xử lý của tác vụ tại một thời điểm.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JobSnapshot {
    /// Định danh duy nhất của tác vụ.
    pub id: u64,
    /// Nhãn mô tả tác vụ hiển thị cho người dùng (ví dụ: "Đổi sang JPG").
    pub label: String,
    /// Số lượng file đã hoàn thành.
    pub done: usize,
    /// Tổng số file cần xử lý.
    pub total: usize,
    /// Trạng thái hiện tại của tác vụ.
    pub state: JobState,
}

/// Thông tin về tác vụ đang chạy trên luồng nền.
struct ActiveJob {
    id: u64,
    cancel: CancelToken,
    snapshot: JobSnapshot,
}

struct Inner {
    next_id: AtomicU64,
    active: Mutex<Option<ActiveJob>>,
    report: Mutex<Option<(u64, JobReport)>>,
    worker: Mutex<Option<JoinHandle<()>>>,
}

/// Trình quản lý tác vụ xử lý ảnh trên luồng nền, đảm bảo chỉ có tối đa một tác vụ chạy tại một thời điểm.
#[derive(Clone)]
pub struct JobRunner {
    inner: Arc<Inner>,
}

impl Default for JobRunner {
    fn default() -> Self {
        Self::new()
    }
}

impl JobRunner {
    /// Khởi tạo một `JobRunner` mới với bộ đếm id bắt đầu từ 1.
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Inner {
                next_id: AtomicU64::new(1),
                active: Mutex::new(None),
                report: Mutex::new(None),
                worker: Mutex::new(None),
            }),
        }
    }

    /// Khởi chạy một tác vụ mới trên luồng nền.
    ///
    /// Trả về `Err` nếu đang có tác vụ chạy hoặc báo cáo của tác vụ trước đó chưa được lấy ra qua `take_report`.
    pub fn start(
        &self,
        job: Job,
        label: String,
        on_update: Arc<dyn Fn(JobSnapshot) + Send + Sync>,
    ) -> Result<u64, String> {
        // Sử dụng unwrap_or_else(|p| p.into_inner()) khi khoá Mutex:
        // Nếu một luồng trước đó gặp panic trong vùng găng, khoá sẽ bị "poisoned". Thay vì gây đổ vỡ
        // dây chuyền (cascade panic) làm crash giao diện người dùng, ta lấy lại dữ liệu bên trong
        // để khôi phục trạng thái an toàn.
        let mut active_guard = self.inner.active.lock().unwrap_or_else(|p| p.into_inner());
        if active_guard.is_some() {
            return Err("đang có việc chạy, đợi xong hoặc huỷ".to_string());
        }

        let report_guard = self.inner.report.lock().unwrap_or_else(|p| p.into_inner());
        if report_guard.is_some() {
            return Err("đang có việc chạy, đợi xong hoặc huỷ".to_string());
        }
        drop(report_guard);

        // Thu dọn luồng worker cũ đã hoàn thành từ lượt chạy trước để giải phóng tài nguyên luồng của hệ điều hành.
        // Luồng cũ đã hoàn thành vì cả active và report đều đã None.
        let old_worker = {
            let mut worker_guard = self.inner.worker.lock().unwrap_or_else(|p| p.into_inner());
            worker_guard.take()
        };
        if let Some(handle) = old_worker {
            if let Err(panic_payload) = handle.join() {
                let panic_msg = if let Some(s) = panic_payload.downcast_ref::<&str>() {
                    (*s).to_string()
                } else if let Some(s) = panic_payload.downcast_ref::<String>() {
                    s.clone()
                } else {
                    "Luồng worker gặp panic không xác định".to_string()
                };
                tracing::warn!(%panic_msg, "Luồng worker trước đó đã panic khi join");
            }
        }

        let id = self.inner.next_id.fetch_add(1, Ordering::SeqCst);

        // Đối với MergeImagesToPdf, mọi file đầu vào gộp thành 1 đầu ra nên tổng số mục là 1;
        // các thao tác khác xử lý độc lập từng file đầu vào.
        let total = match job.operation {
            Operation::MergeImagesToPdf => 1,
            _ => job.inputs.len(),
        };

        let cancel_token = CancelToken::new();
        let initial_snapshot = JobSnapshot {
            id,
            label: label.clone(),
            done: 0,
            total,
            state: JobState::Running,
        };

        *active_guard = Some(ActiveJob {
            id,
            cancel: cancel_token.clone(),
            snapshot: initial_snapshot,
        });

        // Nhả khoá active trước khi spawn luồng để tránh giữ khoá lâu không cần thiết
        drop(active_guard);

        let inner_clone = Arc::clone(&self.inner);
        let cancel_clone = cancel_token;
        let label_clone = label;

        let spawn_res = std::thread::Builder::new()
            .name(format!("morphix-job-{id}"))
            .spawn(move || {
                let on_progress = {
                    let inner = Arc::clone(&inner_clone);
                    let on_update = Arc::clone(&on_update);
                    move |progress: Progress| {
                        let snapshot = {
                            let mut guard =
                                inner.active.lock().unwrap_or_else(|p| p.into_inner());
                            if let Some(active) = guard.as_mut() {
                                active.snapshot.done = progress.done;
                                active.snapshot.total = progress.total;
                                active.snapshot.clone()
                            } else {
                                return;
                            }
                        };
                        // Tuyệt đối không giữ khoá Mutex khi gọi callback người dùng để tránh nguy cơ deadlock
                        on_update(snapshot);
                    }
                };

                let report = run_job(&job, &cancel_clone, &on_progress);

                let is_cancelled = cancel_clone.is_cancelled();
                let final_state = if is_cancelled {
                    JobState::Cancelled
                } else {
                    JobState::Finished {
                        ok: report.succeeded(),
                        failed: report.failed(),
                    }
                };

                let final_snapshot = {
                    let mut active_guard =
                        inner_clone.active.lock().unwrap_or_else(|p| p.into_inner());
                    let mut report_guard =
                        inner_clone.report.lock().unwrap_or_else(|p| p.into_inner());

                    *report_guard = Some((id, report));

                    let total = active_guard
                        .as_ref()
                        .map(|a| a.snapshot.total)
                        .unwrap_or(0);
                    let done = active_guard
                        .as_ref()
                        .map(|a| a.snapshot.done)
                        .unwrap_or(total);

                    *active_guard = None;

                    JobSnapshot {
                        id,
                        label: label_clone,
                        done,
                        total,
                        state: final_state,
                    }
                };

                // Gọi on_update đúng một lần cuối cùng với Finished hoặc Cancelled bên ngoài lock
                on_update(final_snapshot);
            });

        match spawn_res {
            Ok(handle) => {
                let mut worker_guard =
                    self.inner.worker.lock().unwrap_or_else(|p| p.into_inner());
                *worker_guard = Some(handle);
                Ok(id)
            }
            Err(err) => {
                // Khôi phục trạng thái active nếu hệ điều hành từ chối tạo luồng mới
                let mut guard = self.inner.active.lock().unwrap_or_else(|p| p.into_inner());
                *guard = None;
                Err(format!("Không thể khởi chạy luồng nền: {err}"))
            }
        }
    }

    /// Yêu cầu huỷ tác vụ đang chạy nếu định danh `id` khớp. Trả về `true` nếu huỷ thành công.
    pub fn cancel(&self, id: u64) -> bool {
        let guard = self.inner.active.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(active) = guard.as_ref() {
            if active.id == id {
                active.cancel.cancel();
                return true;
            }
        }
        false
    }

    /// Lấy ảnh chụp nhanh của tác vụ đang chạy nếu có.
    pub fn active(&self) -> Option<JobSnapshot> {
        let guard = self.inner.active.lock().unwrap_or_else(|p| p.into_inner());
        guard.as_ref().map(|active| active.snapshot.clone())
    }

    /// Lấy báo cáo chi tiết của tác vụ sau khi kết thúc.
    ///
    /// Thao tác này lấy một lần duy nhất (`take`); các lần gọi tiếp theo cho cùng `id` sẽ trả về `None`.
    pub fn take_report(&self, id: u64) -> Option<JobReport> {
        let mut guard = self.inner.report.lock().unwrap_or_else(|p| p.into_inner());
        if let Some((report_id, _)) = guard.as_ref() {
            if *report_id == id {
                return guard.take().map(|(_, report)| report);
            }
        }
        None
    }

    /// Dừng toàn diện trình chạy việc: huỷ tác vụ hiện tại nếu có, join worker luồng nền và dọn dẹp.
    ///
    /// Chuyển lỗi panic của luồng thành `Err(String)` để lớp trên có thể log lại.
    pub fn shutdown(&self) -> Result<(), String> {
        // 1. Kích hoạt cờ huỷ trên tác vụ đang chạy nếu có
        {
            let guard = self.inner.active.lock().unwrap_or_else(|p| p.into_inner());
            if let Some(active) = guard.as_ref() {
                active.cancel.cancel();
            }
        }

        // 2. Rút join handle khỏi mutex để thực hiện join mà không phong toả các luồng khác truy cập runner
        let handle = {
            let mut guard = self.inner.worker.lock().unwrap_or_else(|p| p.into_inner());
            guard.take()
        };

        // 3. Chờ worker hoàn tất và chuyển đổi lỗi panic thành Err có ngữ cảnh
        let result = if let Some(handle) = handle {
            handle.join().map_err(|panic_payload| {
                let msg = if let Some(s) = panic_payload.downcast_ref::<&str>() {
                    (*s).to_string()
                } else if let Some(s) = panic_payload.downcast_ref::<String>() {
                    s.clone()
                } else {
                    "Luồng worker gặp panic không xác định".to_string()
                };
                format!("Luồng xử lý gặp panic: {msg}")
            })
        } else {
            Ok(())
        };

        // 4. Xoá active để đảm bảo không còn tác vụ nào được xem là đang chạy
        {
            let mut guard = self.inner.active.lock().unwrap_or_else(|p| p.into_inner());
            *guard = None;
        }

        result
    }
}

/// Định dạng thông báo lỗi cho người dùng.
///
/// Nếu lỗi từ lõi đã kèm đường dẫn file (Io, Unsupported, Corrupt, NotSmaller), ta giữ nguyên thông báo của lõi.
/// Nếu không (ví dụ InvalidInput, Cancelled), ta bổ sung tên file đầu vào vào đầu thông báo.
pub fn describe_error(error: &MorphixError, input: &Path) -> String {
    match error {
        MorphixError::Io { .. }
        | MorphixError::Unsupported { .. }
        | MorphixError::Corrupt { .. }
        | MorphixError::NotSmaller { .. } => error.to_string(),
        _ => {
            let name = input
                .file_name()
                .map(|n| n.to_string_lossy())
                .unwrap_or_else(|| input.to_string_lossy());
            if name.is_empty() {
                error.to_string()
            } else {
                format!("{name}: {error}")
            }
        }
    }
}
