use std::sync::Arc;
use tokio::sync::RwLock;

use crate::backend::BackendRegistry;
use crate::backend::openclaw_server::OpenClawServer;
use crate::backend::vision_server::VisionServer;
use crate::db::connection::Database;
use crate::models::downloader::DownloadManager;
use crate::models::manager::ModelManager;
use crate::resource::monitor::SystemResourceMonitor;

pub struct AppState {
    pub backends: Arc<RwLock<BackendRegistry>>,
    pub model_manager: Arc<ModelManager>,
    pub download_manager: Arc<DownloadManager>,
    pub db: Arc<Database>,
    pub resource_monitor: Arc<SystemResourceMonitor>,
    pub api_server_running: Arc<RwLock<bool>>,
    pub vision_server: Arc<VisionServer>,
    pub openclaw_server: Arc<OpenClawServer>,
}
