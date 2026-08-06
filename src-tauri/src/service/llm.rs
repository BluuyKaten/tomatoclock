//! LLM 深度复盘服务（V1.2 功能，当前为 trait 占位）
//!
//! 默认关闭；需用户在设置中显式授权 + 配置 API Key 后启用。
//! 失败返回 2001（AppError::Llm）。

use crate::db::DbPool;
use crate::domain::responses::*;
use crate::error::{AppError, AppResult};

pub struct LlmService;

impl LlmService {
    /// 调用 Claude API 生成复盘
    /// 当前为占位实现：返回 2001 错误提示用户该功能未启用
    pub async fn summary(
        _pool: &DbPool,
        _user_id: i64,
        _from: i64,
        _to: i64,
        _language: Option<&str>,
    ) -> AppResult<LlmSummaryResponse> {
        // TODO(V1.2)：接入 Claude API（claude-axon 或原生 HTTP）
        // 1. 读取 llm.api_key（需解密，见设计问题 #5）
        // 2. 读取 llm.enabled 配置，未启用则拒绝
        // 3. 组装 prompt：从 stats 服务取数据 → 生成自然语言复盘
        // 4. 本地缓存近期总结
        Err(AppError::Llm(
            "LLM 深度复盘为 V1.2 功能，当前版本未启用".into(),
        ))
    }
}
