<script lang="ts">
  import { onMount } from "svelte";
  import { runtimeApi } from "../../app/bridge/runtimeApi";
  import {
    getDeviceLossGuardStatus,
    type DeviceLossGuardStatus,
    type RuntimeWatchdogStatus,
    type StartupRecoveryReport,
  } from "../../app/bridge/reliabilityApi";
  import StatusBadge from "../ui/StatusBadge.svelte";

  let watchdog = $state<RuntimeWatchdogStatus | null>(null);
  let recovery = $state<StartupRecoveryReport | null>(null);
  let devices = $state<DeviceLossGuardStatus | null>(null);

  async function refresh(): Promise<void> {
    [watchdog, recovery, devices] = await Promise.all([
      runtimeApi.getRuntimeWatchdogStatus().catch(() => null),
      runtimeApi.getStartupRecoveryStatus().catch(() => null),
      getDeviceLossGuardStatus().catch(() => null),
    ]);
  }

  onMount(() => {
    void refresh();
  });
</script>

<article class="ti-panel p-5">
  <div class="flex items-start justify-between gap-4">
    <div>
      <span class="ti-field-label">Reliability</span>
      <h4 class="mb-0 mt-1.5 text-[14px] font-semibold">Runtime health</h4>
    </div>
    <StatusBadge
      label={watchdog?.healthy ? "Healthy" : watchdog?.state === "idle" ? "Idle" : "Needs attention"}
      tone={watchdog?.healthy ? "good" : watchdog?.action_required ? "danger" : "warning"}
    />
  </div>

  <div class="mt-4 grid grid-cols-3 gap-3">
    <div class="ti-state-card">
      <span class="ti-field-label">Watchdog</span>
      <strong class="mt-2 block text-[12px]">{watchdog?.state ?? "Not checked"}</strong>
      <p class="mb-0 mt-1 text-[11px] leading-5 text-[var(--ti-text-soft)]">{watchdog?.note ?? "Open Diagnostics again to refresh runtime health."}</p>
    </div>
    <div class="ti-state-card">
      <span class="ti-field-label">Devices</span>
      <strong class="mt-2 block text-[12px]">{devices?.state ?? "Not checked"}</strong>
      <p class="mb-0 mt-1 text-[11px] leading-5 text-[var(--ti-text-soft)]">{devices?.note ?? "Device-loss status has not been loaded."}</p>
    </div>
    <div class="ti-state-card">
      <span class="ti-field-label">Previous shutdown</span>
      <strong class="mt-2 block text-[12px]">{recovery?.previous_unclean_shutdown ? "Recovered" : recovery?.another_instance_detected ? "Another instance detected" : "Clean"}</strong>
      <p class="mb-0 mt-1 text-[11px] leading-5 text-[var(--ti-text-soft)]">{recovery?.note ?? "Startup recovery status has not been loaded."}</p>
    </div>
  </div>
</article>
