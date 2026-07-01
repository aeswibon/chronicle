<script>
  import '../app.css';
  import { invoke } from '@tauri-apps/api/core';
  import { onMount } from 'svelte';
  import { page } from '$app/stores';
  import { theme, initTheme, toggleTheme } from '$lib/theme.svelte.js';

  let { children } = $props();
  let daemonConnected = $state(false);

  async function checkDaemon() {
    try {
      await invoke('get_status');
      daemonConnected = true;
    } catch {
      daemonConnected = false;
    }
  }

  onMount(() => {
    initTheme();
    checkDaemon();
    const interval = setInterval(checkDaemon, 5000);
    return () => clearInterval(interval);
  });

  const navItems = [
    { href: '/', label: 'Timeline' },
    { href: '/projects', label: 'Projects' },
    { href: '/errors', label: 'Errors' },
    { href: '/search', label: 'Search' },
    { href: '/settings', label: 'Settings' },
  ];
</script>

<div class="min-h-screen bg-[var(--bg)] text-[var(--text)] flex flex-col">
  <nav class="border-b border-[var(--border)] px-6 py-0 flex items-center gap-6 h-12 shrink-0 bg-[var(--bg-elevated)]">
    <h1 class="text-sm font-semibold text-[var(--accent)] tracking-wide">chronicle</h1>
    <div class="flex items-center h-full gap-1">
      {#each navItems as item}
        <a
          href={item.href}
          class="relative flex items-center h-full px-3 text-xs font-medium transition-colors"
          class:text-[var(--accent)]={$page.url.pathname === item.href}
          class:text-[var(--text-muted)]={$page.url.pathname !== item.href}
          class:hover:text-[var(--text-secondary)]={$page.url.pathname !== item.href}
        >
          {item.label}
          {#if $page.url.pathname === item.href}
            <span class="absolute bottom-0 left-1/2 -translate-x-1/2 w-6 h-[2px] bg-[var(--accent)] rounded-full"></span>
          {/if}
        </a>
      {/each}
    </div>
    <div class="ml-auto flex items-center gap-4">
      <button
        type="button"
        onclick={toggleTheme}
        class="p-1.5 rounded-md text-[var(--text-muted)] hover:text-[var(--text-secondary)] hover:bg-[var(--bg-muted)] transition-colors"
        aria-label={theme.isDark ? 'Switch to light mode' : 'Switch to dark mode'}
        title={theme.isDark ? 'Light mode' : 'Dark mode'}
      >
        {#if theme.isDark}
          <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="4"/><path d="M12 2v2"/><path d="M12 20v2"/><path d="m4.93 4.93 1.41 1.41"/><path d="m17.66 17.66 1.41 1.41"/><path d="M2 12h2"/><path d="M20 12h2"/><path d="m6.34 17.66-1.41 1.41"/><path d="m19.07 4.93-1.41 1.41"/></svg>
        {:else}
          <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 3a6 6 0 0 0 9 9 9 9 0 1 1-9-9Z"/></svg>
        {/if}
      </button>
      <div class="flex items-center gap-2 text-xs">
        <span
          class="w-1.5 h-1.5 rounded-full transition-colors"
          class:bg-[var(--accent)]={daemonConnected}
          class:bg-[var(--text-muted)]={!daemonConnected}
        ></span>
        <span class="text-[var(--text-muted)]">daemon {daemonConnected ? 'connected' : 'disconnected'}</span>
      </div>
    </div>
  </nav>
  <main class="flex-1 px-8 py-10 overflow-y-auto">
    {#if children}
      {@render children()}
    {/if}
  </main>
</div>
