import type { MeetingCommittedTurn } from "../bridge/runtimeApi";

export function meetingSpeakerLabel(lane: string): string {
  return lane === "you" ? "You" : "Person 1";
}

export function meetingSpeakerDescription(lane: string): string {
  return lane === "you" ? "Your microphone" : "Combined incoming audio; speakers are not identified";
}

export function meetingTime(unixMs: number | null | undefined): string {
  if (typeof unixMs !== "number" || !Number.isFinite(unixMs) || unixMs <= 0) return "Time unavailable";
  return new Intl.DateTimeFormat("en-GB", { hour: "2-digit", minute: "2-digit" }).format(new Date(unixMs));
}

export function meetingSessionTitle(unixMs: number | null | undefined): string {
  if (typeof unixMs !== "number" || !Number.isFinite(unixMs) || unixMs <= 0) return "Meeting session";
  const label = new Intl.DateTimeFormat("en-GB", {
    day: "numeric", month: "short", year: "numeric", hour: "2-digit", minute: "2-digit",
  }).format(new Date(unixMs));
  return `Meeting · ${label}`;
}

export function meetingFirstRecordedTime(turns: readonly MeetingCommittedTurn[]): number | null {
  // Oldest committed turn is sufficient for display, not an exact meeting start time.
  return turns.reduce<number | null>((earliest, turn) =>
    Number.isFinite(turn.created_unix_ms) && turn.created_unix_ms > 0
      ? earliest === null ? turn.created_unix_ms : Math.min(earliest, turn.created_unix_ms)
      : earliest, null);
}

/** Missing AI output is a distinct status; never render it as translated text. */
export function meetingTranslationDisplay(
  turn: Pick<MeetingCommittedTurn, "translation_state" | "translated_text">,
): string {
  if (turn.translation_state === "failed") return "Translation unavailable";
  if (turn.translation_state === "pending") return "Translating...";
  return turn.translated_text.trim() || "Translation unavailable";
}
