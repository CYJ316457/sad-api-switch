use crate::admin::{AdminMode, AdminStatus};
use crate::error::AppError;
use crate::AppState;
use tauri::State;

#[tauri::command]
pub async fn get_admin_status(state: State<'_, AppState>) -> Result<AdminStatus, AppError> {
    let settings = state.settings.read().await.clone();
    let admin_guard = state.admin.read().await;

    Ok(match admin_guard.as_ref() {
        Some(server) => server.get_status(),
        None => {
            let running = settings.web_admin_enabled
                && state.proxy.read().await.is_some()
                && matches!(crate::admin::admin_mode(&settings), AdminMode::Combined);
            AdminStatus {
                running,
                address: "127.0.0.1".to_string(),
                port: settings.web_admin_port,
            }
        }
    })
}
