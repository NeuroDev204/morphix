/// Preset chất lượng cho mọi bộ mã hoá có mất mát. Người dùng chọn theo ý định (nhỏ, vừa, đẹp),
/// không phải theo con số của từng định dạng.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Quality {
    Small,
    #[default]
    Balanced,
    High,
}

impl Quality {
    pub fn jpeg(self) -> u8 {
        match self {
            Quality::Small => 70,
            Quality::Balanced => 85,
            Quality::High => 95,
        }
    }

    pub fn webp(self) -> f32 {
        f32::from(self.jpeg())
    }

    /// AVIF nén hiệu quả hơn JPEG nên cùng cảm nhận chất lượng cần số thấp hơn.
    pub fn avif(self) -> u8 {
        match self {
            Quality::Small => 55,
            Quality::Balanced => 70,
            Quality::High => 85,
        }
    }
}
