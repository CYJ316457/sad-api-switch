use crate::admin::RestartInfo;
use crate::database::AppSettings;
use crate::error::AppError;
use crate::AppState;
use serde::{Deserialize, Serialize};
use tauri::State;

async fn restart_proxy_if_running(
    app: tauri::AppHandle,
    state: &AppState,
    previous_settings: &AppSettings,
) -> Result<(), AppError> {
    let mut proxy_guard = state.proxy.write().await;
    let Some(server) = proxy_guard.take() else {
        return Ok(());
    };

    let _ = server.stop().await;
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    let settings = state.settings.read().await.clone();
    if !settings.proxy_enabled {
        return Ok(());
    }

    let admin_router = crate::admin::build_combined_router(
        &settings,
        crate::admin::AdminState::new_runtime(state.clone(), app.clone()),
    );
    let new_server = crate::proxy::ProxyServer::new(
        settings.listen_port,
        state.db.clone(),
        state.settings.clone(),
Some(app.clone()),
        state.failure_counts.clone(),
    );
    if let Err(error) = new_server.start_with_admin(admin_router).await {
        // Rollback: restore previous settings and restart proxy with old config
        state.db.update_settings(previous_settings)?;
        let restored_settings = refresh_settings_l1(state).await?;
        sync_autostart(&restored_settings);

        let rollback_server = crate::proxy::ProxyServer::new(
            previous_settings.listen_port,
            state.db.clone(),
            state.settings.clone(),
            Some(app.clone()),
            state.failure_counts.clone(),
        );
        let rollback_admin_router = crate::admin::build_combined_router(
            &restored_settings,
            crate::admin::AdminState::new_runtime(state.clone(), app.clone()),
        );
        rollback_server
            .start_with_admin(rollback_admin_router)
            .await
            .map_err(|restore_error| {
                AppError::Proxy(format!("{error}; rollback failed: {restore_error}"))
            })?;
        *proxy_guard = Some(rollback_server);
        // Rollback succeeded - log the original error but don't propagate it
        log::error!("Proxy restart failed, rolled back to previous config: {error}");
    }

    state.db.set_config_value("proxy_enabled", "1")?;
    *proxy_guard = Some(new_server);
    Ok(())
}

const GITHUB_REPO: &str = "CYJ316457/sad-api-switch";

#[derive(Deserialize)]
struct GithubRelease {
    tag_name: String,
    html_url: String,
    #[allow(dead_code)]
    body: Option<String>,
    #[serde(default)]
    assets: Vec<GithubReleaseAsset>,
}

#[derive(Deserialize)]
struct GithubReleaseAsset {
    name: String,
    browser_download_url: String,
    #[serde(default)]
    size: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    current: String,
    latest: String,
    url: String,
    asset_name: String,
    download_url: String,
    asset_size: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallUpdateParams {
    download_url: String,
    asset_name: Option<String>,
}

fn parse_version_parts(value: &str) -> Vec<u64> {
    value
        .trim()
        .trim_start_matches('v')
        .split(['.', '-'])
        .map(|part| {
            part.chars()
                .take_while(|ch| ch.is_ascii_digit())
                .collect::<String>()
        })
        .filter(|part| !part.is_empty())
        .map(|part| part.parse::<u64>().unwrap_or(0))
        .collect()
}

fn is_newer_version(latest: &str, current: &str) -> bool {
    let latest_parts = parse_version_parts(latest);
    let current_parts = parse_version_parts(current);
    let len = latest_parts.len().max(current_parts.len());
    for index in 0..len {
        let latest_part = *latest_parts.get(index).unwrap_or(&0);
        let current_part = *current_parts.get(index).unwrap_or(&0);
        if latest_part != current_part {
            return latest_part > current_part;
        }
    }
    false
}

fn choose_windows_exe_asset(assets: &[GithubReleaseAsset]) -> Option<&GithubReleaseAsset> {
    assets
        .iter()
        .filter(|asset| asset.name.to_ascii_lowercase().ends_with(".exe"))
        .max_by_key(|asset| asset.size)
}

#[tauri::command]
pub async fn check_update() -> Result<Option<UpdateInfo>, AppError> {
    let current = env!("CARGO_PKG_VERSION");

    let url = format!(
        "https://api.github.com/repos/{}/releases/latest",
        GITHUB_REPO
    );
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| AppError::Network(e.to_string()))?;

    let resp = client
        .get(&url)
        .header("User-Agent", "api-switch")
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|e| AppError::Network(e.to_string()))?;

    if !resp.status().is_success() {
        return Ok(None);
    }

    let release: GithubRelease = resp
        .json()
        .await
        .map_err(|e| AppError::Network(e.to_string()))?;
    let latest = release.tag_name.trim_start_matches('v').to_string();

    if !is_newer_version(&latest, current) {
        return Ok(None);
    }

    let asset = choose_windows_exe_asset(&release.assets)
        .ok_or_else(|| AppError::Internal("No Windows exe asset found in latest release".into()))?;

    Ok(Some(UpdateInfo {
        current: current.to_string(),
        latest,
        url: release.html_url,
        asset_name: asset.name.clone(),
        download_url: asset.browser_download_url.clone(),
        asset_size: asset.size,
    }))
}

fn powershell_literal(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

#[cfg(windows)]
fn spawn_update_script(script_path: &std::path::Path) -> Result<(), AppError> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x08000000;

    std::process::Command::new("powershell")
        .args([
            "-NoProfile",
            "-ExecutionPolicy",
            "Bypass",
            "-File",
            &script_path.to_string_lossy(),
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map_err(|e| AppError::Internal(format!("Failed to start updater: {e}")))?;
    Ok(())
}

#[cfg(not(windows))]
fn spawn_update_script(_script_path: &std::path::Path) -> Result<(), AppError> {
    Err(AppError::Internal(
        "Portable auto update is currently implemented for Windows only".into(),
    ))
}

#[tauri::command]
pub async fn install_update(
    app: tauri::AppHandle,
    params: InstallUpdateParams,
) -> Result<(), AppError> {
    let current_exe =
        std::env::current_exe().map_err(|e| AppError::Internal(format!("current_exe: {e}")))?;
    let asset_name = params
        .asset_name
        .as_deref()
        .filter(|name| name.to_ascii_lowercase().ends_with(".exe"))
        .unwrap_or("api-switch-update.exe");
    let update_dir = std::env::temp_dir().join("api-switch-update");
    std::fs::create_dir_all(&update_dir)
        .map_err(|e| AppError::Internal(format!("create update dir: {e}")))?;
    let downloaded_exe = update_dir.join(asset_name);

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .map_err(|e| AppError::Network(e.to_string()))?;
    let bytes = client
        .get(&params.download_url)
        .header("User-Agent", "api-switch")
        .send()
        .await
        .map_err(|e| AppError::Network(e.to_string()))?
        .error_for_status()
        .map_err(|e| AppError::Network(e.to_string()))?
        .bytes()
        .await
        .map_err(|e| AppError::Network(e.to_string()))?;

    std::fs::write(&downloaded_exe, &bytes)
        .map_err(|e| AppError::Internal(format!("write update file: {e}")))?;

    let script_path = update_dir.join("apply-update.ps1");
    let pid = std::process::id();
    let script = format!(
        "$ErrorActionPreference = 'Stop'\n\
         $pidToWait = {pid}\n\
         $source = {source}\n\
         $target = {target}\n\
         $backup = \"$target.bak\"\n\
         Wait-Process -Id $pidToWait -ErrorAction SilentlyContinue\n\
         Start-Sleep -Milliseconds 500\n\
         if (Test-Path $backup) {{ Remove-Item -LiteralPath $backup -Force -ErrorAction SilentlyContinue }}\n\
         if (Test-Path $target) {{ Move-Item -LiteralPath $target -Destination $backup -Force }}\n\
         Copy-Item -LiteralPath $source -Destination $target -Force\n\
         Start-Process -FilePath $target\n\
         Start-Sleep -Seconds 2\n\
         Remove-Item -LiteralPath $backup -Force -ErrorAction SilentlyContinue\n",
        source = powershell_literal(&downloaded_exe.to_string_lossy()),
        target = powershell_literal(&current_exe.to_string_lossy()),
    );
    std::fs::write(&script_path, script)
        .map_err(|e| AppError::Internal(format!("write update script: {e}")))?;

    spawn_update_script(&script_path)?;
    app.exit(0);
    Ok(())
}

fn sync_autostart(settings: &AppSettings) {
    let app_name = "API Switch";
    let exe = match std::env::current_exe() {
        Ok(e) => e,
        Err(e) => {
            log::error!("Failed to get exe path: {e}");
            return;
        }
    };
    let exe_path = exe.to_string_lossy().to_string();

    let auto = match auto_launch::AutoLaunchBuilder::new()
        .set_app_name(app_name)
        .set_app_path(&exe_path)
        .build()
    {
        Ok(a) => a,
        Err(e) => {
            log::error!("Failed to create AutoLaunch: {e}");
            return;
        }
    };

    let result = if settings.autostart {
        auto.enable()
    } else {
        auto.disable()
    };

    if let Err(e) = result {
        log::error!("Failed to sync autostart: {e}");
    }
}

pub async fn refresh_settings_l1(state: &AppState) -> Result<AppSettings, AppError> {
    // Settings writes are rare and settings are small.
    // Keep DB as the source of truth: after every settings write,
    // rebuild the L1 settings cache from DB instead of patching fields manually.
    let settings = state.db.get_settings()?;
    *state.settings.write().await = settings.clone();
    Ok(settings)
}

pub async fn apply_settings_update(
    app: tauri::AppHandle,
    state: &AppState,
    settings: AppSettings,
    restart_async: bool,
) -> Result<(), AppError> {
    let _ = apply_settings_update_with_restart(app, state, settings, restart_async).await?;
    Ok(())
}

pub async fn apply_settings_update_with_restart(
    app: tauri::AppHandle,
    state: &AppState,
    settings: AppSettings,
    restart_async: bool,
) -> Result<Option<RestartInfo>, AppError> {
    let previous_settings = state.settings.read().await.clone();
    let mut settings = settings;
    if settings.lan_share_enabled {
        settings.access_key_required = true;
    }
    let requires_proxy_restart = previous_settings.listen_port != settings.listen_port
        || previous_settings.lan_share_enabled != settings.lan_share_enabled
        || previous_settings.web_admin_enabled != settings.web_admin_enabled
        || previous_settings.web_admin_username != settings.web_admin_username
        || previous_settings.web_admin_password != settings.web_admin_password
        || previous_settings.web_admin_port != settings.web_admin_port;

    state.db.update_settings(&settings)?;
    let settings = refresh_settings_l1(state).await?;
    sync_autostart(&settings);

    let admin_relocated = settings.web_admin_port != previous_settings.web_admin_port;
    let proxy_was_running = state.proxy.read().await.is_some();
    let proxy_restart_required = proxy_was_running && requires_proxy_restart;

    let mut restart_info = RestartInfo {
        admin_relocated,
        new_admin_base_url: if admin_relocated {
            Some(format!(
                "http://127.0.0.1:{}/admin",
                settings.web_admin_port
            ))
        } else {
            None
        },
        proxy_restart_required,
        proxy_restarted: false,
    };

    if requires_proxy_restart {
        let state_for_restart = state.clone();
        let app_for_restart = app.clone();
        let previous_settings_for_restart = previous_settings.clone();
        let restart_work = async move {
            restart_proxy_if_running(
                app_for_restart.clone(),
                &state_for_restart,
                &previous_settings_for_restart,
            )
            .await?;

            if let Err(e) = crate::admin::restart_admin(
                state_for_restart.clone(),
                app_for_restart.clone(),
                state_for_restart.admin.clone(),
            )
            .await
            {
                log::error!("Failed to restart admin server after settings update: {e}");
            }
            Ok::<(), AppError>(())
        };

        if restart_async {
            // For async mode, mark that restart was triggered
            restart_info.proxy_restarted = true;
            tauri::async_runtime::spawn(async move {
                match restart_work.await {
                    Ok(_) => log::debug!("Settings side effects applied successfully"),
                    Err(e) => log::error!("Failed to apply settings runtime side effects: {e}"),
                }
            });
        } else {
            restart_work.await?;
            restart_info.proxy_restarted = true;
        }
    }

    crate::refresh_tray_if_enabled(&app);
    Ok(Some(restart_info))
}

#[tauri::command]
pub async fn get_settings(state: State<'_, AppState>) -> Result<AppSettings, AppError> {
    Ok(state.settings.read().await.clone())
}

#[tauri::command]
pub async fn update_settings(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    settings: AppSettings,
) -> Result<(), AppError> {
    apply_settings_update(app, &state, settings, false).await
}
