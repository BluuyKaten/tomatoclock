-- =============================================================================
-- 番茄钟 TomatoClock - 会话令牌
-- 版本：v1.1  日期：2026-08-05
--
-- 用于「记住用户名 + 自动登录」：
--   - 前端存明文 token 到 localStorage
--   - 后端只存 SHA-256 哈希（防数据库泄露）
--   - 30 天过期，每次验证通过刷新 expires_at
-- =============================================================================

CREATE TABLE IF NOT EXISTS session_tokens (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id     INTEGER NOT NULL,
    token_hash  TEXT    NOT NULL UNIQUE,   -- hex(SHA256(token))
    device_info TEXT,                      -- 可选，便于后续查看登录设备
    created_at  INTEGER NOT NULL,
    expires_at  INTEGER NOT NULL,
    last_used_at INTEGER NOT NULL,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_session_tokens_user  ON session_tokens(user_id);
CREATE INDEX IF NOT EXISTS idx_session_tokens_hash  ON session_tokens(token_hash);
CREATE INDEX IF NOT EXISTS idx_session_tokens_expire ON session_tokens(expires_at);
