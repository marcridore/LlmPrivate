use serde::Serialize;
use tauri::State;

use crate::error::AppError;
use crate::resource::types::*;
use crate::state::AppState;

#[derive(Serialize)]
pub struct BackendCapabilities {
    pub gpu_compiled: bool,
    pub gpu_backend: Option<String>,
}

#[tauri::command]
pub fn get_backend_capabilities() -> BackendCapabilities {
    let (gpu_compiled, gpu_backend) = if cfg!(feature = "cuda") {
        (true, Some("cuda".to_string()))
    } else if cfg!(feature = "vulkan") {
        (true, Some("vulkan".to_string()))
    } else {
        (false, None)
    };

    BackendCapabilities {
        gpu_compiled,
        gpu_backend,
    }
}

#[tauri::command]
pub async fn get_system_resources(
    state: State<'_, AppState>,
) -> Result<ResourceSnapshot, AppError> {
    state.resource_monitor.snapshot()
}

#[tauri::command]
pub async fn get_gpu_info(state: State<'_, AppState>) -> Result<Vec<GpuInfo>, AppError> {
    state.resource_monitor.gpu_info()
}

#[tauri::command]
pub async fn get_model_recommendation(
    state: State<'_, AppState>,
    model_size_bytes: u64,
    model_quant: String,
) -> Result<ModelLoadRecommendation, AppError> {
    state
        .resource_monitor
        .recommend_params(model_size_bytes, &model_quant)
}
