//! 番茄计时器命令（对齐上游 §7.3）

use std::sync::Arc;

use tauri::{State};

use crate::command::helpers::ok;
use crate::domain::requests::*;
use crate::domain::responses::*;
use crate::error::AppError;
use crate::events::EventBus;
use crate::repository::pomodoros::PomodoroRepo;
use crate::service::distraction::DistractionService;
use crate::service::timer::TimerService;
use crate::AppState;

/// POST /api/pomodoro/start
#[tauri::command]
pub async fn start_pomodoro(
    state: State<'_, AppState>,
    timer: State<'_, Arc<TimerService>>,
    distraction: State<'_, DistractionService>,
    bus: State<'_, EventBus>,
    req: StartPomodoroRequest,
) -> Result<ApiResponse<StartPomodoroResponse>, AppError> {
    tracing::info!(?req, "收到 start_pomodoro 请求");
    let user_id = state.current_user_id().map_err(|e| {
        tracing::error!("start_pomodoro 获取当前用户失败: {}", e);
        e
    })?;
    let pool = state.db();
    // [FIX] timer.start 需要 6 个参数，补充 &EventBus
    let id = timer.start(pool, user_id, req.task_id, req.subject_id, req.duration, &bus).await.map_err(|e| {
        tracing::error!("timer.start 执行失败: {}", e);
        e
    })?;
    // 启动分心检测（含后台 tick 循环）
    distraction.on_pomodoro_start(user_id, id).await;
    tracing::info!(pomodoro_id = id, "番茄创建成功");
    let pomo = PomodoroRepo::find_by_id(pool, id)?.ok_or_else(|| AppError::NotFound("番茄不存在".into()))?;
    ok(StartPomodoroResponse {
        id,
        started_at: pomo.started_at,
        planned_duration: pomo.planned_duration,
        status: pomo.status,
    })
}

/// POST /api/pomodoro/{id}/pause
#[tauri::command]
pub async fn pause_pomodoro(
    state: State<'_, AppState>,
    timer: State<'_, Arc<TimerService>>,
    distraction: State<'_, DistractionService>,
) -> Result<ApiResponse<PausePomodoroResponse>, AppError> {
    let _user_id = state.current_user_id()?;
    let pool = state.db();
    let id = timer.pause(pool).await?;
    // 暂停分心检测（休息期间不计分心）
    distraction.on_pomodoro_pause().await;
    // [FIX] 通过 current() 获取准确的剩余秒数
    let view = timer.current(pool).await;
    let remaining = view.map(|v| v.remaining_seconds).unwrap_or(0);
    let paused_at = crate::repository::now_ms();
    ok(PausePomodoroResponse {
        id,
        paused_at,
        accumulated_seconds: remaining,
    })
}

/// POST /api/pomodoro/{id}/resume
#[tauri::command]
pub async fn resume_pomodoro(
    state: State<'_, AppState>,
    timer: State<'_, Arc<TimerService>>,
    distraction: State<'_, DistractionService>,
) -> Result<ApiResponse<ResumePomodoroResponse>, AppError> {
    let _user_id = state.current_user_id()?;
    let pool = state.db();
    let id = timer.resume(pool).await?;
    // 恢复分心检测
    distraction.on_pomodoro_resume().await;
    ok(ResumePomodoroResponse {
        id,
        resumed_at: crate::repository::now_ms(),
    })
}

/// POST /api/pomodoro/{id}/complete
#[tauri::command]
pub async fn complete_pomodoro(
    state: State<'_, AppState>,
    timer: State<'_, Arc<TimerService>>,
    distraction: State<'_, DistractionService>,
) -> Result<ApiResponse<CompletePomodoroResponse>, AppError> {
    let _user_id = state.current_user_id()?;
    let pool = state.db();
    // 先停止分心检测，再完成番茄（避免完成后 tick 仍在跑）
    distraction.on_pomodoro_end().await;
    let (id, actual, distraction_count) = timer.complete(pool).await?;
    let pomo = PomodoroRepo::find_by_id(pool, id)?.ok_or_else(|| AppError::NotFound("番茄不存在".into()))?;
    ok(CompletePomodoroResponse {
        id,
        ended_at: pomo.ended_at.unwrap_or_default(),
        actual_duration: actual,
        distraction_count,
    })
}

/// POST /api/pomodoro/{id}/abandon
#[tauri::command]
pub async fn abandon_pomodoro(
    state: State<'_, AppState>,
    timer: State<'_, Arc<TimerService>>,
    distraction: State<'_, DistractionService>,
    req: AbandonPomodoroRequest,
) -> Result<ApiResponse<AbandonPomodoroResponse>, AppError> {
    let _user_id = state.current_user_id()?;
    let pool = state.db();
    // TODO(设计待确认 #2)：reason 入参不落盘，仅记录日志
    if let Some(ref reason) = req.reason {
        tracing::info!(reason, "番茄放弃原因");
    }
    // 先停止分心检测，再放弃番茄
    distraction.on_pomodoro_end().await;
    let id = timer.abandon(pool).await?;
    ok(AbandonPomodoroResponse { id, status: 2 })
}

/// GET /api/pomodoro/current
#[tauri::command]
pub async fn get_current_pomodoro(
    state: State<'_, AppState>,
    timer: State<'_, Arc<TimerService>>,
) -> Result<ApiResponse<Option<CurrentPomodoroResponse>>, AppError> {
    let _user_id = state.current_user_id()?;
    let pool = state.db();
    let view = timer.current(pool).await;
    let resp = view.map(|v| {
        let started_at = crate::repository::now_ms()
            - (v.remaining_seconds.max(0) * 1000);
        CurrentPomodoroResponse {
            id: v.id,
            started_at,
            planned_duration: v.planned_duration,
            remaining_seconds: v.remaining_seconds,
            status: v.status,
            distraction_count: v.distraction_count,
        }
    });
    ok(resp)
}
