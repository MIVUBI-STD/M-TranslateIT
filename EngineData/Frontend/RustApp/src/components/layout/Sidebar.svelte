<script lang="ts">
  import { AudioLines, Languages, Mic, Settings } from "@lucide/svelte";
  import type { AppRoute } from "../../app/shared/types";

  let {
    active,
    presence,
    onNavigate,
  }: {
    active: AppRoute;
    presence: string;
    onNavigate: (route: AppRoute) => void;
  } = $props();

  const items = [
    { route: "meeting" as const, label: "Meeting", description: "Voice translation", icon: Mic },
    { route: "text" as const, label: "Text", description: "Indonesian ↔ English", icon: Languages },
    { route: "my-voice" as const, label: "My Voice", description: "Create your meeting voice", icon: AudioLines },
    { route: "settings" as const, label: "Settings", description: "Audio & setup", icon: Settings },
  ];

  function presenceDot(): string {
    if (presence === "Live" || presence === "Ready") return "bg-[var(--ti-success)]";
    if (presence === "Unavailable") return "bg-[var(--ti-danger)]";
    if (presence === "Setup Needed") return "bg-[var(--ti-warning)]";
    return "bg-[var(--ti-text-soft)]";
  }

  function presenceLabel(): string {
    if (presence === "Live") return "Translation is live";
    if (presence === "Ready") return "Ready to translate";
    if (presence === "Degraded") return "Text translation ready";
    if (presence === "Setup Needed") return "Setup needed";
    if (presence === "Unavailable") return "Unavailable";
    return "Checking";
  }
</script>

<aside class="flex min-h-screen w-[var(--ti-sidebar-width)] shrink-0 flex-col border-r border-[var(--ti-border)] bg-[var(--ti-sidebar)] px-3 py-4">
  <header class="flex items-center gap-2.5 px-2 py-1">
    <div class="grid size-9 place-items-center rounded-[var(--ti-radius-md)] border border-[var(--ti-border-strong)] bg-[var(--ti-surface-raised)] text-sm font-bold">T</div>
    <div class="min-w-0">
      <h1 class="m-0 truncate text-[14px] font-bold tracking-[-0.02em]">TranslateIT</h1>
      <p class="mt-0.5 truncate text-[11px] text-[var(--ti-text-soft)]">Indonesian ↔ English</p>
    </div>
  </header>

  <nav class="mt-7 grid gap-1" aria-label="Primary navigation">
    {#each items as item}
      {@const Icon = item.icon}
      <button
        type="button"
        class={`group grid min-h-11 grid-cols-[20px_1fr] items-center gap-2.5 rounded-[var(--ti-radius-md)] border px-3 text-left transition-colors ${active === item.route ? "border-[var(--ti-border)] bg-[var(--ti-surface-raised)] text-[var(--ti-text)]" : "border-transparent text-[var(--ti-text-muted)] hover:bg-[var(--ti-surface-soft)] hover:text-[var(--ti-text)]"}`}
        aria-current={active === item.route ? "page" : undefined}
        title={item.description}
        onclick={() => onNavigate(item.route)}
      >
        <Icon size={17} strokeWidth={1.8} />
        <strong class="truncate text-[13px] font-semibold">{item.label}</strong>
      </button>
    {/each}
  </nav>

  <div class="mt-auto border-t border-[var(--ti-border)] px-2 pt-3">
    <div class="flex items-center gap-2" role="status" aria-live="polite" aria-atomic="true">
      <span class={`size-1.5 rounded-full ${presenceDot()}`} aria-hidden="true"></span>
      <strong class="truncate text-[11px] font-semibold text-[var(--ti-text-muted)]">{presenceLabel()}</strong>
    </div>
  </div>
</aside>
