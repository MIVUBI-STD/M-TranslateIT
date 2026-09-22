import { myVoiceBuildApi } from "../bridge/myVoiceBuildApi";

function noticeForVoiceBuildReason(reason: string): string | undefined {
  if (reason === "voice_build_completed") {
    return "My Voice creation finished. Open My Voice to review it.";
  }
  if (reason === "voice_build_cancelled") {
    return "My Voice creation stopped.";
  }
  if (reason === "voice_build_failed") {
    return "My Voice creation couldn't finish. Open My Voice or Diagnostics.";
  }
  return undefined;
}

export function startVoiceBuildRuntimeSync(
  refresh: (message?: string) => void | Promise<void>,
): () => void {
  let disposed = false;
  let unlisten: (() => void) | null = null;

  void myVoiceBuildApi.subscribeRuntime((event) => {
    if (disposed || event.revision <= 0) return;
    void refresh(noticeForVoiceBuildReason(event.reason));
  }).then((stop) => {
    if (disposed) {
      stop();
      return;
    }
    unlisten = stop;
  }).catch(() => {});

  return () => {
    disposed = true;
    unlisten?.();
  };
}
