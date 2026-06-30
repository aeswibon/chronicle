<script>
  import { invoke } from '@tauri-apps/api/core';
  let events = $state([]);
  let loading = $state(true);

  async function loadTimeline() {
    loading = true;
    try {
      const since = Date.now() - 86400000;
      events = await invoke('get_timeline', { since });
    } catch (e) {
      console.error('Failed to load timeline:', e);
    } finally {
      loading = false;
    }
  }

  $effect(() => {
    loadTimeline();
  });
</script>

<div class="max-w-3xl mx-auto">
  <div class="flex items-center justify-between mb-6">
    <h2 class="text-xl font-medium">Today</h2>
    <button
      onclick={loadTimeline}
      class="text-xs text-gray-400 hover:text-white border border-gray-700 rounded px-3 py-1 transition-colors"
    >
      Refresh
    </button>
  </div>

  {#if loading}
    <div class="text-center text-gray-500 py-12">Loading…</div>
  {:else if events.length === 0}
    <div class="text-center text-gray-500 py-12">
      <p class="mb-2">No activity recorded yet.</p>
      <p class="text-sm">Start the daemon to begin tracking.</p>
    </div>
  {:else}
    <div class="space-y-3">
      {#each events as event}
        <div class="bg-gray-900 border border-gray-800 rounded-lg p-4">
          <div class="flex items-center justify-between">
            <span class="text-sm font-medium">{event.type}</span>
            <span class="text-xs text-gray-500">{event.source}</span>
          </div>
          {#if event.project}
            <p class="text-xs text-gray-400 mt-1">{event.project}</p>
          {/if}
        </div>
      {/each}
    </div>
  {/if}
</div>
