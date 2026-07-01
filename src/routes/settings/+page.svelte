<script>
  import { invoke } from '@tauri-apps/api/core';
  import { onMount } from 'svelte';
  import PageShell from '$lib/components/PageShell.svelte';
  import { theme, setTheme } from '$lib/theme.svelte.js';

  let status = $state(null);
  let watchDirs = $state('');
  let configMessage = $state('');
  let hookMessage = $state('');
  let saving = $state(false);
  let installingHook = $state(false);

  onMount(async () => {
    try {
      status = await invoke('get_status');
    } catch {}
    try {
      const cfg = await invoke('get_config');
      watchDirs = (cfg.watch_dirs ?? []).join('\n');
    } catch {}
  });

  async function saveWatchDirs() {
    saving = true;
    configMessage = '';
    try {
      const dirs = watchDirs
        .split('\n')
        .map((s) => s.trim())
        .filter(Boolean);
      await invoke('set_config', { watchDirs: dirs });
      configMessage = 'Saved. Restart the daemon for watch directories to take effect.';
    } catch (e) {
      configMessage = String(e);
    } finally {
      saving = false;
    }
  }

  async function installHook() {
    installingHook = true;
    hookMessage = '';
    try {
      await invoke('install_shell_hook', { shell: 'zsh' });
      hookMessage = 'Shell hook installed. Restart your terminal or run: source ~/.zshrc';
    } catch (e) {
      hookMessage = String(e);
    } finally {
      installingHook = false;
    }
  }

  const themeOptions = /** @type {const} */ ([
    { value: 'system', label: 'System' },
    { value: 'light', label: 'Light' },
    { value: 'dark', label: 'Dark' },
  ]);
</script>

<PageShell title="Settings" description="Capture paths, shell hook, and appearance.">
  <section class="mb-8">
    <h3 class="text-xs font-medium text-[var(--text-muted)] uppercase tracking-wider mb-4">Watch directories</h3>
    <p class="text-sm text-[var(--text-secondary)] leading-relaxed mb-3">
      Git and filesystem collectors scan these paths for repositories (one per line).
    </p>
    <textarea
      bind:value={watchDirs}
      rows="4"
      placeholder="/Volumes/Seagate/developer&#10;~/Developer"
      class="w-full text-xs font-mono bg-[var(--bg-muted)] border border-[var(--border)] rounded-lg px-4 py-3 text-[var(--text)] resize-y"
    ></textarea>
    <div class="flex items-center gap-3 mt-3">
      <button
        type="button"
        onclick={saveWatchDirs}
        disabled={saving}
        class="px-4 py-2 text-sm rounded-lg bg-[var(--accent)] text-white hover:opacity-90 transition-opacity disabled:opacity-50"
      >
        {saving ? 'Saving…' : 'Save watch dirs'}
      </button>
      {#if configMessage}
        <p class="text-xs text-[var(--text-muted)]">{configMessage}</p>
      {/if}
    </div>
  </section>

  <section class="mb-8">
    <h3 class="text-xs font-medium text-[var(--text-muted)] uppercase tracking-wider mb-4">Shell hook</h3>
    <p class="text-sm text-[var(--text-secondary)] leading-relaxed">
      Records terminal commands via UDP <code class="font-mono text-xs">127.0.0.1:9712</code>.
    </p>
    <button
      type="button"
      onclick={installHook}
      disabled={installingHook}
      class="mt-3 px-4 py-2 text-sm rounded-lg border border-[var(--border)] text-[var(--text-secondary)] hover:border-[var(--accent)]/40 transition-colors disabled:opacity-50"
    >
      {installingHook ? 'Installing…' : 'Install zsh hook'}
    </button>
    {#if hookMessage}
      <p class="text-xs text-[var(--text-muted)] mt-2">{hookMessage}</p>
    {/if}
  </section>

  <section class="mb-8">
    <h3 class="text-xs font-medium text-[var(--text-muted)] uppercase tracking-wider mb-4">Appearance</h3>
    <div class="flex gap-2">
      {#each themeOptions as opt}
        <button
          type="button"
          onclick={() => setTheme(opt.value)}
          class="px-4 py-2 text-sm rounded-lg border transition-colors"
          class:border-[var(--accent)]={theme.preference === opt.value}
          class:text-[var(--accent)]={theme.preference === opt.value}
          class:bg-[var(--accent-muted)]={theme.preference === opt.value}
          class:border-[var(--border)]={theme.preference !== opt.value}
          class:text-[var(--text-secondary)]={theme.preference !== opt.value}
          class:hover:border-[var(--text-muted)]={theme.preference !== opt.value}
        >
          {opt.label}
        </button>
      {/each}
    </div>
    <p class="text-xs text-[var(--text-muted)] mt-3">
      Currently {theme.isDark ? 'dark' : 'light'}{theme.preference === 'system' ? ' (following system)' : ''}.
    </p>
  </section>

  <section>
    <h3 class="text-xs font-medium text-[var(--text-muted)] uppercase tracking-wider mb-4">Daemon</h3>
    <div class="bg-[var(--bg-elevated)] border border-[var(--border)] rounded-xl divide-y divide-[var(--border-subtle)]">
      {#if status}
        <div class="flex items-center justify-between px-5 py-4">
          <span class="text-sm text-[var(--text-secondary)]">Version</span>
          <span class="text-sm text-[var(--text)] tabular-nums">{status.version}</span>
        </div>
        <div class="flex items-center justify-between px-5 py-4">
          <span class="text-sm text-[var(--text-secondary)]">Events recorded</span>
          <span class="text-sm text-[var(--text)] tabular-nums">{status.events_count}</span>
        </div>
        <div class="flex items-center justify-between px-5 py-4">
          <span class="text-sm text-[var(--text-secondary)]">Uptime</span>
          <span class="text-sm text-[var(--text)] tabular-nums">
            {Math.floor(status.uptime_secs / 3600)}h {Math.floor((status.uptime_secs % 3600) / 60)}m
          </span>
        </div>
        <div class="flex items-center justify-between px-5 py-4">
          <span class="text-sm text-[var(--text-secondary)]">Socket</span>
          <code class="text-xs text-[var(--text-muted)] font-mono">/tmp/chronicle.sock</code>
        </div>
      {:else}
        <div class="px-5 py-16 text-center">
          <p class="text-sm text-[var(--text-secondary)]">Daemon not connected.</p>
          <p class="text-xs text-[var(--text-muted)] mt-2">
            Start with <code class="text-[var(--accent)] font-mono">chronicle-daemon start</code>
          </p>
        </div>
      {/if}
    </div>
  </section>
</PageShell>
