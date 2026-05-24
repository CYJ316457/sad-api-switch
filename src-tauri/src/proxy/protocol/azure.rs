use super::ProtocolAdapter;
/// Azure OpenAI protocol adapter.
///
/// Azure Chat Completions API is OpenAI-protocol compatible — the request/response
/// body format is identical to OpenAI. The differences are:
///
/// - **URL**: `https://<resource>.openai.azure.com/openai/deployments/<deployment>/chat/completions?api-version=2024-02-01`
/// - **Auth**: `api-key` header (not `Bearer` token)
/// - **Model**: The `model` field in the request body is *ignored* by Azure;
///   the deployment name is in the URL path.
/// - **Response**: Uses OpenAI-compatible format natively. SSE streaming is also
///   OpenAI-compatible (no transformation needed).
///
/// NOTE: `api-version` is configurable. We use `2024-02-01` as the default
/// because it is widely supported. Newer versions add features but this one
/// covers chat completions + tools + streaming.
use serde_json::{json, Value};

/// Default API version for Azure OpenAI.
const AZURE_API_VERSION: &str = "2024-02-01";

/// 是否在翻译时穿透本协议官方文档未定义的字段。
///
/// Azure OpenAI 的请求/响应与 OpenAI 协议完全兼容，body 字段直通即可
/// （见 `azure_to_openai_request` / `AzureAdapter::transform_request`），
/// 因此本协议**没有独立的 passthrough 代码分支**，该常量仅作范式一致性标记：
/// 所有协议模块顶部都有这个常量，便于未来按需扩展时一处切换。
///
/// 如果未来 Azure 出现需要按字段穿透的场景，可在翻译函数里加上
/// `if ENABLE_UNKNOWN_FIELD_PASSTHROUGH { ... }` 分支。
#[allow(dead_code)]
const ENABLE_UNKNOWN_FIELD_PASSTHROUGH: bool = true;

pub struct AzureAdapter;

impl ProtocolAdapter for AzureAdapter {
    fn build_chat_url(&self, base_url: &str, model: &str) -> String {
        // Azure format:
        //   {base_url}/openai/deployments/{deployment}/chat/completions?api-version=...
        // `model` from the API entry is used as the deployment name.
        let base = base_url.trim_end_matches('/');
        format!(
            "{}/openai/deployments/{}/chat/completions?api-version={}",
            base, model, AZURE_API_VERSION
        )
    }

    fn build_models_url(&self, base_url: &str, _api_key: &str) -> String {
        let base = base_url.trim_end_matches('/');
        format!(
            "{}/openai/deployments?api-version={}",
            base, AZURE_API_VERSION
        )
    }

    fn uses_query_auth(&self) -> bool {
        false
    }

    fn build_auth_headers(&self, api_key: &str) -> Vec<(String, String)> {
        vec![("api-key".to_string(), api_key.to_string())]
    }

    fn apply_auth(
        &self,
        builder: reqwest::RequestBuilder,
        api_key: &str,
    ) -> reqwest::RequestBuilder {
        builder.header("api-key", api_key)
    }

    fn transform_request(&self, body: &mut Value, _actual_model: &str) {
        // Azure ignores the `model` field in the request body — the deployment
        // name is already in the URL. We still set it so logging / compatibility
        // tools can see what was requested, but it has no effect on routing.
        // No other transformation needed — Azure uses OpenAI format natively.
        // Remove `model` from body to avoid Azure 400 errors for unknown model.
        if let Some(obj) = body.as_object_mut() {
            obj.remove("model");
        }
    }

    fn transform_response(&self, _body: &mut Value) {
        // Azure response is already in OpenAI format. No transformation needed.
    }

    fn needs_sse_transform(&self) -> bool {
        false
    }

    fn extract_sse_usage(&self, data_line: &str) -> (i64, i64) {
        if data_line == "[DONE]" {
            return (0, 0);
        }
        let Ok(value) = serde_json::from_str::<Value>(data_line) else {
            return (0, 0);
        };
        let prompt = value
            .get("usage")
            .and_then(|u| u.get("prompt_tokens"))
            .and_then(Value::as_i64)
            .unwrap_or(0);
        let completion = value
            .get("usage")
            .and_then(|u| u.get("completion_tokens"))
            .and_then(Value::as_i64)
            .unwrap_or(0);
        (prompt, completion)
    }

    fn transform_sse_line(&self, data_line: &str) -> Option<String> {
        // Should never be called (needs_sse_transform = false).
        Some(data_line.to_string())
    }

    fn parse_models_response(&self, body: &Value) -> Vec<(String, Option<String>)> {
        // Azure returns: { data: [{ id: "deployment-name", model: "gpt-4o", ... }] }
        // The `id` is the deployment name, `model` is the underlying model.
        // We return deployment name as the id, model as the display name.
        body.get("data")
            .and_then(|d| d.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|m| {
                        let id = m.get("id")?.as_str()?.to_string();
                        // Use "model" field as display name if available
                        let owned_by = m
                            .get("model")
                            .and_then(|v| v.as_str())
                            .map(String::from)
                            .or_else(|| {
                                m.get("owned_by").and_then(|v| v.as_str()).map(String::from)
                            });
                        Some((id, owned_by))
                    })
                    .collect()
            })
            .unwrap_or_default()
    }
}

// ═══════════════════════════════════════════════════════════════════
//  Public API: Azure OpenAI -> OpenAI request conversion
// ═══════════════════════════════════════════════════════════════════

/// Convert Azure OpenAI request format to standard OpenAI format.
///
/// Azure and OpenAI request formats are identical except Azure puts the
/// deployment name (model) in the URL path and ignores `model` in the body.
/// This function takes an Azure request and returns an OpenAI-compatible request,
/// ensuring the `model` field is set from the deployment name if not present.
///
/// - model: set from deployment name if missing in body
/// - messages, max_tokens, temperature, etc.: passthrough as-is
/// - tools, tool_choice, stream, stop: passthrough as-is
pub fn azure_to_openai_request(azure: &Value, deployment: &str) -> Value {
    let mut openai = azure.clone();

    // Azure puts model in URL path, not body. Ensure model field exists for OpenAI.
    if openai.get("model").is_none() {
        openai["model"] = json!(deployment);
    }

    openai
}

// ═══════════════════════════════════════════════════════════════════
//  Tests
// ═══════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // ─── azure_to_openai_request tests ────────────────────────────

    #[test]
    fn basic_azure_to_openai() {
        let azure = json!({
            "messages": [
                {"role": "user", "content": "Hello"}
            ],
            "max_tokens": 100,
            "temperature": 0.7
        });

        let openai = azure_to_openai_request(&azure, "my-gpt4-deployment");

        // model should be injected from deployment name
        assert_eq!(openai["model"], "my-gpt4-deployment");
        assert_eq!(openai["messages"][0]["role"], "user");
        assert_eq!(openai["messages"][0]["content"], "Hello");
        assert_eq!(openai["max_tokens"], 100);
        assert_eq!(openai["temperature"], 0.7);
    }

    #[test]
    fn azure_to_openai_preserves_existing_model() {
        let azure = json!({
            "model": "gpt-4",
            "messages": [
                {"role": "user", "content": "Hi"}
            ]
        });

        let openai = azure_to_openai_request(&azure, "my-deployment");

        // Existing model in body should be kept (not overwritten)
        assert_eq!(openai["model"], "gpt-4");
    }

    #[test]
    fn azure_to_openai_passthrough_all_fields() {
        let azure = json!({
            "messages": [{"role": "user", "content": "test"}],
            "max_tokens": 512,
            "temperature": 0.9,
            "top_p": 0.95,
            "stream": true,
            "stop": ["END"],
            "tools": [{"type": "function", "function": {"name": "search"}}],
            "tool_choice": "auto"
        });

        let openai = azure_to_openai_request(&azure, "deployment-1");

        assert_eq!(openai["max_tokens"], 512);
        assert_eq!(openai["temperature"], 0.9);
        assert_eq!(openai["top_p"], 0.95);
        assert_eq!(openai["stream"], true);
        assert_eq!(openai["stop"], json!(["END"]));
        assert_eq!(openai["tools"][0]["function"]["name"], "search");
        assert_eq!(openai["tool_choice"], "auto");
    }

    #[test]
    fn azure_to_openai_empty_body() {
        let azure = json!({});
        let openai = azure_to_openai_request(&azure, "gpt-4-deploy");
        assert_eq!(openai["model"], "gpt-4-deploy");
    }
}
