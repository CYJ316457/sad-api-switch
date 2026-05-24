//! Admin HTTP handler for translation relay.

use crate::admin::error::AdminError;
use crate::admin::state::AdminState;
use crate::services::translation_service;
use axum::extract::State;
use axum::Json;
use serde::Serialize;

/// Response for GET /admin/translation-relay.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TranslationRelayResponse {
    pub latest: Option<crate::TranslationRelayPayload>,
}

/// GET /admin/translation-relay - return the latest cached translation payload.
pub async fn get_translation_relay(
    State(state): State<AdminState>,
) -> Result<Json<TranslationRelayResponse>, AdminError> {
    // Use the same AppState cache via AdminState.runtime
    let runtime = state.runtime.as_ref().ok_or_else(|| {
        AdminError::Internal(
            "AdminState runtime not initialized; cannot access translation relay".to_string(),
        )
    })?;

    let latest = translation_service::get_latest(runtime).await;

    Ok(Json(TranslationRelayResponse { latest }))
}
