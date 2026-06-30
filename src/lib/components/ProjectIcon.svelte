<script>
  import { getPathIconDataUrl } from '$lib/appIcons.js';

  /** @type {{ name: string, path: string, size?: number, iconUrl?: string | null, class?: string }} */
  let { name, path, size = 36, iconUrl = null, class: className = '' } = $props();

  let dataUrl = $state(iconUrl);
  let loading = $state(!iconUrl);

  $effect(() => {
    if (iconUrl) {
      dataUrl = iconUrl;
      loading = false;
      return;
    }

    loading = true;
    dataUrl = null;

    let cancelled = false;
    getPathIconDataUrl(path).then((url) => {
      if (cancelled) return;
      dataUrl = url;
      loading = false;
    });

    return () => {
      cancelled = true;
    };
  });
</script>

<div
  class="rounded-lg bg-[var(--bg-muted)] flex items-center justify-center overflow-hidden shrink-0 {className}"
  style="width: {size}px; height: {size}px"
>
  {#if dataUrl}
    <img src={dataUrl} alt="" class="w-full h-full object-contain" draggable="false" />
  {:else if !loading}
    <span class="text-sm font-medium text-[var(--text-secondary)]">{name.charAt(0).toUpperCase()}</span>
  {/if}
</div>
