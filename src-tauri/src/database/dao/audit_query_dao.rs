use crate::database::{lock_conn, Database};
use crate::error::AppError;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct AuditLogItem {
    pub id: i64,
    pub action: String,
    pub detail: String,
    pub created_at: i64,
}

impl Database {
    pub fn list_audit_logs(&self, limit: usize) -> Result<Vec<AuditLogItem>, AppError> {
        let conn = lock_conn!(self.conn);
        let mut stmt = conn.prepare(
            "SELECT id, action, detail, created_at FROM audit_log ORDER BY id DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map([limit as i64], |row| {
            Ok(AuditLogItem {
                id: row.get(0)?,
                action: row.get(1)?,
                detail: row.get(2)?,
                created_at: row.get(3)?,
            })
        })?;

        Ok(rows.filter_map(|row| row.ok()).collect())
    }
}
