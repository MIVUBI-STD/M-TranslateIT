<script lang="ts">
  import { onMount } from "svelte";
  import type { ProductRuntimeSnapshot } from "../../app/bridge/runtimeProductFacade";
  import type { MeetingControlState } from "../../app/bridge/productMeetingState";
  import { buildProductCommands, type UtilityDestination } from "../../app/runtime/productCommandRegistry";
  import type { AppRoute } from "../../app/shared/types";
  import CommandPalette from "./CommandPalette.svelte";
  import UpdateAction from "./UpdateAction.svelte";

  let { snapshot, meetingControl, audioLocked, audioOwnerKind, voiceBuildActive,
    myVoiceRecording, onNavigate, onOpenUtility, onToggleMeeting, onNotice }: {
    snapshot: ProductRuntimeSnapshot;
    meetingControl: MeetingControlState;
    audioLocked: boolean;
    audioOwnerKind: string;
    voiceBuildActive: boolean;
    myVoiceRecording: boolean;
    onNavigate: (route: AppRoute) => void;
    onOpenUtility: (destination: UtilityDestination) => void;
    onToggleMeeting: () => void | Promise<void>;
    onNotice: (message: string) => void;
  } = $props();

  let paletteOpen = $state(false);
  const commands = $derived.by(() => {
    snapshot;
    return buildProductCommands({
      meetingCanStart: meetingControl.enabled && meetingControl.action === "start",
      meetingCanStop: meetingControl.enabled && meetingControl.action === "stop",
      meetingActionLabel: meetingControl.label,
      navigationLocked: myVoiceRecording,
      navigate: onNavigate,
      openUtility: onOpenUtility,
      toggleMeeting: onToggleMeeting,
    });
  });

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
  <UpdateAction {audioLocked} {audioOwnerKind} {voiceBuildActive} {myVoiceRecording} {onNotice} />
</div>
<CommandPalette bind:open={paletteOpen} {commands} />
