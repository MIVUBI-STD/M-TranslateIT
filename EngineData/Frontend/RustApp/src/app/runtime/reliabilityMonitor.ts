import {
  getDeviceLossGuardStatus,
  getLongSessionHealthStatus,
  getRuntimeWatchdogStatus,
} from "../bridge/reliabilityApi";

const LIVE_RELIABILITY_POLL_MS = 8_000;
const LONG_SESSION_POLL_MS = 30_000;

export type ReliabilityNotice = {
  key: string;
  message: string;
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

export function startMeetingRuntimeMonitors(
  pollMeeting: () => void | Promise<void>,
  onNotice: (message: string) => void,
  reliabilityEnabled: boolean,
): () => void {
  void pollMeeting();
  const meetingTimer = window.setInterval(() => void pollMeeting(), 1_200);
  const stopReliability = reliabilityEnabled
    ? startMeetingReliabilityMonitor(onNotice)
    : () => {};
  return () => {
    window.clearInterval(meetingTimer);
    stopReliability();
  };
}
