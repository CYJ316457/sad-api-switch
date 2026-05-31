use crate::admin::error::AdminError;
use crate::admin::state::AdminState;
use crate::commands::backup::{
    build_app_backup, emit_backup_changed_events, restore_data_tables, AppBackupPayload,
};
use axum::extract::State;
use axum::Json;

pub async fn export_backup(
    State(state): State<AdminState>,
) -> Result<Json<AppBackupPayload>, AdminError> {
    let runtime = state
        .runtime
        .as_ref()
        .ok_or_else(|| AdminError::Internal("Runtime state unavailable".to_string()))?;
    Ok(Json(build_app_backup(runtime).await?))
}

pub async fn import_backup(
    State(state): State<AdminState>,
    Json(payload): Json<AppBackupPayload>,
) -> Result<Json<serde_json::Value>, AdminError> {
    let runtime = state
        .runtime
        .as_ref()
        .ok_or_else(|| AdminError::Internal("Runtime state unavailable".to_string()))?;
    restore_data_tables(runtime, &payload)?;

    if let Some(app_handle) = state.app_handle.clone() {
        crate::commands::config::apply_settings_update(
            app_handle.clone(),
            runtime,
            payload.settings,
            true,
        )
        .await?;
        emit_backup_changed_events(&app_handle);
    } else {
        runtime.db.update_settings(&payload.settings)?;
        *runtime.settings.write().await = runtime.db.get_settings()?;
    }

    crate::state_version::bump();
    Ok(Json(serde_json::json!({ "ok": true })))
}
