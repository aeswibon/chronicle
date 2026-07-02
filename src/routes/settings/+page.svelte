<script>
  import { invoke } from '@tauri-apps/api/core';
  import { onMount } from 'svelte';
  import PageShell from '$lib/components/PageShell.svelte';
  import { theme, setTheme } from '$lib/theme.svelte.js';

  let status = $state(/** @type {{ version: string; events_count: number; uptime_secs: number; macos_capture?: { monitor_running: boolean; frontmost_app?: string; title_source?: string; accessibility_trusted: boolean; screen_capture_granted: boolean; can_read_window_titles: boolean } } | null} */ (null));
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
  let ai = $state({
    enabled: false,
    base_url: 'http://127.0.0.1:11434',
    model: 'smallthinker',
    api_key_env: '',
    timeout_secs: 60,
  });
  let ollamaModels = $state(/** @type {string[]} */ ([]));
  let aiTestMessage = $state('');
  let testingAi = $state(false);
  let loadingModels = $state(false);
  let summaries = $state({
    auto_daily: true,
    auto_daily_hour_local: 21,
  });
  let shellChoice = $state('zsh');
  let configMessage = $state('');
  let hookMessage = $state('');
  let pruneMessage = $state('');
  let purgingTimeline = $state(false);
  let pruning = $state(false);
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
      const dirs = cfg.watchDirs ?? cfg.watch_dirs ?? [];
      watchDirs = dirs.join('\n');
      const c = cfg.collectors;
      if (c) {
        collectors = {
          window_focus: c.windowFocus ?? c.window_focus ?? true,
          filesystem: c.filesystem ?? true,
          git: c.git ?? true,
          shell: c.shell ?? true,
        };
      }
      const p = cfg.privacy;
      if (p) {
        const domains = p.allowedDomains ?? p.allowed_domains ?? [];
        privacy = {
          allowed_domains: domains.join('\n'),
          strip_query_params: p.stripQueryParams ?? p.strip_query_params ?? true,
          retention_days: p.retentionDays ?? p.retention_days ?? null,
          redact_shell_secrets: p.redactShellSecrets ?? p.redact_shell_secrets ?? true,
        };
      }
      const a = cfg.ai;
      if (a) {
        ai = {
          enabled: a.enabled ?? false,
          base_url: a.baseUrl ?? a.base_url ?? 'http://127.0.0.1:11434',
          model: a.model ?? 'smallthinker',
          api_key_env: a.apiKeyEnv ?? a.api_key_env ?? '',
          timeout_secs: a.timeoutSecs ?? a.timeout_secs ?? 60,
        };
      }
      const s = cfg.summaries;
      if (s) {
        summaries = {
          auto_daily: s.autoDaily ?? s.auto_daily ?? true,
          auto_daily_hour_local: s.autoDailyHourLocal ?? s.auto_daily_hour_local ?? 21,
        };
      }
    } catch {}
    await refreshOllamaModels();
  });

  function buildAiPayload() {
    return {
      enabled: ai.enabled,
      baseUrl: ai.base_url.trim(),
      model: ai.model.trim(),
      apiKeyEnv: ai.api_key_env.trim() || null,
      timeoutSecs: ai.timeout_secs,
    };
  }

  async function refreshOllamaModels() {
    loadingModels = true;
    try {
      ollamaModels = await invoke('list_ollama_models', { ai: buildAiPayload() });
    } catch {
      ollamaModels = [];
    } finally {
      loadingModels = false;
    }
  }

  async function testAiConnection() {
    testingAi = true;
    aiTestMessage = '';
    try {
      aiTestMessage = await invoke('test_ai_connection', { ai: buildAiPayload() });
    } catch (e) {
      aiTestMessage = String(e);
    } finally {
      testingAi = false;
    }
  }

  function buildConfigPayload() {
    const dirs = watchDirs
      .split('\n')
      .map((line) => line.trim())
      .filter(Boolean);
    return {
      watchDirs: dirs,
      collectors: {
        windowFocus: collectors.window_focus,
        filesystem: collectors.filesystem,
        git: collectors.git,
        shell: collectors.shell,
      },
      privacy: {
        allowedDomains: privacy.allowed_domains
          .split('\n')
          .map((line) => line.trim())
          .filter(Boolean),
        stripQueryParams: privacy.strip_query_params,
        retentionDays: privacy.retention_days,
        redactShellSecrets: privacy.redact_shell_secrets,
      },
      ai: {
        enabled: ai.enabled,
        baseUrl: ai.base_url.trim(),
        model: ai.model.trim(),
        apiKeyEnv: ai.api_key_env.trim() || null,
        timeoutSecs: ai.timeout_secs,
      },
      summaries: {
        autoDaily: summaries.auto_daily,
        autoDailyHourLocal: summaries.auto_daily_hour_local,
      },
    };
  }

  async function saveSettings() {
    saving = true;
    configMessage = '';
    try {
      await invoke('set_config', { config: buildConfigPayload() });
      configMessage =
        'Settings saved. Restart the daemon for watch directories and collector toggles to take effect.';
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

  async function purgeTimeline() {
    if (!confirm('Delete all events, spans, and session rollups? This cannot be undone.')) {
      return;
    }
    purgingTimeline = true;
    pruneMessage = '';
    try {
      const result = await invoke('purge_capture_timeline');
      pruneMessage = `Reset complete: removed ${result.events_deleted} events, ${result.spans_deleted} spans, and ${result.sessions_deleted} sessions.`;
    } catch (e) {
      pruneMessage = String(e);
    } finally {
      purgingTimeline = false;
    }
  }

  async function pruneNoise() {
    pruning = true;
    pruneMessage = '';
    try {
      const result = await invoke('prune_noise_events');
      pruneMessage = `Removed ${result.events_deleted} low-signal events. Restart the daemon if counts look stale.`;
      status = await invoke('get_status');
    } catch (e) {
      pruneMessage = String(e);
    } finally {
      pruning = false;
    }
  }

  let captureBusy = $state(false);
  let captureMessage = $state('');

  async function requestMacosAccessibility() {
    captureBusy = true;
    captureMessage = '';
    try {
      const cap = await invoke('request_macos_accessibility');
      if (status) status = { ...status, macos_capture: cap };
      captureMessage = cap.accessibility_trusted
        ? 'Accessibility granted.'
        : 'System prompt shown — enable Chronicle focus monitor in Privacy & Security → Accessibility.';
    } catch (e) {
      captureMessage = String(e);
    } finally {
      captureBusy = false;
    }
  }

  async function openMacosPrivacy(section) {
    try {
      await invoke('open_macos_privacy_settings', { section });
    } catch (e) {
      captureMessage = String(e);
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


  const shellOptions = /** @type {const} */ ([
    { value: 'zsh', label: 'Zsh' },
    { value: 'bash', label: 'Bash' },
    { value: 'fish', label: 'Fish' },
  ]);

  const themeOptions = /** @type {const} */ ([
    { value: 'system', label: 'System' },
    { value: 'light', label: 'Light' },
    { value: 'dark', label: 'Dark' },
  ]);

  const collectorRows = [
    { key: 'window_focus', label: 'Window focus', hint: 'Frontmost app via NSWorkspace; window titles need Accessibility (or Screen Recording fallback)' },
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


    {#if status?.macos_capture}
      <section class="mb-8">
        <h3 class="text-xs font-medium text-[var(--text-muted)] uppercase tracking-wider mb-4">macOS capture</h3>
        <p class="text-sm text-[var(--text-secondary)] leading-relaxed mb-4">
          Window focus uses Apple's NSWorkspace API. Window titles use the Accessibility API when permitted; otherwise Chronicle falls back to Screen Recording metadata.
        </p>
        <div class="bg-[var(--bg-elevated)] border border-[var(--border)] rounded-xl divide-y divide-[var(--border-subtle)]">
          <div class="flex items-center justify-between px-5 py-4">
            <span class="text-sm text-[var(--text-secondary)]">Focus monitor</span>
            <span class="text-sm text-[var(--text)]">{status.macos_capture.monitor_running ? 'Running' : 'Stopped'}</span>
          </div>
          <div class="flex items-center justify-between px-5 py-4">
            <span class="text-sm text-[var(--text-secondary)]">Accessibility</span>
            <span class="text-sm text-[var(--text)]">{status.macos_capture.accessibility_trusted ? 'Granted' : 'Needed for window titles'}</span>
          </div>
          <div class="flex items-center justify-between px-5 py-4">
            <span class="text-sm text-[var(--text-secondary)]">Screen Recording</span>
            <span class="text-sm text-[var(--text)]">{status.macos_capture.screen_capture_granted ? 'Granted' : 'Optional fallback'}</span>
          </div>
          {#if status.macos_capture.frontmost_app}
            <div class="flex items-center justify-between px-5 py-4">
              <span class="text-sm text-[var(--text-secondary)]">Frontmost</span>
              <span class="text-sm text-[var(--text)]">{status.macos_capture.frontmost_app}</span>
            </div>
          {/if}
          <div class="px-5 py-4 flex flex-wrap gap-2">
            <button
              type="button"
              onclick={requestMacosAccessibility}
              disabled={captureBusy}
              class="px-4 py-2 text-sm rounded-lg border border-[var(--border)] text-[var(--text-secondary)] hover:border-[var(--accent)]/40 transition-colors disabled:opacity-50"
            >
              {captureBusy ? 'Requesting…' : 'Request Accessibility'}
            </button>
            <button
              type="button"
              onclick={() => openMacosPrivacy('accessibility')}
              class="px-4 py-2 text-sm rounded-lg border border-[var(--border)] text-[var(--text-secondary)] hover:border-[var(--accent)]/40 transition-colors"
            >
              Open Accessibility Settings
            </button>
            <button
              type="button"
              onclick={() => openMacosPrivacy('screen')}
              class="px-4 py-2 text-sm rounded-lg border border-[var(--border)] text-[var(--text-secondary)] hover:border-[var(--accent)]/40 transition-colors"
            >
              Open Screen Recording Settings
            </button>
          </div>
          {#if captureMessage}
            <p class="text-xs text-[var(--text-muted)] px-5 pb-4">{captureMessage}</p>
          {/if}
        </div>
      </section>
    {/if}

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
    <h3 class="text-xs font-medium text-[var(--text-muted)] uppercase tracking-wider mb-4">Daily rollups</h3>
    <p class="text-sm text-[var(--text-secondary)] leading-relaxed mb-4">
      The daemon can auto-generate today's summary after the hour you choose (local time). Manual "Summarize today" on Sessions still works anytime.
    </p>
    <div class="bg-[var(--bg-elevated)] border border-[var(--border)] rounded-xl divide-y divide-[var(--border-subtle)] mb-4">
      <label class="flex items-start gap-4 px-5 py-4 cursor-pointer">
        <input type="checkbox" class="mt-1 accent-[var(--accent)]" bind:checked={summaries.auto_daily} />
        <span>
          <span class="text-sm text-[var(--text)] block">Auto-summarize each day</span>
          <span class="text-xs text-[var(--text-muted)]">Requires the daemon to be running near the scheduled hour</span>
        </span>
      </label>
    </div>
    <label class="block text-sm text-[var(--text-secondary)] mb-4">
      Auto-summarize after (local hour, 0–23)
      <input
        type="number"
        min="0"
        max="23"
        bind:value={summaries.auto_daily_hour_local}
        class="mt-1 w-24 text-sm bg-[var(--bg-muted)] border border-[var(--border)] rounded-lg px-3 py-2 text-[var(--text)]"
      />
    </label>
  </section>

  <section class="mb-8">
    <h3 class="text-xs font-medium text-[var(--text-muted)] uppercase tracking-wider mb-4">Timeline cleanup</h3>
    <p class="text-sm text-[var(--text-secondary)] leading-relaxed mb-3">
      Remove low-signal git noise (fetch/pull/checkout) and terminal junk already stored. Use <strong class="font-medium text-[var(--text)]">Reset timeline</strong> for a full wipe before re-capturing with a fixed daemon build.
    </p>
    <div class="flex flex-wrap gap-2">
      <button
        type="button"
        onclick={pruneNoise}
        disabled={pruning || purgingTimeline}
        class="px-4 py-2 text-sm rounded-lg border border-[var(--border)] text-[var(--text-secondary)] hover:border-[var(--accent)]/40 transition-colors disabled:opacity-50"
      >
        {pruning ? 'Pruning…' : 'Remove low-signal events'}
      </button>
      <button
        type="button"
        onclick={purgeTimeline}
        disabled={purgingTimeline || pruning}
        class="px-4 py-2 text-sm rounded-lg border border-red-500/40 text-red-600 dark:text-red-400 hover:border-red-500/70 transition-colors disabled:opacity-50"
      >
        {purgingTimeline ? 'Resetting…' : 'Reset timeline'}
      </button>
    </div>
    {#if pruneMessage}
      <p class="text-xs text-[var(--text-muted)] mt-2">{pruneMessage}</p>
    {/if}
  </section>

  <section class="mb-8">
    <h3 class="text-xs font-medium text-[var(--text-muted)] uppercase tracking-wider mb-4">AI summaries</h3>
    <p class="text-sm text-[var(--text-secondary)] leading-relaxed mb-4">
      {#if ai.enabled}
        Daily rollups use your OpenAI-compatible endpoint (Ollama at
        <code class="font-mono text-xs">127.0.0.1:11434</code> by default). Falls back to rules if the model is unreachable.
      {:else}
        <strong class="font-medium text-[var(--text)]">Rules mode is active.</strong> Summaries are deterministic text from filtered activity — enable AI below for richer daily reports.
      {/if}
    </p>
    <div class="bg-[var(--bg-elevated)] border border-[var(--border)] rounded-xl divide-y divide-[var(--border-subtle)] mb-4">
      <label class="flex items-start gap-4 px-5 py-4 cursor-pointer">
        <input type="checkbox" class="mt-1 accent-[var(--accent)]" bind:checked={ai.enabled} />
        <span>
          <span class="text-sm text-[var(--text)] block">Enable AI summaries</span>
          <span class="text-xs text-[var(--text-muted)]">Uses enriched event metadata for richer daily reports</span>
        </span>
      </label>
    </div>
    <div class="grid gap-4 sm:grid-cols-2 mb-4">
      <label class="block text-sm text-[var(--text-secondary)]">
        Base URL
        <input
          type="url"
          bind:value={ai.base_url}
          placeholder="http://127.0.0.1:11434"
          class="mt-1 w-full text-sm font-mono bg-[var(--bg-muted)] border border-[var(--border)] rounded-lg px-3 py-2 text-[var(--text)]"
        />
      </label>
      <label class="block text-sm text-[var(--text-secondary)]">
        Model
        <input
          type="text"
          bind:value={ai.model}
          list="ollama-model-options"
          placeholder="smallthinker"
          class="mt-1 w-full text-sm font-mono bg-[var(--bg-muted)] border border-[var(--border)] rounded-lg px-3 py-2 text-[var(--text)]"
        />
        <datalist id="ollama-model-options">
          {#each ollamaModels as modelName}
            <option value={modelName}></option>
          {/each}
        </datalist>
      </label>
      <label class="block text-sm text-[var(--text-secondary)]">
        API key env var
        <input
          type="text"
          bind:value={ai.api_key_env}
          placeholder="OPENAI_API_KEY (optional)"
          class="mt-1 w-full text-sm font-mono bg-[var(--bg-muted)] border border-[var(--border)] rounded-lg px-3 py-2 text-[var(--text)]"
        />
      </label>
      <label class="block text-sm text-[var(--text-secondary)]">
        Timeout (seconds)
        <input
          type="number"
          min="10"
          max="300"
          bind:value={ai.timeout_secs}
          class="mt-1 w-full text-sm bg-[var(--bg-muted)] border border-[var(--border)] rounded-lg px-3 py-2 text-[var(--text)]"
        />
      </label>
    </div>
    <div class="flex flex-wrap items-center gap-3 mb-3">
      <button
        type="button"
        onclick={testAiConnection}
        disabled={testingAi || !ai.model.trim()}
        class="px-4 py-2 text-sm rounded-lg border border-[var(--border)] text-[var(--text)] hover:bg-[var(--bg-muted)] transition-colors disabled:opacity-50"
      >
        {testingAi ? 'Testing…' : 'Test connection'}
      </button>
      <button
        type="button"
        onclick={refreshOllamaModels}
        disabled={loadingModels}
        class="px-4 py-2 text-sm rounded-lg border border-[var(--border)] text-[var(--text-secondary)] hover:bg-[var(--bg-muted)] transition-colors disabled:opacity-50"
      >
        {loadingModels ? 'Refreshing…' : 'Refresh models'}
      </button>
      {#if ollamaModels.length > 0}
        <span class="text-xs text-[var(--text-muted)]">{ollamaModels.length} model(s) from Ollama</span>
      {/if}
    </div>
    {#if aiTestMessage}
      <p class="text-sm mb-3 {aiTestMessage.startsWith('Connected') ? 'text-[var(--text-secondary)]' : 'text-red-400'}">
        {aiTestMessage}
      </p>
    {/if}
    <p class="text-xs text-[var(--text-muted)] mt-3">
      Use <strong class="font-medium">Save settings</strong> below to persist AI, privacy, collectors, and watch directories together.
    </p>
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
    <div class="flex flex-col sm:flex-row sm:items-end gap-4">
      <div>
        <p class="text-sm text-[var(--text-secondary)] mb-2">Shell</p>
        <div class="flex flex-wrap gap-2" role="group" aria-label="Shell type">
          {#each shellOptions as opt}
            <button
              type="button"
              onclick={() => (shellChoice = opt.value)}
              class="px-4 py-2 text-sm rounded-lg border transition-colors min-w-[4.5rem]"
              class:border-[var(--accent)]={shellChoice === opt.value}
              class:text-[var(--accent)]={shellChoice === opt.value}
              class:bg-[var(--accent-muted)]={shellChoice === opt.value}
              class:border-[var(--border)]={shellChoice !== opt.value}
              class:text-[var(--text-secondary)]={shellChoice !== opt.value}
              class:hover:border-[var(--text-muted)]={shellChoice !== opt.value}
            >
              {opt.label}
            </button>
          {/each}
        </div>
      </div>
      <button
        type="button"
        onclick={installHook}
        disabled={installingHook}
        class="px-4 py-2 text-sm rounded-lg border border-[var(--border)] text-[var(--text-secondary)] hover:border-[var(--accent)]/40 transition-colors disabled:opacity-50 shrink-0"
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
          <code class="text-xs text-[var(--text-muted)] font-mono">~/.chronicle/chronicle.sock</code>
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
          <p class="text-sm text-[var(--text-secondary)]">Background service not running.</p>
          <p class="text-xs text-[var(--text-muted)] mt-2 max-w-md mx-auto leading-relaxed">
            Chronicle runs a per-user background service (no administrator password). Open the app once or click Restart daemon below — it installs automatically.
          </p>
          <button
            type="button"
            onclick={restartDaemon}
            disabled={restarting}
            class="mt-4 px-4 py-2 text-sm rounded-lg bg-[var(--accent)] text-white hover:opacity-90 disabled:opacity-50"
          >
            {restarting ? 'Starting…' : 'Start capturing'}
          </button>
        </div>
      {/if}
    </div>
  </section>
</PageShell>
