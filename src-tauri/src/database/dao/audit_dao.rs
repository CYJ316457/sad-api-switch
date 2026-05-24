use crate::database::{lock_conn, Database};
use crate::error::AppError;

fn redact_audit_detail(detail: &str) -> String {
    let mut sanitized = detail.to_string();
    let replacements = [
        (r#"(?i)("password"\s*:\s*")([^"]*)(")"#, "$1[redacted]$3"),
        (r#"(?i)("token"\s*:\s*")([^"]*)(")"#, "$1[redacted]$3"),
        (r#"(?i)("api[_ ]?key"\s*:\s*")([^"]*)(")"#, "$1[redacted]$3"),
        (r#"(?i)(password\s*[=:]\s*)([^,;\s]+)"#, "$1[redacted]"),
        (r#"(?i)(token\s*[=:]\s*)([^,;\s]+)"#, "$1[redacted]"),
        (r#"(?i)(api[_ ]?key\s*[=:]\s*)([^,;\s]+)"#, "$1[redacted]"),
        (
            r#"(?i)(authorization\s*:\s*bearer\s+)([^,;\s]+)"#,
            "$1[redacted]",
        ),
        (r#"(?i)(bearer\s+)([^,;\s]+)"#, "$1[redacted]"),
    ];

    for (pattern, replacement) in replacements {
        if let Ok(regex) = regex::Regex::new(pattern) {
            sanitized = regex.replace_all(&sanitized, replacement).into_owned();
        }
    }

    sanitized
}

impl Database {
    pub fn add_audit_log(&self, action: &str, detail: &str) -> Result<(), AppError> {
        let conn = lock_conn!(self.conn);
        let sanitized_detail = redact_audit_detail(detail);
        conn.execute(
            "INSERT INTO audit_log (action, detail, created_at) VALUES (?1, ?2, ?3)",
            rusqlite::params![action, sanitized_detail, chrono::Utc::now().timestamp()],
        )?;
        Ok(())
    }
}
