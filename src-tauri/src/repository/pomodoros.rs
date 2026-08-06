//! 番茄时段仓储（pomodoros 表）—— 核心业务表

use crate::db::DbPool;
use crate::domain::entities::Pomodoro;
use crate::error::AppResult;
use crate::repository::{collect_rows, now_ms, DbConn};

pub struct PomodoroRepo;

impl PomodoroRepo {
    /// 创建进行中的番茄
    pub fn create(
        pool: &DbPool,
        user_id: i64,
        task_id: Option<i64>,
        subject_id: Option<i64>,
        planned_duration: i64,
    ) -> AppResult<i64> {
        let conn = pool.conn()?;
        let now = now_ms();
        conn.execute(
            "INSERT INTO pomodoros
             (user_id, task_id, subject_id, started_at, planned_duration, status, distraction_count, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, 0, 0, ?4)",
            rusqlite::params![user_id, task_id, subject_id, now, planned_duration],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn find_by_id(pool: &DbPool, id: i64) -> AppResult<Option<Pomodoro>> {
        let conn = pool.conn()?;
        let mut stmt = conn.prepare("SELECT * FROM pomodoros WHERE id = ?1")?;
        let mut rows = stmt.query_map([id], Pomodoro::from_row)?;
        Ok(rows.next().transpose()?)
    }

    /// 取用户当前进行中的番茄（按 started_at 倒序取最近一条 status=0）
    pub fn find_current(pool: &DbPool, user_id: i64) -> AppResult<Option<Pomodoro>> {
        let conn = pool.conn()?;
        let mut stmt = conn.prepare(
            "SELECT * FROM pomodoros
             WHERE user_id = ?1 AND status = 0
             ORDER BY started_at DESC LIMIT 1",
        )?;
        let mut rows = stmt.query_map([user_id], Pomodoro::from_row)?;
        Ok(rows.next().transpose()?)
    }

    /// 更新状态与实际时长（完成/放弃/打断）
    pub fn finish(
        pool: &DbPool,
        id: i64,
        status: i32,
        ended_at: i64,
        actual_duration: i64,
        distraction_count: i32,
    ) -> AppResult<bool> {
        let conn = pool.conn()?;
        let n = conn.execute(
            "UPDATE pomodoros
             SET status = ?1, ended_at = ?2, actual_duration = ?3, distraction_count = ?4
             WHERE id = ?5",
            rusqlite::params![status, ended_at, actual_duration, distraction_count, id],
        )?;
        Ok(n > 0)
    }

    /// 累加分心次数（番茄进行中每次分心 +1）
    pub fn increment_distraction_count(pool: &DbPool, id: i64) -> AppResult<()> {
        let conn = pool.conn()?;
        conn.execute(
            "UPDATE pomodoros SET distraction_count = distraction_count + 1 WHERE id = ?1",
            [&id.to_string()],
        )?;
        Ok(())
    }

    /// 回填 note_id（番茄结束关联笔记）
    pub fn set_note_id(pool: &DbPool, id: i64, note_id: i64) -> AppResult<()> {
        let conn = pool.conn()?;
        conn.execute(
            "UPDATE pomodoros SET note_id = ?1 WHERE id = ?2",
            [&note_id.to_string(), &id.to_string()],
        )?;
        Ok(())
    }

    /// 时间范围内列表（统计用）
    pub fn list_by_time_range(
        pool: &DbPool,
        user_id: i64,
        from: i64,
        to: i64,
    ) -> AppResult<Vec<Pomodoro>> {
        let conn = pool.conn()?;
        let mut stmt = conn.prepare(
            "SELECT * FROM pomodoros
             WHERE user_id = ?1 AND started_at BETWEEN ?2 AND ?3
             ORDER BY started_at ASC",
        )?;
        let rows = stmt.query_map(rusqlite::params![user_id, from, to], Pomodoro::from_row)?;
        collect_rows(rows)
    }
}
