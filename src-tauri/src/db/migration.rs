//! 数据库迁移执行器
//!
//! 采用按版本号顺序执行的 SQL 脚本：migrations/001_init.sql、002_xxx.sql ...
//! 通过 schema_migrations 表记录已应用的版本，幂等可重复执行。

use rusqlite::Connection;

use crate::error::AppResult;

// [FIX] 内置迁移脚本（按版本顺序）—— 使用 // 注释而非 //! 内部文档注释
const MIGRATIONS: &[(&str, &str)] = &[
    ("001", include_str!("../../migrations/001_init.sql")),
    ("002", include_str!("../../migrations/002_session_tokens.sql")),
];

/// 执行所有未应用的迁移（幂等）
pub fn run_migrations(conn: &Connection) -> AppResult<()> {
    // 建迁移记录表
    conn.execute(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
            version TEXT PRIMARY KEY,
            applied_at INTEGER NOT NULL
        )",
        [],
    )?;

    for (version, sql) in MIGRATIONS {
        let applied: bool = conn
            .query_row(
                "SELECT 1 FROM schema_migrations WHERE version = ?1",
                [version],
                |row| row.get::<_, i64>(0),
            )
            .is_ok();

        if applied {
            tracing::debug!(version, "迁移已应用，跳过");
            continue;
        }

        tracing::info!(version, "应用迁移");

        // 单条执行多个语句（rusqlite 的 execute_batch 支持分号分隔）
        conn.execute_batch(sql).map_err(|e| {
            crate::error::AppError::Internal(format!("迁移 {version} 失败: {e}"))
        })?;

        let now = chrono::Utc::now().timestamp_millis();
        conn.execute(
            "INSERT INTO schema_migrations (version, applied_at) VALUES (?1, ?2)",
            // [FIX] rusqlite 需要 &dyn ToSql，&String 不匹配，需转成 &&str 或直接用 &str
            [version, &now.to_string() as &str],
        )?;
    }

    Ok(())
}
