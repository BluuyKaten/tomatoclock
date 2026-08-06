# 分心检测增强：暂停计时 + 弹窗提醒

> 创建日期：2026-08-07

## 背景

当前分心检测逻辑：后台 tick 循环每秒检测一次，**每次**命中分心规则都调用 `record()`，导致分心次数 1→2→3→… 持续累加，同时计时器继续运行。

**用户需求**：检测到分心应用时暂停番茄钟、只计一次分心、弹出番茄钟软件提示；用户回到专注时自动恢复。

## 决策记录

| 决策点 | 选择 |
|--------|------|
| 暂停后恢复方式 | 自动恢复（用户切回专注应用后立即继续倒计时） |
| 触发时机 | 立即触发（前景窗口一旦命中规则就暂停） |
| 分心计数口径 | 每次分心事件 +1（从专注→分心→回到专注算一次） |
| 提示形式 | 应用内模态弹窗 + 系统通知组合 |
| 弹窗是否抢焦点 | 抢焦点（强制把番茄钟窗口置顶） |
| 技术方案 | 方案 A：后端 DistractionService 直接持有 Arc<TimerService>，检测到分心时调用 pause/resume |

## 架构

```
┌─────────────────────────────────────────────────────────┐
│  Backend (Rust)                                         │
│                                                         │
│  ┌─────────────────┐    ┌─────────────────────────┐    │
│  │ DistractionService│───▶│ TimerService (Arc)      │    │
│  │ (tick 循环)      │    │ pause() / resume()      │    │
│  └────────┬─────────┘    └─────────────────────────┘    │
│           │                                             │
│           ▼                                             │
│  ┌─────────────────┐                                    │
│  │ EventBus        │                                    │
│  │ DistractionPaused / DistractionResumed              │
│  └────────┬─────────┘                                    │
└───────────│─────────────────────────────────────────────┘
            │ Tauri emit
            ▼
┌─────────────────────────────────────────────────────────┐
│  Frontend (Svelte)                                      │
│                                                         │
│  TimerView / App.svelte                                 │
│  - 监听 DistractionPaused → 弹窗 + 抢焦点               │
│  - 监听 DistractionResumed → 关弹窗                     │
│  - 监听 TimerTick → 刷新状态                            │
└─────────────────────────────────────────────────────────┘
```

## 状态机

后端 `DistractionRuntime` 新增 `distracted: bool` 字段：

```
             检测到分心窗口
[正常检测] ────────────────────▶ [分心暂停中]
   ▲                                  │
   │                                  │ 检测到窗口不再命中规则
   │         自动恢复                 │
   └──────────────────────────────────┘
```

## 数据流

### 检测到分心

```
后台 tick → check_window() → matches_rule() = true
  → 如果 distracted == false:
    → timer.pause()  (TimerService)
    → record()  (分心次数 +1, 系统通知)
    → emit DistractionPaused 事件
    → distracted = true
```

### 用户回到专注

```
后台 tick → check_window() → matches_rule() = false
  → 如果 distracted == true:
    → timer.resume()  (TimerService)
    → emit DistractionResumed 事件
    → distracted = false
```

## 实现细节

### 1. 后端 — events.rs

新增两个事件类型：

```rust
DistractionPaused {
    pomodoro_id: i64,
    app_name: Option<String>,
    window_title: Option<String>,
},
DistractionResumed {
    pomodoro_id: i64,
},
```

事件名：`tomatoclock://distraction-paused` / `tomatoclock://distraction-resumed`

### 2. 后端 — service/distraction.rs

**构造函数签名变更**：

```rust
pub fn new(
    app: tauri::AppHandle,
    bus: EventBus,
    pool: DbPool,
    timer: Arc<TimerService>,  // 新增
) -> Self
```

**DistractionRuntime 新增字段**：

```rust
struct DistractionRuntime {
    // ... 现有字段
    distracted: bool,  // 是否处于分心暂停状态
}
```

**on_pomodoro_start 重置状态**：

```rust
rt.distracted = false;
```

**tick_once 逻辑调整**：

- 窗口检测始终执行（用于判断恢复时机）
- 分心暂停期间跳过输入空闲检测，避免重复触发

**check_window 状态转换逻辑**：

```rust
async fn check_window(...) -> AppResult<()> {
    let Some(candidate) = WindowDetector.detect().await else {
        return Ok(());
    };

    let is_distracted = Self::matches_rule(pool, user_id, &candidate);
    let mut rt = inner.lock().await;

    if is_distracted && !rt.distracted {
        // 进入分心状态
        rt.distracted = true;
        drop(rt);
        // 暂停计时器（TimerService 使用 std::sync::Mutex，不会阻塞 tokio）
        timer.pause(pool).await?;
        // 记录分心（+1）+ 系统通知
        Self::record(inner, pool, bus, app, user_id, pomodoro_id, candidate, 0).await?;
        // 发事件通知前端弹窗
        bus.emit(AppEvent::DistractionPaused { ... });
    } else if !is_distracted && rt.distracted {
        // 恢复专注
        rt.distracted = false;
        drop(rt);
        // 恢复计时器
        timer.resume(pool).await?;
        // 发事件通知前端关闭弹窗
        bus.emit(AppEvent::DistractionResumed { pomodoro_id });
    } else {
        drop(rt);
    }
    Ok(())
}
```

### 3. 后端 — lib.rs

setup 中构建 `Arc<TimerService>` 并注入 `DistractionService`：

```rust
let timer = Arc::new(TimerService::new(handle.clone(), event_bus.clone()));
app.manage(timer.clone());

let distraction = DistractionService::new(
    handle.clone(),
    event_bus.clone(),
    pool,
    timer.clone(),  // 注入
);
app.manage(distraction);
```

### 4. 前端 — stores/distraction.ts（新建）

分心弹窗状态 store：

```typescript
import { writable } from 'svelte/store';

export interface DistractionState {
  isDistracted: boolean;
  appName: string | null;
  windowTitle: string | null;
}

export const distractionStore = writable<DistractionState>({
  isDistracted: false,
  appName: null,
  windowTitle: null,
});
```

### 5. 前端 — stores/index.ts 或 App.svelte

监听分心事件：

```typescript
import { listen } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { distractionStore } from './distraction';

// 分心暂停 → 弹窗 + 抢焦点
listen('tomatoclock://distraction-paused', (event) => {
  const payload = event.payload as { pomodoro_id: number; app_name?: string; window_title?: string };
  distractionStore.set({
    isDistracted: true,
    appName: payload.app_name ?? null,
    windowTitle: payload.window_title ?? null,
  });
  // 抢焦点：强制把番茄钟窗口置顶
  getCurrentWindow().setFocus();
});

// 分心恢复 → 关弹窗
listen('tomatoclock://distraction-resumed', () => {
  distractionStore.set({ isDistracted: false, appName: null, windowTitle: null });
});
```

### 6. 前端 — 分心弹窗组件

在 App.svelte 或 TimerView.svelte 中新增模态弹窗：

```svelte
{#if $distractionStore.isDistracted}
  <div class="distraction-modal-mask">
    <div class="distraction-modal">
      <h3>⚠️ 分心提醒</h3>
      <p>检测到您在分心：</p>
      <p class="app-name">{$distractionStore.appName}</p>
      <p class="muted small">番茄钟已暂停，请回到专注！</p>
      <button class="btn-primary" on:click={() => {/* 仅标记已知，等待自动恢复 */}}>
        我知道了
      </button>
    </div>
  </div>
{/if}
```

弹窗样式：居中模态，半透明遮罩。

## 边界情况处理

| 场景 | 处理方式 |
|------|----------|
| 多个分心应用快速切换 | `distracted` 状态保证只 +1 次，不重复暂停 |
| 番茄结束时还在分心 | `on_pomodoro_end` 设 `enabled = false`，不再检测 |
| 短暂误切 | 按用户选择"立即触发"，严格暂停（后续可改为持续 N 秒模式） |
| 输入空闲检测 | 分心暂停期间跳过，避免重复触发 |
| 应用退出时还在分心 | `on_pomodoro_end` 会清理状态 |
| TimerService 锁竞争 | `pause()` / `resume()` 使用 `std::sync::Mutex`，不会阻塞 tokio 运行时 |

## 配置项

无需新增配置项。复用现有：

- `distraction.window_check_interval_ms`：窗口检测间隔（默认 1000ms）
- `distraction.idle_threshold_sec`：输入空闲阈值（默认 30s）

## 测试策略

### 单元测试

- `matches_rule` 逻辑已有测试，继续保留
- 新增状态转换测试：模拟窗口切换序列，验证 `distracted` 状态转换正确

### 集成测试

- 启动番茄钟 → 模拟分心窗口 → 验证 timer.pause() 被调用 + DistractionPaused 事件发出
- 模拟恢复 → 验证 timer.resume() 被调用 + DistractionResumed 事件发出

### 手动测试

1. 启动番茄钟
2. 打开黑名单应用（如 chrome.exe）
3. 验证：计时器暂停、弹窗出现、窗口抢焦点、分心次数 +1
4. 切回专注应用（或桌面）
5. 验证：计时器恢复、弹窗关闭
6. 系统通知正常显示

## 文件清单

### 修改

- `src-tauri/src/events.rs` — 新增 DistractionPaused / DistractionResumed 事件
- `src-tauri/src/service/distraction.rs` — 持有 TimerService、状态转换、暂停检测循环
- `src-tauri/src/lib.rs` — 注入 TimerService 到 DistractionService

### 新增

- `frontend/src/stores/distraction.ts` — 分心弹窗状态 store

### 修改（前端）

- `frontend/src/stores/index.ts` 或 `frontend/src/App.svelte` — 监听分心事件
- `frontend/src/App.svelte` — 新增分心弹窗组件

## 风险与回滚

- **风险**：TimerService 被分心服务持有时，若 panic 可能导致锁中毒。回退方案：使用 `lock().unwrap_or_else(|e| e.into_poison())` 恢复中毒锁。
- **回滚**：若新逻辑有问题，可恢复 `check_window` 中旧的 `record()` 调用，移除 `timer.pause()` / `timer.resume()` 调用。
