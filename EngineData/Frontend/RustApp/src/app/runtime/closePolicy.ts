export type CloseDialogAction = "stop" | "retry" | null;

export type CloseVerdict =
  | { kind: "dialog"; title: string; message: string; action: CloseDialogAction }
  | { kind: "destroy" }
  | { kind: "stop-and-close" }
  | { kind: "wait-for-stop"; title: string; message: string };

export type ClosePolicySnapshot = {
  recordingLineId: number | null;
  pendingReview: boolean;
  runtimeUnavailable: boolean;
  voiceBuildActive: boolean;
  audioLocked: boolean;
  ownerKind: string;
  lifecycle: string;
};

function dialog(title: string, message: string, action: CloseDialogAction): CloseVerdict {
  return { kind: "dialog", title, message, action };
}

export function resolveClosePolicy(snapshot: ClosePolicySnapshot): CloseVerdict {
  if (snapshot.recordingLineId !== null) {
    return dialog(
      "Voice recording is still running",
      "Stop the current My Voice recording before closing TranslateIT so the take can be reviewed safely.",
      null,
    );
  }

  if (snapshot.pendingReview) {
    return dialog(
      "Review the current voice take",
      "Accept or retry the current My Voice take before closing TranslateIT.",
      null,
    );
  }

  if (snapshot.runtimeUnavailable) {
    return dialog(
      "Can't confirm runtime state",
      "TranslateIT can't confirm whether shared runtime resources are still active. Keep the app open and try again.",
      "retry",
    );
  }

  if (snapshot.voiceBuildActive) {
    return dialog(
      "My Voice is still being created",
      "Stop My Voice creation before closing TranslateIT so the training process can end safely.",
      null,
    );
  }

  if (!snapshot.audioLocked) return { kind: "destroy" };

  if (snapshot.ownerKind !== "meeting") {
    return dialog(
      "Audio is still in use",
      "Another TranslateIT action is still using the microphone. Finish that action before closing the app.",
      null,
    );
  }

  if (snapshot.lifecycle === "meeting_stopping") {
    return {
      kind: "wait-for-stop",
      title: "Translation is stopping",
      message: "TranslateIT will close after Meeting translation finishes stopping.",
    };
  }

  return { kind: "stop-and-close" };
}
