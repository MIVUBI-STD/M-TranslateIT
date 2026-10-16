<script lang="ts">
  import { Check, Copy, Download, ExternalLink, FolderOpen, RefreshCw, Trash2 } from "@lucide/svelte";
  import { runtimeApi, type MeetingCommittedTurn, type MeetingCommittedTurnsSnapshot, type SavedMeetingSessions } from "../../app/bridge/runtimeApi";
  import { languageName } from "../../app/shared/state";
  import { meetingTime } from "../../app/runtime/meetingTranscriptPresentation";

  let { hasSession = false, onNotice, onDeleted }: {
    hasSession?: boolean;
    onNotice: (message: string) => void;
    onDeleted?: () => void;
  } = $props();
  let history = $state<SavedMeetingSessions | null>(null);
  let expanded = $state<string | null>(null);
  let preview = $state<MeetingCommittedTurnsSnapshot | null>(null);
  let language = $state<"bilingual" | "id" | "en">("bilingual");
  let confirming = $state<string | null>(null);
  let busy = $state(false);
  let loading = $state(false);
  let notice = $state("");
  let htmlSample = $state<{ sessionId: string; html: string; entryCount: number } | null>(null);
  let previewBusy = $state(false);
  let htmlPreviewRevision = 0;
  let receipt = $state<{ sessionId: string; path: string; count: number; incomplete: boolean } | null>(null);
  let previousActive = $state<boolean | null>(null);

  function sessionLabel(id: string): string {
    const ms = Number(id.split("_")[1]);
    const date = new Date(ms);
    return Number.isSafeInteger(ms) && ms > 0 && !Number.isNaN(date.getTime())
      ? date.toLocaleString(undefined, { dateStyle: "medium", timeStyle: "short" })
      : "Saved Meeting";
  }
  function textFor(turn: MeetingCommittedTurn, code: "id" | "en") {
    if (turn.source_language === code) return { text: turn.source_text, label: "Original", fallback: false };
    if (turn.translation_state === "translated" && turn.translated_text.trim()) {
      return { text: turn.translated_text, label: "Translation", fallback: false };
    }
    return { text: turn.source_text, label: "Translation unavailable · Original fallback", fallback: true };
  }
  async function refresh(): Promise<void> {
    if (loading) return;
    loading = true;
    try {
      history = await runtimeApi.listSavedMeetingTranscripts();
      notice = history.ok ? "" : history.message;
    } catch {
      notice = "Saved transcripts are unavailable.";
    } finally { loading = false; }
  }
  async function togglePreview(sessionId: string): Promise<void> {
    confirming = null;
    notice = "";
    if (expanded === sessionId) { expanded = null; preview = null; return; }
    expanded = sessionId;
    preview = null;
    const result = await runtimeApi.getSavedMeetingTranscriptPreview(sessionId);
    if (expanded !== sessionId) return;
    if (!result.ok) notice = result.note || "Saved transcript could not be loaded.";
    else preview = result;
  }
  async function toggleHtmlPreview(sessionId: string): Promise<void> {
    if (previewBusy) return;
    if (htmlSample?.sessionId === sessionId) {
      htmlSample = null;
      return;
    }
    previewBusy = true;
    const revision = ++htmlPreviewRevision;
    notice = "";
    try {
      const result = await runtimeApi.previewSavedMeetingTranscriptHtml(sessionId);
      if (revision !== htmlPreviewRevision) return;
      if (!result.ok) {
        htmlSample = null;
        notice = result.message || "HTML preview is unavailable.";
        return;
      }
      htmlSample = { sessionId, html: result.html, entryCount: result.entry_count };
    } catch {
      if (revision === htmlPreviewRevision) {
        htmlSample = null;
        notice = "HTML preview could not be rendered.";
      }
    } finally { previewBusy = false; }
  }

  async function exportHtml(sessionId: string): Promise<void> {
    if (busy) return;
    busy = true; notice = ""; receipt = null; confirming = null;
    try {
      const result = await runtimeApi.exportSavedMeetingTranscriptHtml(sessionId);
      if (!result.ok || !result.file_path) { notice = result.message; return; }
      receipt = { sessionId, path: result.file_path, count: result.turn_count, incomplete: result.incomplete };
      onNotice("HTML transcript exported.");
      await refresh();
    } catch { notice = "HTML export could not be completed."; }
    finally { busy = false; }
  }
  async function deleteArchive(sessionId: string): Promise<void> {
    if (busy || confirming !== sessionId) return;
    busy = true; notice = "";
    try {
      const result = await runtimeApi.deleteSavedMeetingTranscript(sessionId);
      if (!result.ok) { notice = result.message; return; }
      if (expanded === sessionId) { expanded = null; preview = null; }
      if (htmlSample?.sessionId === sessionId) htmlSample = null;
      htmlPreviewRevision += 1;
      if (receipt?.sessionId === sessionId) receipt = null;
      confirming = null;
      onDeleted?.();
      onNotice("Saved transcript deleted.");
      await refresh();
    } catch { notice = "Saved transcript could not be deleted."; }
    finally { busy = false; }
  }
  async function openExport(folder: boolean): Promise<void> {
    if (!receipt) return;
    const fileName = receipt.path.split("/").pop();
    if (!fileName) return;
    const result = await runtimeApi.openSavedMeetingHtml(receipt.sessionId, fileName, folder);
    if (!result.ok) notice = result.message;
  }
  async function copyPath(): Promise<void> {
    if (!receipt) return;
    try {
      await navigator.clipboard.writeText(receipt.path);
      onNotice("Transcript file location copied.");
    } catch { notice = "Could not copy the file location."; }
  }
  $effect(() => {
    if (previousActive !== hasSession) {
      previousActive = hasSession;
      void refresh();
    }
  });
</script>

<section class="ti-panel mt-4 overflow-hidden" aria-label="Saved Meeting transcripts">
  <header class="flex flex-wrap items-start justify-between gap-3 px-5 py-4">
    <div class="grid gap-1">
      <h3 class="m-0 text-[15px] font-semibold">Saved Transcripts</h3>
      <p class="m-0 max-w-xl text-xs leading-5 text-[var(--ti-text-muted)]">
        Conversations stay on this device until you delete them. Exporting HTML does not delete saved entries.
      </p>
    </div>
    <button type="button" class="ti-button ti-button-secondary min-h-9 text-xs" disabled={loading || busy}
      onclick={() => void refresh()}><RefreshCw size={14}/>{loading ? "Checking..." : "Refresh"}</button>
  </header>
  {#if notice}
    <p class="mx-5 mb-3 text-xs text-[var(--ti-warning)]" role="status">{notice}</p>
  {/if}
  {#if history && history.active_write_errors > 0}
    <p class="mx-5 mb-3 text-xs text-[var(--ti-warning)]" role="alert">
      Some transcript entries could not be saved; storage completeness is unverified.
    </p>
  {/if}
  {#if history?.ok && history.sessions.length === 0}
    <p class="m-0 border-t border-[var(--ti-border)] px-5 py-6 text-sm text-[var(--ti-text-muted)]">
      No conversation entries have been saved yet.
    </p>
  {:else if !history}
    <p class="m-0 border-t border-[var(--ti-border)] px-5 py-5 text-xs text-[var(--ti-text-muted)]">
      Saved transcript list is not available.
    </p>
  {:else}
    <div class="divide-y divide-[var(--ti-border)] border-t border-[var(--ti-border)]">
      {#each history.sessions as session (session.session_id)}
        <div class="grid gap-3 px-5 py-4">
          <div class="flex flex-wrap items-center justify-between gap-3">
            <div class="min-w-0">
              <strong class="block text-[13px] font-semibold">{sessionLabel(session.session_id)}</strong>
              <p class="m-0 mt-1 text-xs text-[var(--ti-text-muted)]">
                {session.entry_count} saved entries · {session.complete ? "Session ended" : "Storage finalization unverified"}
              </p>
              {#if session.storage_warning || !session.complete}
                <p class="m-0 mt-1 text-xs text-[var(--ti-warning)]">Check this transcript for missing entries.</p>
              {/if}
            </div>
            <div class="flex flex-wrap gap-2">
              <button type="button" class="ti-button ti-button-secondary min-h-9 text-xs"
                aria-expanded={expanded === session.session_id}
                onclick={() => void togglePreview(session.session_id)}>
                {expanded === session.session_id ? "Hide" : "Read"}
              </button>
              <button type="button" class="ti-button ti-button-secondary min-h-9 text-xs"
                disabled={busy || previewBusy} aria-expanded={htmlSample?.sessionId === session.session_id}
                onclick={() => void toggleHtmlPreview(session.session_id)}>
                {htmlSample?.sessionId === session.session_id ? "Hide HTML Preview" : "Preview HTML"}
              </button>
              <button type="button" class="ti-button ti-button-secondary min-h-9 text-xs" disabled={busy}
                onclick={() => void exportHtml(session.session_id)}><Download size={14}/>Export HTML</button>
              <button type="button" class="ti-button ti-button-secondary min-h-9 text-xs" disabled={busy}
                onclick={() => { confirming = session.session_id; notice = ""; }}><Trash2 size={14}/>Delete</button>
            </div>
          </div>
          {#if confirming === session.session_id}
            <div class="rounded-[var(--ti-radius-md)] border border-[var(--ti-danger-border)] bg-[var(--ti-danger-surface)] p-3">
              <p class="m-0 text-sm text-[var(--ti-danger)]">
                Delete this saved conversation permanently? Existing exported HTML files will remain.
              </p>
              <div class="mt-3 flex flex-wrap gap-2">
                <button type="button" class="ti-button ti-button-danger min-h-9"
                  onclick={() => void deleteArchive(session.session_id)} disabled={busy}>Delete saved transcript</button>
                <button type="button" class="ti-button ti-button-secondary min-h-9"
                  onclick={() => confirming = null}>Cancel</button>
              </div>
            </div>
          {/if}
          {#if htmlSample?.sessionId === session.session_id}
            <div class="grid gap-2 rounded-[var(--ti-radius-md)] border border-[var(--ti-border)] p-3">
              <strong class="text-sm">HTML document preview</strong>
              <p class="m-0 text-xs text-[var(--ti-text-muted)]">
                Sample of the latest {htmlSample.entryCount} saved entries, rendered with the HTML export template.
                The final file includes all archived entries. Switch reading language inside the preview.
              </p>
              <iframe title="Offline HTML transcript document preview" sandbox=""
                srcdoc={htmlSample.html} class="h-[480px] w-full rounded-[var(--ti-radius-md)] border border-[var(--ti-border)] bg-[var(--ti-bg)]"></iframe>
            </div>
          {/if}
          {#if expanded === session.session_id}
            <div class="rounded-[var(--ti-radius-md)] border border-[var(--ti-border)] bg-[var(--ti-surface-soft)] p-3">
              <div class="mb-3 flex flex-wrap items-center justify-between gap-3">
                <div>
                  <strong class="text-sm">Conversation</strong>
                  <p class="m-0 text-xs text-[var(--ti-text-muted)]">
                    Latest {preview?.turns.length ?? 0} entries · HTML includes all archived entries.
                  </p>
                </div>
                <div class="flex flex-wrap gap-1 rounded-[var(--ti-radius-md)] border border-[var(--ti-border)] p-1"
                  aria-label="Read transcript in">
                  {#each [{ code: "bilingual", name: "Bilingual" }, { code: "id", name: "Bahasa Indonesia" }, { code: "en", name: "English" }] as option (option.code)}
                    <button type="button" class={"rounded-[6px] px-2 py-1.5 text-xs " +
                      (language === option.code ? "bg-[var(--ti-surface-raised)]" : "text-[var(--ti-text-muted)]")}
                      aria-pressed={language === option.code}
                      onclick={() => language = option.code as "bilingual" | "id" | "en"}>{option.name}</button>
                  {/each}
                </div>
              </div>
              {#if !preview}
                <p class="m-0 text-xs text-[var(--ti-text-muted)]">Loading saved conversation...</p>
              {:else}
                <div class="max-h-[360px] divide-y divide-[var(--ti-border)] overflow-y-auto" aria-label="Saved conversation">
                  {#each preview.turns as turn (turn.sequence)}
                    <article class="py-3 first:pt-0 last:pb-0">
                      <header class="mb-2 flex items-center justify-between gap-3">
                        <strong class="text-xs">{turn.lane === "you" ? "You" : "Meeting audio"}</strong>
                        <span class="text-[11px] text-[var(--ti-text-muted)]">{meetingTime(turn.created_unix_ms)}</span>
                      </header>
                      {#if language === "bilingual"}
                        <span class="text-[10px] text-[var(--ti-text-muted)]">ORIGINAL · {languageName(turn.source_language)}</span>
                        <p class="m-0 whitespace-pre-wrap break-words text-[13px] leading-6" lang={turn.source_language}>{turn.source_text}</p>
                        <div class="mt-2 rounded-[var(--ti-radius-sm)] bg-[var(--ti-surface-raised)] p-2.5">
                          <span class="text-[10px] text-[var(--ti-text-muted)]">TRANSLATION · {languageName(turn.target_language)}</span>
                          <p class={"m-0 whitespace-pre-wrap break-words text-[13px] leading-6 " +
                            (turn.translation_state === "translated" ? "" : "text-[var(--ti-warning)]")}>
                            {turn.translation_state === "translated" ? turn.translated_text : "Translation unavailable"}
                          </p>
                        </div>
                      {:else}
                        {@const selected = textFor(turn, language)}
                        <span class="text-[10px] text-[var(--ti-text-muted)]">{selected.label} · {languageName(selected.fallback ? turn.source_language : language)}</span>
                        <p class={"m-0 whitespace-pre-wrap break-words text-[13px] leading-6 " +
                          (selected.fallback ? "text-[var(--ti-warning)]" : "")}>{selected.text}</p>
                      {/if}
                    </article>
                  {/each}
                </div>
              {/if}
            </div>
          {/if}
        </div>
      {/each}
    </div>
  {/if}

  {#if receipt}
    <div class="grid gap-2 border-t border-[var(--ti-border)] bg-[var(--ti-surface-soft)] px-5 py-4" role="status">
      <div class="flex items-center gap-2">
        <Check size={16} class="text-[var(--ti-success)]"/>
        <strong class="text-sm">HTML transcript saved</strong>
      </div>
      <p class="m-0 break-words text-xs text-[var(--ti-text-muted)]">{receipt.count} entries · {receipt.path}</p>
      {#if receipt.incomplete}
        <p class="m-0 text-xs text-[var(--ti-warning)]">Storage completeness could not be verified. The HTML includes a warning.</p>
      {/if}
      <div class="flex flex-wrap gap-2">
        <button type="button" class="ti-button ti-button-secondary min-h-9 text-xs"
          onclick={() => void openExport(false)}><ExternalLink size={14}/>Open File</button>
        <button type="button" class="ti-button ti-button-secondary min-h-9 text-xs"
          onclick={() => void openExport(true)}><FolderOpen size={14}/>Open Folder</button>
        <button type="button" class="ti-button ti-button-secondary min-h-9 text-xs"
          onclick={() => void copyPath()}><Copy size={14}/>Copy Location</button>
      </div>
      <p class="m-0 text-xs text-[var(--ti-text-muted)]">
        The source transcript remains saved until you choose Delete.
      </p>
    </div>
  {/if}
</section>
