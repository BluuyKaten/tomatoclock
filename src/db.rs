use rusqlite::{params, Connection};
use std::path::PathBuf;

// 数据库文件存放到用户数据目录，避免临时目录被系统清理
fn db_path() -> PathBuf {
    let base = dirs_or_temp();
    base.join("tomatoclock.db")
}

// 优先使用用户数据目录，回退到临时目录
fn dirs_or_temp() -> PathBuf {
    if let Some(d) = dirs_data_dir() {
        return d;
    }
    std::env::temp_dir()
}

#[cfg(not(target_os = "linux"))]
fn dirs_data_dir() -> Option<PathBuf> {
    // Windows: %APPDATA%\tomatoclock，macOS: ~/Library/Application Support/tomatoclock
    if let Some(base) = std::env::var_os("APPDATA") {
        let p = PathBuf::from(base).join("tomatoclock");
        let _ = std::fs::create_dir_all(&p);
        return Some(p);
    }
    None
}

#[cfg(target_os = "linux")]
fn dirs_data_dir() -> Option<PathBuf> {
    if let Some(base) = std::env::var_os("HOME") {
        let p = PathBuf::from(base).join(".config").join("tomatoclock");
        let _ = std::fs::create_dir_all(&p);
        return Some(p);
    }
    None
}

// 获取数据库连接，并确保表已创建
fn connect() -> rusqlite::Result<Connection> {
    let conn = Connection::open(db_path())?;
    // 番茄完成记录表
    conn.execute(
        "CREATE TABLE IF NOT EXISTS pomodoros (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            ts TEXT NOT NULL,
            date TEXT NOT NULL,
            time TEXT NOT NULL,
            mins INTEGER NOT NULL
        )",
        [],
    )?;
    // 设置表：key-value 形式
    conn.execute(
        "CREATE TABLE IF NOT EXISTS settings (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        )",
        [],
    )?;
    // 白噪音条目表
    conn.execute(
        "CREATE TABLE IF NOT EXISTS noise (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            file_path TEXT NOT NULL,
            is_builtin INTEGER NOT NULL DEFAULT 0
        )",
        [],
    )?;
    Ok(conn)
}

// 暴露数据目录给外部（用于复制导入的音频文件）
pub fn data_dir() -> PathBuf {
    dirs_or_temp()
}

// 白噪音条目
#[derive(Clone, PartialEq)]
pub struct NoiseItem {
    pub id: i64,
    pub name: String,
    pub file_path: String,
    pub is_builtin: bool,
}

// 首次启动时初始化预置白噪音（若表为空）
pub fn ensure_noise_defaults() {
    if let Ok(conn) = connect() {
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM noise", [], |row| row.get(0))
            .unwrap_or(0);
        if count == 0 {
            let defaults = [
                ("🌧 雨声", "rain.mp4"),
                ("🌲 森林", "forest.mp4"),
                ("🔥 壁炉", "fire.mp4"),
                ("🌊 海浪", "wave.mp4"),
            ];
            for (name, path) in defaults {
                let _ = conn.execute(
                    "INSERT INTO noise (name, file_path, is_builtin) VALUES (?1, ?2, 1)",
                    params![name, path],
                );
            }
        }
    }
}

// 读取全部白噪音条目（按 id 升序，预置在前）
pub fn load_noise_list() -> Vec<NoiseItem> {
    let mut list = Vec::new();
    if let Ok(conn) = connect() {
        let mut stmt = match conn.prepare(
            "SELECT id, name, file_path, is_builtin FROM noise ORDER BY is_builtin DESC, id ASC",
        ) {
            Ok(s) => s,
            Err(_) => return list,
        };
        let rows = stmt.query_map([], |row| {
            let is_builtin: i64 = row.get(3)?;
            Ok(NoiseItem {
                id: row.get(0)?,
                name: row.get(1)?,
                file_path: row.get(2)?,
                is_builtin: is_builtin != 0,
            })
        });
        if let Ok(rows) = rows {
            for row in rows {
                if let Ok(item) = row {
                    list.push(item);
                }
            }
        }
    }
    list
}

// 新增白噪音条目，返回新 id
pub fn add_noise(name: &str, file_path: &str, is_builtin: bool) -> i64 {
    if let Ok(conn) = connect() {
        let _ = conn.execute(
            "INSERT INTO noise (name, file_path, is_builtin) VALUES (?1, ?2, ?3)",
            params![name, file_path, is_builtin as i64],
        );
        return conn.last_insert_rowid();
    }
    0
}

// 修改白噪音名称
pub fn update_noise_name(id: i64, name: &str) {
    if let Ok(conn) = connect() {
        let _ = conn.execute(
            "UPDATE noise SET name = ?1 WHERE id = ?2",
            params![name, id],
        );
    }
}

// 删除白噪音条目
pub fn delete_noise(id: i64) {
    if let Ok(conn) = connect() {
        let _ = conn.execute("DELETE FROM noise WHERE id = ?1", params![id]);
    }
}

// 追加一条完成记录
pub fn save_pomodoro(ts: &str, mins: u32) {
    // ts 格式: YYYY-MM-DD HH:MM:SS
    let (date, time) = match ts.split_once(' ') {
        Some((d, t)) => (d, t),
        None => (ts, ""),
    };
    if let Ok(conn) = connect() {
        let _ = conn.execute(
            "INSERT INTO pomodoros (ts, date, time, mins) VALUES (?1, ?2, ?3, ?4)",
            params![ts, date, time, mins as i64],
        );
    }
}

// 读取全部记录，按日期分组为「日期 -> 该日各次(时间, 时长)」
pub fn load_day_times() -> std::collections::HashMap<String, Vec<(String, u32)>> {
    let mut map: std::collections::HashMap<String, Vec<(String, u32)>> =
        std::collections::HashMap::new();
    if let Ok(conn) = connect() {
        let mut stmt = match conn.prepare(
            "SELECT date, time, mins FROM pomodoros ORDER BY id ASC",
        ) {
            Ok(s) => s,
            Err(_) => return map,
        };
        let rows = stmt.query_map([], |row| {
            let date: String = row.get(0)?;
            let time: String = row.get(1)?;
            let mins: i64 = row.get(2)?;
            Ok((date, time, mins as u32))
        });
        if let Ok(rows) = rows {
            for row in rows {
                if let Ok((date, time, mins)) = row {
                    map.entry(date).or_default().push((time, mins));
                }
            }
        }
    }
    map
}

// 读取字符串类型设置
pub fn get_setting(key: &str) -> Option<String> {
    let conn = connect().ok()?;
    let mut stmt = conn.prepare("SELECT value FROM settings WHERE key = ?1").ok()?;
    let mut rows = stmt.query_map(params![key], |row| row.get::<_, String>(0)).ok()?;
    rows.next()?.ok()
}

// 写入设置（存在则更新）
pub fn set_setting(key: &str, value: &str) {
    if let Ok(conn) = connect() {
        let _ = conn.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        );
    }
}

// 读取布尔设置
pub fn get_bool(key: &str, default: bool) -> bool {
    get_setting(key)
        .and_then(|v| v.parse::<bool>().ok())
        .unwrap_or(default)
}

// 写入布尔设置
pub fn set_bool(key: &str, value: bool) {
    set_setting(key, &value.to_string());
}

// 读取整数设置
pub fn get_int(key: &str, default: i32) -> i32 {
    get_setting(key)
        .and_then(|v| v.parse::<i32>().ok())
        .unwrap_or(default)
}

// 写入整数设置
pub fn set_int(key: &str, value: i32) {
    set_setting(key, &value.to_string());
}

// 读取浮点设置
pub fn get_float(key: &str, default: f32) -> f32 {
    get_setting(key)
        .and_then(|v| v.parse::<f32>().ok())
        .unwrap_or(default)
}

// 写入浮点设置
pub fn set_float(key: &str, value: f32) {
    set_setting(key, &value.to_string());
}

// 今日完成番茄数
pub fn today_count() -> u32 {
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    if let Ok(conn) = connect() {
        if let Ok(count) = conn.query_row(
            "SELECT COUNT(*) FROM pomodoros WHERE date = ?1",
            params![today],
            |row| row.get::<_, i64>(0),
        ) {
            return count as u32;
        }
    }
    0
}
