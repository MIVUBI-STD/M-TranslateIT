# TranslateIT Skill Map

Canonical project skill root:

Machine-readable skill **identity and classification** is owned by [the registry](../../../.agents/skill-registry.json). This document remains the explanatory routing map. Permission decisions are tested using [the policy](../../../.agents/permissions/permission-policy.json) and [evaluation cases](../../../.agents/evals/permission-cases.json). These do not add project skills or new runtime owners.

```text
.agents/skills/
```

Approved baseline is intentionally small:

```text
development-brief
desktop-runtime-development
desktop-ui-design-development
local-ai-runtime-development
windows-audio-runtime-development
release-packaging-development
```

## Roles

### development-brief

Complex/ambiguous development front door. Defines goal, success metric, forbidden proxy, first evidence, semantic owner, execution partition, scope, acceptance, proof and STOP condition before implementation.

### desktop-runtime-development

Current Tauri 2 + Svelte 5 desktop product/runtime shell, navigation, readiness/state mapping, settings integration and frontend↔Rust runtime facade.

### desktop-ui-design-development

Current Svelte/Tailwind/tokens/Bits UI/Lucide visual hierarchy, layout, component states, accessibility, motion and rendered visual acceptance.

### local-ai-runtime-development

Canonical Python worker, ASR, Indonesian↔English translation, bounded outbound context, context-free incoming translation, GPT-SoVITS voice inference, built-in voice runtime and My Voice training/inference.

### windows-audio-runtime-development

Continuous Session Listening capture, finalized utterances, VAD/segmentation, Windows devices, Meeting Sound, virtual route and TranslateIT Meeting Microphone delivery.

### release-packaging-development

Offline Setup/payload, private Python runtime, controlled models/voice assets, provider delivery, notices/provenance, installed layout and clean-machine proof.

## Frozen baseline rule

Do not add/rename/split/duplicate a project skill because a new framework/library/model/tool appears. A new skill requires evidence of a genuinely distinct reusable semantic capability that existing owners cannot represent without overlap.

Do not create generic `rust-expert`, `python-expert`, `svelte-expert`, `researcher`, `tester`, `profiler`, `anti-slop` or similar project skills.

## Technical helpers

Official Svelte documentation/autofixers, current primary-source library documentation, Ruff/pytest/profilers and other development tools may be used when the current semantic owner proves a need. They do not consume the one-specialist budget and do not become product authority.

## Current architecture guard

The active frontend already uses Svelte 5. The product already removed Realtime/Quality user modes, Push to Talk and Document Translation. Skill text that describes those as current architecture/product behavior is stale and must be fixed before using it as execution authority.
