use crate::database::dao::PaginatedResult;
use crate::database::{AccessKey, Database};
use crate::error::AppError;

/// List all access keys
pub fn list_access_keys(db: &Database) -> Result<Vec<AccessKey>, AppError> {
    db.list_access_keys()
}

pub fn list_access_keys_paginated(
    db: &Database,
    page: i32,
    page_size: i32,
) -> Result<PaginatedResult<AccessKey>, AppError> {
    db.list_access_keys_paginated(page, page_size)
}

/// Create a new access key
pub fn create_access_key(db: &Database, name: &str) -> Result<AccessKey, AppError> {
    let key = db.create_access_key(name)?;
    crate::state_version::bump();
    Ok(key)
}

/// Delete an access key by ID
pub fn delete_access_key(
    db: &Database,
    id: &str,
    app: Option<&tauri::AppHandle>,
) -> Result<(), AppError> {
    db.delete_access_key(id)?;
    if let Some(app) = app {
        crate::refresh_tray_if_enabled(app);
    }
    crate::state_version::bump();
    Ok(())
}

/// Toggle access key enabled state
pub fn toggle_access_key(
    db: &Database,
    id: &str,
    enabled: bool,
    app: Option<&tauri::AppHandle>,
) -> Result<(), AppError> {
    db.toggle_access_key(id, enabled)?;
    if let Some(app) = app {
        crate::refresh_tray_if_enabled(app);
    }
    crate::state_version::bump();
    Ok(())
}

pub fn update_access_key_models(
    db: &Database,
    id: &str,
    allowed_models: Option<Vec<String>>,
    app: Option<&tauri::AppHandle>,
) -> Result<(), AppError> {
    let normalized = allowed_models.map(|models| {
        let mut cleaned: Vec<String> = models
            .into_iter()
            .map(|model| model.trim().to_string())
            .filter(|model| !model.is_empty())
            .collect();
        cleaned.sort();
        cleaned.dedup();
        cleaned
    });
    db.update_access_key_models(id, normalized)?;
    if let Some(app) = app {
        crate::refresh_tray_if_enabled(app);
    }
    crate::state_version::bump();
    Ok(())
}
