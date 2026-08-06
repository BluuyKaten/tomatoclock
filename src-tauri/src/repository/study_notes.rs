//! 学习笔记仓储（study_notes + note_images 表）

use crate::db::DbPool;
use crate::domain::entities::{NoteImage, StudyNote};
use crate::error::AppResult;
use crate::repository::{collect_rows, now_ms, DbConn};

pub struct StudyNoteRepo;

impl StudyNoteRepo {
    /// 创建笔记，并批量关联图片路径
    /// 注意：image_paths 必须已由应用复制到应用数据目录下（见设计问题 #9）
    pub fn create(
        pool: &DbPool,
        user_id: i64,
        pomodoro_id: Option<i64>,
        subject_id: Option<i64>,
        title: Option<&str>,
        content: &str,
        tags_json: Option<&str>,
        image_paths: &[String],
    ) -> AppResult<i64> {
        let conn = pool.conn()?;
        let tx = conn.unchecked_transaction()?;
        let now = now_ms();

        tx.execute(
            "INSERT INTO study_notes
             (user_id, pomodoro_id, subject_id, title, content, tags, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)",
            rusqlite::params![user_id, pomodoro_id, subject_id, title, content, tags_json, now],
        )?;
        let note_id = tx.last_insert_rowid();

        for path in image_paths {
            tx.execute(
                "INSERT INTO note_images (note_id, file_path, created_at) VALUES (?1, ?2, ?3)",
                rusqlite::params![note_id, path, now],
            )?;
        }

        tx.commit()?;
        Ok(note_id)
    }

    /// 查询笔记列表（带图片、按时间倒序）
    pub fn list(
        pool: &DbPool,
        user_id: i64,
        subject_id: Option<i64>,
        tag_contains: Option<&str>,
        from: Option<i64>,
        to: Option<i64>,
        page: i64,
        page_size: i64,
    ) -> AppResult<(i64, Vec<(StudyNote, Vec<NoteImage>)>)> {
        let conn = pool.conn()?;

        // 组装动态 WHERE
        let mut clauses = vec!["n.user_id = ?".to_string()];
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(user_id)];
        let mut pindex = 2;

        if subject_id.is_some() {
            clauses.push(format!("n.subject_id = ?{pindex}"));
            params.push(Box::new(subject_id));
            pindex += 1;
        }
        if let Some(from_ms) = from {
            clauses.push(format!("n.created_at >= ?{pindex}"));
            params.push(Box::new(from_ms));
            pindex += 1;
        }
        if let Some(to_ms) = to {
            clauses.push(format!("n.created_at <= ?{pindex}"));
            params.push(Box::new(to_ms));
            pindex += 1;
        }
        if let Some(tag) = tag_contains {
            // tags 为 JSON 数组字符串，使用 LIKE 子串匹配
            clauses.push(format!("n.tags LIKE ?{pindex}"));
            params.push(Box::new(format!("%{tag}%")));
            pindex += 1;
        }

        let where_sql = clauses.join(" AND ");
        let offset = (page - 1).max(0) * page_size;

        // 总数
        let total: i64 = conn.query_row(
            &format!("SELECT COUNT(*) FROM study_notes n WHERE {where_sql}"),
            rusqlite::params_from_iter(params.iter().map(|p| p.as_ref())),
            |row| row.get(0),
        )?;

        // 分页查笔记
        let mut stmt = conn.prepare(&format!(
            "SELECT * FROM study_notes n WHERE {where_sql}
             ORDER BY n.created_at DESC LIMIT ?{pindex} OFFSET ?{}",
            pindex + 1
        ))?;
        params.push(Box::new(page_size));
        params.push(Box::new(offset));

        let rows = stmt.query_map(
            rusqlite::params_from_iter(params.iter().map(|p| p.as_ref())),
            StudyNote::from_row,
        )?;

        let mut out = Vec::new();
        for r in rows {
            let note = r?;
            let images = Self::list_images_internal(&conn, note.id)?;
            out.push((note, images));
        }

        Ok((total, out))
    }

    pub fn find_by_id(pool: &DbPool, id: i64) -> AppResult<Option<(StudyNote, Vec<NoteImage>)>> {
        let conn = pool.conn()?;
        let mut stmt = conn.prepare("SELECT * FROM study_notes WHERE id = ?1")?;
        let mut rows = stmt.query_map([id], StudyNote::from_row)?;
        if let Some(note) = rows.next().transpose()? {
            let images = Self::list_images_internal(&conn, note.id)?;
            Ok(Some((note, images)))
        } else {
            Ok(None)
        }
    }

    pub fn update(
        pool: &DbPool,
        id: i64,
        title: Option<&str>,
        content: Option<&str>,
        tags_json: Option<&str>,
    ) -> AppResult<bool> {
        let conn = pool.conn()?;
        let now = now_ms();
        let mut sets = vec!["updated_at = ?".to_string()];
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(now)];
        let mut idx = 2;

        if title.is_some() {
            sets.push(format!("title = ?{idx}"));
            params.push(Box::new(title.unwrap().to_string()));
            idx += 1;
        }
        if content.is_some() {
            sets.push(format!("content = ?{idx}"));
            params.push(Box::new(content.unwrap().to_string()));
            idx += 1;
        }
        if tags_json.is_some() {
            sets.push(format!("tags = ?{idx}"));
            params.push(Box::new(tags_json.unwrap().to_string()));
        }

        let sql = format!("UPDATE study_notes SET {} WHERE id = ?", sets.join(", "));
        params.push(Box::new(id));

        let n = conn.execute(&sql, rusqlite::params_from_iter(params.iter().map(|p| p.as_ref())))?;
        Ok(n > 0)
    }

    pub fn delete(pool: &DbPool, id: i64) -> AppResult<bool> {
        let conn = pool.conn()?;
        let n = conn.execute("DELETE FROM study_notes WHERE id = ?1", [&id])?;
        Ok(n > 0)
    }

    fn list_images_internal(conn: &rusqlite::Connection, note_id: i64) -> AppResult<Vec<NoteImage>> {
        let mut stmt = conn.prepare(
            "SELECT * FROM note_images WHERE note_id = ?1 ORDER BY id ASC",
        )?;
        let rows = stmt.query_map([note_id], NoteImage::from_row)?;
        collect_rows(rows)
    }
}
