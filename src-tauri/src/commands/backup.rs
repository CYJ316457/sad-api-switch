use crate::commands::config::apply_settings_update;
use crate::database::{lock_conn, AccessKey, ApiEntry, AppSettings, Channel};
use crate::error::AppError;
use crate::AppState;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppBackupPayload {
    pub version: i32,
    pub exported_at: String,
    pub settings: AppSettings,
    pub channels: Vec<Channel>,
    pub api_entries: Vec<ApiEntry>,
    pub access_keys: Vec<AccessKey>,
}

#[tauri::command]
pub async fn export_app_backup(state: State<'_, AppState>) -> Result<AppBackupPayload, AppError> {
    build_app_backup(&state).await
}

#[tauri::command]
pub async fn import_app_backup(
    app: AppHandle,
    state: State<'_, AppState>,
    payload: AppBackupPayload,
) -> Result<(), AppError> {
    restore_data_tables(&state, &payload)?;
    apply_settings_update(app.clone(), &state, payload.settings, true).await?;
    emit_backup_changed_events(&app);
    crate::state_version::bump();
    Ok(())
}

pub async fn build_app_backup(state: &AppState) -> Result<AppBackupPayload, AppError> {
    Ok(AppBackupPayload {
        version: 1,
        exported_at: chrono::Utc::now().to_rfc3339(),
        settings: state.settings.read().await.clone(),
        channels: state.db.list_channels()?,
        api_entries: state.db.list_entries()?,
        access_keys: state.db.list_access_keys()?,
    })
}

pub fn restore_data_tables(state: &AppState, payload: &AppBackupPayload) -> Result<(), AppError> {
    let mut conn = lock_conn!(state.db.conn);
    let tx = conn.transaction()?;

    tx.execute("DELETE FROM api_entries", [])?;
    tx.execute("DELETE FROM channels", [])?;
    tx.execute("DELETE FROM access_keys", [])?;

    for channel in &payload.channels {
        let available_models = serde_json::to_string(&channel.available_models)
            .map_err(|err| AppError::Internal(err.to_string()))?;
        let selected_models = serde_json::to_string(&channel.selected_models)
            .map_err(|err| AppError::Internal(err.to_string()))?;
        tx.execute(
            "INSERT OR REPLACE INTO channels (
                id, name, api_type, base_url, api_key, available_models, selected_models,
                enabled, use_system_proxy, last_fetch_at, notes, response_ms, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            rusqlite::params![
                channel.id,
                channel.name,
                channel.api_type,
                channel.base_url,
                channel.api_key,
                available_models,
                selected_models,
                channel.enabled as i32,
                channel.use_system_proxy as i32,
                channel.last_fetch_at,
                channel.notes,
                channel.response_ms,
                channel.created_at,
                channel.updated_at,
            ],
        )?;
    }

    for entry in &payload.api_entries {
        if !payload.channels.iter().any(|channel| channel.id == entry.channel_id) {
            continue;
        }
        tx.execute(
            "INSERT OR REPLACE INTO api_entries (
                id, channel_id, model, upstream_model, display_name, sort_index, enabled,
                locked, cooldown_until, response_ms, provider_logo, release_date,
                model_meta_zh, model_meta_en, group_name, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)",
            rusqlite::params![
                entry.id,
                entry.channel_id,
                entry.model,
                entry.upstream_model.clone().unwrap_or_else(|| entry.model.clone()),
                entry.display_name,
                entry.sort_index,
                entry.enabled as i32,
                entry.locked as i32,
                entry.cooldown_until,
                entry.response_ms.clone().unwrap_or_default(),
                entry.provider_logo.clone().unwrap_or_default(),
                entry.release_date.clone().unwrap_or_default(),
                entry.model_meta_zh.clone().unwrap_or_default(),
                entry.model_meta_en.clone().unwrap_or_default(),
                entry.group_name.clone().unwrap_or_else(|| "auto".to_string()),
                entry.created_at,
                entry.updated_at,
            ],
        )?;
    }

    for access_key in &payload.access_keys {
        let allowed_models = access_key
            .allowed_models
            .as_ref()
            .map(serde_json::to_string)
            .transpose()
            .map_err(|err| AppError::Internal(err.to_string()))?;
        tx.execute(
            "INSERT OR REPLACE INTO access_keys (id, name, key, enabled, created_at, allowed_models)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![
                access_key.id,
                access_key.name,
                access_key.key,
                access_key.enabled as i32,
                access_key.created_at,
                allowed_models,
            ],
        )?;
    }

    tx.commit()?;
    Ok(())
}

pub fn emit_backup_changed_events(app: &AppHandle) {
    let _ = app.emit("channels-changed", ());
    let _ = app.emit("entries-changed", ());
    let _ = app.emit("tokens-changed", ());
    let _ = app.emit("settings-changed", ());
}
