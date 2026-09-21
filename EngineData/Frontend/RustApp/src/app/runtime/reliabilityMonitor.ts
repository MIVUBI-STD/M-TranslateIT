import {
  getDeviceLossGuardStatus,
  getRuntimeWatchdogStatus,
  getStartupRecoveryStatus,
} from "../bridge/reliabilityApi";

const LIVE_RELIABILITY_POLL_MS = 8_000;

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

export function startMeetingReliabilityMonitor(
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
  return () => {
    disposed = true;
    window.clearInterval(timer);
  };
}

export async function startupRecoveryNotice(): Promise<string | null> {
  const recovery = await getStartupRecoveryStatus();
  if (recovery.blocker) return recovery.note;
  if (recovery.previous_unclean_shutdown && recovery.cleanup_attempted) return recovery.note;
  return null;
}
