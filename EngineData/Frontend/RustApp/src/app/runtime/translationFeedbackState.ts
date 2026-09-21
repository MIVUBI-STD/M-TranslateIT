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
  const id = item["id"];
  const category = item["category"];
  const source = item["source"];
  const translation = item["translation"];
  const sourceLanguage = item["sourceLanguage"];
  const targetLanguage = item["targetLanguage"];
  const createdAt = item["createdAt"];
  if (
    typeof id !== "string"
    || typeof category !== "string"
    || !VALID_CATEGORIES.has(category as TranslationIssueCategory)
    || typeof source !== "string"
    || typeof translation !== "string"
    || typeof sourceLanguage !== "string"
    || typeof targetLanguage !== "string"
    || typeof createdAt !== "number"
    || !Number.isFinite(createdAt)
  ) return null;
  return {
    id: id.slice(0, 80),
    category: category as TranslationIssueCategory,
    source: source.slice(0, MAX_TEXT_CHARS),
    translation: translation.slice(0, MAX_TEXT_CHARS),
    sourceLanguage: sourceLanguage.slice(0, 16),
    targetLanguage: targetLanguage.slice(0, 16),
    createdAt,
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

export function saveTranslationFeedback(entry: Omit<TranslationFeedbackEntry, "id" | "createdAt">): boolean {
  const next: TranslationFeedbackEntry = {
    ...entry,
    source: entry.source.slice(0, MAX_TEXT_CHARS),
    translation: entry.translation.slice(0, MAX_TEXT_CHARS),
    id: `${Date.now()}-${Math.random().toString(36).slice(2, 8)}`,
    createdAt: Date.now(),
  };
  try {
    localStorage.setItem(KEY, JSON.stringify([next, ...readTranslationFeedback()].slice(0, MAX_ENTRIES)));
    return true;
  } catch {
    return false;
  }
}

export function clearTranslationFeedback(): boolean {
  try {
    localStorage.removeItem(KEY);
    return true;
  } catch {
    return false;
  }
}
