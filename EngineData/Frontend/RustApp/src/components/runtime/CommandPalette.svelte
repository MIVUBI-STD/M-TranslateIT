<script lang="ts">
  import { Search, X } from "@lucide/svelte";
  import { tick } from "svelte";
  import type { ProductCommand } from "../../app/runtime/productCommandRegistry";

  let {
    open = $bindable(false),
    commands,
  }: {
    open?: boolean;
    commands: ProductCommand[];
  } = $props();

  let query = $state("");
  let input = $state<HTMLInputElement | null>(null);

  const filtered = $derived.by(() => {
    const needle = query.trim().toLowerCase();
    if (!needle) return commands;
    return commands.filter((command) =>
      [command.label, command.description, ...command.keywords]
        .join(" ")
        .toLowerCase()
        .includes(needle),
    );
  });

  async function show(): Promise<void> {
    query = "";
    await tick();
    input?.focus();
  }

  async function run(command: ProductCommand): Promise<void> {
    if (command.disabled) return;
    open = false;
    query = "";
    await command.run();
  }

  function handleKeydown(event: KeyboardEvent): void {
    if (event.key === "Escape") {
      event.preventDefault();
      open = false;
      return;
    }
    if (event.key === "Enter") {
      const command = filtered.find((entry) => !entry.disabled);
      if (command) {
        event.preventDefault();
        void run(command);
      }
    }
  }

  $effect(() => {
    if (open) void show();
  });
</script>

{#if open}
  <div
    class="fixed inset-0 z-[80] grid place-items-start bg-black/35 px-6 pt-[12vh]"
    role="presentation"
    onclick={(event) => { if (event.currentTarget === event.target) open = false; }}
  >
    <div
      class="w-full max-w-[620px] overflow-hidden rounded-[16px] border border-[var(--ti-border-strong)] bg-[var(--ti-surface-raised)] shadow-2xl"
      role="dialog"
      tabindex="-1"
      aria-modal="true"
      aria-label="Quick actions"
      onkeydown={handleKeydown}
    >
      <header class="flex items-center gap-3 border-b border-[var(--ti-border)] px-4 py-3">
        <Search size={17} class="shrink-0 text-[var(--ti-text-soft)]" />
        <input
          bind:this={input}
          bind:value={query}
          class="min-w-0 flex-1 border-0 bg-transparent text-[14px] outline-none placeholder:text-[var(--ti-text-soft)]"
          placeholder="Search actions..."
          aria-label="Search quick actions"
        />
        <button type="button" class="grid size-8 place-items-center rounded-[8px] text-[var(--ti-text-muted)] hover:bg-[var(--ti-surface-soft)]" aria-label="Close quick actions" onclick={() => { open = false; }}>
          <X size={16} />
        </button>
      </header>

      <div class="max-h-[420px] overflow-y-auto p-2">
        {#if filtered.length === 0}
          <p class="m-0 px-3 py-8 text-center text-[12.5px] text-[var(--ti-text-muted)]">No matching action.</p>
        {:else}
          {#each filtered as command (command.id)}
            <button
              type="button"
              class="flex w-full items-center justify-between gap-5 rounded-[10px] px-3 py-3 text-left transition-colors hover:bg-[var(--ti-surface-soft)] disabled:cursor-not-allowed disabled:opacity-45"
              disabled={command.disabled}
              onclick={() => void run(command)}
            >
              <span class="min-w-0">
                <strong class="block text-[13px] font-semibold">{command.label}</strong>
                <span class="mt-0.5 block truncate text-[11.5px] text-[var(--ti-text-soft)]">{command.description}</span>
              </span>
            </button>
          {/each}
        {/if}
      </div>

      <footer class="flex items-center justify-between border-t border-[var(--ti-border)] bg-[var(--ti-surface-soft)] px-4 py-2.5 text-[10.5px] text-[var(--ti-text-soft)]">
        <span>Enter to run · Esc to close</span>
        <span>Local actions only</span>
      </footer>
    </div>
  </div>
{/if}
