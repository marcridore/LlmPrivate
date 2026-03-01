use std::collections::HashMap;

use tauri::State;

use crate::error::AppError;
use crate::state::AppState;

#[tauri::command]
pub async fn get_settings(
    state: State<'_, AppState>,
    keys: Vec<String>,
) -> Result<HashMap<String, String>, AppError> {
    let mut result = HashMap::new();
    for key in keys {
        if let Some(value) = state.db.get_setting(&key)? {
            result.insert(key, value);
        }
    }
    Ok(result)
}

#[tauri::command]
pub async fn update_settings(
    state: State<'_, AppState>,
    settings: HashMap<String, String>,
) -> Result<(), AppError> {
    for (key, value) in settings {
        state.db.set_setting(&key, &value)?;
    }
    Ok(())
}
