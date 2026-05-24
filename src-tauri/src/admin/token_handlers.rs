use crate::admin::error::AdminError;
use crate::admin::state::AdminState;
use crate::database::dao::PaginatedResult;
use crate::database::AccessKey;
use crate::services::token_service;
use axum::extract::{Json, Path, Query, State};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct CreateTokenParams {
    pub name: String,
}

// ---------- Handlers -------------------------------------------------------

pub async fn list_tokens(
    State(state): State<AdminState>,
) -> Result<Json<Vec<AccessKey>>, AdminError> {
    let keys = token_service::list_access_keys(&state.db)?;
    Ok(Json(keys))
}

#[derive(Deserialize)]
pub struct TokenPageParams {
    pub page: Option<i32>,
    pub page_size: Option<i32>,
}

pub async fn list_tokens_paginated(
    State(state): State<AdminState>,
    Query(params): Query<TokenPageParams>,
) -> Result<Json<PaginatedResult<AccessKey>>, AdminError> {
    let keys = token_service::list_access_keys_paginated(
        &state.db,
        params.page.unwrap_or(1),
        params.page_size.unwrap_or(20),
    )?;
    Ok(Json(keys))
}

pub async fn create_token(
    State(state): State<AdminState>,
    Json(payload): Json<CreateTokenParams>,
) -> Result<Json<AccessKey>, AdminError> {
    let key = token_service::create_access_key(&state.db, &payload.name)?;
    Ok(Json(key))
}

pub async fn delete_token(
    State(state): State<AdminState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, AdminError> {
    token_service::delete_access_key(&state.db, &id, state.app_handle.as_ref())?;
    Ok(Json(serde_json::json!({"ok": true})))
}

pub async fn toggle_token(
    State(state): State<AdminState>,
    Path(id): Path<String>,
    Json(enabled): Json<bool>,
) -> Result<Json<serde_json::Value>, AdminError> {
    token_service::toggle_access_key(&state.db, &id, enabled, state.app_handle.as_ref())?;
    Ok(Json(serde_json::json!({"ok": true})))
}
