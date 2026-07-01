<script>
  import { invoke } from '@tauri-apps/api/core';
  import { onMount } from 'svelte';
  import PageShell from '$lib/components/PageShell.svelte';
  import { theme, setTheme } from '$lib/theme.svelte.js';

  let status = $state(/** @type {{ version: string; events_count: number; uptime_secs: number } | null} */ (null));
  let watchDirs = $state('');
  let collectors = $state({
    window_focus: true,
    filesystem: true,
    git: true,
    shell: true,
  });
  let privacy = $state({
    allowed_domains: '',
    strip_query_params: true,
    retention_days: null,
    redact_shell_secrets: true,
  });
  let shellChoice = $state('zsh');
  let configMessage = $state('');
  let hookMessage = $state('');
  let saving = $state(false);
  let installingHook = $state(false);
  let restarting = $state(false);
  let restartMessage = $state('');

  onMount(async () => {
    try {
      status = await invoke('get_status');
    } catch {}
    try {
      const cfg = await invoke('get_config');
      watchDirs = (cfg.watch_dirs ?? []).join('\n');
      if (cfg.collectors) {
        collectors = { ...cfg.collectors };
      }
      if (cfg.privacy) {
        privacy = {
          allowed_domains: (cfg.privacy.allowed_domains ?? []).join('\n'),
          strip_query_params: cfg.privacy.strip_query_params ?? true,
          retention_days: cfg.privacy.retention_days ?? null,
          redact_shell_secrets: cfg.privacy.redact_shell_secrets ?? true,
        };
      }
    } catch {}
  });

  async function saveSettings() {
    saving = true;
    configMessage = '';
    try {
      const dirs = watchDirs
        .split('\n')
        .map((s) => s.trim())
        .filter(Boolean);
      await invoke('set_config', {
        watch_dirs: dirs,
        collectors: { ...collectors },
        privacy: {
          allowed_domains: privacy.allowed_domains
            .split('\n')
            .map((s) => s.trim())
            .filter(Boolean),
          strip_query_params: privacy.strip_query_params,
          retention_days: privacy.retention_days,
          redact_shell_secrets: privacy.redact_shell_secrets,
        },
      });
      configMessage = 'Saved. Restart the daemon for watch dirs and collector toggles to take effect.';
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
      await invoke('install_shell_hook', { shell: shellChoice });
      const rc =
        shellChoice === 'fish'
          ? '~/.config/fish/config.fish'
          : shellChoice === 'bash'
            ? '~/.bashrc'
            : '~/.zshrc';
      hookMessage = `Shell hook installed. Restart your terminal or run: source ${rc}`;
    } catch (e) {
      hookMessage = String(e);
    } finally {
      installingHook = false;
    }
  }

  async function restartDaemon() {
    restarting = true;
    restartMessage = '';
    try {
      await invoke('restart_daemon');
      restartMessage = 'Daemon restarted.';
      status = await invoke('get_status');
    } catch (e) {
      restartMessage = String(e);
    } finally {
      restarting = false;
    }
  }

  const themeOptions = /** @type {const} */ ([
    { value: 'system', label: 'System' },
    { value: 'light', label: 'Light' },
    { value: 'dark', label: 'Dark' },
  ]);

  const collectorRows = [
    { key: 'window_focus', label: 'Window focus', hint: 'Active app and window title (macOS)' },
    { key: 'filesystem', label: 'Filesystem', hint: 'Source file create/delete under watch dirs' },
    { key: 'git', label: 'Git', hint: 'Commits, checkouts, merges via reflog' },
    { key: 'shell', label: 'Shell hook', hint: 'Terminal commands via UDP (requires hook install)' },
  ];
</script>

<PageShell title="Settings" description="Collectors, capture paths, shell hook, and appearance.">
  <section class="mb-8">
    <h3 class="text-xs font-medium text-[var(--text-muted)] uppercase tracking-wider mb-4">Collectors</h3>
    <p class="text-sm text-[var(--text-secondary)] leading-relaxed mb-4">
      Each collector is opt-in. Disabled collectors are not started when the daemon restarts.
    </p>
    <div class="bg-[var(--bg-elevated)] border border-[var(--border)] rounded-xl divide-y divide-[var(--border-subtle)]">
      {#each collectorRows as row}
        <label class="flex items-start gap-4 px-5 py-4 cursor-pointer">
          <input
            type="checkbox"
            class="mt-1 accent-[var(--accent)]"
            checked={collectors[row.key]}
            onchange={() => {
              collectors[row.key] = !collectors[row.key];
            }}
          />
          <span>
            <span class="text-sm text-[var(--text)] block">{row.label}</span>
            <span class="text-xs text-[var(--text-muted)]">{row.hint}</span>
          </span>
        </label>
      {/each}
    </div>
  </section>

  <section class="mb-8">
    <h3 class="text-xs font-medium text-[var(--text-muted)] uppercase tracking-wider mb-4">Privacy</h3>
    <p class="text-sm text-[var(--text-secondary)] leading-relaxed mb-3">
      Browser allowlist (one domain per line; empty = record all). Retention prunes on daemon start.
    </p>
    <textarea
      bind:value={privacy.allowed_domains}
      rows="3"
      placeholder="github.com&#10;stackoverflow.com"
      class="w-full text-xs font-mono bg-[var(--bg-muted)] border border-[var(--border)] rounded-lg px-4 py-3 text-[var(--text)] resize-y mb-4"
    ></textarea>
    <div class="bg-[var(--bg-elevated)] border border-[var(--border)] rounded-xl divide-y divide-[var(--border-subtle)] mb-4">
      <label class="flex items-start gap-4 px-5 py-4 cursor-pointer">
        <input type="checkbox" class="mt-1 accent-[var(--accent)]" bind:checked={privacy.strip_query_params} />
        <span>
          <span class="text-sm text-[var(--text)] block">Strip URL query parameters</span>
          <span class="text-xs text-[var(--text-muted)]">Browser events omit ?query and #fragment</span>
        </span>
      </label>
      <label class="flex items-start gap-4 px-5 py-4 cursor-pointer">
        <input type="checkbox" class="mt-1 accent-[var(--accent)]" bind:checked={privacy.redact_shell_secrets} />
        <span>
          <span class="text-sm text-[var(--text)] block">Redact shell secrets</span>
          <span class="text-xs text-[var(--text-muted)]">Mask API_KEY, TOKEN, PASSWORD in command strings</span>
        </span>
      </label>
    </div>
    <label class="block text-sm text-[var(--text-secondary)] mb-2">
      Retention (days)
      <input
        type="number"
        min="1"
        placeholder="Keep forever"
        bind:value={privacy.retention_days}
        class="mt-1 w-32 text-sm bg-[var(--bg-muted)] border border-[var(--border)] rounded-lg px-3 py-2 text-[var(--text)]"
      />
    </label>
  </section>

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
        onclick={saveSettings}
        disabled={saving}
        class="px-4 py-2 text-sm rounded-lg bg-[var(--accent)] text-white hover:opacity-90 transition-opacity disabled:opacity-50"
      >
        {saving ? 'Saving…' : 'Save settings'}
      </button>
      {#if configMessage}
        <p class="text-xs text-[var(--text-muted)]">{configMessage}</p>
      {/if}
    </div>
  </section>

  <section class="mb-8">
    <h3 class="text-xs font-medium text-[var(--text-muted)] uppercase tracking-wider mb-4">Shell hook</h3>
    <p class="text-sm text-[var(--text-secondary)] leading-relaxed mb-3">
      Records terminal commands via UDP <code class="font-mono text-xs">127.0.0.1:9712</code>. Enable the shell collector above.
    </p>
    <div class="flex flex-wrap items-center gap-3">
      <select
        bind:value={shellChoice}
        class="text-sm bg-[var(--bg-muted)] border border-[var(--border)] rounded-lg px-3 py-2 text-[var(--text)]"
      >
        <option value="zsh">zsh</option>
        <option value="bash">bash</option>
        <option value="fish">fish</option>
      </select>
      <button
        type="button"
        onclick={installHook}
        disabled={installingHook}
        class="px-4 py-2 text-sm rounded-lg border border-[var(--border)] text-[var(--text-secondary)] hover:border-[var(--accent)]/40 transition-colors disabled:opacity-50"
      >
        {installingHook ? 'Installing…' : 'Install shell hook'}
      </button>
    </div>
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
        <div class="px-5 py-4 border-t border-[var(--border-subtle)]">
          <button
            type="button"
            onclick={restartDaemon}
            disabled={restarting}
            class="px-4 py-2 text-sm rounded-lg border border-[var(--border)] text-[var(--text-secondary)] hover:border-[var(--accent)]/40 transition-colors disabled:opacity-50"
          >
            {restarting ? 'Restarting…' : 'Restart daemon'}
          </button>
          {#if restartMessage}
            <p class="text-xs text-[var(--text-muted)] mt-2">{restartMessage}</p>
          {/if}
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
