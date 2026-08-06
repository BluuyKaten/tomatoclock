<!--
  登录/注册视图
-->
<script lang="ts">
  import { authApi } from '../api/client';
  import {
    currentUser,
    pushToast,
    loadRememberedUsername,
    persistRememberedUsername,
    persistToken,
  } from '../stores/index';

  // 主界面窗口尺寸（登录后放大）
  const MAIN_WIDTH = 980;
  const MAIN_HEIGHT = 680;

  let mode: 'login' | 'register' = 'login';
  let username = '';
  let password = '';
  let rememberMe = true; // 默认勾选「记住我」
  let busy = false;

  // 启动时自动填充上次记住的用户名
  const remembered = loadRememberedUsername();
  if (remembered) {
    username = remembered;
  }

  /**
   * 安全地放大窗口：延迟加载 Tauri window API，避免模块加载失败导致白屏。   */
  async function enlargeWindow() {
    try {
      const mod = await import('@tauri-apps/api/window');
      const win = mod.getCurrentWindow();
      const size = new mod.PhysicalSize(MAIN_WIDTH, MAIN_HEIGHT);
      await win.setSize(size);
      await win.center();
    } catch (e) {
      console.warn('[window] 放大失败（可忽略）：', e);
    }
  }

  async function submit() {
    busy = true;
    try {
      if (mode === 'login') {
        const r = await authApi.login(username, password, rememberMe);
        currentUser.set({ user_id: r.user_id, username: r.username });
        // 记住登录：持久化令牌 + 用户名
        persistToken(r.token ?? null);
        if (rememberMe) {
          persistRememberedUsername(username);
        } else {
          persistRememberedUsername(null);
        }
        pushToast({ kind: 'success', message: `欢迎回来，${r.username}` });
        await enlargeWindow();
      } else {
        await authApi.register(username, password);
        pushToast({ kind: 'success', message: '注册成功，请登录' });
        mode = 'login';
      }
    } catch (e) {
      pushToast({ kind: 'error', message: String(e) });
    } finally {
      busy = false;
    }
  }
</script>

<div class="login-wrap center">
  <div class="login-card col gap-16">
    <h1>🍅 番茄钟</h1>
    <p class="muted">{mode === 'login' ? '登录开始专注' : '创建本地账号'}</p>

    <label class="col" style="gap:4px">
      <span class="muted">用户名</span>
      <input bind:value={username} placeholder="3-32 位" />
    </label>
    <label class="col" style="gap:4px">
      <span class="muted">密码</span>
      <input type="password" bind:value={password} placeholder="至少 6 位" />
    </label>

    <!-- 记住我：仅登录模式显示 -->
    {#if mode === 'login'}
      <label class="row" style="gap:8px; align-items:center">
        <input type="checkbox" bind:checked={rememberMe} />
        <span class="muted" style="font-size:13px">记住我（30 天内自动登录）</span>
      </label>
    {/if}

    <button class="btn-primary" disabled={busy} on:click={submit}>
      {busy ? '处理中…' : mode === 'login' ? '登录' : '注册'}
    </button>

    <button class="btn-ghost" on:click={() => (mode = mode === 'login' ? 'register' : 'login')}>
      {mode === 'login' ? '没有账号？去注册' : '已有账号？去登录'}
    </button>
  </div>
</div>

<style>
  .login-wrap {
    height: 100%;
    width: 100%;
    background: linear-gradient(135deg, #fff5f5, #fff);
    padding: 16px;
  }
  .login-card {
    width: 100%;
    max-width: 320px;
    padding: 24px;
  }
  h1 { margin: 0; font-size: 24px; text-align: center; }
  .row { display: flex; }
</style>
