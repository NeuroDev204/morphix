use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use crate::error::{MorphixError, Result};

/// Cờ huỷ dùng chung giữa giao diện và bộ chạy việc. Các bản sao cùng trỏ vào một cờ, nên
/// nút Huỷ ở luồng giao diện dừng được việc đang chạy ở luồng khác.
#[derive(Clone, Default)]
pub struct CancelToken(Arc<AtomicBool>);

impl CancelToken {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
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
    fn clones_share_the_cancel_flag() {
        let token = CancelToken::new();
        let clone = token.clone();
        assert!(clone.check().is_ok());
        token.cancel();
        assert!(matches!(clone.check(), Err(MorphixError::Cancelled)));
    }
}
