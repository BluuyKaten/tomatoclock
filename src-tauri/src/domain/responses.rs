//! 响应 DTO（对齐上游 §7 接口出参）
//!
//! 所有 Tauri Command 返回 Result<ApiResponse<T>, AppError>，
//! 前端通过 ApiResponse 的 code/msg/data 字段统一处理。

use serde::Serialize;

/// 通用响应信封
#[derive(Debug, Serialize)]
pub struct ApiResponse<T: Serialize> {
    pub code: i32,
    pub msg: String,
    pub data: T,
}

impl<T: Serialize> ApiResponse<T> {
    pub fn ok(data: T) -> Self {
        Self {
            code: 0,
            msg: "ok".into(),
            data,
        }
    }
}

// ---- 账号 ----

#[derive(Debug, Serialize)]
pub struct AuthUser {
    pub user_id: i64,
    pub username: String,
    pub created_at: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct LoginResult {
    pub user_id: i64,
    pub username: String,
    /// 记住登录时返回明文会话令牌；否则为 None
    pub token: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct AutoLoginResult {
    pub user_id: i64,
    pub username: String,
    /// 刷新后的会话令牌（滚动过期）
    pub token: String,
}

#[derive(Debug, Serialize)]
pub struct LogoutResult {
    pub success: bool,
}

// ---- 番茄 ----

#[derive(Debug, Serialize)]
pub struct PomodoroView {
    pub id: i64,
    pub user_id: i64,
    pub task_id: Option<i64>,
    pub subject_id: Option<i64>,
    pub started_at: i64,
    pub ended_at: Option<i64>,
    pub planned_duration: i64,
    pub actual_duration: Option<i64>,
    pub status: i32,
    pub distraction_count: i32,
    pub note_id: Option<i64>,
    pub created_at: i64,
}

#[derive(Debug, Serialize)]
pub struct StartPomodoroResponse {
    pub id: i64,
    pub started_at: i64,
    pub planned_duration: i64,
    pub status: i32,
}

#[derive(Debug, Serialize)]
pub struct PausePomodoroResponse {
    pub id: i64,
    pub paused_at: i64,
    pub accumulated_seconds: i64,
}

#[derive(Debug, Serialize)]
pub struct ResumePomodoroResponse {
    pub id: i64,
    pub resumed_at: i64,
}

#[derive(Debug, Serialize)]
pub struct CompletePomodoroResponse {
    pub id: i64,
    pub ended_at: i64,
    pub actual_duration: i64,
    pub distraction_count: i32,
}

#[derive(Debug, Serialize)]
pub struct AbandonPomodoroResponse {
    pub id: i64,
    pub status: i32,
}

#[derive(Debug, Serialize)]
pub struct CurrentPomodoroResponse {
    pub id: i64,
    pub started_at: i64,
    pub planned_duration: i64,
    pub remaining_seconds: i64,
    pub status: i32,
    pub distraction_count: i32,
}

// ---- 规则 ----

#[derive(Debug, Serialize)]
pub struct RuleView {
    pub id: i64,
    pub rule_type: i32,
    pub app_name: String,
    pub window_title_pattern: Option<String>,
    pub is_enabled: bool,
}

#[derive(Debug, Serialize)]
pub struct RuleListResponse {
    pub rules: Vec<RuleView>,
}

// ---- 笔记 ----

#[derive(Debug, Serialize)]
pub struct NoteImageView {
    pub id: i64,
    pub file_path: String,
    pub mime_type: Option<String>,
    pub size_bytes: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct NoteView {
    pub id: i64,
    pub user_id: i64,
    pub pomodoro_id: Option<i64>,
    pub subject_id: Option<i64>,
    pub title: Option<String>,
    pub content: String,
    pub tags: Option<Vec<String>>,
    pub images: Vec<NoteImageView>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Serialize)]
pub struct CreateNoteResponse {
    pub id: i64,
    pub created_at: i64,
}

#[derive(Debug, Serialize)]
pub struct UpdateNoteResponse {
    pub id: i64,
    pub updated_at: i64,
}

#[derive(Debug, Serialize)]
pub struct NoteListResponse {
    pub total: i64,
    pub items: Vec<NoteView>,
}

// ---- 统计 ----

#[derive(Debug, Serialize)]
pub struct SubjectDistribution {
    pub subject_id: Option<i64>,
    pub name: String,
    pub minutes: i64,
}

#[derive(Debug, Serialize)]
pub struct OverviewResponse {
    pub total_minutes: i64,
    pub completed_pomos: i64,
    pub abandoned_pomos: i64,
    pub distraction_count: i64,
    // TODO(设计待确认 #7)：上游未给公式；当前实现 = distraction_count / completed_pomos（完成番茄均分）
    pub distraction_rate: f64,
    pub subject_distribution: Vec<SubjectDistribution>,
}

#[derive(Debug, Serialize)]
pub struct TrendPoint {
    pub date: String, // 粒度起始日期（YYYY-MM-DD）
    pub minutes: i64,
    pub pomodoros: i64,
    pub distractions: i64,
}

#[derive(Debug, Serialize)]
pub struct TrendResponse {
    pub points: Vec<TrendPoint>,
}

#[derive(Debug, Serialize)]
pub struct AppHotspot {
    pub app_name: Option<String>,
    pub count: i64,
}

#[derive(Debug, Serialize)]
pub struct HourHotspot {
    pub hour: i32, // 0-23
    pub count: i64,
}

#[derive(Debug, Serialize)]
pub struct TypeHotspot {
    pub r#type: i32, // 1/2/3；字段名 type 为关键字故序列化别名
    pub count: i64,
}

#[derive(Debug, Serialize)]
pub struct DistractionHotspotResponse {
    pub by_app: Vec<AppHotspot>,
    pub by_hour: Vec<HourHotspot>,
    pub by_type: Vec<TypeHotspot>,
}

#[derive(Debug, Serialize)]
pub struct InsightItem {
    // 示例：high_distraction_hour / subject_neglect / duration_decline / streak_record / camera_distraction_high
    pub r#type: String,
    // TODO(设计待确认)：severity 枚举未定义；当前用 info/warn/critical
    pub severity: String,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct RulesSummaryResponse {
    pub insights: Vec<InsightItem>,
}

#[derive(Debug, Serialize)]
pub struct LlmSummaryResponse {
    pub summary: String,
    pub suggestions: Vec<String>,
}

// ---- 配置 ----

#[derive(Debug, Serialize)]
pub struct SettingsResponse {
    pub settings: std::collections::HashMap<String, serde_json::Value>,
}

#[derive(Debug, Serialize)]
pub struct UpdateSettingsResponse {
    pub updated_keys: Vec<String>,
}
