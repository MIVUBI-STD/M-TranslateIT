<script lang="ts">
  import { Plus, Trash2, Video } from "@lucide/svelte";
  import type { MeetingAppDetection } from "../../app/bridge/runtimeApi";
  import { runtimeApi } from "../../app/bridge/runtimeApi";
  import {
    deleteMeetingPreset,
    meetingPresetFromSettings,
    readMeetingPresets,
    rememberProviderPreset,
    saveMeetingPreset,
    suggestedPresetForProvider,
    type MeetingPreset,
  } from "../../app/runtime/meetingPresetState";
  import type { RuntimeSettings } from "../../app/shared/types";
  import StatusBadge from "../ui/StatusBadge.svelte";

  let {
    detection,
    settings,
    locked = false,
    advisoryCopy,
    onRefresh,
    onNotice,
  }: {
    detection: MeetingAppDetection | null;
    settings: RuntimeSettings;
    locked?: boolean;
    advisoryCopy: string;
    onRefresh: (preferredNotice?: string, knownSettings?: RuntimeSettings) => void | Promise<void>;
    onNotice: (message: string) => void;
  } = $props();

  let presets = $state<MeetingPreset[]>(readMeetingPresets());
  let presetName = $state("");
  let applying = $state(false);
  const suggested = $derived.by(() => {
    presets;
    return suggestedPresetForProvider(detection?.detected ? detection.provider : null);
  });

  function saveCurrent(): void {
    if (locked) return;
    const name = presetName.trim();
    if (!name) { onNotice("Name the preset before saving it."); return; }
    const result = saveMeetingPreset(meetingPresetFromSettings(`preset-${Date.now()}`, name, settings));
    if (!result.ok) {
      onNotice("Meeting preset couldn't be saved locally. The previous presets were kept.");
      return;
    }
    presets = result.presets;
    presetName = "";
    onNotice("Meeting preset saved.");
  }

  function removePreset(id: string): void {
    if (applying) return;
    const result = deleteMeetingPreset(id);
    if (!result.ok) {
      onNotice("Meeting preset couldn't be removed from local storage.");
      return;
    }
    presets = result.presets;
    onNotice("Meeting preset removed.");
  }

  async function applyPreset(preset: MeetingPreset): Promise<void> {
    if (locked || applying) {
      onNotice("Stop Translation before changing a Meeting preset.");
      return;
    }
    applying = true;
    try {
      const current = (await runtimeApi.loadSettings()) ?? settings;
      const candidate: RuntimeSettings = {
        ...current,
        source_language: preset.sourceLanguage,
        target_language: preset.targetLanguage,
        meeting_listen_source_language: preset.listenSourceLanguage,
        meeting_listen_target_language: preset.listenTargetLanguage,
        translation_style: preset.translationStyle,
        audio: {
          ...current.audio,
          input_device_id: preset.microphoneId,
          output_device_id: preset.meetingSoundId,
          noise_suppression: preset.noiseSuppression,
        },
      };
      const result = await runtimeApi.applyMeetingPreset(candidate);
      if (!result.ok) { onNotice(result.message); return; }
      if (detection?.detected && detection.provider) rememberProviderPreset(detection.provider, preset.id);
      await onRefresh(`${preset.name} preset applied.`, result.settings);
    } catch {
      onNotice("Meeting preset couldn't be applied. The previous setup was kept.");
    } finally {
      applying = false;
    }
  }
</script>

<section class="border-b border-[var(--ti-border)] bg-[var(--ti-surface)] px-5 py-3.5">
  <div class="flex flex-wrap items-center justify-between gap-3">
    <div class="flex min-w-0 items-center gap-3">
      {#if detection?.detected}
        <div class="grid size-8 shrink-0 place-items-center rounded-[9px] border border-[var(--ti-border)] bg-[var(--ti-surface-soft)] text-[var(--ti-text-muted)]" aria-hidden="true"><Video size={15} /></div>
        <div class="min-w-0">
          <strong class="block truncate text-[12.5px] font-semibold">{detection.provider} detected</strong>
          <p class="mb-0 mt-0.5 truncate text-[11px] text-[var(--ti-text-soft)]">{suggested ? `Suggested preset: ${suggested.name}. ${advisoryCopy}` : advisoryCopy}</p>
        </div>
        <StatusBadge label="Detected" tone="good" />
      {:else}
        <div>
          <strong class="block text-[12.5px] font-semibold">Meeting presets</strong>
          <p class="mb-0 mt-0.5 text-[11px] text-[var(--ti-text-soft)]">Save up to 6 user-driven setups. Devices are validated before switching.</p>
        </div>
      {/if}
    </div>

    <div class="flex flex-wrap items-center gap-2">
      {#if suggested}
        <button type="button" class="ti-button min-h-8 px-3 text-xs" disabled={locked || applying} onclick={() => void applyPreset(suggested)}>Use {suggested.name}</button>
      {/if}
      <select class="ti-field min-h-8 w-[150px] px-2 text-xs" disabled={locked || applying || presets.length === 0} aria-label="Saved Meeting preset" onchange={(event) => {
        const element = event.currentTarget as HTMLSelectElement;
        const preset = presets.find((entry) => entry.id === element.value);
        if (preset) void applyPreset(preset);
        element.value = "";
      }}>
        <option value="">Use preset...</option>
        {#each presets as preset (preset.id)}<option value={preset.id}>{preset.name}</option>{/each}
      </select>
    </div>
  </div>

  <div class="mt-3 flex flex-wrap items-center gap-2">
    <input class="ti-field min-h-8 w-[200px] px-2.5 text-xs" maxlength="40" placeholder="Preset name" bind:value={presetName} disabled={locked || applying} />
    <button type="button" class="ti-button ti-button-secondary min-h-8 px-3 text-xs" disabled={locked || applying || !presetName.trim()} onclick={saveCurrent}><Plus size={14} /> Save current</button>
    {#if presets.length > 0}
      <div class="ml-auto flex max-w-full flex-wrap justify-end gap-1.5">
        {#each presets as preset (preset.id)}
          <button type="button" class="ti-pill flex items-center gap-1.5" title={`Remove ${preset.name}`} disabled={applying} onclick={() => removePreset(preset.id)}>
            {preset.name}<Trash2 size={11} />
          </button>
        {/each}
      </div>
    {/if}
  </div>
</section>
