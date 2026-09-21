import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import {
  getDeviceLossGuardStatus,
  getLongSessionHealthStatus,
  getRuntimeWatchdogStatus,
} from "../bridge/reliabilityApi";

const LIVE_RELIABILITY_POLL_MS = 8_000;
const LONG_SESSION_POLL_MS = 30_000;
const MEETING_RECONCILIATION_POLL_MS = 10_000;
const MEETING_EVENT_DEBOUNCE_MS = 80;
const MEETING_RUNTIME_EVENT = "translateit://meeting-runtime";

export type ReliabilityNotice = {
  key: string;
  message: string;
};

type MeetingRuntimeEvent = {
  revision: number;
  reason: string;
  session_id: string | null;
  sequence: number | null;
};

async function readLiveReliabilityNotice(): Promise<ReliabilityNotice | null> {
  const [watchdog, devices] = await Promise.all([
    getRuntimeWatchdogStatus(),
    getDeviceLossGuardStatus(),
  ]);

  if (devices.action_required && devices.blocker) {
    return { key: devices.blocker, message: devices.note };
  }
  if (watchdog.action_required && watchdog.blocker) {
    return { key: watchdog.blocker, message: watchdog.note };
  }
  if (devices.optional_device_lost && devices.blocker) {
    return { key: devices.blocker, message: devices.note };
  }
  return null;
}

function startMeetingReliabilityMonitor(
  onNotice: (message: string) => void,
): () => void {
  let disposed = false;
  let inFlight = false;
  let lastKey = "";

  const poll = async () => {
    if (disposed || inFlight) return;
    inFlight = true;
    try {
      const notice = await readLiveReliabilityNotice();
      if (!notice) {
        lastKey = "";
        return;
      }
      if (notice.key !== lastKey) {
        lastKey = notice.key;
        onNotice(notice.message);
      }
    } finally {
      inFlight = false;
    }
  };

  void poll();
  const timer = window.setInterval(() => void poll(), LIVE_RELIABILITY_POLL_MS);

  let lastLongSessionState = "";
  let longSessionInFlight = false;
  const pollLongSession = async () => {
    if (disposed || longSessionInFlight) return;
    longSessionInFlight = true;
    try {
      const health = await getLongSessionHealthStatus();
      if (!health.healthy && health.warning_count > 0) {
        const key = [
          health.outbound_overflow_dropped,
          health.outbound_evicted_pending,
          health.deferred_incoming_dropped_overflow,
          health.deferred_incoming_dropped_stale,
          health.transcript_dropped_turns,
          health.meeting_temp_file_count,
        ].join(":");
        if (key !== lastLongSessionState) {
          lastLongSessionState = key;
          onNotice(health.note);
        }
      } else {
        lastLongSessionState = "";
      }
    } finally {
      longSessionInFlight = false;
    }
  };
  const longSessionTimer = window.setInterval(
    () => void pollLongSession(),
    LONG_SESSION_POLL_MS,
  );

  return () => {
    disposed = true;
    window.clearInterval(timer);
    window.clearInterval(longSessionTimer);
  };
}

function startMeetingChangeEvents(
  pollMeeting: () => void | Promise<void>,
): () => void {
  let disposed = false;
  let unlisten: UnlistenFn | null = null;
  let debounceTimer: number | null = null;
  let lastRevision = 0;

  const scheduleRefresh = (payload: MeetingRuntimeEvent) => {
    if (disposed || payload.revision <= lastRevision) return;
    lastRevision = payload.revision;
    if (debounceTimer !== null) window.clearTimeout(debounceTimer);
    debounceTimer = window.setTimeout(() => {
      debounceTimer = null;
      if (!disposed) void pollMeeting();
    }, MEETING_EVENT_DEBOUNCE_MS);
  };

  void listen<MeetingRuntimeEvent>(MEETING_RUNTIME_EVENT, (event) => {
    scheduleRefresh(event.payload);
  }).then((stop) => {
    if (disposed) {
      stop();
      return;
    }
    unlisten = stop;
  }).catch(() => {});

  return () => {
    disposed = true;
    if (debounceTimer !== null) window.clearTimeout(debounceTimer);
    unlisten?.();
  };
}

export function startMeetingRuntimeMonitors(
  pollMeeting: () => void | Promise<void>,
  onNotice: (message: string) => void,
  reliabilityEnabled: boolean,
): () => void {
  void pollMeeting();
  const stopMeetingEvents = startMeetingChangeEvents(pollMeeting);
  const meetingTimer = window.setInterval(
    () => void pollMeeting(),
    MEETING_RECONCILIATION_POLL_MS,
  );
  const stopReliability = reliabilityEnabled
    ? startMeetingReliabilityMonitor(onNotice)
    : () => {};
  return () => {
    window.clearInterval(meetingTimer);
    stopMeetingEvents();
    stopReliability();
  };
}
