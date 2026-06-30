<script>
  import '../app.css';
  let { children } = $props();
</script>

<div class="min-h-screen bg-gray-950 text-gray-100">
  <nav class="border-b border-gray-800 px-6 py-3 flex items-center gap-6">
    <h1 class="text-lg font-semibold text-emerald-400">Chronicle</h1>
    <div class="flex gap-4 text-sm text-gray-400">
      <a href="/" class="hover:text-white transition-colors">Timeline</a>
      <a href="/projects" class="hover:text-white transition-colors">Projects</a>
      <a href="/search" class="hover:text-white transition-colors">Search</a>
      <a href="/settings" class="hover:text-white transition-colors">Settings</a>
    </div>
    <div class="ml-auto flex items-center gap-2 text-xs">
      <span class="w-2 h-2 rounded-full bg-emerald-500 inline-block"></span>
      <span class="text-gray-500">Daemon connected</span>
    </div>
  </nav>
  <main class="p-6">
    {#if children}
      {@render children()}
    {/if}
  </main>
</div>
