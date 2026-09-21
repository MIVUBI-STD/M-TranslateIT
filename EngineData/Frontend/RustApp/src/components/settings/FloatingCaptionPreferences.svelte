<script lang="ts">
  import { Eye } from "@lucide/svelte";
  import {
    notifyOverlayPreferencesChanged,
    showTranslationOverlay,
  } from "../../app/runtime/translationOverlayRuntime";
  import {
    readOverlayDiagnostic,
    readOverlayPreferences,
    updateOverlayPreferences,
  } from "../../app/runtime/translationOverlayState";
  import type {
    OverlayContrast,
    OverlayTextSize,
    OverlayWidth,
  } from "../../app/runtime/translationOverlayPolicy";

  let {
    onNotice,
  }: {
    onNotice: (message: string) => void;
  } = $props();

  let preferences = $state(readOverlayPreferences());
  let diagnostic = $state(readOverlayDiagnostic());

  async function update(
    patch: {
      meetingEnabled?: boolean;
      textSize?: OverlayTextSize;
      width?: OverlayWidth;
      contrast?: OverlayContrast;
    },
  ): Promise<void> {
    preferences = updateOverlayPreferences(patch);
    await notifyOverlayPreferencesChanged(preferences);
    diagnostic = readOverlayDiagnostic();
    onNotice("Floating caption preference saved.");
  }

  async function show(): Promise<void> {
    const shown = await showTranslationOverlay();
    preferences = readOverlayPreferences();
    diagnostic = readOverlayDiagnostic();
    onNotice(shown ? "Floating caption shown." : "Floating caption is unavailable right now.");
  }
</script>

<article class="ti-panel overflow-hidden">
  <header class="border-b border-[var(--ti-border)] bg-[var(--ti-surface-soft)] px-5 py-4">
    <h3 class="m-0 text-[15px] font-semibold">Floating captions</h3>
    <p class="mb-0 mt-1 text-[12px] leading-5 text-[var(--ti-text-muted)]">
      Keep translated speech readable above other windows without turning the caption into a styling panel.
    </p>
  </header>

  <div class="grid gap-5 p-5">
    <label class="flex items-start justify-between gap-5">
      <span>
        <strong class="block text-[13px] font-semibold">Show during meetings</strong>
        <small class="mt-1 block text-[11.5px] leading-5 text-[var(--ti-text-soft)]">
          New committed meeting translations appear automatically. Hiding the caption is always respected.
        </small>
      </span>
      <input
        type="checkbox"
        class="mt-1 size-4"
        checked={preferences.meetingEnabled}
        onchange={(event) => void update({ meetingEnabled: (event.currentTarget as HTMLInputElement).checked })}
      />
    </label>

    <div class="grid grid-cols-3 gap-3">
      <label class="grid gap-2">
        <span class="ti-field-label">Text size</span>
        <select
          class="ti-field min-h-10 px-3"
          value={preferences.textSize}
          onchange={(event) => void update({ textSize: (event.currentTarget as HTMLSelectElement).value as OverlayTextSize })}
        >
          <option value="small">Small</option>
          <option value="medium">Medium · recommended</option>
          <option value="large">Large</option>
          <option value="extra-large">Extra large</option>
        </select>
      </label>

      <label class="grid gap-2">
        <span class="ti-field-label">Reading width</span>
        <select
          class="ti-field min-h-10 px-3"
          value={preferences.width}
          onchange={(event) => void update({ width: (event.currentTarget as HTMLSelectElement).value as OverlayWidth })}
        >
          <option value="compact">Compact</option>
          <option value="standard">Standard · recommended</option>
          <option value="wide">Wide</option>
        </select>
      </label>

      <label class="grid gap-2">
        <span class="ti-field-label">Contrast</span>
        <select
          class="ti-field min-h-10 px-3"
          value={preferences.contrast}
          onchange={(event) => void update({ contrast: (event.currentTarget as HTMLSelectElement).value as OverlayContrast })}
        >
          <option value="standard">Standard</option>
          <option value="high">High contrast</option>
        </select>
      </label>
    </div>

    <div>
      <button type="button" class="ti-button ti-button-secondary" onclick={() => void show()}>
        <Eye size={15} /> Show floating caption
      </button>
    </div>

    {#if diagnostic}
      <p class="m-0 text-[11.5px] leading-5 text-[var(--ti-warning)]">
        Floating caption last reported: {diagnostic.message}
      </p>
    {/if}
  </div>
</article>
