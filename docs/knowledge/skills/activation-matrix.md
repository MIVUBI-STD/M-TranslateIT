---
id: document.knowledge.skills.activation-matrix
class: DOCUMENT
domain: knowledge
role: GUIDE
authority: CANONICAL
lifecycle: ACTIVE
---

# Skill Activation Matrix

Root `AGENTS.md` owns context/mode/budget. This file answers **when one TranslateIT specialist adds semantic value**.

## Budget

```text
Bounded Maintenance → zero/one specialist
Standard Development → zero/one specialist
Complex Development → development-brief + zero/one specialist
Plan / Recovery → none by default
```

Framework documentation, autofixers, testing/profiling tools and external research helpers are tools/procedures, not extra project specialists.

## Routing

| Semantic boundary | Specialist |
|---|---|
| Complex/ambiguous development contract | `development-brief` |
| Desktop shell/navigation/readiness/settings/frontend-runtime facade | `desktop-runtime-development` |
| Visual hierarchy/layout/tokens/component states/rendered acceptance | `desktop-ui-design-development` |
| ASR/translation/TTS/model/provider/AI worker/CUDA behavior | `local-ai-runtime-development` |
| Physical mic/capture/VAD/Windows devices/Meeting route/delivery | `windows-audio-runtime-development` |
| Installer/private Python/runtime assets/models/audio-provider delivery | `release-packaging-development` |

## Activation and scope isolation

An assigned semantic scope is not automatic permission to load its Skill. A known bounded owner may be handled directly. Load a single specialist only when its reusable procedure materially changes the decision; `complex-development` additionally needs the existing `development-brief`. Do not preload sibling specialist Skills, and never create a generic manager/agent/router just to select existing ones.

Cross-domain diagnosis stays with the originating scope until evidence establishes a distinct target. A handoff carries observed vs expected, minimum reproduction/evidence, named target scope and resume stage. Handoff output is advisory: it never switches active specialist, authorizes target mutation or converts an UNKNOWN into an asserted product bug. Current context projection is read-only in `tools/repository_context.py` and source/boundary meaning remains in root `AGENTS.md` and the canonical domain owner.

## Current product terminology

Specialists must use current product law:

```text
one canonical translation pipeline
outbound rolling context: last 3 committed own-voice pairs
incoming: context-free
Session Listening only
Built-in Male/Female Meeting voice available day one
My Voice optional trained upgrade
Svelte 5 is current frontend architecture
```

`Realtime/Quality` user modes, Push to Talk, document translation and a pending vanilla→Svelte migration are retired concepts, not active specialist routing.

## Procedure evaluation boundary

Every domain specialist remains governed by its own `## Procedure` and `## Proof` section. The existing `.agents/evals/skill-procedure.json` tests safe ownership, no-skill, cross-owner handoff and proof-claim boundaries without adding a seventh specialist. `development-brief` remains the complex-development meta procedure. Static admission and externally scored receipts never authorize a lane/scope switch.

## Selection test

Before loading a specialist ask:

1. What exact behavior/contract is wrong or changing?
2. Which semantic owner remains responsible if the implementation language changes?
3. Does this specialist add reusable domain judgment beyond root rules?
4. Is one specialist sufficient for the current acceptance boundary?

If not, do not load it.

## Multi-domain symptoms

Choose the cause, not the visible file.

```text
worker reports route_missing correctly, UI says Ready
→ desktop-runtime-development

device/route detection itself wrong
→ windows-audio-runtime-development

valid finalized segment exists, ASR ignores it
→ local-ai-runtime-development

runtime works in dev, installer omits required asset
→ release-packaging-development
```

A second independent problem becomes a separate bounded task rather than a stacked-specialist session.
