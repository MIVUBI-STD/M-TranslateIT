<script lang="ts">
  import { Search, X } from "@lucide/svelte";
  import { tick } from "svelte";
  import type { ProductCommand } from "../../app/runtime/productCommandRegistry";

  let { open = $bindable(false), commands }: {
    open?: boolean;
    commands: ProductCommand[];
  } = $props();

  const groups: ProductCommand["group"][] = ["Navigation", "Meeting utilities", "Text utilities", "Settings & support"];
  let query = $state("");
  let input = $state<HTMLInputElement>();
  let dialog = $state<HTMLDivElement>();
  let previousFocus: HTMLElement | null = null;
  const filtered = $derived.by(() => {
    const needle = query.trim().toLowerCase();
    return needle ? commands.filter(command =>
      [command.label, command.description, ...command.keywords].join(" ").toLowerCase().includes(needle)
    ) : commands;
  });

  async function run(command: ProductCommand): Promise<void> {
    if (command.disabled) return;
    // Destination components own focus after a utility jump. Do not race
    // their focus with the palette trigger restoration.
    if (command.handsOffFocus) previousFocus = null;
    open = false;
    query = "";
    await command.run();
  }

  function availableButtons(): HTMLButtonElement[] {
    return Array.from(dialog?.querySelectorAll<HTMLButtonElement>("button[data-command-id]:not(:disabled)") ?? []);
  }

  function onInputKeydown(event: KeyboardEvent): void {
    if (event.key === "ArrowDown") {
      event.preventDefault();
      availableButtons()[0]?.focus();
    } else if (event.key === "Enter") {
      // Enter in search executes the first enabled matching command, never a
      // different command while a separate result button has keyboard focus.
      event.preventDefault();
      const first = filtered.find(command => !command.disabled);
      if (first) void run(first);
    }
  }

  function onResultKeydown(event: KeyboardEvent, id: string): void {
    if (event.key !== "ArrowDown" && event.key !== "ArrowUp") return;
    event.preventDefault();
    const buttons = availableButtons();
    const index = buttons.findIndex(button => button.dataset["commandId"] === id);
    const target = index + (event.key === "ArrowDown" ? 1 : -1);
    if (target < 0) input?.focus();
    else buttons[Math.min(target, buttons.length - 1)]?.focus();
  }

  function onDialogKeydown(event: KeyboardEvent): void {
    if (event.key === "Escape") {
      event.preventDefault();
      open = false;
      return;
    }
    if (event.key !== "Tab") return;
    const focusable = Array.from(dialog?.querySelectorAll<HTMLElement>(
      'input:not([disabled]),button:not([disabled])'
    ) ?? []);
    if (!focusable.length) return;
    const first = focusable[0], last = focusable[focusable.length - 1];
    if (!first || !last) return;
    if (event.shiftKey && document.activeElement === first) {
      event.preventDefault();
      last.focus();
    } else if (!event.shiftKey && document.activeElement === last) {
      event.preventDefault();
      first.focus();
    }
  }

  $effect(() => {
    if (open) {
      previousFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null;
      query = "";
      void tick().then(() => { if (open) input?.focus(); });
    } else if (previousFocus) {
      const restore = previousFocus;
      previousFocus = null;
      void tick().then(() => { if (restore.isConnected) restore.focus(); });
    }
  });
</script>

{#if open}
  <div class="fixed inset-0 z-[80] grid place-items-start bg-black/35 px-4 pt-[8vh] sm:px-6"
    role="presentation" onclick={(event) => { if (event.currentTarget === event.target) open = false; }}>
    <div bind:this={dialog}
      class="w-full max-w-[620px] overflow-hidden rounded-[16px] border border-[var(--ti-border-strong)] bg-[var(--ti-surface-raised)] shadow-2xl"
      role="dialog" tabindex="-1" aria-modal="true" aria-label="Quick actions"
      aria-describedby="command-help" onkeydown={onDialogKeydown}>
      <header class="flex items-center gap-3 border-b border-[var(--ti-border)] px-4 py-3">
        <Search size={17} class="shrink-0 text-[var(--ti-text-soft)]" />
        <input bind:this={input} bind:value={query} onkeydown={onInputKeydown}
          class="min-w-0 flex-1 border-0 bg-transparent text-[14px] outline-none placeholder:text-[var(--ti-text-soft)]"
          placeholder="Search tools and settings..." aria-label="Search quick actions" />
        <button type="button" class="grid size-8 place-items-center rounded-[8px] text-[var(--ti-text-muted)] hover:bg-[var(--ti-surface-soft)]"
          aria-label="Close quick actions" onclick={() => { open = false; }}><X size={16}/></button>
      </header>
      <div class="max-h-[min(60vh,420px)] overflow-y-auto p-2" role="region" aria-label="Available actions">
        {#if filtered.length === 0}
          <p class="m-0 px-3 py-8 text-center text-[12.5px] text-[var(--ti-text-muted)]">No matching action. Try “mic”, “captions”, or “terms”.</p>
        {:else}
          {#each groups as group}
            {#if filtered.some(command => command.group === group)}
              <h3 class="px-3 pb-1 pt-3 text-[10.5px] font-semibold text-[var(--ti-text-muted)]">{group}</h3>
              {#each filtered.filter(command => command.group === group) as command (command.id)}
                <button type="button" data-command-id={command.id}
                  class="flex w-full items-center justify-between gap-5 rounded-[10px] px-3 py-2.5 text-left transition-colors hover:bg-[var(--ti-surface-soft)] focus-visible:outline-2 focus-visible:outline-offset-[-2px] focus-visible:outline-[var(--ti-accent)] disabled:cursor-not-allowed disabled:opacity-50"
                  disabled={command.disabled} title={command.description}
                  onkeydown={(event) => onResultKeydown(event, command.id)}
                  onclick={() => void run(command)}>
                  <span class="min-w-0">
                    <strong class="block text-[13px] font-semibold">{command.label}</strong>
                    <span class="mt-0.5 block text-[11.5px] leading-5 text-[var(--ti-text-soft)]">{command.description}</span>
                  </span>
                </button>
              {/each}
            {/if}
          {/each}
        {/if}
      </div>
      <footer id="command-help" class="border-t border-[var(--ti-border)] bg-[var(--ti-surface-soft)] px-4 py-2.5 text-[10.5px] text-[var(--ti-text-soft)]">
        Search and Enter to run · ↓ to browse · Esc to close · Actions stay local
      </footer>
    </div>
  </div>
{/if}
