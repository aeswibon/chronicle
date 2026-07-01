<script>
  import { invoke } from '@tauri-apps/api/core';
  import { onMount } from 'svelte';
  import { page } from '$app/stores';
  import PageShell from '$lib/components/PageShell.svelte';
  import AppIcon from '$lib/components/AppIcon.svelte';
  import {
    eventCategoryLabel,
    eventLabel,
    eventSubtitle,
    shouldShowCategoryBadge,
    formatDateTime,
    formatDuration,
    formatTime,
  } from '$lib/format.js';

  let spanId = $derived($page.params.id ?? '');
  let span = $state(null);
  let events = $state([]);
  let loading = $state(true);
  let error = $state('');

  onMount(async () => {
    try {
      const detail = await invoke('get_span_detail', { id: spanId, eventLimit: 100 });
      span = detail.span;
      events = detail.events;
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  });
</script>

<PageShell
  title={span ? `${span.span_type} session` : 'Session'}
  description={span ? formatDateTime(span.started_at) : 'Session detail'}
>
  <div class="mb-6">
    <a href="/" class="text-xs text-[var(--text-muted)] hover:text-[var(--accent)] transition-colors">
      ← Timeline
    </a>
    {#if span?.project}
      <span class="text-[var(--text-muted)] mx-2">·</span>
      <a
        href="/projects/{encodeURIComponent(span.project)}"
        class="text-xs text-[var(--text-muted)] hover:text-[var(--accent)] transition-colors"
      >
        {span.project}
      </a>
    {/if}
  </div>

  {#if loading}
    <p class="text-sm text-[var(--text-muted)]">Loading…</p>
  {:else if error}
    <div class="text-center py-16 rounded-xl border border-dashed border-[var(--border)]">
      <p class="text-sm text-[var(--text-secondary)]">{error}</p>
    </div>
  {:else if span}
    <div class="bg-[var(--bg-elevated)] border border-[var(--border)] rounded-xl px-5 py-4 mb-8">
      <div class="flex flex-wrap items-center gap-4 text-sm">
        <span class="font-medium text-[var(--text)] capitalize">{span.span_type}</span>
        <span class="text-[var(--text-muted)]">{formatDuration(span.duration_ms)}</span>
        <span class="text-[var(--text-muted)]">{span.event_count} events</span>
        {#if span.project}
          <span class="text-[var(--text-muted)]">· {span.project}</span>
        {/if}
      </div>
      <p class="text-xs text-[var(--text-muted)] mt-2">
        {formatDateTime(span.started_at)}
        {#if span.ended_at}
          → {formatTime(span.ended_at)}
        {/if}
      </p>
    </div>

    <section>
      <h3 class="text-xs font-medium text-[var(--text-muted)] uppercase tracking-wider mb-4">Events</h3>
      {#if events.length === 0}
        <p class="text-sm text-[var(--text-muted)]">No events in this session window.</p>
      {:else}
        <div class="space-y-2">
          {#each events as event}
            <div class="flex items-start gap-3 bg-[var(--bg-elevated)] border border-[var(--border)] rounded-xl px-4 py-3.5">
              <AppIcon {event} size={32} />
              <div class="min-w-0 flex-1">
                <div class="flex items-start justify-between gap-3">
                  <div class="min-w-0">
                    <div class="flex items-center gap-2 flex-wrap">
                      <span class="text-sm font-medium text-[var(--text)] truncate">{eventLabel(event)}</span>
                      {#if shouldShowCategoryBadge(event)}
                        <span class="inline-flex px-1.5 py-0.5 rounded text-[10px] font-medium uppercase tracking-wide bg-[var(--bg-muted)] text-[var(--text-muted)]">
                          {eventCategoryLabel(event)}
                        </span>
                      {/if}
                    </div>
                    <p class="text-xs text-[var(--text-muted)] mt-1 truncate">{eventSubtitle(event)}</p>
                  </div>
                  <span class="text-xs text-[var(--text-muted)] shrink-0 tabular-nums">{formatTime(event.timestamp)}</span>
                </div>
              </div>
            </div>
          {/each}
        </div>
      {/if}
    </section>
  {/if}
</PageShell>
