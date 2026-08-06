//! 数据库连接池（r2d2 + rusqlite）
//!
//! 职责：
//! - 在 Tauri 应用数据目录下创建/打开 SQLite 文件
//! - 启用 WAL 模式提升并发读性能
//! - 启用外键约束（PRAGMA foreign_keys = ON）
//! - 执行迁移脚本

use std::path::PathBuf;
use std::sync::Arc;

use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::Connection;
use tauri::{AppHandle, Manager};

use crate::error::AppResult;

/// 数据库连接池类型别名
pub type DbPool = Arc<Pool<SqliteConnectionManager>>;

/// 在 Tauri 应用数据目录下创建数据库连接池
pub fn create_pool(app: &AppHandle) -> AppResult<DbPool> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| crate::error::AppError::Internal(format!("无法获取应用数据目录: {e}")))?;
    std::fs::create_dir_all(&dir)?;
    let db_path = dir.join("tomatoclock.db");

    tracing::info!(path = %db_path.display(), "打开数据库");

    let manager = SqliteConnectionManager::file(&db_path)
        // 每次新建连接时启用外键与 WAL
        .with_init(|conn: &mut Connection| {
            conn.execute_batch(
                "PRAGMA foreign_keys = ON;
                 PRAGMA journal_mode = WAL;
                 PRAGMA synchronous = NORMAL;",
            )
        });

    let pool = Pool::builder()
        .max_size(8)
        .connection_timeout(std::time::Duration::from_secs(30))
        .build(manager)
        .map_err(|e| crate::error::AppError::Internal(format!("连接池创建失败: {e}")))?;

    // 执行迁移
    let conn = pool.get().map_err(|e| {
        crate::error::AppError::Internal(format!("获取连接失败: {e}"))
    })?;
    // [FIX] 使用完整路径调用 migration 模块
    crate::db::migration::run_migrations(&conn)?;

    Ok(Arc::new(pool))
}

/// 便捷函数：取应用数据目录下子路径
pub fn app_data_dir(app: &AppHandle) -> AppResult<PathBuf> {
    app.path()
        .app_data_dir()
        .map_err(|e| crate::error::AppError::Internal(format!("无法获取应用数据目录: {e}")))
}

pub fn ensure_dir(p: &PathBuf) -> AppResult<()> {
    std::fs::create_dir_all(p)?;
    Ok(())
}
