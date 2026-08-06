<!--
  统计视图：总览 + 趋势 + 分心热点 + 规则分析
-->
<script lang="ts">
  import { onMount } from 'svelte';
  import {
    statsApi,
    type OverviewResponse,
    type TrendPoint,
    type DistractionHotspotResponse,
    type InsightItem,
  } from '../api/client';
  import { pushToast } from '../stores/index';
  import * as time from '../lib/time';

  // [FIX] 模板中不支持内联箭头函数，需提取到 script 中
  const tabs = ['overview', 'trend', 'hotspot', 'insights'] as const;
  const tabLabels: Record<string, string> = {
    overview: '总览',
    trend: '趋势',
    hotspot: '分心热点',
    insights: '学习分析',
  };
  // [FIX] 模板不支持对象字面量 + 计算属性访问语法，提取为函数
  const distractionTypeLabels: Record<number, string> = { 1: '窗口', 2: '输入空闲', 3: '摄像头' };
  function distractionLabel(type: number): string {
    return distractionTypeLabels[type] ?? String(type);
  }

  let overview: OverviewResponse | null = null;
  let trend: TrendPoint[] = [];
  let hotspot: DistractionHotspotResponse | null = null;
  let insights: InsightItem[] = [];
  // [FIX] 当前激活的统计标签页
  let tab: 'overview' | 'trend' | 'hotspot' | 'insights' = 'overview';

  let from = time.startOfWeekMs();
  let to = time.endOfDayMs();
  let granularity: 'day' | 'week' | 'month' = 'day';

  async function loadOverview() {
    try { overview = await statsApi.overview(from, to); }
    catch (e) { pushToast({ kind: 'error', message: String(e) }); }
  }
  async function loadTrend() {
    try { trend = (await statsApi.trend(from, to, granularity)).points; }
    catch (e) { pushToast({ kind: 'error', message: String(e) }); }
  }
  async function loadHotspot() {
    try { hotspot = await statsApi.distractionHotspot(from, to); }
    catch (e) { pushToast({ kind: 'error', message: String(e) }); }
  }
  async function loadInsights() {
    try { insights = (await statsApi.rulesSummary(from, to)).insights; }
    catch (e) { pushToast({ kind: 'error', message: String(e) }); }
  }

  async function reloadAll() {
    if (tab === 'overview') loadOverview();
    if (tab === 'trend') loadTrend();
    if (tab === 'hotspot') loadHotspot();
    if (tab === 'insights') loadInsights();
  }

  onMount(loadOverview);

  function setRange(kind: 'today' | 'week' | 'month') {
    const now = new Date();
    if (kind === 'today') {
      from = time.startOfDayMs(now);
      to = time.endOfDayMs(now);
      granularity = 'day';
    } else if (kind === 'week') {
      from = time.startOfWeekMs(now);
      to = time.endOfDayMs(now);
      granularity = 'day';
    } else {
      from = time.startOfMonthMs(now);
      to = time.endOfDayMs(now);
      granularity = 'day';
    }
    reloadAll();
  }
</script>

<div class="stats-view col gap-16">
  <div class="row">
    <h2 style="margin:0">数据统计</h2>
    <div class="flex-1"></div>
    <button class="btn-ghost" on:click={() => setRange('today')}>今日</button>
    <button class="btn-ghost" on:click={() => setRange('week')}>本周</button>
    <button class="btn-ghost" on:click={() => setRange('month')}>本月</button>
  </div>

  <div class="row" style="gap:4px">
    {#each tabs as t}
      <button
        class="btn-ghost"
        class:btn-primary={tab === t}
        style={tab === t ? 'background:var(--primary);color:#fff' : ''}
        on:click={() => { tab = t; reloadAll(); }}
      >
        {tabLabels[t]}
      </button>
    {/each}
  </div>

  {#if tab === 'overview' && overview}
    <div class="grid">
      <div class="card center col"><span class="muted">专注时长</span><strong>{overview.total_minutes} 分钟</strong></div>
      <div class="card center col"><span class="muted">完成番茄</span><strong>{overview.completed_pomos}</strong></div>
      <div class="card center col"><span class="muted">放弃番茄</span><strong>{overview.abandoned_pomos}</strong></div>
      <div class="card center col"><span class="muted">分心次数</span><strong>{overview.distraction_count}</strong></div>
      <div class="card center col"><span class="muted">分心率</span><strong>{overview.distraction_rate.toFixed(2)}</strong></div>
    </div>
    {#if overview.subject_distribution.length}
      <div class="card col" style="gap:8px">
        <strong>科目分布</strong>
        {#each overview.subject_distribution as d}
          <div class="row"><span class="flex-1">{d.name}</span><span class="muted">{d.minutes} 分钟</span></div>
        {/each}
      </div>
    {/if}
  {:else if tab === 'trend' && trend.length}
    <div class="card col" style="gap:8px">
      <div class="row">
        <strong class="flex-1">趋势</strong>
        <select bind:value={granularity} on:change={loadTrend}>
          <option value="day">按天</option>
          <option value="week">按周</option>
          <option value="month">按月</option>
        </select>
      </div>
      <div class="trend-chart">
        {#each trend as p}
          <div class="trend-bar col center" style="flex:1">
            <div class="bar" style="height: {Math.min(100, p.minutes / 2)}px"></div>
            <span class="muted small">{p.date.slice(5)}</span>
          </div>
        {/each}
      </div>
    </div>
  {:else if tab === 'hotspot' && hotspot}
    <div class="grid">
      <div class="card col" style="gap:8px">
        <strong>应用分心 Top</strong>
        {#each hotspot.by_app.slice(0, 5) as h}
          <div class="row"><span class="flex-1">{h.app_name ?? '未知'}</span><span class="muted">{h.count}</span></div>
        {/each}
      </div>
      <div class="card col" style="gap:8px">
        <strong>类型分布</strong>
        {#each hotspot.by_type as h}
          <div class="row"><span class="flex-1">{distractionLabel(h.type)}</span><span class="muted">{h.count}</span></div>
        {/each}
      </div>
    </div>
  {:else if tab === 'insights' && insights.length}
    <div class="col gap-16">
      {#each insights as i}
        <div class="card insight insight-{i.severity}">
          <strong>[{i.severity}]</strong>
          <p style="margin:4px 0 0">{i.message}</p>
        </div>
      {/each}
    </div>
  {:else}
    <p class="muted">加载中…</p>
  {/if}
</div>

<style>
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(160px, 1fr));
    gap: 12px;
  }
  .trend-chart {
    display: flex;
    align-items: flex-end;
    gap: 6px;
    height: 160px;
    padding-top: 16px;
  }
  .bar {
    width: 100%;
    background: var(--primary);
    border-radius: 4px 4px 0 0;
    min-height: 4px;
  }
  .trend-bar { height: 100%; justify-content: flex-end; }
  .small { font-size: 11px; }
  .insight { border-left: 4px solid var(--text-muted); }
  .insight-warn { border-left-color: var(--warn); }
  .insight-critical { border-left-color: var(--danger); }
  .insight-info { border-left-color: var(--success); }
</style>
