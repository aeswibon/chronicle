<script>
  let { title, description = '', children } = $props();
</script>

<div class="max-w-2xl mx-auto">
  <header class="mb-8 pb-6 border-b border-[var(--border-subtle)]">
    <h2 class="text-lg font-semibold text-[var(--text)] tracking-tight">{title}</h2>
    {#if description}
      <p class="mt-2 text-sm text-[var(--text-muted)] leading-relaxed break-all line-clamp-2" title={description}>
        {description}
      </p>
    {/if}
  </header>

  <div class="pt-2">
    {#if children}
      {@render children()}
    {/if}
  </div>
</div>
