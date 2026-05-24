//! Tauri commands for translation relay.

use crate::services::translation_service;
use crate::{AppError, AppState};
use tauri::State;

/// Translate text and store result in cache.
#[tauri::command]
pub async fn translate_and_relay(
    state: State<'_, AppState>,
    request: translation_service::TranslationRelayRequest,
) -> Result<crate::TranslationRelayPayload, AppError> {
    let payload = translation_service::translate_and_store(&state, request).await?;
    Ok(payload)
}

/// Get the latest translation relay result from cache.
#[tauri::command]
pub async fn get_translation_relay(
    state: State<'_, AppState>,
) -> Result<Option<crate::TranslationRelayPayload>, AppError> {
    let payload = translation_service::get_latest(&state).await;
    Ok(payload)
}
