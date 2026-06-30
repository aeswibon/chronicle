<script>
  import { getAppIconDataUrl } from '$lib/appIcons.js';
  import { eventIconChar, eventIconKey } from '$lib/format.js';

  /** @type {{ event: Record<string, unknown>, size?: number, class?: string }} */
  let { event, size = 32, class: className = '' } = $props();

  let dataUrl = $state(null);
  let loading = $state(true);

  $effect(() => {
    const key = eventIconKey(event);
    loading = true;
    dataUrl = null;

    if (!key) {
      loading = false;
      return;
    }

    let cancelled = false;
    getAppIconDataUrl(key).then((url) => {
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
    <span class="text-xs font-medium text-[var(--text-secondary)]">{eventIconChar(event)}</span>
  {/if}
</div>
