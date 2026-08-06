// 仓储层：每个表一个 repository，统一 user_id 隔离
pub mod app_rules;
pub mod app_settings;
pub mod distractions;
pub mod pomodoros;
pub mod session_tokens;
pub mod study_notes;
pub mod subjects;
pub mod tasks;
pub mod users;

use crate::db::DbPool;
use crate::error::AppResult;
use r2d2_sqlite::SqliteConnectionManager;

/// 数据库访问辅助 trait：从连接池取连接
pub trait DbConn {
    fn conn(&self) -> AppResult<r2d2::PooledConnection<SqliteConnectionManager>>;
}

impl DbConn for DbPool {
    fn conn(&self) -> AppResult<r2d2::PooledConnection<SqliteConnectionManager>> {
        self.get()
            .map_err(|e| crate::error::AppError::Internal(format!("获取连接失败: {e}")))
    }
}

/// 当前时间（Unix 毫秒）
pub fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

/// 通用收集辅助：把 query_map 迭代器收集为 Vec
pub fn collect_rows<T, F>(rows: rusqlite::MappedRows<F>) -> AppResult<Vec<T>>
where
    F: FnMut(&rusqlite::Row) -> rusqlite::Result<T>,
{
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(crate::error::AppError::Db)?);
    }
    Ok(out)
}
