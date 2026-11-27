use std::sync::{
    atomic::{AtomicBool, AtomicU32, Ordering},
    Arc,
};

use crate::error::{MorphixError, Result};

/// Cờ huỷ dùng chung giữa giao diện và bộ chạy việc. Các bản sao cùng trỏ vào một cờ, nên
/// nút Huỷ ở luồng giao diện dừng được việc đang chạy ở luồng khác.
#[derive(Clone, Default)]
pub struct CancelToken(Arc<AtomicBool>, Arc<AtomicU32>);

impl CancelToken {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }

    /// Tiến trình của mục đang chạy (0.0..=1.0), do ffmpeg báo. WHY nằm trong token: token đã đi tới mọi nơi chạy ffmpeg
    /// và được chia sẻ với giao diện, nên không phải đổi chữ ký của cả chuỗi hàm.
    pub fn set_fraction(&self, fraction: f32) {
        self.1.store((fraction.clamp(0.0, 1.0) * 1000.0) as u32, Ordering::Relaxed);
    }

    pub fn fraction(&self) -> f32 {
        self.1.load(Ordering::Relaxed) as f32 / 1000.0
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }

    /// Dùng giữa các bước dài: `cancel.check()?` thoát sớm bằng lỗi `Cancelled`.
    pub fn check(&self) -> Result<()> {
        if self.is_cancelled() {
            Err(MorphixError::Cancelled)
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::MorphixError;

    #[test]
    fn fraction_is_shared_and_clamped() {
        let token = CancelToken::new();
        let clone = token.clone();
        assert_eq!(clone.fraction(), 0.0);
        token.set_fraction(0.25);
        assert_eq!(clone.fraction(), 0.25);
        token.set_fraction(7.0);
        assert_eq!(clone.fraction(), 1.0);
    }

    #[test]
    fn clones_share_the_cancel_flag() {
        let token = CancelToken::new();
        let clone = token.clone();
        assert!(clone.check().is_ok());
        token.cancel();
        assert!(matches!(clone.check(), Err(MorphixError::Cancelled)));
    }
}
