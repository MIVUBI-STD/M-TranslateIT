# TranslateIT Tauri Desktop Application

This directory is the current desktop application package on branch `Local`.

## Frontend Ownership

```text
index.html
-> src/main.ts
-> one Svelte mount
-> src/App.svelte
   ├─ pages/FirstSetup.svelte
   ├─ pages/Meeting.svelte
   ├─ pages/Text.svelte
   ├─ pages/MyVoice.svelte
   └─ pages/Settings.svelte
```

Runtime boundaries remain separate:

```text
src/app/bridge/runtimeProductFacade.ts
-> product-readable state/actions

src/app/bridge/runtimeApi.ts
-> Tauri command boundary

src/app/bridge/myVoiceApi.ts + myVoiceBuildApi.ts
-> My Voice recording/build command boundary

src-tauri/src/commands/
-> Rust desktop/runtime commands
```

The previous manual DOM controller/template-string frontend is removed from the active source graph rather than retained as a parallel compatibility shell.

## Frontend Stack

Approved source stack:

```text
Tauri 2
Svelte 5
Vite
TypeScript
Tailwind CSS 4
CSS custom-property design tokens
selective Bits UI
Lucide Svelte
```

No SvelteKit, frontend router, Redux-like state library, heavy UI framework, CSS-in-JS, or general animation framework is required for the current product.

`styles/tokens.css` owns the small semantic token set. `styles/app.css` owns Tailwind loading, base desktop rules, focus behavior, reduced-motion behavior, and a few justified shared component classes.

## Product Surface

Normal UI is Meeting / Text / My Voice / Settings, with Setup shown when required. Audio Studio, History/Saved, Documents, tone/mode controls, and developer pipeline controls are not initial core.

Meeting lifecycle, settings persistence, audio truth, model/provider truth, My Voice training state, and translation execution remain owned by the existing Rust/Python runtime. Svelte state is presentation/application state only.

## Dependency / Proof Boundary

Frontend dependencies and `package-lock.json` are current repository-owned inputs. Source changes require the existing Svelte typecheck/build and code-health gates before they are treated as verified.

Target-PC behavior still requires the Rust/Tauri/Python/audio/installer proof described by the canonical project docs; repository/hosted checks do not substitute for installed Windows acceptance.

Resume work through root `AGENTS.md`, `CONTEXT.md`, and `planning/development.md`.
