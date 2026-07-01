<script>
  import { invoke } from '@tauri-apps/api/core';
  import { onMount } from 'svelte';
  import PageShell from '$lib/components/PageShell.svelte';
  import { formatDateTime, formatDuration } from '$lib/format.js';

  let sessions = $state([]);
  let loading = $state(true);
  let error = $state('');
  let notice = $state('');
  let generating = $state(false);
  /** @type {{ summary: string; started_at: number; session_type: string; span_count: number; event_count: number } | null} */
  let preview = $state(null);

  async function load() {
    loading = true;
    error = '';
    try {
      const since = Date.now() - 30 * 86400000;
      sessions = await invoke('get_sessions', { since, until: null });
    } catch (e) {
      error = String(e);
      sessions = [];
    } finally {
      loading = false;
    }
  }

  async function generateToday() {
    generating = true;
    error = '';
    notice = '';
    preview = null;
    try {
      const since = new Date();
      since.setHours(0, 0, 0, 0);
      const result = await invoke('summarize_day', { since: since.getTime(), until: null });
      if (result.persisted) {
        preview = null;
        await load();
      } else {
        preview = {
          summary: result.summary,
          started_at: since.getTime(),
          session_type: 'focus',
          span_count: 0,
          event_count: 0,
        };
        notice = result.notice ?? 'Summary was not saved to the database.';
      }
    } catch (e) {
      error = String(e);
    } finally {
      generating = false;
    }
  }

  onMount(load);
</script>

<PageShell title="Sessions" description="AI rollups persisted from daily activity summaries.">
  <div class="flex items-center gap-3 mb-6">
    <button
      type="button"
      onclick={generateToday}
      disabled={generating}
      class="px-4 py-2 text-sm rounded-lg bg-[var(--accent)] text-white hover:opacity-90 disabled:opacity-50"
    >
      {generating ? 'Summarizing…' : 'Summarize today'}
    </button>
  </div>

  {#if error}
    <p class="text-sm text-[var(--text-secondary)] mb-4">{error}</p>
  {/if}

  {#if notice}
    <p class="text-xs text-[var(--text-muted)] mb-4 leading-relaxed">{notice}</p>
  {/if}

  {#if preview}
    <article class="bg-[var(--bg-elevated)] border border-[var(--accent)]/40 rounded-xl px-5 py-4 mb-6">
      <p class="text-xs font-medium text-[var(--accent)] uppercase tracking-wider mb-2">Today's summary</p>
      <p class="text-sm text-[var(--text-secondary)] leading-relaxed">{preview.summary}</p>
    </article>
  {/if}

  {#if loading}
    <p class="text-sm text-[var(--text-muted)]">Loading…</p>
  {:else if sessions.length === 0 && !preview}
    <div class="text-center py-20 rounded-xl border border-dashed border-[var(--border)]">
      <p class="text-sm text-[var(--text-secondary)]">No session rollups yet.</p>
      <p class="text-xs text-[var(--text-muted)] mt-2">Generate a summary from today's spans and events.</p>
    </div>
  {:else if sessions.length > 0}
    <div class="space-y-3">
      {#each sessions as session}
        <article class="bg-[var(--bg-elevated)] border border-[var(--border)] rounded-xl px-5 py-4">
          <div class="flex items-start justify-between gap-4 mb-2">
            <div>
              <p class="text-sm font-medium text-[var(--text)] capitalize">{session.session_type}</p>
              {#if session.project}
                <p class="text-xs text-[var(--text-muted)] mt-1">{session.project}</p>
              {/if}
            </div>
            <div class="text-right text-xs text-[var(--text-muted)] tabular-nums">
              <p>{formatDateTime(session.started_at)}</p>
              {#if session.duration_ms}
                <p class="mt-1">{formatDuration(session.duration_ms)}</p>
              {/if}
            </div>
          </div>
          {#if session.summary}
            <p class="text-sm text-[var(--text-secondary)] leading-relaxed">{session.summary}</p>
          {/if}
          <p class="text-[11px] text-[var(--text-muted)] mt-3">
            {session.span_count} spans · {session.event_count} events
          </p>
        </article>
      {/each}
    </div>
  {/if}
</PageShell>
