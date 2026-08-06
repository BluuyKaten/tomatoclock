// [FIX] 新增文件：二进制入口
// Tauri 应用启动入口，调用 lib.rs 中的 run() 函数
fn main() {
    tomatoclock_lib::run()
}
