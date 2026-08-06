<!--
  设置视图：计时器配置 + 分心规则 + 摄像头/LLM 开关
-->
<script lang="ts">
  import { onMount } from 'svelte';
  import { settingsApi } from '../api/client';
  import { distractionApi, type RuleView, type WindowInfo } from '../api/client';
  import { pushToast } from '../stores/index';

  let focusDuration = 25;
  let shortBreak = 5;
  let longBreak = 15;
  let idleThreshold = 30;
  let windowInterval = 1000;

  let rules: RuleView[] = [];
  let newRuleType = 1;
  let newAppName = '';
  let newPattern = '';

  const ruleTypeLabel: Record<number, string> = { 1: '黑', 2: '白' };

  // 从 Record<string, unknown> 中安全读取数值配置
  const num = (s: Record<string, unknown>, k: string, fallback: number) => {
    const v = s[k];
    return typeof v === 'number' ? v : fallback;
  };

  async function loadSettings() {
    try {
      const { settings } = await settingsApi.get();
      focusDuration = num(settings, 'timer.focus_duration', 25);
      shortBreak = num(settings, 'timer.short_break', 5);
      longBreak = num(settings, 'timer.long_break', 15);
      idleThreshold = num(settings, 'distraction.idle_threshold_sec', 30);
      windowInterval = num(settings, 'distraction.window_check_interval_ms', 1000);
    } catch (e) { pushToast({ kind: 'error', message: String(e) }); }
  }

  async function loadRules() {
    try { rules = (await distractionApi.listRules()).rules; }
    catch (e) { pushToast({ kind: 'error', message: String(e) }); }
  }

  async function save() {
    try {
      await settingsApi.update({
        'timer.focus_duration': focusDuration,
        'timer.short_break': shortBreak,
        'timer.long_break': longBreak,
        'distraction.idle_threshold_sec': idleThreshold,
        'distraction.window_check_interval_ms': windowInterval,
      });
      pushToast({ kind: 'success', message: '已保存' });
    } catch (e) { pushToast({ kind: 'error', message: String(e) }); }
  }

  // 「选择应用」对话框状态
  let showAppPicker = false;
  let pickerWindows: WindowInfo[] = [];
  let pickerLoading = false;
  let pickerFilter = '';

  async function openAppPicker() {
    pickerLoading = true;
    showAppPicker = true;
    pickerFilter = '';
    try {
      const { windows } = await distractionApi.listTopWindows();
      pickerWindows = windows;
    } catch (e) {
      pushToast({ kind: 'error', message: String(e) });
      showAppPicker = false;
    } finally {
      pickerLoading = false;
    }
  }

  function selectWindow(w: WindowInfo) {
    newAppName = w.app_name;
    showAppPicker = false;
    pushToast({ kind: 'success', message: `已选择：${w.app_name}` });
  }

  function filteredPickerWindows() {
    const f = pickerFilter.trim().toLowerCase();
    return f
      ? pickerWindows.filter(
          (w) => w.app_name.toLowerCase().includes(f) || w.window_title.toLowerCase().includes(f)
        )
      : pickerWindows;
  }

  async function addRule() {
    try {
      await distractionApi.createRule({
        rule_type: newRuleType,
        app_name: newAppName,
        window_title_pattern: newPattern || undefined,
      });
      newAppName = '';
      newPattern = '';
      loadRules();
    } catch (e) { pushToast({ kind: 'error', message: String(e) }); }
  }

  async function toggleRule(r: RuleView) {
    try {
      await distractionApi.updateRule(r.id, { is_enabled: !r.is_enabled });
      loadRules();
    } catch (e) { pushToast({ kind: 'error', message: String(e) }); }
  }

  async function removeRule(r: RuleView) {
    try { await distractionApi.deleteRule(r.id); loadRules(); }
    catch (e) { pushToast({ kind: 'error', message: String(e) }); }
  }

  onMount(() => { loadSettings(); loadRules(); });
</script>

<div class="settings-view col gap-16">
  <h2>设置</h2>

  <div class="card col gap-16">
    <strong>计时器</strong>
    <label class="row"><span style="width:160px">专注时长（分钟）</span><input type="number" bind:value={focusDuration} /></label>
    <label class="row"><span style="width:160px">短休（分钟）</span><input type="number" bind:value={shortBreak} /></label>
    <label class="row"><span style="width:160px">长休（分钟）</span><input type="number" bind:value={longBreak} /></label>
    <button class="btn-primary" style="align-self:flex-start" on:click={save}>保存</button>
  </div>

  <div class="card col gap-16">
    <strong>分心检测</strong>
    <label class="row"><span style="width:160px">输入空闲阈值（秒）</span><input type="number" bind:value={idleThreshold} /></label>
    <label class="row"><span style="width:160px">窗口检测间隔（ms）</span><input type="number" bind:value={windowInterval} /></label>
    <p class="muted small">摄像头检测默认关闭（V1.1 启用）</p>
  </div>

  <div class="card col gap-16">
    <strong>应用分心规则</strong>
    <div class="row">
      <select bind:value={newRuleType}>
        <option value={1}>黑名单（命中即分心）</option>
        <option value={2}>白名单（不在名单即分心）</option>
      </select>
      <input class="flex-1" bind:value={newAppName} placeholder="应用名（如 chrome.exe）" />
      <input bind:value={newPattern} placeholder="标题正则（可选）" />
      <button class="btn-ghost" on:click={openAppPicker} title="从当前打开的窗口中选择">选择应用</button>
      <button class="btn-primary" on:click={addRule}>添加</button>
    </div>
    <div class="col" style="gap:4px">
      {#each rules as r}
        <div class="row">
          <span class="tag" class:tag-black={r.rule_type === 1} class:tag-white={r.rule_type === 2}>
            {ruleTypeLabel[r.rule_type]}
          </span>
          <span class="flex-1">{r.app_name}</span>
          {#if r.window_title_pattern}<span class="muted small">{r.window_title_pattern}</span>{/if}
          <button class="btn-ghost" on:click={() => toggleRule(r)}>{r.is_enabled ? '启用' : '禁用'}</button>
          <button class="btn-ghost" on:click={() => removeRule(r)}>删除</button>
        </div>
      {:else}
        <p class="muted">暂无规则</p>
      {/each}
    </div>
  </div>

  <!-- 「选择应用」对话框 -->
  {#if showAppPicker}
    <!-- [FIX] a11y：模态遮罩需具备键盘可达性（role + tabindex + keydown） -->
    <div
      class="modal-mask"
      role="button"
      tabindex="0"
      on:click|self={() => (showAppPicker = false)}
      on:keydown={(e) => {
        if (e.key === 'Enter' || e.key === ' ' || e.key === 'Escape') showAppPicker = false;
      }}
    >
      <div class="modal card col gap-12">
        <div class="row" style="justify-content:space-between">
          <strong>选择应用</strong>
          <button class="btn-ghost" on:click={() => (showAppPicker = false)}>关闭</button>
        </div>
        <input bind:value={pickerFilter} placeholder="搜索应用名或窗口标题…" />
        <div class="picker-list col" style="gap:4px">
          {#if pickerLoading}
            <p class="muted">正在枚举窗口…</p>
          {:else if filteredPickerWindows().length === 0}
            <p class="muted">没有可见窗口，或没有匹配「{pickerFilter}」的应用。</p>
          {:else}
            {#each filteredPickerWindows() as w}
              <button class="row picker-item" on:click={() => selectWindow(w)} title={w.window_title}>
                <span class="flex-1">{w.app_name}</span>
                <span class="muted small">{w.window_title}</span>
              </button>
            {/each}
          {/if}
        </div>
      </div>
    </div>
  {/if}
</div>

<style>
  .tag { padding: 2px 8px; border-radius: 4px; font-size: 12px; color: #fff; }
  .tag-black { background: #374151; }
  .tag-white { background: #10b981; }
  .small { font-size: 12px; }
  .modal-mask { position: fixed; inset: 0; background: rgba(0,0,0,0.4); display: flex; align-items: center; justify-content: center; z-index: 1000; }
  .modal { width: min(560px, 90vw); max-height: 70vh; overflow: auto; }
  .picker-list { max-height: 50vh; overflow: auto; }
  .picker-item { text-align: left; background: transparent; border: none; padding: 8px; border-radius: 6px; cursor: pointer; }
  .picker-item:hover { background: rgba(0,0,0,0.05); }
</style>
