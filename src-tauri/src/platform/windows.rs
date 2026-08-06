//! Windows 平台适配：活跃窗口信息 + 输入空闲时长
//!
//! 使用 Win32 FFI 实现，避免引入重型 GUI 依赖。
//! 注意：这些 API 仅在 Windows 编译期可用。

use std::mem;

/// 顶层窗口信息（用于设置页「选择应用」对话框）
#[derive(Debug, Clone, serde::Serialize)]
pub struct WindowInfo {
    pub app_name: String,
    pub window_title: String,
}

/// EnumWindows 回调：返回 true 继续枚举，false 停止
extern "system" fn enum_callback(hwnd: *mut core::ffi::c_void, lparam: isize) -> i32 {
    unsafe {
        let ctx = &mut *(lparam as *mut (Vec<WindowInfo>, std::collections::HashSet<String>));
        let (windows, seen) = ctx;

        // 仅可见窗口
        if IsWindowVisible(hwnd) == 0 {
            return 1;
        }
        // 过滤工具窗口（悬浮小部件等）
        let ex_style = GetWindowLongW(hwnd, GWL_EXSTYLE) as u32;
        if (ex_style & WS_EX_TOOLWINDOW) != 0 {
            return 1;
        }

        // 窗口标题
        let mut title_buf = vec![0u16; 512];
        let title_len = GetWindowTextW(hwnd, title_buf.as_mut_ptr(), title_buf.len() as i32);
        let title = if title_len > 0 {
            String::from_utf16_lossy(&title_buf[..title_len as usize])
        } else {
            return 1;
        };
        if title.trim().is_empty() {
            return 1;
        }

        // 进程名
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, &mut pid);
        let app_name = process_name_from_pid(pid).unwrap_or_default();
        if app_name.is_empty() {
            return 1;
        }

        // 去重：同一应用只保留一个条目
        if seen.insert(app_name.clone()) {
            windows.push(WindowInfo { app_name, window_title: title });
        }
        1
    }
}

/// 枚举所有可见顶层窗口，按 app_name 去重
pub fn list_top_windows() -> Vec<WindowInfo> {
    let mut ctx: (Vec<WindowInfo>, std::collections::HashSet<String>) = (Vec::new(), std::collections::HashSet::new());
    unsafe {
        EnumWindows(enum_callback, &mut ctx as *mut _ as isize);
    }
    ctx.0
}

/// 活跃窗口信息（应用名 / 窗口标题）
pub fn active_window_info() -> Option<(String, String)> {
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.is_null() {
            return None;
        }

        // 窗口标题
        let mut title_buf = vec![0u16; 512];
        let title_len = GetWindowTextW(hwnd, title_buf.as_mut_ptr(), title_buf.len() as i32);
        let title = if title_len > 0 {
            String::from_utf16_lossy(&title_buf[..title_len as usize])
        } else {
            String::new()
        };

        // 进程 ID → 进程名
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, &mut pid);
        let app_name = process_name_from_pid(pid).unwrap_or_default();

        Some((app_name, title))
    }
}

fn process_name_from_pid(pid: u32) -> Option<String> {
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if handle.is_null() {
            return None;
        }
        let mut buf = vec![0u16; 512];
        let mut len = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(handle, 0, buf.as_mut_ptr(), &mut len);
        let _ = CloseHandle(handle);
        if ok == 0 {
            return None;
        }
        let path = String::from_utf16_lossy(&buf[..len as usize]);
        // 取文件名
        path.rfind('\\')
            .map(|i| path[i + 1..].to_string())
            .or(Some(path))
    }
}

/// 系统输入空闲秒数（键盘/鼠标自上次输入以来的时间）
pub fn idle_seconds() -> i64 {
    unsafe {
        let mut lii = LASTINPUTINFO {
            cbSize: mem::size_of::<LASTINPUTINFO>() as u32,
            dwTime: 0,
        };
        if GetLastInputInfo(&mut lii) == 0 {
            return 0;
        }
        let tick = GetTickCount();
        let idle_ms = tick.saturating_sub(lii.dwTime);
        idle_ms as i64 / 1000
    }
}

// ---- Win32 FFI ----

#[repr(C)]
#[allow(non_snake_case)] // 字段名与 Win32 LASTINPUTINFO 头文件保持一致
struct LASTINPUTINFO {
    cbSize: u32,
    dwTime: u32,
}

#[link(name = "user32")]
extern "system" {
    fn GetForegroundWindow() -> *mut core::ffi::c_void;
    fn GetWindowTextW(hWnd: *mut core::ffi::c_void, lpString: *mut u16, nMaxCount: i32) -> i32;
    fn GetWindowThreadProcessId(
        hWnd: *mut core::ffi::c_void,
        lpdwProcessId: *mut u32,
    ) -> u32;
    fn GetLastInputInfo(plii: *mut LASTINPUTINFO) -> i32;
    fn EnumWindows(
        lpenumfunc: extern "system" fn(*mut core::ffi::c_void, isize) -> i32,
        lparam: isize,
    ) -> i32;
    fn IsWindowVisible(hWnd: *mut core::ffi::c_void) -> i32;
    fn GetWindowLongW(hWnd: *mut core::ffi::c_void, nIndex: i32) -> i32;
}

#[link(name = "kernel32")]
extern "system" {
    fn GetTickCount() -> u32;
    fn CloseHandle(hObject: *mut core::ffi::c_void) -> i32;
    fn OpenProcess(
        dwDesiredAccess: u32,
        bInheritHandle: i32,
        dwProcessId: u32,
    ) -> *mut core::ffi::c_void;
    fn QueryFullProcessImageNameW(
        hProcess: *mut core::ffi::c_void,
        dwFlags: u32,
        lpExeName: *mut u16,
        lpdwSize: *mut u32,
    ) -> i32;
}

const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;
const GWL_EXSTYLE: i32 = -20;
const WS_EX_TOOLWINDOW: u32 = 0x00000080;
