//! Translation relay service.
//!
//! Sends the source text to an LLM (via the same entry/channel/adapter
//! pipeline used by the "test chat" command) and writes both success
//! and failure results into the in-memory `AppState.translation_relay`
//! cache so the Web Admin can always display the latest attempt status.

use crate::proxy::protocol::get_adapter;
use crate::{AppError, AppState, TranslationRelayPayload};
use serde::Deserialize;
use serde_json::json;

/// Request payload for translation.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranslationRelayRequest {
    pub text: String,
    pub source_lang: Option<String>,
    pub target_lang: Option<String>,
    /// Entry ID used to route to a specific API entry for translation.
    /// When None, falls back to the first available entry in the database.
    pub entry_id: Option<String>,
}

// ── Translation helpers ───────────────────────────────────────────────────

fn build_system_prompt() -> String {
    "You are a translator. You output ONLY the translated text. \
     Do NOT add quotes, explanations, or any context beyond the \
     translated text itself."
        .into()
}

fn build_user_message(request: &TranslationRelayRequest) -> String {
    match (&request.source_lang, &request.target_lang) {
        (Some(src), Some(tgt)) => {
            format!("Translate from {src} to {tgt}:\n\n{}", request.text)
        }
        (None, Some(tgt)) => {
            format!("Translate to {tgt}:\n\n{}", request.text)
        }
        _ => {
            // source_lang but no target – treat as free-form translate
            format!("Translate:\n\n{}", request.text)
        }
    }
}

// ── LLM call ──────────────────────────────────────────────────────────────

/// Performs a single non-streaming chat completion call to an LLM entry.
/// Returns the content string on success, or a human-readable error string on failure.
async fn perform_llm_call(
    state: &AppState,
    entry_id: Option<&str>,
    user_text: &str,
) -> Result<String, String> {
    let db = &state.db;

    // Resolve entry: use the requested entry, otherwise fall back to first available
    let entries = db
        .get_entries_for_routing_all()
        .map_err(|e| format!("Failed to load API entries: {e}"))?;

    let entry = match entry_id {
        Some(id) => entries
            .iter()
            .find(|e| e.id == id)
            .ok_or_else(|| format!("Entry {id} not found in database"))?
            .clone(),
        None => {
            // No entry_id specified – use the first available entry
            let first = entries
                .first()
                .ok_or("No API entries configured in the system")?
                .clone();
            log::info!(
                "[translation] No entry_id specified, falling back to first entry: {}",
                first.id
            );
            first
        }
    };

    let channel = db
        .get_channel(&entry.channel_id)
        .map_err(|e| format!("Channel {} not found: {e}", entry.channel_id))?;

    let adapter = get_adapter(&channel.api_type);
    let upstream_model = entry.upstream_model();
    let url = adapter.build_chat_url(&channel.base_url, upstream_model);

    // Build request body in OpenAI chat format (the adapters will transform as needed)
    let mut upstream_body = json!({
        "model": upstream_model,
        "messages": [
            {"role": "system", "content": build_system_prompt()},
            {"role": "user",   "content": user_text}
        ],
        "stream": false,
    });
    adapter.transform_request(&mut upstream_body, upstream_model);

    let client = crate::http_client::channel_client(
        channel.use_system_proxy,
        std::time::Duration::from_secs(30),
    )
    .map_err(|e| format!("HTTP client: {e}"))?;
    let request = adapter
        .apply_auth(client.post(&url), &channel.api_key)
        .json(&upstream_body);

    let response = request
        .send()
        .await
        .map_err(|e| format!("LLM request failed: {e}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("LLM upstream error {status}: {body}"));
    }

    let json_body: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse LLM response: {e}"))?;

    let mut json_body = json_body;
    adapter.transform_response(&mut json_body);

    // Extract content from standard OpenAI response format
    let content = json_body
        .get("choices")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("message"))
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_str())
        .unwrap_or("")
        .to_string();

    if content.is_empty() {
        return Err("LLM returned empty translated text".into());
    }

    Ok(content)
}

// ── Public API ────────────────────────────────────────────────────────────

/// Translate source text and write the result (success or failure) into the
/// in-memory `AppState.translation_relay` cache.
///
/// Success path: resolves an LLM entry by `request.entry_id`, sends a
/// translation prompt, extracts the reply.
///
/// Failure path: catches any error thrown during resolution or LLM call
/// and writes `success=false` into the cache, so the Web Admin always
/// shows a stable latest status.
pub async fn translate_and_store(
    state: &AppState,
    request: TranslationRelayRequest,
) -> Result<TranslationRelayPayload, AppError> {
    let source_text = request.text.clone();
    let source_lang = request.source_lang.clone();
    let target_lang = request.target_lang.clone();
    let user_message = build_user_message(&request);

    let attempt = perform_llm_call(state, request.entry_id.as_deref(), &user_message).await;
    let now = chrono::Utc::now().timestamp_millis();

    let payload = match attempt {
        Ok(translated_text) => TranslationRelayPayload {
            source_text,
            translated_text,
            source_lang,
            target_lang,
            success: true,
            error: None,
            updated_at: now,
        },
        Err(err_msg) => {
            log::warn!("[translation] Attempt failed: {err_msg}");
            TranslationRelayPayload {
                source_text,
                translated_text: String::new(),
                source_lang,
                target_lang,
                success: false,
                error: Some(err_msg),
                updated_at: now,
            }
        }
    };

    // Write to in-memory cache (latest attempt always wins)
    let mut cache = state.translation_relay.write().await;
    *cache = Some(payload.clone());

    Ok(payload)
}

/// Read the latest translation relay result from cache.
pub async fn get_latest(state: &AppState) -> Option<TranslationRelayPayload> {
    state.translation_relay.read().await.clone()
}

// Re-export dead-code utilities kept for backward compatibility; they are no
// longer needed now that the cache is written directly inside
// `translate_and_store`, but removing them would break any existing callers.

#[allow(dead_code)]
pub async fn write_cache(state: &AppState, payload: TranslationRelayPayload) {
    let mut cache = state.translation_relay.write().await;
    *cache = Some(payload);
}

#[allow(dead_code)]
pub async fn read_cache(state: &AppState) -> Option<TranslationRelayPayload> {
    state.translation_relay.read().await.clone()
}
