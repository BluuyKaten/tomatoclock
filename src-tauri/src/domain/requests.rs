//! 请求 DTO（对齐上游 §7 接口入参）

use serde::Deserialize;

/// 注册账号
#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    pub username: String,
    pub password: String,
}

/// 登录
#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
    /// 是否记住登录（服务端在验证通过后创建会话令牌）
    pub remember_me: Option<bool>,
}

/// 通过会话令牌自动登录
#[derive(Debug, Deserialize)]
pub struct AutoLoginRequest {
    pub token: String,
}

/// 开始番茄
#[derive(Debug, Deserialize)]
pub struct StartPomodoroRequest {
    pub task_id: Option<i64>,
    pub subject_id: Option<i64>,
    /// 计划时长（秒），空则取配置 timer.focus_duration
    pub duration: Option<i64>,
}

/// 放弃番茄
#[derive(Debug, Deserialize)]
pub struct AbandonPomodoroRequest {
    // TODO(设计待确认 #2)：上游 §7.3 接收 reason 入参，但 §6.2 pomodoros 表无 reason 列
    // 当前版本：reason 仅记录到 tracing 日志，不落盘
    pub reason: Option<String>,
}

/// 创建应用分心规则
#[derive(Debug, Deserialize)]
pub struct CreateRuleRequest {
    pub rule_type: i32,
    pub app_name: String,
    pub window_title_pattern: Option<String>,
    pub is_enabled: Option<bool>,
}

/// 更新应用规则
#[derive(Debug, Deserialize)]
pub struct UpdateRuleRequest {
    pub rule_type: Option<i32>,
    pub app_name: Option<String>,
    pub window_title_pattern: Option<String>,
    pub is_enabled: Option<bool>,
}

/// 创建笔记
#[derive(Debug, Deserialize)]
pub struct CreateNoteRequest {
    pub pomodoro_id: Option<i64>,
    pub subject_id: Option<i64>,
    pub title: Option<String>,
    pub content: String,
    pub tags: Option<Vec<String>>,
    // TODO(设计待确认 #9)：上游写 image_paths?，但客户端传任意路径有越权风险
    // 当前版本：由前端通过 invoke('save_note_image') 单独上传图片并返回路径，
    // 再通过本字段关联；应用会把路径限定在应用数据目录下
    pub image_paths: Option<Vec<String>>,
}

/// 更新笔记
#[derive(Debug, Deserialize)]
pub struct UpdateNoteRequest {
    pub title: Option<String>,
    pub content: Option<String>,
    pub tags: Option<Vec<String>>,
}

/// 笔记列表查询
#[derive(Debug, Default, Deserialize)]
pub struct ListNotesRequest {
    pub subject_id: Option<i64>,
    // TODO(设计待确认)：tag 匹配语义未定义；当前实现为"包含该标签"（JSON 数组包含）
    pub tag: Option<String>,
    pub from: Option<i64>, // 起始时间（Unix 毫秒，含）
    pub to: Option<i64>,   // 结束时间（Unix 毫秒，含）
    pub page: Option<i64>,
    pub page_size: Option<i64>,
}

/// 统计通用时间范围
#[derive(Debug, Deserialize)]
pub struct TimeRangeRequest {
    pub from: i64, // 起始时间（Unix 毫秒，含）
    pub to: i64,   // 结束时间（Unix 毫秒，含）
}

/// 趋势统计
#[derive(Debug, Deserialize)]
pub struct TrendRequest {
    pub from: i64,
    pub to: i64,
    pub granularity: String, // day | week | month
}

/// LLM 复盘
#[derive(Debug, Deserialize)]
pub struct LlmSummaryRequest {
    pub from: i64,
    pub to: i64,
    pub language: Option<String>, // zh | en
}

/// 批量更新配置
#[derive(Debug, Deserialize)]
pub struct UpdateSettingsRequest {
    pub settings: std::collections::HashMap<String, serde_json::Value>,
}
