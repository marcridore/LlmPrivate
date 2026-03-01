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
    state
        .model_manager
        .load_model(model_path.into(), params.unwrap_or_default())
        .await
}

#[tauri::command]
pub async fn unload_model(
    state: State<'_, AppState>,
    handle: ModelHandle,
) -> Result<(), AppError> {
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
