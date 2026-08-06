//! 分心事件仓储（distractions 表）

use crate::db::DbPool;
use crate::domain::entities::Distraction;
use crate::error::AppResult;
use crate::repository::{collect_rows, now_ms, DbConn};

pub struct DistractionRepo;

impl DistractionRepo {
    pub fn create(
        pool: &DbPool,
        pomodoro_id: i64,
        user_id: i64,
        distraction_type: i32,
        app_name: Option<&str>,
        app_wm_class: Option<&str>,
        window_title: Option<&str>,
        idle_seconds: Option<i64>,
        face_detected: Option<bool>,
        gaze_left: Option<bool>,
        reminder_level: i32,
    ) -> AppResult<i64> {
        let conn = pool.conn()?;
        let now = now_ms();
        conn.execute(
            "INSERT INTO distractions
             (pomodoro_id, user_id, detected_at, distraction_type,
              app_name, app_wm_class, window_title, idle_seconds,
              face_detected, gaze_left, reminder_level, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?3)",
            rusqlite::params![
                pomodoro_id,
                user_id,
                now,
                distraction_type,
                app_name,
                app_wm_class,
                window_title,
                idle_seconds,
                face_detected.map(|b| if b { 1 } else { 0 }),
                gaze_left.map(|b| if b { 1 } else { 0 }),
                reminder_level,
            ],
        )?;
        Ok(conn.last_insert_rowid())
    }

    /// 某番茄的分心事件
    pub fn list_by_pomodoro(pool: &DbPool, pomodoro_id: i64) -> AppResult<Vec<Distraction>> {
        let conn = pool.conn()?;
        let mut stmt = conn.prepare(
            "SELECT * FROM distractions WHERE pomodoro_id = ?1 ORDER BY detected_at ASC",
        )?;
        let rows = stmt.query_map([pomodoro_id], Distraction::from_row)?;
        collect_rows(rows)
    }

    /// 时间范围内列表（统计用）
    pub fn list_by_time_range(
        pool: &DbPool,
        user_id: i64,
        from: i64,
        to: i64,
    ) -> AppResult<Vec<Distraction>> {
        let conn = pool.conn()?;
        let mut stmt = conn.prepare(
            "SELECT * FROM distractions
             WHERE user_id = ?1 AND detected_at BETWEEN ?2 AND ?3
             ORDER BY detected_at ASC",
        )?;
        let rows = stmt.query_map(rusqlite::params![user_id, from, to], Distraction::from_row)?;
        collect_rows(rows)
    }

    /// 应用维度聚合（分心热点）
    pub fn count_by_app(
        pool: &DbPool,
        user_id: i64,
        from: i64,
        to: i64,
    ) -> AppResult<Vec<(Option<String>, i64)>> {
        let conn = pool.conn()?;
        let mut stmt = conn.prepare(
            "SELECT app_name, COUNT(*) AS c FROM distractions
             WHERE user_id = ?1 AND detected_at BETWEEN ?2 AND ?3
             GROUP BY app_name ORDER BY c DESC LIMIT 20",
        )?;
        let rows = stmt.query_map(rusqlite::params![user_id, from, to], |row| {
            Ok((row.get::<_, Option<String>>(0)?, row.get::<_, i64>(1)?))
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    /// 小时维度聚合（0-23）
    pub fn count_by_hour(
        pool: &DbPool,
        user_id: i64,
        from: i64,
        to: i64,
    ) -> AppResult<Vec<(i32, i64)>> {
        let conn = pool.conn()?;
        // 取 detected_at 本地小时：(ms/1000 + 时区偏移) % 86400 / 3600
        // 简化：依赖应用层传入的 from/to 已是本地时间；这里按 UTC 小时近似统计
        // TODO(设计待确认)：by_hour 时区未明确；当前按 UTC 小时输出
        let mut stmt = conn.prepare(
            "SELECT CAST(strftime('%H', detected_at/1000, 'unixepoch') AS INTEGER) AS h,
                    COUNT(*) AS c
             FROM distractions
             WHERE user_id = ?1 AND detected_at BETWEEN ?2 AND ?3
             GROUP BY h ORDER BY h ASC",
        )?;
        let rows = stmt.query_map(rusqlite::params![user_id, from, to], |row| {
            Ok((row.get::<_, i32>(0)?, row.get::<_, i64>(1)?))
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    /// 类型维度聚合
    pub fn count_by_type(
        pool: &DbPool,
        user_id: i64,
        from: i64,
        to: i64,
    ) -> AppResult<Vec<(i32, i64)>> {
        let conn = pool.conn()?;
        let mut stmt = conn.prepare(
            "SELECT distraction_type, COUNT(*) AS c FROM distractions
             WHERE user_id = ?1 AND detected_at BETWEEN ?2 AND ?3
             GROUP BY distraction_type ORDER BY distraction_type ASC",
        )?;
        let rows = stmt.query_map(rusqlite::params![user_id, from, to], |row| {
            Ok((row.get::<_, i32>(0)?, row.get::<_, i64>(1)?))
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }
}
