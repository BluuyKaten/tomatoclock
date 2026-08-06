# 分心检测增强：暂停计时 + 弹窗提醒 实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 检测到分心应用时暂停番茄钟、只计一次分心、弹出应用内模态弹窗 + 系统通知；用户回到专注时自动恢复。

**Architecture:** 后端 `DistractionService` 持有 `Arc<TimerService>`，在 `check_window` 中根据 `matches_rule` + `distracted` 状态转换调用 `timer.pause()` / `resume()`，并通过事件总线发 `DistractionPaused` / `DistractionResumed`。前端监听这两个事件，更新 `distractionStore` 并弹出模态弹窗 + 抢焦点。

**Tech Stack:** Rust (chrono, tokio, tauri, rusqlite, tracing), Svelte 5, Tauri 2, TypeScript

## Global Constraints

- 所有时间字段均为 Unix 毫秒（i64）
- 事件名约定：`tomatoclock://<snake_case_tag>`
- 后端使用 `tracing` 日志，前端使用 `pushToast` 提示
- TimerService 使用 `std::sync::Mutex`，不会阻塞 tokio 运行时
- DistractionService 使用 `tokio::sync::Mutex`
- 新增事件必须是 `#[serde(tag = "kind", rename_all = "snake_case")]` 枚举变体
- 前端使用 Tauri v2 的 `@tauri-apps/api/event` listen + `@tauri-apps/api/window` getCurrentWindow

---

## 文件结构

### 修改

| 文件 | 职责 |
|------|------|
| `src-tauri/src/events.rs` | 新增 `DistractionPaused` / `DistractionResumed` 事件类型与事件名映射 |
| `src-tauri/src/service/distraction.rs` | 持有 `Arc<TimerService>`、`distracted` 状态字段、状态转换逻辑 |
| `src-tauri/src/lib.rs` | setup 中构建 `Arc<TimerService>` 并注入 `DistractionService` |

### 新增

| 文件 | 职责 |
|------|------|
| `frontend/src/stores/distraction.ts` | 分心弹窗状态 store（isDistracted / appName / windowTitle） |

### 修改（前端）

| 文件 | 职责 |
|------|------|
| `frontend/src/App.svelte` | 监听分心事件、更新 store、渲染分心模态弹窗组件 |

---

### Task 1: 新增 DistractionPaused / DistractionResumed 事件

**Files:**
- Modify: `src-tauri/src/events.rs:38-44`（DistractionDetected 之后追加新事件）
- Modify: `src-tauri/src/events.rs:57-68`（event_name 匹配臂）

**Interfaces:**
- Consumes: 无
- Produces:
  - `AppEvent::DistractionPaused { pomodoro_id: i64, app_name: Option<String>, window_title: Option<String> }`
  - `AppEvent::DistractionResumed { pomodoro_id: i64 }`
  - 事件名常量：`tomatoclock://distraction-paused`、`tomatoclock://distraction-resumed`

- [ ] **Step 1: 在 AppEvent 枚举中追加两个事件变体**

在 `src-tauri/src/events.rs` 的 `DistractionDetected` 变体之后、`ReminderTriggered` 之前，追加：

```rust
    /// 分心暂停：检测到分心窗口，计时器已暂停
    DistractionPaused {
        pomodoro_id: i64,
        app_name: Option<String>,
        window_title: Option<String>,
    },
    /// 分心恢复：用户回到专注，计时器已恢复
    DistractionResumed {
        pomodoro_id: i64,
    },
```

- [ ] **Step 2: 在 event_name 方法中追加匹配臂**

在 `impl AppEvent` 的 `event_name` 方法中，`DistractionDetected` 匹配臂之后追加：

```rust
            AppEvent::DistractionPaused { .. } => "tomatoclock://distraction-paused",
            AppEvent::DistractionResumed { .. } => "tomatoclock://distraction-resumed",
```

- [ ] **Step 3: 编译验证**

Run: `cargo check --manifest-path src-tauri/Cargo.toml`
Expected: 编译通过，零错误

- [ ] **Step 4: 提交**

```bash
git add src-tauri/src/events.rs
git commit -m "feat(events): 新增 DistractionPaused / DistractionResumed 事件"
```

---

### Task 2: 提取状态转换纯函数（可测试核心逻辑）

**Files:**
- Modify: `src-tauri/src/service/distraction.rs:137-149`（DistractionRuntime 定义）

**Interfaces:**
- Consumes: 无
- Produces:
  - `pub enum DistractionTransition { EnteredDistraction, ExitedDistraction, NoChange }`
  - `pub fn evaluate_distraction_transition(distracted: bool, matches_rule: bool) -> DistractionTransition`

- [ ] **Step 1: 在 service/distraction.rs 顶部（imports 之后）定义纯函数**

```rust
/// 分心状态转换结果
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DistractionTransition {
    /// 进入分心状态（需要暂停计时 + 记录分心）
    EnteredDistraction,
    /// 退出分心状态（需要恢复计时）
    ExitedDistraction,
    /// 状态无变化
    NoChange,
}

/// 纯函数：根据当前状态与窗口匹配结果，计算状态转换
/// 可独立单测，无需 Tauri / tokio 环境
pub fn evaluate_distraction_transition(distracted: bool, matches_rule: bool) -> DistractionTransition {
    match (distracted, matches_rule) {
        (false, true) => DistractionTransition::EnteredDistraction,
        (true, false) => DistractionTransition::ExitedDistraction,
        _ => DistractionTransition::NoChange,
    }
}
```

- [ ] **Step 2: 追加单元测试模块**

在 `src-tauri/src/service/distraction.rs` 文件末尾追加：

```rust
#[cfg(test)]
mod tests {
    use super::evaluate_distraction_transition;
    use super::DistractionTransition;

    #[test]
    fn test_entered_distraction_when_not_distracted_and_matches() {
        assert_eq!(
            evaluate_distraction_transition(false, true),
            DistractionTransition::EnteredDistraction
        );
    }

    #[test]
    fn test_exited_distraction_when_distracted_and_not_matches() {
        assert_eq!(
            evaluate_distraction_transition(true, false),
            DistractionTransition::ExitedDistraction
        );
    }

    #[test]
    fn test_no_change_when_not_distracted_and_not_matches() {
        assert_eq!(
            evaluate_distraction_transition(false, false),
            DistractionTransition::NoChange
        );
    }

    #[test]
    fn test_no_change_when_distracted_and_matches() {
        assert_eq!(
            evaluate_distraction_transition(true, true),
            DistractionTransition::NoChange
        );
    }
}
```

- [ ] **Step 3: 运行单元测试**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib service::distraction::tests -- --nocapture`
Expected: running 4 tests / 4 passed

- [ ] **Step 4: 提交**

```bash
git add src-tauri/src/service/distraction.rs
git commit -m "feat(distraction): 提取分心状态转换纯函数并单测"
```

---

### Task 3: DistractionRuntime 新增 distracted 字段 + 构造函数注入 TimerService

**Files:**
- Modify: `src-tauri/src/service/distraction.rs:127-167`（DistractionService 结构与构造函数）
- Modify: `src-tauri/src/service/distraction.rs:137-149`（DistractionRuntime 字段）

**Interfaces:**
- Consumes: `crate::service::timer::TimerService`
- Produces: `DistractionService::new` 新签名（第 4 参数 `timer: std::sync::Arc<TimerService>`）

- [ ] **Step 1: 在 imports 追加 TimerService 导入**

在 `src-tauri/src/service/distraction.rs` 文件顶部 imports 区域追加：

```rust
use crate::service::timer::TimerService;
```

- [ ] **Step 2: DistractionRuntime 新增 distracted 字段**

```rust
#[derive(Debug)]
struct DistractionRuntime {
    enabled: bool,
    current_pomodoro: Option<i64>,
    current_user: Option<i64>,
    last_window_check: Option<std::time::Instant>,
    last_input_check: Option<std::time::Instant>,
    tick_stop: Option<tokio::sync::oneshot::Sender<()>>,
    last_notification: Option<std::time::Instant>,
    /// [FIX] 是否处于分心暂停状态（检测到分心窗口后 true，回到专注后 false）
    distracted: bool,
}
```

- [ ] **Step 3: 构造函数签名变更 + 初始化**

```rust
pub struct DistractionService {
    inner: Arc<Mutex<DistractionRuntime>>,
    pool: DbPool,
    bus: EventBus,
    app: tauri::AppHandle,
    /// [FIX] 持有计时器服务，用于分心时暂停 / 恢复
    timer: std::sync::Arc<TimerService>,
}

impl DistractionService {
    pub fn new(
        app: tauri::AppHandle,
        bus: EventBus,
        pool: DbPool,
        timer: std::sync::Arc<TimerService>,
    ) -> Self {
        Self {
            inner: Arc::new(Mutex::new(DistractionRuntime {
                enabled: false,
                current_pomodoro: None,
                current_user: None,
                last_window_check: None,
                last_input_check: None,
                tick_stop: None,
                last_notification: None,
                distracted: false,
            })),
            pool,
            bus,
            app,
            timer,
        }
    }
```

- [ ] **Step 4: on_pomodoro_start 重置 distracted 状态**

在 `rt.last_input_check = None;` 之后追加：

```rust
        rt.distracted = false;
```

- [ ] **Step 5: 编译验证（此时 DistractionService::new 调用点会报错，属正常，Task 6 修复）**

Run: `cargo check --manifest-path src-tauri/Cargo.toml 2>&1 | grep -E "error|warning: unused" | head -20`
Expected: 仅 `lib.rs` 中 `DistractionService::new` 调用报错（参数数量不匹配），无其他错误

- [ ] **Step 6: 提交**

```bash
git add src-tauri/src/service/distraction.rs
git commit -m "feat(distraction): 注入 TimerService + distracted 状态字段"
```

---

### Task 4: tick_once 中分心期间跳过输入空闲检测

**Files:**
- Modify: `src-tauri/src/service/distraction.rs:249-288`（tick_once 方法）

**Interfaces:**
- Consumes: `rt.distracted` 字段
- Produces: 修改后的 `tick_once` 逻辑

- [ ] **Step 1: 重写 tick_once 的输入空闲检测部分**

将 `tick_once` 中以下代码：

```rust
        if should_check_window {
            rt.last_window_check = Some(now);
        }
        // 提前释放锁，避免在 await 中持锁
        drop(rt);

        if should_check_window {
            Self::check_window(inner, pool, bus, app, user_id, pomodoro_id).await?;
        }
        Self::check_input(inner, pool, bus, app, user_id, pomodoro_id, idle_threshold).await?;
```

替换为：

```rust
        if should_check_window {
            rt.last_window_check = Some(now);
        }
        // [FIX] 分心暂停期间跳过输入空闲检测，避免重复触发
        let distracted = rt.distracted;
        // 提前释放锁，避免在 await 中持锁
        drop(rt);

        if should_check_window {
            Self::check_window(inner, pool, bus, app, user_id, pomodoro_id).await?;
        }
        if !distracted {
            Self::check_input(inner, pool, bus, app, user_id, pomodoro_id, idle_threshold).await?;
        }
```

- [ ] **Step 2: 编译验证**

Run: `cargo check --manifest-path src-tauri/Cargo.toml 2>&1 | grep -E "^error" | head -10`
Expected: 仅 `lib.rs` DistractionService::new 调用报错

- [ ] **Step 3: 提交**

```bash
git add src-tauri/src/service/distraction.rs
git commit -m "feat(distraction): 分心暂停期间跳过输入空闲检测"
```

---

### Task 5: check_window 实现状态转换 + 调用 timer.pause/resume

**Files:**
- Modify: `src-tauri/src/service/distraction.rs:290-318`（check_window 方法）

**Interfaces:**
- Consumes: `evaluate_distraction_transition`、`self.timer`、`DistractionTransition`
- Produces: 状态转换后的副作用（timer.pause/resume + record + 事件 emit）

- [ ] **Step 1: 重写 check_window 方法**

将整个 `check_window` 方法体替换为：

```rust
    async fn check_window(
        inner: &Arc<Mutex<DistractionRuntime>>,
        pool: &DbPool,
        bus: &EventBus,
        app: &tauri::AppHandle,
        user_id: i64,
        pomodoro_id: i64,
        timer: &std::sync::Arc<TimerService>,
    ) -> AppResult<()> {
        let Some(candidate) = WindowDetector.detect().await else {
            debug!("[distraction] 无法读取前台窗口信息");
            return Ok(());
        };
        debug!(
            app_name = %candidate.app_name.as_deref().unwrap_or(""),
            window_title = %candidate.window_title.as_deref().unwrap_or(""),
            "[distraction] 检测到前台窗口"
        );

        let matches_rule = Self::matches_rule(pool, user_id, &candidate);

        // 读取当前 distracted 状态
        let mut rt = inner.lock().await;
        let transition = evaluate_distraction_transition(rt.distracted, matches_rule);

        match transition {
            DistractionTransition::EnteredDistraction => {
                // 进入分心状态
                rt.distracted = true;
                let app_name = candidate.app_name.clone();
                let window_title = candidate.window_title.clone();
                let candidate_clone = candidate.clone();
                drop(rt); // 释放锁后再调用外部服务

                info!(
                    app_name = %app_name.as_deref().unwrap_or(""),
                    "[distraction] 命中规则，暂停计时并记录分心"
                );
                // 暂停计时器
                if let Err(e) = timer.pause(pool).await {
                    warn!(error = %e, "[distraction] 暂停计时器失败");
                }
                // 记录分心（+1）+ 系统通知
                Self::record(inner, pool, bus, app, user_id, pomodoro_id, candidate_clone, 0).await?;
                // 发事件通知前端弹窗
                bus.emit(AppEvent::DistractionPaused {
                    pomodoro_id,
                    app_name,
                    window_title,
                });
            }
            DistractionTransition::ExitedDistraction => {
                // 恢复专注
                rt.distracted = false;
                drop(rt);

                info!("[distraction] 回到专注，恢复计时");
                // 恢复计时器
                if let Err(e) = timer.resume(pool).await {
                    warn!(error = %e, "[distraction] 恢复计时器失败");
                }
                // 发事件通知前端关闭弹窗
                bus.emit(AppEvent::DistractionResumed { pomodoro_id });
            }
            DistractionTransition::NoChange => {
                debug!(
                    distracted = rt.distracted,
                    matches_rule,
                    "[distraction] 状态无变化"
                );
                drop(rt);
            }
        }

        Ok(())
    }
```

- [ ] **Step 2: 更新 check_window 调用点（补 timer 参数）**

在 `tick_once` 中找到：

```rust
            Self::check_window(inner, pool, bus, app, user_id, pomodoro_id).await?;
```

替换为：

```rust
            Self::check_window(inner, pool, bus, app, user_id, pomodoro_id, &self.timer).await?;
```

- [ ] **Step 3: 编译验证**

Run: `cargo check --manifest-path src-tauri/Cargo.toml 2>&1 | grep -E "^error" | head -10`
Expected: 仅 `lib.rs` DistractionService::new 调用报错（下一 Task 修复）

- [ ] **Step 4: 提交**

```bash
git add src-tauri/src/service/distraction.rs
git commit -m "feat(distraction): check_window 状态转换 + 暂停/恢复计时器"
```

---

### Task 6: lib.rs 注入 Arc\<TimerService\> 到 DistractionService

**Files:**
- Modify: `src-tauri/src/lib.rs:66-91`（setup 闭包）

**Interfaces:**
- Consumes: `TimerService`（Task 3）
- Produces: 正确装配的 `DistractionService`

- [ ] **Step 1: 重写 setup 中的服务装配**

将 `src-tauri/src/lib.rs` setup 闭包中以下代码：

```rust
            let timer = TimerService::new(handle.clone(), event_bus.clone());
            app.manage(timer);

            let pool = app.state::<AppState>().db().clone();
            let distraction = DistractionService::new(handle.clone(), event_bus.clone(), pool);
            app.manage(distraction);
```

替换为：

```rust
            let timer = std::sync::Arc::new(TimerService::new(handle.clone(), event_bus.clone()));
            app.manage(timer.clone());

            let pool = app.state::<AppState>().db().clone();
            let distraction = DistractionService::new(handle.clone(), event_bus.clone(), pool, timer.clone());
            app.manage(distraction);
```

- [ ] **Step 2: 完整编译验证**

Run: `cargo check --manifest-path src-tauri/Cargo.toml 2>&1 | tail -5`
Expected: Finished / 零错误（允许未使用导入警告，但应为零）

- [ ] **Step 3: 运行全量单测**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib 2>&1 | tail -10`
Expected: 所有测试通过（含 Task 2 的 4 个新测试）

- [ ] **Step 4: 提交**

```bash
git add src-tauri/src/lib.rs
git commit -m "feat: 注入 Arc<TimerService> 到 DistractionService"
```

---

### Task 7: 前端 distraction store + 事件监听

**Files:**
- Create: `frontend/src/stores/distraction.ts`
- Modify: `frontend/src/App.svelte`

**Interfaces:**
- Consumes: `@tauri-apps/api/event` listen、`@tauri-apps/api/window` getCurrentWindow
- Produces: `distractionStore`（isDistracted / appName / windowTitle）

- [ ] **Step 1: 新建 frontend/src/stores/distraction.ts**

```typescript
//! 分心弹窗状态 store
//! 当后端检测到分心窗口时，isDistracted=true，前端弹出模态提醒。

import { writable } from 'svelte/store';

export interface DistractionState {
  isDistracted: boolean;
  appName: string | null;
  windowTitle: string | null;
}

function createDistractionStore() {
  const { subscribe, set } = writable<DistractionState>({
    isDistracted: false,
    appName: null,
    windowTitle: null,
  });

  return {
    subscribe,
    set,
    reset: () => set({ isDistracted: false, appName: null, windowTitle: null }),
  };
}

export const distractionStore = createDistractionStore();
```

- [ ] **Step 2: 在 App.svelte 中导入依赖**

在 `frontend/src/App.svelte` 的 `<script>` 标签内追加：

```typescript
import { onMount } from 'svelte';
import { listen } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { distractionStore } from './stores/distraction';
```

- [ ] **Step 3: 在 App.svelte 中注册事件监听**

在 `<script>` 末尾追加：

```typescript
// [FIX] 监听分心暂停/恢复事件，驱动模态弹窗 + 抢焦点
onMount(async () => {
  // 分心暂停 → 弹窗 + 抢焦点
  await listen('tomatoclock://distraction-paused', (event) => {
    const payload = event.payload as {
      pomodoro_id: number;
      app_name?: string;
      window_title?: string;
    };
    distractionStore.set({
      isDistracted: true,
      appName: payload.app_name ?? null,
      windowTitle: payload.window_title ?? null,
    });
    // 抢焦点：强制把番茄钟窗口置顶
    getCurrentWindow().setFocus().catch(() => {});
  });

  // 分心恢复 → 关弹窗
  await listen('tomatoclock://distraction-resumed', () => {
    distractionStore.reset();
  });
});
```

- [ ] **Step 4: 前端类型检查**

Run: `cd frontend && npx svelte-check --tsconfig ./tsconfig.json 2>&1 | tail -10`
Expected: 零错误

- [ ] **Step 5: 提交**

```bash
git add frontend/src/stores/distraction.ts frontend/src/App.svelte
git commit -m "feat(frontend): 分心弹窗 store + 事件监听"
```

---

### Task 8: App.svelte 渲染分心模态弹窗组件

**Files:**
- Modify: `frontend/src/App.svelte`（Template + Style）

**Interfaces:**
- Consumes: `distractionStore`
- Produces: 居中模态弹窗 UI

- [ ] **Step 1: 在 App.svelte 的 markup 区域追加分心弹窗**

在 `</main>` 之后、根 `</div>` 之前（或根 markup 末尾）追加：

```svelte
<!-- [FIX] 分心提醒模态弹窗：检测到分心窗口时弹出 -->
{#if $distractionStore.isDistracted}
  <div class="distraction-modal-mask">
    <div class="distraction-modal card col gap-12">
      <h3>⚠️ 分心提醒</h3>
      <p>检测到您在分心：</p>
      <p class="app-name">{$distractionStore.appName ?? '未知应用'}</p>
      {#if $distractionStore.windowTitle}
        <p class="muted small">{$distractionStore.windowTitle}</p>
      {/if}
      <p class="muted small">番茄钟已暂停，请回到专注！</p>
      <button
        class="btn-primary"
        style="align-self:center"
        on:click={() => {
          // 用户已知晓；弹窗保持显示直到后端发 DistractionResumed 才关闭
          // （避免用户没真的回到专注就手动关掉）
        }}
      >
        我知道了
      </button>
    </div>
  </div>
{/if}
```

- [ ] **Step 2: 在 App.svelte 的 `<style>` 中追加弹窗样式**

```css
.distraction-modal-mask {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.5);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 2000;
}
.distraction-modal {
  width: min(420px, 90vw);
  padding: 32px;
  animation: modal-pop 0.2s ease-out;
}
.distraction-modal .app-name {
  font-weight: 600;
  font-size: 16px;
  text-align: center;
  padding: 8px 16px;
  background: rgba(239, 68, 68, 0.1);
  border-radius: 6px;
  color: #ef4444;
}
@keyframes modal-pop {
  from { transform: scale(0.9); opacity: 0; }
  to { transform: scale(1); opacity: 1; }
}
```

- [ ] **Step 3: 前端类型检查 + 构建验证**

Run: `cd frontend && npx svelte-check --tsconfig ./tsconfig.json 2>&1 | tail -10`
Expected: 零错误

- [ ] **Step 4: 提交**

```bash
git add frontend/src/App.svelte
git commit -m "feat(frontend): 分心提醒模态弹窗组件"
```

---

### Task 9: 端到端验证（手动测试）

**Files:** 无（手动验证步骤）

- [ ] **Step 1: 启动开发服务器**

Run: `cargo tauri dev`
Expected: 编译通过，应用启动

- [ ] **Step 2: 配置一个测试用分心规则**

在设置页添加一条黑名单规则，app_name 填系统必然存在的进程名（如当前测试机的某个可见应用）。

- [ ] **Step 3: 启动番茄钟 → 打开分心应用**

Expected:
- 计时器立即暂停（倒计时停止、status=Paused）
- 应用内弹出「⚠️ 分心提醒」模态弹窗
- 番茄钟窗口抢到前台焦点
- 系统通知「🍅 分心提醒」触发
- 分心次数显示 +1（不再累加）

- [ ] **Step 4: 切回专注应用（或桌面）**

Expected:
- 计时器自动恢复（倒计时继续、status=Running）
- 模态弹窗关闭
- 分心次数保持不变

- [ ] **Step 5: 反复切换验证稳定性**

快速在分心应用与专注应用间切换 3-5 次，验证：
- 每次「进入分心」只 +1 次分心计数
- 弹窗不重叠/不卡死
- 恢复后计时器行为正确

---

## 关键边界验证清单

完成 Task 9 时确认以下场景：

- [ ] 多个分心应用快速切换 → 只 +1 次
- [ ] 番茄结束时还在分心 → 不再发事件，无 panic
- [ ] 短暂误切（< 1s）→ 严格按"立即触发"暂停（预期行为）
- [ ] 后端日志无 `pause/resume 失败` 错误
- [ ] 前端控制台无未捕获异常

---

## 风险与回滚

- **风险：** TimerService 锁中毒（panic 时）。缓解：`pause()` / `resume()` 使用 `lock().expect()`，若发生中毒会在日志中暴露，可改为 `unwrap_or_else(|e| e.into_poison())`。
- **回滚：** 若新逻辑异常，还原 `check_window` 为直接 `record()` 调用（移除 `timer.pause()` / `timer.resume()` 与 `evaluate_distraction_transition` 调用），恢复 `tick_once` 中 `check_input` 无条件执行。
