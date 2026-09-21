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

const VALID_CATEGORIES = new Set<TranslationIssueCategory>([
  "meaning_changed",
  "wrong_terminology",
  "name_number",
  "incomplete",
  "unnatural",
]);

function validFeedback(value: unknown): TranslationFeedbackEntry | null {
  if (!value || typeof value !== "object") return null;
  const item = value as Record<string, unknown>;
  if (
    typeof item.id !== "string"
    || typeof item.category !== "string"
    || !VALID_CATEGORIES.has(item.category as TranslationIssueCategory)
    || typeof item.source !== "string"
    || typeof item.translation !== "string"
    || typeof item.sourceLanguage !== "string"
    || typeof item.targetLanguage !== "string"
    || typeof item.createdAt !== "number"
    || !Number.isFinite(item.createdAt)
  ) return null;
  return {
    id: item.id.slice(0, 80),
    category: item.category as TranslationIssueCategory,
    source: item.source.slice(0, MAX_TEXT_CHARS),
    translation: item.translation.slice(0, MAX_TEXT_CHARS),
    sourceLanguage: item.sourceLanguage.slice(0, 16),
    targetLanguage: item.targetLanguage.slice(0, 16),
    createdAt: item.createdAt,
  };
}

export function readTranslationFeedback(): TranslationFeedbackEntry[] {
  try {
    const raw = localStorage.getItem(KEY);
    const parsed = raw ? JSON.parse(raw) : [];
    if (!Array.isArray(parsed)) return [];
    return parsed.map(validFeedback).filter((value): value is TranslationFeedbackEntry => value !== null).slice(0, MAX_ENTRIES);
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
