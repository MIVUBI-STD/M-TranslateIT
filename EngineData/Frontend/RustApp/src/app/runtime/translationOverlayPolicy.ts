export const TRANSLATION_OVERLAY_EVENT = "translation-overlay:update";
export const TRANSLATION_OVERLAY_PREFERENCES_EVENT = "translation-overlay:preferences";

export type TranslationOverlayPayload = {
  text: string;
  language: string;
  source: "text" | "meeting";
  revision: string;
};

export type OverlayVisibility = "expanded" | "collapsed" | "hidden";
export type OverlayTextSize = "small" | "medium" | "large" | "extra-large";

export type TranslationOverlayPreferences = {
  meetingEnabled: boolean;
  visibility: OverlayVisibility;
  textSize: OverlayTextSize;
};

export type PhysicalPoint = { x: number; y: number };
export type PhysicalRect = { x: number; y: number; width: number; height: number };

type MeetingTurnLike = { sequence: number; lane: string; translated_text: string; };
type MeetingTurnsLike = { ok: boolean; has_session: boolean; session_id: string | null; turns: MeetingTurnLike[]; };

export const DEFAULT_OVERLAY_PREFERENCES: TranslationOverlayPreferences = {
  meetingEnabled: true,
  visibility: "expanded",
  textSize: "medium",
};

export function normalizeOverlayText(value: string): string {
  return value.replace(/\r\n/g, "\n").trim();
}

export function latestMeetingCaption(snapshot: MeetingTurnsLike | null): TranslationOverlayPayload | null {
  if (!snapshot?.ok || !snapshot.has_session || !snapshot.session_id) return null;
  const latest = snapshot.turns
    .filter((turn) => normalizeOverlayText(turn.translated_text).length > 0)
    .reduce<MeetingTurnLike | null>((current, turn) => !current || turn.sequence > current.sequence ? turn : current, null);
  if (!latest) return null;
  return {
    text: normalizeOverlayText(latest.translated_text),
    language: latest.lane === "incoming" ? "id" : "en",
    source: "meeting",
    revision: `${snapshot.session_id}:${latest.sequence}`,
  };
}

export function overlayHeightForTextSize(size: OverlayTextSize, collapsed = false): number {
  if (collapsed) return 64;
  if (size === "small") return 168;
  if (size === "large") return 210;
  if (size === "extra-large") return 240;
  return 180;
}

export function overlayFontSize(size: OverlayTextSize): number {
  if (size === "small") return 18;
  if (size === "large") return 26;
  if (size === "extra-large") return 30;
  return 22;
}

function intersectionArea(a: PhysicalRect, b: PhysicalRect): number {
  const width = Math.max(0, Math.min(a.x + a.width, b.x + b.width) - Math.max(a.x, b.x));
  const height = Math.max(0, Math.min(a.y + a.height, b.y + b.height) - Math.max(a.y, b.y));
  return width * height;
}

export function bestWorkAreaForWindow(position: PhysicalPoint, windowWidth: number, windowHeight: number, workAreas: PhysicalRect[]): PhysicalRect | null {
  const windowRect = { x: position.x, y: position.y, width: windowWidth, height: windowHeight };
  let best: PhysicalRect | null = null;
  let bestArea = 0;
  for (const workArea of workAreas) {
    const area = intersectionArea(windowRect, workArea);
    if (area > bestArea) { best = workArea; bestArea = area; }
  }
  return best;
}

export function clampPositionToWorkArea(position: PhysicalPoint, windowWidth: number, windowHeight: number, workArea: PhysicalRect): PhysicalPoint {
  const maxX = Math.max(workArea.x, workArea.x + workArea.width - windowWidth);
  const maxY = Math.max(workArea.y, workArea.y + workArea.height - windowHeight);
  return {
    x: Math.min(Math.max(position.x, workArea.x), maxX),
    y: Math.min(Math.max(position.y, workArea.y), maxY),
  };
}

export function defaultBottomCenterPosition(windowWidth: number, windowHeight: number, workArea: PhysicalRect, bottomGap: number): PhysicalPoint {
  return clampPositionToWorkArea({
    x: Math.round(workArea.x + (workArea.width - windowWidth) / 2),
    y: Math.round(workArea.y + workArea.height - windowHeight - bottomGap),
  }, windowWidth, windowHeight, workArea);
}
