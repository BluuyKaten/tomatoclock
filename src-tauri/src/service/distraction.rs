//! 分心检测服务（窗口 / 输入 / 摄像头）
//!
//! 设计要点（对齐上游 §8.2）：
//! - 三种检测器各自产出"候选分心事件"，由判定引擎融合
//! - 摄像头模块当前为 trait 占位（V1.1 接 ONNX），默认关闭
//! - 平台相关代码隔离在 src/platform/windows/ 下

use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, info, warn};

use crate::db::DbPool;
use crate::error::AppResult;
use crate::events::{AppEvent, EventBus};
use crate::repository::app_rules::AppRuleRepo;
use crate::repository::distractions::DistractionRepo;
// [FIX] 导入 AppSettingRepo 以读取分心检测配置
use crate::repository::app_settings::AppSettingRepo;
use crate::service::timer::TimerService;
use tauri_plugin_notification::NotificationExt;

/// 分心状态转换结果
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DistractionTransition {
    /// 进入分心状态（需要暂停计时 + 记录分心）
    EnteredDistraction,
    /// 退出分心状态（需要恢复计时）
    ExitedDistraction,
    /// 状态无变化
    NoChange,
}

/// 纯函数：根据当前状态与窗口匹配结果，计算状态转换
/// 可独立单测，无需 Tauri / tokio 环境
pub fn evaluate_distraction_transition(distracted: bool, matches_rule: bool) -> DistractionTransition {
    match (distracted, matches_rule) {
        (false, true) => DistractionTransition::EnteredDistraction,
        (true, false) => DistractionTransition::ExitedDistraction,
        _ => DistractionTransition::NoChange,
    }
}

/// 候选分心事件（检测器产出）
#[derive(Debug, Clone)]
pub struct DistractionCandidate {
    pub distraction_type: i32, // 1 窗口 / 2 输入空闲 / 3 摄像头
    pub app_name: Option<String>,
    pub app_wm_class: Option<String>,
    pub window_title: Option<String>,
    pub idle_seconds: Option<i64>,
    pub face_detected: Option<bool>,
    pub gaze_left: Option<bool>,
}

/// 分心检测器 trait（便于单测替换 / 平台适配）
#[async_trait::async_trait]
pub trait Detector: Send + Sync {
    /// 检测器名称
    fn name(&self) -> &'static str;
    /// 是否启用
    fn is_enabled(&self) -> bool;
    /// 执行一次检测，返回候选分心（None 表示未分心）
    async fn detect(&self) -> Option<DistractionCandidate>;
}

/// 窗口检测器：检查活跃窗口是否命中黑名单（或不在白名单）
pub struct WindowDetector;

#[async_trait::async_trait]
impl Detector for WindowDetector {
    fn name(&self) -> &'static str {
        "window"
    }
    fn is_enabled(&self) -> bool {
        true
    }
    async fn detect(&self) -> Option<DistractionCandidate> {
        // 平台相关：读取活跃窗口
        let info = crate::platform::windows::active_window_info();
        match info {
            Some((app, title)) => Some(DistractionCandidate {
                distraction_type: 1,
                app_name: Some(app),
                app_wm_class: None,
                window_title: Some(title),
                idle_seconds: None,
                face_detected: None,
                gaze_left: None,
            }),
            None => None,
        }
    }
}

/// 输入空闲检测器（rdev / device_state 抽象）
pub struct InputIdleDetector {
    threshold_secs: i64,
}

impl InputIdleDetector {
    pub fn new(threshold_secs: i64) -> Self {
        Self { threshold_secs }
    }
}

#[async_trait::async_trait]
impl Detector for InputIdleDetector {
    fn name(&self) -> &'static str {
        "input_idle"
    }
    fn is_enabled(&self) -> bool {
        true
    }
    async fn detect(&self) -> Option<DistractionCandidate> {
        let idle_secs = crate::platform::windows::idle_seconds();
        if idle_secs >= self.threshold_secs {
            Some(DistractionCandidate {
                distraction_type: 2,
                app_name: None,
                app_wm_class: None,
                window_title: None,
                idle_seconds: Some(idle_secs),
                face_detected: None,
                gaze_left: None,
            })
        } else {
            None
        }
    }
}

/// 摄像头检测器（V1.1 预留 trait 占位，当前始终返回 None）
pub struct CameraDetector;

#[async_trait::async_trait]
impl Detector for CameraDetector {
    fn name(&self) -> &'static str {
        "camera"
    }
    fn is_enabled(&self) -> bool {
        false // 默认关闭；需用户在设置中显式授权 + 下载模型
    }
    async fn detect(&self) -> Option<DistractionCandidate> {
        None
    }
}

/// 分心判定引擎 + 运行时
pub struct DistractionService {
    inner: Arc<Mutex<DistractionRuntime>>,
    /// 数据库连接池（后台 tick 循环需要持有）
    pool: DbPool,
    /// 事件总线（后台 tick 循环需要持有）
    bus: EventBus,
    /// Tauri AppHandle（用于发送 Windows 系统通知）
    app: tauri::AppHandle,
    /// [FIX] 持有计时器服务，用于分心时暂停 / 恢复
    timer: std::sync::Arc<TimerService>,
}

#[derive(Debug)]
struct DistractionRuntime {
    enabled: bool,
    current_pomodoro: Option<i64>,
    current_user: Option<i64>,
    // 上次检测时间（用于节流）
    last_window_check: Option<std::time::Instant>,
    last_input_check: Option<std::time::Instant>,
    // 后台 tick 循环的停止信号发送端（Some 表示循环正在运行）
    tick_stop: Option<tokio::sync::oneshot::Sender<()>>,
    // 上次发送系统通知的时间（用于通知冷却，避免刷屏）
    last_notification: Option<std::time::Instant>,
    /// [FIX] 是否处于分心暂停状态（检测到分心窗口后 true，回到专注后 false）
    distracted: bool,
}

impl DistractionService {
    pub fn new(
        app: tauri::AppHandle,
        bus: EventBus,
        pool: DbPool,
        timer: std::sync::Arc<TimerService>,
    ) -> Self {
        Self {
            inner: Arc::new(Mutex::new(DistractionRuntime {
                enabled: false,
                current_pomodoro: None,
                current_user: None,
                last_window_check: None,
                last_input_check: None,
                tick_stop: None,
                last_notification: None,
                distracted: false,
            })),
            pool,
            bus,
            app,
            timer,
        }
    }

    /// 番茄开始时调用：启动检测 + 启动后台 tick 循环
    pub async fn on_pomodoro_start(&self, user_id: i64, pomodoro_id: i64) {
        let mut rt = self.inner.lock().await;
        rt.enabled = true;
        rt.current_pomodoro = Some(pomodoro_id);
        rt.current_user = Some(user_id);
        rt.last_window_check = None;
        rt.last_input_check = None;
        rt.distracted = false;

        // 创建停止通道，并把发送端存入运行时
        let (stop_tx, stop_rx) = tokio::sync::oneshot::channel();
        rt.tick_stop = Some(stop_tx);
        let inner = self.inner.clone();
        let pool = self.pool.clone();
        let bus = self.bus.clone();
        drop(rt); // 释放锁，避免任务内重复持锁死锁

        let app = self.app.clone();
        // 启动后台 tick 循环（每秒一次）
        tokio::spawn(async move {
            Self::tick_loop(inner, pool, bus, app, stop_rx).await;
        });

        info!(pomodoro_id, "分心检测已启动（含后台 tick 循环）");
    }

    /// 番茄暂停时调用：暂停检测（不终止循环，恢复时可快速继续）
    pub async fn on_pomodoro_pause(&self) {
        let mut rt = self.inner.lock().await;
        rt.enabled = false;
        info!("分心检测已暂停");
    }

    /// 番茄恢复时调用：恢复检测
    pub async fn on_pomodoro_resume(&self) {
        let mut rt = self.inner.lock().await;
        rt.enabled = true;
        info!("分心检测已恢复");
    }

    /// 番茄结束时调用：停止检测 + 终止后台 tick 循环
    pub async fn on_pomodoro_end(&self) {
        let mut rt = self.inner.lock().await;
        rt.enabled = false;
        rt.current_pomodoro = None;
        rt.current_user = None;
        // 发出停止信号，让后台循环退出
        if let Some(tx) = rt.tick_stop.take() {
            let _ = tx.send(());
        }
        info!("分心检测已停止（含后台 tick 循环）");
    }

    /// 后台 tick 循环：每秒执行一次检测，直到收到停止信号
    async fn tick_loop(
        inner: Arc<Mutex<DistractionRuntime>>,
        pool: DbPool,
        bus: EventBus,
        app: tauri::AppHandle,
        mut stop_rx: tokio::sync::oneshot::Receiver<()>,
    ) {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(1));
        // 若某次检测耗时超过 1s，跳过积压的 tick，避免雪崩
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            tokio::select! {
                _ = interval.tick() => {
                    if let Err(e) = Self::tick_once(&inner, &pool, &bus, &app).await {
                        warn!(error = %e, "分心检测 tick 失败");
                    }
                }
                _ = &mut stop_rx => {
                    info!("分心检测后台循环收到停止信号，退出");
                    break;
                }
            }
        }
    }

    /// 单次 tick（可被循环调用，也可外部单独调用）
    async fn tick_once(
        inner: &Arc<Mutex<DistractionRuntime>>,
        pool: &DbPool,
        bus: &EventBus,
        app: &tauri::AppHandle,
    ) -> AppResult<()> {
        let mut rt = inner.lock().await;
        if !rt.enabled {
            return Ok(());
        }
        let pomodoro_id = rt.current_pomodoro.unwrap();
        let user_id = rt.current_user.unwrap();

        // 窗口检测节流：默认 1s
        let window_interval = AppSettingRepo::get(pool, user_id, "distraction.window_check_interval_ms")
            .ok().flatten().and_then(|v| v.parse::<u64>().ok()).unwrap_or(1000);
        let window_interval = std::time::Duration::from_millis(window_interval);

        let now = std::time::Instant::now();
        let should_check_window = rt
            .last_window_check
            .map(|t| now - t >= window_interval)
            .unwrap_or(true);

        let idle_threshold = AppSettingRepo::get(pool, user_id, "distraction.idle_threshold_sec")
            .ok().flatten().and_then(|v| v.parse::<i64>().ok()).unwrap_or(30);

        if should_check_window {
            rt.last_window_check = Some(now);
        }
        // [FIX] 分心暂停期间跳过输入空闲检测，避免重复触发
        let distracted = rt.distracted;
        // 提前释放锁，避免在 await 中持锁
        drop(rt);

        if should_check_window {
            Self::check_window(inner, pool, bus, app, user_id, pomodoro_id).await?;
        }
        if !distracted {
            Self::check_input(inner, pool, bus, app, user_id, pomodoro_id, idle_threshold).await?;
        }

        Ok(())
    }

    async fn check_window(
        inner: &Arc<Mutex<DistractionRuntime>>,
        pool: &DbPool,
        bus: &EventBus,
        app: &tauri::AppHandle,
        user_id: i64,
        pomodoro_id: i64,
    ) -> AppResult<()> {
        let Some(candidate) = WindowDetector.detect().await else {
            debug!("[distraction] 无法读取前台窗口信息");
            return Ok(());
        };
        debug!(
            app_name = %candidate.app_name.as_deref().unwrap_or(""),
            window_title = %candidate.window_title.as_deref().unwrap_or(""),
            "[distraction] 检测到前台窗口"
        );
        if Self::matches_rule(pool, user_id, &candidate) {
            info!(
                app_name = %candidate.app_name.as_deref().unwrap_or(""),
                "[distraction] 命中规则，记录分心"
            );
            Self::record(inner, pool, bus, app, user_id, pomodoro_id, candidate, 0)
                .await?;
        } else {
            debug!(app_name = %candidate.app_name.as_deref().unwrap_or(""), "[distraction] 未命中任何规则");
        }
        Ok(())
    }

    async fn check_input(
        inner: &Arc<Mutex<DistractionRuntime>>,
        pool: &DbPool,
        bus: &EventBus,
        app: &tauri::AppHandle,
        user_id: i64,
        pomodoro_id: i64,
        threshold: i64,
    ) -> AppResult<()> {
        let detector = InputIdleDetector::new(threshold);
        if let Some(candidate) = detector.detect().await {
            Self::record(inner, pool, bus, app, user_id, pomodoro_id, candidate, 0)
                .await?;
        }
        Ok(())
    }

    /// 判断候选事件是否命中用户配置规则
    fn matches_rule(pool: &DbPool, user_id: i64, c: &DistractionCandidate) -> bool {
        let rules = match AppRuleRepo::list_by_user(pool, user_id) {
            Ok(r) if !r.is_empty() => r,
            _ => {
                debug!("[distraction] 用户无任何规则");
                return false;
            }
        };
        let active_count = rules.iter().filter(|r| r.is_enabled).count();
        debug!(active_rule_count = active_count, "[distraction] 当前启用规则数");

        // 规则语义：黑名单命中 = 分心；白名单模式下"不在白名单" = 分心
        let mut has_whitelist = false;
        let mut hit_blacklist = false;
        let mut hit_whitelist = false;

        for rule in rules {
            if !rule.is_enabled {
                continue;
            }
            let app_matches = rule.app_name == c.app_name.as_deref().unwrap_or("");
            let title_matches = match (&rule.window_title_pattern, &c.window_title) {
                (Some(pattern), Some(title)) => regex::Regex::new(pattern)
                    .map(|re| re.is_match(title))
                    .unwrap_or(false),
                (None, _) => true, // 未配置标题规则则只看 app_name
                (Some(_), None) => false,
            };

            if app_matches && title_matches {
                match rule.rule_type {
                    1 => hit_blacklist = true,
                    2 => { has_whitelist = true; hit_whitelist = true; }
                    _ => {}
                }
            }
        }

        if has_whitelist {
            hit_whitelist
        } else {
            hit_blacklist
        }
    }

    /// 通知冷却时间（避免连续分心时系统通知刷屏）
    const NOTIFY_COOLDOWN_SECS: u64 = 30;

    async fn record(
        inner: &Arc<Mutex<DistractionRuntime>>,
        pool: &DbPool,
        bus: &EventBus,
        app: &tauri::AppHandle,
        user_id: i64,
        pomodoro_id: i64,
        c: DistractionCandidate,
        reminder_level: i32,
    ) -> AppResult<()> {
        DistractionRepo::create(
            pool,
            pomodoro_id,
            user_id,
            c.distraction_type,
            c.app_name.as_deref(),
            c.app_wm_class.as_deref(),
            c.window_title.as_deref(),
            c.idle_seconds,
            c.face_detected,
            c.gaze_left,
            reminder_level,
        )?;
        crate::repository::pomodoros::PomodoroRepo::increment_distraction_count(pool, pomodoro_id)?;

        // 发送 Windows 系统通知（带冷却，避免刷屏）
        let now = std::time::Instant::now();
        let mut rt = inner.lock().await;
        let should_notify = rt
            .last_notification
            .map(|t| now.duration_since(t).as_secs() >= Self::NOTIFY_COOLDOWN_SECS)
            .unwrap_or(true);
        if should_notify {
            rt.last_notification = Some(now);
            let app_name = c.app_name.as_deref().unwrap_or("未知应用");
            let body = format!("检测到您在使用「{app_name}」，请回到专注！");
            match app.notification().builder()
                .title("🍅 分心提醒")
                .body(&body)
                .show()
            {
                Ok(_) => info!(app_name, "已发送分心系统通知"),
                Err(e) => warn!(error = %e, "发送分心系统通知失败"),
            }
        }
        drop(rt);

        bus.emit(AppEvent::DistractionDetected {
            pomodoro_id,
            distraction_type: c.distraction_type,
            app_name: c.app_name,
            window_title: c.window_title,
        });

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::evaluate_distraction_transition;
    use super::DistractionTransition;

    #[test]
    fn test_entered_distraction_when_not_distracted_and_matches() {
        assert_eq!(
            evaluate_distraction_transition(false, true),
            DistractionTransition::EnteredDistraction
        );
    }

    #[test]
    fn test_exited_distraction_when_distracted_and_not_matches() {
        assert_eq!(
            evaluate_distraction_transition(true, false),
            DistractionTransition::ExitedDistraction
        );
    }

    #[test]
    fn test_no_change_when_not_distracted_and_not_matches() {
        assert_eq!(
            evaluate_distraction_transition(false, false),
            DistractionTransition::NoChange
        );
    }

    #[test]
    fn test_no_change_when_distracted_and_matches() {
        assert_eq!(
            evaluate_distraction_transition(true, true),
            DistractionTransition::NoChange
        );
    }
}
