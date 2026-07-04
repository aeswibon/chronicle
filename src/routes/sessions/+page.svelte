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
      const rollup = {
        summary: result.summary,
        started_at: Date.now(),
        session_type: 'focus',
        span_count: result.span_count ?? 0,
        event_count: result.event_count ?? 0,
        duration_ms: result.duration_ms ?? null,
        source: summarySource,
        summary_source: summarySource,
      };

      if (result.ai_error) {
        notice = result.ai_error;
      } else if (result.notice) {
        notice = result.notice;
      }

      if (result.persisted) {
        await load();
        if (sessions.length === 0 && rollup.summary?.trim()) {
          preview = rollup;
          if (!notice) {
            notice =
              'Summary was saved but is hidden until you restart the daemon with the latest build. Use Settings → Restart daemon after rebuilding.';
          }
        }
      } else {
        preview = rollup;
        notice = result.notice ?? notice ?? 'Summary was not saved to the database.';
      }
    } catch (e) {
      error = String(e);
    } finally {
      generating = false;
    }
  }

  /** @param {{ id?: string; session_id?: string; started_at: number }} session */
  function sessionIdOf(session) {
    const raw = session?.id ?? session?.session_id;
    return raw == null ? '' : String(raw).trim();
  }

  /** @param {{ id?: string; session_id?: string; started_at: number }} session */
  async function deleteSession(session) {
    const id = sessionIdOf(session);
    if (!id) {
      error = 'Cannot delete: missing session id. Refresh the list and try again.';
      return;
    }
    if (!confirm(`Delete the summary for ${formatDateTime(session.started_at)}?`)) {
      return;
    }
    deletingId = id;
    error = '';
    try {
      await invoke('delete_session', { id });
      sessions = sessions.filter((s) => sessionIdOf(s) !== id);
      if (preview && sessionIdOf(preview) === id) {
        preview = null;
      }
    } catch (e) {
      const msg = String(e);
      error = msg;
      if (/not found/i.test(msg)) {
        await load();
      }
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
    <p class="text-sm text-red-400 mb-4">{error}</p>
  {/if}

  {#if notice}
    <p class="text-xs text-[var(--text-muted)] mb-4 leading-relaxed">{notice}</p>
  {/if}

  {#if preview}
    <article class="relative bg-[var(--bg-elevated)] border border-[var(--accent)]/40 rounded-xl px-5 py-4 mb-6">
      {#if preview.source}
        <span
          class="absolute top-4 right-4 text-[10px] uppercase tracking-wider px-2 py-0.5 rounded-full border"
          class:border-[var(--accent)]={preview.source === 'ai'}
          class:text-[var(--accent)]={preview.source === 'ai'}
          class:border-[var(--border)]={preview.source !== 'ai'}
          class:text-[var(--text-muted)]={preview.source !== 'ai'}
        >
          {preview.source === 'ai' ? 'AI' : 'Rules'}
        </span>
      {/if}
      <p class="text-sm font-medium text-[var(--text)] pr-16">Daily summary</p>
      <p class="text-sm text-[var(--text-secondary)] leading-relaxed mt-3">{preview.summary}</p>
      <p class="text-[11px] text-[var(--text-muted)] mt-4 text-right tabular-nums">
        Generated {formatDateTime(preview.started_at)}
      </p>
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
        <article class="relative bg-[var(--bg-elevated)] border border-[var(--border)] rounded-xl px-5 py-4">
          <div class="flex items-start justify-between gap-3 pr-20">
            <p class="text-sm font-medium text-[var(--text)]">Daily summary</p>
            {#if session.summary_source}
              <span
                class="absolute top-4 right-4 text-[10px] uppercase tracking-wider px-2 py-0.5 rounded-full border"
                class:border-[var(--accent)]={session.summary_source === 'ai'}
                class:text-[var(--accent)]={session.summary_source === 'ai'}
                class:border-[var(--border)]={session.summary_source !== 'ai'}
                class:text-[var(--text-muted)]={session.summary_source !== 'ai'}
              >
                {session.summary_source === 'ai' ? 'AI' : 'Rules'}
              </span>
            {/if}
          </div>
          {#if session.project}
            <p class="text-xs text-[var(--text-muted)] mt-1">{session.project}</p>
          {/if}
          {#if session.summary}
            <p class="text-sm text-[var(--text-secondary)] leading-relaxed mt-3 pr-2">{session.summary}</p>
          {/if}
          <div class="flex items-end justify-between gap-4 mt-4 pt-3 border-t border-[var(--border-subtle)]">
            <p class="text-[11px] text-[var(--text-muted)]">
              {session.span_count} spans · {session.event_count} events
              {#if session.duration_ms}
                <span class="mx-1">·</span>
                <span title="Sum of focus span durations for this day">Focus {formatDuration(session.duration_ms)}</span>
              {/if}
            </p>
            <div class="flex items-center gap-3 shrink-0 text-right">
              <button
                type="button"
                onclick={() => deleteSession(session)}
                disabled={deletingId === sessionIdOf(session)}
                class="text-[11px] text-[var(--text-muted)] hover:text-red-400 transition-colors disabled:opacity-50"
                aria-label="Delete summary"
              >
                {deletingId === sessionIdOf(session) ? 'Deleting…' : 'Delete'}
              </button>
              <p class="text-[11px] text-[var(--text-muted)] tabular-nums">{formatDateTime(session.started_at)}</p>
            </div>
          </div>
        </article>
      {/each}
    </div>
  {/if}
</PageShell>
