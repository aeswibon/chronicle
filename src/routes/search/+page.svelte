<script>
  import { invoke } from '@tauri-apps/api/core';
  import PageShell from '$lib/components/PageShell.svelte';
  import { eventCategoryLabel, eventLabel, eventSubtitle, formatDateTime, groupByDate, highlightMatch, shouldShowCategoryBadge } from '$lib/format.js';
  import AppIcon from '$lib/components/AppIcon.svelte';

  let query = $state('');
  let results = $state([]);
  let searched = $state(false);
  let searching = $state(false);
  let semantic = $state(false);

  async function search() {
    const q = query.trim();
    if (!q) return;
    searching = true;
    searched = true;
    try {
      results = await invoke('search_events', { query: q, limit: 100, semantic });
    } catch {
      results = [];
    } finally {
      searching = false;
    }
  }

  let groupedResults = $derived(groupByDate(results));
</script>

<PageShell title="Search" description="Find events by app, project, or activity type across the last 30 days.">
  <div class="flex flex-wrap items-center gap-4 mb-8">
    <div class="flex gap-2 flex-1 min-w-[12rem]">
      <input
        type="text"
        bind:value={query}
        onkeydown={(e) => e.key === 'Enter' && search()}
        placeholder="Search events..."
        class="flex-1 bg-[var(--bg-elevated)] border border-[var(--border)] rounded-lg px-4 py-2.5 text-sm text-[var(--text)] placeholder-[var(--text-muted)] outline-none focus:border-[var(--accent)]/50 transition-colors"
      />
      <button
        type="button"
        onclick={search}
        disabled={!query.trim() || searching}
        class="px-4 py-2.5 text-sm font-medium rounded-lg bg-[var(--accent)] text-white dark:text-zinc-900 disabled:opacity-40 disabled:cursor-not-allowed transition-opacity"
      >
        {searching ? 'Searching…' : 'Search'}
      </button>
    </div>
    <label class="flex items-center gap-2 text-xs text-[var(--text-secondary)] cursor-pointer">
      <input type="checkbox" class="accent-[var(--accent)]" bind:checked={semantic} />
      Semantic search (experimental)
    </label>
  </div>

  {#if !searched}
    <div class="text-center py-20">
      <p class="text-sm text-[var(--text-secondary)]">Enter a query to search your activity history.</p>
    </div>
  {:else if searching}
    <p class="text-sm text-[var(--text-muted)]">Searching…</p>
  {:else if results.length === 0}
    <div class="text-center py-20 rounded-xl border border-dashed border-[var(--border)]">
      <p class="text-sm text-[var(--text-secondary)]">No results for "{query}"</p>
      <p class="text-xs text-[var(--text-muted)] mt-2">Try a different app name, project, or event type.</p>
    </div>
  {:else}
    <p class="text-xs text-[var(--text-muted)] mb-6">
      {results.length} result{results.length === 1 ? '' : 's'}
    </p>

    <div class="space-y-8">
      {#each groupedResults as [label, events]}
        <section>
          <h3 class="text-xs font-medium text-[var(--text-muted)] uppercase tracking-wider mb-3">{label}</h3>
          <div class="space-y-2">
            {#each events as event}
              <article class="bg-[var(--bg-elevated)] border border-[var(--border)] rounded-xl px-4 py-3.5 hover:border-[var(--accent)]/30 transition-colors">
                <div class="flex items-start gap-3">
                  <AppIcon event={event} size={32} />
                  <div class="min-w-0 flex-1">
                    <div class="flex items-start justify-between gap-3">
                      <div class="min-w-0">
                        <div class="flex items-center gap-2 flex-wrap">
                          <p class="text-sm font-medium text-[var(--text)]">
                            {@html highlightMatch(eventLabel(event), query)}
                          </p>
                          {#if shouldShowCategoryBadge(event)}
                            <span class="inline-flex px-1.5 py-0.5 rounded text-[10px] font-medium uppercase tracking-wide bg-[var(--bg-muted)] text-[var(--text-muted)]">
                              {eventCategoryLabel(event)}
                            </span>
                          {/if}
                        </div>
                        <p class="text-xs text-[var(--text-muted)] mt-1 truncate">{eventSubtitle(event)}</p>
                      </div>
                      <time class="text-xs text-[var(--text-muted)] shrink-0 tabular-nums">{formatDateTime(event.timestamp)}</time>
                    </div>
                    <div class="flex flex-wrap items-center gap-2 mt-2">
                      {#if event.type}
                        <span class="inline-flex items-center px-2 py-0.5 rounded-md text-[11px] font-medium bg-[var(--bg-muted)] text-[var(--text-secondary)]">
                          {@html highlightMatch(event.type, query)}
                        </span>
                      {/if}
                      {#if event.project}
                        <span class="inline-flex items-center px-2 py-0.5 rounded-md text-[11px] font-medium bg-[var(--accent-muted)] text-[var(--accent)]">
                          {@html highlightMatch(event.project, query)}
                        </span>
                      {/if}
                      {#if event.source && event.source !== eventLabel(event)}
                        <span class="text-[11px] text-[var(--text-muted)] truncate">
                          {@html highlightMatch(event.source, query)}
                        </span>
                      {/if}
                    </div>
                  </div>
                </div>
              </article>
            {/each}
          </div>
        </section>
      {/each}
    </div>
  {/if}
</PageShell>
