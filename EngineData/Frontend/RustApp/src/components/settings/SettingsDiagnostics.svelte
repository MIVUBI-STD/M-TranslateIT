<script lang="ts">
  import { ArrowLeft, Bug, RefreshCw } from "@lucide/svelte";
  import { runtimeApi } from "../../app/bridge/runtimeApi";
  import {
    parseWorkerCapabilities,
    type ProductRuntimeSnapshot,
    type ProductSetupAction,
  } from "../../app/bridge/runtimeProductFacade";
  import { sanitizeDiagnosticText } from "../../app/shared/diagnosticPrivacy";
  import MeetingPerformanceDiagnostics from "./MeetingPerformanceDiagnostics.svelte";
  import RuntimeReliabilityDiagnostics from "./RuntimeReliabilityDiagnostics.svelte";

  let {
    snapshot,
    setupBusy = false,
    diagnosticsLoading = false,
    onRefreshDiagnostics,
    onSetupAction,
    onBack,
  }: {
    snapshot: ProductRuntimeSnapshot;
    setupBusy?: boolean;
    diagnosticsLoading?: boolean;
    onRefreshDiagnostics: () => void | Promise<void>;
    onSetupAction: (action: ProductSetupAction) => void | Promise<void>;
    onBack: () => void;
  } = $props();

  const workerDiagnostics = $derived(parseWorkerCapabilities(snapshot.workerStatus));
  const helperDiagnosticMessage = $derived(
    sanitizeDiagnosticText(snapshot.helper?.message, "Refresh status to check the local worker."),
  );
  const commandErrors = $derived(runtimeApi.getCommandErrors());
</script>

<section class="grid gap-4">
  <header class="ti-page-header">
    <div>
      <h3 class="m-0 text-xl font-semibold tracking-[-0.02em]">Diagnostics</h3>
      <p class="mb-0 mt-1.5 text-[12.5px] text-[var(--ti-text-muted)]">Detailed technical information for troubleshooting.</p>
    </div>
    <button type="button" class="ti-button ti-button-secondary" onclick={onBack}><ArrowLeft size={15} /> Back</button>
  </header>

  <article class="ti-panel p-5">
    <div class="grid grid-cols-[repeat(3,minmax(0,1fr))] gap-3">
      <div class="ti-state-card"><span class="ti-field-label">Worker</span><strong class="mt-2 block text-[13px]">{snapshot.helper?.state ?? "Not checked"}</strong></div>
      <div class="ti-state-card"><span class="ti-field-label">Outbound provider</span><strong class="mt-2 block text-[13px]">{snapshot.helper?.provider_ready ? "Available" : "Setup Needed"}</strong></div>
      <div class="ti-state-card"><span class="ti-field-label">Execution device</span><strong class="mt-2 block text-[13px]">{snapshot.helper?.cuda_ready ? "CUDA" : snapshot.helper?.degraded_mode ? "CPU / degraded" : "Not verified"}</strong></div>
    </div>

    <p class="mb-0 mt-4 text-[12px] leading-5 text-[var(--ti-text-muted)]">{helperDiagnosticMessage}</p>

    <div class="ti-action-row mt-4">
      <button type="button" class="ti-button ti-button-secondary" disabled={setupBusy || diagnosticsLoading} onclick={() => void onRefreshDiagnostics()}><RefreshCw size={15} /> {diagnosticsLoading || setupBusy ? "Refreshing..." : "Check Again"}</button>
      <button type="button" class="ti-button ti-button-secondary" disabled={setupBusy || diagnosticsLoading} onclick={() => void onSetupAction("verify-models")}><Bug size={15} /> Check AI Files</button>
    </div>
  </article>

  <article class="ti-panel p-5">
    <div>
      <span class="ti-field-label">Technical components</span>
      <h4 class="mb-0 mt-1.5 text-[14px] font-semibold">Local AI status</h4>
    </div>
    <div class="mt-4 grid grid-cols-[repeat(3,minmax(0,1fr))] gap-3">
      <div class="ti-state-card">
        <span class="ti-field-label">ASR</span>
        <strong class="mt-2 block break-words text-[12px] leading-5">{workerDiagnostics.asrDisplay}</strong>
      </div>
      <div class="ti-state-card">
        <span class="ti-field-label">Translation</span>
        <strong class="mt-2 block break-words text-[12px] leading-5">{workerDiagnostics.translationDisplay}</strong>
      </div>
      <div class="ti-state-card">
        <span class="ti-field-label">Meeting voice</span>
        <strong class="mt-2 block break-words text-[12px] leading-5">{workerDiagnostics.voiceDisplay}</strong>
      </div>
    </div>
    <p class="mb-0 mt-3 text-[11.5px] leading-5 text-[var(--ti-text-soft)]">Processing mode: {workerDiagnostics.executionDisplay}. This information is only needed for troubleshooting.</p>
  </article>

  <RuntimeReliabilityDiagnostics />
  <MeetingPerformanceDiagnostics status={snapshot.meetingSession} />

  <article class="ti-panel p-5">
    <div class="flex items-end justify-between gap-5">
      <div>
        <span class="ti-field-label">Troubleshooting</span>
        <h4 class="mb-0 mt-1.5 text-[14px] font-semibold">Recent technical errors</h4>
      </div>
      <span class="ti-pill">{commandErrors.length} recent</span>
    </div>
    <div class="mt-4 grid gap-2">
      {#if commandErrors.length === 0}
        <p class="m-0 text-[12px] text-[var(--ti-text-muted)]">No recent technical errors.</p>
      {:else}
        {#each commandErrors as error (`${error.occurred_at}-${error.command}`)}
          <div class="ti-subtle-card px-4 py-3">
            <strong class="text-[11px]">{error.command}</strong>
            <p class="mb-0 mt-1 text-[11px] leading-5 text-[var(--ti-text-muted)]">{error.message}</p>
          </div>
        {/each}
      {/if}
    </div>
  </article>
</section>
