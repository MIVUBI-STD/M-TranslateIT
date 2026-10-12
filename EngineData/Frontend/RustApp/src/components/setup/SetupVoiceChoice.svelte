<script lang="ts">
  import StatusRow from "../ui/StatusRow.svelte";
  import type { ProductRuntimeSnapshot } from "../../app/bridge/runtimeProductFacade";
  import type { VirtualMicRouteCheck } from "../../app/bridge/virtualMicRoutePresentation";
  type BuiltinVoiceId = "MaleVoice" | "FemaleVoice";
  let {
    snapshot, meetingVoiceReady, routeCheck, busy, builtinPendingId,
    microphone, meetingMicrophone, meetingSound,
    onSelect, onConfirm, onCancel, onOpenMyVoice,
  }: {
    snapshot: ProductRuntimeSnapshot | null;
    meetingVoiceReady: boolean | null;
    routeCheck: VirtualMicRouteCheck;
    busy: boolean;
    builtinPendingId: BuiltinVoiceId | null;
    microphone: string;
    meetingMicrophone: string;
    meetingSound: string;
    onSelect: (voiceId: BuiltinVoiceId) => void | Promise<void>;
    onConfirm: () => void | Promise<void>;
    onCancel: () => void;
    onOpenMyVoice: () => void | Promise<void>;
  } = $props();
</script>

<div>
          <span class="ti-kicker">Voice others hear</span>
          <h1 class="ti-page-title">{meetingVoiceReady && snapshot?.readiness.meetingReady && routeCheck === "ready" ? "TranslateIT is ready for a call." : "Choose the English voice others will hear."}</h1>
          <p class="ti-page-copy">Choose a built-in voice to start quickly, or make a personalized voice later. In your calling app, select the TranslateIT microphone before speaking.</p>
        </div>

        <div class="grid grid-cols-2 gap-3">
          <button type="button" class="ti-subtle-card p-5 text-left" disabled={busy} onclick={() => void onSelect("MaleVoice")}>
            <span class="ti-field-label">Ready now</span>
            <strong class="mt-2 block text-sm font-semibold">Built-in Male</strong>
            <span class="mt-1 block text-xs leading-5 text-[var(--ti-text-soft)]">Use this voice in meetings.</span>
          </button>
          <button type="button" class="ti-subtle-card p-5 text-left" disabled={busy} onclick={() => void onSelect("FemaleVoice")}>
            <span class="ti-field-label">Ready now</span>
            <strong class="mt-2 block text-sm font-semibold">Built-in Female</strong>
            <span class="mt-1 block text-xs leading-5 text-[var(--ti-text-soft)]">Use this voice in meetings.</span>
          </button>
        </div>

        {#if builtinPendingId}
          <div class="rounded-[var(--ti-radius-md)] border border-[var(--ti-warning-border)] bg-[var(--ti-warning-surface)] p-4">
            <strong class="text-sm font-semibold">Replace the current Voice others hear?</strong>
            <p class="mb-0 mt-1 text-sm leading-5 text-[var(--ti-text-muted)]">{(builtinPendingId === "MaleVoice" ? "Built-in Male" : "Built-in Female")} will replace the voice currently selected for Meeting.</p>
            <div class="ti-action-row mt-4">
              <button type="button" class="ti-button ti-button-secondary" disabled={busy} onclick={onCancel}>Cancel</button>
              <button type="button" class="ti-button" disabled={busy} onclick={() => void onConfirm()}>Replace Voice</button>
            </div>
          </div>
        {/if}

        <div class="ti-subtle-card divide-y divide-[var(--ti-border)] overflow-hidden">
          <StatusRow label="Microphone" value={microphone} status={snapshot?.readiness.microphoneReady ? "Ready" : "Setup Needed"} tone={snapshot?.readiness.microphoneReady ? "good" : "warning"} />
          <StatusRow label="Voice others hear" value={meetingVoiceReady ? "A Meeting voice is ready" : meetingVoiceReady === null ? "Checking the Meeting voice" : "Choose a built-in voice or create My Voice"} status={meetingVoiceReady ? "Ready" : meetingVoiceReady === null ? "Checking" : "Setup Needed"} tone={meetingVoiceReady ? "good" : meetingVoiceReady === null ? "neutral" : "warning"} />
          <StatusRow label="TranslateIT microphone" value={meetingMicrophone}
            detail={routeCheck === "unavailable" ? "Couldn't verify the Windows device. Select Check Again." : "Windows availability does not confirm selection inside your calling app."}
            status={routeCheck === "ready" ? "Available" : routeCheck === "unavailable" ? "Unable to check" : routeCheck === "checking" ? "Checking" : "Setup Needed"}
            tone={routeCheck === "ready" ? "good" : routeCheck === "setup_needed" ? "warning" : "neutral"} />
          <StatusRow label="Translate what you hear" value={`English → Indonesian text · ${meetingSound}`} detail="Optional. You can turn this on or change it later." status="Optional" tone="neutral" />
        </div>

        <button type="button" class="ti-button ti-button-secondary justify-self-start" disabled={busy} onclick={() => void onOpenMyVoice()}>Create my own voice instead</button>
        {#if snapshot && (!snapshot.readiness.meetingReady || routeCheck !== "ready")}
          <div class="rounded-[var(--ti-radius-md)] border border-[var(--ti-warning-border)] bg-[var(--ti-warning-surface)] p-4" role="status">
            <strong class="block text-sm font-semibold">What still needs attention</strong>
            <p class="mb-0 mt-1 text-sm leading-6 text-[var(--ti-text-muted)]">
              {routeCheck === "unavailable"
                ? "Couldn't check the TranslateIT microphone in Windows. Go back to the Meeting app step and select Check Again."
                : snapshot.readiness.level === "checking" || routeCheck === "checking"
                  ? "The local translation check is still running. Select Check Again."
                : !snapshot.readiness.microphoneReady
                  ? "Your input microphone is not ready. Go back to the Microphone step to select or check it."
                  : !snapshot.readiness.meetingRouteReady
                    ? "The TranslateIT microphone is not available in Windows. Go back to the Meeting app step to check it."
                    : meetingVoiceReady === false
                      ? "Choose a built-in Meeting voice above or create My Voice."
                      : "The local translation check is not ready. Select Check Again, then Fix Setup if needed."}
            </p>
          </div>
        {/if}
