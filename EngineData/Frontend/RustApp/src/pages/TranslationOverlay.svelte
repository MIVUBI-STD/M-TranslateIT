<script lang="ts">
  import { LogicalSize, PhysicalPosition } from "@tauri-apps/api/dpi";
  import { emit, listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { availableMonitors, getCurrentWindow, primaryMonitor } from "@tauri-apps/api/window";
  import { ArrowLeft, History, Maximize2, Minus, Pause, Play, X } from "@lucide/svelte";
  import { onMount, tick } from "svelte";
  import { runtimeApi } from "../app/bridge/runtimeApi";
  import {
    TRANSLATION_OVERLAY_EVENT,
    TRANSLATION_OVERLAY_CLEAR_EVENT,
    TRANSLATION_OVERLAY_READY_EVENT,
    shouldAcceptOverlayCaption,
    TRANSLATION_OVERLAY_PREFERENCES_EVENT,
    clampPositionToWorkArea,
    intersectionArea,
    defaultBottomCenterPosition,
    captionHeightForContent,
    fitOverlayWidth,
    fitOverlayHeight,
    isProgrammaticOverlayMove,
    overlayFontSize,
    overlayWidth,
    recentMeetingCaptions,
    type PhysicalRect,
    type RecentCaptionEntry,
    type TranslationOverlayPayload,
    type TranslationOverlayPreferences,
  } from "../app/runtime/translationOverlayPolicy";
  import { hideTranslationOverlay } from "../app/runtime/translationOverlayRuntime";
  import {
    readOverlayPosition,
    readOverlayPreferences,
    discardLegacyOverlayCaption,
    recordOverlayError,
    updateOverlayPreferences,
    writeOverlayPosition,
  } from "../app/runtime/translationOverlayState";

  const COLLAPSED_WIDTH = 260;
  const HISTORY_HEIGHT = 420;
  let caption = $state<TranslationOverlayPayload | null>(null);
  let preferences = $state<TranslationOverlayPreferences>(readOverlayPreferences());
  let captionBody = $state<HTMLDivElement | null>(null);
  let captionText = $state<HTMLParagraphElement | null>(null);
  let measuredCaptionHeight = $state(0);
  let disposed = false;
  let lastAutomaticPosition: { x: number; y: number } | null = null;
  let layoutQueue: Promise<void> = Promise.resolve();
  let historyOpen = $state(false);
  let historyLoading = $state(false);
  let historyEntries = $state<RecentCaptionEntry[]>([]);
  let historyMessage = $state("");
  let captionsPaused = $state(false);
  let pendingCaption = $state<TranslationOverlayPayload | null>(null);

  const collapsed = $derived(preferences.visibility === "collapsed");
  const currentWidth = $derived(collapsed ? COLLAPSED_WIDTH : overlayWidth(preferences.width));
  const currentHeight = $derived(historyOpen ? HISTORY_HEIGHT : captionHeightForContent(measuredCaptionHeight, preferences.textSize, collapsed));
  const fontSize = $derived(overlayFontSize(preferences.textSize));

  function languageLabel(language: string): string {
    if (language === "id") return "INDONESIAN";
    if (language === "en") return "ENGLISH";
    return language.trim().toUpperCase() || "TRANSLATION";
  }
  function workAreaRect(monitor: Awaited<ReturnType<typeof primaryMonitor>>): PhysicalRect | null {
    if (!monitor) return null;
    return { x: monitor.workArea.position.x, y: monitor.workArea.position.y, width: monitor.workArea.size.width, height: monitor.workArea.size.height };
  }

  // Serialize native moves/resizes: stale async layout requests cannot race ahead of newer UI state.
  function scheduleLayout(): Promise<void> {
    layoutQueue = layoutQueue.then(() => restorePosition()).catch(() => {
      if (!disposed) recordOverlayError("Floating caption position could not be updated.");
    });
    return layoutQueue;
  }

  async function restorePosition(): Promise<void> {
    if (disposed) return;
    const nativeWindow = getCurrentWindow();
    const monitors = await availableMonitors();
    if (disposed) return;
    const stored = readOverlayPosition();
    const candidates = monitors.map((monitor) => {
      const workArea = workAreaRect(monitor)!;
      const scale = monitor.scaleFactor;
      const width = fitOverlayWidth(currentWidth, workArea.width, scale);
      const height = fitOverlayHeight(currentHeight, workArea.height, scale);
      return { workArea, width, height, scale };
    });
    const selected = stored ? candidates.map((candidate) => ({
      ...candidate,
      overlap: intersectionArea({
        x: stored.x, y: stored.y,
        width: candidate.width * candidate.scale,
        height: candidate.height * candidate.scale,
      }, candidate.workArea),
    })).sort((a, b) => b.overlap - a.overlap)[0] : null;
    const primary = await primaryMonitor();
    if (disposed) return;
    const fallback = primary ? {
      workArea: workAreaRect(primary)!,
      width: fitOverlayWidth(currentWidth, primary.workArea.size.width, primary.scaleFactor),
      height: fitOverlayHeight(currentHeight, primary.workArea.size.height, primary.scaleFactor),
      scale: primary.scaleFactor,
    } : candidates[0];
    const target = selected && selected.overlap > 0 ? selected : fallback;
    if (!target) return;
    const physicalWidth = target.width * target.scale;
    const physicalHeight = target.height * target.scale;
    const position = stored && selected && selected.overlap > 0
      ? clampPositionToWorkArea(stored, physicalWidth, physicalHeight, target.workArea)
      : defaultBottomCenterPosition(physicalWidth, physicalHeight, target.workArea, 72 * target.scale);
    const next = { x: Math.round(position.x), y: Math.round(position.y) };
    if (disposed) return;
    lastAutomaticPosition = next;
    // Move first so LogicalSize is interpreted using the destination monitor's DPI.
    await nativeWindow.setPosition(new PhysicalPosition(next.x, next.y));
    if (disposed) return;
    await nativeWindow.setSize(new LogicalSize(target.width, target.height));
  }

  async function resizeCaption(): Promise<void> {
    await tick();
    if (disposed || historyOpen || collapsed) return;
    const measured = captionText?.scrollHeight ?? 0;
    if (measured === measuredCaptionHeight) return;
    measuredCaptionHeight = measured;
    await scheduleLayout();
  }

  async function applyPreferences(next: TranslationOverlayPreferences): Promise<void> {
    preferences = next;
    if (next.visibility === "hidden") {
      try { await getCurrentWindow().setIgnoreCursorEvents(false); } catch { /* Still hide below. */ }
      await getCurrentWindow().hide();
      return;
    }
    if (next.visibility === "collapsed") historyOpen = false;
    await scheduleLayout();
    await resizeCaption();
    try { await getCurrentWindow().setIgnoreCursorEvents(next.clickThrough); }
    catch { preferences = updateOverlayPreferences({ clickThrough: false }); }
  }
  async function setVisibility(visibility: "expanded" | "collapsed"): Promise<void> {
    await applyPreferences(updateOverlayPreferences({ visibility }));
  }
  async function hide(): Promise<void> {
    await hideTranslationOverlay();
    preferences = readOverlayPreferences();
  }
  async function openRecentHistory(): Promise<void> {
    if (historyLoading || caption?.source !== "meeting") return;
    historyLoading = true;
    historyMessage = "";
    try {
      const snapshot = await runtimeApi.getMeetingCommittedTurns();
      historyEntries = recentMeetingCaptions(snapshot, 20);
      historyMessage = historyEntries.length === 0 ? "No recent committed translations yet." : "";
      historyOpen = true;
      await scheduleLayout();
    } catch {
      historyEntries = [];
      historyMessage = "Recent translations are unavailable right now.";
      historyOpen = true;
      await scheduleLayout();
    } finally {
      historyLoading = false;
    }
  }

  async function closeRecentHistory(): Promise<void> {
    historyOpen = false;
    historyMessage = "";
    await scheduleLayout();
    await resizeCaption();
  }

  async function applyCaption(next: TranslationOverlayPayload): Promise<void> {
    if (!shouldAcceptOverlayCaption(pendingCaption ?? caption, next)) return;
    if (captionsPaused && next.source === "meeting") {
      pendingCaption = next;
      return;
    }
    if (next.source !== "meeting") captionsPaused = false;
    caption = next;
    pendingCaption = null;
    await resizeCaption();
    if (next.source === "meeting" && captionBody) captionBody.scrollTop = 0;
  }

  function clearCaption(): void {
    caption = null;
    pendingCaption = null;
    historyEntries = [];
    historyOpen = false;
    captionsPaused = false;
    measuredCaptionHeight = 0;
    void scheduleLayout();
  }

  function onOverlayKeydown(event: KeyboardEvent): void {
    if (event.key !== "Escape") return;
    if (historyOpen) { event.preventDefault(); void closeRecentHistory(); }
    else if (preferences.visibility !== "hidden") { event.preventDefault(); void hide(); }
  }

  async function togglePause(): Promise<void> {
    if (caption?.source !== "meeting") return;
    if (captionsPaused) {
      captionsPaused = false;
      const latest = pendingCaption;
      pendingCaption = null;
      if (latest?.source === "meeting") await applyCaption(latest);
      return;
    }
    captionsPaused = true;
  }

  // Observe actual wrap changes after DPI, language/font and native window width updates.
  $effect(() => {
    const node = captionText;
    if (!node || typeof ResizeObserver === "undefined") return;
    const observer = new ResizeObserver(() => { if (!disposed) void resizeCaption(); });
    observer.observe(node);
    return () => observer.disconnect();
  });

  onMount(() => {
    disposed = false;
    const unlisteners: UnlistenFn[] = [];
    const onFocus = () => { if (!disposed) void scheduleLayout(); };
    const track = async (registration: Promise<UnlistenFn>): Promise<boolean> => {
      const unlisten = await registration;
      if (disposed) { unlisten(); return false; }
      unlisteners.push(unlisten);
      return true;
    };
    const initialize = async () => {
      if (!await track(listen<TranslationOverlayPayload>(TRANSLATION_OVERLAY_EVENT, (event) => {
        if (!disposed) void applyCaption(event.payload);
      }))) return;
      if (!await track(listen(TRANSLATION_OVERLAY_CLEAR_EVENT, () => {
        if (!disposed) clearCaption();
      }))) return;
      if (!await track(listen<TranslationOverlayPreferences>(TRANSLATION_OVERLAY_PREFERENCES_EVENT, (event) => {
        if (!disposed) void applyPreferences(event.payload);
      }))) return;
      discardLegacyOverlayCaption();
      preferences = readOverlayPreferences();
      await applyPreferences(preferences);
      if (disposed) return;
      if (!await track(getCurrentWindow().onMoved(({ payload }) => {
        if (disposed || isProgrammaticOverlayMove(payload, lastAutomaticPosition)) return;
        writeOverlayPosition({ x: payload.x, y: payload.y });
      }))) return;
      if (!await track(getCurrentWindow().onScaleChanged(() => {
        if (!disposed) void scheduleLayout();
      }))) return;
      if (!disposed) await emit(TRANSLATION_OVERLAY_READY_EVENT);
    };
    window.addEventListener("focus", onFocus);
    window.addEventListener("keydown", onOverlayKeydown);
    void initialize().catch(() => {
      if (!disposed) recordOverlayError("Floating caption setup is unavailable.");
    });
    return () => {
      disposed = true;
      window.removeEventListener("focus", onFocus);
      window.removeEventListener("keydown", onOverlayKeydown);
      for (const unlisten of unlisteners) unlisten();
    };
  });
</script>

<main class:collapsed class:high-contrast={preferences.contrast === "high"} class="overlay-shell" style={`--caption-font-size: ${fontSize}px`}>
  <section class="caption-card" aria-label="Floating translation caption">
    <header class="caption-header" data-tauri-drag-region>
      <div class="caption-meta" data-tauri-drag-region>
        {#if historyOpen}
          <span>RECENT · {historyEntries.length}</span>
        {:else if caption?.source === "meeting"}
          <span class:paused-dot={captionsPaused} class="live-dot" aria-hidden="true"></span>
          <span>{captionsPaused ? "PAUSED" : "LIVE"} · {languageLabel(caption.language)}</span>
        {:else}
          <span>{caption ? languageLabel(caption.language) : "TRANSLATION"}</span>
        {/if}
      </div>
      <div class="caption-controls">
        {#if historyOpen}
          <button type="button" class="caption-control" aria-label="Back to live caption" title="Back to live" onclick={() => void closeRecentHistory()}><ArrowLeft size={15} /></button>
        {:else if caption?.source === "meeting" && !collapsed}
          <button
            type="button"
            class="caption-control"
            aria-label={captionsPaused ? "Resume floating captions" : "Pause floating captions"}
            title={captionsPaused ? "Resume captions" : "Pause captions"}
            onclick={() => void togglePause()}
          >
            {#if captionsPaused}<Play size={15} />{:else}<Pause size={15} />{/if}
          </button>
          <button type="button" class="caption-control" aria-label="Show recent translations" title="Recent translations" disabled={historyLoading} onclick={() => void openRecentHistory()}><History size={15} /></button>
        {/if}
        <button type="button" class="caption-control" aria-label={collapsed ? "Expand floating caption" : "Collapse floating caption"} title={collapsed ? "Expand" : "Collapse"} onclick={() => void setVisibility(collapsed ? "expanded" : "collapsed")}>
          {#if collapsed}<Maximize2 size={15} />{:else}<Minus size={16} />{/if}
        </button>
        <button type="button" class="caption-control" aria-label="Hide floating caption" title="Hide" onclick={() => void hide()}><X size={15} /></button>
      </div>
    </header>
    {#if !collapsed}
      {#if historyOpen}
        <div class="history-body" aria-label="Recent committed translations">
          {#if historyMessage}
            <p class="history-empty">{historyMessage}</p>
          {:else}
            {#each historyEntries as entry (entry.sequence)}
              <article class="history-entry">
                <div class="history-entry-meta">
                  <span>{entry.lane === "incoming" ? "THEM" : "YOU"}</span>
                  {#if entry.createdUnixMs > 0}
                    <time datetime={new Date(entry.createdUnixMs).toISOString()}>{new Date(entry.createdUnixMs).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })}</time>
                  {/if}
                </div>
                <p lang={entry.language}>{entry.text}</p>
              </article>
            {/each}
          {/if}
        </div>
      {:else}
        <div class="caption-body" bind:this={captionBody} aria-live="polite" aria-atomic="true">
          {#if caption}<p lang={caption.language} bind:this={captionText}>{caption.text}</p>{:else}<p class="caption-placeholder">Your latest translation will appear here.</p>{/if}
        </div>
      {/if}
    {/if}
  </section>
</main>

<style>
  :global(html.ti-overlay-document body) { user-select: none; }
  .overlay-shell { width: 100vw; height: 100vh; padding: 8px; background: transparent; }
  .caption-card { display: flex; flex-direction: column; height: 100%; overflow: hidden; border: 1px solid rgb(255 255 255 / 0.14); border-radius: 14px; background: rgb(16 19 22 / 0.96); box-shadow: 0 16px 46px rgb(0 0 0 / 0.38); color: #f7f8fa; }
  .high-contrast .caption-card { background: #000; border: 2px solid #fff; box-shadow: none; }
  .high-contrast .caption-meta, .high-contrast .history-entry-meta, .high-contrast .caption-control, .high-contrast .caption-body .caption-placeholder, .high-contrast .history-empty { color: #fff; }
  .high-contrast .caption-control:focus-visible { outline: 3px solid #fff; }
  .caption-header { flex: 0 0 auto; display: flex; min-height: 38px; align-items: center; justify-content: space-between; gap: 12px; padding: 7px 9px 5px 14px; cursor: grab; }
  .caption-header:active { cursor: grabbing; }
  .caption-meta { display: flex; min-width: 0; align-items: center; gap: 8px; color: rgb(232 237 242 / 0.68); font-size: 10px; font-weight: 700; letter-spacing: 0.09em; }
  .live-dot { width: 6px; height: 6px; flex: 0 0 auto; border-radius: 999px; background: #63d59b; box-shadow: 0 0 0 3px rgb(99 213 155 / 0.12); }
  .paused-dot { background: #f0b45a; box-shadow: 0 0 0 3px rgb(240 180 90 / 0.12); }
  .caption-controls { display: flex; align-items: center; gap: 2px; }
  .caption-control { display: grid; width: 30px; height: 28px; flex: 0 0 auto; place-items: center; border: 0; border-radius: 8px; background: transparent; color: rgb(242 245 248 / 0.72); }
  .caption-control:hover { background: rgb(255 255 255 / 0.08); color: #fff; }
  .caption-control:focus-visible { outline: 2px solid rgb(255 255 255 / 0.88); outline-offset: 1px; }
  .caption-body { flex: 1 1 auto; min-height: 0; overflow-y: auto; padding: 4px 22px 18px; user-select: text; }
  .caption-body p { margin: 0; color: #f7f8fa; font-size: var(--caption-font-size); font-weight: 560; line-height: 1.45; letter-spacing: -0.012em; overflow-wrap: anywhere; }
  .caption-body .caption-placeholder { color: rgb(232 237 242 / 0.52); font-weight: 500; }
  .history-body { flex: 1 1 auto; min-height: 0; overflow-y: auto; padding: 2px 10px 12px; user-select: text; }
  .history-entry { padding: 12px 12px 13px; border-top: 1px solid rgb(255 255 255 / 0.08); }
  .history-entry:first-child { border-top: 0; }
  .history-entry-meta { display: flex; align-items: center; justify-content: space-between; gap: 12px; margin-bottom: 5px; color: rgb(232 237 242 / 0.48); font-size: 9px; font-weight: 700; letter-spacing: 0.08em; }
  .history-entry p { margin: 0; color: #f7f8fa; font-size: 15px; font-weight: 520; line-height: 1.45; overflow-wrap: anywhere; }
  .history-empty { margin: 0; padding: 18px 12px; color: rgb(232 237 242 / 0.52); font-size: 13px; line-height: 1.5; }
  .collapsed { padding: 7px; }
  .collapsed .caption-card { border-radius: 12px; }
  .collapsed .caption-header { height: 100%; min-height: 0; padding: 7px 8px 7px 13px; }
  @media (forced-colors: active) {
    .caption-card { border-color: CanvasText; background: Canvas; box-shadow: none; color: CanvasText; forced-color-adjust: auto; }
    .caption-meta, .caption-control, .caption-body p, .caption-body .caption-placeholder, .history-entry p, .history-entry-meta, .history-empty { color: CanvasText; }
    .live-dot, .paused-dot { background: Highlight; box-shadow: none; }
    .caption-control:focus-visible { outline-color: Highlight; }
  }
  @media (prefers-reduced-motion: reduce) { .caption-control { transition: none; } }
</style>
