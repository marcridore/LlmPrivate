use tauri::Manager;
use tauri_plugin_notification::NotificationExt;

pub fn notify_download_complete(app: &tauri::AppHandle, model_name: &str) {
    app.notification()
        .builder()
        .title("Download Complete")
        .body(&format!("{} is ready to use", model_name))
        .show()
        .ok();
}

pub fn notify_update_available(app: &tauri::AppHandle, version: &str) {
    app.notification()
        .builder()
        .title("Update Available")
        .body(&format!(
            "LlmPrivate {} is available. Click to update.",
            version
        ))
        .show()
        .ok();
}

pub fn notify_generation_complete(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        if !window.is_focused().unwrap_or(true) {
            app.notification()
                .builder()
                .title("Generation Complete")
                .body("Your AI response is ready")
                .show()
                .ok();
        }
    }
}
