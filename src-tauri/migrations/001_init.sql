-- =============================================================================
-- 番茄钟 TomatoClock - 数据库初始化迁移
-- 版本：v1.0  日期：2026-08-04
-- 严格对齐上游设计文档 §6
--
-- 规范：
--   - 表名 snake_case 复数
--   - 主键 id INTEGER PRIMARY KEY AUTOINCREMENT
--   - 时间戳 INTEGER（Unix 毫秒）
--   - 布尔值 INTEGER 0/1
--   - 启用外键：PRAGMA foreign_keys = ON
--
-- 对上游设计的最小填补（已用 TODO(设计待确认) 在代码中标记）：
--   [修补-1] note_images 增加 ON DELETE CASCADE（上游 §6.2 遗漏，见设计问题 #4）
--   [修补-2] app_settings 显式声明 PRIMARY KEY (user_id, key)（上游 §6.2 只文字描述，见 #6）
-- =============================================================================

PRAGMA foreign_keys = ON;

-- -----------------------------------------------------------------------------
-- 1. 用户账号
-- -----------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS users (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    username        TEXT    UNIQUE NOT NULL,
    password_hash   TEXT    NOT NULL,
    -- TODO(设计待确认 #1)：argon2 哈希已内含盐，salt 列冗余；为严格对齐上游保留，存入空字符串占位
    salt            TEXT    NOT NULL DEFAULT '',
    avatar_path     TEXT,
    is_cloud_bound  INTEGER NOT NULL DEFAULT 0,         -- 是否已绑定云账号（V2）
    last_login_at   INTEGER,
    created_at      INTEGER NOT NULL,
    updated_at      INTEGER NOT NULL
);

-- -----------------------------------------------------------------------------
-- 2. 科目/分类
-- -----------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS subjects (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id     INTEGER NOT NULL,
    name        TEXT    NOT NULL,
    color       TEXT,                                   -- #RRGGBB
    sort_order  INTEGER NOT NULL DEFAULT 0,
    created_at  INTEGER NOT NULL,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

-- -----------------------------------------------------------------------------
-- 3. 任务
--    status: 0 待办 / 1 进行中 / 2 完成 / 3 归档
-- -----------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS tasks (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id         INTEGER NOT NULL,
    subject_id      INTEGER,                            -- 关联科目（可空）
    title           TEXT    NOT NULL,
    estimate_pomos  INTEGER NOT NULL DEFAULT 0,
    status          INTEGER NOT NULL DEFAULT 0,
    due_at          INTEGER,
    completed_at    INTEGER,
    created_at      INTEGER NOT NULL,
    updated_at      INTEGER NOT NULL,
    FOREIGN KEY (user_id)    REFERENCES users(id)    ON DELETE CASCADE,
    FOREIGN KEY (subject_id) REFERENCES subjects(id) ON DELETE SET NULL
);

-- -----------------------------------------------------------------------------
-- 4. 番茄时段记录（核心表）
--    status: 0 进行中 / 1 完成 / 2 放弃 / 3 打断
-- -----------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS pomodoros (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id          INTEGER NOT NULL,
    task_id          INTEGER,
    subject_id       INTEGER,
    started_at       INTEGER NOT NULL,
    ended_at         INTEGER,
    planned_duration INTEGER NOT NULL,                  -- 计划时长（秒）
    actual_duration  INTEGER,                           -- 实际专注时长（秒）
    status           INTEGER NOT NULL,                  -- 0/1/2/3
    distraction_count INTEGER NOT NULL DEFAULT 0,
    note_id          INTEGER,                           -- 关联笔记（可空）
    created_at       INTEGER NOT NULL,
    FOREIGN KEY (user_id)    REFERENCES users(id)    ON DELETE CASCADE,
    FOREIGN KEY (task_id)    REFERENCES tasks(id)    ON DELETE SET NULL,
    FOREIGN KEY (subject_id) REFERENCES subjects(id) ON DELETE SET NULL
    -- 注意：note_id 外键见 study_notes 表注释（避免循环外键，见设计问题 #3）
);

-- 番茄表索引（统计主用）
CREATE INDEX IF NOT EXISTS idx_pomos_user_time  ON pomodoros(user_id, started_at);
CREATE INDEX IF NOT EXISTS idx_pomos_task      ON pomodoros(task_id);
CREATE INDEX IF NOT EXISTS idx_pomos_subject   ON pomodoros(subject_id);

-- -----------------------------------------------------------------------------
-- 5. 分心事件
--    distraction_type: 1 窗口 / 2 输入空闲 / 3 摄像头
--    reminder_level:   0 未提醒 / 1~4 对应渐进级别
-- -----------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS distractions (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    pomodoro_id      INTEGER NOT NULL,
    user_id          INTEGER NOT NULL,
    detected_at      INTEGER NOT NULL,
    distraction_type INTEGER NOT NULL,                  -- 1/2/3
    app_name         TEXT,
    app_wm_class     TEXT,
    window_title     TEXT,
    idle_seconds     INTEGER,                           -- type=2
    face_detected    INTEGER,                           -- type=3
    gaze_left        INTEGER,                           -- type=3
    reminder_level   INTEGER NOT NULL DEFAULT 0,
    created_at       INTEGER NOT NULL,
    FOREIGN KEY (pomodoro_id) REFERENCES pomodoros(id) ON DELETE CASCADE,
    FOREIGN KEY (user_id)     REFERENCES users(id)     ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_dist_pomo      ON distractions(pomodoro_id);
CREATE INDEX IF NOT EXISTS idx_dist_user_time ON distractions(user_id, detected_at);

-- -----------------------------------------------------------------------------
-- 6. 学习笔记
--    tags: JSON 数组字符串
-- -----------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS study_notes (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id     INTEGER NOT NULL,
    pomodoro_id INTEGER,                                -- 关联番茄（可空）
    subject_id  INTEGER,
    title       TEXT,
    content     TEXT    NOT NULL,                       -- 纯文本/Markdown
    tags        TEXT,                                   -- JSON 数组
    created_at  INTEGER NOT NULL,
    updated_at  INTEGER NOT NULL,
    FOREIGN KEY (user_id)     REFERENCES users(id)     ON DELETE CASCADE,
    FOREIGN KEY (pomodoro_id) REFERENCES pomodoros(id) ON DELETE SET NULL,
    FOREIGN KEY (subject_id)  REFERENCES subjects(id)  ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_notes_user_time ON study_notes(user_id, created_at);
CREATE INDEX IF NOT EXISTS idx_notes_subject  ON study_notes(subject_id);

-- -----------------------------------------------------------------------------
-- 7. 笔记图片（一对多）
--    [修补-1] 增加 ON DELETE CASCADE：上游 §6.2 遗漏此约束（设计问题 #4）
-- -----------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS note_images (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    note_id     INTEGER NOT NULL,
    file_path   TEXT    NOT NULL,                       -- 本地文件路径
    mime_type   TEXT,
    size_bytes  INTEGER,
    created_at  INTEGER NOT NULL,
    FOREIGN KEY (note_id) REFERENCES study_notes(id) ON DELETE CASCADE
);

-- -----------------------------------------------------------------------------
-- 8. 分心判定应用规则
--    rule_type: 1 黑名单 / 2 白名单
-- -----------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS app_rules (
    id                    INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id               INTEGER NOT NULL,
    rule_type             INTEGER NOT NULL,             -- 1/2
    app_name              TEXT    NOT NULL,
    window_title_pattern  TEXT,                         -- 正则
    is_enabled            INTEGER NOT NULL DEFAULT 1,
    created_at            INTEGER NOT NULL,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

-- -----------------------------------------------------------------------------
-- 9. 用户配置（KV）
--    [修补-2] 显式声明复合主键（设计问题 #6）
-- -----------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS app_settings (
    user_id     INTEGER NOT NULL,
    key         TEXT    NOT NULL,
    value       TEXT,                                   -- JSON
    updated_at  INTEGER NOT NULL,
    PRIMARY KEY (user_id, key),
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

-- -----------------------------------------------------------------------------
-- 10. 同步状态（V2 预留）
-- -----------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS sync_state (
    user_id     INTEGER PRIMARY KEY,
    last_sync_at INTEGER,
    device_id   TEXT,
    cursor      TEXT,
    updated_at  INTEGER NOT NULL,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

-- -----------------------------------------------------------------------------
-- 关于 pomodoros.note_id 与 study_notes.pomodoro_id 的循环外键（设计问题 #3）
-- 处理方式：
--   - study_notes.pomodoro_id 作为"笔记归属番茄"的权威外键，建外键约束。
--   - pomodoros.note_id 仅作为"番茄结束时的快速反查"冗余字段，不建 SQLite 外键约束，
--     由应用层在 complete_pomodoro 时回填；删除笔记时不清空此字段（避免级联环路）。
-- 这样既保留上游字段，又避免循环引用导致的插入/删除顺序死锁。
-- -----------------------------------------------------------------------------
