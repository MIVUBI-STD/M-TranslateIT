<script lang="ts">
  import "../styles/translation-overlay.css";
  import { LogicalSize, PhysicalPosition } from "@tauri-apps/api/dpi";
  import { emit, listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { availableMonitors, getCurrentWindow, primaryMonitor } from "@tauri-apps/api/window";
  import { Maximize2, Minus, X } from "@lucide/svelte";
  import { onMount, tick } from "svelte";
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
    mayPresentOverlayCaption,
    overlayFontSize,
    overlayWidth,
    type PhysicalRect,
    type TranslationOverlayPayload,
    type TranslationOverlayClearPayload,
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
  let caption = $state<TranslationOverlayPayload | null>(null);
  let preferences = $state<TranslationOverlayPreferences>(readOverlayPreferences());
  let captionBody = $state<HTMLDivElement | null>(null);
  let captionText = $state<HTMLParagraphElement | null>(null);
  let measuredCaptionHeight = $state(0);
  let disposed = false;
  let lastAutomaticPosition: { x: number; y: number } | null = null;
  let layoutQueue: Promise<void> = Promise.resolve();
  let layoutPending = false;

  const collapsed = $derived(preferences.visibility === "collapsed");
  const currentWidth = $derived(collapsed ? COLLAPSED_WIDTH : overlayWidth(preferences.width));
  const currentHeight = $derived(captionHeightForContent(measuredCaptionHeight, preferences.textSize, collapsed));
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

  // Serialize native moves, coalescing repetitive ResizeObserver/DPI requests.
  // At most one queued layout follows the one currently executing.
  function scheduleLayout(): Promise<void> {
    if (disposed || layoutPending) return layoutQueue;
    layoutPending = true;
    layoutQueue = layoutQueue.then(async () => {
      layoutPending = false;
      if (!disposed) await restorePosition();
    }).catch(() => {
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
    if (disposed || collapsed) return;
    const measured = captionText?.scrollHeight ?? 0;
    if (measured === measuredCaptionHeight) return;
    measuredCaptionHeight = measured;
    await scheduleLayout();
  }

  async function applyPreferences(next: TranslationOverlayPreferences): Promise<void> {
    preferences = next;
    if (next.visibility === "hidden") {
      clearCaption();
      try { await getCurrentWindow().setIgnoreCursorEvents(false); } catch { /* Still hide below. */ }
      await getCurrentWindow().hide();
      return;
    }
    await scheduleLayout();
    await resizeCaption();
    try { await getCurrentWindow().setIgnoreCursorEvents(next.clickThrough); }
    catch { preferences = updateOverlayPreferences({ clickThrough: false }); }
  }
  async function setVisibility(visibility: "expanded" | "collapsed"): Promise<void> {
    const previous = readOverlayPreferences().visibility;
    try { await applyPreferences(updateOverlayPreferences({ visibility })); }
    catch {
      preferences = updateOverlayPreferences({ visibility: previous });
      recordOverlayError("Floating caption size could not be changed.");
    }
  }
  async function hide(): Promise<void> {
    clearCaption();
    await hideTranslationOverlay();
    preferences = readOverlayPreferences();
  }
  async function applyCaption(next: TranslationOverlayPayload): Promise<void> {
    // Ignore duplicates and stale Meeting revisions without buffering old private text.
    if (disposed || !mayPresentOverlayCaption(readOverlayPreferences(), next)
      || !shouldAcceptOverlayCaption(caption, next)) return;
    caption = next;
    await resizeCaption();
    if (next.source === "meeting" && captionBody) captionBody.scrollTop = 0;
  }

  function clearCaption(): void {
    caption = null;
    measuredCaptionHeight = 0;
    void scheduleLayout();
  }

  function onOverlayKeydown(event: KeyboardEvent): void {
    if (event.key === "Escape" && preferences.visibility !== "hidden") {
      event.preventDefault();
      void hide();
    }
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
      if (!await track(listen<TranslationOverlayClearPayload>(TRANSLATION_OVERLAY_CLEAR_EVENT, (event) => {
        if (disposed) return;
        const onlyRevision = event.payload?.onlyRevision;
        if (onlyRevision && caption?.revision !== onlyRevision) return;
        clearCaption();
      }))) return;
      if (!await track(listen<TranslationOverlayPreferences>(TRANSLATION_OVERLAY_PREFERENCES_EVENT, () => {
        // Notification payloads can arrive out of order. Persistent preferences own truth.
        if (!disposed) void applyPreferences(readOverlayPreferences()).catch(() => {
          if (!disposed) recordOverlayError("Floating caption preferences could not be applied.");
        });
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
        {#if caption?.source === "meeting"}
          <span class="live-dot" aria-hidden="true"></span>
          <span>MEETING · {languageLabel(caption.language)}</span>
        {:else}
          <span>{caption ? languageLabel(caption.language) : "TRANSLATION"}</span>
        {/if}
      </div>
      <div class="caption-controls">
        <button type="button" class="caption-control" aria-label={collapsed ? "Expand floating caption" : "Collapse floating caption"} title={collapsed ? "Expand" : "Collapse"} onclick={() => void setVisibility(collapsed ? "expanded" : "collapsed")}>
          {#if collapsed}<Maximize2 size={15} />{:else}<Minus size={16} />{/if}
        </button>
        <button type="button" class="caption-control" aria-label="Hide floating caption" title="Hide" onclick={() => void hide()}><X size={15} /></button>
      </div>
    </header>
    {#if !collapsed}
        <!-- svelte-ignore a11y_no_noninteractive_tabindex (Named scroll regions need keyboard focus to scroll their read-only text.) -->
        <div class="caption-body" bind:this={captionBody} role="region" tabindex="0" aria-label="Current translation" aria-live="polite" aria-atomic="true">
          {#if caption}<p lang={caption.language} bind:this={captionText}>{caption.text}</p>{:else}<p class="caption-placeholder">Your latest translation will appear here.</p>{/if}
        </div>
    {/if}
  </section>
</main>

