# Omega Package Manager

This crate owns Omega's registry-free package workflow. Read its source in the
same order the workflow runs:

```text
src/
├── lib.rs          public crate entrance and responsibility exports
├── commands/       complete command-facing operations; start here
├── declarations/   read and conservatively edit build.omg
├── resolution/     turn declared sources into one validated closure
│   ├── package/    bind immutable snapshots to package declarations
│   └── graph/      traverse, reconcile, validate, and identify the closure
├── review/         compile, compare, triage, and apply root review policy
```

`commands/source_audit/` is the first complete operation. Install/update
orchestration remains intentionally absent until its admission prerequisites
are implemented.

## Source custody

Immutable acquisition is delegated to
[`source/acquisition`](../source/acquisition/README.md), which composes confined
native execution from [`source/execution`](../source/execution/README.md).
Acquisition does not know graph identity. `resolution/package/` performs the
declaration join before `resolution/graph/` derives graph-owned identities.

## Dependency graph

```text
resolution/graph/
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
├── advisory/        deterministic source-review evidence assembly
├── baseline/        non-admitting review baseline persistence
├── reconstruction/  exact closure reconstruction questions
└── policy/          root-owned review decisions
```

The package manager is not admission-complete. In particular, no accepted lock
or atomic install/update transaction exists yet. Remaining work is maintained
in [`TASKS_PACKAGE_MANAGER.md`](../../../../../TASKS_PACKAGE_MANAGER.md), and
resolver guarantees and gaps are maintained in
[`SOURCE_RESOLVER_SECURITY.md`](SOURCE_RESOLVER_SECURITY.md).

Capability-safe atomic record persistence is shared infrastructure owned by
`omega-platform-custody`, not a package-manager responsibility.

Model-facing source-review prompts, response schemas, and runner integration
live in the separate optional `omega-package-advisory` crate. Package core
publishes deterministic bounded review input only; optional tooling cannot
change acceptance, conflicts, or compiler-owned audit recommendations.

Package-authored code never chooses admitted capabilities, accepted lock state,
resolver policy, or review outcome. `build.omg` provides compiler-checked
declarations; [`review/evidence`](../review/evidence/README.md) projects checked
semantic facts without admission authority.

Language and design references:

- [`package_manager_first_draft.md`](../../../../../wiki/design_briefs/package_manager_first_draft.md)
- [`build_and_package_model.md`](../../../../../wiki/design_briefs/build_and_package_model.md)
- [`chapter_15_modules_imports_visibility.md`](../../../../../wiki/language_guide/chapter_15_modules_imports_visibility.md)
- [`chapter_19_capabilities_effects_boundaries.md`](../../../../../wiki/language_guide/chapter_19_capabilities_effects_boundaries.md)
