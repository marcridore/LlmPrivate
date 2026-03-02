use tauri::State;

use crate::backend::openclaw_server::{
    ChannelStatus, OpenClawStatus, QrResponse, SetupStatus, WaitResponse,
};
use crate::error::AppError;
use crate::state::AppState;

/// Check if Node.js and OpenClaw are installed.
#[tauri::command]
pub async fn openclaw_check_prerequisites(
    state: State<'_, AppState>,
) -> Result<SetupStatus, AppError> {
    Ok(state.openclaw_server.check_prerequisites().await)
}

/// Install Node.js (Windows: silent MSI install).
#[tauri::command]
pub async fn openclaw_install_node(
    state: State<'_, AppState>,
) -> Result<(), AppError> {
    state.openclaw_server.install_node().await
}

/// Install OpenClaw globally via npm.
#[tauri::command]
pub async fn openclaw_install(
    state: State<'_, AppState>,
) -> Result<(), AppError> {
    state.openclaw_server.install_openclaw().await
}

/// Start the OpenClaw gateway subprocess.
#[tauri::command]
pub async fn openclaw_start(
    state: State<'_, AppState>,
) -> Result<u16, AppError> {
    state.openclaw_server.start().await
}

/// Stop the OpenClaw gateway.
#[tauri::command]
pub async fn openclaw_stop(
    state: State<'_, AppState>,
) -> Result<(), AppError> {
    state.openclaw_server.stop().await;
    Ok(())
}

/// Get the current status of the OpenClaw gateway.
#[tauri::command]
pub async fn openclaw_status(
    state: State<'_, AppState>,
) -> Result<OpenClawStatus, AppError> {
    Ok(state.openclaw_server.status().await)
}

/// Configure the LLM provider for OpenClaw.
#[tauri::command]
pub async fn openclaw_configure_provider(
    state: State<'_, AppState>,
    provider: String,
    model: String,
    api_key: String,
) -> Result<(), AppError> {
    state
        .openclaw_server
        .configure_provider(&provider, &model, &api_key)
        .await
}

/// Start WhatsApp QR code login flow.
#[tauri::command]
pub async fn openclaw_whatsapp_start(
    state: State<'_, AppState>,
    force: bool,
) -> Result<QrResponse, AppError> {
    state.openclaw_server.whatsapp_login_start(force).await
}

/// Wait for the user to scan the WhatsApp QR code (long-poll).
#[tauri::command]
pub async fn openclaw_whatsapp_wait(
    state: State<'_, AppState>,
) -> Result<WaitResponse, AppError> {
    state.openclaw_server.whatsapp_login_wait().await
}

/// Log out of WhatsApp.
#[tauri::command]
pub async fn openclaw_whatsapp_logout(
    state: State<'_, AppState>,
) -> Result<(), AppError> {
    state.openclaw_server.whatsapp_logout().await
}

/// Get WhatsApp channel connection status.
#[tauri::command]
pub async fn openclaw_channel_status(
    state: State<'_, AppState>,
) -> Result<ChannelStatus, AppError> {
    state.openclaw_server.get_channel_status().await
}
