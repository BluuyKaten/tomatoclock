//! 会话令牌仓储（session_tokens 表）
//!
//! 后端只存 SHA-256 哈希，不存明文。明文 token 仅在创建时返回给前端一次。

use crate::db::DbPool;
use crate::error::AppResult;
use crate::repository::{now_ms, DbConn};

pub struct SessionTokenRepo;

impl SessionTokenRepo {
    /// 创建令牌记录，返回 (token_hash, expires_at)
    pub fn create(
        pool: &DbPool,
        user_id: i64,
        token_hash: &str,
        device_info: Option<&str>,
        ttl_days: i64,
    ) -> AppResult<i64> {
        let conn = pool.conn()?;
        let now = now_ms();
        let expires = now + ttl_days * 24 * 3600 * 1000;
        conn.execute(
            "INSERT INTO session_tokens (user_id, token_hash, device_info, created_at, expires_at, last_used_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            [
                &user_id.to_string(),
                token_hash,
                device_info.unwrap_or(""),
                &now.to_string(),
                &expires.to_string(),
                &now.to_string(),
            ],
        )?;
        Ok(conn.last_insert_rowid())
    }

    /// 按哈希查找未过期的令牌；命中则刷新 last_used_at 与 expires_at（滚动过期）
    pub fn find_valid_and_refresh(
        pool: &DbPool,
        token_hash: &str,
        ttl_days: i64,
    ) -> AppResult<Option<i64>> {
        let conn = pool.conn()?;
        let now = now_ms();
        let mut stmt = conn.prepare(
            "SELECT id, user_id, expires_at FROM session_tokens WHERE token_hash = ?1 LIMIT 1",
        )?;
        let mut rows = stmt.query_map([token_hash], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?, row.get::<_, i64>(2)?))
        })?;
        let Some((id, user_id, expires_at)) = rows.next().transpose()? else {
            return Ok(None);
        };
        drop(rows);

        if expires_at < now {
            // 已过期：删除并返回 None
            conn.execute("DELETE FROM session_tokens WHERE id = ?1", [&id.to_string()])?;
            return Ok(None);
        }

        // 滚动刷新过期时间
        let new_expires = now + ttl_days * 24 * 3600 * 1000;
        conn.execute(
            "UPDATE session_tokens SET last_used_at = ?1, expires_at = ?2 WHERE id = ?3",
            [&now.to_string(), &new_expires.to_string(), &id.to_string()],
        )?;
        Ok(Some(user_id))
    }

    /// 删除某用户的所有令牌（登出 / 改密码等场景）
    pub fn delete_by_user(pool: &DbPool, user_id: i64) -> AppResult<usize> {
        let conn = pool.conn()?;
        let n = conn.execute(
            "DELETE FROM session_tokens WHERE user_id = ?1",
            [&user_id.to_string()],
        )?;
        Ok(n)
    }
}
