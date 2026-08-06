//! 领域实体（与数据库表一一对应）
//!
//! 所有时间字段均为 Unix 毫秒（i64），与上游 §6 规范一致。

use chrono::DateTime;
use rusqlite::Row;
use serde::{Deserialize, Serialize};

/// 用户
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: i64,
    pub username: String,
    pub password_hash: String,
    pub salt: String,
    pub avatar_path: Option<String>,
    pub is_cloud_bound: bool,
    pub last_login_at: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
}

/// 科目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Subject {
    pub id: i64,
    pub user_id: i64,
    pub name: String,
    pub color: Option<String>,
    pub sort_order: i32,
    pub created_at: i64,
}

/// 任务
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: i64,
    pub user_id: i64,
    pub subject_id: Option<i64>,
    pub title: String,
    pub estimate_pomos: i32,
    pub status: i32, // 0 待办 / 1 进行中 / 2 完成 / 3 归档
    pub due_at: Option<i64>,
    pub completed_at: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
}

/// 番茄时段记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pomodoro {
    pub id: i64,
    pub user_id: i64,
    pub task_id: Option<i64>,
    pub subject_id: Option<i64>,
    pub started_at: i64,
    pub ended_at: Option<i64>,
    pub planned_duration: i64, // 秒
    pub actual_duration: Option<i64>,
    pub status: i32, // 0 进行中 / 1 完成 / 2 放弃 / 3 打断
    pub distraction_count: i32,
    pub note_id: Option<i64>,
    pub created_at: i64,
}

/// 分心事件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Distraction {
    pub id: i64,
    pub pomodoro_id: i64,
    pub user_id: i64,
    pub detected_at: i64,
    pub distraction_type: i32, // 1 窗口 / 2 输入空闲 / 3 摄像头
    pub app_name: Option<String>,
    pub app_wm_class: Option<String>,
    pub window_title: Option<String>,
    pub idle_seconds: Option<i64>,
    pub face_detected: Option<bool>,
    pub gaze_left: Option<bool>,
    pub reminder_level: i32,
    pub created_at: i64,
}

/// 学习笔记
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StudyNote {
    pub id: i64,
    pub user_id: i64,
    pub pomodoro_id: Option<i64>,
    pub subject_id: Option<i64>,
    pub title: Option<String>,
    pub content: String,
    pub tags: Option<String>, // JSON 数组字符串
    pub created_at: i64,
    pub updated_at: i64,
}

/// 笔记图片
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoteImage {
    pub id: i64,
    pub note_id: i64,
    pub file_path: String,
    pub mime_type: Option<String>,
    pub size_bytes: Option<i64>,
    pub created_at: i64,
}

/// 应用分心规则
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppRule {
    pub id: i64,
    pub user_id: i64,
    pub rule_type: i32, // 1 黑名单 / 2 白名单
    pub app_name: String,
    pub window_title_pattern: Option<String>,
    pub is_enabled: bool,
    pub created_at: i64,
}

/// 应用配置项（KV）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSetting {
    pub user_id: i64,
    pub key: String,
    pub value: Option<String>,
    pub updated_at: i64,
}

/// 同步状态（V2 预留）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncState {
    pub user_id: i64,
    pub last_sync_at: Option<i64>,
    pub device_id: Option<String>,
    pub cursor: Option<String>,
    pub updated_at: i64,
}

// =============================================================================
// Row 映射辅助：统一从 rusqlite::Row 构造实体
// =============================================================================

// [FIX] 将 ms_to_iso 标记为 pub，供其它模块使用
pub fn ms_to_iso(ms: i64) -> String {
    let secs = ms / 1000;
    let nsecs = ((ms % 1000) * 1_000_000) as u32;
    // [FIX] DateTime::from_timestamp 替代已弃用的 NaiveDateTime::from_timestamp_opt
    DateTime::from_timestamp(secs, nsecs)
        .map(|dt: DateTime<chrono::Utc>| dt.format("%Y-%m-%dT%H:%M:%S%.3f").to_string())
        .unwrap_or_default()
}

fn opt_ms(row: &Row, idx: &str) -> rusqlite::Result<Option<i64>> {
    row.get::<_, Option<i64>>(idx)
}

impl User {
    pub fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get("id")?,
            username: row.get("username")?,
            password_hash: row.get("password_hash")?,
            salt: row.get("salt")?,
            avatar_path: row.get("avatar_path")?,
            is_cloud_bound: row.get::<_, i64>("is_cloud_bound")? != 0,
            last_login_at: opt_ms(row, "last_login_at")?,
            created_at: row.get("created_at")?,
            updated_at: row.get("updated_at")?,
        })
    }
}

impl Subject {
    pub fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get("id")?,
            user_id: row.get("user_id")?,
            name: row.get("name")?,
            color: row.get("color")?,
            sort_order: row.get("sort_order")?,
            created_at: row.get("created_at")?,
        })
    }
}

impl Task {
    pub fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get("id")?,
            user_id: row.get("user_id")?,
            subject_id: row.get("subject_id")?,
            title: row.get("title")?,
            estimate_pomos: row.get("estimate_pomos")?,
            status: row.get("status")?,
            due_at: opt_ms(row, "due_at")?,
            completed_at: opt_ms(row, "completed_at")?,
            created_at: row.get("created_at")?,
            updated_at: row.get("updated_at")?,
        })
    }
}

impl Pomodoro {
    pub fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get("id")?,
            user_id: row.get("user_id")?,
            task_id: row.get("task_id")?,
            subject_id: row.get("subject_id")?,
            started_at: row.get("started_at")?,
            ended_at: opt_ms(row, "ended_at")?,
            planned_duration: row.get("planned_duration")?,
            actual_duration: opt_ms(row, "actual_duration")?,
            status: row.get("status")?,
            distraction_count: row.get("distraction_count")?,
            note_id: row.get("note_id")?,
            created_at: row.get("created_at")?,
        })
    }
}

impl Distraction {
    pub fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get("id")?,
            pomodoro_id: row.get("pomodoro_id")?,
            user_id: row.get("user_id")?,
            detected_at: row.get("detected_at")?,
            distraction_type: row.get("distraction_type")?,
            app_name: row.get("app_name")?,
            app_wm_class: row.get("app_wm_class")?,
            window_title: row.get("window_title")?,
            idle_seconds: row.get("idle_seconds")?,
            face_detected: row.get::<_, Option<i64>>("face_detected")?.map(|v| v != 0),
            gaze_left: row.get::<_, Option<i64>>("gaze_left")?.map(|v| v != 0),
            reminder_level: row.get("reminder_level")?,
            created_at: row.get("created_at")?,
        })
    }
}

impl StudyNote {
    pub fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get("id")?,
            user_id: row.get("user_id")?,
            pomodoro_id: row.get("pomodoro_id")?,
            subject_id: row.get("subject_id")?,
            title: row.get("title")?,
            content: row.get("content")?,
            tags: row.get("tags")?,
            created_at: row.get("created_at")?,
            updated_at: row.get("updated_at")?,
        })
    }
}

impl NoteImage {
    pub fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get("id")?,
            note_id: row.get("note_id")?,
            file_path: row.get("file_path")?,
            mime_type: row.get("mime_type")?,
            size_bytes: row.get("size_bytes")?,
            created_at: row.get("created_at")?,
        })
    }
}

impl AppRule {
    pub fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get("id")?,
            user_id: row.get("user_id")?,
            rule_type: row.get("rule_type")?,
            app_name: row.get("app_name")?,
            window_title_pattern: row.get("window_title_pattern")?,
            is_enabled: row.get::<_, i64>("is_enabled")? != 0,
            created_at: row.get("created_at")?,
        })
    }
}

impl AppSetting {
    pub fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            user_id: row.get("user_id")?,
            key: row.get("key")?,
            value: row.get("value")?,
            updated_at: row.get("updated_at")?,
        })
    }
}

impl SyncState {
    pub fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            user_id: row.get("user_id")?,
            last_sync_at: opt_ms(row, "last_sync_at")?,
            device_id: row.get("device_id")?,
            cursor: row.get("cursor")?,
            updated_at: row.get("updated_at")?,
        })
    }
}

// [FIX] 删除重复的 use 导入（ms_to_iso 已在本文件定义，无需重新导出）
