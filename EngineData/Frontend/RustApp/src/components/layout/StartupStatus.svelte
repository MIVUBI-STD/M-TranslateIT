<script lang="ts">
  let { loading, message = "", onRetry }: {
    loading: boolean;
    message?: string;
    onRetry: () => void | Promise<void>;
  } = $props();
</script>

{#if loading}
<main class="grid min-h-screen place-items-center bg-[var(--ti-bg)] p-8">
    <section class="w-full max-w-[560px] text-center">
      <div class="mx-auto grid size-12 place-items-center rounded-[14px] border border-[var(--ti-border-strong)] bg-[var(--ti-surface-raised)] text-lg font-bold">T</div>
      <h1 class="mb-0 mt-5 text-2xl font-semibold tracking-[-0.03em]">Getting TranslateIT ready</h1>
      <p class="mb-0 mt-2 text-sm leading-6 text-[var(--ti-text-muted)]">Checking your saved setup.</p>
    </section>
  </main>
{:else}
<main class="grid min-h-screen place-items-center bg-[var(--ti-bg)] p-8">
    <section class="ti-panel w-full max-w-[560px] p-7">
      <span class="ti-kicker">TranslateIT</span>
      <h1 class="mb-0 mt-3 text-2xl font-semibold">TranslateIT isn't ready yet</h1>
      <p class="mb-0 mt-3 text-sm leading-6 text-[var(--ti-text-muted)]">{message}</p>
      <button type="button" class="ti-button mt-5" onclick={() => void onRetry()}>Try Again</button>
    </section>
  </main>
{/if}
