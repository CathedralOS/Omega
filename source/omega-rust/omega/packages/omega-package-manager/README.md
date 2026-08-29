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
|   |-- dependency_projection/ Extract and validate checked dependency requests.
|   `-- dependency_edit/   Plan and render conservative declaration patches.
|-- source/                Package sources, from names to retained snapshots.
|   |-- identity/          Stable package and source identity facade.
|   |   |-- names.rs       Package names, aliases, and source-bound keys.
|   |   |-- git.rs         Git repository lineages and accepted transports.
|   |   |-- locator.rs     Strict Git locator parsing and normalization.
|   |   |-- local.rs       Workspace and external-local lineages and digests.
|   |   `-- resolution.rs  Immutable source pins and typed Git object IDs.
|   |-- package_resolution/ Bind Git, workspace, and local requests to packages.
|   |-- inspection.rs      Source-inspection command boundary.
|   `-- acquisition/       Capture hostile source under resolver custody.
|       |-- storage.rs     Private per-user storage and retained cache lanes.
|       |-- local/         Local snapshot capture and authentication.
|       |-- git/           Git request, fetch, authentication, and materialization.
|       |   |-- cache/     Create, authenticate, invalidate, and reuse Git caches.
|       |   |-- objects/   Authenticate commits, trees, paths, and bounded blobs.
|       |   |-- snapshot/  Construct, validate, and atomically publish source trees.
|       |   `-- execution/ Confine Git processes and establish executable custody.
|       |-- observations/  Issue resolved-source, execution, and accounting records.
|       `-- custody/       Tree checks, host policy, locks, and atomic publication.
|-- closure/               Resolve and identify one complete package closure.
|   |-- traversal/         Connect declarations to workspace, local, and Git sources.
|   |   |-- mod.rs         Root-source workflow facade and public entry points.
|   |   |-- workspace.rs   Explicit workspace-member roots.
|   |   |-- git.rs         Immutable Git roots and request matching.
|   |   |-- external_local.rs Explicit local roots outside a workspace.
|   |   |-- dependency_resolution.rs Resolve declared Path and Git edges.
|   |   |-- cache.rs       Access retained source-storage lanes.
|   |   `-- errors.rs      Public traversal failure vocabulary.
|   |-- reconciliation/    Reconcile the complete dependency closure.
|   |   |-- mod.rs         Reconciliation facade and result vocabulary.
|   |   |-- source_custody.rs Exact root requests and immutable custody.
|   |   |-- resolution.rs  Bounded traversal and conflict-path collection.
|   |   |-- resolved_closure.rs Validated closure and source-selection views.
|   |   `-- model.rs       Paths, limits, conflicts, and failures.
|   |-- graph/             Validate package nodes, edges, aliases, and reachability.
|   `-- subject/           Canonically encode the exact resolved closure.
|-- review/
|   |-- compilation_inputs.rs Compiler inputs derived from exact source custody.
|   |-- review_set_validation.rs Join complete review sets to package custody.
|   |-- compiler_review/   Compile a closure and retain compiler-issued evidence.
|   |-- evidence/          Bind compiler output to source and closure commitments.
|   |-- comparison/        Compare candidate and baseline capabilities.
|   |   |-- model.rs       Bounded conflict and error vocabulary.
|   |   |-- compare.rs     Exact row comparison and closure commitments.
|   |   `-- format.rs      Fixed review rendering and canonical tags.
|   |-- source_diff/       Produce bounded source changes for human/LLM review.
|   |   |-- snapshot.rs    Capture and classify resolver-owned snapshots.
|   |   |-- diff.rs        Bound line splitting, diff work, and hunk construction.
|   |   `-- output.rs      Escape hostile bytes into a bounded output sink.
|   |-- review_triage/     Derive deterministic blockers and audit recommendations.
|   |   |-- mod.rs         Review decisions from exact candidate/baseline evidence.
|   |   `-- render.rs      Bounded fixed-vocabulary advisory-review input.
|   |-- advisory_review/   Assemble and invoke the advisory-review boundary.
|   |-- reconstruction_question/ Bind review evidence to exact closure identity.
|   |-- baseline/          Capture and recover review-only comparison baselines.
|   |   |-- capsule.rs     In-memory baseline packages and capsules.
|   |   |-- storage.rs     Private rooted record persistence.
|   |   |-- validation.rs  Canonical graph and resource checks.
|   |   `-- encoding.rs    Canonical binary codec and identity encoding.
|   `-- root_policy/       Resolve, encode, and store root-owned review policy.
`-- records/               Bounded internal record persistence.
```

The public facade preserves historical flat names through an explicit
compatibility list; it does not inherit transitive glob exports. New code should
enter through the responsibility modules above.

## Trust boundaries

- `omega-package-manager` composes source, compiler evidence, review, and root policy.
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
