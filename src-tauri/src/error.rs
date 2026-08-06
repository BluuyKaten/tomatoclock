//! 应用错误类型 + 统一 Result
//!
//! 错误码对齐上游 §7.1：
//!   0     成功
//!   -1    通用错误
//!   1001  认证失败
//!   1002  参数错误
//!   2001  LLM 调用失败

use std::fmt;

use serde::Serialize;
use thiserror::Error;

pub type AppResult<T> = std::result::Result<T, AppError>;

/// 发送给前端的错误信封（与 ApiResponse 同结构）
#[derive(Debug, Serialize)]
pub struct ApiErrorEnvelope {
    pub code: i32,
    pub msg: String,
}

#[derive(Error, Debug)]
pub enum AppError {
    #[error("认证失败: {0}")]
    AuthError(String),

    #[error("未登录")]
    AuthRequired,

    #[error("参数错误: {0}")]
    InvalidParam(String),

    #[error("未找到: {0}")]
    NotFound(String),

    #[error("业务冲突: {0}")]
    Conflict(String),

    #[error("数据库错误: {0}")]
    Db(#[from] rusqlite::Error),

    // [FIX] 添加 From<std::io::Error> 支持，使 create_dir_all 可用 ? 操作符
    #[error("IO 错误: {0}")]
    Io(#[from] std::io::Error),

    #[error("内部错误: {0}")]
    Internal(String),

    #[error("LLM 调用失败: {0}")]
    Llm(String),
}

impl AppError {
    /// 对应前端错误码
    pub fn code(&self) -> i32 {
        match self {
            AppError::AuthError(_) | AppError::AuthRequired => 1001,
            AppError::InvalidParam(_) => 1002,
            AppError::Llm(_) => 2001,
            // [FIX] 添加 Io 错误码映射
            AppError::NotFound(_) | AppError::Conflict(_) | AppError::Db(_) | AppError::Io(_) | AppError::Internal(_) => -1,
        }
    }

    /// 构建统一错误信封
    pub fn to_envelope(&self) -> ApiErrorEnvelope {
        ApiErrorEnvelope {
            code: self.code(),
            msg: self.to_string(),
        }
    }
}

/// 为 Tauri command 提供统一错误序列化：把 AppError 变成 JSON 字符串返回给前端
///
/// Tauri v2 中 command 返回 Result<T, E> 时 E 需要实现 Serialize。
/// 我们让 AppError 通过自定义序列化输出 {code, msg} 结构。
impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeMap;
        let mut map = serializer.serialize_map(Some(2))?;
        map.serialize_entry("code", &self.code())?;
        map.serialize_entry("msg", &self.to_string())?;
        map.end()
    }
}

/// 便捷：把任意错误字符串转成 Internal
pub fn internal_err<S: fmt::Display>(msg: S) -> AppError {
    AppError::Internal(msg.to_string())
}
