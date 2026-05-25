// Proxy administration handlers (admin side)
// Provides status, start, and stop endpoints for the HTTP proxy.

use crate::admin::{error::AdminError, state::AdminState};
use crate::commands::config::refresh_settings_l1;
use crate::proxy::ProxyStatus;
use axum::{extract::State, Json};
use serde::Serialize;

#[derive(Serialize)]
pub struct ProxyStatusResponse {
    pub running: bool,
    pub port: i32,
    pub address: String,
    #[serde(rename = "lanAddress")]
    pub lan_address: Option<String>,
    #[serde(rename = "lanShareEnabled")]
    pub lan_share_enabled: bool,
}

/// Get the current proxy status.
///
/// Returns JSON with `running` flag and the configured listen `port`.
pub async fn get_status(
    State(state): State<AdminState>,
) -> Result<Json<ProxyStatusResponse>, AdminError> {
    let runtime = state
        .runtime
        .as_ref()
        .ok_or_else(|| AdminError::Internal("AdminState missing runtime".to_string()))?;

    let settings = runtime.settings.read().await.clone();
    let proxy_guard = runtime.proxy.read().await;

    let status = match proxy_guard.as_ref() {
        Some(server) => server.get_status(),
        None => ProxyStatus {
            running: false,
            address: crate::proxy::bind_address(&settings).to_string(),
            port: settings.listen_port,
            lan_address: crate::proxy::local_lan_ipv4()
                .map(|ip| format!("http://{ip}:{}/v1", settings.listen_port)),
            lan_share_enabled: settings.lan_share_enabled,
        },
    };

    Ok(Json(ProxyStatusResponse {
        running: status.running,
        port: status.port,
        address: status.address,
        lan_address: status.lan_address,
        lan_share_enabled: status.lan_share_enabled,
    }))
}

/// Start the proxy server.
///
/// Fails with an error if the proxy is already running.
pub async fn start(
    State(state): State<AdminState>,
) -> Result<Json<ProxyStatusResponse>, AdminError> {
    let runtime = state
        .runtime
        .as_ref()
        .ok_or_else(|| AdminError::Internal("AdminState missing runtime".to_string()))?;
    let app_handle = state
        .app_handle
        .as_ref()
        .ok_or_else(|| AdminError::Internal("AdminState missing app handle".to_string()))?
        .clone();

    let settings = runtime.settings.read().await.clone();
    let port = settings.listen_port;

    // Ensure proxy is not already running
    let mut proxy_guard = runtime.proxy.write().await;
    if proxy_guard.is_some() {
        return Err(AdminError::BadRequest("Proxy already running".to_string()));
    }

    // Build the proxy server
    let server = crate::proxy::ProxyServer::new(
        port,
        runtime.db.clone(),
        runtime.settings.clone(),
Some(app_handle.clone()),
        runtime.failure_counts.clone(),
    );

    // Build admin router for combined mode (same logic as proxy_cmd::start_proxy)
    let admin_router = crate::admin::build_combined_router(
        &settings,
        crate::admin::AdminState::new_runtime(runtime.clone(), app_handle.clone()),
    );

    server
        .start_with_admin(admin_router)
        .await
        .map_err(|e| AdminError::Internal(e.to_string()))?;

    // Store the server handle
    *proxy_guard = Some(server);

    // Update config and refresh L1 cache
    runtime
        .db
        .set_config_value("proxy_enabled", "1")
        .map_err(|e| AdminError::Internal(e.to_string()))?;
    refresh_settings_l1(&runtime)
        .await
        .map_err(|e| AdminError::Internal(e.to_string()))?;

    // Return the new status
    Ok(Json(ProxyStatusResponse {
        running: true,
        port,
        address: crate::proxy::bind_address(&settings).to_string(),
        lan_address: crate::proxy::local_lan_ipv4().map(|ip| format!("http://{ip}:{port}/v1")),
        lan_share_enabled: settings.lan_share_enabled,
    }))
}

/// Stop the proxy server.
///
/// Fails if the proxy is not currently running.
pub async fn stop(
    State(state): State<AdminState>,
) -> Result<Json<ProxyStatusResponse>, AdminError> {
    let runtime = state
        .runtime
        .as_ref()
        .ok_or_else(|| AdminError::Internal("AdminState missing runtime".to_string()))?;
    let settings = runtime.settings.read().await.clone();

    let mut proxy_guard = runtime.proxy.write().await;
    let (running, port) = if let Some(server) = proxy_guard.take() {
        server
            .stop()
            .await
            .map_err(|e| AdminError::Internal(e.to_string()))?;
        (false, settings.listen_port)
    } else {
        // Proxy was not running
        return Err(AdminError::BadRequest("Proxy not running".to_string()));
    };

    // Update config and refresh L1 cache
    runtime
        .db
        .set_config_value("proxy_enabled", "0")
        .map_err(|e| AdminError::Internal(e.to_string()))?;
    refresh_settings_l1(&runtime)
        .await
        .map_err(|e| AdminError::Internal(e.to_string()))?;

    Ok(Json(ProxyStatusResponse {
        running,
        port,
        address: crate::proxy::bind_address(&settings).to_string(),
        lan_address: crate::proxy::local_lan_ipv4().map(|ip| format!("http://{ip}:{port}/v1")),
        lan_share_enabled: settings.lan_share_enabled,
    }))
}
