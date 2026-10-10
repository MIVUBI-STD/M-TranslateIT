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

ChatGPT uses `Local` on GitHub/cloud for source-verifiable development; user-PC setup, toolchain installs, local checkout and native Windows/audio testing are not prerequisites for a remote code/docs change. Higher-context proof is reported `UNKNOWN / NOT EXECUTED` until genuinely observed, without an automatic user-PC handoff. Human local development remains optional; see `GITHUB_RULES.md`.

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

Verification workflows are `workflow_dispatch` only. A source/static change does not require an automatic or ceremonial CI run. Prefer the smallest meaningful local/source check; manual GitHub Actions provide optional stronger execution proof when the exact claim demands it. Do not change the GitHub default branch from `main`: it is reserved for finalized results. Manual dispatch of workflows present only on `Local` is not yet guaranteed; use available source/local checks and report any missing CI proof rather than promoting unfinished files.


Run the cheapest relevant proof.

Repository/governance:

```bash
python tools/verify_repository.py
```

Desktop/frontend/Rust and Python worker proof follow their owning package/workflow. Do not run target-Windows/device checks ceremonially for source-only changes.

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

A commit is not a checkpoint, CI trigger, or proof marker. Material commits carry truthful `Work:`, `State:`, and `Proof:` trailers, with `Decision:`, `Unresolved:`, or `Next:` only when relevant. They preserve revision-scoped recovery evidence; `docs/knowledge/next-action.md` remains the sole active continuation owner. See `GITHUB_RULES.md` for expected-SHA fast-forward delivery.

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
