# TranslateIT Agent Skills

This is a navigation entry, not an agent, work-state registry or second set of instructions. Root [AGENTS.md](../AGENTS.md) selects one work mode and [GITHUB_RULES.md](../GITHUB_RULES.md) governs GitHub/cloud-only execution, atomic commits, proof and STOP.

## Existing six skills

- `development-brief` — complex/ambiguous scope contract
- `desktop-runtime-development` — Tauri/Svelte application runtime ownership
- `desktop-ui-design-development` — visual UI/design ownership
- `local-ai-runtime-development` — canonical ASR/translation/TTS worker
- `windows-audio-runtime-development` — microphone/VAD/virtual audio device
- `release-packaging-development` — offline install/runtime assets/release

The canonical identities/classes are in [skill-registry.json](./skill-registry.json); detailed procedures live only in their matching `skills/*/SKILL.md`. Before a write, apply the existing [permission policy](./permissions/permission-policy.json) through `tools/repository_permissions.py`. The existing evaluation corpora in [evals](./evals/manifest.json) measure declared routing/procedure/permission expectations; synthetic receipts never demonstrate real model accuracy.

## Activation budget

Context Recovery and Plan load no specialist by default. Bounded and Standard Development use at most one specialist. Complex Development uses the existing brief plus at most one domain specialist, only when needed. Semantic ownership selects the skill; language, framework or library do not. Cross-domain symptoms produce a bounded proposed handoff, **not** automatic specialist switching.

Skills are data-only and subject to `tools/repository_skill_admission.py` checks. No seventh generic agent, alternate registry, automated instruction execution or user-PC testing handoff. See [Skill Map](../docs/knowledge/skills/skill-map.md) for reusable selection details.
