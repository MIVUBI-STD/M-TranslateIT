<script lang="ts">
  let { style, disabled = false, onSelect }: {
    style: string;
    disabled?: boolean;
    onSelect: (style: "natural" | "formal") => void | Promise<void>;
  } = $props();
</script>

<article class="ti-panel overflow-hidden">
  <header class="border-b border-[var(--ti-border)] bg-[var(--ti-surface-soft)] px-5 py-4">
    <h3 class="m-0 text-[15px] font-semibold">Translation style</h3>
    <p class="mb-0 mt-1 text-[12px] leading-5 text-[var(--ti-text-muted)]">Choose how translated wording should sound. Meaning, names, numbers, and preferred terminology stay authoritative.</p>
  </header>
  <div class="grid grid-cols-2 gap-3 p-5">
    <button type="button" class={`rounded-[10px] border p-4 text-left ${style !== "formal" ? "border-[var(--ti-border-strong)] bg-[var(--ti-surface-raised)]" : "border-[var(--ti-border)] bg-[var(--ti-surface)]"}`} disabled={disabled} aria-pressed={style !== "formal"} onclick={() => void onSelect("natural")}>
      <strong class="block text-[13px] font-semibold">Natural</strong>
      <span class="mt-1 block text-[11.5px] leading-5 text-[var(--ti-text-soft)]">Default · clear, conversational wording that follows the source naturally.</span>
    </button>
    <button type="button" class={`rounded-[10px] border p-4 text-left ${style === "formal" ? "border-[var(--ti-border-strong)] bg-[var(--ti-surface-raised)]" : "border-[var(--ti-border)] bg-[var(--ti-surface)]"}`} disabled={disabled} aria-pressed={style === "formal"} onclick={() => void onSelect("formal")}>
      <strong class="block text-[13px] font-semibold">Formal</strong>
      <span class="mt-1 block text-[11.5px] leading-5 text-[var(--ti-text-soft)]">Professional register for client calls, presentations, and formal communication.</span>
    </button>
  </div>
</article>
