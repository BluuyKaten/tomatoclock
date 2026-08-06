//! 账号服务：注册 / 登录 / 登出
//!
//! 密码使用 argon2id 哈希；哈希字符串已内含盐与参数，DB 的 salt 列为空串占位
//! （详见设计问题 #1）

use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use rand::RngCore;
use sha2::{Digest, Sha256};

use crate::db::DbPool;
use crate::error::{AppError, AppResult};
use crate::repository::session_tokens::SessionTokenRepo;
use crate::repository::users::UserRepo;

/// 会话令牌有效期（天）
const SESSION_TTL_DAYS: i64 = 30;
/// 令牌字节数（hex 编码后长度翻倍）
const TOKEN_BYTES: usize = 32;

pub struct AuthService;

impl AuthService {
    /// 注册：成功返回 user_id
    pub fn register(pool: &DbPool, username: &str, password: &str) -> AppResult<i64> {
        // 参数校验
        if username.len() < 3 || username.len() > 32 {
            return Err(AppError::InvalidParam("用户名长度需为 3-32".into()));
        }
        if password.len() < 6 {
            return Err(AppError::InvalidParam("密码至少 6 位".into()));
        }

        // 用户名唯一性
        if UserRepo::find_by_username(pool, username)?.is_some() {
            return Err(AppError::Conflict("用户名已存在".into()));
        }

        let hash = Self::hash_password(password)?;
        let id = UserRepo::create(pool, username, &hash)?;
        Ok(id)
    }

    /// 登录：校验密码并更新最近登录时间
    pub fn login(pool: &DbPool, username: &str, password: &str) -> AppResult<i64> {
        let user = UserRepo::find_by_username(pool, username)?
            .ok_or_else(|| AppError::AuthError("用户名或密码错误".into()))?;

        Self::verify_password(password, &user.password_hash)?;
        UserRepo::touch_login(pool, user.id)?;
        Ok(user.id)
    }

    /// 为已登录用户创建会话令牌（记住登录）
    /// 返回明文 token，由前端持久化；后端只存 SHA-256 哈希。
    pub fn create_session(pool: &DbPool, user_id: i64, device_info: Option<&str>) -> AppResult<String> {
        let token = Self::generate_token();
        let hash = Self::hash_token(&token);
        SessionTokenRepo::create(pool, user_id, &hash, device_info, SESSION_TTL_DAYS)?;
        Ok(token)
    }

    /// 通过会话令牌登录：校验成功返回 user_id，并刷新令牌过期时间
    pub fn login_with_token(pool: &DbPool, token: &str) -> AppResult<i64> {
        let hash = Self::hash_token(token);
        SessionTokenRepo::find_valid_and_refresh(pool, &hash, SESSION_TTL_DAYS)?
            .ok_or_else(|| AppError::AuthError("登录已过期，请重新登录".into()))
    }

    /// 登出：清理会话令牌
    pub fn logout(pool: &DbPool, user_id: i64) -> AppResult<()> {
        SessionTokenRepo::delete_by_user(pool, user_id)?;
        tracing::info!(user_id, "用户登出，会话已清理");
        Ok(())
    }

    // ---- 密码哈希 ----

    fn hash_password(password: &str) -> AppResult<String> {
        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();
        let hash = argon2
            .hash_password(password.as_bytes(), &salt)
            .map_err(|e| AppError::Internal(format!("密码哈希失败: {e}")))?;
        Ok(hash.to_string())
    }

    fn verify_password(password: &str, hashed: &str) -> AppResult<()> {
        let parsed = PasswordHash::new(hashed)
            .map_err(|e| AppError::Internal(format!("密码哈希解析失败: {e}")))?;
        let argon2 = Argon2::default();
        argon2
            .verify_password(password.as_bytes(), &parsed)
            .map_err(|_| AppError::AuthError("用户名或密码错误".into()))
    }

    // ---- 会话令牌 ----

    /// 生成高熵随机令牌（hex 编码）
    fn generate_token() -> String {
        let mut buf = vec![0u8; TOKEN_BYTES];
        rand::thread_rng().fill_bytes(&mut buf);
        hex::encode(buf)
    }

    /// 计算令牌哈希（SHA-256，hex 编码）
    fn hash_token(token: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(token.as_bytes());
        hex::encode(hasher.finalize())
    }
}
