//! 渐进式提醒服务
//!
//! 4 级渐进提醒（对齐上游 §2.1 M6）：
//!   1 级：托盘闪烁
//!   2 级：桌面通知
//!   3 级：提示音 + 语音"请回到学习"
//!   4 级：浮窗强提醒
//!
//! 每级有冷却时间，避免重复轰炸。
//!
//! TODO(设计待确认 #8)：reminder.levels 配置 JSON schema 未定义；当前使用内置默认值。
//! 默认 schema 约定：
//! {
//!   "levels": [
//!     { "level": 1, "cooldown_seconds": 30, "tray_bounce": true,  "notify": false, "sound": false, "tts": false },
//!     { "level": 2, "cooldown_seconds": 60, "tray_bounce": false, "notify": true,  "sound": false, "tts": false },
//!     { "level": 3, "cooldown_seconds": 90, "tray_bounce": false, "notify": true,  "sound": true,  "tts": true  },
//!     { "level": 4, "cooldown_seconds": 120,"tray_bounce": false, "notify": true,  "sound": true,  "tts": true, "force_window": true }
//!   ]
//! }

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::Mutex;
use tauri::AppHandle;
use tracing::{info, warn};

use crate::error::AppResult;
use crate::events::{AppEvent, EventBus};

#[derive(Debug, Clone, Copy)]
pub struct ReminderLevel {
    pub level: i32,
    pub cooldown_seconds: i64,
    pub tray_bounce: bool,
    pub notify: bool,
    pub sound: bool,
    pub tts: bool,
    pub force_window: bool,
}

/// 默认 4 级配置
fn default_levels() -> Vec<ReminderLevel> {
    vec![
        ReminderLevel { level: 1, cooldown_seconds: 30,  tray_bounce: true,  notify: false, sound: false, tts: false, force_window: false },
        ReminderLevel { level: 2, cooldown_seconds: 60,  tray_bounce: false, notify: true,  sound: false, tts: false, force_window: false },
        ReminderLevel { level: 3, cooldown_seconds: 90,  tray_bounce: false, notify: true,  sound: true,  tts: true,  force_window: false },
        ReminderLevel { level: 4, cooldown_seconds: 120, tray_bounce: false, notify: true,  sound: true,  tts: true,  force_window: true  },
    ]
}

pub struct ReminderService {
    inner: Arc<Mutex<ReminderRuntime>>,
    app: AppHandle,
    bus: EventBus,
}

#[derive(Debug)]
struct ReminderRuntime {
    levels: Vec<ReminderLevel>,
    // 每级的上次触发时间（按 level 索引）
    last_triggered: HashMap<i32, Instant>,
    // 番茄累计分心次数（用于升级判定）
    distraction_accum: i32,
    current_level: i32,
}

impl ReminderService {
    pub fn new(app: AppHandle, bus: EventBus) -> Self {
        Self {
            inner: Arc::new(Mutex::new(ReminderRuntime {
                levels: default_levels(),
                last_triggered: HashMap::new(),
                distraction_accum: 0,
                current_level: 0,
            })),
            app,
            bus,
        }
    }

    /// 番茄开始时重置
    pub async fn reset(&self) {
        let mut rt = self.inner.lock().await;
        rt.distraction_accum = 0;
        rt.current_level = 0;
        rt.last_triggered.clear();
    }

    /// 收到分心事件时调用：累加并判定是否升级提醒
    pub async fn on_distraction(&self) -> AppResult<()> {
        let mut rt = self.inner.lock().await;
        rt.distraction_accum += 1;

        // 简单升级策略：
        //   第 1 次分心 → 1 级；第 2 次 → 2 级；第 4 次 → 3 级；第 6 次 → 4 级
        let target_level = match rt.distraction_accum {
            0 => 0,
            1 => 1,
            2..=3 => 2,
            4..=5 => 3,
            _ => 4,
        };

        if target_level > rt.current_level {
            rt.current_level = target_level;
        }

        let level_opt = rt.levels.iter().find(|l| l.level == rt.current_level).copied();
        let _accum = rt.distraction_accum;
        let level = rt.current_level;
        drop(rt);

        if let Some(cfg) = level_opt {
            self.trigger(level, cfg).await?;
        }

        Ok(())
    }

    async fn trigger(&self, level: i32, cfg: ReminderLevel) -> AppResult<()> {
        // 冷却检查
        {
            let mut rt = self.inner.lock().await;
            let now = Instant::now();
            if let Some(last) = rt.last_triggered.get(&level) {
                if now - *last < std::time::Duration::from_secs(cfg.cooldown_seconds as u64) {
                    return Ok(());
                }
            }
            rt.last_triggered.insert(level, now);
        }

        let message = format!("第 {} 次分心，请回到学习", level);

        // 桌面通知
        if cfg.notify {
            self.send_notification(&message);
        }

        // TTS 语音
        if cfg.tts {
            self.speak(&message);
        }

        // 浮窗强提醒：V1.0 用额外窗口实现（此处仅发事件，前端监听后弹出）
        if cfg.force_window {
            self.bus.emit(AppEvent::ReminderTriggered {
                pomodoro_id: 0,
                level,
                message: message.clone(),
            });
        }

        info!(level, "渐进式提醒触发");
        self.bus.emit(AppEvent::ReminderTriggered {
            pomodoro_id: 0,
            level,
            message,
        });

        Ok(())
    }

    fn send_notification(&self, body: &str) {
        // 通过 tauri-plugin-notification 发送
        // [FIX] 导入 NotificationExt trait 以使用 notification() 方法
        use tauri_plugin_notification::NotificationExt;
        
        match self.app.notification().builder().title("番茄钟").body(body).show() {
            Ok(_) => {}
            Err(e) => warn!(error = %e, "通知发送失败"),
        }
    }

    fn speak(&self, text: &str) {
        // 系统 TTS：V1.0 占位；后续接 Windows SAPI / macOS say
        // 隐私优先：语音仅在本地合成，不上传
        tracing::debug!(text, "TTS 占位");
    }
}
