use super::circuit_breaker::CircuitBreaker;
use super::handlers;
use super::responses_handler;
use crate::database::{AppSettings, Database};
use axum::extract::Query;
use axum::routing::{get, post};
use axum::Router;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::{Ipv4Addr, SocketAddr, UdpSocket};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{oneshot, Mutex as AsyncMutex, RwLock};
use tokio::task::JoinHandle;
use tower_http::cors::{Any, CorsLayer};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyStatus {
    pub running: bool,
    pub address: String,
    pub port: i32,
    #[serde(rename = "lanAddress")]
    pub lan_address: Option<String>,
    #[serde(rename = "lanShareEnabled")]
    pub lan_share_enabled: bool,
}

/// Shared proxy state
#[derive(Clone)]
pub struct ProxyState {
    pub db: Arc<Database>,
    pub settings: Arc<RwLock<AppSettings>>,
    pub circuit_breakers: Arc<RwLock<HashMap<String, CircuitBreaker>>>,
    pub failure_counts: Arc<RwLock<HashMap<String, u32>>>, // Entry ID -> consecutive failure count
    pub app_handle: Option<tauri::AppHandle>,
    pub http_client_direct: reqwest::Client,
    pub http_client_system_proxy: reqwest::Client,
    pub response_store: Arc<RwLock<HashMap<String, serde_json::Value>>>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct AzureDeploymentsQuery {
    #[serde(rename = "api-version")]
    _api_version: Option<String>,
}

/// HTTP proxy server
pub struct ProxyServer {
    port: i32,
    bind_address: String,
    lan_share_enabled: bool,
    connect_timeout_secs: u64,
    state: ProxyState,
    shutdown_tx: Arc<RwLock<Option<oneshot::Sender<()>>>>,
    server_task: Arc<AsyncMutex<Option<JoinHandle<()>>>>,
}

impl ProxyServer {
    pub fn new(
        port: i32,
        db: Arc<Database>,
        settings: Arc<RwLock<AppSettings>>,
        app_handle: Option<tauri::AppHandle>,
        failure_counts: Arc<RwLock<HashMap<String, u32>>>,
    ) -> Self {
        let connect_timeout_secs = settings
            .try_read()
            .map(|settings| settings.proxy_connect_timeout_secs.clamp(1, 300))
            .unwrap_or(30);
        let lan_share_enabled = settings
            .try_read()
            .map(|settings| settings.lan_share_enabled)
            .unwrap_or(false);
        let bind_address = bind_address_from_lan_share(lan_share_enabled).to_string();
        let state = ProxyState {
            db,
            settings,
            circuit_breakers: Arc::new(RwLock::new(HashMap::new())),
            failure_counts,
            app_handle,
            http_client_direct: crate::http_client::channel_client_builder(false)
                .connect_timeout(Duration::from_secs(connect_timeout_secs))
                .read_timeout(Duration::from_secs(120))
                .gzip(true)
                .build()
                .expect("failed to build direct proxy HTTP client"),
            http_client_system_proxy: crate::http_client::channel_client_builder(true)
                .connect_timeout(Duration::from_secs(connect_timeout_secs))
                .read_timeout(Duration::from_secs(120))
                .gzip(true)
                .build()
                .expect("failed to build system-proxy HTTP client"),
            response_store: Arc::new(RwLock::new(HashMap::new())),
        };

        Self {
            port,
            bind_address,
            lan_share_enabled,
            connect_timeout_secs,
            state,
            shutdown_tx: Arc::new(RwLock::new(None)),
            server_task: Arc::new(AsyncMutex::new(None)),
        }
    }

    pub async fn start(&self) -> Result<(), String> {
        self.start_with_admin(None).await
    }

    pub async fn start_with_admin(&self, admin_router: Option<Router>) -> Result<(), String> {
        if self.shutdown_tx.read().await.is_some() {
            return Err("Proxy already running".to_string());
        }

        let addr: SocketAddr = format!("{}:{}", self.bind_address, self.port)
            .parse()
            .map_err(|e| format!("Invalid address: {e}"))?;

        let (shutdown_tx, shutdown_rx) = oneshot::channel();

        let cors = CorsLayer::new()
            .allow_origin(Any)
            .allow_methods(Any)
            .allow_headers(Any);

        let mut app = Router::new()
            .route("/health", get(handlers::health_check))
            .route(
                "/v1/chat/completions",
                post(handlers::handle_chat_completions),
            )
            .route("/v1/messages", post(handlers::handle_messages))
            .route("/v1/models", get(handlers::handle_list_models))
            .route("/anthropic/v1/models", get(handlers::handle_list_models_claude))
            .route("/v1beta/models", get(handlers::handle_list_models_gemini))
            .route(
                "/openai/deployments",
                get(
                    |state, query: Query<AzureDeploymentsQuery>, headers| async move {
                        handlers::handle_list_models_azure(state, query, headers).await
                    },
                ),
            )
            // Gemini native endpoint (generateContent + streamGenerateContent)
            .route("/v1beta/models/*rest", post(handlers::handle_gemini_native))
            // Gemini single model detail
            .route("/v1beta/models/{model}", get(handlers::handle_gemini_model_detail))
            // Azure native endpoint
            .route(
                "/openai/deployments/*rest",
                post(handlers::handle_azure_chat),
            )
            // OpenAI Responses API (Chat Completions format under the hood)
            .route("/v1/responses", post(responses_handler::handle_responses))
            .route(
                "/v1/responses/:response_id",
                get(responses_handler::get_response).delete(responses_handler::delete_response),
            )
            .route(
                "/v1/responses/:response_id/cancel",
                post(responses_handler::cancel_response),
            )
            .layer(cors)
            .with_state(self.state.clone());

        if let Some(admin_router) = admin_router {
            app = app.merge(admin_router);
        }

        let listener = tokio::net::TcpListener::bind(&addr)
            .await
            .map_err(|e| format!("Failed to bind: {e}"))?;

        log::info!(
            "Proxy server started on {addr}, connect_timeout={}s",
            self.connect_timeout_secs
        );

        *self.shutdown_tx.write().await = Some(shutdown_tx);

        let server_task = tokio::spawn(async move {
            axum::serve(listener, app)
                .with_graceful_shutdown(async {
                    let _ = shutdown_rx.await;
                })
                .await
                .unwrap_or_else(|e| {
                    log::error!("Proxy server error: {e}");
                });

            log::info!("Proxy server stopped");
        });
        *self.server_task.lock().await = Some(server_task);

        Ok(())
    }

    pub async fn stop(&self) -> Result<(), String> {
        if let Some(tx) = self.shutdown_tx.write().await.take() {
            let _ = tx.send(());
            if let Some(task) = self.server_task.lock().await.take() {
                task.await
                    .map_err(|e| format!("Proxy server shutdown failed: {e}"))?;
            }
            Ok(())
        } else {
            Err("Proxy not running".to_string())
        }
    }

    pub fn get_status(&self) -> ProxyStatus {
        let running = self
            .shutdown_tx
            .try_read()
            .map(|guard| guard.is_some())
            .unwrap_or(true);

        ProxyStatus {
            running,
            address: self.bind_address.clone(),
            port: self.port,
            lan_address: local_lan_ipv4().map(|ip| format!("http://{ip}:{}/v1", self.port)),
            lan_share_enabled: self.lan_share_enabled,
        }
    }
}

pub fn bind_address(settings: &AppSettings) -> &'static str {
    bind_address_from_lan_share(settings.lan_share_enabled)
}

fn bind_address_from_lan_share(enabled: bool) -> &'static str {
    if enabled {
        "0.0.0.0"
    } else {
        "127.0.0.1"
    }
}

pub fn local_lan_ipv4() -> Option<Ipv4Addr> {
    let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).ok()?;
    socket.connect((Ipv4Addr::new(8, 8, 8, 8), 80)).ok()?;
    let local_addr = socket.local_addr().ok()?;
    match local_addr.ip() {
        std::net::IpAddr::V4(ip) if !ip.is_loopback() && !ip.is_unspecified() => Some(ip),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bind_address_defaults_to_loopback_when_lan_share_disabled() {
        let settings = AppSettings {
            lan_share_enabled: false,
            ..AppSettings::default()
        };

        assert_eq!(bind_address(&settings), "127.0.0.1");
    }

    #[test]
    fn bind_address_uses_all_interfaces_when_lan_share_enabled() {
        let settings = AppSettings {
            lan_share_enabled: true,
            ..AppSettings::default()
        };

        assert_eq!(bind_address(&settings), "0.0.0.0");
    }
}
