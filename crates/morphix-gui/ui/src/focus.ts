// Quyết định ẩn Menu nhanh khi mất focus. Logic thuần, không DOM, để vitest chạy trong node.

// WHY 600 ms: khi yêu cầu đến từ Khay, hộp chọn file đóng rồi focus quay về Khay (luôn ở trên),
// nên Menu nhận blur ngay sau khi hiện. Bỏ qua blur trong khoảng này để Menu không tự ẩn mất.
export const BLUR_GRACE_MS: number = 600;

// WHY: job hoặc thông báo lỗi cần ở lại để người dùng xem kết quả.
export function shouldHideOnBlur(nowMs: number, shownAtMs: number | null, keepVisible: boolean): boolean {
  if (keepVisible) {
    return false;
  }
  if (shownAtMs === null) {
    return true;
  }
  return nowMs - shownAtMs >= BLUR_GRACE_MS;
}
