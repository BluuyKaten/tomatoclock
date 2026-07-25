use chrono::Datelike;
use dioxus::desktop::{trayicon, use_tray_menu_event_handler, use_window};
use dioxus::prelude::*;
use rodio::Source;
use std::time::{Duration, Instant};

mod db;
mod audio;

const FAVICON: Asset = asset!("/assets/favicon.ico");
const MAIN_CSS: Asset = asset!("/assets/main.css");

fn main() {
    dioxus::LaunchBuilder::new()
        .with_cfg(dioxus::desktop::Config::new().with_window(
            dioxus::desktop::WindowBuilder::new()
                .with_min_inner_size(dioxus::desktop::LogicalSize::new(900.0, 800.0))
                .with_decorations(false),
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
    mut deadline: Signal<Option<(Instant, i32)>>,
    running: Signal<bool>,
    phase: Signal<Phase>,
    mins: i32,
) {
    work_mins.set(mins);
    if !running() && phase() == Phase::Work {
        remaining.set(mins * 60);
        deadline.set(None);
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
fn play_beep(freq: f32, file: Option<String>, volume: f32) {
    std::thread::spawn(move || {
        let _ = std::panic::catch_unwind(move || {
            let (_stream, handle) = rodio::OutputStream::try_default().unwrap();
            let sink = rodio::Sink::try_new(&handle).unwrap();
            match file {
                Some(path) => {
                    // 尝试解码并播放用户音频文件，失败则回退默认音
                    if let Ok(f) = std::fs::File::open(&path) {
                        if let Ok(source) = rodio::Decoder::new(f) {
                            sink.append(source.amplify(volume));
                            sink.sleep_until_end();
                            return;
                        }
                    }
                    let src = rodio::source::SineWave::new(freq)
                        .take_duration(Duration::from_millis(400))
                        .amplify(0.2 * volume);
                    sink.append(src);
                    sink.sleep_until_end();
                }
                None => {
                    let src = rodio::source::SineWave::new(freq)
                        .take_duration(Duration::from_millis(400))
                        .amplify(0.2 * volume);
                    sink.append(src);
                    sink.sleep_until_end();
                }
            }
        });
    });
}

// 白噪音：播放预制的音频文件，循环播放
// 全局持有 sink，切歌时停止旧的
type NoiseSink = std::sync::Arc<std::sync::Mutex<Option<rodio::Sink>>>;

static NOISE_SINK: std::sync::OnceLock<NoiseSink> = std::sync::OnceLock::new();

fn noise_sink() -> NoiseSink {
    NOISE_SINK
        .get_or_init(|| std::sync::Arc::new(std::sync::Mutex::new(None)))
        .clone()
}

// 引用整个 noise 文件夹，预置条目运行时按名称解析各文件路径
const NOISE_DIR: Asset = asset!("/assets/noise");

// 播放指定白噪音，停止旧的
// is_builtin=true 时 file_path 为 assets/noise 下的相对文件名；否则为绝对路径
fn play_noise(file_path: String, is_builtin: bool, volume: f32) {
    let sink_holder = noise_sink();
    // 停止旧的
    if let Ok(mut g) = sink_holder.lock() {
        if let Some(old) = g.take() {
            old.stop();
        }
    }
    std::thread::spawn(move || {
        let _ = std::panic::catch_unwind(move || {
            let source = match audio::StreamingSource::new(file_path.clone(), is_builtin) {
                Some(s) => s,
                None => {
                    eprintln!("[noise] failed to create streaming source for {file_path}");
                    return;
                }
            };
            let (stream, handle) = match rodio::OutputStream::try_default() {
                Ok(sh) => sh,
                Err(e) => {
                    eprintln!("[noise] OutputStream failed: {e}");
                    return;
                }
            };
            let sink = match rodio::Sink::try_new(&handle) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("[noise] Sink failed: {e}");
                    return;
                }
            };
            // 按音量放大并播放
            sink.append(source.amplify(volume));
            // 暂存 sink 以便外部停止；stream 随子线程存活
            {
                if let Ok(mut g) = sink_holder.lock() {
                    *g = Some(sink);
                }
            }
            // 阻塞直到 sink 被外部取走（停止）
            loop {
                std::thread::sleep(Duration::from_millis(300));
                let alive = sink_holder.lock().map(|g| g.is_some()).unwrap_or(false);
                if !alive {
                    break;
                }
            }
            drop(stream);
        });
    });
}

// 停止白噪音
fn stop_noise() {
    if let Ok(mut g) = noise_sink().lock() {
        if let Some(sink) = g.take() {
            sink.stop();
        }
    }
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
    // 深色模式开关（false=浅色，true=深色），从数据库读取
    let theme = use_signal(|| db::get_bool("theme", false));
    // 侧边栏是否收起，从数据库读取
    let collapsed = use_signal(|| db::get_bool("collapsed", false));
    // 通过 context 共享 collapsed 给 Timer 组件，专注时自动收起侧边栏
    use_context_provider(|| collapsed);
    // 副本用于响应变化时保存
    let theme_for_save = theme;
    let collapsed_for_save = collapsed;

    // 主题/侧边栏变化时立即保存
    use_effect(move || {
        db::set_bool("theme", theme_for_save());
    });
    use_effect(move || {
        db::set_bool("collapsed", collapsed_for_save());
    });

    // 初始化系统托盘（只执行一次）
    static TRAY_INIT: std::sync::Once = std::sync::Once::new();
    TRAY_INIT.call_once(|| {
        trayicon::init_tray_icon(build_tray_menu(), None);
        db::ensure_noise_defaults();
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
            TitleBar { theme }
            div { class: "main-body",
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
}

// 自定义标题栏：包含窗口控制按钮和可拖动区域
#[component]
fn TitleBar(theme: Signal<bool>) -> Element {
    let mut maximized = use_signal(|| false);
    let win = use_window().window.clone();
    let win_drag = win.clone();
    let win_min = win.clone();
    let win_max = win.clone();

    rsx! {
        div { class: "title-bar",
            div { class: "title-drag", onmousedown: move |_| { let _ = win_drag.drag_window(); } }
            div { class: "title-buttons",
                button {
                    class: "title-btn",
                    title: "最小化",
                    onclick: move |_| win_min.set_minimized(true),
                    "—"
                }
                button {
                    class: "title-btn",
                    title: if maximized() { "还原" } else { "最大化" },
                    onclick: move |_| {
                        let m = win_max.is_maximized();
                        if m {
                            win_max.set_maximized(false);
                            maximized.set(false);
                        } else {
                            win_max.set_maximized(true);
                            maximized.set(true);
                        }
                    },
                    if maximized() { "❐" } else { "▢" }
                }
                button {
                    class: "title-btn close",
                    title: "关闭",
                    onclick: move |_| -> () {
                        std::process::exit(0);
                    },
                    "✕"
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
    let mut work_mins = use_signal(|| db::get_int("work_mins", 25));
    let mut break_mins = use_signal(|| db::get_int("break_mins", 5));
    let mut remaining = use_signal(|| db::get_int("work_mins", 25) * 60);
    let mut running = use_signal(|| false);
    // 阶段结束弹窗：记录刚结束的阶段
    let mut popup = use_signal(|| None::<Phase>);
    // 提示音开关与音调（频率），从数据库读取
    let mut sound_on = use_signal(|| db::get_bool("sound_on", true));
    let mut beep_freq = use_signal(|| db::get_int("beep_freq", 660));
    // 用户自定义提示音文件（None 表示使用默认音）
    let mut sound_file = use_signal(|| db::get_setting("sound_file"));
    // 音量大小（0.0 ~ 1.0），从数据库读取
    let mut volume = use_signal(|| db::get_float("volume", 1.0));
    // 今日完成番茄数
    let mut count = use_signal(|| db::today_count());
    // 计时基准：记录本轮开始的时刻和剩余秒数，避免 sleep 累积漂移
    let mut deadline = use_signal(|| None::<(Instant, i32)>);

    // 设置变化时持久化
    {
        let work_mins_save = work_mins;
        let break_mins_save = break_mins;
        let sound_on_save = sound_on;
        let beep_freq_save = beep_freq;
        use_effect(move || {
            db::set_int("work_mins", work_mins_save());
        });
        use_effect(move || {
            db::set_int("break_mins", break_mins_save());
        });
        use_effect(move || {
            db::set_bool("sound_on", sound_on_save());
        });
        use_effect(move || {
            db::set_int("beep_freq", beep_freq_save());
        });
    }

    // 计时循环：基于真实时间戳计算剩余，避免 sleep 累积漂移
    use_future(move || async move {
        loop {
            if running() {
                if remaining() > 0 {
                    // 每次开始倒计时或重置后，记录截止时刻
                    if deadline().is_none() {
                        deadline.set(Some((Instant::now(), remaining())));
                    }
                    tokio::time::sleep(Duration::from_millis(200)).await;
                    if let Some((start, start_left)) = deadline() {
                        let elapsed = start.elapsed().as_secs() as i32;
                        let left = start_left - elapsed;
                        if left >= 0 {
                            remaining.set(left);
                        } else {
                            remaining.set(0);
                        }
                    }
                } else {
                    // 当前阶段结束：弹窗 + 提示音，并自动切换到下一阶段
                    deadline.set(None);
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
                        db::save_pomodoro(&ts, mins);
                        count.set(db::today_count());
                    }
                    // 开启提示音时播放（按选定的音调或自定义文件）
                    if sound_on() {
                        play_beep(beep_freq() as f32, sound_file(), volume());
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

    // 专注开始时缩小窗口（最小尺寸 + 实际尺寸）并收起侧边栏，停止时恢复
    let win = use_window().window.clone();
    let mut collapsed = use_context::<Signal<bool>>();
    use_effect(move || {
        let (min_size, target_size) = if running() {
            (
                dioxus::desktop::LogicalSize::new(400.0, 500.0),
                dioxus::desktop::LogicalSize::new(420.0, 520.0),
            )
        } else {
            (
                dioxus::desktop::LogicalSize::new(900.0, 800.0),
                dioxus::desktop::LogicalSize::new(900.0, 800.0),
            )
        };
        let _ = win.set_min_inner_size(Some(min_size));
        let _ = win.set_inner_size(target_size);
        // 专注时自动收起侧边栏
        if collapsed() != running() {
            collapsed.set(running());
        }
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
                            deadline.set(None);
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
                            deadline.set(None);
                        }
                    },
                }
                span { "分钟" }
            }
            div { class: "presets",
                button { onclick: move |_| apply_preset(work_mins, remaining, deadline, running, phase, 25), "25" }
                button { onclick: move |_| apply_preset(work_mins, remaining, deadline, running, phase, 45), "45" }
                button { onclick: move |_| apply_preset(work_mins, remaining, deadline, running, phase, 60), "60" }
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
            div { class: "volume-settings",
                span { "音量" }
                input {
                    r#type: "range",
                    min: "0",
                    max: "100",
                    value: "{(volume() * 100.0) as i32}",
                    oninput: move |e| {
                        let v = e.value().parse::<f32>().unwrap_or(100.0) / 100.0;
                        volume.set(v);
                        db::set_float("volume", v);
                    },
                }
                span { "{(volume() * 100.0) as i32}%" }
            }
            div { class: "sound-file",
                button {
                    onclick: move |_| {
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter("音频", &["mp3", "wav", "ogg", "flac", "m4a"])
                            .pick_file()
                        {
                            let p = path.to_string_lossy().to_string();
                            sound_file.set(Some(p.clone()));
                            db::set_setting("sound_file", &p);
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
                            db::set_setting("sound_file", "");
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
                        // 暂停时清除基准；开始时记录基准
                        if running() {
                            deadline.set(None);
                            running.set(false);
                        } else {
                            running.set(true);
                        }
                    },
                    "{toggle_label}"
                }
                button {
                    onclick: move |_| {
                        running.set(false);
                        deadline.set(None);
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

// 白噪音页面：支持增删改查
#[component]
fn Music() -> Element {
    // 正在播放的条目 id
    let mut playing = use_signal(|| None::<i64>);
    // 正在编辑名称的条目 id
    let mut editing = use_signal(|| None::<i64>);
    // 编辑中的名称
    let mut edit_name = use_signal(|| String::new());
    // 白噪音列表，从数据库加载
    let mut list = use_signal(|| db::load_noise_list());
    // 音量从数据库读取
    let mut volume = use_signal(|| db::get_float("noise_volume", 0.5));

    rsx! {
        div { class: "music",
            h1 { "白噪音" }
            div { class: "noise-volume",
                span { "音量" }
                input {
                    r#type: "range",
                    min: "0",
                    max: "100",
                    value: "{(volume() * 100.0) as i32}",
                    oninput: move |e| {
                        let v = e.value().parse::<f32>().unwrap_or(50.0) / 100.0;
                        volume.set(v);
                        db::set_float("noise_volume", v);
                        // 实时调整：重新播放当前条目
                        if let Some(id) = playing() {
                            if let Some(item) = list().iter().find(|n| n.id == id).cloned() {
                                play_noise(item.file_path, item.is_builtin, v);
                            }
                        }
                    },
                }
                span { "{(volume() * 100.0) as i32}%" }
            }
            for item in list() {
                {
                    let item = item.clone();
                    let item_for_play = item.clone();
                    let item_for_edit = item.clone();
                    let item_for_del = item.clone();
                    let is_playing = playing() == Some(item.id);
                    let is_editing = editing() == Some(item.id);
                    rsx! {
                        div { class: "sound-row",
                            if is_editing {
                                input {
                                    class: "name-edit",
                                    value: "{edit_name}",
                                    oninput: move |e| edit_name.set(e.value()),
                                    onkeydown: move |e| {
                                        if e.key() == Key::Enter {
                                            db::update_noise_name(item_for_edit.id, &edit_name());
                                            editing.set(None);
                                            list.set(db::load_noise_list());
                                        } else if e.key() == Key::Escape {
                                            editing.set(None);
                                        }
                                    },
                                }
                                button {
                                    class: "icon-btn",
                                    title: "保存",
                                    onclick: move |_| {
                                        db::update_noise_name(item_for_edit.id, &edit_name());
                                        editing.set(None);
                                        list.set(db::load_noise_list());
                                    },
                                    "✓"
                                }
                                button {
                                    class: "icon-btn",
                                    title: "取消",
                                    onclick: move |_| editing.set(None),
                                    "✕"
                                }
                            } else {
                                span { class: "sound-name", "{item.name}" }
                                button {
                                    class: "icon-btn",
                                    title: "编辑名称",
                                    onclick: move |_| {
                                        edit_name.set(item_for_edit.name.clone());
                                        editing.set(Some(item_for_edit.id));
                                    },
                                    "✎"
                                }
                                button {
                                    class: "icon-btn del",
                                    title: "删除",
                                    onclick: move |_| {
                                        if playing() == Some(item_for_del.id) {
                                            stop_noise();
                                            playing.set(None);
                                        }
                                        db::delete_noise(item_for_del.id);
                                        list.set(db::load_noise_list());
                                    },
                                    "🗑"
                                }
                                button {
                                    class: if is_playing { "play-btn active" } else { "play-btn" },
                                    onclick: move |_| {
                                        if is_playing {
                                            stop_noise();
                                            playing.set(None);
                                        } else {
                                            play_noise(item_for_play.file_path.clone(), item_for_play.is_builtin, volume());
                                            playing.set(Some(item_for_play.id));
                                        }
                                    },
                                    if is_playing { "⏸ 停止" } else { "▶ 播放" }
                                }
                            }
                        }
                    }
                }
            }
            div { class: "noise-add",
                button {
                    onclick: move |_| {
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter("音频", &["mp3", "mp4", "wav", "ogg", "flac", "m4a", "aac"])
                            .pick_file()
                        {
                            // 复制文件到数据目录，避免原文件移动后失效
                            let noise_dir = db::data_dir().join("noise");
                            let _ = std::fs::create_dir_all(&noise_dir);
                            let ext = path.extension().map(|e| e.to_string_lossy().to_string()).unwrap_or_else(|| "mp4".to_string());
                            let ts = chrono::Local::now().timestamp_millis();
                            let dest = noise_dir.join(format!("{ts}.{ext}"));
                            // 文件名取不含扩展名的部分作为初始名称
                            let name = path.file_stem().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "未命名".to_string());
                            if std::fs::copy(&path, &dest).is_ok() {
                                db::add_noise(&name, &dest.to_string_lossy(), false);
                                list.set(db::load_noise_list());
                            }
                        }
                    },
                    "＋ 添加白噪音"
                }
            }
            p { class: "hint", "点击播放，✎ 改名，🗑 删除，＋ 添加本地音频" }
            div { class: "noise-add",
                button {
                    onclick: move |_| {
                        // 生成 440Hz 正弦波测试音
                        let vol = volume();
                        let sink_holder = noise_sink();
                        if let Ok(mut g) = sink_holder.lock() {
                            if let Some(old) = g.take() { old.stop(); }
                        }
                        std::thread::spawn(move || {
                            let _ = std::panic::catch_unwind(move || {
                                let (stream, handle) = rodio::OutputStream::try_default().unwrap();
                                let sink = rodio::Sink::try_new(&handle).unwrap();
                                // 使用 rodio 的 SineWave 作为测试音
                                let source = rodio::source::SineWave::new(440.0)
                                    .take_duration(Duration::from_secs(2))
                                    .amplify(vol);
                                sink.append(source);
                                {
                                    if let Ok(mut g) = sink_holder.lock() {
                                        *g = Some(sink);
                                    }
                                }
                                loop {
                                    std::thread::sleep(Duration::from_millis(300));
                                    let alive = sink_holder.lock().map(|g| g.is_some()).unwrap_or(false);
                                    if !alive { break; }
                                }
                                drop(stream);
                            });
                        });
                    },
                    "🔊 测试音"
                }
            }
        }
    }
}

// 总览页面：展示当月日历与当年学习统计
#[component]
fn Overview(page: Signal<Page>) -> Element {
    let mut day_times = use_signal(|| db::load_day_times());
    // 每次进入总览页时重新从文件加载，避免重启才能看到新记录
    use_effect(move || {
        if page() == Page::Overview {
            day_times.set(db::load_day_times());
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
