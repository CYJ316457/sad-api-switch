use crate::database::dao::PaginatedResult;
use crate::database::{lock_conn, Database};
use crate::error::AppError;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessKey {
    pub id: String,
    pub name: String,
    pub key: String,
    pub enabled: bool,
    pub created_at: i64,
    pub allowed_models: Option<Vec<String>>,
}

impl AccessKey {
    pub fn allows_model(&self, model: &str) -> bool {
        match &self.allowed_models {
            None => true,
            Some(models) => models.iter().any(|allowed| allowed == model),
        }
    }
}

impl Database {
    pub fn list_access_keys(&self) -> Result<Vec<AccessKey>, AppError> {
        let conn = lock_conn!(self.conn);
        let mut stmt = conn.prepare(
            "SELECT id, name, key, enabled, created_at, allowed_models FROM access_keys ORDER BY created_at",
        )?;

        let keys = stmt
            .query_map([], |row| {
                let enabled: i32 = row.get(3)?;
                Ok(AccessKey {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    key: row.get(2)?,
                    enabled: enabled != 0,
                    created_at: row.get(4)?,
                    allowed_models: parse_allowed_models(row.get(5)?)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| AppError::Database(e.to_string()))?;

        Ok(keys)
    }

    pub fn list_access_keys_paginated(
        &self,
        page: i32,
        page_size: i32,
    ) -> Result<PaginatedResult<AccessKey>, AppError> {
        let conn = lock_conn!(self.conn);
        let page = page.max(1);
        let page_size = page_size.max(1).min(100);
        let offset = i64::from(page.saturating_sub(1)) * i64::from(page_size);

        let total: i64 = conn.query_row(
            "SELECT COUNT(*) FROM access_keys",
            [],
            |row| row.get(0),
        )?;

        let mut stmt = conn.prepare(
            "SELECT id, name, key, enabled, created_at, allowed_models FROM access_keys ORDER BY created_at LIMIT ?1 OFFSET ?2",
        )?;

        let keys = stmt
            .query_map(rusqlite::params![page_size, offset], |row| {
                let enabled: i32 = row.get(3)?;
                Ok(AccessKey {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    key: row.get(2)?,
                    enabled: enabled != 0,
                    created_at: row.get(4)?,
                    allowed_models: parse_allowed_models(row.get(5)?)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| AppError::Database(e.to_string()))?;

        Ok(PaginatedResult {
            items: keys,
            total,
            page,
            page_size,
        })
    }

    pub fn create_access_key(&self, name: &str) -> Result<AccessKey, AppError> {
        let conn = lock_conn!(self.conn);
        let id = uuid::Uuid::new_v4().to_string();
        let key = format!("sk-{}", uuid::Uuid::new_v4().to_string().replace("-", ""));
        let now = chrono::Utc::now().timestamp();

        conn.execute(
            "INSERT INTO access_keys (id, name, key, enabled, created_at) VALUES (?1, ?2, ?3, 1, ?4)",
            rusqlite::params![id, name, key, now],
        )?;

        Ok(AccessKey {
            id,
            name: name.to_string(),
            key,
            enabled: true,
            created_at: now,
            allowed_models: None,
        })
    }

    pub fn delete_access_key(&self, id: &str) -> Result<(), AppError> {
        let conn = lock_conn!(self.conn);
        conn.execute("DELETE FROM access_keys WHERE id = ?1", [id])?;
        Ok(())
    }

    pub fn toggle_access_key(&self, id: &str, enabled: bool) -> Result<(), AppError> {
        let conn = lock_conn!(self.conn);
        conn.execute(
            "UPDATE access_keys SET enabled = ?1 WHERE id = ?2",
            rusqlite::params![enabled as i32, id],
        )?;
        Ok(())
    }

    pub fn update_access_key_models(
        &self,
        id: &str,
        allowed_models: Option<Vec<String>>,
    ) -> Result<(), AppError> {
        let conn = lock_conn!(self.conn);
        let allowed_models_json = serialize_allowed_models(&allowed_models)?;
        conn.execute(
            "UPDATE access_keys SET allowed_models = ?1 WHERE id = ?2",
            rusqlite::params![allowed_models_json, id],
        )?;
        Ok(())
    }

    pub fn find_access_key_by_key(&self, key: &str) -> Result<Option<AccessKey>, AppError> {
        let conn = lock_conn!(self.conn);
        let result = conn.query_row(
            "SELECT id, name, key, enabled, created_at, allowed_models FROM access_keys WHERE key = ?1",
            [key],
            |row| {
                let enabled: i32 = row.get(3)?;
                Ok(AccessKey {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    key: row.get(2)?,
                    enabled: enabled != 0,
                    created_at: row.get(4)?,
                    allowed_models: parse_allowed_models(row.get(5)?)?,
                })
            },
        );

        match result {
            Ok(ak) => Ok(Some(ak)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(AppError::Database(e.to_string())),
        }
    }
}

fn parse_allowed_models(value: Option<String>) -> rusqlite::Result<Option<Vec<String>>> {
    let Some(value) = value else {
        return Ok(None);
    };
    if value.trim().is_empty() {
        return Ok(None);
    }
    serde_json::from_str::<Vec<String>>(&value)
        .map(Some)
        .map_err(|err| rusqlite::Error::FromSqlConversionFailure(
            0,
            rusqlite::types::Type::Text,
            Box::new(err),
        ))
}

fn serialize_allowed_models(value: &Option<Vec<String>>) -> Result<Option<String>, AppError> {
    value
        .as_ref()
        .map(|models| {
            serde_json::to_string(models).map_err(|err| AppError::Database(err.to_string()))
        })
        .transpose()
}

#[cfg(test)]
mod tests {
    use super::AccessKey;

    fn key_with_models(allowed_models: Option<Vec<&str>>) -> AccessKey {
        AccessKey {
            id: "key-1".to_string(),
            name: "test".to_string(),
            key: "sk-test".to_string(),
            enabled: true,
            created_at: 0,
            allowed_models: allowed_models
                .map(|models| models.into_iter().map(str::to_string).collect()),
        }
    }

    #[test]
    fn none_allowed_models_means_all_models() {
        let key = key_with_models(None);

        assert!(key.allows_model("gpt-4o"));
        assert!(key.allows_model("client-alias"));
    }

    #[test]
    fn empty_allowed_models_means_no_models() {
        let key = key_with_models(Some(vec![]));

        assert!(!key.allows_model("gpt-4o"));
    }

    #[test]
    fn allowed_models_match_downstream_model_exactly() {
        let key = key_with_models(Some(vec!["client-alias"]));

        assert!(key.allows_model("client-alias"));
        assert!(!key.allows_model("provider-real-model"));
        assert!(!key.allows_model("CLIENT-ALIAS"));
    }
}
