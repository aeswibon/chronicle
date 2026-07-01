<script>
  import { invoke } from '@tauri-apps/api/core';
  import { onMount } from 'svelte';
  import PageShell from '$lib/components/PageShell.svelte';
  import ProjectIcon from '$lib/components/ProjectIcon.svelte';
  import { preloadPathIcons } from '$lib/appIcons.js';
  import { formatDateTime } from '$lib/format.js';

  let projects = $state([]);
  /** @type {Record<string, string>} */
  let iconUrls = $state({});
  let loading = $state(true);
  let error = $state('');

  onMount(async () => {
    try {
      projects = await invoke('list_projects', { limit: 50 });
    } catch (e) {
      error = String(e);
      projects = [];
    } finally {
      loading = false;
    }

    if (projects.length > 0) {
      preloadPathIcons(projects.map((p) => p.path))
        .then((urls) => {
          iconUrls = urls;
        })
        .catch(() => {});
    }
  });
</script>

<PageShell title="Projects" description="Git and cargo projects detected from your activity.">
  {#if loading}
    <p class="text-sm text-[var(--text-muted)]">Loading…</p>
  {:else if error}
    <div class="text-center py-20 rounded-xl border border-dashed border-[var(--border)]">
      <p class="text-sm text-[var(--text-secondary)]">{error}</p>
      {#if error.includes('connect') || error.includes('eof')}
        <p class="text-xs text-[var(--text-muted)] mt-3 max-w-md mx-auto">
          Reopen Chronicle or go to Settings → Start capturing to connect the background service.
        </p>
      {/if}
    </div>
  {:else if projects.length === 0}
    <div class="text-center py-20 rounded-xl border border-dashed border-[var(--border)]">
      <p class="text-sm text-[var(--text-secondary)]">No projects detected yet.</p>
      <p class="text-xs text-[var(--text-muted)] mt-2">Work in a git repo to register a project automatically.</p>
    </div>
  {:else}
    <p class="text-xs text-[var(--text-muted)] mb-6">{projects.length} project{projects.length === 1 ? '' : 's'}</p>

    <div class="grid gap-3">
      {#each projects as proj}
        <a
          href="/projects/{encodeURIComponent(proj.name)}"
          class="flex items-center justify-between bg-[var(--bg-elevated)] border border-[var(--border)] rounded-xl px-5 py-4 hover:border-[var(--accent)]/30 transition-colors"
        >
          <div class="flex items-center gap-4 min-w-0">
            <ProjectIcon name={proj.name} path={proj.path} size={36} iconUrl={iconUrls[proj.path] ?? null} />
            <div class="min-w-0">
              <p class="text-sm font-medium text-[var(--text)] truncate">{proj.name}</p>
              <p class="text-xs text-[var(--text-muted)] mt-0.5 truncate font-mono">{proj.path}</p>
            </div>
          </div>
          <div class="text-right shrink-0 ml-4">
            <p class="text-xs text-[var(--text-muted)] tabular-nums">{formatDateTime(proj.last_active)}</p>
            {#if proj.language}
              <p class="text-xs text-[var(--text-secondary)] mt-0.5">{proj.language}</p>
            {/if}
          </div>
        </a>
      {/each}
    </div>
  {/if}
</PageShell>
