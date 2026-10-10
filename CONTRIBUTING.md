# Contributing

TranslateIT is currently developed as a controlled personal/internal product repository. Public visibility does not mean external contributions, reuse, or redistribution are automatically accepted.

## Repository model

**Local-only.**

```text
Local
→ sole active development/source/governance/CI/proof authority
→ one logical outcome per final Local commit by default
```

Routine work is performed directly on `Local`. Do not create task branches, promotion branches, compatibility branches, or branch-per-proof workflows as part of the normal method. A different branch lifecycle requires a new explicit user decision.

## ChatGPT repository route

ChatGPT uses GitHub branch `Local` for repository source/version management **only**. No cloud compute, cloud inference, hosted builds or CI dispatch. User-PC checkout, tool installation and per-change Windows/audio tests are not prerequisites for source edits; executable results remain `UNKNOWN / NOT EXECUTED` until a later integrated local phase. See `GITHUB_RULES.md`.

## Development method

Follow `GITHUB_RULES.md` and `AGENTS.md`.

```text
PIN Local
→ execution context
→ smallest owner
→ first wrong owner
→ minimum complete change
→ cheapest falsifiable proof
→ one logical Local delivery
→ STOP
```

Use the Bounded, Standard, or Complex contract from `AGENTS.md`. Complex/ambiguous work uses `.agents/skills/development-brief/SKILL.md`.

## Before committing

Workflow definitions remain manual-only but inactive under no-cloud policy. Prefer exact-source inspection and never dispatch hosted GitHub Actions. Do not ask the user to run incremental tests; defer actual compiler/package/device checks to the integrated local acceptance phase. Do not change the GitHub default branch from `main`: it is reserved for finalized results. Manual dispatch of workflows present only on `Local` is not yet guaranteed; use available source/local checks and report any missing CI proof rather than promoting unfinished files.


Run the cheapest relevant proof.

Repository/governance:

```bash
python tools/verify_repository.py
```

Desktop/frontend/Rust and Python worker proof follow their owning package/workflow. Native Windows/device acceptance is single planned stage after an integrated test-ready candidate meets `docs/knowledge/operations/target-windows-performance.md`, not a per-commit test.

## Commit discipline

Use meaningful categorized history:

```text
feat:      new capability
fix:       behavior correction
refactor:  internal restructuring without intended behavior change
docs:      documentation/policy-only change
test:      regression-contract-only change
ci:        workflow/CI change
build:     dependency/toolchain change
release:   release-source/artifact state
chore:     bounded maintenance when no clearer category fits
```

A commit is not a checkpoint, CI trigger, or proof marker. Material commits carry truthful `Work:`, `State:`, and `Proof:` trailers, with `Decision:`, `Unresolved:`, or `Next:` only when relevant. They preserve revision-scoped recovery evidence; `planning/development.md` remains the sole active continuation owner. See `GITHUB_RULES.md` for expected-SHA fast-forward delivery.

## Pull requests

Pull requests are not part of the normal Local-only development path. If a PR is explicitly requested for a specific exceptional case, it must target `Local`; it does not create a second source authority.

## Data and privacy boundary

Never commit:

- API keys, tokens, credentials, private keys, or `.env` secrets;
- personal voice recordings or user-created My Voice datasets/actors;
- private meeting/conversation bodies;
- unredacted user paths or private diagnostics;
- generated caches/logs/build output;
- locally staged private runtime/model payloads unless repository policy explicitly declares the exact asset tracked and redistributable.

`UserData/` tracks structure/documentation only; user-owned runtime contents remain ignored.

## Release boundary

Release-source validation runs from current `Local`. Publishing a tag or GitHub Release is a separate explicit action and does not change repository branch authority.

## License boundary

No repository license is inferred by public visibility. External reuse/redistribution terms remain an explicit owner/legal decision. Do not add or change a license as routine development cleanup.
