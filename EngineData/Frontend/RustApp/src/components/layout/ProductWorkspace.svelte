<script lang="ts">
  import type { ProductRuntimeSnapshot, ProductSetupAction } from "../../app/bridge/runtimeProductFacade";
  import type { MeetingCommittedTurnsSnapshot, MeetingSessionStatus } from "../../app/bridge/runtimeApi";
  import type { AppRoute, RuntimeSettings } from "../../app/shared/types";
  import type { UtilityDestination } from "../../app/runtime/productCommandRegistry";
  import Sidebar from "./Sidebar.svelte";
  import AppHeader from "./AppHeader.svelte";
  import Meeting from "../../pages/Meeting.svelte";
  import MyVoice from "../../pages/MyVoice.svelte";
  import Settings from "../../pages/Settings.svelte";
  import Text from "../../pages/Text.svelte";

  let {
    snapshot, route, presence, notice, myVoiceRecording, meetingStatus, meetingTurns,
    utilityTarget, utilityRequest, meetingActionBusy, setupActionBusy, micTestBusy,
    onNavigate, onOpenUtility, onToggleMeeting, onRefresh, onFixSetup,
    onSettingsChange, onSetupAction, onMicTest, onRecordingChange, onMeetingVoiceChanged, onNotice,
  }: {
    snapshot: ProductRuntimeSnapshot;
    route: AppRoute;
    presence: string;
    notice: string;
    myVoiceRecording: boolean;
    meetingStatus: MeetingSessionStatus | null;
    meetingTurns: MeetingCommittedTurnsSnapshot | null;
    utilityTarget: UtilityDestination | null;
    utilityRequest: number;
    meetingActionBusy: boolean;
    setupActionBusy: boolean;
    micTestBusy: boolean;
    onNavigate: (route: AppRoute) => void;
    onOpenUtility: (target: UtilityDestination) => void;
    onToggleMeeting: () => void | Promise<void>;
    onRefresh: (preferredNotice?: string, knownSettings?: RuntimeSettings) => void | Promise<void>;
    onFixSetup: () => void | Promise<void>;
    onSettingsChange: (settings: RuntimeSettings) => void | Promise<void>;
    onSetupAction: (action: ProductSetupAction) => void | Promise<void>;
    onMicTest: () => void | Promise<void>;
    onRecordingChange: (recording: boolean) => void;
    onMeetingVoiceChanged: (message?: string) => void | Promise<void>;
    onNotice: (message: string) => void;
  } = $props();
</script>

<main class="flex h-screen min-h-0 bg-[var(--ti-bg)]">
    <Sidebar active={route} {presence} {onNavigate} />

    <section class="flex min-w-0 flex-1 flex-col">
      <AppHeader
        {snapshot}
        {route}
        {notice}
        {myVoiceRecording}
        {onNavigate}
        {onOpenUtility}
        {onToggleMeeting}
        {onNotice}
      />

      <div class="min-h-0 flex-1 overflow-y-auto">
        {#if route === "meeting"}
          <Meeting
            {snapshot}
            {meetingStatus}
            {meetingTurns}
            {utilityTarget}
            {utilityRequest}
            actionBusy={meetingActionBusy}
            onMeetingAction={onToggleMeeting}
            {onRefresh}
            {onFixSetup}
            onOpenMyVoice={() => onNavigate("my-voice")}
          />
        {:else if route === "text"}
          <!-- Text workspace stays mounted below during navigation. -->
        {:else if route === "my-voice"}
          <MyVoice
            {onNotice}
            {onMeetingVoiceChanged}
            {onRecordingChange}
          />
        {:else}
          <Settings
            {utilityTarget}
            {utilityRequest}
            {snapshot}
            settings={snapshot.settings}
            setupBusy={setupActionBusy}
            {micTestBusy}
            {onSettingsChange}
            {onRefresh}
            {onSetupAction}
            {onFixSetup}
            {onMicTest}
            {onNotice}
          />
        {/if}
        <div class={route === "text" ? "" : "hidden"}>
          <Text
            settings={snapshot.settings}
            textStatus={snapshot.readiness.textStatus}
            {onSettingsChange}
            {onNotice}
          />
        </div>
      </div>
    </section>
  </main>
