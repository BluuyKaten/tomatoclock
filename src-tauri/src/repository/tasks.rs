//! 任务仓储（tasks 表）

use crate::db::DbPool;
use crate::domain::entities::Task;
use crate::error::AppResult;
use crate::repository::{collect_rows, now_ms, DbConn};

pub struct TaskRepo;

impl TaskRepo {
    pub fn create(
        pool: &DbPool,
        user_id: i64,
        subject_id: Option<i64>,
        title: &str,
        estimate_pomos: i32,
    ) -> AppResult<i64> {
        let conn = pool.conn()?;
        let now = now_ms();
        conn.execute(
            "INSERT INTO tasks (user_id, subject_id, title, estimate_pomos, status, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, 0, ?5, ?5)",
            rusqlite::params![
                user_id,
                subject_id,
                title,
                estimate_pomos,
                now,
            ],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn list_by_user(pool: &DbPool, user_id: i64) -> AppResult<Vec<Task>> {
        let conn = pool.conn()?;
        let mut stmt = conn.prepare(
            "SELECT * FROM tasks WHERE user_id = ?1 ORDER BY created_at DESC",
        )?;
        let rows = stmt.query_map([user_id], Task::from_row)?;
        collect_rows(rows)
    }

    pub fn find_by_id(pool: &DbPool, id: i64) -> AppResult<Option<Task>> {
        let conn = pool.conn()?;
        let mut stmt = conn.prepare("SELECT * FROM tasks WHERE id = ?1")?;
        let mut rows = stmt.query_map([id], Task::from_row)?;
        Ok(rows.next().transpose()?)
    }

    pub fn update_status(pool: &DbPool, id: i64, status: i32) -> AppResult<bool> {
        let conn = pool.conn()?;
        let now = now_ms();
        let n = conn.execute(
            "UPDATE tasks SET status = ?1, updated_at = ?2,
             completed_at = CASE WHEN ?1 = 2 THEN ?2 ELSE completed_at END
             WHERE id = ?3",
            rusqlite::params![status, now, id],
        )?;
        Ok(n > 0)
    }
}
