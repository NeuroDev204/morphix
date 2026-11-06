#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

fn main() -> Result<(), String> {
    // WHY: Explorer khởi động file này với -Embedding để phục vụ menu Windows 11. Đây là exe kiểu cửa sổ nên
    // không mở terminal; chỉ phục vụ COM, không mở cửa sổ Tauri.
    #[cfg(windows)]
    if std::env::args().any(|arg| arg == "-Embedding" || arg == "/Embedding") {
        return morphix_shell::serve_command().map_err(|error| error.to_string());
    }
    // WHY exit on clap error: `--help` và lỗi cú pháp phải in ra và thoát trước khi Tauri khởi động.
    let startup = morphix_gui::startup_from_args(std::env::args().collect())
        .unwrap_or_else(|error| error.exit());
    morphix_gui::run(startup)
}
