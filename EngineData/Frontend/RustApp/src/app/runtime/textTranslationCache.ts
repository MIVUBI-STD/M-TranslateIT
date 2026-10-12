import type { RuntimeSettings } from "../shared/types";

export type TextTranslationCacheEntry = {
  key: string;
  translated: string;
  reviewHints: string[];
  needsReview: boolean;
};

const MAX_ENTRIES = 32;
const entries: TextTranslationCacheEntry[] = [];

export function textTranslationCacheKey(source: string, settings: RuntimeSettings): string {
  return JSON.stringify([
    source.trim(),
    settings.source_language,
    settings.target_language,
  ]);
}

export function getCachedTextTranslation(key: string): TextTranslationCacheEntry | null {
  const index = entries.findIndex((entry) => entry.key === key);
  if (index < 0) return null;
  const hit = entries[index];
  if (!hit) return null;
  entries.splice(index, 1);
  entries.unshift(hit);
  return { ...hit, reviewHints: [...hit.reviewHints] };
}

export function putCachedTextTranslation(entry: TextTranslationCacheEntry): void {
  const index = entries.findIndex((item) => item.key === entry.key);
  if (index >= 0) entries.splice(index, 1);
  entries.unshift({ ...entry, reviewHints: [...entry.reviewHints] });
  if (entries.length > MAX_ENTRIES) entries.length = MAX_ENTRIES;
}

