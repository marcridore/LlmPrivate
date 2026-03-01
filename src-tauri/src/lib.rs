mod api;
mod backend;
mod commands;
mod db;
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
use crate::backend::BackendRegistry;
use crate::state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt::init();

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
    }
}
