<script lang="ts">
  import { ChevronDown, HardDriveDownload, ShieldCheck, FolderInput } from "@lucide/svelte";
  import { myVoiceApi, type MyVoiceBackupReport } from "../../app/bridge/myVoiceApi";
  let {
    voiceChangesLocked,
    onNotice,
    onRestored,
  }: {
    voiceChangesLocked: boolean;
    onNotice: (message: string) => void;
    onRestored: () => void | Promise<void>;
  } = $props();

  let open = $state(false);
  let busy = $state(false);
  let actionUnknown = $state(false);
  let destination = $state("");
  let source = $state("");
  let authorized = $state(false);
  let lastReport = $state<MyVoiceBackupReport | null>(null);
  let lastAction = $state<"export" | "verify" | "restore" | null>(null);

  async function run(action: "export" | "verify" | "restore"): Promise<void> {
    if (busy || voiceChangesLocked) return;
    const folder = action === "export" ? destination.trim() : source.trim();
    if (!folder) {
      onNotice("Enter an existing folder path. You can copy the full path from File Explorer.");
      return;
    }
    if (action === "restore" && !authorized) {
      onNotice("Confirm that this backup contains your own or an authorized voice.");
      return;
    }
    busy = true;
    actionUnknown = false;
    lastReport = null;
    lastAction = action;
    try {
      const report = action === "export"
        ? await myVoiceApi.createBackup(folder)
        : action === "verify"
          ? await myVoiceApi.verifyBackup(folder)
          : await myVoiceApi.restoreBackup(folder, authorized);
      if (!report) {
        actionUnknown = true;
        onNotice("Couldn't verify the backup operation. Check the folder before retrying.");
        return;
      }
      lastReport = report;
      onNotice(report.message);
      if (report.ok && action === "export" && report.backup_folder) {
        source = report.backup_folder;
      }
      if (report.ok && action === "restore") {
        authorized = false;
        await onRestored();
      }
    } catch {
      actionUnknown = true;
      onNotice("Backup status is unknown. Check the folder and saved versions before retrying.");
    } finally {
      busy = false;
    }
  }
</script>

<section class="mt-4" aria-label="My Voice storage and backup">
  <button type="button"
    class="flex items-center gap-2 text-left text-sm font-medium text-[var(--ti-text-muted)] underline underline-offset-4"
    aria-expanded={open} onclick={() => { open = !open; }}>
    <ShieldCheck size={15} /> Storage &amp; backup <ChevronDown size={14} />
  </button>
  {#if open}
    <div class="ti-panel mt-3 space-y-4 p-4 sm:p-5">
      <p class="mb-0 text-sm leading-5 text-[var(--ti-text-muted)]">
        Your accepted recordings and voice versions are saved on this computer.
        Create a verified copy in another folder, preferably on an external drive.
        The application will not delete or replace earlier backups.
      </p>
      <p class="mb-0 text-xs leading-5 text-[var(--ti-text-muted)]">
        Folder selection currently uses a full path. Open File Explorer, navigate to an existing destination folder, and copy its path.
        The app creates a new backup folder inside that destination.
      </p>
      <div>
        <label class="block text-sm font-medium" for="myvoice-backup-destination">Backup destination (existing folder)</label>
        <input id="myvoice-backup-destination" class="ti-input mt-2 w-full" type="text"
          bind:value={destination} placeholder="D:\Voice Backups" disabled={busy} />
        <button type="button" class="ti-button ti-button-secondary mt-3"
          disabled={busy || voiceChangesLocked || !destination.trim()}
          onclick={() => void run("export")}>
          <HardDriveDownload size={15} /> {busy && lastAction === "export" ? "Creating..." : "Create verified backup"}
        </button>
      </div>
      <div class="border-t border-[var(--ti-border)] pt-4">
        <label class="block text-sm font-medium" for="myvoice-backup-source">Existing My Voice backup folder</label>
        <input id="myvoice-backup-source" class="ti-input mt-2 w-full" type="text"
          bind:value={source} placeholder="D:\Voice Backups\TranslateIT-MyVoice-..." disabled={busy} />
        <div class="mt-3 flex flex-wrap gap-2">
          <button type="button" class="ti-button ti-button-secondary"
            disabled={busy || voiceChangesLocked || !source.trim()}
            onclick={() => void run("verify")}>
            <ShieldCheck size={15} /> {busy && lastAction === "verify" ? "Checking..." : "Verify backup"}
          </button>
        </div>
        <label class="mt-4 flex items-start gap-3 text-sm leading-5 text-[var(--ti-text-muted)]">
          <input class="mt-0.5 size-4 accent-[var(--ti-accent)]" type="checkbox"
            bind:checked={authorized} disabled={busy}/>
          This backup contains my own or an authorized voice. I understand that restoring imports saved versions without changing my currently selected Meeting voice.
        </label>
        <button type="button" class="ti-button ti-button-secondary mt-3"
          disabled={busy || voiceChangesLocked || !authorized || !source.trim()}
          onclick={() => void run("restore")}>
          <FolderInput size={15} /> {busy && lastAction === "restore" ? "Restoring..." : "Import saved recordings and versions"}
        </button>
      </div>
      {#if actionUnknown}
        <p role="alert" class="text-sm text-[var(--ti-warning)]">
          Couldn't confirm the operation. No new backup or restored data should be assumed until the folder is checked.
        </p>
      {:else if lastReport?.ok}
        <div role="status" class="rounded-[var(--ti-radius-md)] border border-[var(--ti-border)] p-3">
          <p class="mb-1 text-sm font-medium">{lastReport.message}</p>
          <p class="mb-0 break-all text-xs text-[var(--ti-text-muted)]">
            {lastReport.file_count} verified files · {Math.round(lastReport.total_bytes / 1048576)} MB
            {#if lastAction === "restore"} · {lastReport.newly_imported} added, {lastReport.already_present} already present{/if}
          </p>
          {#if lastReport.backup_folder}
            <p class="mb-0 mt-2 break-all text-xs text-[var(--ti-text-muted)]">
              Folder: {lastReport.backup_folder}
            </p>
          {/if}
        </div>
      {/if}
      <p class="mb-0 text-xs leading-5 text-[var(--ti-text-soft)]">
        Backup folders are not encrypted. Store them on a drive you control.
        File checks verify integrity, not the authenticity of an unknown source.
        Only restore backups you trust. A backup on the same drive does not protect against drive failure.
      </p>
    </div>
  {/if}
</section>
