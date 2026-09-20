import { getCurrentWindow } from "@tauri-apps/api/window";
import { destroyTranslationOverlay } from "./translationOverlayRuntime";
import { myVoiceApi } from "../bridge/myVoiceApi";
import { myVoiceBuildApi } from "../bridge/myVoiceBuildApi";
import { runtimeApi } from "../bridge/runtimeApi";
import {
  mapProductMeetingState,
  meetingBridgeUnavailable,
  runtimeProductFacade,
} from "../bridge/runtimeProductFacade";
import { resolveClosePolicy, type CloseDialogAction, type CloseVerdict } from "./closePolicy";

function dialog(title: string, message: string, action: CloseDialogAction): CloseVerdict {
  return { kind: "dialog", title, message, action };
}

export async function installNativeCloseGuard(
  onCloseRequested: () => void | Promise<void>,
): Promise<() => void> {
  return getCurrentWindow().onCloseRequested(async (event) => {
    event.preventDefault();
    await onCloseRequested();
  });
}

export async function destroyTranslateItWindows(): Promise<void> {
  await destroyTranslationOverlay();
  await getCurrentWindow().destroy();
}

export async function resolveNativeCloseVerdict(): Promise<CloseVerdict> {
  const [myVoice, build, status] = await Promise.all([
    myVoiceApi.getState(),
    myVoiceBuildApi.getStatus(),
    runtimeApi.getMeetingSessionStatus(),
  ]);
  const meeting = mapProductMeetingState(status);

  return resolveClosePolicy({
    recordingLineId: myVoice.recording_line_id,
    pendingReview: myVoice.pending_review !== null,
    buildUnavailable: build.phase === "unavailable",
    buildActive: build.active,
    meetingUnavailable: meetingBridgeUnavailable(status),
    hasMeetingSession: status.has_session,
    meetingApplicationOwned: meeting.applicationOwned,
    meetingLifecycle: meeting.lifecycle,
  });
}

export async function stopAndResolveNativeClose(): Promise<CloseVerdict> {
  const verdict = await resolveNativeCloseVerdict();
  if (verdict.kind !== "stop-and-close") return verdict;

  const result = await runtimeProductFacade.runProductMeetingAction("stop");
  if (!result.ok) {
    return dialog(
      "Couldn't stop translation",
      "TranslateIT will stay open. Try Stop again or check Diagnostics.",
      "stop",
    );
  }

  const verified = await runtimeApi.getMeetingSessionStatus();
  if (meetingBridgeUnavailable(verified)) {
    return dialog(
      "Couldn't confirm Stop",
      "TranslateIT couldn't confirm that Meeting translation ended, so the app will stay open.",
      "retry",
    );
  }
  if (!verified.has_session) return { kind: "destroy" };

  const verifiedMeeting = mapProductMeetingState(verified);
  if (verifiedMeeting.applicationOwned && verifiedMeeting.lifecycle === "stopping") {
    return {
      kind: "wait-for-stop",
      title: "Translation is stopping",
      message: "TranslateIT will close after translation finishes stopping.",
    };
  }

  return dialog(
    "Translation is still active",
    "TranslateIT hasn't confirmed that Meeting translation ended, so the app will stay open.",
    verifiedMeeting.applicationOwned ? "stop" : null,
  );
}
