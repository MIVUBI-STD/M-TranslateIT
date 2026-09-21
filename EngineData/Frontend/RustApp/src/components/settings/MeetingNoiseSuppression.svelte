<script lang="ts">
  import { runtimeApi } from "../../app/bridge/runtimeApi";
  import type { RuntimeSettings } from "../../app/shared/types";

  let {
    settings,
    locked = false,
    onSettingsChange,
    onNotice,
  }: {
    settings: RuntimeSettings;
    locked?: boolean;
    onSettingsChange: (settings: RuntimeSettings) => void | Promise<void>;
    onNotice: (message: string) => void;
  } = $props();

  let saving = $state(false);

  async function changeMode(mode: "auto" | "off"): Promise<void> {
    if (saving || locked || settings.audio.noise_suppression === mode) return;
    saving = true;
    try {
      const candidate: RuntimeSettings = {
        ...settings,
        audio: { ...settings.audio, noise_suppression: mode },
      };
      const result = await runtimeApi.saveSettings(candidate);
      if (!result.ok) {
        onNotice(result.message || "Noise suppression setting couldn't be saved.");
        return;
      }
      const saved = (await runtimeApi.loadSettings()) ?? candidate;
      await onSettingsChange(saved);
      onNotice(mode === "auto" ? "Noise suppression set to Auto." : "Noise suppression turned off.");
    } catch {
      onNotice("Noise suppression setting couldn't be saved.");
    } finally {
      saving = false;
    }
  }
</script>

<div class="flex items-start justify-between gap-5 border-t border-[var(--ti-border)] px-5 py-4">
  <div class="min-w-0">
    <span class="ti-field-label">Noise suppression</span>
    <strong class="mt-1.5 block text-[13px] font-semibold leading-5">
      {settings.audio.noise_suppression === "off" ? "Off" : "Auto · recommended"}
    </strong>
    <p class="mb-0 mt-1 text-[11.5px] leading-5 text-[var(--ti-text-soft)]">
      Auto lightly reduces low-level background floor on finalized speech before ASR. Speech detection still uses the original audio.
    </p>
  </div>
  <select
    class="ti-field min-h-9 w-[180px] px-3"
    disabled={locked || saving}
    value={settings.audio.noise_suppression === "off" ? "off" : "auto"}
    aria-label="Noise suppression"
    onchange={(event) => void changeMode((event.currentTarget as HTMLSelectElement).value as "auto" | "off")}
  >
    <option value="auto">Auto · recommended</option>
    <option value="off">Off</option>
  </select>
</div>
