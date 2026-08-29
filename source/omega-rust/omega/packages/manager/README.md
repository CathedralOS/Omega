# Omega Package Manager

This crate owns Omega's registry-free package workflow. Read its source in the
same order the workflow runs:

```text
src/
├── lib.rs          public entrance; reexports only
├── manifest/       read and conservatively edit build.omg
├── source/         establish immutable package-source custody
├── graph/          construct, validate, and identify the dependency closure
├── review/         compile, compare, triage, and apply root review policy
└── storage/        shared bounded atomic record files
```

## Source custody

```text
source/
├── mod.rs          source facade and responsibility map
├── audit.rs        read-only command boundary
├── identity/       package names, aliases, lineages, locators, and exact pins
├── local/          capture and publish immutable local snapshots
├── git/            fetch, authenticate, materialize, and retain Git snapshots
├── custody/        locks, host policy, tree checks, and atomic publication
├── observations/   successful resolution and execution observations
├── package/        bind one retained source to its package declaration
├── storage.rs      private per-user resolver storage lanes
├── limits.rs       compiler-owned acquisition ceilings
└── error.rs        source-resolution failures
```

Hostile-process confinement is delegated to
[`resolver-execution`](../resolver-execution/README.md). Source identity,
package identity, and final resolution issuance remain here.

## Dependency graph

```text
graph/
├── mod.rs           graph facade
├── traversal/       follow declared workspace, local, and Git edges
├── reconciliation/  reconcile one complete closure
├── validation/      validate nodes, edges, aliases, and reachability
└── subject/         canonical identity of the exact closure
```

## Review

```text
review/
├── compiler/        compile exact custody into compiler-issued review rows
├── evidence/        bind rows to source and closure commitments
├── comparison/      compare candidate rows with a retained baseline
├── diff/            bounded hostile-source differences
├── triage/          deterministic blockers and audit recommendations
├── advisor/         bounded advisory human/LLM review boundary
├── baseline/        non-admitting review baseline persistence
├── reconstruction/  exact closure reconstruction questions
└── policy/          root-owned review decisions
```

The package manager is not admission-complete. In particular, no accepted lock
or atomic install/update transaction exists yet. Remaining work is maintained
in [`TASKS_PACKAGE_MANAGER.md`](../../../../../TASKS_PACKAGE_MANAGER.md), and
resolver guarantees and gaps are maintained in
[`SOURCE_RESOLVER_SECURITY.md`](SOURCE_RESOLVER_SECURITY.md).

Package-authored code never chooses admitted capabilities, accepted lock state,
resolver policy, or review outcome. `build.omg` provides compiler-checked
declarations; [`compiler-review`](../compiler-review/README.md) projects checked
semantic facts without admission authority.

Language and design references:

- [`package_manager_first_draft.md`](../../../../../wiki/design_briefs/package_manager_first_draft.md)
- [`build_and_package_model.md`](../../../../../wiki/design_briefs/build_and_package_model.md)
- [`chapter_15_modules_imports_visibility.md`](../../../../../wiki/language_guide/chapter_15_modules_imports_visibility.md)
- [`chapter_19_capabilities_effects_boundaries.md`](../../../../../wiki/language_guide/chapter_19_capabilities_effects_boundaries.md)
