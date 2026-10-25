use std::fs::OpenOptions;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::sync::{Mutex, Once};

use clap::{Parser, Subcommand};
use morphix_ipc::{send_request, socket_name, IpcServer, MenuRequest};
use state::{AppPaths, AppState};
use tauri::{AppHandle, Manager, RunEvent, WindowEvent};

#[cfg(target_os = "linux")]
pub mod capture;
pub mod commands;
pub mod history;
pub mod integration;
pub mod jobs;
pub mod menu_items;
pub mod settings;
pub mod state;
pub mod tray;
pub mod windows;

static INIT_LOGGING: Once = Once::new();

/// Trả về chuỗi phiên bản của crate (từ `Cargo.toml`).
pub fn app_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Khởi tạo `tracing_subscriber` đúng một lần trước Tauri.
///
/// Lấy thư mục dữ liệu từ `morphix_core::data_dir()`, tạo thư mục nếu cần và mở
/// nối thêm vào file `morphix.log`. Nếu không mở được file log thì ghi nguyên nhân
/// ra `stderr` và chuyển sang writer `stderr`, không panic.
pub fn init_logging() {
    INIT_LOGGING.call_once(|| {
        let dir = morphix_core::data_dir();
        let opened = std::fs::create_dir_all(&dir).and_then(|()| {
            OpenOptions::new()
                .create(true)
                .append(true)
                .open(dir.join("morphix.log"))
        });
        match opened {
            Ok(file) => {
                let _ = tracing_subscriber::fmt()
                    .with_writer(Mutex::new(file))
                    .with_ansi(false)
                    .try_init();
            }
            Err(error) => {
                eprintln!("cảnh báo: không ghi được log ở {}: {error}", dir.display());
                let _ = tracing_subscriber::fmt()
                    .with_writer(std::io::stderr)
                    .try_init();
            }
        }
    });
}

/// Cú pháp dòng lệnh của `morphix-gui`: không có lệnh thì mở app bình thường.
#[derive(Parser)]
#[command(
    name = "morphix-gui",
    version,
    about = "Morphix: Menu nhanh và khay hệ thống."
)]
struct GuiCli {
    /// Mở Khay; do `morphix tray` gọi. Được đọc trước khi argv bị coi là đường dẫn file.
    #[arg(long)]
    show_khay: bool,
    #[arg(long, conflicts_with = "show_khay")]
    capture_selection: bool,
    #[arg(long, conflicts_with = "capture_selection")]
    unsupported_selection: bool,
    #[command(subcommand)]
    command: Option<GuiCommand>,
}

#[derive(Subcommand)]
enum GuiCommand {
    /// Mở Menu nhanh cho các file đã cho.
    Menu {
        #[arg(required = true)]
        files: Vec<PathBuf>,
    },
}

/// Yêu cầu khởi động đã phân tích từ argv: có mở Khay không và các file của lệnh `menu`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartupRequest {
    pub show_khay: bool,
    pub capture_selection: bool,
    pub unsupported_selection: bool,
    pub paths: Vec<PathBuf>,
}

const UNSUPPORTED_SELECTION_MESSAGE: &str =
    "Morphix chỉ nhận file thường lưu trên máy. Hãy chọn file khác hoặc thêm file trực tiếp trong app.";

/// Phân tích `args` (kể cả argv[0]) thành yêu cầu khởi động. Lỗi cú pháp trả về để `main` in trợ giúp
/// hoặc để callback của thể hiện thứ hai tự log.
pub fn startup_from_args(args: Vec<String>) -> Result<StartupRequest, clap::Error> {
    let cli = GuiCli::try_parse_from(args)?;
    let paths = match cli.command {
        Some(GuiCommand::Menu { files }) => files,
        None => Vec::new(),
    };
    Ok(StartupRequest {
        show_khay: cli.show_khay,
        capture_selection: cli.capture_selection,
        unsupported_selection: cli.unsupported_selection,
        paths,
    })
}

fn lock_ipc_server<'a>(state: &'a AppState) -> std::sync::MutexGuard<'a, Option<IpcServer>> {
    // WHY poison recovery: slot chỉ giữ handle server, dữ liệu vẫn đúng dù luồng khác từng panic.
    state
        .ipc_server
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Quyết định có mở Menu nhanh hay bỏ lô khi app đang thoát.
/// WHY: khi thoát, luồng chính đang join luồng IPC; callback đọc `is_visible` sẽ chờ luồng chính và treo.
fn should_show_menu(exiting: bool) -> bool {
    !exiting
}

/// Mở Menu nhanh rồi log lỗi thay vì panic: một lô lỗi không được làm sập server IPC.
fn show_menu_for(app: &AppHandle, paths: Vec<PathBuf>, collecting: bool) {
    let state = app.state::<AppState>();
    if !should_show_menu(state.exiting.load(Ordering::SeqCst)) {
        tracing::warn!(
            files = paths.len(),
            collecting,
            "bỏ lô file vì Morphix đang thoát"
        );
        return;
    }
    let (route, expired) = {
        let mut wait = state.khay_wait_guard();
        // WHY: callback lô đang gom không được tiêu thụ yêu cầu chờ; Khay cần toàn bộ lô cuối.
        if wait.is_some() && collecting {
            return;
        }
        let now = std::time::Instant::now();
        let expired = wait.as_ref().is_some_and(|wait| now >= wait.deadline);
        (state::route_for_batch(&mut wait, now), expired)
    };
    let result = match route {
        state::BatchRoute::Khay => windows::open_khay_with_files(app, paths),
        state::BatchRoute::Wheel => windows::open_menu(app, paths, collecting),
    };
    if let Err(reason) = result {
        tracing::error!(%reason, collecting, "không mở được Menu nhanh");
    }
    // WHY: lô tới trễ có thể xoá wait ngay trước luồng timeout; nhánh này vẫn phải báo lỗi trong Khay.
    if expired {
        if let Err(reason) = windows::open_khay_with_error(
            app,
            "Không nhận được file nào trong thời gian chờ. Hãy chọn file rồi thử lại.".into(),
        ) {
            tracing::error!(%reason, "không hiện được lỗi hết hạn lấy file");
        }
    }
}

/// Thể hiện thứ hai chỉ chuyển yêu cầu cho thể hiện đang chạy: `--show-khay` mở/focus Khay có sẵn, còn file đi qua IPC.
/// WHY không tạo cửa sổ: Khay là một cửa sổ cố định, nên thể hiện thứ hai chỉ được phép đưa nó lên trước.
fn handle_second_instance(app: &AppHandle, args: Vec<String>, cwd: &str) -> Result<(), String> {
    let startup = startup_from_args(args).map_err(|error| error.to_string())?;
    if startup.capture_selection {
        capture_selection(app);
    }
    if startup.unsupported_selection {
        windows::open_khay_with_error(app, UNSUPPORTED_SELECTION_MESSAGE.into())?;
    } else if startup.show_khay {
        windows::open_khay(app)?;
    }
    forward_paths(startup.paths, cwd)
}

/// Chuyển đường dẫn của thể hiện thứ hai qua đúng socket IPC của thể hiện đang chạy.
/// WHY cwd: đường dẫn tương đối của thể hiện thứ hai chỉ có nghĩa theo thư mục làm việc của nó.
fn forward_paths(paths: Vec<PathBuf>, cwd: &str) -> Result<(), String> {
    if paths.is_empty() {
        return Ok(());
    }
    let absolute_paths = paths
        .into_iter()
        .map(|path| Path::new(cwd).join(path))
        .collect();
    send_request(
        &socket_name(),
        &MenuRequest {
            paths: absolute_paths,
        },
    )
    .map_err(|error| format!("không chuyển được file tới Morphix đang chạy: {error}"))
}

/// Dừng dịch vụ nền khi app thoát: việc trước (để ghi lịch sử), rồi server IPC.
fn shutdown_services(app: &AppHandle) {
    let state = app.state::<AppState>();
    // WHY first: đặt cờ trước mọi join để callback IPC đang chạy không gọi cửa sổ nữa.
    state.exiting.store(true, Ordering::SeqCst);
    if let Err(reason) = state.jobs.shutdown() {
        tracing::error!(%reason, "luồng việc bị panic khi thoát");
    }
    // WHY take: `IpcServer::shutdown` nhận quyền sở hữu, và khoá không được giữ trong lúc join.
    let server = lock_ipc_server(&state).take();
    if let Some(server) = server {
        if let Err(error) = server.shutdown() {
            tracing::error!(error = %error, "không dừng được server IPC");
        }
    }
}

/// Chuyển đường dẫn tương đối thành tuyệt đối theo thư mục làm việc của thể hiện hiện tại.
/// WHY: thể hiện đang chạy resolve đường dẫn theo cwd của nó, không phải cwd của lệnh `menu`.
fn absolute_paths(paths: Vec<PathBuf>) -> io::Result<Vec<PathBuf>> {
    paths.into_iter().map(std::path::absolute).collect()
}

/// Điểm khởi chạy chính của ứng dụng GUI Tauri.
///
/// `startup.paths` là file từ `morphix-gui menu`; nếu đã có thể hiện đang chạy thì gửi cho nó và thoát.
/// `startup.show_khay` mở Khay sau khi setup xong (đường dẫn `morphix tray`).
pub fn run(startup: StartupRequest) -> Result<(), String> {
    init_logging();
    let initial_paths = absolute_paths(startup.paths).map_err(|error| {
        tracing::error!(error = %error, "không xác định được đường dẫn file từ dòng lệnh");
        format!("không xác định được đường dẫn file: {error}")
    })?;
    let show_khay = startup.show_khay;
    let capture_selection_requested = startup.capture_selection;
    let unsupported_selection_requested = startup.unsupported_selection;
    let request = MenuRequest {
        paths: initial_paths,
    };
    if !request.paths.is_empty() {
        match send_request(&socket_name(), &request) {
            Ok(()) => return Ok(()),
            Err(error) => {
                tracing::warn!(error = %error, "chưa gửi được file tới Morphix đang chạy, khởi động thể hiện mới");
            }
        }
    }
    tauri::Builder::default()
        // WHY first: thể hiện thứ hai phải bị chặn trước khi setup kịp tạo socket trùng tên.
        .plugin(tauri_plugin_single_instance::init(|app, args, cwd| {
            if let Err(reason) = handle_second_instance(app, args, &cwd) {
                tracing::error!(%reason, "không xử lý được yêu cầu của thể hiện thứ hai");
            }
        }))
        // WHY: hộp chọn file của Khay và Settings gọi plugin-dialog qua IPC; thiếu đăng ký thì lệnh báo "plugin dialog not found".
        .plugin(tauri_plugin_dialog::init())
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                if windows::close_hides_window(window.label()) {
                    api.prevent_close();
                    if let Err(error) = window.hide() {
                        tracing::error!(error = %error, label = window.label(), "không ẩn được cửa sổ khi đóng: {error}");
                    }
                }
            }
        })
        .manage(AppState::load(AppPaths::from_data_dir(
            &morphix_core::data_dir(),
        )))
        .setup(move |app| {
            let handle = app.handle().clone();
            // WHY log-only: khay hệ thống có thể không có trên một số môi trường Linux; Morphix vẫn chạy được qua Menu nhanh.
            if let Err(error) = tray::build_tray(&handle) {
                tracing::error!(error = %error, "không tạo được khay hệ thống, Morphix chạy không có khay: {error}");
            }
            let pending_handle = handle.clone();
            // WHY before any window: yêu cầu đến từ CLI phải có nơi nhận ngay khi cửa sổ hiện.
            let server = IpcServer::start(
                &socket_name(),
                move |paths| show_menu_for(&pending_handle, paths, true),
                move |paths| show_menu_for(&handle, paths, false),
            )
            .inspect_err(|error| {
                tracing::error!(error = %error, "không mở được socket IPC của Morphix");
            })?;
            *lock_ipc_server(&app.state::<AppState>()) = Some(server);
            if !request.paths.is_empty() {
                // WHY via socket: đi cùng luồng gom lô như mọi yêu cầu khác, nên không có đường riêng.
                send_request(&socket_name(), &request).inspect_err(
                    |error| tracing::error!(error = %error, "không gửi được file vào Menu nhanh"),
                )?;
            }
            if capture_selection_requested { capture_selection(app.handle()); }
            if unsupported_selection_requested {
                if let Err(reason) = windows::open_khay_with_error(
                    app.handle(),
                    UNSUPPORTED_SELECTION_MESSAGE.into(),
                ) {
                    tracing::error!(%reason, "không mở được Khay cho lựa chọn không hỗ trợ");
                }
            } else if show_khay {
                // WHY sau setup: cửa sổ cấu hình đã được Tauri tạo trước hook này; Khay là đường mở phụ nên lỗi chỉ được log.
                if let Err(reason) = windows::open_khay(app.handle()) {
                    tracing::error!(%reason, "không mở được Khay khi khởi động");
                }
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::khay_selection,
            commands::clear_khay_selection,
            commands::menu_payload,
            commands::run_action,
            commands::cancel_job,
            commands::active_job,
            commands::pending_request,
            commands::discard_pending,
            commands::submit_menu_request,
            commands::recent_history,
            commands::open_output_directory,
            commands::clear_history,
            commands::get_settings,
            commands::save_settings,
            commands::open_settings,
            commands::quit,
            integration::integration_status,
        ])
        .build(tauri::generate_context!())
        .expect("error while building morphix application")
        .run(|app, event| {
            if let RunEvent::Exit = event {
                shutdown_services(app);
            }
        });
    Ok(())
}

fn capture_selection(app: &AppHandle) {
    #[cfg(target_os = "linux")]
    capture::run(app);
    #[cfg(not(target_os = "linux"))]
    if let Err(reason) =
        windows::open_khay_with_error(app, "Lấy file đang chọn hiện chỉ hỗ trợ Linux X11.".into())
    {
        tracing::error!(%reason, "không mở được Khay");
    }
}

#[cfg(test)]
mod tests {
    use super::should_show_menu;

    #[test]
    fn drops_menu_requests_once_exit_has_started() {
        assert!(should_show_menu(false));
        assert!(!should_show_menu(true));
    }
}
