// Handler for test chat via admin API
use crate::admin::error::AdminError;
use crate::admin::state::AdminState;
use crate::proxy::protocol::get_adapter;
use crate::services::log_service::{insert_test_usage_log, TestUsageLogInput};
use axum::extract::{Json, State};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::time::Instant;

#[derive(Debug, Serialize, Deserialize)]
pub struct TestChatRequest {
    pub entry_id: String,
    pub messages: Vec<TestChatMessage>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TestChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Serialize)]
pub struct TestChatResponse {
    pub content: String,
    pub latency_ms: u64,
    pub usage: Option<TestChatUsage>,
}

#[derive(Debug, Serialize)]
pub struct TestChatUsage {
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
    pub total_tokens: i64,
}

pub async fn test_chat(
    State(state): State<AdminState>,
    Json(payload): Json<TestChatRequest>,
) -> Result<Json<TestChatResponse>, AdminError> {
    // Ensure runtime is available
    let runtime = state
        .runtime
        .as_ref()
        .ok_or_else(|| AdminError::BadRequest("Admin runtime not initialized".to_string()))?;
    let db = runtime.db.clone();

    // Get all entries for routing (including disabled)
    let entries = db.get_entries_for_routing_all()?;
    let entry = entries
        .iter()
        .find(|e| e.id == payload.entry_id)
        .ok_or_else(|| AdminError::NotFound(format!("Entry {} not found", payload.entry_id)))?
        .clone();

    // Get channel info
    let channel = db.get_channel(&entry.channel_id)?;

    // Get protocol adapter
    let adapter = get_adapter(&channel.api_type);

    // Build URL and request body
    let upstream_model = entry.upstream_model();
    let url = adapter.build_chat_url(&channel.base_url, upstream_model);
    let mut upstream_body = json!({
        "model": upstream_model,
        "messages": payload.messages,
        "stream": false,
    });
    adapter.transform_request(&mut upstream_body, upstream_model);

    let start = Instant::now();
    let client = crate::http_client::channel_client(
        channel.use_system_proxy,
        std::time::Duration::from_secs(30),
    )
    .map_err(|e| AdminError::Internal(format!("HTTP client: {e}")))?;
    let request = adapter
        .apply_auth(client.post(&url), &channel.api_key)
        .json(&upstream_body);
    let response = match request.send().await {
        Ok(response) => response,
        Err(e) => {
            let latency_ms = start.elapsed().as_millis() as i64;
            let message = format!("Network request failed: {}", e);
            insert_test_usage_log(
                &db,
                state.app_handle.as_ref(),
                TestUsageLogInput {
                    entry: &entry,
                    channel: &channel,
                    operation: "test_chat",
                    log_group: "test_chat",
                    prompt_tokens: 0,
                    completion_tokens: 0,
                    latency_ms,
                    status_code: 502,
                    success: false,
                    error_message: Some(&message),
                    error_kind: Some("network_error"),
                    response_ms: None,
                    error_preview: None,
                },
            );
            return Err(AdminError::Internal(message));
        }
    };

    if !response.status().is_success() {
        let latency_ms = start.elapsed().as_millis() as i64;
        let status = response.status();
        let status_code = status.as_u16() as i32;
        let body = response.text().await.unwrap_or_default();
        let error_message = format!("Upstream error {status}: {body}");
        let log_message = format!("upstream_http_{}", status.as_u16());
        insert_test_usage_log(
            &db,
            state.app_handle.as_ref(),
            TestUsageLogInput {
                entry: &entry,
                channel: &channel,
                operation: "test_chat",
                log_group: "test_chat",
                prompt_tokens: 0,
                completion_tokens: 0,
                latency_ms,
                status_code,
                success: false,
                error_message: Some(&log_message),
                error_kind: Some("http_error"),
                response_ms: None,
                error_preview: Some(&body),
            },

        );
        return Err(AdminError::Internal(error_message));
    }

    let latency_ms = start.elapsed().as_millis() as u64;
    let mut json_body: serde_json::Value = match response.json().await {
        Ok(body) => body,
        Err(e) => {
            let message = format!("Failed to parse response: {}", e);
            insert_test_usage_log(
                &db,
                state.app_handle.as_ref(),
                TestUsageLogInput {
                    entry: &entry,
                    channel: &channel,
                    operation: "test_chat",
                    log_group: "test_chat",
                    prompt_tokens: 0,
                    completion_tokens: 0,
                    latency_ms: latency_ms as i64,
                    status_code: 502,
                    success: false,
                    error_message: Some(&message),
                    error_kind: Some("parse_error"),
                    response_ms: None,
                    error_preview: None,
                },
            );
            return Err(AdminError::Internal(message));
        }
    };
    // Transform response if needed (e.g., Claude -> OpenAI format)
    adapter.transform_response(&mut json_body);

    // Extract content
    let content = json_body
        .get("choices")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("message"))
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_str())
        .unwrap_or("")
        .to_string();

    // Extract usage if present
    let usage = json_body.get("usage").map(|u| TestChatUsage {
        prompt_tokens: u.get("prompt_tokens").and_then(|v| v.as_i64()).unwrap_or(0),
        completion_tokens: u
            .get("completion_tokens")
            .and_then(|v| v.as_i64())
            .unwrap_or(0),
        total_tokens: u.get("total_tokens").and_then(|v| v.as_i64()).unwrap_or(0),
    });

    insert_test_usage_log(
        &db,
        state.app_handle.as_ref(),
        TestUsageLogInput {
            entry: &entry,
            channel: &channel,
            operation: "test_chat",
            log_group: "test_chat",
            prompt_tokens: usage.as_ref().map(|u| u.prompt_tokens).unwrap_or(0),
            completion_tokens: usage.as_ref().map(|u| u.completion_tokens).unwrap_or(0),
            latency_ms: latency_ms as i64,
            status_code: 200,
            success: true,
            error_message: None,
            error_kind: None,
            response_ms: Some(&latency_ms.to_string()),
            error_preview: None,
        },
    );

    Ok(Json(TestChatResponse {
        content,
        latency_ms,
        usage,
    }))
}
