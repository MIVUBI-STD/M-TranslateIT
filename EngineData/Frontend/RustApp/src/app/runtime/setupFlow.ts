export type SetupStep = 1 | 2 | 3 | 4;

export function setupCheckpoint(value: unknown): SetupStep {
  const raw = Math.round(Number(value || 1));
  if (raw <= 1) return 1;
  if (raw === 2) return 2;
  if (raw === 3 || raw === 4) return 3;
  return 4;
}

export function persistedSetupCheckpoint(value: SetupStep): number {
  return value === 4 ? 5 : value;
}

export function safeSetupResumeStep(
  value: SetupStep,
  readiness: { microphoneReady: boolean; meetingRouteReady: boolean },
): SetupStep {
  if (value <= 2) return value;
  if (!readiness.microphoneReady) return 2;
  if (value >= 4 && !readiness.meetingRouteReady) return 3;
  return value;
}

export function setupStateNeedsResume(value: unknown): boolean {
  return value === "deferred";
}
