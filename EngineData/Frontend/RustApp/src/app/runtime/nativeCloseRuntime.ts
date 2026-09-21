import { getCurrentWindow } from "@tauri-apps/api/window";
import { applicationRuntimeApi } from "../bridge/applicationRuntimeApi";
import { myVoiceApi } from "../bridge/myVoiceApi";
import { runtimeProductFacade } from "../bridge/runtimeProductFacade";
import { resolveClosePolicy, type CloseDialogAction, type CloseVerdict } from "./closePolicy";
import { destroyTranslationOverlay } from "./translationOverlayRuntime";

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
  const [myVoice, application] = await Promise.all([
    myVoiceApi.getState(),
    applicationRuntimeApi.getSnapshot(),
  ]);

  return resolveClosePolicy({
    recordingLineId: myVoice.recording_line_id,
    pendingReview: myVoice.pending_review !== null,
    runtimeUnavailable: application.revision <= 0 || application.lifecycle === "unavailable",
    voiceBuildActive: application.summaries.voice.build_active,
    audioLocked: application.resources.audio_locked,
    ownerKind: application.resources.owner_kind,
    lifecycle: application.lifecycle,
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

  const verified = await applicationRuntimeApi.getSnapshot();
  if (verified.revision <= 0 || verified.lifecycle === "unavailable") {
    return dialog(
      "Couldn't confirm Stop",
      "TranslateIT couldn't confirm that shared runtime resources were released, so the app will stay open.",
      "retry",
    );
  }

  if (!verified.resources.audio_locked) return { kind: "destroy" };

  if (
    verified.resources.owner_kind === "meeting"
    && verified.lifecycle === "meeting_stopping"
  ) {
    return {
      kind: "wait-for-stop",
      title: "Translation is stopping",
      message: "TranslateIT will close after translation finishes stopping.",
    };
  }

  return dialog(
    "Translation is still active",
    "TranslateIT hasn't confirmed that Meeting translation ended, so the app will stay open.",
    verified.resources.owner_kind === "meeting" ? "stop" : null,
  );
}
