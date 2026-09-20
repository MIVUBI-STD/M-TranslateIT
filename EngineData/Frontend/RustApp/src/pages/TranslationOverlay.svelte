<script lang="ts">
  import { LogicalSize, PhysicalPosition } from "@tauri-apps/api/dpi";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { getCurrentWindow, primaryMonitor, availableMonitors } from "@tauri-apps/api/window";
  import { Maximize2, Minus } from "@lucide/svelte";
  import { onMount } from "svelte";
  import {
    TRANSLATION_OVERLAY_EVENT,
    type TranslationOverlayPayload,
  } from "../app/runtime/translationOverlayPolicy";

  const NORMAL_WIDTH = 620;
  const NORMAL_HEIGHT = 180;
  const COLLAPSED_WIDTH = 260;
  const COLLAPSED_HEIGHT = 64;
  const POSITION_KEY = "translateit.translationOverlay.position.v1";
  const COLLAPSED_KEY = "translateit.translationOverlay.collapsed.v1";

  type StoredPosition = { x: number; y: number };

  let caption = $state<TranslationOverlayPayload | null>(null);
  let collapsed = $state(false);

  function languageLabel(language: string): string {
    if (language === "id") return "INDONESIAN";
    if (language === "en") return "ENGLISH";
    return language.trim().toUpperCase() || "TRANSLATION";
  }

  function readStoredPosition(): StoredPosition | null {
    try {
      const parsed = JSON.parse(localStorage.getItem(POSITION_KEY) ?? "null") as StoredPosition | null;
      if (!parsed || !Number.isFinite(parsed.x) || !Number.isFinite(parsed.y)) return null;
      return parsed;
    } catch {
      return null;
    }
  }

  function positionIsOnScreen(position: StoredPosition, monitors: Awaited<ReturnType<typeof availableMonitors>>): boolean {
    return monitors.some((monitor) => {
      const left = monitor.position.x;
      const top = monitor.position.y;
      const right = left + monitor.size.width;
      const bottom = top + monitor.size.height;
      return position.x >= left - 40 && position.x < right - 40 && position.y >= top - 40 && position.y < bottom - 40;
    });
  }

  async function restorePosition(): Promise<void> {
    const nativeWindow = getCurrentWindow();
    const stored = readStoredPosition();
    const monitors = await availableMonitors();
    if (stored && positionIsOnScreen(stored, monitors)) {
      await nativeWindow.setPosition(new PhysicalPosition(stored.x, stored.y));
      return;
    }

    const monitor = await primaryMonitor();
    if (!monitor) return;
    const scale = monitor.scaleFactor;
    const width = (collapsed ? COLLAPSED_WIDTH : NORMAL_WIDTH) * scale;
    const height = (collapsed ? COLLAPSED_HEIGHT : NORMAL_HEIGHT) * scale;
    const x = monitor.position.x + Math.max(0, Math.round((monitor.size.width - width) / 2));
    const y = monitor.position.y + Math.max(0, Math.round(monitor.size.height - height - 96 * scale));
    await nativeWindow.setPosition(new PhysicalPosition(x, y));
  }

  async function applyCollapsedState(next: boolean): Promise<void> {
    collapsed = next;
    localStorage.setItem(COLLAPSED_KEY, next ? "1" : "0");
    const nativeWindow = getCurrentWindow();
    await nativeWindow.setSize(new LogicalSize(
      next ? COLLAPSED_WIDTH : NORMAL_WIDTH,
      next ? COLLAPSED_HEIGHT : NORMAL_HEIGHT,
    ));
  }

  onMount(() => {
    let disposed = false;
    let unlistenCaption: UnlistenFn | null = null;
    let unlistenMoved: UnlistenFn | null = null;

    const initialize = async () => {
      collapsed = localStorage.getItem(COLLAPSED_KEY) === "1";
      await applyCollapsedState(collapsed);
      await restorePosition();

      unlistenCaption = await listen<TranslationOverlayPayload>(TRANSLATION_OVERLAY_EVENT, (event) => {
        if (disposed) return;
        caption = event.payload;
      });

      unlistenMoved = await getCurrentWindow().onMoved(({ payload }) => {
        localStorage.setItem(POSITION_KEY, JSON.stringify({ x: payload.x, y: payload.y }));
      });
    };

    void initialize();

    return () => {
      disposed = true;
      unlistenCaption?.();
      unlistenMoved?.();
    };
  });
</script>

<main class:collapsed class="overlay-shell">
  <section class="caption-card" aria-label="Floating translation caption">
    <header class="caption-header" data-tauri-drag-region>
      <div class="caption-meta" data-tauri-drag-region>
        <span class="live-dot" aria-hidden="true"></span>
        <span>{collapsed ? "TranslateIT" : caption ? languageLabel(caption.language) : "TRANSLATION"}</span>
      </div>
      <button
        type="button"
        class="caption-control"
        aria-label={collapsed ? "Expand floating caption" : "Collapse floating caption"}
        title={collapsed ? "Expand" : "Collapse"}
        onclick={() => void applyCollapsedState(!collapsed)}
      >
        {#if collapsed}<Maximize2 size={15} />{:else}<Minus size={16} />{/if}
      </button>
    </header>

    {#if !collapsed}
      <div class="caption-body" aria-live="polite" aria-atomic="true">
        {#if caption}
          <p lang={caption.language}>{caption.text}</p>
        {:else}
          <p class="caption-placeholder">Your latest translation will appear here.</p>
        {/if}
      </div>
    {/if}
  </section>
</main>

<style>
  :global(html.ti-overlay-document body) {
    user-select: none;
  }

  .overlay-shell {
    width: 100vw;
    height: 100vh;
    padding: 8px;
    background: transparent;
  }

  .caption-card {
    height: 100%;
    overflow: hidden;
    border: 1px solid rgb(255 255 255 / 0.14);
    border-radius: 14px;
    background: rgb(16 19 22 / 0.96);
    box-shadow: 0 16px 46px rgb(0 0 0 / 0.38);
    color: #f7f8fa;
  }

  .caption-header {
    display: flex;
    min-height: 38px;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 7px 9px 5px 14px;
    cursor: grab;
  }

  .caption-header:active {
    cursor: grabbing;
  }

  .caption-meta {
    display: flex;
    min-width: 0;
    align-items: center;
    gap: 8px;
    color: rgb(232 237 242 / 0.68);
    font-size: 10px;
    font-weight: 700;
    letter-spacing: 0.09em;
  }

  .live-dot {
    width: 6px;
    height: 6px;
    flex: 0 0 auto;
    border-radius: 999px;
    background: #63d59b;
    box-shadow: 0 0 0 3px rgb(99 213 155 / 0.12);
  }

  .caption-control {
    display: grid;
    width: 30px;
    height: 28px;
    flex: 0 0 auto;
    place-items: center;
    border: 0;
    border-radius: 8px;
    background: transparent;
    color: rgb(242 245 248 / 0.72);
  }

  .caption-control:hover {
    background: rgb(255 255 255 / 0.08);
    color: #fff;
  }

  .caption-control:focus-visible {
    outline: 2px solid rgb(255 255 255 / 0.88);
    outline-offset: 1px;
  }

  .caption-body {
    max-height: 132px;
    overflow-y: auto;
    padding: 4px 22px 18px;
    user-select: text;
  }

  .caption-body p {
    margin: 0;
    color: #f7f8fa;
    font-size: 22px;
    font-weight: 560;
    line-height: 1.45;
    letter-spacing: -0.012em;
    overflow-wrap: anywhere;
  }

  .caption-body .caption-placeholder {
    color: rgb(232 237 242 / 0.52);
    font-weight: 500;
  }

  .collapsed {
    padding: 7px;
  }

  .collapsed .caption-card {
    border-radius: 12px;
  }

  .collapsed .caption-header {
    height: 100%;
    min-height: 0;
    padding: 7px 8px 7px 13px;
  }

  @media (prefers-reduced-motion: reduce) {
    .caption-control {
      transition: none;
    }
  }
</style>
