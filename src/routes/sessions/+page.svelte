<script>
  import { invoke } from '@tauri-apps/api/core';
  import { onMount } from 'svelte';
  import PageShell from '$lib/components/PageShell.svelte';
  import { formatDateTime, formatDuration, isListableSummary } from '$lib/format.js';

  let sessions = $state([]);
  let loading = $state(true);
  let error = $state('');
  let notice = $state('');
  let generating = $state(false);
  let deletingId = $state(null);
  let summarySource = $state('');
  /** @type {{ summary: string; started_at: number; session_type: string; span_count: number; event_count: number; source?: string } | null} */
  let preview = $state(null);

  async function load() {
    loading = true;
    error = '';
    try {
      const since = Date.now() - 30 * 86400000;
      sessions = await invoke('get_sessions', { since, until: null });
      sessions = sessions.filter((s) => isListableSummary(s));
      sessions.sort((a, b) => b.started_at - a.started_at);
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
    summarySource = '';
    try {
      const result = await invoke('summarize_today');
      summarySource = result.source ?? 'rules';
      if (result.persisted) {
        preview = null;
        await load();
      } else {
        preview = {
          summary: result.summary,
          started_at: Date.now(),
          session_type: 'focus',
          span_count: 0,
          event_count: 0,
          source: summarySource,
        };
        notice = result.notice ?? 'Summary was not saved to the database.';
      }
    } catch (e) {
      error = String(e);
    } finally {
      generating = false;
    }
  }

  /** @param {{ id: string; started_at: number }} session */
  async function deleteSession(session) {
    if (!confirm(`Delete the summary for ${formatDateTime(session.started_at)}?`)) {
      return;
    }
    deletingId = session.id;
    error = '';
    try {
      await invoke('delete_session', { id: session.id });
      sessions = sessions.filter((s) => s.id !== session.id);
    } catch (e) {
      error = String(e);
    } finally {
      deletingId = null;
    }
  }

  onMount(load);
</script>

<PageShell title="Sessions" description="Daily rollups from your activity. Enable AI in Settings for richer summaries, or use rules mode by default.">
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
      <div class="flex items-center justify-between gap-3 mb-2">
        <p class="text-xs font-medium text-[var(--accent)] uppercase tracking-wider">
          Generated {formatDateTime(preview.started_at)}
        </p>
        {#if preview.source}
          <span
            class="text-[10px] uppercase tracking-wider px-2 py-0.5 rounded-full border"
            class:border-[var(--accent)]={preview.source === 'ai'}
            class:text-[var(--accent)]={preview.source === 'ai'}
            class:border-[var(--border)]={preview.source !== 'ai'}
            class:text-[var(--text-muted)]={preview.source !== 'ai'}
          >
            {preview.source === 'ai' ? 'AI' : 'Rules'}
          </span>
        {/if}
      </div>
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
              <p class="text-sm font-medium text-[var(--text)] capitalize">Daily summary</p>
              {#if session.project}
                <p class="text-xs text-[var(--text-muted)] mt-1">{session.project}</p>
              {/if}
            </div>
            <div class="text-right text-xs text-[var(--text-muted)] tabular-nums shrink-0">
              <p>{formatDateTime(session.started_at)}</p>
              {#if session.summary_source}
                <span
                  class="inline-block mt-1 text-[10px] uppercase tracking-wider px-2 py-0.5 rounded-full border"
                  class:border-[var(--accent)]={session.summary_source === 'ai'}
                  class:text-[var(--accent)]={session.summary_source === 'ai'}
                  class:border-[var(--border)]={session.summary_source !== 'ai'}
                >
                  {session.summary_source === 'ai' ? 'AI' : 'Rules'}
                </span>
              {/if}
              {#if session.duration_ms}
                <p class="mt-1">{formatDuration(session.duration_ms)}</p>
              {/if}
              <button
                type="button"
                onclick={() => deleteSession(session)}
                disabled={deletingId === session.id}
                class="mt-2 text-[11px] text-[var(--text-muted)] hover:text-red-400 transition-colors disabled:opacity-50"
              >
                {deletingId === session.id ? 'Deleting…' : 'Delete'}
              </button>
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
