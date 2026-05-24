use serde::Serialize;
use std::sync::atomic::{AtomicU64, Ordering};

static STATE_VERSION: AtomicU64 = AtomicU64::new(0);

/// 通知前端状态已变更。每次数据写入后调用。
/// 前端轮询此值，变化时自动刷新 UI。
pub fn bump() {
    STATE_VERSION.fetch_add(1, Ordering::Release);
}

/// 返回当前版本号（供 HTTP handler 调用）
pub fn current() -> u64 {
    STATE_VERSION.load(Ordering::Acquire)
}

/// 版本号响应结构体
#[derive(Debug, Clone, Copy, Serialize)]
pub struct StateVersionResponse {
    pub version: u64,
}
