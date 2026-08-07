<!--
  番茄钟主视图：开始/暂停/恢复/完成/放弃 + 实时倒计时
-->
<script lang="ts">
  import { onMount } from 'svelte';
  import { timer, timerDisplay, isTimerRunning, pushToast, startLocalTimer, stopLocalTimer } from '../stores/index';
  import { pomodoroApi } from './../api/client';

  let subjectId: number | undefined;
  let duration = 25; // 分钟
  let busy = false;

  onMount(async () => {
    try {
      const c = await pomodoroApi.current();
      if (c) {
        timer.set({
          id: c.id,
          remaining_seconds: c.remaining_seconds,
          planned_duration: c.planned_duration > 0 ? c.planned_duration : c.remaining_seconds,
          status: c.status,
          distraction_count: c.distraction_count ?? 0,
        });
        if (c.status === 0) startLocalTimer();
      }
    } catch { /* 未登录或无当前番茄 */ }
  });

  async function start() {
    console.log('[TimerView] start() 调用, duration=', duration);
    busy = true;
    try {
      const secs = duration * 60;
      console.log('[TimerView] 调用 pomodoroApi.start, secs=', secs);
      const r = await pomodoroApi.start(undefined, subjectId, secs);
      console.log('[TimerView] pomodoroApi.start 返回:', r);
      timer.set({
        id: r.id,
        remaining_seconds: r.planned_duration,
        planned_duration: r.planned_duration,
        status: r.status,
        distraction_count: 0,
      });
      // [FIX] 启动本地定时器驱动倒计时
      startLocalTimer();
      pushToast({ kind: 'success', message: '番茄开始！' });
    } catch (e) {
      console.error('[TimerView] start 失败:', e);
      pushToast({ kind: 'error', message: String(e) });
    } finally {
      busy = false;
    }
  }

  async function pause() {
    try {
      await pomodoroApi.pause();
      stopLocalTimer();
      timer.update((t) => ({ ...t, status: 1 }));
      pushToast({ kind: 'info', message: '已暂停' });
    } catch (e) { pushToast({ kind: 'error', message: String(e) }); }
  }
  async function resume() {
    try {
      await pomodoroApi.resume();
      startLocalTimer();
      timer.update((t) => ({ ...t, status: 0 }));
      pushToast({ kind: 'success', message: '继续专注' });
    } catch (e) { pushToast({ kind: 'error', message: String(e) }); }
  }
  async function complete() {
    try {
      stopLocalTimer();
      await pomodoroApi.complete();
    } catch (e) { pushToast({ kind: 'error', message: String(e) }); }
  }
  async function abandon() {
    if (!confirm('确定放弃本次番茄？')) return;
    try {
      stopLocalTimer();
      await pomodoroApi.abandon();
      timer.set({ id: null, remaining_seconds: 0, planned_duration: 0, status: -1, distraction_count: 0 });
    } catch (e) { pushToast({ kind: 'error', message: String(e) }); }
  }

  $: progress = $timer.planned_duration > 0
    ? (1 - $timer.remaining_seconds / $timer.planned_duration) * 100
    : 0;
</script>

<div class="timer-view col center gap-16">
  <div class="card timer-card col center gap-16">
    <div class="display">{$timerDisplay}</div>
    {#if $timer.id !== null}
      <div class="distraction-count" class:distraction-active={$timer.distraction_count > 0}>
        分心 <strong>{$timer.distraction_count}</strong> 次
      </div>
    {/if}
    <div class="progress">
      <div class="progress-bar" style="width: {Math.min(100, progress)}%"></div>
    </div>

    {#if !$isTimerRunning && $timer.id === null}
      <div class="row">
        <label for="duration-input" class="muted">时长（分钟）</label>
        <input id="duration-input" type="number" min="1" max="180" bind:value={duration} style="width:80px" />
      </div>
      <button class="btn-primary" disabled={busy} on:click={start}>开始专注</button>
    {:else if $isTimerRunning}
      <div class="row">
        <button class="btn-ghost" on:click={pause}>暂停</button>
        <button class="btn-danger" on:click={abandon}>放弃</button>
        <button class="btn-primary" on:click={complete}>完成</button>
      </div>
    {:else if $timer.id !== null}
      <div class="row">
        <button class="btn-primary" on:click={resume}>恢复</button>
        <button class="btn-danger" on:click={abandon}>放弃</button>
      </div>
    {/if}
  </div>
  <p class="muted">保持专注，番茄钟会常驻后台</p>
</div>

<style>
  .timer-view { height: 100%; }
  .timer-card {
    min-width: 360px;
    padding: 40px;
  }
  .display {
    font-size: 96px;
    font-weight: 200;
    font-variant-numeric: tabular-nums;
    color: var(--primary);
    letter-spacing: -2px;
  }
  .distraction-count {
    font-size: 14px;
    color: var(--text-muted, #6b7280);
    min-height: 20px;
  }
  .distraction-count.distraction-active {
    color: var(--danger, #ef4444);
  }
  .distraction-count strong {
    font-variant-numeric: tabular-nums;
  }
  .progress {
    width: 100%;
    height: 6px;
    background: var(--border);
    border-radius: 3px;
    overflow: hidden;
  }
  .progress-bar {
    height: 100%;
    background: var(--primary);
    transition: width 1s linear;
  }
</style>
