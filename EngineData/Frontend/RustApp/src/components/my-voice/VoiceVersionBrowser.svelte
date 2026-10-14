<script lang="ts">
  import { Check, ChevronDown, History, RotateCcw } from "@lucide/svelte";
  import { onMount } from "svelte";
  import { myVoiceBuildApi, type ActorVersionPage } from "../../app/bridge/myVoiceBuildApi";

  let {
    voiceChangesLocked,
    onNotice,
    onVoiceActivated,
    onRecheck,
  }: {
    voiceChangesLocked: boolean;
    onNotice: (message: string) => void;
    onVoiceActivated: (message?: string) => void | Promise<void>;
    onRecheck: () => Promise<boolean>;
  } = $props();

  let opened = $state(false);
  let busy = $state(false);
  let error = $state(false);
  let selectionUnknown = $state(false);
  let versions = $state<ActorVersionPage | null>(null);
  let pendingId = $state<string | null>(null);
  let disposed = false;

  async function load(more = false): Promise<void> {
    if (busy || (more && !versions?.next_cursor)) return;
    busy = true;
    try {
      const next = await myVoiceBuildApi.listActorVersions(
        more ? versions?.next_cursor ?? null : null,
      );
      if (disposed) return;
      if (!next) {
        error = true;
        pendingId = null;
        return;
      }
      versions = more && versions
        ? { ...next, items: [...versions.items, ...next.items] }
        : next;
      error = false;
      if (pendingId && !versions.items.some((item) => item.version_id === pendingId)) {
        pendingId = null;
      }
    } catch {
      if (!disposed) {
        error = true;
        pendingId = null;
      }
    } finally {
      busy = false;
    }
  }

  function toggle(): void {
    // An unfinished read/mutation must settle before hiding its result.
    if (busy) return;
    opened = !opened;
    if (opened) void load();
    else pendingId = null;
  }

  async function retrySelectionStatus(): Promise<void> {
    if (busy) return;
    busy = true;
    try {
      const verified = await onRecheck();
      if (disposed) return;
      selectionUnknown = !verified;
      if (!verified) onNotice("Couldn't verify the current Meeting voice. Voice selection remains unavailable.");
    } catch {
      if (!disposed) selectionUnknown = true;
    } finally {
      busy = false;
    }
  }

  async function reconcileUncertainSelection(message: string): Promise<void> {
    if (disposed) return;
    pendingId = null;
    selectionUnknown = true;
    // Refresh current Meeting voice without replaying a potentially
    // completed restore. Its identity cannot be inferred from kind alone.
    try {
      await onRecheck();
    } catch {
      // Keep the unknown lock for an explicit status retry.
    }
    if (!disposed) onNotice(message);
  }

  async function selectVersion(versionId: string, confirmed: boolean): Promise<void> {
    if (busy || error || selectionUnknown || voiceChangesLocked || !opened) return;
    if (!versions?.items.some((entry) => entry.version_id === versionId)) return;
    busy = true;
    try {
      const result = await myVoiceBuildApi.selectActorVersion(versionId, confirmed);
      if (disposed) return;
      if (result.state === "frontend_bridge_error" || result.build.phase === "unavailable") {
        await reconcileUncertainSelection(
          "Couldn't verify the selected voice. Check the voice shown above before changing it again.",
        );
        return;
      }
      if (result.state === "approval_required") {
        if (opened) pendingId = versionId;
        return;
      }
      if (!result.ok) {
        pendingId = null;
        onNotice(result.message);
        return;
      }
      pendingId = null;
      await onVoiceActivated(result.message);
      if (disposed) return;
      busy = false;
      await load();
    } catch {
      await reconcileUncertainSelection(
        "Couldn't confirm whether the voice changed. Check the current Meeting voice.",
      );
    } finally {
      busy = false;
    }
  }

  onMount(() => {
    disposed = false;
    return () => { disposed = true; };
  });
</script>

<section class="mt-4" aria-label="Saved voice versions">
  <button type="button"
    class="flex items-center gap-2 text-left text-sm font-medium text-[var(--ti-text-muted)] underline underline-offset-4"
    aria-expanded={opened} disabled={busy} onclick={toggle}>
    <History size={15} /><span>Voice versions</span><ChevronDown size={14} />
  </button>

  {#if opened}
    <div class="ti-panel mt-3 p-4 sm:p-5">
      <p class="mb-3 text-sm leading-5 text-[var(--ti-text-muted)]">
        Previous voice versions are stored on this computer. Full model verification happens before selection. Your existing voice is preserved.
      </p>

      {#if busy}
        <p role="status" class="mb-3 text-sm text-[var(--ti-text-muted)]">
          Checking saved voice files or switching the selected version...
        </p>
      {/if}
      {#if selectionUnknown}
        <p role="alert" class="text-sm text-[var(--ti-warning)]">
          Selection status is unknown. Check the current voice above before trying another change.
        </p>
        <button type="button" class="ti-button ti-button-secondary mb-3"
          disabled={busy} onclick={() => void retrySelectionStatus()}>Retry voice status</button>
      {/if}

      {#if error}
        <p role="alert" class="text-sm text-[var(--ti-warning)]">
          Couldn't read the version library. No voice files were changed.
        </p>
        <button type="button" class="ti-button ti-button-secondary"
          disabled={busy} onclick={() => void load()}>Retry versions</button>
      {:else if busy && !versions}
        <p role="status" class="text-sm text-[var(--ti-text-muted)]">Checking saved versions...</p>
      {:else if versions}
        {#if versions.attention_entries > 0}
          <p role="alert" class="mb-3 text-sm text-[var(--ti-warning)]">
            {versions.attention_entries} version entries need attention and were not listed as available.
          </p>
        {/if}
        {#if versions.items.length === 0}
          <p class="text-sm text-[var(--ti-text-muted)]">No saved personal voice versions found yet.</p>
        {:else}
          <div class="space-y-2">
            {#each versions.items as voice (voice.version_id)}
              <div class="flex flex-wrap items-center gap-3 rounded-[var(--ti-radius-md)] border border-[var(--ti-border)] p-3">
                <div class="min-w-0 flex-1">
                  <p class="m-0 text-sm font-semibold">Saved voice · {voice.version_id.slice(0, 10)}</p>
                  <p class="mb-0 mt-1 text-xs text-[var(--ti-text-muted)]">
                    Full model integrity will be verified before use.
                  </p>
                </div>
                <button type="button" class="ti-button ti-button-secondary"
                  disabled={busy || error || voiceChangesLocked || selectionUnknown}
                  onclick={() => void selectVersion(voice.version_id, false)}>
                  <RotateCcw size={14} /> Use version
                </button>
              </div>
            {/each}
          </div>
        {/if}
        {#if versions.next_cursor}
          <button type="button" class="ti-button ti-button-secondary mt-3"
            disabled={busy} onclick={() => void load(true)}>
            {busy ? "Loading..." : "Show more versions"}
          </button>
        {/if}
      {/if}

      {#if pendingId}
        <div class="mt-4 rounded-[var(--ti-radius-md)] border border-[var(--ti-warning-border)] p-4"
          role="group" aria-label="Confirm saved voice version selection">
          <strong class="text-sm font-semibold">Use this saved voice for meetings?</strong>
          <p class="mb-0 mt-1 text-sm leading-5 text-[var(--ti-text-muted)]">
            The current Meeting voice will change only after this version passes integrity checks. Your saved versions will remain available.
          </p>
          <div class="mt-3 flex flex-wrap gap-2">
            <button type="button" class="ti-button ti-button-secondary" disabled={busy}
              onclick={() => { pendingId = null; }}>Cancel</button>
            <button type="button" class="ti-button"
              disabled={busy || error || voiceChangesLocked || selectionUnknown}
              onclick={() => { if (pendingId) void selectVersion(pendingId, true); }}>
              <Check size={14} /> Confirm and use
            </button>
          </div>
        </div>
      {/if}
    </div>
  {/if}
</section>
