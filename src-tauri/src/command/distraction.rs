//! 分心检测规则命令（对齐上游 §7.4）

use tauri::{State};

use crate::command::helpers::ok;
use crate::domain::requests::*;
use crate::domain::responses::*;
use crate::error::{AppError, AppResult};
use crate::repository::app_rules::AppRuleRepo;
use crate::AppState;

/// GET /api/distraction/top_windows
/// 枚举当前所有可见顶层窗口（按应用名去重），用于设置页「选择应用」对话框
#[tauri::command]
pub fn list_top_windows(
    state: State<'_, AppState>,
) -> Result<ApiResponse<serde_json::Value>, AppError> {
    let _user_id = state.current_user_id()?;
    let windows = crate::platform::windows::list_top_windows();
    let views: Vec<serde_json::Value> = windows
        .into_iter()
        .map(|w| serde_json::json!({ "app_name": w.app_name, "window_title": w.window_title }))
        .collect();
    ok(serde_json::json!({ "windows": views }))
}

/// GET /api/distraction/rules
#[tauri::command]
pub fn list_rules(state: State<'_, AppState>) -> Result<ApiResponse<RuleListResponse>, AppError> {
    let user_id = state.current_user_id()?;
    let pool = state.db();
    let rules = AppRuleRepo::list_by_user(pool, user_id)?;
    let views: Vec<RuleView> = rules
        .into_iter()
        .map(|r| RuleView {
            id: r.id,
            rule_type: r.rule_type,
            app_name: r.app_name,
            window_title_pattern: r.window_title_pattern,
            is_enabled: r.is_enabled,
        })
        .collect();
    ok(RuleListResponse { rules: views })
}

/// POST /api/distraction/rules
#[tauri::command]
pub fn create_rule(
    state: State<'_, AppState>,
    req: CreateRuleRequest,
) -> Result<ApiResponse<serde_json::Value>, AppError> {
    let user_id = state.current_user_id()?;
    let pool = state.db();
    validate_rule_type(req.rule_type)?;
    let id = AppRuleRepo::create(
        pool,
        user_id,
        req.rule_type,
        &req.app_name,
        req.window_title_pattern.as_deref(),
        req.is_enabled.unwrap_or(true),
    )?;
    ok(serde_json::json!({ "id": id }))
}

/// PUT /api/distraction/rules/{id}
#[tauri::command]
pub fn update_rule(
    state: State<'_, AppState>,
    id: i64,
    req: UpdateRuleRequest,
) -> Result<ApiResponse<serde_json::Value>, AppError> {
    let user_id = state.current_user_id()?;
    let pool = state.db();
    let existing = AppRuleRepo::find_by_id(pool, id)?
        .ok_or_else(|| AppError::NotFound("规则不存在".into()))?;
    if existing.user_id != user_id {
        return Err(AppError::AuthError("无权操作".into()));
    }
    AppRuleRepo::update(
        pool,
        id,
        req.rule_type,
        req.app_name.as_deref(),
        // [FIX] 函数签名要求 Option<Option<&str>>，需要再包一层 Some
        req.window_title_pattern.as_ref().map(|o| Some(o.as_str())),
        req.is_enabled,
    )?;
    ok(serde_json::json!({ "id": id }))
}

/// DELETE /api/distraction/rules/{id}
#[tauri::command]
pub fn delete_rule(
    state: State<'_, AppState>,
    id: i64,
) -> Result<ApiResponse<serde_json::Value>, AppError> {
    let user_id = state.current_user_id()?;
    let pool = state.db();
    let existing = AppRuleRepo::find_by_id(pool, id)?
        .ok_or_else(|| AppError::NotFound("规则不存在".into()))?;
    if existing.user_id != user_id {
        return Err(AppError::AuthError("无权操作".into()));
    }
    AppRuleRepo::delete(pool, id)?;
    ok(serde_json::json!({ "id": id }))
}

fn validate_rule_type(t: i32) -> AppResult<()> {
    if t == 1 || t == 2 {
        Ok(())
    } else {
        Err(AppError::InvalidParam("rule_type 必须为 1 或 2".into()))
    }
}
