//! Đổi URI `file:` mà Dolphin truyền qua `%U` thành đường dẫn. WHY url crate: nó tự giải mã `%XX` và giữ
//! Unicode đúng, còn tự cắt chuỗi dễ sai với `%23`, `+` hay ký tự nhiều byte.

use std::path::PathBuf;

use url::Url;

/// Mọi URI phải là `file:` cục bộ; một URI lỗi làm cả lô bị từ chối để không gửi nửa danh sách.
pub fn file_uris_to_paths(uris: &[String]) -> Result<Vec<PathBuf>, String> {
    if uris.is_empty() {
        return Err("danh sách URI rỗng".to_string());
    }
    uris.iter().map(|uri| file_uri_to_path(uri)).collect()
}

fn file_uri_to_path(uri: &str) -> Result<PathBuf, String> {
    let parsed = Url::parse(uri).map_err(|error| format!("URI không hợp lệ {uri}: {error}"))?;
    if parsed.scheme() != "file" {
        return Err(format!(
            "chỉ nhận URI file:, không nhận {}: {uri}",
            parsed.scheme()
        ));
    }
    // WHY lỗi chung: `to_file_path` chỉ trả `()` khi host không phải local, nên không có lý do cụ thể để báo thêm.
    parsed
        .to_file_path()
        .map_err(|()| format!("URI không trỏ tới file cục bộ: {uri}"))
}
