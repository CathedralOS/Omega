# Omega Package Manager

This crate owns Omega's registry-free package workflow. Read its source in the
same order the workflow runs:

```text
src/
├── lib.rs          public compatibility exports
├── workflow/       complete command-facing operations; start here
├── manifest/       read and conservatively edit build.omg
├── source/         acquire immutable local and Git source snapshots
├── package/        bind snapshots to declarations and dependency rows
├── graph/          construct, validate, and identify the dependency closure
├── review/         compile, compare, triage, and apply root review policy
└── records/        shared bounded atomic record files
```

`workflow/source_audit/` is the first complete operation. Install/update
orchestration remains intentionally absent until its admission prerequisites
are implemented.

## Source custody

```text
source/
├── mod.rs          acquisition boundary and public source vocabulary
├── identity/       package names, aliases, lineages, locators, and exact pins
├── local/          capture and publish immutable local snapshots
├── git/            fetch, authenticate, materialize, and retain Git snapshots
├── custody/        locks, host policy, tree checks, and atomic publication
├── observations/   successful resolution and execution observations
├── storage.rs      private per-user resolver storage lanes
├── limits.rs       compiler-owned acquisition ceilings
└── error.rs        source-resolution failures
```

Hostile-process confinement is delegated to
[`resolver-execution`](../resolver-execution/README.md). Source acquisition
does not know graph identity. `package/` performs the declaration join before
`graph/` derives graph-owned identities.

## Dependency graph

```text
graph/
├── mod.rs           graph boundary and public graph vocabulary
├── root_request.rs  exact request selecting the root
├── traversal/       follow declared workspace, local, and Git edges
├── reconciliation/  reconcile one complete closure
├── validation/      validate nodes, edges, aliases, and reachability
└── subject/         canonical identity of the exact closure
```

### Identity and reconciliation

`PackageKey` is the authored package name plus canonical source lineage. For
Git, lineage names the repository namespace and deliberately excludes revisions,
commits, trees, and content; those identify an exact package instance. Requester
aliases are local graph edges and never enter package, type, conformance, or
evidence identity.

A multi-package Git request acquires one repository/revision and separately
selects `Root` or `Named(PackageName)`. Selected members share the authenticated
fetch and tree. The resolved member path is retained for navigation, replay, and
relative-dependency custody, but moving it does not replace the package.

Requests for one key that resolve to the same immutable source deduplicate.
Different resolutions reject with all dependency paths. Multiple simultaneous
instances per key are unsupported because they would require package-instance
qualification throughout the nominal identity substrate, not merely new aliases.

## Review

```text
review/
├── compilation/     compile exact custody into compiler-issued review rows
├── evidence/        bind rows to source and closure commitments
├── comparison/      compare candidate rows with a retained baseline
├── source_diff/     bounded hostile-source differences
├── triage/          deterministic blockers and audit recommendations
├── advisory/        bounded advisory human/LLM review boundary
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
declarations; [`package-review`](../package-review/README.md) projects checked
semantic facts without admission authority.

Language and design references:

- [`package_manager_first_draft.md`](../../../../../wiki/design_briefs/package_manager_first_draft.md)
- [`build_and_package_model.md`](../../../../../wiki/design_briefs/build_and_package_model.md)
- [`chapter_15_modules_imports_visibility.md`](../../../../../wiki/language_guide/chapter_15_modules_imports_visibility.md)
- [`chapter_19_capabilities_effects_boundaries.md`](../../../../../wiki/language_guide/chapter_19_capabilities_effects_boundaries.md)
