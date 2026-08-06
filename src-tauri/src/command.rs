// Tauri 命令层（前后端契约）
// 每个 command 返回 Result<ApiResponse<T>, AppError>
pub mod auth;
pub mod distraction;
pub mod helpers;
pub mod notes;
pub mod pomodoro;
pub mod settings;
pub mod stats;
