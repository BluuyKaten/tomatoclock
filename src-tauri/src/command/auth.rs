//! 账号命令（对齐上游 §7.2）

use tauri::State;

use crate::command::helpers::ok;
use crate::domain::requests::*;
use crate::domain::responses::*;
use crate::error::AppError;
use crate::events::EventBus;
use crate::repository::users::UserRepo;
use crate::service::auth::AuthService;
use crate::AppState;

/// POST /api/auth/register
#[tauri::command]
pub async fn register(
    state: State<'_, AppState>,
    req: RegisterRequest,
) -> Result<ApiResponse<AuthUser>, AppError> {
    let pool = state.db();
    let id = AuthService::register(pool, &req.username, &req.password)?;
    let user = UserRepo::find_by_id(pool, id)?.ok_or_else(|| AppError::NotFound("用户不存在".into()))?;
    ok(AuthUser {
        user_id: user.id,
        username: user.username,
        created_at: Some(user.created_at),
    })
}

/// POST /api/auth/login
#[tauri::command]
pub async fn login(
    state: State<'_, AppState>,
    bus: State<'_, EventBus>,
    req: LoginRequest,
) -> Result<ApiResponse<LoginResult>, AppError> {
    let pool = state.db();
    let id = AuthService::login(pool, &req.username, &req.password)?;
    let user = UserRepo::find_by_id(pool, id)?.ok_or_else(|| AppError::NotFound("用户不存在".into()))?;

    // 记住登录：创建会话令牌并返回明文给前端
    let token = if req.remember_me.unwrap_or(false) {
        Some(AuthService::create_session(pool, user.id, None)?)
    } else {
        None
    };

    state.set_current_user(Some(id))?;
    bus.emit(crate::events::AppEvent::UserChanged { user_id: Some(id) });
    ok(LoginResult {
        user_id: user.id,
        username: user.username,
        token,
    })
}

/// POST /api/auth/auto-login（通过会话令牌自动登录）
#[tauri::command]
pub async fn auto_login(
    state: State<'_, AppState>,
    bus: State<'_, EventBus>,
    req: AutoLoginRequest,
) -> Result<ApiResponse<AutoLoginResult>, AppError> {
    let pool = state.db();
    let id = AuthService::login_with_token(pool, &req.token)?;
    let user = UserRepo::find_by_id(pool, id)?.ok_or_else(|| AppError::NotFound("用户不存在".into()))?;

    state.set_current_user(Some(id))?;
    bus.emit(crate::events::AppEvent::UserChanged { user_id: Some(id) });
    ok(AutoLoginResult {
        user_id: user.id,
        username: user.username,
        token: req.token,
    })
}

/// POST /api/auth/logout
#[tauri::command]
pub async fn logout(
    state: State<'_, AppState>,
    bus: State<'_, EventBus>,
) -> Result<ApiResponse<LogoutResult>, AppError> {
    let pool = state.db();
    if let Ok(id) = state.current_user_id() {
        AuthService::logout(pool, id)?;
    }
    state.set_current_user(None)?;
    bus.emit(crate::events::AppEvent::UserChanged { user_id: None });
    ok(LogoutResult { success: true })
}
