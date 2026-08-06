//! 用户配置仓储（app_settings KV 表）

use crate::db::DbPool;
use crate::error::AppResult;
use crate::repository::{now_ms, DbConn};
use std::collections::HashMap;

pub struct AppSettingRepo;

impl AppSettingRepo {
    /// 取单个配置
    pub fn get(pool: &DbPool, user_id: i64, key: &str) -> AppResult<Option<String>> {
        let conn = pool.conn()?;
        let mut stmt = conn.prepare(
            "SELECT value FROM app_settings WHERE user_id = ?1 AND key = ?2",
        )?;
        let mut rows = stmt.query_map(rusqlite::params![user_id, key], |row| {
            row.get::<_, Option<String>>(0)
        })?;
        Ok(rows.next().transpose()?.flatten())
    }

    /// 批量取所有配置
    pub fn get_all(pool: &DbPool, user_id: i64) -> AppResult<HashMap<String, Option<String>>> {
        let conn = pool.conn()?;
        let mut stmt = conn.prepare(
            "SELECT key, value FROM app_settings WHERE user_id = ?1",
        )?;
        let rows = stmt.query_map([user_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?))
        })?;
        let mut map = HashMap::new();
        for r in rows {
            let (k, v) = r?;
            map.insert(k, v);
        }
        Ok(map)
    }

    /// 写入/更新单个配置
    pub fn set(pool: &DbPool, user_id: i64, key: &str, value: Option<&str>) -> AppResult<()> {
        let conn = pool.conn()?;
        let now = now_ms();
        conn.execute(
            "INSERT INTO app_settings (user_id, key, value, updated_at)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(user_id, key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
            rusqlite::params![user_id, key, value, now],
        )?;
        Ok(())
    }

    /// 批量更新
    pub fn set_many(
        pool: &DbPool,
        user_id: i64,
        kvs: &HashMap<String, Option<String>>,
    ) -> AppResult<Vec<String>> {
        let conn = pool.conn()?;
        let tx = conn.unchecked_transaction()?;
        let now = now_ms();
        let mut updated = Vec::new();
        for (k, v) in kvs {
            tx.execute(
                "INSERT INTO app_settings (user_id, key, value, updated_at)
                 VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(user_id, key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
                rusqlite::params![user_id, k, v, now],
            )?;
            updated.push(k.clone());
        }
        tx.commit()?;
        Ok(updated)
    }
}
