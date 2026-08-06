<!--
  根组件：登录检查 + 路由切换 + 事件监听
-->
<script lang="ts">
  import { onMount } from 'svelte';
  import {
    currentUser,
    currentRoute,
    toasts,
    timer,
    pushToast,
    loadPersistedToken,
    persistToken,
    persistRememberedUsername,
    type Route,
  } from './stores/index';
  import { authApi } from './api/client';
  import { listen } from '@tauri-apps/api/event';

  import LoginView from './views/LoginView.svelte';
  import TimerView from './views/TimerView.svelte';
  import NotesView from './views/NotesView.svelte';
  import StatsView from './views/StatsView.svelte';
  import SettingsView from './views/SettingsView.svelte';

  let loading = true;

  onMount(async () => {
    // 监听后端事件，刷新 UI
    await listen('tomatoclock://timer-tick', (e) => {
      const p = e.payload as { pomodoro_id: number; remaining_seconds: number; status: number };
      timer.update((t) => ({
        ...t,
        id: p.pomodoro_id,
        remaining_seconds: p.remaining_seconds,
        status: p.status,
        // [FIX] 保留 planned_duration：若已有值则保留，否则用 remaining 兜底（首次 tick 近似值）
        planned_duration: t.planned_duration > 0 ? t.planned_duration : p.remaining_seconds,
      }));
    });
    await listen('tomatoclock://pomodoro-completed', () => {
      timer.set({ id: null, remaining_seconds: 0, planned_duration: 0, status: 1, distraction_count: 0 });
      pushToast({ kind: 'success', message: '番茄完成！该休息了' });
    });
    await listen('tomatoclock://distraction', () => {
      // 实时累加当前番茄的分心次数
      timer.update((t) => ({ ...t, distraction_count: t.distraction_count + 1 }));
      pushToast({ kind: 'info', message: '检测到分心，请回到学习' });
    });

    // 自动登录：本地有会话令牌则尝试恢复会话
    const token = loadPersistedToken();
    if (token) {
      try {
        const r = await authApi.autoLogin(token);
        currentUser.set({ user_id: r.user_id, username: r.username });
        persistToken(r.token);
        persistRememberedUsername(r.username);
        pushToast({ kind: 'success', message: `欢迎回来，${r.username}` });
      } catch (e) {
        console.warn('[auto-login] 自动登录失败，需重新登录：', e);
        // 令牌无效 / 过期：清理本地状态
        persistToken(null);
      }
    }

    loading = false;
  });

  async function logout() {
    try {
      await authApi.logout();
      currentUser.set(null);
      persistToken(null);
      // 注意：保留记住的用户名，方便下次登录时自动填充
      pushToast({ kind: 'info', message: '已登出' });
    } catch (e) {
      pushToast({ kind: 'error', message: String(e) });
    }
  }

  const navItems: { key: Route; label: string }[] = [
    { key: 'timer', label: '番茄钟' },
    { key: 'notes', label: '笔记' },
    { key: 'stats', label: '统计' },
    { key: 'settings', label: '设置' },
  ];
</script>

{#if loading}
  <div class="center" style="height:100%">加载中…</div>
{:else if !$currentUser}
  <LoginView />
{:else}
  <div class="layout">
    <aside class="sidebar">
      <div class="brand">🍅 番茄钟</div>
      <nav class="col">
        {#each navItems as item}
          <button
            class="nav-item"
            class:active={$currentRoute === item.key}
            on:click={() => currentRoute.set(item.key)}
          >
            {item.label}
          </button>
        {/each}
      </nav>
      <div class="user-box">
        <span class="muted">{$currentUser.username}</span>
        <button class="btn-ghost" on:click={logout}>登出</button>
      </div>
    </aside>
    <main class="content">
      {#if $currentRoute === 'timer'}
        <TimerView />
      {:else if $currentRoute === 'notes'}
        <NotesView />
      {:else if $currentRoute === 'stats'}
        <StatsView />
      {:else if $currentRoute === 'settings'}
        <SettingsView />
      {/if}
    </main>
  </div>
{/if}

<!-- 全局消息 -->
<div class="toast-container">
  {#each $toasts as t}
    <div class="toast toast-{t.kind}">{t.message}</div>
  {/each}
</div>

<style>
  /* [FIX] 让 #app 撑满视口，避免登录/加载容器高度塌陷 */
  :global(#app) {
    height: 100vh;
    width: 100vw;
  }
  .layout {
    display: flex;
    height: 100%;
  }
  .sidebar {
    width: 200px;
    background: var(--surface);
    border-right: 1px solid var(--border);
    padding: 16px;
    display: flex;
    flex-direction: column;
    gap: 16px;
  }
  .brand {
    font-size: 18px;
    font-weight: 700;
    color: var(--primary);
    padding: 8px 0;
  }
  nav { gap: 4px; }
  .nav-item {
    text-align: left;
    padding: 10px 12px;
    border-radius: 8px;
    background: transparent;
    color: var(--text);
  }
  .nav-item:hover { background: rgba(0, 0, 0, 0.04); }
  .nav-item.active {
    background: var(--primary);
    color: #fff;
  }
  .user-box {
    margin-top: auto;
    display: flex;
    flex-direction: column;
    gap: 8px;
    border-top: 1px solid var(--border);
    padding-top: 12px;
  }
  .content {
    flex: 1;
    padding: 24px;
    overflow-y: auto;
  }
  .toast-container {
    position: fixed;
    top: 16px;
    right: 16px;
    display: flex;
    flex-direction: column;
    gap: 8px;
    z-index: 999;
  }
  .toast {
    padding: 10px 16px;
    border-radius: 8px;
    color: #fff;
    font-size: 14px;
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.15);
  }
  .toast-info { background: #3b82f6; }
  .toast-success { background: var(--success); }
  .toast-error { background: var(--danger); }
</style>
