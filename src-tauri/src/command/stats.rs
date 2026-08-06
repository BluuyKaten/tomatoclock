//! 统计与分析命令（对齐上游 §7.6）

use tauri::{State};

use crate::command::helpers::ok;
use crate::domain::requests::*;
use crate::domain::responses::*;
use crate::error::AppError;
use crate::service::insights::InsightsService;
use crate::service::llm::LlmService;
use crate::service::stats::StatsService;
use crate::AppState;

/// GET /api/stats/overview
#[tauri::command]
pub fn overview(
    state: State<'_, AppState>,
    req: TimeRangeRequest,
) -> Result<ApiResponse<OverviewResponse>, AppError> {
    let user_id = state.current_user_id()?;
    let pool = state.db();
    ok(StatsService::overview(pool, user_id, req.from, req.to)?)
}

/// GET /api/stats/trend
#[tauri::command]
pub fn trend(
    state: State<'_, AppState>,
    req: TrendRequest,
) -> Result<ApiResponse<TrendResponse>, AppError> {
    let user_id = state.current_user_id()?;
    let pool = state.db();
    let gran = match req.granularity.as_str() {
        "week" | "month" => req.granularity.as_str(),
        _ => "day",
    };
    ok(StatsService::trend(pool, user_id, req.from, req.to, gran)?)
}

/// GET /api/stats/distraction-hotspot
#[tauri::command]
pub fn distraction_hotspot(
    state: State<'_, AppState>,
    req: TimeRangeRequest,
) -> Result<ApiResponse<DistractionHotspotResponse>, AppError> {
    let user_id = state.current_user_id()?;
    let pool = state.db();
    ok(StatsService::distraction_hotspot(pool, user_id, req.from, req.to)?)
}

/// GET /api/insights/rules-summary
#[tauri::command]
pub fn rules_summary(
    state: State<'_, AppState>,
    req: TimeRangeRequest,
) -> Result<ApiResponse<RulesSummaryResponse>, AppError> {
    let user_id = state.current_user_id()?;
    let pool = state.db();
    ok(InsightsService::rules_summary(pool, user_id, req.from, req.to)?)
}

/// POST /api/insights/llm-summary
#[tauri::command]
pub async fn llm_summary(
    state: State<'_, AppState>,
    req: LlmSummaryRequest,
) -> Result<ApiResponse<LlmSummaryResponse>, AppError> {
    let user_id = state.current_user_id()?;
    let pool = state.db();
    let language = req.language.as_deref();
    // 失败返回 2001
    let resp = LlmService::summary(pool, user_id, req.from, req.to, language).await?;
    ok(resp)
}
