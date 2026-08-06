//! 番茄计时器服务
//!
//! 设计要点（对齐上游 §8.2）：
//! - 使用 std::time::Instant 单调时钟，避免系统时间调整导致计时漂移
//! - 状态机：Idle → Running → Paused → Running → Finished/Abandoned/Interrupted
//! - 每秒通过事件总线 emit TimerTick，前端订阅刷新 UI
//! - 单用户本地场景：进程内只维护一个"当前进行中的番茄"

use std::sync::{Arc, Mutex};
use std::time::Instant;
use tracing::info;

use crate::db::DbPool;
use crate::error::{AppError, AppResult};
use crate::events::{AppEvent, EventBus};
use crate::repository::pomodoros::PomodoroRepo;
use crate::repository::app_settings::AppSettingRepo;
use crate::repository::DbConn;
use tauri::AppHandle;

/// 计时器状态
/// 注意：数值必须与 pomodoros.status 数据库约定对齐：
///   0 = Running（进行中），1 = Paused（暂停），2 = Idle/无活跃番茄
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimerStatus {
    Running = 0,
    Paused = 1,
    Idle = 2,
}

/// 运行时状态（单次番茄）
#[derive(Debug)]
pub struct TimerState {
    pub pomodoro_id: i64,
    pub planned_duration: i64,       // 秒
    pub accumulated_secs: i64,       // 已累积的专注秒数（不含暂停时段）
    pub segment_start: Option<Instant>, // 当前运行段起点（None 表示暂停中）
    pub status: TimerStatus,
    pub distraction_count: i32,      // 分心次数快照（current() 返回给前端）
}

impl TimerState {
    /// 当前剩余秒数（运行中会实时计算）
    pub fn remaining_seconds(&self) -> i64 {
        let extra = match self.segment_start {
            Some(start) => start.elapsed().as_secs() as i64,
            None => 0,
        };
        let elapsed = self.accumulated_secs + extra;
        (self.planned_duration - elapsed).max(0)
    }
}

/// 计时器服务（由 Tauri 管理，持有当前状态 + 后台 tick 任务控制）
pub struct TimerService {
    inner: Arc<Mutex<TimerRuntime>>,
    bus: EventBus,
}

#[derive(Debug)]
struct TimerRuntime {
    state: Option<TimerState>,
    // [NOTE] tick_stop 已移除：后端不再驱动周期性 tick，由前端本地定时器驱动
}

impl TimerService {
    pub fn new(_app: AppHandle, bus: EventBus) -> Self {
        Self {
            inner: Arc::new(Mutex::new(TimerRuntime {
                state: None,
            })),
            bus,
        }
    }

    /// 开始新番茄（若已有进行中的则拒绝）
    pub async fn start(
        &self,
        pool: &DbPool,
        user_id: i64,
        task_id: Option<i64>,
        subject_id: Option<i64>,
        duration_override: Option<i64>,
        _bus: &EventBus,
    ) -> AppResult<i64> {
        // [FIX] 使用 std::sync::Mutex 的 lock() 而非 tokio::sync::Mutex 的 lock().await
        let mut rt = self.inner.lock().expect("锁中毒");
        if let Some(ref s) = rt.state {
            if s.status != TimerStatus::Idle {
                return Err(AppError::Conflict("已有进行中的番茄".into()));
            }
        }

        // 解析计划时长：优先入参，否则读配置，最终默认 1500s
        let planned = duration_override.unwrap_or_else(|| {
            AppSettingRepo::get(pool, user_id, "timer.focus_duration")
                .ok()
                .flatten()
                .and_then(|v| v.parse::<i64>().ok())
                .unwrap_or(1500)
        });
        if planned <= 0 || planned > 86400 {
            return Err(AppError::InvalidParam("计划时长需在 1-86400 秒".into()));
        }

        let id = PomodoroRepo::create(pool, user_id, task_id, subject_id, planned)?;

        rt.state = Some(TimerState {
            pomodoro_id: id,
            planned_duration: planned,
            accumulated_secs: 0,
            segment_start: Some(Instant::now()),
            status: TimerStatus::Running,
            distraction_count: 0,
        });

        // [FIX] 移除 start_tick 后台线程：前端已有本地定时器驱动每秒递减，
        // 后端不再 emit 周期性 tick，避免 blocking_lock 阻塞 tokio 运行时。
        // 仅 emit 一次初始事件，让前端立即刷新状态。
        info!(pomodoro_id = id, planned_secs = planned, "番茄开始");
        self.bus.emit(AppEvent::TimerTick {
            pomodoro_id: id,
            remaining_seconds: planned,
            status: TimerStatus::Running as i32,
        });

        Ok(id)
    }

    /// 暂停
    pub async fn pause(&self, _pool: &DbPool) -> AppResult<i64> {
        // [FIX] 使用 std::sync::Mutex 的 lock()
        let mut rt = self.inner.lock().expect("锁中毒");
        let state = rt.state.as_mut().ok_or_else(|| AppError::NotFound("无进行中的番茄".into()))?;

        if state.status != TimerStatus::Running {
            return Err(AppError::Conflict("番茄未在运行".into()));
        }

        // 结算当前运行段
        if let Some(start) = state.segment_start.take() {
            state.accumulated_secs += start.elapsed().as_secs() as i64;
        }
        state.status = TimerStatus::Paused;

        // [FIX] 先计算需要的数据，再 drop 借用
        let remaining = state.remaining_seconds();
        let id = state.pomodoro_id;
        let accumulated = state.accumulated_secs;
        drop(rt);
        // [NOTE] 不再调用 stop_tick（已移除）

        info!(pomodoro_id = id, accumulated = accumulated, "番茄暂停");
        // [FIX] 暂停事件必须携带 status=Paused(1)，否则前端 isTimerRunning 永远为 true
        self.bus.emit(AppEvent::TimerTick {
            pomodoro_id: id,
            remaining_seconds: remaining,
            status: TimerStatus::Paused as i32,
        });

        Ok(id)
    }

    /// 恢复
    pub async fn resume(&self, _pool: &DbPool) -> AppResult<i64> {
        let mut rt = self.inner.lock().expect("锁中毒");
        let state = rt.state.as_mut().ok_or_else(|| AppError::NotFound("无进行中的番茄".into()))?;

        if state.status != TimerStatus::Paused {
            return Err(AppError::Conflict("番茄未在暂停".into()));
        }

        state.segment_start = Some(Instant::now());
        state.status = TimerStatus::Running;
        // [FIX] 同 start：不再启动后端 tick，由前端本地定时器驱动

        let remaining = state.remaining_seconds();
        let id = state.pomodoro_id;
        info!(pomodoro_id = id, "番茄恢复");
        // [FIX] 恢复事件携带 status=Running(0)
        self.bus.emit(AppEvent::TimerTick {
            pomodoro_id: id,
            remaining_seconds: remaining,
            status: TimerStatus::Running as i32,
        });

        Ok(id)
    }

    /// 完成（正常响铃）
    pub async fn complete(&self, pool: &DbPool) -> AppResult<(i64, i64, i32)> {
        let mut rt = self.inner.lock().expect("锁中毒");
        let mut state = rt.state.take().ok_or_else(|| AppError::NotFound("无进行中的番茄".into()))?;

        // 结算最后运行段
        if let Some(start) = state.segment_start.take() {
            state.accumulated_secs += start.elapsed().as_secs() as i64;
        }

        let actual = state.accumulated_secs.min(state.planned_duration);
        let ended = crate::repository::now_ms();
        let distraction_count = Self::count_distractions(pool, state.pomodoro_id).unwrap_or(0);

        PomodoroRepo::finish(pool, state.pomodoro_id, 1, ended, actual, distraction_count)?;

        info!(pomodoro_id = state.pomodoro_id, actual_secs = actual, "番茄完成");
        self.bus.emit(AppEvent::PomodoroCompleted {
            pomodoro_id: state.pomodoro_id,
            actual_duration: actual,
            distraction_count,
        });

        Ok((state.pomodoro_id, actual, distraction_count))
    }

    /// 放弃（status=2）
    pub async fn abandon(&self, pool: &DbPool) -> AppResult<i64> {
        let mut rt = self.inner.lock().expect("锁中毒");
        let mut state = rt.state.take().ok_or_else(|| AppError::NotFound("无进行中的番茄".into()))?;

        if let Some(start) = state.segment_start.take() {
            state.accumulated_secs += start.elapsed().as_secs() as i64;
        }

        let actual = state.accumulated_secs.min(state.planned_duration);
        let ended = crate::repository::now_ms();
        let distraction_count = Self::count_distractions(pool, state.pomodoro_id).unwrap_or(0);

        PomodoroRepo::finish(pool, state.pomodoro_id, 2, ended, actual, distraction_count)?;

        info!(pomodoro_id = state.pomodoro_id, "番茄放弃");
        self.bus.emit(AppEvent::PomodoroStopped {
            pomodoro_id: state.pomodoro_id,
            status: 2,
        });

        Ok(state.pomodoro_id)
    }

    /// 查询当前状态（供 get_current 命令）
    /// 注意：distraction_count 从 DB 实时读取，而非内存快照，避免页面切换后丢失
    pub async fn current(&self, pool: &DbPool) -> Option<CurrentView> {
        let rt = self.inner.lock().expect("锁中毒");
        rt.state.as_ref().map(|s| {
            let db_count = Self::count_distractions(pool, s.pomodoro_id).unwrap_or(0);
            CurrentView {
                id: s.pomodoro_id,
                started_at: 0, // 不暴露 started_at 细节，由前端从 pomodoro 记录取
                planned_duration: s.planned_duration,
                accumulated_secs: s.accumulated_secs,
                remaining_seconds: s.remaining_seconds(),
                status: s.status as i32,
                distraction_count: db_count,
            }
        })
    }

    /// 强制重置（异常恢复用）
    pub async fn reset(&self) {
        let mut rt = self.inner.lock().expect("锁中毒");
        rt.state = None;
    }

    // ---- 内部 ----

    // [NOTE] start_tick / stop_tick / stop_tick_self 已移除：
    // 后端不再驱动周期性 tick，倒计时由前端本地定时器驱动。
    // 此改动消除了 blocking_lock 阻塞 tokio 运行时导致命令 hang 的 bug。

    fn count_distractions(pool: &DbPool, pomodoro_id: i64) -> AppResult<i32> {
        let conn = pool.conn()?;
        let c: i64 = conn.query_row(
            "SELECT COUNT(*) FROM distractions WHERE pomodoro_id = ?1",
            [&pomodoro_id],
            |row| row.get(0),
        )?;
        Ok(c as i32)
    }
}

#[derive(Debug, Clone)]
pub struct CurrentView {
    pub id: i64,
    pub started_at: i64,
    pub planned_duration: i64,
    pub accumulated_secs: i64,
    pub remaining_seconds: i64,
    pub status: i32,
    pub distraction_count: i32,
}
