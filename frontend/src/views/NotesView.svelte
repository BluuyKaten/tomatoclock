<!--
  学习笔记视图：列表 + 新建/编辑
-->
<script lang="ts">
  import { onMount } from 'svelte';
  import { notesApi, type NoteView } from '../api/client';
  import { pushToast } from '../stores/index';

  let notes: NoteView[] = [];
  let total = 0;
  let page = 1;
  const pageSize = 20;

  let showEditor = false;
  let editing: NoteView | null = null;
  let title = '';
  let content = '';
  let tagsRaw = '';

  async function load() {
    try {
      const r = await notesApi.list({ page, page_size: pageSize });
      notes = r.items;
      total = r.total;
    } catch (e) {
      pushToast({ kind: 'error', message: String(e) });
    }
  }

  onMount(load);

  function newNote() {
    editing = null;
    title = '';
    content = '';
    tagsRaw = '';
    showEditor = true;
  }

  function edit(n: NoteView) {
    editing = n;
    title = n.title ?? '';
    content = n.content;
    tagsRaw = n.tags?.join(', ') ?? '';
    showEditor = true;
  }

  async function save() {
    try {
      const tags = tagsRaw.split(',').map((t) => t.trim()).filter(Boolean);
      if (editing) {
        await notesApi.update(editing.id, { title: title || undefined, content, tags });
        pushToast({ kind: 'success', message: '已更新' });
      } else {
        await notesApi.create({ title: title || undefined, content, tags });
        pushToast({ kind: 'success', message: '已创建' });
      }
      showEditor = false;
      load();
    } catch (e) {
      pushToast({ kind: 'error', message: String(e) });
    }
  }

  async function remove(n: NoteView) {
    if (!confirm('确定删除该笔记？')) return;
    try { await notesApi.delete(n.id); pushToast({ kind: 'success', message: '已删除' }); load(); }
    catch (e) { pushToast({ kind: 'error', message: String(e) }); }
  }

  function fmtTime(ms: number): string {
    return new Date(ms).toLocaleString('zh-CN');
  }
</script>

<div class="notes-view col gap-16">
  <div class="row">
    <h2 style="margin:0">学习笔记</h2>
    <div class="flex-1"></div>
    <button class="btn-primary" on:click={newNote}>+ 新建笔记</button>
  </div>

  {#if showEditor}
    <div class="card col gap-16">
      <input bind:value={title} placeholder="标题（可选）" />
      <textarea bind:value={content} rows="6" placeholder="支持 Markdown" />
      <input bind:value={tagsRaw} placeholder="标签，逗号分隔" />
      <div class="row">
        <button class="btn-primary" on:click={save}>保存</button>
        <button class="btn-ghost" on:click={() => (showEditor = false)}>取消</button>
      </div>
    </div>
  {/if}

  <div class="col gap-16">
    {#each notes as n}
      <div class="card note-item col" style="gap:8px">
        <div class="row">
          <strong class="flex-1">{n.title ?? '（无标题）'}</strong>
          <span class="muted">{fmtTime(n.created_at)}</span>
        </div>
        <p class="muted" style="margin:0">{n.content.slice(0, 120)}{n.content.length > 120 ? '…' : ''}</p>
        {#if n.tags && n.tags.length}
          <div class="row wrap">
            {#each n.tags as t}
              <span class="tag">{t}</span>
            {/each}
          </div>
        {/if}
        <div class="row">
          <button class="btn-ghost" on:click={() => edit(n)}>编辑</button>
          <button class="btn-ghost" on:click={() => remove(n)}>删除</button>
        </div>
      </div>
    {:else}
      <p class="muted">还没有笔记</p>
    {/each}
  </div>

  <p class="muted">共 {total} 条</p>
</div>

<style>
  .tag {
    background: #fff0f0;
    color: var(--primary);
    padding: 2px 8px;
    border-radius: 12px;
    font-size: 12px;
  }
</style>
