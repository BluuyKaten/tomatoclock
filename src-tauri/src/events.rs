//! 事件总线：计时器 tick、分心事件、提醒触发通过 broadcast 异步通信
//!
//! 设计要点（对齐上游 §5）：
//! - 各服务只发命令 / 订阅事件，不直接互相调用
//! - 前端通过 Tauri 的 listen() 订阅这些事件，实现 UI 实时刷新
//!
//! 事件名约定：前端通过 listen(`tomatoclock://<kind>`) 订阅，
//! 其中 <kind> 对应 AppEvent 的 serde tag 值（snake_case）。

use serde::Serialize;
use tauri::Emitter;
use tokio::sync::broadcast;

/// 事件容量（背压策略：慢消费者会丢旧事件）
const CAP: usize = 256;

/// 应用事件类型
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AppEvent {
    /// 计时器每秒 tick（remaining_seconds）
    TimerTick {
        pomodoro_id: i64,
        remaining_seconds: i64,
        status: i32,
    },
    /// 番茄完成
    PomodoroCompleted {
        pomodoro_id: i64,
        actual_duration: i64,
        distraction_count: i32,
    },
    /// 番茄放弃/打断
    PomodoroStopped {
        pomodoro_id: i64,
        status: i32,
    },
    /// 分心事件
    DistractionDetected {
        pomodoro_id: i64,
        distraction_type: i32,
        app_name: Option<String>,
        window_title: Option<String>,
    },
    /// 渐进式提醒触发
    ReminderTriggered {
        pomodoro_id: i64,
        level: i32,
        message: String,
    },
    /// 用户切换
    UserChanged {
        user_id: Option<i64>,
    },
}

impl AppEvent {
    /// 事件名（用于 Tauri emit 与前端 listen）
    pub fn event_name(&self) -> &'static str {
        match self {
            AppEvent::TimerTick { .. } => "tomatoclock://timer-tick",
            AppEvent::PomodoroCompleted { .. } => "tomatoclock://pomodoro-completed",
            AppEvent::PomodoroStopped { .. } => "tomatoclock://pomodoro-stopped",
            AppEvent::DistractionDetected { .. } => "tomatoclock://distraction",
            AppEvent::ReminderTriggered { .. } => "tomatoclock://reminder",
            AppEvent::UserChanged { .. } => "tomatoclock://user-changed",
        }
    }
}

/// 事件总线句柄（Clone 廉价）
#[derive(Clone)]
pub struct EventBus {
    tx: broadcast::Sender<AppEvent>,
    app: Option<tauri::AppHandle>,
}

impl EventBus {
    pub fn new() -> Self {
        let (tx, _rx) = broadcast::channel(CAP);
        Self { tx, app: None }
    }

    /// 绑定 Tauri AppHandle，启用后 emit 会同时走 Tauri 全局事件
    pub fn with_app(mut self, app: tauri::AppHandle) -> Self {
        self.app = Some(app);
        self
    }

    /// 发布事件（忽略无订阅者错误）
    pub fn emit(&self, event: AppEvent) {
        // [FIX] 降级为 debug 日志：无订阅者是正常情况（前端通过 Tauri 全局事件接收）
        if let Err(_e) = self.tx.send(event.clone()) {
            // 广播通道无订阅者是正常情况
        }
        // 同时走 Tauri 全局事件（前端 listen 接收）
        // [FIX] 使用独立线程 emit，避免 app.emit 阻塞 async 运行时
        if let Some(app) = &self.app {
            let name = event.event_name().to_string();
            let app = app.clone();
            std::thread::spawn(move || {
                // 序列化为 JSON Value 再 emit，避免 serde 阻塞
                if let Ok(json) = serde_json::to_value(&event) {
                    let _ = app.emit(&name, json);
                }
            });
        }
    }

    /// 订阅（进程内服务间用）
    pub fn subscribe(&self) -> broadcast::Receiver<AppEvent> {
        self.tx.subscribe()
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}
