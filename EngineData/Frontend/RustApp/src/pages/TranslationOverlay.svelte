<script lang="ts">
  import { LogicalSize, PhysicalPosition } from "@tauri-apps/api/dpi";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { availableMonitors, getCurrentWindow, primaryMonitor } from "@tauri-apps/api/window";
  import { Maximize2, Minus, X } from "@lucide/svelte";
  import { onMount, tick } from "svelte";
  import {
    TRANSLATION_OVERLAY_EVENT,
    TRANSLATION_OVERLAY_PREFERENCES_EVENT,
    clampPositionToWorkArea,
    intersectionArea,
    defaultBottomCenterPosition,
    overlayFontSize,
    overlayHeightForTextSize,
    type PhysicalRect,
    type TranslationOverlayPayload,
    type TranslationOverlayPreferences,
  } from "../app/runtime/translationOverlayPolicy";
  import { hideTranslationOverlay } from "../app/runtime/translationOverlayRuntime";
  import {
    readLatestOverlayCaption,
    readOverlayPosition,
    readOverlayPreferences,
    updateOverlayPreferences,
    writeOverlayPosition,
  } from "../app/runtime/translationOverlayState";

  const NORMAL_WIDTH = 620;
  const COLLAPSED_WIDTH = 260;
  let caption = $state<TranslationOverlayPayload | null>(null);
  let preferences = $state<TranslationOverlayPreferences>(readOverlayPreferences());
  let captionBody = $state<HTMLDivElement | null>(null);

  const collapsed = $derived(preferences.visibility === "collapsed");
  const currentWidth = $derived(collapsed ? COLLAPSED_WIDTH : NORMAL_WIDTH);
  const currentHeight = $derived(overlayHeightForTextSize(preferences.textSize, collapsed));
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

  async function restorePosition(): Promise<void> {
    const nativeWindow = getCurrentWindow();
    const monitors = await availableMonitors();
    const stored = readOverlayPosition();
    if (stored && monitors.length > 0) {
      const candidates = monitors.map((monitor) => ({
        workArea: workAreaRect(monitor)!,
        width: currentWidth * monitor.scaleFactor,
        height: currentHeight * monitor.scaleFactor,
      }));
      const selected = candidates
        .map((entry) => ({
          ...entry,
          visibleArea: intersectionArea(
            { x: stored.x, y: stored.y, width: entry.width, height: entry.height },
            entry.workArea,
          ),
        }))
        .sort((left, right) => right.visibleArea - left.visibleArea)[0];
      if (selected && selected.visibleArea > 0) {
        const next = clampPositionToWorkArea(stored, selected.width, selected.height, selected.workArea);
        await nativeWindow.setPosition(new PhysicalPosition(next.x, next.y));
        return;
      }
    }
    const monitor = await primaryMonitor();
    const workArea = workAreaRect(monitor);
    if (!monitor || !workArea) return;
    const next = defaultBottomCenterPosition(currentWidth * monitor.scaleFactor, currentHeight * monitor.scaleFactor, workArea, 72 * monitor.scaleFactor);
    await nativeWindow.setPosition(new PhysicalPosition(next.x, next.y));
  }

  async function applyPreferences(next: TranslationOverlayPreferences, reposition = false): Promise<void> {
    preferences = next;
    if (next.visibility === "hidden") { await getCurrentWindow().hide(); return; }
    await getCurrentWindow().setSize(new LogicalSize(
      next.visibility === "collapsed" ? COLLAPSED_WIDTH : NORMAL_WIDTH,
      overlayHeightForTextSize(next.textSize, next.visibility === "collapsed"),
    ));
    if (reposition) await restorePosition();
  }
  async function setVisibility(visibility: "expanded" | "collapsed"): Promise<void> {
    await applyPreferences(updateOverlayPreferences({ visibility }), true);
  }
  async function hide(): Promise<void> {
    await hideTranslationOverlay();
    preferences = readOverlayPreferences();
  }
  async function applyCaption(next: TranslationOverlayPayload): Promise<void> {
    caption = next;
    if (next.source === "meeting") {
      await tick();
      if (captionBody) captionBody.scrollTop = 0;
    }
  }

  onMount(() => {
    let disposed = false;
    let unlistenCaption: UnlistenFn | null = null;
    let unlistenPreferences: UnlistenFn | null = null;
    let unlistenMoved: UnlistenFn | null = null;
    const initialize = async () => {
      unlistenCaption = await listen<TranslationOverlayPayload>(TRANSLATION_OVERLAY_EVENT, (event) => {
        if (!disposed) void applyCaption(event.payload);
      });
      unlistenPreferences = await listen<TranslationOverlayPreferences>(TRANSLATION_OVERLAY_PREFERENCES_EVENT, (event) => {
        if (!disposed) void applyPreferences(event.payload, true);
      });
      caption = readLatestOverlayCaption();
      preferences = readOverlayPreferences();
      await applyPreferences(preferences);
      await restorePosition();
      unlistenMoved = await getCurrentWindow().onMoved(({ payload }) => writeOverlayPosition({ x: payload.x, y: payload.y }));
    };
    void initialize();
    return () => { disposed = true; unlistenCaption?.(); unlistenPreferences?.(); unlistenMoved?.(); };
  });
</script>

<main class:collapsed class="overlay-shell" style={`--caption-font-size: ${fontSize}px`}>
  <section class="caption-card" aria-label="Floating translation caption">
    <header class="caption-header" data-tauri-drag-region>
      <div class="caption-meta" data-tauri-drag-region>
        {#if caption?.source === "meeting"}<span class="live-dot" aria-hidden="true"></span><span>LIVE · {languageLabel(caption.language)}</span>
        {:else}<span>{caption ? languageLabel(caption.language) : "TRANSLATION"}</span>{/if}
      </div>
      <div class="caption-controls">
        <button type="button" class="caption-control" aria-label={collapsed ? "Expand floating caption" : "Collapse floating caption"} title={collapsed ? "Expand" : "Collapse"} onclick={() => void setVisibility(collapsed ? "expanded" : "collapsed")}>
          {#if collapsed}<Maximize2 size={15} />{:else}<Minus size={16} />{/if}
        </button>
        <button type="button" class="caption-control" aria-label="Hide floating caption" title="Hide" onclick={() => void hide()}><X size={15} /></button>
      </div>
    </header>
    {#if !collapsed}
      <div class="caption-body" bind:this={captionBody} aria-live="polite" aria-atomic="true">
        {#if caption}<p lang={caption.language}>{caption.text}</p>{:else}<p class="caption-placeholder">Your latest translation will appear here.</p>{/if}
      </div>
    {/if}
  </section>
</main>

<style>
  :global(html.ti-overlay-document body) { user-select: none; }
  .overlay-shell { width: 100vw; height: 100vh; padding: 8px; background: transparent; }
  .caption-card { height: 100%; overflow: hidden; border: 1px solid rgb(255 255 255 / 0.14); border-radius: 14px; background: rgb(16 19 22 / 0.96); box-shadow: 0 16px 46px rgb(0 0 0 / 0.38); color: #f7f8fa; }
  .caption-header { display: flex; min-height: 38px; align-items: center; justify-content: space-between; gap: 12px; padding: 7px 9px 5px 14px; cursor: grab; }
  .caption-header:active { cursor: grabbing; }
  .caption-meta { display: flex; min-width: 0; align-items: center; gap: 8px; color: rgb(232 237 242 / 0.68); font-size: 10px; font-weight: 700; letter-spacing: 0.09em; }
  .live-dot { width: 6px; height: 6px; flex: 0 0 auto; border-radius: 999px; background: #63d59b; box-shadow: 0 0 0 3px rgb(99 213 155 / 0.12); }
  .caption-controls { display: flex; align-items: center; gap: 2px; }
  .caption-control { display: grid; width: 30px; height: 28px; flex: 0 0 auto; place-items: center; border: 0; border-radius: 8px; background: transparent; color: rgb(242 245 248 / 0.72); }
  .caption-control:hover { background: rgb(255 255 255 / 0.08); color: #fff; }
  .caption-control:focus-visible { outline: 2px solid rgb(255 255 255 / 0.88); outline-offset: 1px; }
  .caption-body { height: calc(100% - 38px); overflow-y: auto; padding: 4px 22px 18px; user-select: text; }
  .caption-body p { margin: 0; color: #f7f8fa; font-size: var(--caption-font-size); font-weight: 560; line-height: 1.45; letter-spacing: -0.012em; overflow-wrap: anywhere; }
  .caption-body .caption-placeholder { color: rgb(232 237 242 / 0.52); font-weight: 500; }
  .collapsed { padding: 7px; }
  .collapsed .caption-card { border-radius: 12px; }
  .collapsed .caption-header { height: 100%; min-height: 0; padding: 7px 8px 7px 13px; }
  @media (forced-colors: active) {
    .caption-card { border-color: CanvasText; background: Canvas; box-shadow: none; color: CanvasText; forced-color-adjust: auto; }
    .caption-meta, .caption-control, .caption-body p, .caption-body .caption-placeholder { color: CanvasText; }
    .live-dot { background: Highlight; box-shadow: none; }
    .caption-control:focus-visible { outline-color: Highlight; }
  }
  @media (prefers-reduced-motion: reduce) { .caption-control { transition: none; } }
</style>
