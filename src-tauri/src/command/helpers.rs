//! 命令层辅助函数

use crate::domain::responses::ApiResponse;
use crate::error::AppResult;

/// 构造成功响应
pub fn ok<T: serde::Serialize>(data: T) -> AppResult<ApiResponse<T>> {
    Ok(ApiResponse::ok(data))
}
