// 模块声明
pub mod command;
pub mod db;
pub mod domain;
pub mod error;
pub mod events;
pub mod platform;
pub mod repository;
pub mod service;

use db::DbPool;
use error::AppResult;
use service::distraction::DistractionService;
use service::reminder::ReminderService;
use service::timer::TimerService;
use std::sync::Arc;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::Manager;
use tauri::Emitter;

use crate::repository::app_settings::AppSettingRepo;

/// 应用全局状态：由各 command 通过 State 注入
pub struct AppState {
    /// 数据库连接池
    pub db: DbPool,
    /// 当前登录用户 ID（未登录为 None）
    /// 注意：本地单机场景，进程内只维护一个当前用户
    pub current_user: Arc<std::sync::RwLock<Option<i64>>>,
}

impl AppState {
    pub fn db(&self) -> &DbPool {
        &self.db
    }

    /// 获取当前用户 ID，未登录返回错误
    pub fn current_user_id(&self) -> AppResult<i64> {
        self.current_user
            .read()
            .map_err(|_| error::AppError::Internal("锁中毒".into()))?
            .ok_or(error::AppError::AuthRequired)
    }

    pub fn set_current_user(&self, user_id: Option<i64>) -> AppResult<()> {
        let mut guard = self.current_user.write().map_err(|_| {
            error::AppError::Internal("锁中毒".into())
        })?;
        *guard = user_id;
        Ok(())
    }
}

/// 启动时初始化：建连、跑迁移、装配服务、注册 Tauri 命令
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "tomatoclock=info,info".into()),
        )
        .with_target(true)
        .with_thread_ids(false)
        .init();

    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .setup(|app| {
            let handle = app.handle().clone();
            let pool = db::create_pool(&handle).expect("数据库初始化失败");
            let state = AppState {
                db: pool,
                current_user: Arc::new(std::sync::RwLock::new(None)),
            };
            app.manage(state);

            // 装配事件总线与核心服务（计时器 / 分心 / 提醒）
            let event_bus = events::EventBus::new().with_app(handle.clone());
            app.manage(event_bus.clone());

            // 装配计时器服务（Arc 包装，同时注册到 Tauri 并注入 DistractionService）
            let timer = std::sync::Arc::new(TimerService::new(handle.clone(), event_bus.clone()));
            app.manage(timer.clone());

            let pool = app.state::<AppState>().db().clone();
            let distraction = DistractionService::new(handle.clone(), event_bus.clone(), pool, timer.clone());
            app.manage(distraction);

            let reminder = ReminderService::new(handle.clone(), event_bus.clone());
            app.manage(reminder);

            // 装配系统托盘：左键单击恢复主窗口，右键菜单提供「显示主窗口 / 开始专注 / 退出」
            let show_item = MenuItem::with_id(app, "tray-show", "显示主窗口", true, None::<&str>)?;
            let start_item = MenuItem::with_id(app, "tray-toggle", "开始专注", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, "tray-quit", "退出程序", true, None::<&str>)?;
            let tray_menu = Menu::with_items(app, &[&show_item, &start_item, &quit_item])?;
            let _tray = TrayIconBuilder::with_id("main-tray")
                .tooltip("番茄钟")
                .menu(&tray_menu)
                .on_tray_icon_event(|ic, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = ic.app_handle();
                        if let Some(win) = app.get_webview_window("main") {
                            let _ = win.show();
                            let _ = win.set_focus();
                        }
                    }
                })
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "tray-show" => {
                        if let Some(win) = app.get_webview_window("main") {
                            let _ = win.show();
                            let _ = win.set_focus();
                        }
                    }
                    "tray-toggle" => {
                        // 通过事件通知前端切换番茄状态（前端根据当前状态决定开始/暂停）
                        app.emit("tray://toggle-pomodoro", ()).ok();
                    }
                    "tray-quit" => app.exit(0),
                    _ => {}
                })
                .build(app)?;
            tracing::info!("番茄钟初始化完成");
            Ok(())
        })
        // 拦截主窗口关闭按钮：按 app.close_behavior 设置分支（quit/taskbar/tray）
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if window.label() != "main" {
                    return;
                }
                let behavior = read_close_behavior(window.app_handle());
                match behavior.as_str() {
                    "quit" => {} // 允许默认关闭
                    "taskbar" => {
                        api.prevent_close();
                        let _ = window.minimize();
                    }
                    "tray" | _ => {
                        api.prevent_close();
                        let _ = window.hide();
                    }
                }
            }
        })
        // 注册所有 Tauri 命令（严格对齐上游 §7 接口）
        .invoke_handler(tauri::generate_handler![
            // 账号
            command::auth::register,
            command::auth::login,
            command::auth::auto_login,
            command::auth::logout,
            // 番茄计时器
            command::pomodoro::start_pomodoro,
            command::pomodoro::pause_pomodoro,
            command::pomodoro::resume_pomodoro,
            command::pomodoro::complete_pomodoro,
            command::pomodoro::abandon_pomodoro,
            command::pomodoro::get_current_pomodoro,
            // 分心检测规则
            command::distraction::list_rules,
            command::distraction::create_rule,
            command::distraction::update_rule,
            command::distraction::delete_rule,
            command::distraction::list_top_windows,
            // 学习记录
            command::notes::create_note,
            command::notes::list_notes,
            command::notes::update_note,
            command::notes::delete_note,
            // 统计与分析
            command::stats::overview,
            command::stats::trend,
            command::stats::distraction_hotspot,
            command::stats::rules_summary,
            command::stats::llm_summary,
            // 配置
            command::settings::get_settings,
            command::settings::update_settings,
        ])
        .run(tauri::generate_context!())
        .expect("Tauri 应用运行失败");
}

/// 读取当前用户的关闭行为设置（app.close_behavior）
/// 未登录或读取失败时默认返回 "tray"（最小化到托盘）
fn read_close_behavior(app: &tauri::AppHandle) -> String {
    let state = app.state::<AppState>();
    let user_id = match state.current_user_id() {
        Ok(id) => id,
        Err(_) => return "tray".to_string(),
    };
    AppSettingRepo::get(state.db(), user_id, "app.close_behavior")
        .ok()
        .flatten()
        .unwrap_or_else(|| "tray".to_string())
}
