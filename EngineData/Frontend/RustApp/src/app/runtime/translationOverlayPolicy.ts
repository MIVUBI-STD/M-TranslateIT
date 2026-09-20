export const TRANSLATION_OVERLAY_EVENT = "translation-overlay:update";

export type TranslationOverlayPayload = {
  text: string;
  language: string;
  source: "text" | "meeting";
  revision: string;
};

type MeetingTurnLike = {
  sequence: number;
  lane: string;
  translated_text: string;
  updated_unix_ms: number;
};

type MeetingTurnsLike = {
  ok: boolean;
  has_session: boolean;
  session_id: string | null;
  turns: MeetingTurnLike[];
};

export function normalizeOverlayText(value: string): string {
  return value.replace(/\r\n/g, "\n").trim();
}

export function latestMeetingCaption(snapshot: MeetingTurnsLike | null): TranslationOverlayPayload | null {
  if (!snapshot?.ok || !snapshot.has_session || !snapshot.session_id) return null;

  const latest = snapshot.turns
    .filter((turn) => normalizeOverlayText(turn.translated_text).length > 0)
    .reduce<MeetingTurnLike | null>((current, turn) => {
      if (!current) return turn;
      return turn.sequence > current.sequence ? turn : current;
    }, null);

  if (!latest) return null;

  return {
    text: normalizeOverlayText(latest.translated_text),
    language: latest.lane === "incoming" ? "id" : "en",
    source: "meeting",
    revision: `${snapshot.session_id}:${latest.sequence}:${latest.updated_unix_ms}`,
  };
}
