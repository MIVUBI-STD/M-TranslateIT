<script lang="ts">
  import { Download } from "@lucide/svelte";
  import { runtimeApi, type MeetingTranscriptExportStatus } from "../../app/bridge/runtimeApi";

  let {
    hasSession = false,
    onNotice,
  }: {
    hasSession?: boolean;
    onNotice: (message: string) => void;
  } = $props();

  let status = $state<MeetingTranscriptExportStatus | null>(null);
  let format = $state<"md" | "txt">("md");
  let busy = $state(false);
  let lastSessionState = $state<boolean | null>(null);
  let receipt = $state<{ path: string; truncated: boolean } | null>(null);
  let feedback = $state("");
  let feedbackError = $state(false);
  let copying = $state(false);

  async function copyExportLocation(): Promise<void> {
    if (!receipt?.path || copying) return;
    copying = true;
    try {
      if (!navigator.clipboard?.writeText) throw new Error("Clipboard is unavailable");
      await navigator.clipboard.writeText(receipt.path);
      feedback = "File location copied.";
      feedbackError = false;
    } catch {
      feedback = "Couldn't copy the location. The exported transcript is still saved.";
      feedbackError = true;
    } finally {
      copying = false;
    }
  }

  async function refreshStatus(): Promise<void> {
    try {
      status = await runtimeApi.getMeetingTranscriptExportStatus();
    } catch {
      status = null;
    }
  }

  async function exportTranscript(): Promise<void> {
    if (busy) return;
    busy = true;
    receipt = null;
    feedback = "";
    feedbackError = false;
    try {
      const result = await runtimeApi.exportMeetingTranscript(format);
      if (!result.ok) {
        feedback = result.message;
        feedbackError = true;
        onNotice("Transcript export failed.");
        return;
      }
      receipt = result.file_path ? { path: result.file_path, truncated: result.truncated } : null;
      feedback = result.truncated
        ? "Exported available phrases only. Earlier phrases were not retained."
        : `Saved ${result.turn_count} phrases as ${result.format.toUpperCase()}.`;
      feedbackError = false;
      onNotice("Transcript exported.");
      await refreshStatus();
    } catch {
      feedback = "Transcript export is unavailable right now.";
      feedbackError = true;
      onNotice("Transcript export failed.");
    } finally {
      busy = false;
    }
  }

  $effect(() => {
    if (lastSessionState !== hasSession) {
      lastSessionState = hasSession;
      receipt = null;
      feedback = "";
      void refreshStatus();
    }
  });
</script>

<div class="grid gap-2">
  <div class="flex flex-wrap items-center gap-2">
  <select
    class="ti-field min-h-9 w-[118px] px-2.5 text-xs"
    bind:value={format}
    aria-label="Transcript export format"
    disabled={busy}
  >
    <option value="md">Markdown</option>
    <option value="txt">TXT</option>
  </select>

  <button
    type="button"
    class="ti-button ti-button-secondary min-h-9"
    disabled={busy || (!hasSession && status?.available !== true)}
    onclick={() => void exportTranscript()}
    title={status?.truncated
      ? "Earlier transcript turns are no longer retained; the exported file will include a warning."
      : "Export the active or most recently ended Meeting transcript."}
  >
    <Download size={15} />
    {busy ? "Exporting..." : "Export transcript"}
  </button>
  {#if receipt}
    <button type="button" class="ti-button ti-button-secondary min-h-9"
      disabled={copying} onclick={() => void copyExportLocation()}>
      {copying ? "Copying..." : "Copy location"}
    </button>
  {/if}
  </div>
  {#if status?.truncated && !receipt}
    <p class="m-0 text-xs text-[var(--ti-warning)]" role="status">
      Earlier phrases are no longer retained. Export will contain available phrases only.
    </p>
  {/if}
  {#if feedback}
    <p class={"m-0 text-xs " + (feedbackError ? "text-[var(--ti-danger)]" :
      receipt?.truncated ? "text-[var(--ti-warning)]" : "text-[var(--ti-text-muted)]")}
      role="status" aria-live="polite">{feedback}</p>
  {/if}
</div>
