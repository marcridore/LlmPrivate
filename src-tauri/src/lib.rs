mod api;
mod backend;
mod commands;
mod db;
mod documents;
mod error;
mod models;
#[allow(dead_code)]
mod notifications;
mod resource;
mod state;
mod tray;

use std::sync::Arc;
use tauri::Manager;
use tokio::sync::RwLock;

use crate::backend::llama_cpp_backend::LlamaCppBackend;
use crate::backend::vision_server::VisionServer;
use crate::backend::BackendRegistry;
use crate::state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // On Windows, C/C++ code (ggml, CUDA, llama.cpp) writing to stderr can trigger
    // a CRT debug assertion: "_osfile(fh) & FOPEN" at osfinfo.cpp when stderr
    // is not connected to a valid handle. We fix this with multiple strategies:
    #[cfg(windows)]
    unsafe {
        // ── Strategy 1: Suppress CRT debug assertion dialogs ──
        // Dynamically find _CrtSetReportMode in debug CRT (ucrtbased.dll)
        // to disable the dialog boxes. In release CRT this is a no-op.
        type HMODULE = *mut core::ffi::c_void;
        type FARPROC = *mut core::ffi::c_void;
        extern "system" {
            fn GetModuleHandleA(name: *const u8) -> HMODULE;
            fn GetProcAddress(module: HMODULE, name: *const u8) -> FARPROC;
        }
        let ucrt = GetModuleHandleA(b"ucrtbased.dll\0".as_ptr());
        if !ucrt.is_null() {
            let proc = GetProcAddress(ucrt, b"_CrtSetReportMode\0".as_ptr());
            if !proc.is_null() {
                type Fn = unsafe extern "C" fn(i32, i32) -> i32;
                let set_mode: Fn = core::mem::transmute(proc);
                set_mode(0, 0); // _CRT_WARN → suppress
                set_mode(1, 0); // _CRT_ERROR → suppress
                set_mode(2, 0); // _CRT_ASSERT → suppress
            }
        }

        // ── Strategy 2: Set invalid parameter handler to a no-op ──
        // Prevents dialog boxes from CRT validation failures (_VALIDATE_RETURN).
        extern "C" {
            fn _set_invalid_parameter_handler(
                handler: Option<
                    unsafe extern "C" fn(
                        *const u16, *const u16, *const u16, u32, usize,
                    ),
                >,
            ) -> Option<
                unsafe extern "C" fn(*const u16, *const u16, *const u16, u32, usize),
            >;
        }
        unsafe extern "C" fn silent_handler(
            _: *const u16, _: *const u16, _: *const u16, _: u32, _: usize,
        ) {
        }
        _set_invalid_parameter_handler(Some(silent_handler));

        // ── Strategy 3: Redirect stdout/stderr to NUL via Win32 API ──
        // Uses CreateFileA (bypasses CRT) → SetStdHandle (Win32 layer)
        // → _open_osfhandle + _dup2 (CRT fd layer).
        type HANDLE = *mut core::ffi::c_void;
        extern "system" {
            fn CreateFileA(
                name: *const u8,
                access: u32,
                share: u32,
                sa: *const core::ffi::c_void,
                disp: u32,
                flags: u32,
                template: HANDLE,
            ) -> HANDLE;
            fn SetStdHandle(id: u32, handle: HANDLE) -> i32;
        }
        extern "C" {
            fn _open_osfhandle(handle: isize, flags: i32) -> i32;
            fn _dup2(fd1: i32, fd2: i32) -> i32;
        }

        const GENERIC_WRITE: u32 = 0x40000000;
        const SHARE_RW: u32 = 0x3; // FILE_SHARE_READ | FILE_SHARE_WRITE
        const OPEN_EXISTING: u32 = 3;
        const INVALID: HANDLE = -1isize as HANDLE;

        // Redirect stderr (fd 2) to NUL
        let h_err = CreateFileA(
            b"NUL\0".as_ptr(),
            GENERIC_WRITE, SHARE_RW,
            core::ptr::null(), OPEN_EXISTING, 0, core::ptr::null_mut(),
        );
        if h_err != INVALID {
            SetStdHandle((-12i32) as u32, h_err); // STD_ERROR_HANDLE
            let fd = _open_osfhandle(h_err as isize, 0);
            if fd >= 0 {
                _dup2(fd, 2);
            }
        }

        // Redirect stdout (fd 1) to NUL
        let h_out = CreateFileA(
            b"NUL\0".as_ptr(),
            GENERIC_WRITE, SHARE_RW,
            core::ptr::null(), OPEN_EXISTING, 0, core::ptr::null_mut(),
        );
        if h_out != INVALID {
            SetStdHandle((-11i32) as u32, h_out); // STD_OUTPUT_HANDLE
            let fd2 = _open_osfhandle(h_out as isize, 0);
            if fd2 >= 0 {
                _dup2(fd2, 1);
            }
        }
    }

    // Write logs to a file (since stderr is redirected to NUL on Windows
    // to suppress CRT assertion dialogs from llama.cpp/ggml).
    {
        let log_dir = if cfg!(windows) {
            std::env::var("APPDATA")
                .map(|a| std::path::PathBuf::from(a).join("com.llmprivate.app"))
                .unwrap_or_else(|_| std::path::PathBuf::from("."))
        } else {
            std::path::PathBuf::from(".")
        };
        std::fs::create_dir_all(&log_dir).ok();
        let log_path = log_dir.join("llmprivate.log");
        if let Ok(file) = std::fs::File::create(&log_path) {
            tracing_subscriber::fmt()
                .with_writer(std::sync::Mutex::new(file))
                .with_ansi(false)
                .with_max_level(tracing::Level::INFO)
                .init();
            tracing::info!("Logging to: {}", log_path.display());
        } else {
            tracing_subscriber::fmt::init();
        }
    }

    tauri::Builder::default()
        // .plugin(tauri_plugin_updater::Builder::new().build()) // Enable in Phase 4
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .setup(|app| {
            let data_dir = app
                .path()
                .app_data_dir()
                .expect("Failed to get app data dir");
            let models_dir = data_dir.join("models");

            let app_state = initialize_app_state(data_dir, models_dir);
            app.manage(app_state);

            tray::setup_tray(app)?;

            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { .. } = event {
                // Exit the entire app when the main window is closed
                // (otherwise the system tray keeps the process alive)
                window.app_handle().exit(0);
            }
        })
        .invoke_handler(tauri::generate_handler![
            // Chat
            commands::chat::send_message,
            commands::chat::stop_generation,
            commands::chat::get_conversations,
            commands::chat::get_messages,
            commands::chat::create_conversation,
            commands::chat::save_user_message,
            commands::chat::cleanup_empty_conversations,
            commands::chat::delete_conversation,
            // Models
            commands::models::list_local_models,
            commands::models::load_model,
            commands::models::unload_model,
            commands::models::delete_model,
            commands::models::get_recommended_models,
            commands::models::download_model,
            commands::models::cancel_download,
            commands::models::get_active_downloads,
            // System
            commands::system::get_backend_capabilities,
            commands::system::get_system_resources,
            commands::system::get_gpu_info,
            commands::system::get_model_recommendation,
            commands::system::get_model_capabilities,
            commands::system::save_clipboard_image,
            // Documents
            commands::documents::create_doc_folder,
            commands::documents::rename_doc_folder,
            commands::documents::delete_doc_folder,
            commands::documents::move_doc_folder,
            commands::documents::get_doc_folder_tree,
            commands::documents::add_document,
            commands::documents::delete_document,
            commands::documents::get_document,
            commands::documents::get_documents_in_folder,
            commands::documents::search_document_chunks,
            commands::documents::chat_with_documents,
            commands::documents::create_doc_chat_session,
            commands::documents::list_doc_chat_sessions,
            commands::documents::get_doc_chat_document_ids,
            commands::documents::toggle_doc_chat_pin,
            // Settings
            commands::settings::get_settings,
            commands::settings::update_settings,
        ])
        .run(tauri::generate_context!())
        .expect("error while running LlmPrivate");
}

fn initialize_app_state(
    data_dir: std::path::PathBuf,
    models_dir: std::path::PathBuf,
) -> AppState {
    // Initialize the real llama.cpp backend
    let llama_backend = Arc::new(
        LlamaCppBackend::new().expect("Failed to initialize llama.cpp backend"),
    );

    // Register backends
    let mut registry = BackendRegistry::new();
    registry.register(llama_backend);
    let backends = Arc::new(RwLock::new(registry));

    // Initialize database
    let db = Arc::new(
        db::connection::Database::new(data_dir).expect("Failed to initialize database"),
    );

    // Initialize resource monitor
    let resource_monitor = Arc::new(resource::monitor::SystemResourceMonitor::new());

    // Initialize download manager
    let download_manager = Arc::new(models::downloader::DownloadManager::new(
        models_dir.clone(),
    ));

    // Initialize model manager
    let model_manager = Arc::new(models::manager::ModelManager::new(
        backends.clone(),
        models_dir,
    ));

    AppState {
        backends,
        model_manager,
        download_manager,
        db,
        resource_monitor,
        api_server_running: Arc::new(RwLock::new(false)),
        vision_server: Arc::new(VisionServer::new()),
    }
}
