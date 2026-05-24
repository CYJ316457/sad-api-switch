//! 转发层中间件
//!
//! 本模块的原初设计目标是把 forwarder 里的横切关注点（stream_options 注入、
//! model:xxx 注入、usage 统计、熔断决策、idle timeout）抽象成可装配的中间件。
//!
//! 阶段 4 的收口决策：**只保留真正有独立价值的中间件**，其他横切逻辑保持
//! 在 forwarder.rs 内原生实现，不强行中间件化。理由：
//!
//! - `StreamOptionsMiddleware`：**保留**。P2 的唯一实现路径，独立价值清晰
//!   （把 `insert` 改成 `entry().or_insert()` 的修复逻辑集中在一处）。
//!
//! - `ModelAnnotationMiddleware`（已删）：与 forwarder.rs 内的 `model_info_delta`
//!   老路径功能重叠且缺少去重，实际装配会造成每 chunk 重复注入。P5 现在通过
//!   在 `should_append_model_info` 里检查 `caller_kind == Responses` 来修复，
//!   无需中间件。
//!
//! - `IdleTimeoutMiddleware`（已删）：idle timeout 依赖 async 状态机
//!   （`tokio::time::sleep` + `poll` 重置），无法用同步 trait 方法表达。
//!   实际实现在 forwarder.rs 的 `STREAMING_IDLE_TIMEOUT` poll 循环，以及
//!   `claude.rs` 的 `transform_openai_sse_to_claude_stream` 里的 `tokio::select!`。
//!
//! - `UsageLoggingMiddleware`（已删）：usage 统计需要跨 chunk 累积状态
//!   （`prompt_tokens` / `completion_tokens` / `first_token_ms` / `StreamLogGuard`），
//!   不适合无状态 middleware 表达。实际在 forwarder.rs 原生实现。
//!
//! - `CircuitBreakerMiddleware`（已删）：熔断决策需要访问共享 `circuit_breakers`
//!   RwLock 以及根据状态码做异步写入，不适合同步 middleware trait。实际在
//!   forwarder.rs 的 `cool_down_entry` / `disable_entry` 实现。
//!
//! **留下的 `RequestContext` 和 `CallerKind`** 仍有价值——forwarder 通过
//! `caller_kind` 区分调用方并在 `should_append_model_info` 等决策点做分支。

use serde_json::Value;
use std::sync::Arc;

/// 调用方类型：标识代理被哪个入口 handler 调用
#[derive(Clone)]
pub enum CallerKind {
    OpenAiChat,
    ClaudeMessages,
    GeminiNative,
    AzureChat,
    Responses,
}

/// 请求上下文，传递给中间件
#[derive(Clone)]
pub struct RequestContext {
    pub caller_kind: CallerKind,
    #[allow(dead_code)]
    pub requested_model: Arc<str>,
}

/// 转发器中间件 trait
///
/// 当前只有 `StreamOptionsMiddleware` 实现。其他横切逻辑保持原生。
pub trait ForwarderMiddleware: Send + Sync {
    /// 请求体发出前
    fn on_request(&self, _body: &mut Value, _ctx: &RequestContext) {}
    /// 响应体接收后（非流式）
    #[allow(dead_code)]
    fn on_response_complete(&self, _body: &mut Value, _ctx: &RequestContext) {}
    /// SSE 数据块（流式）
    #[allow(dead_code)]
    fn on_sse_chunk(&self, _chunk: &mut String, _ctx: &RequestContext) {}
}

/// P2 修复：`stream_options` 不再无条件覆盖，改用 `entry().or_insert()` 合并。
///
/// 保留客户端显式传入的 `stream_options` 字段（如 `include_usage: false`、
/// `continuous_usage_stats` 等），只在未传时补默认 `include_usage: true`。
///
/// 这是 P2 bug 的唯一修复路径——forwarder.rs 原有的 `body_obj.insert("stream_options", ...)`
/// 已删除。
pub struct StreamOptionsMiddleware;

impl ForwarderMiddleware for StreamOptionsMiddleware {
    fn on_request(&self, body: &mut Value, _ctx: &RequestContext) {
        if !body
            .get("stream")
            .and_then(|s| s.as_bool())
            .unwrap_or(false)
        {
            return;
        }
        if let Some(obj) = body.as_object_mut() {
            let so = obj
                .entry("stream_options".to_string())
                .or_insert(serde_json::json!({}));
            if let Some(so_obj) = so.as_object_mut() {
                so_obj
                    .entry("include_usage".to_string())
                    .or_insert(serde_json::json!(true));
            }
        }
    }
}
