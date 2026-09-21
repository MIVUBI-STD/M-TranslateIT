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
    try {
      const result = await runtimeApi.exportMeetingTranscript(format);
      if (!result.ok) {
        onNotice(result.message);
        return;
      }
      onNotice(result.file_path ? `${result.message} Saved to ${result.file_path}` : result.message);
      await refreshStatus();
    } catch {
      onNotice("Transcript export is unavailable right now.");
    } finally {
      busy = false;
    }
  }

  $effect(() => {
    if (lastSessionState !== hasSession) {
      lastSessionState = hasSession;
      void refreshStatus();
    }
  });
</script>

<div class="flex items-center gap-2">
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
</div>
