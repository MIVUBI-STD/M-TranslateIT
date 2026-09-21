import { cloneSettings } from "../shared/state";
import type { RuntimeSettings } from "../shared/types";
import { runtimeProductFacade } from "../bridge/runtimeProductFacade";
import type { ProductRuntimeSnapshot } from "../bridge/runtimeProductTypes";

export type ApplicationControllerState = {
  snapshot: ProductRuntimeSnapshot | null;
  setupSettings: RuntimeSettings;
  setupRequired: boolean;
  runtimeLoaded: boolean;
  revision: number;
};

export type ApplicationRefreshResult = {
  applied: boolean;
  previousSessionId: string | null;
  nextSessionId: string | null;
  setupRequired: boolean;
  snapshot: ProductRuntimeSnapshot | null;
};

export function createApplicationController(initialSettings: RuntimeSettings): {
  read: () => ApplicationControllerState;
  refresh: (knownSettings?: RuntimeSettings) => Promise<ApplicationRefreshResult>;
  applySettings: (settings: RuntimeSettings) => void;
  applyMeetingSession: (session: ProductRuntimeSnapshot["meetingSession"]) => void;
  invalidate: () => number;
} {
  let state: ApplicationControllerState = {
    snapshot: null,
    setupSettings: cloneSettings(initialSettings),
    setupRequired: false,
    runtimeLoaded: false,
    revision: 0,
  };

  const commitSnapshot = (
    next: ProductRuntimeSnapshot,
    requestRevision: number,
  ): ApplicationRefreshResult => {
    const previousSessionId = state.snapshot?.meeting.sessionId ?? null;
    if (requestRevision !== state.revision) {
      return {
        applied: false,
        previousSessionId,
        nextSessionId: state.snapshot?.meeting.sessionId ?? null,
        setupRequired: state.setupRequired,
        snapshot: state.snapshot,
      };
    }

    const nextSettings = cloneSettings(next.settings);
    const setupRequired = nextSettings.meeting_setup_state === "new";
    state = {
      ...state,
      snapshot: setupRequired ? null : next,
      setupSettings: nextSettings,
      setupRequired,
      runtimeLoaded: !setupRequired,
    };

    return {
      applied: true,
      previousSessionId,
      nextSessionId: next.meeting.sessionId,
      setupRequired,
      snapshot: state.snapshot,
    };
  };

  return {
    read: () => state,

    async refresh(knownSettings?: RuntimeSettings): Promise<ApplicationRefreshResult> {
      const requestRevision = ++state.revision;
      const next = await runtimeProductFacade.loadProductRuntimeSnapshot(knownSettings);
      return commitSnapshot(next, requestRevision);
    },

    applySettings(settings: RuntimeSettings): void {
      state.revision += 1;
      const nextSettings = cloneSettings(settings);
      const setupRequired = nextSettings.meeting_setup_state === "new";
      state = {
        ...state,
        setupSettings: nextSettings,
        setupRequired,
        runtimeLoaded: !setupRequired && state.snapshot !== null,
        snapshot: setupRequired
          ? null
          : state.snapshot
            ? { ...state.snapshot, settings: nextSettings }
            : null,
      };
    },

    applyMeetingSession(session: ProductRuntimeSnapshot["meetingSession"]): void {
      if (!state.snapshot) return;
      state.revision += 1;
      const meeting = runtimeProductFacade.mapProductMeetingState(session);
      const readiness = runtimeProductFacade.mapProductReadiness({
        settings: state.snapshot.settings,
        helper: state.snapshot.helper,
        workerStatus: state.snapshot.workerStatus,
        inputStatus: state.snapshot.inputStatus,
        meetingSession: session,
        approvedVoiceReady: state.snapshot.readiness.approvedVoiceReady,
      });
      state = {
        ...state,
        snapshot: {
          ...state.snapshot,
          meetingSession: session,
          meeting,
          readiness,
        },
      };
    },

    invalidate(): number {
      state.revision += 1;
      return state.revision;
    },
  };
}
