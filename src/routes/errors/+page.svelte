<script>
  import { invoke } from '@tauri-apps/api/core';
  import { onMount } from 'svelte';
  import PageShell from '$lib/components/PageShell.svelte';
  import { eventLabel, eventSubtitle, formatDateTime } from '$lib/format.js';
  import AppIcon from '$lib/components/AppIcon.svelte';

  let errors = $state([]);
  let loading = $state(true);
  let loadError = $state('');

  async function load() {
    loading = true;
    loadError = '';
    try {
      const since = Date.now() - 7 * 86400000;
      errors = await invoke('get_errors', { since, limit: 100 });
    } catch (e) {
      loadError = String(e);
      errors = [];
    } finally {
      loading = false;
    }
  }

  onMount(load);
</script>

<PageShell title="Errors" description="Failed commands and non-zero exits from the last 7 days.">
  {#if loadError}
    <div class="text-center py-16 rounded-xl border border-dashed border-[var(--border)]">
      <p class="text-sm text-[var(--text-secondary)]">{loadError}</p>
    </div>
  {:else if loading}
    <p class="text-sm text-[var(--text-muted)]">Loading…</p>
  {:else if errors.length === 0}
    <div class="text-center py-20 rounded-xl border border-dashed border-[var(--border)]">
      <p class="text-sm text-[var(--text-secondary)]">No errors recorded.</p>
      <p class="text-xs text-[var(--text-muted)] mt-2">Shell commands with non-zero exit codes appear here.</p>
    </div>
  {:else}
    <p class="text-xs text-[var(--text-muted)] mb-6">{errors.length} error{errors.length === 1 ? '' : 's'}</p>
    <div class="space-y-2">
      {#each errors as event}
        <div class="flex items-start gap-3 bg-[var(--bg-elevated)] border border-[var(--border)] rounded-xl px-4 py-3.5">
          <AppIcon {event} size={32} />
          <div class="min-w-0 flex-1">
            <div class="flex items-start justify-between gap-3">
              <div class="min-w-0">
                <p class="text-sm font-medium text-[var(--text)] truncate">{eventLabel(event)}</p>
                <p class="text-xs text-[var(--text-muted)] mt-1 truncate">{eventSubtitle(event)}</p>
              </div>
              <time class="text-xs text-[var(--text-muted)] shrink-0 tabular-nums">{formatDateTime(event.timestamp)}</time>
            </div>
          </div>
        </div>
      {/each}
    </div>
  {/if}
</PageShell>
