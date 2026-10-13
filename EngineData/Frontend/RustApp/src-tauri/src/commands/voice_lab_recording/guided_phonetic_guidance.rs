//! Cheap, deterministic and optional corpus guidance.
//! Uses US-English CMUdict-derived *text* hints, not microphone analysis or
//! proof of pronunciation, accent quality, phoneme mastery, or model quality.
use std::collections::HashSet;
use std::sync::OnceLock;
use super::{guided_lines, prompt_metadata};
use super::phonetic_inventory::FEATURES;
#[cfg(test)]
use super::phonetic_inventory::{PAIR_LABELS, PHONE_LABELS};

#[derive(Debug, Clone, serde::Serialize)]
pub(crate) struct RecordingSuggestion {
    pub line_id: u32,
    pub reason: &'static str,
}

fn text_fingerprint(text: &str) -> u64 {
    // FNV-1a over UTF-8 bytes. This guards against using stale pronunciation
    // hints after editing or reordering any prompt without regeneration.
    text.as_bytes().iter().fold(0xcbf29ce484222325_u64, |hash, &byte|
        (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3))
}

fn inventory_current() -> bool {
    static VALID: OnceLock<bool> = OnceLock::new();
    *VALID.get_or_init(|| {
        let prompts = guided_lines();
        prompts.len() == FEATURES.len() && prompts.iter().enumerate().all(|(index, (id, text))| {
            *id as usize == index + 1
                && FEATURES[index].0 == text_fingerprint(text)
                && FEATURES[index].3 <= 1000
        })
    })
}

/// Greedy ranked suggestions: first satisfy a missing legacy-compatible
/// category, then prefer new dictionary phonemes/within-word transitions and
/// a different sentence focus. Missing sounds are inferred from text ONLY.
/// No recommendation affects train eligibility, saved takes or dataset data.
pub(super) fn recommend_lines(
    accepted_line_ids: &[u32],
    required_coverage: Option<&str>,
    limit: usize,
) -> Vec<RecordingSuggestion> {
    if !inventory_current() || limit == 0 {
        return vec![]; // Unverifiable metadata must not invent guidance.
    }
    let prompts = guided_lines();
    let accepted: HashSet<u32> = accepted_line_ids.iter().copied().collect();
    let mut used_phonemes = 0_u64;
    let mut used_pairs = 0_u64;
    let mut used_focus: HashSet<&'static str> = HashSet::new();
    for id in &accepted {
        let Some(index) = id.checked_sub(1).map(|v| v as usize) else { continue; };
        if let Some((_, phones, pairs, _)) = FEATURES.get(index) {
            used_phonemes |= *phones;
            used_pairs |= *pairs;
        }
        if let Some(meta) = prompt_metadata(*id) {
            used_focus.insert(meta.focus);
        }
    }
    let novice = accepted.len() < 8;
    let mut picks = Vec::new();
    let mut excluded = accepted;
    for _ in 0..limit.min(5) {
        let mut best: Option<(i32, u32, u64, u64, &'static str, &'static str)> = None;
        for &(id, _) in prompts {
            if excluded.contains(&id) { continue; }
            let Some(meta) = prompt_metadata(id) else { continue; };
            if required_coverage.is_some_and(|group| meta.coverage != group) { continue; }
            // Do not lead a first-time speaker to the longest passages simply
            // because they contain more different dictionary sound patterns.
            // Required eligibility categories remain exempt from this limit.
            if required_coverage.is_none() && novice {
                if accepted_line_ids.len() < 4 && meta.level != "short" { continue; }
                if meta.level == "detailed" { continue; }
            }
            let (_, phones, pairs, ratio) = FEATURES[(id - 1) as usize];
            if ratio < 750 { continue; } // Prefer dictionary-backed estimates.
            let phone_gain = (phones & !used_phonemes).count_ones().min(12) as i32;
            let pair_gain = (pairs & !used_pairs).count_ones().min(16) as i32;
            let novel_focus = !used_focus.contains(meta.focus);
            let early_readability = match (novice, meta.level) {
                (true, "short") => 20,
                (true, "natural") => 9,
                _ => 0,
            };
            let uncertainty_penalty = if ratio < 950 { 8 } else { 0 };
            let score = phone_gain * 4 + pair_gain * 2
                + (if novel_focus { 8 } else { 0 })
                + early_readability - uncertainty_penalty;
            if required_coverage.is_none() && score <= 0 {
                continue; // No useful marginal text coverage to recommend.
            }
            if best.as_ref().is_none_or(|(best_score, best_id, ..)|
                score > *best_score || (score == *best_score && id < *best_id))
            {
                best = Some((score, id, phones, pairs, meta.focus, meta.coverage));
            }
        }
        let Some((_, id, phones, pairs, focus, _)) = best else { break; };
        let reason = if required_coverage.is_some() {
            "Adds a missing conversation category."
        } else if (phones & !used_phonemes) != 0 {
            "Adds different sound patterns (estimated from text)."
        } else if (pairs & !used_pairs) != 0 {
            "Adds less-represented sound sequences (text estimate)."
        } else {
            "Adds a different sentence style."
        };
        picks.push(RecordingSuggestion { line_id: id, reason });
        excluded.insert(id);
        used_phonemes |= phones;
        used_pairs |= pairs;
        used_focus.insert(focus);
    }
    picks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phonetic_inventory_is_exactly_bound_to_all_six_hundred_current_texts() {
        assert!(inventory_current());
        assert_eq!(FEATURES.len(), 600);
        assert_eq!(PHONE_LABELS.len(), 39);
        assert_eq!(PAIR_LABELS.len(), 64);
        assert!(FEATURES.iter().all(|(_, _, _, ratio)| *ratio <= 1000));
    }

    #[test]
    fn beginner_guidance_starts_with_short_readable_sentences() {
        let hints = recommend_lines(&[], None, 3);
        assert!(!hints.is_empty());
        assert!(hints.iter().all(|suggestion|
            prompt_metadata(suggestion.line_id).unwrap().level == "short"));
    }

    #[test]
    fn uncovered_category_has_deterministic_no_duplicate_suggestions() {
        let recorded = [1, 2, 3, 4, 5];
        let result = recommend_lines(&recorded, Some("details"), 3);
        assert!(!result.is_empty());
        assert!(result.iter().all(|suggestion| {
            prompt_metadata(suggestion.line_id).unwrap().coverage == "details"
                && !recorded.contains(&suggestion.line_id)
        }));
        let ids: HashSet<u32> = result.iter().map(|item| item.line_id).collect();
        assert_eq!(ids.len(), result.len());
        assert_eq!(result.iter().map(|s| s.line_id).collect::<Vec<_>>(),
            recommend_lines(&recorded, Some("details"), 3)
                .iter().map(|s| s.line_id).collect::<Vec<_>>());
    }

    #[test]
    fn generated_data_never_forces_a_pronunciation_or_overrides_eligibility() {
        let all_ids = (1..=600).collect::<Vec<_>>();
        assert!(recommend_lines(&all_ids, None, 3).is_empty());
        assert!(recommend_lines(&[1, 2, 3], Some("not-a-category"), 3).is_empty());
        assert!(recommend_lines(&[1], None, 0).is_empty());
    }
}
