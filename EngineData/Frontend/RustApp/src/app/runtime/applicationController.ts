import { cloneSettings } from "../shared/state";
import type { RuntimeSettings } from "../shared/types";
import type { ApplicationSnapshot } from "../bridge/applicationRuntimeApi";
import { runtimeProductFacade } from "../bridge/runtimeProductFacade";
import type { ProductRuntimeSnapshot } from "../bridge/runtimeProductTypes";

export type ApplicationControllerState = {
  snapshot: ProductRuntimeSnapshot | null;
  setupSettings: RuntimeSettings;
  setupRequired: boolean;
  revision: number;
  applicationRevision: number;
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
  refreshFromApplication: (
    application: ApplicationSnapshot,
    reason: string,
    knownSettings?: RuntimeSettings,
  ) => Promise<ApplicationRefreshResult>;
  applySettings: (settings: RuntimeSettings) => void;
  applyMeetingSession: (session: ProductRuntimeSnapshot["meetingSession"]) => boolean;
  invalidate: () => number;
} {
  let state: ApplicationControllerState = {
    snapshot: null,
    setupSettings: cloneSettings(initialSettings),
    setupRequired: false,
    revision: 0,
    applicationRevision: 0,
  };

  let eventRefreshTail: Promise<void> = Promise.resolve();

  const commitSnapshot = (
    next: ProductRuntimeSnapshot,
    requestRevision: number,
  ): ApplicationRefreshResult => {
    const previousSessionId = state.snapshot?.meeting.sessionId ?? null;
    const staleApplicationSnapshot = next.applicationRevision < state.applicationRevision;
    const supersededRequestWithoutNewerApplicationState =
      requestRevision !== state.revision
      && next.applicationRevision <= state.applicationRevision;

    if (staleApplicationSnapshot || supersededRequestWithoutNewerApplicationState) {
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
      applicationRevision: next.applicationRevision,
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

    async refreshFromApplication(
      application: ApplicationSnapshot,
      reason: string,
      knownSettings?: RuntimeSettings,
    ): Promise<ApplicationRefreshResult> {
      const run = async (): Promise<ApplicationRefreshResult> => {
        if (application.revision <= state.applicationRevision) {
          const sessionId = state.snapshot?.meeting.sessionId ?? null;
          return {
            applied: false,
            previousSessionId: sessionId,
            nextSessionId: sessionId,
            setupRequired: state.setupRequired,
            snapshot: state.snapshot,
          };
        }

        const requestRevision = ++state.revision;
        const next = await runtimeProductFacade.loadProductRuntimeSnapshotFromApplication(
          application,
          state.snapshot,
          reason,
          knownSettings,
        );
        return commitSnapshot(next, requestRevision);
      };

      const result = eventRefreshTail.then(run, run);
      eventRefreshTail = result.then(
        () => undefined,
        () => undefined,
      );
      return result;
    },

    applySettings(settings: RuntimeSettings): void {
      state.revision += 1;
      const nextSettings = cloneSettings(settings);
      const setupRequired = nextSettings.meeting_setup_state === "new";
      state = {
        ...state,
        setupSettings: nextSettings,
        setupRequired,
        snapshot: setupRequired
          ? null
          : state.snapshot
            ? { ...state.snapshot, settings: nextSettings }
            : null,
      };
    },

    applyMeetingSession(session: ProductRuntimeSnapshot["meetingSession"]): boolean {
      if (!state.snapshot) return false;
      const previousOwner = state.snapshot.resources.active_owner;
      const previousLocked = state.snapshot.resources.audio_locked;
      const nextLocked = session?.has_session === true;
      const nextOwner = nextLocked ? session?.owner_id ?? null : null;
      const ownershipChanged =
        previousLocked !== nextLocked || previousOwner !== nextOwner;

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
      return ownershipChanged;
    },

    invalidate(): number {
      state.revision += 1;
      return state.revision;
    },
  };
}
