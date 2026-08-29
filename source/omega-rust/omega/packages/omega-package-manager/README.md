# Omega Package Manager

This crate owns Omega's registry-free package workflow. Read its source in the
same order the workflow runs:

```text
src/
├── lib.rs            public entrance naming each workflow owner
├── commands/         command-facing operations; start here
│   └── audit/source/ acquire and inspect one source without admission
├── identity/         package names, aliases, and source-qualified keys
├── manifest/         read package roles and dependency rows from build.omg
│   └── dependencies/ read exact rows or plan conservative edits
├── resolution/       turn declared sources into one validated closure
│   ├── source/       join immutable snapshots to package declarations
│   └── closure/      traverse, reconcile, validate, and identify the graph
└── review/           compile, compare, triage, and apply root review policy
```

`commands/audit/source/` is the first complete operation. Install/update
orchestration remains intentionally absent until its admission prerequisites
are implemented.

Callers use these owner paths directly—for example `manifest::` for `build.omg`
projection, `resolution::` for source closure construction, and `review::` for
compiler-review orchestration. The crate root does not flatten their APIs into
a second, ambiguous namespace.

## Source custody

Immutable acquisition is delegated to
[`omega-package-source`](../omega-package-source/README.md), which composes
confined native execution from
[`omega-resolver-execution`](../omega-resolver-execution/README.md).
Source acquisition owns syntax-neutral source lineage and immutable custody.
The manager's `identity/` owns package-authored names, requester aliases, and
source-qualified package keys. Acquisition cannot select or admit a closure.
Authored workspace-member paths remain build declarations; the manager converts
them explicitly to source-owned validated relative paths at the custody edge.
`resolution/source/` performs the declaration join before
`resolution/closure/` derives closure-owned identities.
Manager code that composes an already-retained source lane uses the source
crate's named `git::resolution`, `local::operations`, `local::model`, and
`storage` responsibilities instead of hidden crate-root aliases.

```text
resolution/source/
├── git/               resolve Git requests into selected package custody
│   └── workspace/     discover, select, commit, and replay declarations
├── local.rs           bind external-local snapshots
├── workspace.rs       bind explicit live-workspace members
├── custody.rs         transport-erased package source custody
└── materialization.rs exact selected compilation-root commitment
```

## Dependency graph

```text
resolution/closure/
├── mod.rs           closure boundary and public vocabulary
├── model.rs         validated nodes, edges, aliases, and reachability
├── root_request.rs  exact request selecting the root
├── traversal/       follow declared workspace, local, and Git edges
│   └── dependencies/ dispatch Git and Path rows within retained context
├── reconciliation/  reconcile one complete closure
└── identity/        canonical identity of the exact closure
    ├── subject/     closure question, requests, limits, and fingerprint
    ├── codec/       encode framing, source identities, and selections
    └── validation/  reject invalid graphs, requests, and source custody
```

Directory entrances are maps: `mod.rs` names the responsibility and points to
plainly named implementation files. Substantive implementation does not live in
an entrance file.

### Identity and reconciliation

`PackageKey` is the authored package name plus canonical source lineage. For
Git, lineage names the repository namespace and deliberately excludes revisions,
commits, trees, and content; those identify an exact package instance. Requester
aliases are local graph edges and never enter package, type, conformance, or
evidence identity.

A multi-package Git request acquires one repository/revision and separately
selects `Root` or `Named(PackageName)`. Selected members share the authenticated
fetch and tree. The resolved member path is retained for navigation, replay, and
relative-dependency custody. Member-relative `Path` rows may select only exact
members declared by that authenticated root; recursive discovery and undeclared
directories reject.

`PackageKey` remains stable when a member moves within one repository lineage,
but canonical closure evidence binds the selected navigation, so relocation is
still an explicit source-question change. Requests for one key deduplicate only
when immutable resolution, navigation, and projected dependency rows all agree;
machine-specific cache roots do not participate. Different semantic custody
rejects with all dependency paths. Multiple simultaneous instances per key are
unsupported because they would require package-instance qualification throughout
the nominal identity substrate, not merely new aliases.

Package and application roots share `PackageKey`; their explicit
`BuildDeclarationKind` is carried separately. The existing root/non-root binding
rule admits either role only at the selected root and requires every dependency
edge to resolve to a package. External-local, Git, named Git member, and
workspace entry points expose this distinction explicitly: package entry points
reject applications, while project-root entry points retain either admissible
role. Resolution must carry that role through closure, lock, review,
compiler-handoff, and audit evidence instead of coercing an application
declaration into a package declaration after admission.

Compiler handoff uses that same `BuildDeclarationKind` rather than a second
role vocabulary. `PackageCompilationInputs`, its path-free dependency closure,
and ordinary obligation-ledger v2 recovery retain it; a workspace catalog
cannot enter compilation, and re-rooting a dependency gives that independently
compiled dependency its already-enforced package role. Source-consumption v3
and production-manifest v2 identities bind the role as part of exact compiler
custody.

Workspaces are member catalogs, not keyed graph nodes. Each selected application
member forms its own closure; membership alone does not combine applications or
include unrelated packages.

One selected `build.omg` has one free `build(&mut Build)` project entry. Scoped
`Owner::build` declarations are ordinary machines, never alternate manifests.
Static role/member/dependency projection stays direct and root-owned. Evaluated
composition may use ordinary helpers borrowing `&mut Build`, with their complete
transitive contracts retained against the root.

Canonical source-closure encoding v4 binds the selected root's explicit role,
root and dependency selectors, plus one stable navigation value for every
package. Review revalidates the complete
authenticated repository commitment before opening a selected member subtree;
it never compares a member-only digest to the repository digest.
Operational custody separately retains the selected package materialization
commitment and its file/byte counts. This keeps one repository commit/root-tree
resolution shareable across members while every compilation root remains
independently recheckable. Named Git selection additionally retains every
declared member path, package role/name, declaration byte count, and
domain-separated declaration commitment. Closure reconciliation compares the
complete evidence; review and compiler handoff replay it from retained bytes.
Declaration text does not enter the compilation root merely to carry this
evidence.

Repository resolution and selected package materialization are distinct
custody values. Root selection publishes the authenticated repository tree;
named selection publishes only its authenticated member tree. The exact root
commit/tree pin and retained declaration evidence remain independently
replayable, so several members can share one acquisition without exposing
unselected siblings to compilation.

### Target-conditioned projection

`build.omg` keeps one dependency API. Conditional edges are ordinary
`depend`/`depend_as` calls reached through exact branches of immutable
`builder.target`; there is no condition string or `depend_when` operation. The
package manager does not execute the build machine. It statically closes the
finite state graph into `ProjectedDependencies { common, by_profile }`, following
only unconditional transitions and exact target arms. Runtime-subject paths,
wildcard target paths, mixed safe/tainted paths, and unreachable dependency
occurrences reject with transition provenance.

Projection validates referenced profiles against the trusted toolchain catalog
and remains target-independent. Resolution selects `common + by_profile[P]`.
Alias uniqueness is scoped to that active set, so mutually exclusive columns
may reuse an alias. The one workspace lock carries independently populated
per-profile closure/review sections; an absent section rejects in locked mode.

## Review

```text
review/
├── compilation/     compile exact custody into compiler-issued review rows
├── records/         bind rows to source and closure commitments
├── comparison/      compare candidate rows with a retained baseline
├── source_diff/     bounded hostile-source differences
├── triage/          deterministic blockers and audit recommendations
├── audit_input/     deterministic source-review input assembly
├── baseline/        non-admitting review baseline persistence
├── reconstruction/  exact closure reconstruction questions
└── policy/          root-owned review decisions
```

The package manager is not admission-complete. In particular, no accepted lock
or atomic install/update transaction exists yet. Remaining work is maintained
in [`TASKS_PACKAGE_MANAGER.md`](../../../../../TASKS_PACKAGE_MANAGER.md), and
resolver guarantees and gaps are maintained by source acquisition in
[`SOURCE_RESOLVER_SECURITY.md`](../omega-package-source/SOURCE_RESOLVER_SECURITY.md).

Capability-safe atomic record persistence is shared infrastructure owned by
`omega-platform-custody`, not a package-manager responsibility.

Model-facing source-review prompts, response schemas, and runner integration
live in the separate optional `omega-package-advisory` crate. Package core
publishes deterministic bounded review input only; optional tooling cannot
change acceptance, conflicts, or compiler-owned audit recommendations.

Package-authored code never chooses admitted capabilities, accepted lock state,
resolver policy, or review outcome. `build.omg` provides compiler-checked
manifest declarations; [`omega-package-review`](../omega-package-review/README.md)
projects checked semantic facts without admission authority.

Language and design references:

- [`package_manager_first_draft.md`](../../../../../wiki/design_briefs/package_manager_first_draft.md)
- [`build_and_package_model.md`](../../../../../wiki/design_briefs/build_and_package_model.md)
- [`chapter_15_modules_imports_visibility.md`](../../../../../wiki/language_guide/chapter_15_modules_imports_visibility.md)
- [`chapter_19_capabilities_effects_boundaries.md`](../../../../../wiki/language_guide/chapter_19_capabilities_effects_boundaries.md)
