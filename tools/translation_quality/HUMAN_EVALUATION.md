# Human Translation Evaluation

This protocol is the release-quality linguistic review for TranslateIT Indonesian ↔ English translation. It complements the automated regression corpus; it does not replace it.

## Purpose

Use human evaluation when changing the translation model, prompt, context policy, terminology behavior, decoding behavior, or segmentation in a way that could change linguistic output.

## Evaluation set

Use a held-out bilingual set that is not used to tune the candidate. Include both directions and representative samples from:

- normal conversational Indonesian and English;
- online-meeting requests, corrections and follow-ups;
- numbers, dates, currencies and units;
- named entities, product names and exact literals;
- technical language and Indonesian/English code-switching;
- negation, modality, conditionals and quantifier scope;
- spoken disfluency and self-correction;
- contextual outbound Meeting turns;
- terminology-controlled cases.

The 65-case automated corpus may seed risk discovery, but the final human set should contain unseen wording.

## Blind review

Reviewers must not be told which output is baseline or candidate. Randomize output order per item.

At least one reviewer must be fluent in Indonesian and English. For a release-changing model/prompt decision, use two independent bilingual reviewers when practical and adjudicate material disagreements.

## Scoring

Score each output from 1–5 on four dimensions:

1. **Meaning fidelity** — preserves the intended proposition, negation, modality, references and conditions.
2. **Fact/literal fidelity** — preserves names, numbers, dates, units, URLs, versions and required technical literals.
3. **Naturalness** — sounds natural and understandable in the target language without changing meaning.
4. **Terminology consistency** — respects applicable preferred terminology without awkward blind replacement.

A factual reversal, lost negation, changed number/date/unit, invented claim, or materially incomplete translation is a **critical error** regardless of average score.

## Pairwise decision

For each item, reviewers also choose:

- A better;
- B better;
- equivalent;
- both unacceptable.

Do not promote a candidate solely because its mean score is higher. Promotion requires:

- no new critical error in the held-out review;
- no meaningful regression in a high-risk category;
- pairwise evidence that the candidate is at least non-inferior overall;
- exact model/source identity recorded with the evaluation.

## Meeting-specific review

For contextual outbound cases, judge the current utterance with only the bounded context the product is authorized to provide. Do not give reviewers extra conversation history that the runtime would not have.

Incoming EN→ID and standalone Text remain context-free unless product policy changes.

## Recording evidence

Record:

- corpus/set fingerprint or immutable file revision;
- baseline model/source identity;
- candidate model/source identity;
- reviewer count;
- per-item scores and critical-error labels;
- pairwise preferences;
- adjudicated notes for material disagreements.

Human review is evidence for linguistic quality only. It does not prove Windows audio routing, ASR quality, TTS quality, GPU performance, or end-to-end Meeting latency.
