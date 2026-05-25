use crate::database::dao::PaginatedResult;
use crate::database::AccessKey;
use crate::error::AppError;
use crate::services::token_service;
use crate::AppState;
use tauri::{AppHandle, State};

#[tauri::command]
pub fn list_access_keys(state: State<'_, AppState>) -> Result<Vec<AccessKey>, AppError> {
    token_service::list_access_keys(&state.db)
}

#[tauri::command]
pub fn list_access_keys_paginated(
    state: State<'_, AppState>,
    page: i32,
    page_size: i32,
) -> Result<PaginatedResult<AccessKey>, AppError> {
    token_service::list_access_keys_paginated(&state.db, page, page_size)
}

#[tauri::command]
pub fn create_access_key(state: State<'_, AppState>, name: String) -> Result<AccessKey, AppError> {
    token_service::create_access_key(&state.db, &name)
}

#[tauri::command]
pub async fn delete_access_key(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
) -> Result<(), AppError> {
    token_service::delete_access_key(&state.db, &id, Some(&app))
}

#[tauri::command]
pub async fn toggle_access_key(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    enabled: bool,
) -> Result<(), AppError> {
    token_service::toggle_access_key(&state.db, &id, enabled, Some(&app))
}

#[tauri::command]
pub async fn update_access_key_models(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    allowed_models: Option<Vec<String>>,
) -> Result<(), AppError> {
    token_service::update_access_key_models(&state.db, &id, allowed_models, Some(&app))
}
