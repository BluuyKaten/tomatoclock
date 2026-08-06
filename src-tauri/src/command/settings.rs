//! 配置命令（对齐上游 §7.7）

use std::collections::HashMap;

use tauri::{State};

use crate::command::helpers::ok;
use crate::domain::requests::*;
use crate::domain::responses::*;
use crate::error::AppError;
use crate::repository::app_settings::AppSettingRepo;
use crate::AppState;

/// GET /api/settings
#[tauri::command]
pub fn get_settings(
    state: State<'_, AppState>,
) -> Result<ApiResponse<SettingsResponse>, AppError> {
    let user_id = state.current_user_id()?;
    let pool = state.db();
    let raw = AppSettingRepo::get_all(pool, user_id)?;
    let settings: HashMap<String, serde_json::Value> = raw
        .into_iter()
        .map(|(k, v)| {
            let val = v.and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
                .unwrap_or(serde_json::Value::Null);
            (k, val)
        })
        .collect();
    ok(SettingsResponse { settings })
}

/// PUT /api/settings（部分更新）
#[tauri::command]
pub fn update_settings(
    state: State<'_, AppState>,
    req: UpdateSettingsRequest,
) -> Result<ApiResponse<UpdateSettingsResponse>, AppError> {
    let user_id = state.current_user_id()?;
    let pool = state.db();
    let kvs: HashMap<String, Option<String>> = req
        .settings
        .into_iter()
        .map(|(k, v)| {
            let v_str = if v.is_null() {
                None
            } else if v.is_string() {
                v.as_str().map(|s| s.to_string())
            } else {
                Some(v.to_string())
            };
            (k, v_str)
        })
        .collect();
    let updated = AppSettingRepo::set_many(pool, user_id, &kvs)?;
    ok(UpdateSettingsResponse { updated_keys: updated })
}
