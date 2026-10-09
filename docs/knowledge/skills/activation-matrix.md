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
