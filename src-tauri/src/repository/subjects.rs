//! 科目仓储（subjects 表）

use crate::db::DbPool;
use crate::domain::entities::Subject;
use crate::error::AppResult;
use crate::repository::{now_ms, DbConn};

pub struct SubjectRepo;

impl SubjectRepo {
    pub fn create(pool: &DbPool, user_id: i64, name: &str, color: Option<&str>) -> AppResult<i64> {
        let conn = pool.conn()?;
        let now = now_ms();
        conn.execute(
            "INSERT INTO subjects (user_id, name, color, sort_order, created_at)
             VALUES (?1, ?2, ?3, 0, ?4)",
            [user_id.to_string().as_str(), name, color.unwrap_or(""), &now.to_string()],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn list_by_user(pool: &DbPool, user_id: i64) -> AppResult<Vec<Subject>> {
        let conn = pool.conn()?;
        let mut stmt = conn.prepare(
            "SELECT * FROM subjects WHERE user_id = ?1 ORDER BY sort_order ASC, id ASC",
        )?;
        let rows = stmt.query_map([user_id], Subject::from_row)?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    pub fn find_by_id(pool: &DbPool, id: i64) -> AppResult<Option<Subject>> {
        let conn = pool.conn()?;
        let mut stmt = conn.prepare("SELECT * FROM subjects WHERE id = ?1")?;
        let mut rows = stmt.query_map([id], Subject::from_row)?;
        Ok(rows.next().transpose()?)
    }

    /// 删除（仅当属于该用户）
    pub fn delete(pool: &DbPool, user_id: i64, id: i64) -> AppResult<bool> {
        let conn = pool.conn()?;
        let n = conn.execute(
            "DELETE FROM subjects WHERE id = ?1 AND user_id = ?2",
            [&id.to_string(), &user_id.to_string()],
        )?;
        Ok(n > 0)
    }
}
