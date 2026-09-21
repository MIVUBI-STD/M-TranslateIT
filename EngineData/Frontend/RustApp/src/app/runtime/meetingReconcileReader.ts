import { runtimeApi, type MeetingCommittedTurnsSnapshot, type MeetingSessionStatus } from "../bridge/runtimeApi";
import { meetingBridgeUnavailable } from "../bridge/runtimeProductFacade";

export type MeetingPollResult = {
  status: MeetingSessionStatus;
  turns: MeetingCommittedTurnsSnapshot | null;
  transcriptStatusKey: string;
  unavailable: boolean;
};

function transcriptStatusKey(status: MeetingSessionStatus): string {
  return [
    status.session_id ?? "none",
    status.outbound.updated_unix_ms,
    status.incoming.updated_unix_ms,
    status.outbound.utterance_sequence,
  ].join(":");
}

export async function readMeetingPoll(
  currentTurns: MeetingCommittedTurnsSnapshot | null,
  previousStatusKey: string,
): Promise<MeetingPollResult | null> {
  try {
    const status = await runtimeApi.getMeetingSessionStatus();
    if (meetingBridgeUnavailable(status)) {
      return { status, turns: null, transcriptStatusKey: "", unavailable: true };
    }

    if (!status.has_session) {
      return { status, turns: null, transcriptStatusKey: "", unavailable: false };
    }

    const statusKey = transcriptStatusKey(status);
    if (statusKey === previousStatusKey) {
      return { status, turns: currentTurns, transcriptStatusKey: previousStatusKey, unavailable: false };
    }

    const nextTurns = await runtimeApi.getMeetingCommittedTurns();
    if (nextTurns.ok && nextTurns.has_session && nextTurns.session_id === status.session_id) {
      return { status, turns: nextTurns, transcriptStatusKey: statusKey, unavailable: false };
    }

    return {
      status,
      turns: currentTurns ?? nextTurns,
      transcriptStatusKey: previousStatusKey,
      unavailable: false,
    };
  } catch {
    // A thrown poll failure provides no authoritative replacement state.
    return null;
  }
}
