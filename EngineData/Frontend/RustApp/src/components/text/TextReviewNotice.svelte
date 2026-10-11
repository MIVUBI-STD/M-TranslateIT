<script lang="ts">
  import { ShieldAlert } from "@lucide/svelte";
  let { hints }: { hints: string[] } = $props();
</script>
<section class="border-t border-[var(--ti-warning-border)] bg-[var(--ti-warning-surface)] px-5 py-3.5" aria-live="polite">
        <div class="flex items-start gap-2.5">
          <ShieldAlert size={15} class="mt-0.5 shrink-0 text-[var(--ti-warning)]" />
          <div>
            <strong class="text-[12px] font-semibold">Review important details</strong>
            <p class="mb-0 mt-1 text-[11.5px] leading-5 text-[var(--ti-text-muted)]">{hints.join(" ")}</p>
          </div>
        </div>
      </section>
