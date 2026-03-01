use tauri::{
    menu::{MenuBuilder, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
    Manager,
};

pub fn setup_tray(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let show = MenuItem::with_id(app, "show", "Show LlmPrivate", true, None::<&str>)?;
    let api_status =
        MenuItem::with_id(app, "api_status", "API Server: Stopped", false, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;

    let menu = MenuBuilder::new(app)
        .item(&show)
        .item(&api_status)
        .item(&separator)
        .item(&quit)
        .build()?;

    let _tray = TrayIconBuilder::with_id("main-tray")
        .tooltip("LlmPrivate - Local AI")
        .menu(&menu)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => {
                if let Some(window) = app.get_webview_window("main") {
                    window.show().unwrap_or_default();
                    window.set_focus().unwrap_or_default();
                }
            }
            "quit" => {
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let tauri::tray::TrayIconEvent::DoubleClick { .. } = event {
                let app = tray.app_handle();
                if let Some(window) = app.get_webview_window("main") {
                    window.show().unwrap_or_default();
                    window.set_focus().unwrap_or_default();
                }
            }
        })
        .build(app)?;

    Ok(())
}
