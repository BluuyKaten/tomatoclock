//! 用户仓储（users 表）

use crate::db::DbPool;
use crate::domain::entities::User;
use crate::error::AppResult;
use crate::repository::{now_ms, DbConn};

pub struct UserRepo;

impl UserRepo {
    /// 创建用户（argon2 哈希由调用方完成）
    pub fn create(pool: &DbPool, username: &str, password_hash: &str) -> AppResult<i64> {
        let conn = pool.conn()?;
        let now = now_ms();
        conn.execute(
            "INSERT INTO users (username, password_hash, salt, created_at, updated_at)
             VALUES (?1, ?2, '', ?3, ?3)",
            [username, password_hash, &now.to_string()],
        )?;
        Ok(conn.last_insert_rowid())
    }

    /// 按用户名查找
    pub fn find_by_username(pool: &DbPool, username: &str) -> AppResult<Option<User>> {
        let conn = pool.conn()?;
        let mut stmt = conn.prepare(
            "SELECT * FROM users WHERE username = ?1 LIMIT 1",
        )?;
        let mut rows = stmt.query_map([username], User::from_row)?;
        Ok(rows.next().transpose()?)
    }

    /// 按 id 查找
    pub fn find_by_id(pool: &DbPool, id: i64) -> AppResult<Option<User>> {
        let conn = pool.conn()?;
        let mut stmt = conn.prepare("SELECT * FROM users WHERE id = ?1 LIMIT 1")?;
        let mut rows = stmt.query_map([id], User::from_row)?;
        Ok(rows.next().transpose()?)
    }

    /// 更新最近登录时间
    pub fn touch_login(pool: &DbPool, id: i64) -> AppResult<()> {
        let conn = pool.conn()?;
        let now = now_ms();
        conn.execute(
            "UPDATE users SET last_login_at = ?1, updated_at = ?1 WHERE id = ?2",
            [&now.to_string(), &id.to_string()],
        )?;
        Ok(())
    }
}
