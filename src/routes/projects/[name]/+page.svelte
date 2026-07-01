<script>
  import { invoke } from '@tauri-apps/api/core';
  import { onMount } from 'svelte';
  import { page } from '$app/stores';
  import PageShell from '$lib/components/PageShell.svelte';
  import ProjectIcon from '$lib/components/ProjectIcon.svelte';
  import AppIcon from '$lib/components/AppIcon.svelte';
  import { preloadPathIcons } from '$lib/appIcons.js';
  import {
    collapseTimelineEvents,
    eventCategoryLabel,
    eventLabel,
    eventSubtitle,
    formatDateTime,
    formatDuration,
    formatTime,
  } from '$lib/format.js';

  let projectName = $derived(decodeURIComponent($page.params.name ?? ''));
  let project = $state(null);
  let spans = $state([]);
  let events = $state([]);
  let loading = $state(true);
  let error = $state('');
  /** @type {string | null} */
  let iconUrl = $state(null);

  async function load() {
    loading = true;
    error = '';
    try {
      const since = Date.now() - 7 * 86400000;
      const ctx = await invoke('get_project_context', {
        project: projectName,
        since,
        limit: 50,
      });
      project = ctx.project;
      spans = ctx.spans;
      events = ctx.events;
      if (project?.path) {
        preloadPathIcons([project.path])
          .then((urls) => {
            iconUrl = urls[project.path] ?? null;
          })
          .catch(() => {});
      }
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  let feed = $derived(collapseTimelineEvents(events));

  onMount(() => {
    load();
  });
</script>

<PageShell
  title={projectName}
  description={project?.path ?? 'Project activity and sessions.'}
>
  <div class="mb-6">
    <a href="/projects" class="text-xs text-[var(--text-muted)] hover:text-[var(--accent)] transition-colors">
      ← Projects
    </a>
  </div>

  {#if loading}
    <p class="text-sm text-[var(--text-muted)]">Loading…</p>
  {:else if error}
    <div class="text-center py-16 rounded-xl border border-dashed border-[var(--border)]">
      <p class="text-sm text-[var(--text-secondary)]">{error}</p>
    </div>
  {:else}
    {#if project}
      <div class="flex items-center gap-4 mb-8 pb-6 border-b border-[var(--border-subtle)]">
        <ProjectIcon name={project.name} path={project.path} size={44} {iconUrl} />
        <div class="min-w-0">
          <p class="text-sm font-medium text-[var(--text)]">{project.name}</p>
          <p class="text-xs text-[var(--text-muted)] font-mono truncate mt-0.5">{project.path}</p>
          <p class="text-xs text-[var(--text-muted)] mt-1">Last active {formatDateTime(project.last_active)}</p>
        </div>
      </div>
    {/if}

    {#if spans.length > 0}
      <section class="mb-10">
        <h3 class="text-xs font-medium text-[var(--text-muted)] uppercase tracking-wider mb-4">Sessions</h3>
        <div class="space-y-2">
          {#each spans as span}
            <a
              href="/sessions/{span.id}"
              class="block bg-[var(--bg-elevated)] border border-[var(--border)] rounded-xl px-4 py-3.5 hover:border-[var(--accent)]/30 transition-colors"
            >
              <div class="flex items-center justify-between">
                <div class="flex items-center gap-2.5">
                  <span class="w-1.5 h-1.5 rounded-full bg-[var(--accent)] shrink-0"></span>
                  <span class="text-sm font-medium text-[var(--text)] capitalize">{span.span_type}</span>
                  <span class="text-xs text-[var(--text-muted)]">{formatDuration(span.duration_ms)}</span>
                </div>
                <span class="text-xs text-[var(--text-muted)] tabular-nums">{formatTime(span.started_at)}</span>
              </div>
              <p class="text-xs text-[var(--text-muted)] mt-2 ml-4">{span.event_count} events</p>
            </a>
          {/each}
        </div>
      </section>
    {/if}

    <section>
      <h3 class="text-xs font-medium text-[var(--text-muted)] uppercase tracking-wider mb-4">Activity</h3>
      {#if feed.length === 0}
        <p class="text-sm text-[var(--text-muted)]">No activity for this project yet.</p>
      {:else}
        <div class="space-y-2">
          {#each feed as item}
            <div class="flex items-start gap-3 bg-[var(--bg-elevated)] border border-[var(--border)] rounded-xl px-4 py-3.5">
              <AppIcon event={item.event} size={32} />
              <div class="min-w-0 flex-1">
                <div class="flex items-start justify-between gap-3">
                  <div class="min-w-0">
                    <div class="flex items-center gap-2 flex-wrap">
                      <span class="text-sm font-medium text-[var(--text)] truncate">{eventLabel(item.event)}</span>
                      <span class="inline-flex px-1.5 py-0.5 rounded text-[10px] font-medium uppercase tracking-wide bg-[var(--bg-muted)] text-[var(--text-muted)]">
                        {eventCategoryLabel(item.event)}
                      </span>
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
