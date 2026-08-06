//! 应用分心规则仓储（app_rules 表）

use crate::db::DbPool;
use crate::domain::entities::AppRule;
use crate::error::AppResult;
use crate::repository::{collect_rows, now_ms, DbConn};

pub struct AppRuleRepo;

impl AppRuleRepo {
    pub fn create(
        pool: &DbPool,
        user_id: i64,
        rule_type: i32,
        app_name: &str,
        window_title_pattern: Option<&str>,
        is_enabled: bool,
    ) -> AppResult<i64> {
        let conn = pool.conn()?;
        let now = now_ms();
        conn.execute(
            "INSERT INTO app_rules
             (user_id, rule_type, app_name, window_title_pattern, is_enabled, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![user_id, rule_type, app_name, window_title_pattern, is_enabled as i64, now],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn list_by_user(pool: &DbPool, user_id: i64) -> AppResult<Vec<AppRule>> {
        let conn = pool.conn()?;
        let mut stmt = conn.prepare(
            "SELECT * FROM app_rules WHERE user_id = ?1 ORDER BY id ASC",
        )?;
        let rows = stmt.query_map([user_id], AppRule::from_row)?;
        collect_rows(rows)
    }

    pub fn find_by_id(pool: &DbPool, id: i64) -> AppResult<Option<AppRule>> {
        let conn = pool.conn()?;
        let mut stmt = conn.prepare("SELECT * FROM app_rules WHERE id = ?1")?;
        let mut rows = stmt.query_map([id], AppRule::from_row)?;
        Ok(rows.next().transpose()?)
    }

    pub fn update(
        pool: &DbPool,
        id: i64,
        rule_type: Option<i32>,
        app_name: Option<&str>,
        window_title_pattern: Option<Option<&str>>,
        is_enabled: Option<bool>,
    ) -> AppResult<bool> {
        let conn = pool.conn()?;
        let mut sets = Vec::<String>::new();
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
        let mut idx = 1;

        if let Some(v) = rule_type {
            sets.push(format!("rule_type = ?{idx}"));
            params.push(Box::new(v));
            idx += 1;
        }
        if let Some(v) = app_name {
            sets.push(format!("app_name = ?{idx}"));
            params.push(Box::new(v.to_string()));
            idx += 1;
        }
        if let Some(v) = window_title_pattern {
            sets.push(format!("window_title_pattern = ?{idx}"));
            params.push(Box::new(v.map(|s| s.to_string())));
            idx += 1;
        }
        if let Some(v) = is_enabled {
            sets.push(format!("is_enabled = ?{idx}"));
            params.push(Box::new(v as i64));
        }

        if sets.is_empty() {
            return Ok(false);
        }

        let sql = format!("UPDATE app_rules SET {} WHERE id = ?", sets.join(", "));
        params.push(Box::new(id));
        let n = conn.execute(&sql, rusqlite::params_from_iter(params.iter().map(|p| p.as_ref())))?;
        Ok(n > 0)
    }

    pub fn delete(pool: &DbPool, id: i64) -> AppResult<bool> {
        let conn = pool.conn()?;
        let n = conn.execute("DELETE FROM app_rules WHERE id = ?1", [&id])?;
        Ok(n > 0)
    }
}
