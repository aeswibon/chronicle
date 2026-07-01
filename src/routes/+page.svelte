<script>
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import { onMount } from 'svelte';
  import PageShell from '$lib/components/PageShell.svelte';
  import {
    collapseTimelineEvents,
    isInterestingActivity,
    shouldShowCategoryBadge,
    eventCategoryLabel,
    eventLabel,
    eventSubtitle,
    formatTime,
    formatDuration,
    activityLabel,
    spanActivityLabels,
    isSpanActive,
    isListableSpan,
    spanTypeLabel,
    spanAppName,
  } from '$lib/format.js';
  import AppIcon from '$lib/components/AppIcon.svelte';

  let events = $state([]);
  let spans = $state([]);
  let status = $state(null);
  let error = $state('');

  async function loadSpans() {
    const since = Date.now() - 86400000;
    try {
      const [evts, spns, st] = await Promise.all([
        invoke('get_events', { since, until: null, limit: 50 }),
        invoke('get_timeline', { since, until: null, limit: 20 }),
        invoke('get_status'),
      ]);
      events = evts;
      spans = spns;
      status = st;
      error = '';
    } catch (e) {
      error = `Connection failed: ${e}`;
    }
  }

  let feed = $derived(
    collapseTimelineEvents(events.filter((e) => isInterestingActivity(e))),
  );
  let listableSpans = $derived(spans.filter((s) => isListableSpan(s)));

  onMount(() => {
    loadSpans();
    invoke('start_event_stream').catch(() => {});

    let unlisten = () => {};
    listen('chronicle-event', (e) => {
      const event = e.payload;
      events = [event, ...events.filter((x) => x.id !== event.id)].slice(0, 50);
      if (status) status = { ...status, events_count: status.events_count + 1 };
    }).then((fn) => {
      unlisten = fn;
    });

    const interval = setInterval(loadSpans, 5_000);
    return () => {
      unlisten();
      clearInterval(interval);
    };
  });
</script>

<PageShell
  title="Timeline"
  description={status ? `${status.events_count} events recorded · v${status.version}` : 'Your recent activity and sessions.'}
>
  {#if error}
    <div class="text-center py-20 rounded-xl border border-dashed border-[var(--border)]">
      <p class="text-sm text-[var(--text-secondary)] mb-1">{error}</p>
      <p class="text-xs text-[var(--text-muted)]">
        Make sure the daemon is running: <code class="text-[var(--accent)] font-mono">chronicle start</code>
      </p>
    </div>
  {:else}
    <section class="mb-10">
      <h3 class="text-xs font-medium text-[var(--text-muted)] uppercase tracking-wider mb-4">Sessions</h3>
      {#if listableSpans.length > 0}
        <div class="space-y-2">
          {#each listableSpans as span}
            <a
              href="/sessions/{span.id}"
              class="block bg-[var(--bg-elevated)] border border-[var(--border)] rounded-xl px-4 py-3.5 hover:border-[var(--accent)]/30 transition-colors"
            >
              <div class="flex items-center justify-between">
                <div class="flex items-center gap-2.5">
                  <span
                    class="w-1.5 h-1.5 rounded-full shrink-0"
                    class:bg-[var(--accent)]={isSpanActive(span)}
                    class:bg-[var(--text-muted)]={!isSpanActive(span)}
                  ></span>
                  <span class="text-sm font-medium text-[var(--text)]">{spanAppName(span) || spanTypeLabel(span.span_type)}</span>
                  {#each spanActivityLabels(span) as label}
                    <span class="inline-flex px-1.5 py-0.5 rounded text-[10px] font-medium bg-[var(--accent-muted)] text-[var(--accent)]">{label}</span>
                  {/each}
                  {#if span.duration_ms}
                    <span class="text-xs text-[var(--text-muted)]">{formatDuration(span.duration_ms)}</span>
                  {/if}
                </div>
                <span class="text-xs text-[var(--text-muted)] tabular-nums">{formatTime(span.started_at)}</span>
              </div>
              {#if span.project}
                <p class="text-xs text-[var(--text-muted)] mt-2 ml-4">{span.project}</p>
              {/if}
            </a>
          {/each}
        </div>
      {:else}
        <p class="text-sm text-[var(--text-muted)] py-4 px-4 rounded-xl border border-dashed border-[var(--border)]">
          No active sessions — focus an agent IDE, terminal, or editor to start one.
        </p>
      {/if}
    </section>

    <section>
      <h3 class="text-xs font-medium text-[var(--text-muted)] uppercase tracking-wider mb-4">Activity</h3>
      {#if feed.length === 0}
        <div class="text-center py-20 rounded-xl border border-dashed border-[var(--border)]">
          <p class="text-sm text-[var(--text-secondary)]">No activity recorded yet.</p>
          <p class="text-xs text-[var(--text-muted)] mt-2">Switch between apps to generate focus events.</p>
        </div>
      {:else}
        <div class="space-y-2">
          {#each feed as item}
            <div class="flex items-start gap-3 bg-[var(--bg-elevated)] border border-[var(--border)] rounded-xl px-4 py-3.5 hover:border-[var(--accent)]/30 transition-colors">
              <AppIcon event={item.event} size={32} />
              <div class="min-w-0 flex-1">
                <div class="flex items-start justify-between gap-3">
                  <div class="min-w-0">
                    <div class="flex items-center gap-2 flex-wrap">
                      <span class="text-sm font-medium text-[var(--text)] truncate">{eventLabel(item.event)}</span>
                      {#if shouldShowCategoryBadge(item.event)}
                        <span class="inline-flex px-1.5 py-0.5 rounded text-[10px] font-medium uppercase tracking-wide bg-[var(--bg-muted)] text-[var(--text-muted)]">
                          {eventCategoryLabel(item.event)}
                        </span>
                      {/if}
                      {#if activityLabel(item.event)}
                        <span class="inline-flex px-1.5 py-0.5 rounded text-[10px] font-medium bg-[var(--accent-muted)] text-[var(--accent)]">
                          {activityLabel(item.event)}
                        </span>
                      {/if}
                      {#if item.count > 1}
                        <span class="inline-flex px-1.5 py-0.5 rounded text-[10px] font-medium bg-[var(--accent-muted)] text-[var(--accent)]">
                          ×{item.count}
                        </span>
                      {/if}
                    </div>
                    <p class="text-xs text-[var(--text-muted)] mt-1 truncate">{eventSubtitle(item.event)}</p>
                  </div>
                  <span class="text-xs text-[var(--text-muted)] shrink-0 tabular-nums">{formatTime(item.latest)}</span>
                </div>
              </div>
            </div>
          {/each}
        </div>
      {/if}
    </section>
  {/if}
</PageShell>
