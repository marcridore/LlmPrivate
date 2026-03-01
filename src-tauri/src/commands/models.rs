use tauri::ipc::Channel;
use tauri::State;

use crate::backend::types::*;
use crate::error::AppError;
use crate::models::recommended::RecommendedModel;
use crate::state::AppState;

#[tauri::command]
pub async fn list_local_models(
    state: State<'_, AppState>,
) -> Result<Vec<LocalModelEntry>, AppError> {
    state.model_manager.scan_local_models().await
}

#[tauri::command]
pub async fn load_model(
    state: State<'_, AppState>,
    model_path: String,
    params: Option<ModelLoadParams>,
) -> Result<ModelHandle, AppError> {
    let params = params.unwrap_or_default();
    let path = std::path::PathBuf::from(&model_path);

    let handle = state
        .model_manager
        .load_model(path.clone(), params.clone())
        .await?;

    // Check if this model has a companion mmproj file → start vision server
    if let Some(mmproj_path) = crate::backend::llama_cpp_backend::find_mmproj_file(&path) {
        tracing::info!(
            "Model has mmproj companion: {}. Starting vision server...",
            mmproj_path.display()
        );
        match state
            .vision_server
            .start(&path, &mmproj_path, params.n_gpu_layers)
            .await
        {
            Ok(port) => {
                tracing::info!("Vision server started on port {}", port);
                // Update model info to reflect vision capability
                let backends = state.backends.read().await;
                if let Some(backend) = backends.default_backend() {
                    if let Ok(mut info) = backend.get_model_info(handle) {
                        info.supports_vision = true;
                        info.mmproj_path = Some(mmproj_path);
                        // Note: we can't update info in-place on the backend,
                        // but the get_model_capabilities command will check the vision server
                    }
                }
            }
            Err(e) => {
                tracing::warn!("Failed to start vision server: {e}. Vision will be unavailable.");
            }
        }
    }

    Ok(handle)
}

#[tauri::command]
pub async fn unload_model(
    state: State<'_, AppState>,
    handle: ModelHandle,
) -> Result<(), AppError> {
    // Stop vision server if running
    state.vision_server.stop().await;
    state.model_manager.unload_model(handle).await
}

#[tauri::command]
pub async fn delete_model(
    state: State<'_, AppState>,
    model_path: String,
) -> Result<(), AppError> {
    state.model_manager.delete_model(model_path.into()).await
}

#[tauri::command]
pub fn get_recommended_models() -> Vec<RecommendedModel> {
    crate::models::recommended::get_recommended_models()
}

#[tauri::command]
pub async fn download_model(
    state: State<'_, AppState>,
    repo_id: String,
    filename: String,
    on_progress: Channel<DownloadProgress>,
) -> Result<String, AppError> {
    state
        .download_manager
        .download_model(&repo_id, &filename, on_progress)
        .await
}

#[tauri::command]
pub async fn cancel_download(
    state: State<'_, AppState>,
    filename: String,
) -> Result<(), AppError> {
    state.download_manager.cancel_download(&filename).await;
    Ok(())
}

#[tauri::command]
pub async fn get_active_downloads(
    state: State<'_, AppState>,
) -> Result<Vec<(String, String)>, AppError> {
    Ok(state.download_manager.active_downloads().await)
}
