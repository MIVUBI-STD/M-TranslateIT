import type {
  MeetingCommittedTurnsSnapshot,
  MeetingSessionStatus,
} from "../bridge/runtimeApi";
import { readMeetingReconciliation } from "./meetingReconcileReader";
import { publishLatestMeetingOverlay } from "./translationOverlayRuntime";

export type MeetingLiveViewState = {
  turns: MeetingCommittedTurnsSnapshot | null;
  transcriptStatusKey: string;
  overlayRevision: string;
  inFlight: boolean;
  revision: number;
};

export type MeetingReconcileContext = {
  ownerKind: string | null;
  booting: boolean;
  setupRequired: boolean;
  closeAfterExistingStop: boolean;
};

export type MeetingReconcileCallbacks = {
  onStatus: (status: MeetingSessionStatus) => void;
  onNotice: (message: string) => void;
  onStoppedForClose: () => void | Promise<void>;
};

export function createMeetingLiveController(): {
  read: () => MeetingLiveViewState;
  reset: () => MeetingLiveViewState;
  invalidate: () => MeetingLiveViewState;
  reconcile: (
    context: MeetingReconcileContext,
    callbacks: MeetingReconcileCallbacks,
  ) => Promise<MeetingLiveViewState>;
} {
  let state: MeetingLiveViewState = {
    turns: null,
    transcriptStatusKey: "",
    overlayRevision: "",
    inFlight: false,
    revision: 0,
  };

  const publish = (): MeetingLiveViewState => ({ ...state });

  return {
    read: publish,

    reset(): MeetingLiveViewState {
      state = {
        ...state,
        turns: null,
        transcriptStatusKey: "",
        overlayRevision: "",
        revision: state.revision + 1,
      };
      return publish();
    },

    invalidate(): MeetingLiveViewState {
      state = { ...state, revision: state.revision + 1 };
      return publish();
    },

    async reconcile(
      context: MeetingReconcileContext,
      callbacks: MeetingReconcileCallbacks,
    ): Promise<MeetingLiveViewState> {
      if (state.inFlight || context.booting || context.setupRequired) return publish();
      if (context.ownerKind !== "meeting" && !context.closeAfterExistingStop) return publish();

      const revision = state.revision;
      state = { ...state, inFlight: true };
      try {
        const result = await readMeetingReconciliation(state.turns, state.transcriptStatusKey);
        if (!result || revision !== state.revision) return publish();

        callbacks.onStatus(result.status);
        const overlayRevision = await publishLatestMeetingOverlay(
          result.turns,
          state.overlayRevision,
        );
        if (revision !== state.revision) return publish();

        state = {
          ...state,
          turns: result.turns,
          transcriptStatusKey: result.transcriptStatusKey,
          overlayRevision,
        };

        if (result.unavailable) {
          callbacks.onNotice("Meeting translation is temporarily unavailable.");
        } else if (!result.status.has_session && context.closeAfterExistingStop) {
          await callbacks.onStoppedForClose();
        }
      } finally {
        state = { ...state, inFlight: false };
      }
      return publish();
    },
  };
}
