# TranslateIT Tools

Repository-owned Python tools for **source-derived verification, routing, impact and model-quality contracts**. They are not a second application/runtime owner and do not create an automatic agent or CI loop.

- `verify_repository.py` — aggregate structural and governance source checks
- `repository_knowledge.py` — document catalog, graph, scoped retrieval (read-only)
- `repository_context.py` — bounded read-only context/permission projection
- `repository_dependencies.py`, `repository_impact.py` — partial source dependency impact and affected proof planning
- `repository_verification.py` — explicit opt-in execution of existing selected commands only
- `repository_benchmark.py` — opt-in source-derived method-flow/context/permission conformance replay, not model or native proof
- `repository_permissions.py`, `repository_agent_evals.py`, `repository_skill_admission.py` — scoped preflight, evaluation and skill package admission
- `repository_contracts.py` and `interop-contracts.json` — bridge contract location, not duplicate Rust/TS/Python semantics
- `translation_quality/`, `asr_quality/`, `tts_quality/`, `quality_readiness/` — independent current quality contracts; baseline fixture presence is not real inference PASS
- `tests/` — regression tests for these existing tools

`tooling/windows-toolchain/` contains the separate optional developer launcher invoked by root `DEV.cmd`. Source/product ownership is in [Implementation Map](../docs/knowledge/source-ownership.md); repository policy in [GitHub Rules](../GITHUB_RULES.md). No scripts are assigned to the user's PC for incremental ChatGPT-owned development.
