export const TRANSLATION_OVERLAY_EVENT = "translation-overlay:update";
export const TRANSLATION_OVERLAY_PREFERENCES_EVENT = "translation-overlay:preferences";
export const TRANSLATION_OVERLAY_READY_EVENT = "translation-overlay:ready";
export const TRANSLATION_OVERLAY_CLEAR_EVENT = "translation-overlay:clear";

export type TranslationOverlayPayload = {
  text: string;
  language: string;
  source: "text" | "meeting";
  revision: string;
};

export type OverlayVisibility = "expanded" | "collapsed" | "hidden";
export type OverlayTextSize = "small" | "medium" | "large" | "extra-large";
export type OverlayWidth = "compact" | "standard" | "wide";
export type OverlayContrast = "standard" | "high";

export type TranslationOverlayPreferences = {
  meetingEnabled: boolean;
  visibility: OverlayVisibility;
  textSize: OverlayTextSize;
  width: OverlayWidth;
  contrast: OverlayContrast;
  clickThrough: boolean;
};

// Dedupe and reject late committed-turn presentation in the same Meeting session.
export function shouldAcceptOverlayCaption(current: TranslationOverlayPayload | null, next: TranslationOverlayPayload): boolean {
  if (!current) return true;
  if (current.source === next.source && current.revision === next.revision) return false;
  if (current.source !== "meeting" || next.source !== "meeting") return true;
  const previousSeparator = current.revision.lastIndexOf(":");
  const nextSeparator = next.revision.lastIndexOf(":");
  if (previousSeparator < 1 || nextSeparator < 1) return true;
  if (current.revision.slice(0, previousSeparator) !== next.revision.slice(0, nextSeparator)) return true;
  const previous = Number(current.revision.slice(previousSeparator + 1));
  const upcoming = Number(next.revision.slice(nextSeparator + 1));
  return !Number.isSafeInteger(previous) || !Number.isSafeInteger(upcoming) || upcoming > previous;
}

export type PhysicalPoint = { x: number; y: number };
export type PhysicalRect = { x: number; y: number; width: number; height: number };

type MeetingTurnLike = {
  sequence: number;
  lane: string;
  source_language?: string;
  target_language?: string;
  translated_text: string;
  created_unix_ms?: number;
};
type MeetingTurnsLike = { ok: boolean; has_session: boolean; session_id: string | null; turns: MeetingTurnLike[]; };

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
    language: latest.target_language === "id" || latest.target_language === "en"
      ? latest.target_language
      : latest.lane === "incoming" ? "id" : "en",
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

export function overlayWidth(width: OverlayWidth): number {
  if (width === "compact") return 520;
  if (width === "wide") return 760;
  return 620;
}

// Grow to a bounded viewport based on rendered text; never shrink user-selected type.
export function captionHeightForContent(contentHeight: number, size: OverlayTextSize, collapsed = false): number {
  if (collapsed) return overlayHeightForTextSize(size, true);
  const measured = Number.isFinite(contentHeight) ? Math.max(0, Math.ceil(contentHeight)) : 0;
  return Math.min(420, Math.max(overlayHeightForTextSize(size), measured + 76));
}

// Convert physical monitor work area to logical window width with a modest safety margin.
export function fitOverlayWidth(desiredLogicalWidth: number, physicalWorkAreaWidth: number, scaleFactor: number): number {
  if (!Number.isFinite(scaleFactor) || scaleFactor <= 0 || !Number.isFinite(physicalWorkAreaWidth) || physicalWorkAreaWidth <= 0) return desiredLogicalWidth;
  return Math.max(260, Math.min(desiredLogicalWidth, Math.floor(physicalWorkAreaWidth / scaleFactor) - 32));
}

// Work areas are physical pixels; native window sizing uses logical pixels.
// A compact screen can force scrolling, but we never shrink text to fit.
export function fitOverlayHeight(desiredLogicalHeight: number, workAreaPhysicalHeight: number, scaleFactor: number): number {
  if (!Number.isFinite(scaleFactor) || scaleFactor <= 0 || !Number.isFinite(workAreaPhysicalHeight) || workAreaPhysicalHeight <= 0) {
    return desiredLogicalHeight;
  }
  return Math.max(64, Math.min(desiredLogicalHeight, Math.floor(workAreaPhysicalHeight / scaleFactor) - 32));
}

export function isProgrammaticOverlayMove(position: PhysicalPoint, lastAutomaticPosition: PhysicalPoint | null): boolean {
  return lastAutomaticPosition !== null
    && Math.abs(position.x - lastAutomaticPosition.x) <= 2
    && Math.abs(position.y - lastAutomaticPosition.y) <= 2;
}

export function overlayFontSize(size: OverlayTextSize): number {
  if (size === "small") return 18;
  if (size === "large") return 26;
  if (size === "extra-large") return 30;
  return 22;
}

export function intersectionArea(a: PhysicalRect, b: PhysicalRect): number {
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


export type RecentCaptionEntry = {
  sequence: number;
  lane: string;
  text: string;
  language: string;
  createdUnixMs: number;
};

export function recentMeetingCaptions(snapshot: MeetingTurnsLike | null, limit = 20): RecentCaptionEntry[] {
  if (!snapshot?.ok || !snapshot.has_session || !snapshot.session_id) return [];
  const boundedLimit = Math.max(1, Math.min(20, Math.trunc(limit) || 20));
  return [...snapshot.turns]
    .filter((turn) => normalizeOverlayText(turn.translated_text).length > 0)
    .sort((left, right) => right.sequence - left.sequence)
    .slice(0, boundedLimit)
    .map((turn) => ({
      sequence: turn.sequence,
      lane: turn.lane,
      text: normalizeOverlayText(turn.translated_text),
      language: turn.target_language === "id" || turn.target_language === "en"
        ? turn.target_language
        : turn.lane === "incoming" ? "id" : "en",
      createdUnixMs: "created_unix_ms" in turn && typeof turn.created_unix_ms === "number" ? turn.created_unix_ms : 0,
    }));
}
