# Omega Package Manager

This crate owns Omega's registry-free package workflow. It reads package
declarations from `build.omg`, resolves an immutable source closure, asks the
compiler for semantic review evidence, compares that evidence with an accepted
baseline, and will eventually publish install and update transactions.

The crate is not yet an admission-complete package manager. Start with
[`TASKS_PACKAGE_MANAGER.md`](../../../../../TASKS_PACKAGE_MANAGER.md) for the
remaining release work and [`IMPLEMENTATION_STATUS.md`](IMPLEMENTATION_STATUS.md)
for the detailed implemented security floor.

## Read the source in workflow order

```text
src/
|-- lib.rs                 Public facade; no package behavior lives here.
|-- declarations/          Read and conservatively edit build.omg.
|-- resolution/
|   |-- identity.rs        Package names, keys, lineages, and immutable pins.
|   |-- closure/           Resolve and identify one complete package closure.
|   |   |-- sources.rs     Connect declarations to workspace, local, and Git sources.
|   |   |-- reconcile.rs   Reconcile the complete dependency closure.
|   |   |-- graph.rs       Validate package nodes, edges, aliases, and reachability.
|   |   `-- subject.rs     Canonically encode the exact resolved closure.
|   `-- source/            Capture hostile source under resolver custody.
|       |-- storage.rs     Private per-user storage and retained cache lanes.
|       |-- local/         Local snapshot capture and authentication.
|       |-- git/           Git request, fetch, object authentication, and snapshot.
|       `-- custody/       Tree checks, host policy, locks, and atomic publication.
|-- review/
|   |-- compiler_review.rs Compile a resolved closure into compiler-issued evidence.
|   |-- capability_conflict.rs
|   |                       Compare candidate and baseline capabilities.
|   |-- source_patch.rs    Produce bounded source changes for human/LLM review.
|   |-- source_triage.rs   Derive deterministic blockers and audit recommendations.
|   |-- source_review.rs   Assemble and invoke the advisory-review boundary.
|   |-- baseline.rs        Recover review-only comparison baselines.
|   `-- policy.rs          Resolve root-owned review policy.
`-- storage/               Bounded internal record persistence.
```

The current public facade preserves historical flat re-exports. New code should
enter through the responsibility modules above; the flat surface is a
compatibility seam to narrow before command release.

## Trust boundaries

- `omega-packages` composes source, compiler evidence, review, and root policy.
- [`omega-package-review`](../omega-package-review/README.md) projects checked
  compiler state into canonical, non-admitting review evidence.
- [`omega-resolver-execution`](../omega-resolver-execution/README.md) confines
  native acquisition helpers without choosing package identity or admission.

Package-authored code never chooses its admitted capabilities, accepted lock
state, resolver policy, or review outcome. `build.omg` supplies package identity
and source requests through the compiler-owned declaration projection; checked
compiler state supplies capability and API evidence.

## Design references

- [`SOURCE_RESOLVER_SECURITY.md`](SOURCE_RESOLVER_SECURITY.md)
- [`package_manager_first_draft.md`](../../../../../wiki/design_briefs/package_manager_first_draft.md)
- [`build_and_package_model.md`](../../../../../wiki/design_briefs/build_and_package_model.md)
- [`chapter_15_modules_imports_visibility.md`](../../../../../wiki/language_guide/chapter_15_modules_imports_visibility.md)
- [`chapter_19_capabilities_effects_boundaries.md`](../../../../../wiki/language_guide/chapter_19_capabilities_effects_boundaries.md)
