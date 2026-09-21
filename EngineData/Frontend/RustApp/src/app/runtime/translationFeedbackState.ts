export type TranslationIssueCategory =
  | "meaning_changed"
  | "wrong_terminology"
  | "name_number"
  | "incomplete"
  | "unnatural";

export type TranslationFeedbackEntry = {
  id: string;
  category: TranslationIssueCategory;
  source: string;
  translation: string;
  sourceLanguage: string;
  targetLanguage: string;
  createdAt: number;
};

const KEY = "translateit.translation-feedback.v1";
const MAX_ENTRIES = 40;
const MAX_TEXT_CHARS = 2000;

export function readTranslationFeedback(): TranslationFeedbackEntry[] {
  try {
    const raw = localStorage.getItem(KEY);
    const parsed = raw ? JSON.parse(raw) : [];
    return Array.isArray(parsed) ? parsed.slice(0, MAX_ENTRIES) : [];
  } catch {
    return [];
  }
}

export function saveTranslationFeedback(entry: Omit<TranslationFeedbackEntry, "id" | "createdAt">): void {
  const next: TranslationFeedbackEntry = {
    ...entry,
    source: entry.source.slice(0, MAX_TEXT_CHARS),
    translation: entry.translation.slice(0, MAX_TEXT_CHARS),
    id: `${Date.now()}-${Math.random().toString(36).slice(2, 8)}`,
    createdAt: Date.now(),
  };
  try { localStorage.setItem(KEY, JSON.stringify([next, ...readTranslationFeedback()].slice(0, MAX_ENTRIES))); } catch {}
}

export function clearTranslationFeedback(): void {
  try { localStorage.removeItem(KEY); } catch {}
}
