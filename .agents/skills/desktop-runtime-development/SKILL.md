---
name: desktop-runtime-development
description: TranslateIT specialist for the current Tauri 2 + Svelte 5 desktop product/runtime boundary: shell, navigation, lifecycle, readiness/state mapping, settings integration and frontend-to-Rust runtime facade. Do not use for generic visual styling or AI/audio/package internals.
---

# Desktop Runtime Development

## Current architecture

```text
Tauri 2
+ Svelte 5
+ Vite
+ TypeScript
```

This is current source architecture, not a pending migration. `src/App.svelte`, `src/pages/`, current bridges/facades and Rust/Tauri owners are authoritative.

## Owns

- application shell/lifecycle;
- Meeting/Text/My Voice/Settings navigation and workspace composition;
- product-level readiness/capability mapping;
- settings integration;
- frontend↔Tauri bridge/facade ownership;
- product recovery actions;
- Normal UI vs Advanced/Diagnostics boundary.

## Does not own

AI inference/model behavior; physical capture/VAD/device routing; installer/package delivery; generic visual spacing/typography/color craft.

## Rules

- Svelte presentation state must not duplicate Rust/runtime product truth.
- Preserve one application root and current facade boundaries unless a concrete contract defect requires change.
- No SvelteKit/router/Redux-like state framework by default.
- Do not reimplement AI/audio/Meeting session semantics in frontend code.
- Capability readiness is granular; one optional capability must not collapse the entire product into unavailable.
- `command exists`, `helper started`, `device discovered` or `config exists` is not product readiness.
- Current built-in Meeting voice selection and optional My Voice state are product behavior, not future migration work.

## Procedure

1. Ground expected product behavior.
2. Identify current shell/facade/Rust owner.
3. Identify underlying capability contract.
4. Separate runtime truth from presentation state.
5. Make the minimum complete owner change.
6. Check callers/state/action mapping and duplicate ownership.
7. Use Svelte official validation helpers when actual Svelte changes are made and capability exists.
8. Apply proof matching the claim.

## Proof

Root `GITHUB_RULES.md` controls ChatGPT/cloud-only execution and the **integrated test-ready candidate** gate. Do not assign the user's PC as an incremental test runner; name unexecuted native-only proof without claiming PASS.

Static/GitHub proof can establish wiring/ownership/state mapping. Compile/type/build claims require actual tooling. Native Tauri interaction and rendered behavior require corresponding local/target evidence when material.
