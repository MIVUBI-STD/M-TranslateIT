// A Text result can be reused only when it still belongs to the current input
// and translation settings. This controls copy, alternative wording, feedback
// and floating captions; it does not store translation history.
export type TextResultState = "idle" | "translating" | "success" | "review" | "edited" | "stale" | "error";

export type TextResultContext = {
  source: string;
  target: string;
  translatedSource: string | null;
  translatedSettingsKey: string | null;
  currentSettingsKey: string;
  state: TextResultState;
};

export function canUseCurrentTextResult(context: TextResultContext): boolean {
  if (!["success", "review", "edited"].includes(context.state)) return false;
  if (!context.translatedSource?.trim() || !context.translatedSettingsKey) return false;
  if (!context.target.trim()) return false;
  return context.source.trim() === context.translatedSource
    && context.translatedSettingsKey === context.currentSettingsKey;
}
