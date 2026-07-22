use chrono::Datelike;
use dioxus::desktop::{trayicon, use_tray_menu_event_handler, use_window};
use dioxus::prelude::*;
use rodio::Source;
use std::time::Duration;

const FAVICON: Asset = asset!("/assets/favicon.ico");
const MAIN_CSS: Asset = asset!("/assets/main.css");

fn main() {
    dioxus::LaunchBuilder::new()
        .with_cfg(dioxus::desktop::Config::new().with_window(
            dioxus::desktop::WindowBuilder::new()
                .with_min_inner_size(dioxus::desktop::LogicalSize::new(900.0, 800.0)),
        ))
        .launch(App);
}

// 左侧导航对应的页面
#[derive(Clone, Copy, PartialEq)]
enum Page {
    Timer,
    Music,
    Overview,
}

// 番茄钟的两个阶段：专注与休息
#[derive(Clone, Copy, PartialEq)]
enum Phase {
    Work,
    Break,
}

impl Phase {
    fn label(&self) -> &'static str {
        match self {
            Phase::Work => "专注",
            Phase::Break => "休息",
        }
    }
}

// 根据自定义分钟数计算某阶段的时长（秒）
fn phase_duration(phase: Phase, work_mins: i32, break_mins: i32) -> i32 {
    match phase {
        Phase::Work => work_mins * 60,
        Phase::Break => break_mins * 60,
    }
}

// 重置某阶段时恢复的时间：专注回到设定的分钟数，休息回到 0
fn phase_reset(phase: Phase, work_mins: i32) -> i32 {
    match phase {
        Phase::Work => work_mins * 60,
        Phase::Break => 0,
    }
}

// 一键预设专注时长：更新设定，未运行时同步刷新当前倒计时
fn apply_preset(
    mut work_mins: Signal<i32>,
    mut remaining: Signal<i32>,
    running: Signal<bool>,
    phase: Signal<Phase>,
    mins: i32,
) {
    work_mins.set(mins);
    if !running() && phase() == Phase::Work {
        remaining.set(mins * 60);
    }
}

// 今天日期字符串，格式 YYYY-MM-DD
fn today_key() -> String {
    chrono::Local::now().format("%Y-%m-%d").to_string()
}

// 每次专注完成的时间戳存到单独文件，每行 YYYY-MM-DD HH:MM:SS
fn pomodoro_path() -> std::path::PathBuf {
    std::env::temp_dir().join("tomatoclock_pomodoros.txt")
}

// 追加一条完成记录（时间戳 + 该次专注时长，单位分钟）
fn save_pomodoro(ts: &str, mins: u32) {
    use std::io::Write;
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(pomodoro_path())
    {
        let _ = writeln!(f, "{ts} {mins}");
    }
}

// 读取全部记录，按日期分组为「日期 -> 该日各次(时间, 时长)」
fn load_day_times() -> std::collections::HashMap<String, Vec<(String, u32)>> {
    use std::collections::HashMap;
    let mut map: HashMap<String, Vec<(String, u32)>> = HashMap::new();
    if let Ok(content) = std::fs::read_to_string(pomodoro_path()) {
        for line in content.lines() {
            let mut parts = line.splitn(3, ' ');
            if let (Some(date), Some(time)) = (parts.next(), parts.next()) {
                // 兼容旧格式（只有时间戳、无时长字段）：默认按 25 分钟计
                let m = parts.next().and_then(|s| s.parse::<u32>().ok()).unwrap_or(25);
                map.entry(date.to_string()).or_default().push((time.to_string(), m));
            }
        }
    }
    map
}

// 配置持久化：单独文件保存自定义提示音路径
fn config_path() -> std::path::PathBuf {
    std::env::temp_dir().join("tomatoclock_config.txt")
}

fn load_sound_file() -> Option<String> {
    std::fs::read_to_string(config_path())
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn save_sound_file(path: &Option<String>) {
    match path {
        Some(p) => {
            let _ = std::fs::write(config_path(), p);
        }
        None => {
            let _ = std::fs::remove_file(config_path());
        }
    }
}

// 把秒数格式化为 HH:MM:SS
fn format_time(secs: i32) -> String {
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    let s = secs % 60;
    format!("{h:02}:{m:02}:{s:02}")
}

// 阶段结束播放提示音：提供了音频文件则播放文件，否则播放默认正弦音
fn play_beep(freq: f32, file: Option<String>) {
    std::thread::spawn(move || {
        let _ = std::panic::catch_unwind(move || {
            let (_stream, handle) = rodio::OutputStream::try_default().unwrap();
            let sink = rodio::Sink::try_new(&handle).unwrap();
            match file {
                Some(path) => {
                    // 尝试解码并播放用户音频文件，失败则回退默认音
                    if let Ok(f) = std::fs::File::open(&path) {
                        if let Ok(source) = rodio::Decoder::new(f) {
                            sink.append(source);
                            sink.sleep_until_end();
                            return;
                        }
                    }
                    let src = rodio::source::SineWave::new(freq)
                        .take_duration(Duration::from_millis(400))
                        .amplify(0.2);
                    sink.append(src);
                    sink.sleep_until_end();
                }
                None => {
                    let src = rodio::source::SineWave::new(freq)
                        .take_duration(Duration::from_millis(400))
                        .amplify(0.2);
                    sink.append(src);
                    sink.sleep_until_end();
                }
            }
        });
    });
}

// 构建系统托盘右键菜单
fn build_tray_menu() -> trayicon::DioxusTrayMenu {
    use trayicon::menu::{Menu, MenuItem};
    let menu = Menu::new();
    let show = MenuItem::with_id("show", "显示窗口", true, None);
    let hide = MenuItem::with_id("hide", "隐藏窗口", true, None);
    let quit = MenuItem::with_id("quit", "退出", true, None);
    menu.append_items(&[&show, &hide, &quit]).unwrap();
    menu
}

#[component]
fn App() -> Element {
    // 当前所在页面，由左侧导航切换
    let page = use_signal(|| Page::Timer);
    // 深色模式开关（false=浅色，true=深色）
    let theme = use_signal(|| false);
    // 侧边栏是否收起
    let collapsed = use_signal(|| false);

    // 初始化系统托盘（只执行一次）
    static TRAY_INIT: std::sync::Once = std::sync::Once::new();
    TRAY_INIT.call_once(|| {
        trayicon::init_tray_icon(build_tray_menu(), None);
    });

    // 托盘菜单事件：显示 / 隐藏 / 退出
    let win = use_window().window.clone();
    use_tray_menu_event_handler(move |event| match event.id.as_ref() {
        "show" => win.set_visible(true),
        "hide" => win.set_visible(false),
        "quit" => std::process::exit(0),
        _ => {}
    });

    rsx! {
        document::Link { rel: "icon", href: FAVICON }
        document::Link { rel: "stylesheet", href: MAIN_CSS }
        div { id: "container", class: if theme() { "dark" },
            Sidebar { page, theme, collapsed }
            div { class: "content",
                div { class: if page() == Page::Timer { "page active" } else { "page" },
                    Timer {}
                }
                div { class: if page() == Page::Music { "page active" } else { "page" },
                    Music {}
                }
                div { class: if page() == Page::Overview { "page active" } else { "page" },
                    Overview { page }
                }
            }
        }
    }
}

// 左侧导航栏
#[component]
fn Sidebar(page: Signal<Page>, theme: Signal<bool>, collapsed: Signal<bool>) -> Element {
    rsx! {
        div { class: if collapsed() { "sidebar collapsed" } else { "sidebar" },
            button {
                class: "collapse-btn",
                onclick: move |_| collapsed.set(!collapsed()),
                if collapsed() { "»" } else { "☰" }
            }
            h2 { class: "logo", "🍅 番茄钟" }
            button {
                class: if page() == Page::Timer { "active" },
                onclick: move |_| page.set(Page::Timer),
                span { class: "icon", "🍅" }
                span { class: "label", "番茄钟" }
            }
            button {
                class: if page() == Page::Music { "active" },
                onclick: move |_| page.set(Page::Music),
                span { class: "icon", "🎵" }
                span { class: "label", "白噪音" }
            }
            button {
                class: if page() == Page::Overview { "active" },
                onclick: move |_| page.set(Page::Overview),
                span { class: "icon", "📊" }
                span { class: "label", "总览" }
            }
            button { class: "theme-toggle", onclick: move |_| theme.set(!theme()),
                span { class: "icon", if theme() { "🌞" } else { "🌙" } }
                span { class: "label", if theme() { "浅色模式" } else { "深色模式" } }
            }
        }
    }
}

#[component]
fn Timer() -> Element {
    let mut phase = use_signal(|| Phase::Work);
    let mut work_mins = use_signal(|| 25);
    let mut break_mins = use_signal(|| 5);
    let mut remaining = use_signal(|| 25 * 60);
    let mut running = use_signal(|| false);
    // 阶段结束弹窗：记录刚结束的阶段
    let mut popup = use_signal(|| None::<Phase>);
    // 提示音开关与音调（频率）
    let mut sound_on = use_signal(|| true);
    let mut beep_freq = use_signal(|| 660);
    // 用户自定义提示音文件（None 表示使用默认音），从配置文件读取
    let mut sound_file = use_signal(|| load_sound_file());
    // 今日完成番茄数：从按日期保存的完成时间戳中读取，跨重启持久化
    let mut count = use_signal(|| {
        load_day_times()
            .get(&today_key())
            .map(|v| v.len() as u32)
            .unwrap_or(0)
    });

    // 计时循环：只在挂载时启动一次，内部根据 running/remaining 自行判断
    use_future(move || async move {
        loop {
            if running() {
                if remaining() > 0 {
                    tokio::time::sleep(Duration::from_secs(1)).await;
                    let left = remaining();
                    if left > 0 {
                        remaining.set(left - 1);
                    }
                } else {
                    // 当前阶段结束：弹窗 + 提示音，并自动切换到下一阶段
                    let finished = phase();
                    let next = match finished {
                        Phase::Work => Phase::Break,
                        Phase::Break => Phase::Work,
                    };
                    phase.set(next);
                    remaining.set(phase_duration(next, work_mins(), break_mins()));
                    popup.set(Some(finished));
                    // 专注阶段结束：记录本次完成时间与时长，并刷新今日计数
                    if finished == Phase::Work {
                        let ts = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
                        let mins = work_mins() as u32;
                        save_pomodoro(&ts, mins);
                        count.set(count() + 1);
                    }
                    // 开启提示音时播放（按选定的音调或自定义文件）
                    if sound_on() {
                        play_beep(beep_freq() as f32, sound_file());
                    }
                }
            } else {
                // 未运行时轻量轮询，避免空转占用 CPU
                tokio::time::sleep(Duration::from_millis(200)).await;
            }
        }
    });

    // 后台运行时在系统托盘显示剩余时间
    let tray = trayicon::use_tray_icon();
    use_effect(move || {
        if let Some(tray) = &tray {
            let tip = if running() {
                format!("🍅 {} · {}", phase().label(), format_time(remaining()))
            } else {
                "🍅 番茄钟".to_string()
            };
            let _ = tray.set_tooltip(Some(&tip));
        }
    });

    // 专注开始时缩小窗口最小尺寸，停止时恢复
    let win = use_window().window.clone();
    use_effect(move || {
        let size = if running() {
            dioxus::desktop::LogicalSize::new(400.0, 500.0)
        } else {
            dioxus::desktop::LogicalSize::new(900.0, 800.0)
        };
        let _ = win.set_min_inner_size(Some(size));
    });

    // 圆形进度环参数
    let total = phase_duration(phase(), work_mins(), break_mins()).max(1);
    let ratio = remaining() as f32 / total as f32;
    let radius = 140.0;
    let circumference = 2.0 * std::f32::consts::PI * radius;
    let dash_offset = circumference * (1.0 - ratio);

    let toggle_label = if running() { "暂停" } else { "开始" };

    // 当前自定义音频文件名（用于显示）
    let file_name = sound_file().map(|p| {
        std::path::Path::new(&p)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default()
    });

    rsx! {
        div { id: "timer",
            h1 { "{phase().label()}中" }
            div { class: "stats", "🍅 今日完成 {count} 个" }
            div { class: "ring-wrap",
                svg { width: "300", height: "300", view_box: "0 0 300 300",
                    circle {
                        class: "ring-bg",
                        cx: "150",
                        cy: "150",
                        r: "140",
                    }
                    circle {
                        class: if phase() == Phase::Work { "ring-fg work" } else { "ring-fg break" },
                        cx: "150",
                        cy: "150",
                        r: "140",
                        stroke_dasharray: "{circumference}",
                        stroke_dashoffset: "{dash_offset}",
                    }
                }
                div { class: "ring-center",
                    div {
                        id: "time",
                        class: if phase() == Phase::Work { "work" } else { "break" },
                        "{format_time(remaining())}"
                    }
                }
            }
            if !running() {
                div { class: "config",
            div { class: "settings",
                label { "专注" }
                input {
                    r#type: "number",
                    min: "1",
                    max: "120",
                    value: "{work_mins}",
                    oninput: move |e| {
                        let v = e.value().parse().unwrap_or(25).clamp(1, 120);
                        work_mins.set(v);
                        if !running() && phase() == Phase::Work {
                            remaining.set(v * 60);
                        }
                    },
                }
                label { "休息" }
                input {
                    r#type: "number",
                    min: "1",
                    max: "60",
                    value: "{break_mins}",
                    oninput: move |e| {
                        let v = e.value().parse().unwrap_or(5).clamp(1, 60);
                        break_mins.set(v);
                        if !running() && phase() == Phase::Break {
                            remaining.set(v * 60);
                        }
                    },
                }
                span { "分钟" }
            }
            div { class: "presets",
                button { onclick: move |_| apply_preset(work_mins, remaining, running, phase, 25), "25" }
                button { onclick: move |_| apply_preset(work_mins, remaining, running, phase, 45), "45" }
                button { onclick: move |_| apply_preset(work_mins, remaining, running, phase, 60), "60" }
            }
            div { class: "sound-settings",
                label { class: "switch",
                    input {
                        r#type: "checkbox",
                        checked: sound_on(),
                        onchange: move |e| sound_on.set(e.checked()),
                    }
                    "提示音"
                }
                span { "音调" }
                button { class: if beep_freq() == 440 { "active" }, onclick: move |_| beep_freq.set(440), "低" }
                button { class: if beep_freq() == 660 { "active" }, onclick: move |_| beep_freq.set(660), "中" }
                button { class: if beep_freq() == 880 { "active" }, onclick: move |_| beep_freq.set(880), "高" }
            }
            div { class: "sound-file",
                button {
                    onclick: move |_| {
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter("音频", &["mp3", "wav", "ogg", "flac", "m4a"])
                            .pick_file()
                        {
                            sound_file.set(Some(path.to_string_lossy().to_string()));
                            save_sound_file(&sound_file());
                        }
                    },
                    "选择音频"
                }
                if let Some(name) = file_name {
                    span { class: "file-name", "{name}" }
                    button {
                        class: "clear",
                        onclick: move |_| {
                            sound_file.set(None);
                            save_sound_file(&sound_file());
                        },
                        "默认音"
                    }
                } else {
                    span { class: "file-name", "默认音" }
                }
            }
                }
            }
            div { id: "controls",
                button {
                    onclick: move |_| {
                        running.set(!running());
                    },
                    "{toggle_label}"
                }
                button {
                    onclick: move |_| {
                        running.set(false);
                        remaining.set(phase_reset(phase(), work_mins()));
                    },
                    "重置"
                }
            }
        }

        // 阶段结束弹窗
        if let Some(finished) = popup() {
            div { class: "modal-overlay",
                div { class: "modal",
                    h2 {
                        if finished == Phase::Work {
                            "🍅 专注结束！"
                        } else {
                            "☕ 休息结束！"
                        }
                    }
                    p {
                        if finished == Phase::Work {
                            "起来活动一下吧～"
                        } else {
                            "回到座位，继续专注！"
                        }
                    }
                    button { onclick: move |_| popup.set(None), "好的" }
                }
            }
        }
    }
}

// 白噪音页面（播放功能后续接入真实音频）
#[component]
fn Music() -> Element {
    let mut playing = use_signal(|| None::<String>);
    let sounds = ["🌧 雨声", "🌲 森林", "🔥 壁炉", "🌊 海浪"];

    rsx! {
        div { class: "music",
            h1 { "白噪音" }
            for sound in sounds {
                div { class: "sound-row",
                    span { "{sound}" }
                    button {
                        onclick: move |_| {
                            let s = sound.to_string();
                            if playing() == Some(s.clone()) {
                                playing.set(None);
                            } else {
                                playing.set(Some(s));
                            }
                        },
                        if playing() == Some(sound.to_string()) {
                            "⏸ 停止"
                        } else {
                            "▶ 播放"
                        }
                    }
                }
            }
            p { class: "hint", "（播放功能后续接入真实音频）" }
        }
    }
}

// 总览页面：展示当月日历与当年学习统计
#[component]
fn Overview(page: Signal<Page>) -> Element {
    let mut day_times = use_signal(|| load_day_times());
    // 每次进入总览页时重新从文件加载，避免重启才能看到新记录
    use_effect(move || {
        if page() == Page::Overview {
            day_times.set(load_day_times());
        }
    });
    let now = chrono::Local::now().naive_local().date();
    let mut view_year = use_signal(|| now.year());
    let mut view_month = use_signal(|| now.month());
    // 选中查看详情的日期
    let mut selected = use_signal(|| None::<String>);

    let year = view_year();
    let month = view_month();

    // 构建月历网格
    let first = chrono::NaiveDate::from_ymd_opt(year, month, 1).unwrap_or(now);
    let last = if month == 12 {
        chrono::NaiveDate::from_ymd_opt(year + 1, 1, 1).unwrap()
    } else {
        chrono::NaiveDate::from_ymd_opt(year, month + 1, 1).unwrap()
    };
    let days = (last - first).num_days() as i32;
    let lead = first.weekday().num_days_from_monday() as usize;
    let today_str = now.format("%Y-%m-%d").to_string();

    // 当年统计
    let year_prefix = format!("{year}-");
    let year_total: u32 = day_times()
        .iter()
        .filter(|(d, _)| d.starts_with(&year_prefix))
        .map(|(_, v)| v.len() as u32)
        .sum();
    let active_days = day_times()
        .iter()
        .filter(|(d, _)| d.starts_with(&year_prefix))
        .count();

    // 选中日期的详情数据（在 rsx 外计算，避免 `let` 写在 rsx 的 if-let 块中）
    let sel = selected();
    let detail_count = sel
        .as_ref()
        .and_then(|d| day_times().get(d).map(|v| v.len() as u32))
        .unwrap_or(0);
    // 该日累计专注分钟数
    let detail_minutes = sel
        .as_ref()
        .and_then(|d| day_times().get(d).map(|v| v.iter().map(|(_, m)| *m).sum()))
        .unwrap_or(0);
    // 该日各次完成的时间与时长列表
    let detail_times = sel
        .as_ref()
        .and_then(|d| day_times().get(d).cloned())
        .unwrap_or_default();

    rsx! {
        div { class: "overview",
            div { class: "ov-summary",
                div { class: "ov-card",
                    h3 { "今年累计" }
                    p { "🍅 {year_total} 个" }
                }
                div { class: "ov-card",
                    h3 { "学习天数" }
                    p { "{active_days} 天" }
                }
            }
            div { class: "calendar",
                div { class: "cal-header",
                    button {
                        onclick: move |_| {
                            if month == 1 {
                                view_year.set(year - 1);
                                view_month.set(12);
                            } else {
                                view_month.set(month - 1);
                            }
                        },
                        "‹"
                    }
                    span { "{year}年{month}月" }
                    button {
                        onclick: move |_| {
                            if month == 12 {
                                view_year.set(year + 1);
                                view_month.set(1);
                            } else {
                                view_month.set(month + 1);
                            }
                        },
                        "›"
                    }
                }
                div { class: "cal-grid cal-weekdays",
                    for w in ["一", "二", "三", "四", "五", "六", "日"] {
                        div { class: "cal-cell cal-weekday", "{w}" }
                    }
                }
                div { class: "cal-grid",
                    for _ in 0..lead {
                        div { class: "cal-cell empty" }
                    }
                    for d in 1..=days {
                        {
                            let date_str = format!("{year:04}-{month:02}-{d:02}");
                            let c = day_times().get(&date_str).map(|v| v.len() as u32).unwrap_or(0);
                            let is_today = date_str == today_str;
                            rsx! {
                                div {
                                    class: if is_today {
                                        "cal-cell today"
                                    } else if c > 0 {
                                        "cal-cell has-data"
                                    } else {
                                        "cal-cell"
                                    },
                                    onclick: move |_| {
                                        if c > 0 {
                                            selected.set(Some(date_str.clone()));
                                        }
                                    },
                                    span { class: "cal-day", "{d}" }
                                    if c > 0 {
                                        span { class: "cal-count", "🍅{c}" }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            // 点击某天后的详情弹窗
            if let Some(date) = sel {
                div { class: "modal-overlay",
                    onclick: move |_| selected.set(None),
                    div { class: "modal",
                        h2 { "{date}" }
                        p { "🍅 完成 {detail_count} 个番茄" }
                        if detail_count > 0 {
                            p { "累计专注 {detail_minutes} 分钟" }
                            div { class: "detail-times",
                                for (t, m) in detail_times {
                                    div { class: "detail-time", "🕐 {t}（{m}分钟）" }
                                }
                            }
                        }
                        button { onclick: move |_| selected.set(None), "关闭" }
                    }
                }
            }
        }
    }
}
