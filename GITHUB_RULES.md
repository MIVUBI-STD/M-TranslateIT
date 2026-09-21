# GitHub Rules — TranslateIT

Canonical repository operating rules for AI/ChatGPT. Root and nearest `AGENTS.md` files may narrow domain behavior, but they must not weaken safety, integrity, proof, efficiency, security, GitHub-first execution, transfer, or STOP boundaries.

**Local-only repository model:** `Local` is the sole active repository authority. Development, governance, CI, proof, continuation, and release-source validation are performed against `Local`. `main` is outside the current development lifecycle: do not use it as a fallback source, working branch, PR base, promotion target, CI target, or proof source unless the user explicitly changes this policy.

## Core flow

Apply Core Rules 1–7 in order.

```text
PIN
→ EXECUTION CONTEXT
→ EXHAUST REMOTE_GITHUB PARTITION
→ READ MINIMUM
→ DIAGNOSE
→ TOOL + TRANSFER GATE
→ WRITE ONCE
→ VERIFY + FAILURE POLICY
→ STOP
```

# Core Rules

## 1. PIN — establish exact Local authority

Before material work, know repository, exact `Local` HEAD, scope, writability, and proof ceiling.

- **`Local` is the sole active repository authority.**
- Never silently fall back to another branch or ref.
- Direct `Local` branch/file fetch is current-state authority; search is discovery only.
- Every repository write explicitly targets `Local` unless the user explicitly authorizes a different ref for that exact action.
- Re-check `Local` HEAD immediately before an atomic ref move when concurrency is plausible.
- Current source plus matching proof outranks stale continuation prose.
- If `next-action.md` disagrees with current source, reconcile the stale state owner before continuing.
- Historical branches, deleted branches, old chats, and old reports are recovery evidence only.

### Execution context / proof ceiling

Classify by actual capability:

```text
REMOTE_GITHUB  = repository/GitHub + CI evidence
LOCAL_CODE     = exact checkout + development toolchain/filesystem execution
TARGET_WINDOWS = LOCAL_CODE + installed TranslateIT + real Windows GPU/audio/device/meeting environment
```

A label is intent, not proof.

- `REMOTE_GITHUB` may implement source/static/CI-verifiable work.
- `LOCAL_CODE` additionally owns local generators, dependency/toolchain execution, filesystem-sensitive builds and tests.
- `TARGET_WINDOWS` is required for physical microphone, actual GPU practicality, VB-CABLE/device behavior, installed-runtime, meeting-app delivery, end-to-end latency, speaker-quality, and clean-machine claims.
- Hosted Windows CI proves only what that runner executed; it is not automatically `TARGET_WINDOWS`.

### GitHub-first execution partition

`REMOTE_GITHUB` is the default repository-development workbench when it can safely complete the source portion.

```text
GitHub-verifiable
→ exact-source diagnosis/design
→ source implementation that needs no unavailable generated/native output
→ regression/static/integration contracts
→ CI routing + security/provenance
→ deterministic harness/evidence preparation

higher-context residue
→ canonical generated output requiring a capable worktree
→ dependency/toolchain mutation unavailable here
→ installed Windows/GPU/audio/device/meeting proof
```

Rules:

- Exhaust the GitHub-verifiable partition first.
- Do not transfer an entire task because one residue needs higher capability.
- Hand off only the minimum residue with exact inputs, acceptance, and `do not redo` guidance.
- Never claim a prepared harness or CI artifact performed a higher-context action itself.
- If a canonical edit cannot be complete without unavailable generated output, do not move `Local` with an incomplete canonical state.

### Source acceptance

Normal development uses the smallest owning verifier on the exact `Local` SHA under discussion.

When combining separate checks manually, require completed success on the same exact SHA. Different SHAs or ancestor results do not compose into current proof.

Record repository/ref, SHA, run/job, and conclusion when evidence identity materially matters. Missing checks remain missing; do not create proof-only commits or temporary workflows merely to trigger them.

## 2. READ MINIMUM — only what can change the decision

After required continuity boot, default to:

```text
owner/source files   1–3
history reads        0
broad scans          0
```

- Prefer direct fetch for known paths.
- Open more only for a concrete unresolved question.
- Read history only when rationale/regression origin can change the decision.
- Truncated/paginated/partial output is incomplete evidence, not absence.
- Verify exact repo/ref/access once before concluding a target is missing.

## 3. DIAGNOSE — fix the first wrong owner

Establish actual vs expected before writing.

```text
requirement / policy / meaning wrong
→ foundation or semantic owner

implementation wrong
→ implementation owner — IMPLEMENTATION REGRESSION

implementation correct + assertion stale
→ test owner — STALE TEST

implementation/test correct + CI routing wrong
→ workflow/repository policy — ROUTING FAILURE

runtime/toolchain unavailable
→ environment/capability owner — ENVIRONMENT FAILURE

requested evidence missing
→ proof owner — PROOF FAILURE

derived artifact wrong
→ upstream canonical source/generator
```

- Do not widen Maintenance into redesign.
- Do not perform unrelated cleanup, dependency upgrades, compatibility work, framework creation, or documentation synchronization unless required by the same outcome.
- CI failure is evidence, not permission to edit the easiest file.
- Old TODOs/audits/experiments are inactive unless reproduced or explicitly reactivated.
- `No change required` is valid.
- Do not add routers, provider registries, generic evaluators, alternate runtimes, fallback stacks, persistent state, or recovery frameworks without current evidence.

## 4. TOOL + TRANSFER GATE — choose the method that fits

Choose the simplest method that completes the largest valid partition inside the current proof ceiling.

```text
REMOTE_GITHUB
→ exact state: direct GitHub fetch on Local
→ one bounded UTF-8 edit: Contents API on Local
→ coherent multi-file UTF-8 change: atomic Git delivery to Local
→ CI diagnosis: run → failing job/step → relevant log

LOCAL_CODE
→ canonical generator / dependency lock / filesystem-heavy mutation
→ local build/test requiring the actual toolchain

TARGET_WINDOWS
→ installed app / GPU / audio / device / meeting / clean-machine claim

required completion exceeds context
→ finish current valid partition → minimum-residue handoff
```

### Atomic Git delivery

For a coherent multi-file GitHub change:

```text
pin exact Local HEAD + base tree
→ fetch required exact owners
→ finish final contents before mutation
→ create blobs/tree while Local stays unchanged
→ re-check Local HEAD once
→ create one categorized logical commit
→ move Local exactly once
→ relevant final verification
→ STOP
```

Requirements:

- Complete intended file set is known before final ref movement.
- Full-file replacement requires complete current content when preserving it.
- One logical change produces one reviewable `Local` commit by default.
- Low-level Git is not an iterative scratch editor.
- Never split `update_file`; it replaces the whole file.
- Keep blob/content SHA, commit SHA, tree SHA, ref, workflow-run ID, artifact ID, and job ID distinct.
- Never force-push or rewrite `Local` history to work around stale state, CI failure, connector limits, or messy history.
- Connector limitations must not change repository/product architecture.

### Transfer prohibitions

Never create placeholders, transfer-only manifests, temporary loaders, artificial fragments, scratch architecture, one-use workflows, alternate repository layouts, or generated wrappers solely to bypass a tool limitation.

### Execution Handoff

Use only for genuine remaining work above the current context:

```text
FROM_CONTEXT
TO_CONTEXT
repository
branch/ref = Local
pinned HEAD
completed here
residue only
why higher capability is intrinsic
first command/action
acceptance
do not redo
```

## 5. WRITE ONCE — deliver one meaningful Local state

Before repository movement:

```text
repo/Local/current state pinned
scope + owners final
complete final contents ready
no scratch/temporary paths
selected method carries whole current-context delivery
expected proof known

any NO
→ DO NOT WRITE
```

- One intentional write per file and one logical commit per task are defaults.
- Same-file/overlapping mutations are serial.
- Keep one canonical owner per durable rule/state.
- Update README/status/continuation/proof only when the state it owns changes.
- Preserve lockfiles, toolchain constraints, and trusted pins unless they are the actual owner being changed.
- New branches/PRs/issues/comments/releases default to zero unless the user explicitly requests them or scope proves a concrete need.
- Generated artifacts follow source/generator; do not patch generated output to hide an upstream defect.

### Commit discipline

A commit is a categorized logical delivery, not a save/checkpoint/CI trigger/proof marker.

```text
prepare complete logical change
→ cheapest relevant proof
→ review intended state
→ one categorized logical commit on Local
→ one Local ref update
→ relevant CI
→ STOP or hand off named residue
```

Message format:

```text
<type>(<optional-scope>): <concise logical outcome>
```

Use `feat`, `fix`, `docs`, `refactor`, `test`, `ci`, `build`, `release`, or bounded `chore`. Split only for genuinely independent outcomes, never by file, layer, tool call, or discovery order.

### Branch delivery

- Routine repository work lands on `Local` only.
- Do not create a task branch, promotion branch, recovery branch, or alternate development branch as part of the normal method.
- Do not target `main` from automation, CI, PRs, or release-source validation.
- A different branch lifecycle requires a new explicit user decision before implementation.

## 6. VERIFY + FAILURE POLICY — prove only what matters

- Run the cheapest check that can falsify the changed claim.
- Targeted checks are default during iteration.
- Repository/governance changes use `Repository Verify` on `Local`.
- Frontend/Rust/Python/runtime/release checks run only when their owned surface changed.
- Only completed successful verification is PASS. Queued/running/cancelled/skipped/neutral/superseded is not PASS.
- On failure, inspect the exact failing job/step and relevant error before editing.
- Do not weaken/bypass a valid verifier for green status.
- Regression tests protect material recurring invariants, not cosmetic prose.
- Static source/CI does not prove model quality, speaker fidelity, physical audio, target GPU practicality, installed-runtime, meeting-app reception, latency, or clean-machine behavior unless those actually ran.

### Failure / retry matrix

| Failure | Action |
|---|---|
| Known capability mismatch / unsupported transfer | STOP that method; 0 retries; finish other valid partitions and hand off residue |
| Permission/safety denial | STOP; 0 retries unless condition changes |
| Capability genuinely uncertain | at most 1 bounded probe |
| 422 malformed request but valid method | correct once |
| 404 missing/inaccessible | verify exact repo/ref/target once |
| 409 stale/conflict | refetch once and rebuild from current state |
| 429 rate limit | respect server guidance |
| 5xx/timeout/unknown mutation | inspect target before retry |
| Same-cause valid-method failure with new evidence | maximum 2 attempts |

Changing tools/encodings/representations does not reset retry ceilings.

### Interrupted delivery

If current-task writes already occurred before a block, perform at most one bounded recovery pass: identify current-task artifacts, remove only accidental artifacts when safe, preserve legitimate changes, disclose remaining state, then STOP or hand off. Never rewrite `Local` shared history to hide interrupted delivery.

## 7. STOP — completion is terminal

Stop when the requested current-context outcome and relevant proof are satisfied, or when an unavoidable residue has been explicitly handed off.

Do not automatically audit another layer, synchronize unrelated docs, run another verifier, create proof-of-proof, fix adjacent issues, create GitHub objects for ceremony, resume deferred work, or continue because more tooling exists.

## Efficiency budget

```text
owner/source reads            1–3 after boot
history reads                 0 by default
broad scans                   0
uncertain-capability probe    <= 1
same-cause retry              <= 2
capability-denial retry       0
handoff scope                 minimum residue
intentional writes/file       1
logical commits/task          1 by default
Local ref updates/task        1 by default
relevant CI                   0–1 per proof surface
placeholder/transfer hacks    0
adjacent cleanup              0
other-branch mutations        0 unless explicitly authorized
```

# TranslateIT repository boundaries

```text
branch/history/security   → GITHUB_RULES.md
agent routing/modes       → AGENTS.md
current orientation       → CONTEXT.md
product/system law        → docs/foundation/
continuation              → docs/knowledge/next-action.md
current proof meaning     → docs/knowledge/current-validation.md
implementation ownership  → docs/knowledge/source-ownership.md
durable decisions         → docs/knowledge/decisions/
operational runbooks      → docs/knowledge/operations/
specialist routing        → docs/knowledge/skills/
actual behavior           → current Local source + matching proof
historical evidence       → Git history only unless explicitly revalidated
```

`next-action.md` stores only resume-critical state and one next step. `current-validation.md` owns proof interpretation. Historical status/report trees are not current navigation surfaces.

# Conditional GitHub surfaces

## API failures and ambiguous mutations

Interpret 401 as authentication, 403 as permission/policy/rate-limit, 404 as missing/inaccessible/stale, 409 as stale/conflict, 422 as invalid request/policy, 429 as rate limiting, and 5xx/timeout as potentially unknown mutation outcome. After an unknown mutation result, inspect current target state before retry.

## Special files / generated artifacts / binaries

Distinguish regular UTF-8 files from symlinks, submodules, Git LFS pointers, generated artifacts, and binaries. Never hand-edit an LFS pointer as content. Generated output follows its canonical source/generator.

## Pull requests / protection / rulesets

PRs and alternate branch flows are not part of the current Local-only method. If the user explicitly authorizes one, refresh head SHA, base, mergeability, checks/reviews, and protection state immediately before any consequential action.

Branch/tag deletion, PR merge/close, release publication/deletion, repository settings/rules changes, and history-altering operations require explicit authority and exact current targets.

## GitHub Actions and hosted proof

- CI result is evidence; branch protection/rulesets are enforcement. Never infer one from the other.
- If `Local` is unprotected or has no required-check ruleset, a successful workflow still counts only as exact-SHA evidence, not as proof that GitHub prevented unchecked writes.
- Workflows are verification/deployment/artifact infrastructure, not a source editor or remote shell.
- Automatic triggers target `Local` only under the current model.
- Verification workflows are read-only by default and never commit/push back.
- Use least-privilege permissions.
- Pin third-party Actions to immutable 40-character commit SHAs and annotate the intended release.
- `actions/checkout` uses `persist-credentials: false` for read-only verification.
- Jobs have bounded timeouts.
- Dependency installation uses canonical lockfiles/frozen modes where available.
- Treat event-derived strings/paths/names as untrusted input before privileged shell use.
- Do not use `pull_request_target` for untrusted contribution execution unless a future explicit security design proves it necessary.
- Do not create temporary one-use workflows.

## Sensitive data / release / deployment

Never commit or echo secrets, credentials, private keys, auth headers, personal voice recordings, private conversation bodies, or unredacted user paths into public source/workflows/issues/PRs/comments/logs. Report sensitive findings by location/type without repeating protected values.

Release-source validation runs from `Local`. Publishing tags/releases is a separate explicit user action and does not change repository branch authority.
