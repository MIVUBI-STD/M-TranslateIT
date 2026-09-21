<script lang="ts">
  import { onMount } from "svelte";
  import type { ProductRuntimeSnapshot } from "../../app/bridge/runtimeProductFacade";
  import { runtimeApi } from "../../app/bridge/runtimeApi";
  import { buildProductCommands } from "../../app/runtime/productCommandRegistry";
  import { publishTranslationOverlay } from "../../app/runtime/translationOverlayRuntime";
  import type { AppRoute } from "../../app/shared/types";
  import CommandPalette from "./CommandPalette.svelte";
  import UpdateAction from "./UpdateAction.svelte";

  let {
    snapshot,
    meetingBusy,
    myVoiceRecording,
    onNavigate,
    onToggleMeeting,
    onNotice,
  }: {
    snapshot: ProductRuntimeSnapshot;
    meetingBusy: boolean;
    myVoiceRecording: boolean;
    onNavigate: (route: AppRoute) => void;
    onToggleMeeting: () => void | Promise<void>;
    onNotice: (message: string) => void;
  } = $props();

  let paletteOpen = $state(false);
  let quickTranslateBusy = $state(false);

  const commands = $derived.by(() => buildProductCommands({
    meetingHasSession: snapshot.meeting.hasSession,
    meetingCanStart: snapshot.meeting.canStart,
    meetingCanStop: snapshot.meeting.canStop,
    quickTranslateReady: snapshot.readiness.textReady && !quickTranslateBusy,
    navigate: onNavigate,
    toggleMeeting: onToggleMeeting,
    quickTranslateClipboard,
  }));

  async function quickTranslateClipboard(): Promise<void> {
    if (quickTranslateBusy || !snapshot.readiness.textReady) {
      onNotice("Quick Translate is unavailable until Text translation is ready.");
      return;
    }

    quickTranslateBusy = true;
    try {
      if (!navigator.clipboard?.readText) throw new Error("Clipboard access is unavailable.");
      const source = (await navigator.clipboard.readText()).trim();
      if (!source) {
        onNotice("Copy some text first, then run Quick Translate Clipboard.");
        return;
      }
      if (Array.from(source).length > 2000) {
        onNotice("Clipboard text is too long for Quick Translate. Open Text for longer content.");
        return;
      }

      onNotice("Quick translating clipboard...");
      const result = await runtimeApi.quickTranslateText(source);
      if (!result.ok || !result.translated_text.trim()) {
        onNotice(result.user_message || "Quick Translate couldn't translate the clipboard.");
        return;
      }

      const shown = await publishTranslationOverlay({
        text: result.translated_text.trim(),
        language: result.target_language || snapshot.settings.target_language,
        source: "text",
        revision: `quick:${Date.now()}`,
      }, true);
      onNotice(shown === "shown"
        ? "Clipboard translated in the floating caption."
        : "Translation is ready, but the floating caption couldn't open.");
    } catch {
      onNotice("Quick Translate couldn't read the clipboard. Try again from Text.");
    } finally {
      quickTranslateBusy = false;
    }
  }

  onMount(() => {
    const handleKeydown = (event: KeyboardEvent) => {
      if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "k") {
        event.preventDefault();
        paletteOpen = !paletteOpen;
      }
    };
    window.addEventListener("keydown", handleKeydown);
    return () => window.removeEventListener("keydown", handleKeydown);
  });
</script>

<div class="flex shrink-0 items-center gap-2">
  <button
    type="button"
    class="ti-button ti-button-secondary min-h-8 shrink-0 px-2.5 text-[11px]"
    aria-label="Open quick actions"
    title="Quick actions · Ctrl+K"
    onclick={() => { paletteOpen = true; }}
  >
    Quick Actions <kbd class="ml-1 text-[9px] opacity-65">Ctrl K</kbd>
  </button>

  <UpdateAction {meetingBusy} {myVoiceRecording} {onNotice} />
</div>

<CommandPalette bind:open={paletteOpen} {commands} />
