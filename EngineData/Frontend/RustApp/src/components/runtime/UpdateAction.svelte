<script lang="ts">
  import { onMount } from "svelte";
  import { appUpdateApi, type AppUpdateCheck } from "../../app/update/appUpdateApi";

  let {
    meetingBusy,
    voiceBuildActive,
    myVoiceRecording,
    onNotice,
  }: {
    meetingBusy: boolean;
    voiceBuildActive: boolean;
    myVoiceRecording: boolean;
    onNotice: (message: string) => void;
  } = $props();

  let update = $state<AppUpdateCheck | null>(null);
  let busy = $state(false);

  onMount(() => {
    void (async () => {
      try {
        const result = await appUpdateApi.checkAtStartupOnce();
        if (result?.available) {
          update = result;
          onNotice(`TranslateIT ${result.version ?? "update"} is available.`);
        }
      } catch {
        // Update availability is optional and must never block app startup.
      }
    })();
  });

  async function install(): Promise<void> {
    if (busy) return;
    if (meetingBusy) {
      onNotice("Stop Translation or Mic Test before updating TranslateIT.");
      return;
    }
    if (myVoiceRecording) {
      onNotice("Stop the current My Voice recording before updating TranslateIT.");
      return;
    }
    if (voiceBuildActive) {
      onNotice("Wait for My Voice creation to finish before updating TranslateIT.");
      return;
    }

    busy = true;
    onNotice("Preparing TranslateIT update...");
    try {
      const result = await appUpdateApi.install();
      if (!result) {
        onNotice("The update couldn't be started. Try again later.");
        return;
      }
      onNotice(result.message);
      if (result.installed) update = null;
    } finally {
      busy = false;
    }
  }
</script>

{#if update?.available}
  <button
    type="button"
    class="ti-button ti-button-secondary min-h-8 shrink-0 px-3 text-[11.5px]"
    disabled={busy || meetingBusy || myVoiceRecording || voiceBuildActive}
    title={meetingBusy
      ? "Stop Translation or Mic Test before updating."
      : myVoiceRecording
        ? "Stop the current My Voice recording before updating."
        : voiceBuildActive
          ? "Wait for My Voice creation to finish before updating."
          : update.notes ?? "A TranslateIT update is ready."}
    onclick={() => void install()}
  >{busy ? "Updating..." : `Update ${update.version ?? ""}`}</button>
{/if}
