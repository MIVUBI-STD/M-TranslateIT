---
name: desktop-ui-design-development
description: TranslateIT specialist for current Svelte desktop visual/UI design: hierarchy, layout, spacing, typography, tokens, component states, accessibility, motion, responsive composition and rendered visual acceptance. Do not use for runtime truth, AI/audio/package behavior or architecture migration.
---

# Desktop UI Design Development

## Current visual stack

```text
Svelte 5
+ Tailwind CSS 4
+ CSS custom-property semantic tokens
+ selective Bits UI
+ Lucide Svelte
```

This stack is already current. Do not describe vanilla→Svelte migration as pending.

## Owns

- visual hierarchy/composition;
- spacing/density/alignment;
- typography and readable contrast;
- semantic colors/radii/elevation/tokens;
- visible interaction states;
- responsive/resizable desktop composition;
- visual accessibility and reduced-motion behavior;
- rendered visual acceptance.

## Does not own

Navigation semantics, readiness truth, AI/audio behavior, persistence, installer/package behavior or product requirements.

## Working modes

`ALIGN` when a coherent visual language exists; `ESTABLISH` only when the surface genuinely lacks one or the user explicitly approves replacement.

For external references separate:

```text
OBSERVED
INFERRED
ADOPTED
```

A screenshot is evidence, not automatic policy.

## Rules

Prioritize task/state hierarchy before decoration. Reject generic AI gradients/glow, gratuitous glassmorphism, unnecessary card grids, component inflation, animation without a job and large UI-library adoption without a current need.

Material states should cover only states that can actually occur: default, focus, active/selected, checking/loading, ready/success, degraded/setup-needed/error, disabled/unavailable.

## Procedure

1. Ground the current page, user task and real interaction states in Svelte source.
2. Reuse existing semantic tokens, component patterns and accessibility boundaries before adding UI structure.
3. Separate visual styling and state presentation from navigation, readiness and backend ownership.
4. Change only the owning UI surfaces; inspect responsive, focus, disabled, degraded and reduced-motion cases as relevant.
5. Use rendered evidence at the required window sizes for visual claims; static CSS inspection is not visual acceptance.
6. Stop after the scoped visual acceptance. If runtime truth is wrong, document evidence for a separate desktop-runtime handoff instead of rewriting it here.

## Proof

Static proof can establish style ownership/tokens/markup. Claims such as visual match, responsive composition, motion feel or rendered accessibility require rendered evidence at relevant dimensions/context.
