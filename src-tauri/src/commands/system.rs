use serde::Serialize;
use tauri::{AppHandle, Manager, State};

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

#[derive(Serialize)]
pub struct ModelCapabilities {
    pub supports_vision: bool,
}

#[tauri::command]
pub async fn get_model_capabilities(
    state: State<'_, AppState>,
    model_handle: u64,
) -> Result<ModelCapabilities, AppError> {
    tracing::info!("get_model_capabilities called for handle {}", model_handle);

    // Vision is supported if the vision server is running
    let vision_server_running = state.vision_server.is_running().await;

    let backends = state.backends.read().await;
    let backend = backends.default_backend().ok_or(AppError::NoBackend)?;
    let info = backend.get_model_info(model_handle)?;

    let supports_vision = info.supports_vision || vision_server_running;

    tracing::info!(
        "get_model_capabilities: handle={} name={} supports_vision={} (server={})",
        model_handle, info.name, supports_vision, vision_server_running
    );
    Ok(ModelCapabilities {
        supports_vision,
    })
}

/// Save clipboard image data (base64-encoded) to a temp file and return the file path.
/// This is needed because clipboard-pasted images exist only in the browser's memory;
/// the vision backend needs a file path to read from.
#[tauri::command]
pub async fn save_clipboard_image(
    app: AppHandle,
    data: String,
    extension: String,
) -> Result<String, AppError> {
    let app_data = app
        .path()
        .app_data_dir()
        .map_err(|e| AppError::Io(format!("Cannot resolve app data dir: {e}")))?;
    let temp_dir = app_data.join("temp");
    std::fs::create_dir_all(&temp_dir)?;

    // Sanitize extension
    let ext = extension
        .trim_start_matches('.')
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect::<String>();
    let ext = if ext.is_empty() { "png".to_string() } else { ext };

    let filename = format!("clipboard_{}.{}", uuid::Uuid::new_v4(), ext);
    let path = temp_dir.join(&filename);

    // Decode base64
    let bytes = base64_decode(&data)?;

    std::fs::write(&path, &bytes)?;
    tracing::info!(
        "Saved clipboard image: {} ({} bytes)",
        path.display(),
        bytes.len()
    );

    Ok(path.to_string_lossy().to_string())
}

/// Decode base64 (standard alphabet with padding) to bytes.
fn base64_decode(input: &str) -> Result<Vec<u8>, AppError> {
    const DECODE: [u8; 256] = {
        let mut table = [255u8; 256];
        let chars = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut i = 0;
        while i < 64 {
            table[chars[i] as usize] = i as u8;
            i += 1;
        }
        table
    };

    let input = input.as_bytes();
    let mut output = Vec::with_capacity(input.len() * 3 / 4);
    let mut buf: u32 = 0;
    let mut bits: u32 = 0;

    for &b in input {
        if b == b'=' || b == b'\n' || b == b'\r' || b == b' ' {
            continue;
        }
        let val = DECODE[b as usize];
        if val == 255 {
            return Err(AppError::Io(format!(
                "Invalid base64 character: {}",
                b as char
            )));
        }
        buf = (buf << 6) | val as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            output.push((buf >> bits) as u8);
            buf &= (1 << bits) - 1;
        }
    }

    Ok(output)
}
