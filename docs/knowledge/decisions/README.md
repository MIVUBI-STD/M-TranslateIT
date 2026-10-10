---
id: document.knowledge.decisions.router
class: DOCUMENT
domain: knowledge
role: ROUTER
authority: CANONICAL
lifecycle: ACTIVE
---

# TranslateIT Decision Register

Durable decisions store **why a current rule exists**. Current product law belongs in `docs/foundation/`; active status belongs in `next-action.md`; proof belongs in `current-validation.md`; historical detail belongs in Git history or `history-legacy.md`.

Historical rationale is retained as [historical evidence](./history-legacy.md), not current product authority.

## Current decisions

### D-001 — Local-only repository authority

`Local` is the sole active repository authority for development, governance, CI, proof, continuation, and release-source validation.

Other branches are outside the current development lifecycle and are not fallback source, proof, continuation, PR, or promotion targets unless the user explicitly changes this policy.

Reason: the team explicitly works only from `Local`; repository mechanics must not create a second active authority or silently promote state elsewhere.

### D-002 — One desktop product and one canonical local worker

Keep one Tauri/Rust desktop application and one normal Python worker path for ASR, translation, and voice inference. Parallel normal engines/fallback stacks require a new explicit product decision.

### D-003 — Reliable translation before feature breadth

Meeting/Text reliability, completeness, understandable translation, and safe delivery outrank additional modes/workspaces/automation.

### D-004 — One Meeting lifecycle authority

The Rust Meeting session/runtime owner controls Ready → Starting → Live → Stopping → Ended, stale-work rejection, and safe Stop/Close behavior.

### D-011 — Svelte 5 desktop architecture is current

The active frontend is Svelte 5 + Vite + TypeScript inside Tauri 2, with Tailwind CSS 4, semantic CSS tokens, selective Bits UI, and Lucide Svelte. This is current architecture, not a pending migration.

### D-019 — CUDA preferred; capability failures remain truthful

Use validated acceleration when available. CPU/capability fallback handles named supported conditions only; unknown failures are not masked as healthy degradation.

### D-020 — My Voice is one trained GPT-SoVITS V2ProPlus upgrade

My Voice uses authorized guided recordings, held-out evaluation, and explicit approval. Training is occasional; normal Meeting inference reuses the approved actor.

### D-021 — Standard VB-CABLE is the initial Meeting audio-provider direction

Provider delivery is a release boundary; detection/configuration/use remains Windows-audio runtime ownership.

### D-022 — Shared GitHub operating discipline

TranslateIT follows the same reusable governance discipline as PRD-Creator/BuildIT, adapted to its Local-only branch model:

```text
PIN
→ EXECUTION CONTEXT
→ GitHub-first partition
→ READ MINIMUM
→ DIAGNOSE first wrong owner
→ TOOL + TRANSFER GATE
→ WRITE ONCE
→ VERIFY + FAILURE POLICY
→ STOP
```

Domain-specific proof contexts are `REMOTE_GITHUB | LOCAL_CODE | TARGET_WINDOWS`.

### D-023 — Offline Setup + colocated payload

Controlled distribution uses one user-facing Setup plus colocated payload files. Users do not manually operate Python/pip/core-model setup.

### D-033 — Asymmetric Meeting context

Outbound translation may use the last three committed own-voice translation pairs from the same session. Incoming meeting audio remains context-free.

### D-034 — Current product direction locks

- one canonical translation pipeline;
- no Realtime/Quality user modes;
- Text paste/type only; no document translation;
- Indonesian ↔ English only;
- full English UI;
- personal/owned-machine distribution;
- Built-in Male/Female Meeting voices available without training;
- My Voice optional after explicit approval.

### D-035 — Built-in voice source provenance

Built-in references are LibriSpeech/OpenSLR-derived CC-BY-4.0 material with exact source/utterance/hash metadata. They are **not** described as public-domain material.

### D-036 — Stability-first realtime latency architecture

**Context:** current outbound correctness is strong, but synchronous playback can make later utterances wait behind earlier audio, repeated native output setup may add jitter, repeated Start proof may cost warm-start time, and GPT-SoVITS supports fragment return that could reduce first-audio delay.

**Decision:** keep one canonical worker and one Meeting lifecycle owner. Optimize in isolated, evidence-gated phases: (A) one-ahead bounded AI→playback decoupling while retaining full-WAV delivery, (B) persistent per-generation native output stream only if delivery remains material, (C) functional AI proof rebinding only under exact runtime/model/actor identity plus invalidation epoch, and (D) GPT-SoVITS `return_fragment=True` only after safer phases when TTS remains dominant and target quality parity is proven. `docs/foundation/04-realtime-latency-architecture.md` owns the detailed architecture contract.

**Why:** this removes avoidable waiting while keeping translation/TTS quality, output ordering, at-most-once delivery, bounded memory, Stop semantics, and truthful readiness intact.

**Tradeoffs / not chosen:** reject unbounded pipelines, parallel TTS engines, a second worker, automatic uncertain replay, broad mid-session output recovery, `streaming_mode=True`, lower-quality fixed chunks, user-facing Realtime/Quality modes, and weakening Start checks to file presence.

**Evidence / proof boundary:** REMOTE_GITHUB can prove source bounds/order/identity/cleanup contracts. TARGET_WINDOWS is still required for real latency, native callback/device stability, meeting-app reception, resource pressure, and voice-quality claims.

**Follow-up owner:** `meeting_session.rs` for lifecycle/handoff, `engine/audio/meeting_output.rs` for playback/output ownership, `helper_bridge.rs` for proof identity/rebinding, and the canonical WorkerRuntime voice owner only if fragment delivery becomes evidence-justified.

### D-037 — Manual-only CI with Local as repository authority

**Context:** automatic push/PR verification repeatedly interrupted GitHub-first source development, while the GitHub default branch remained `main` although the source authority is `Local`.

**Decision (corrected by the user's explicit branch-lifecycle clarification):** all existing verification/release-source workflows use `workflow_dispatch` only; source/static acceptance uses proportional evidence and hosted checks are optional when available. `Local` owns active development; GitHub's default branch remains `main` and is reserved for finalized, approved results. Do not change the default branch, copy unfinished workflow files to `main`, or promote unaccepted source merely to enable manual dispatch. Retain the eight existing verification owners and security/release contracts.

**Why:** prevent CI queues and repeated runs from becoming a completion prerequisite. Avoid duplicate workflows, trigger-only commits, and parallel branch authority.

**Tradeoffs:** verification no longer runs automatically on each change. Because the workflow files currently exist only on `Local`, normal GitHub manual dispatch may remain unavailable until an authorized final/release process establishes them on the default branch. Exact-SHA executed CI and native Windows acceptance are still required for claims that inherently need those proof levels; unavailable checks must be reported as missing, never fabricated.

**Owner:** `GITHUB_RULES.md`, `.github/workflows/`, and the repository CI verifier; GitHub settings are external administration.

### D-038 — Source-grounded, commit-based work continuity

**Context:** source development crosses short ChatGPT/Codex sessions, while plain chat history and a mutable active-next-action document cannot safely reconstruct every prior decision.

**Decision:** material logical commits record `Work`, `State`, and actual `Proof`, with `Decision`, `Unresolved`, and `Next` only when applicable. Cold start is read-only, recovers relevant topic commits, and verifies their meaning against current canonical source before resuming. Current active continuation remains owned by `docs/knowledge/next-action.md`; Git history owns older revision-scoped evidence.

**Why:** recover development accurately without extra state databases, heartbeat commits, duplicate TODO documents, or reliance on inaccessible chat context.

**Tradeoffs:** commit metadata is not an executable test, automatic issue tracker, or permission to resume another lane. Legacy commits without trailers are interpreted only from their source diff and actual evidence.

**Owner:** `GITHUB_RULES.md`, `AGENTS.md`, and existing continuation documentation.

### D-039 — Evaluate existing agent routing without inventing new skills

**Context:** TranslateIT already has six canonical project skills and a path/action permission corpus; a skill registry alone does not prove correct model behavior on bilingual, negative or cross-owner requests.

**Decision:** retain the six current skills. Add provider-neutral routing expectations and a read-only scorer for external observed decisions, while preserving permission evaluation as a separate canonical owner. Neither synthetic scoring nor registry presence is model execution evidence.

**Why:** improve misrouting detection without spawning an extra router, general-purpose agent, state database or duplicated specialist policy.

**Tradeoffs:** actual routing accuracy remains unmeasured until real provider-run receipts are supplied; source/corpus verification has a lower proof ceiling.

**Owner:** `AGENTS.md`, `.agents/evals/`, `tools/repository_agent_evals.py`.

### D-040 — Ephemeral source dependency graph, not a second runtime owner

**Context:** the bounded impact planner knew only four explicit interop contracts and literal test references, so normal Rust/TypeScript/Python import consumers were not visible during affected verification.

**Decision:** derive a bounded, in-memory source-import graph from current source only, and integrate it with the existing `tools/repository_impact.py` and canonical `tools/interop-contracts.json`. Do not persist a graph, create a second source registry, or infer runtime/type semantics from static imports.

**Why:** surface transitive consumers for targeted tests while preserving immutable canonical source and the one-owner boundary.

**Tradeoffs:** unknown/dynamic imports, macro-generated dependencies and protocol semantics remain partial evidence requiring conservative review or broader verification; result is never executable proof.

**Owner:** `tools/repository_dependencies.py` parses source; `tools/repository_impact.py` owns affected verification planning.

### D-041 — Explicit affected execution with exact-SHA source evidence

**Context:** the source-dependency graph and existing cross-language impact planner recommend affected tests, but prior work only inspected source and could not run matching executable regression evidence in GitHub-only contexts.

**Decision:** add one opt-in, bounded affected-check executor `tools/repository_verification.py`. Select existing canonical Node validators, repository Python tests, targeted TypeScript/pytest regressions and optional offline Cargo checks without arbitrary shell commands or new CI triggers. Execute only against a clean, exact-SHA `Local` checkout when the user explicitly requests `--execute`.

**Why:** reduce redundant full-suite execution while preserving a truthful source-test proof boundary, permission preflight, and fail-fast diagnosis.

**Tradeoffs:** targeted PASS is not a complete transitive dependency, native Windows, installer, or release PASS. Missing tools, partial import edges, omitted tests and altered worktrees keep the overall result incomplete. Offline flags do not sandbox repository-authored test code; actual execution still requires informed operator authorization.

**Owner:** `tools/repository_verification.py` executes selected checks. `tools/repository_impact.py` remains the read-only plan owner; `GITHUB_RULES.md` owns policy and CI boundaries.

### D-042 — Data-only Skill admission and evidence-gated procedure evaluation

**Context:** the six existing TranslateIT Skills had routing expectations but lacked nested package safety checks, distinct procedure collision/pressure cases, and externally scored procedure receipts.

**Decision:** keep Skill identity/classification in `.agents/skill-registry.json`, procedures in each existing `SKILL.md`, routing expectations in the current evaluation corpus, and permission authority in the current policy. Add `tools/repository_skill_admission.py` as a static data-only supply-chain gate. Extend the existing `tools/repository_agent_evals.py` with linked procedure cases and external receipt-consistency scoring; add no seventh skill or execution framework.

**Tradeoffs:** static scans cannot exclude all prompt injection or prove model obedience. User approval, release authority, actual target evidence and external network/code execution remain separate gates; receipts cannot be promoted to behavior proof.

**Owner:** existing Skill/registry/permission owners, `.agents/evals/skill-procedure.json`, and the two canonical repository verification helpers.

## Recording policy

Record a durable decision only when architecture/workflow/reasoning must survive sessions, multiple owners depend on it, a meaningful tradeoff exists, or an old method is explicitly superseded.

Minimum shape for substantial new entries:

```text
Context
Decision
Why
Tradeoffs / not chosen
Evidence / proof boundary
Follow-up owner
```

Do not create a decision entry for ordinary wording changes, straightforward bug fixes, or generated artifacts.

## Historical register

`history-legacy.md` preserves the former monolithic decision log. It is historical evidence and may contain superseded branch/product language.
