#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

fn main() -> Result<(), String> {
    // WHY exit on clap error: `--help` và lỗi cú pháp phải in ra và thoát trước khi Tauri khởi động.
    let startup = morphix_gui::startup_from_args(std::env::args().collect())
        .unwrap_or_else(|error| error.exit());
    morphix_gui::run(startup)
}
