# Omega Package Manager

This crate owns Omega's registry-free package workflow. Its source tree is
organized by questions a reader naturally asks. Start at `src/lib.rs`, then
follow the owner that matches the work.

```text
src/
├── lib.rs          public owner map
├── workflows/      real user-facing entrances
│   ├── audit_source/ acquire and inspect one source without admission
│   └── local_project.rs prepare ordinary package-aware compilation
├── project/        package roles and dependencies declared by build.omg
│   ├── roles.rs    package, application, and workspace roles
│   └── dependencies/
├── sources/        bind declarations to immutable package source custody
│   ├── git/        Git package and workspace-member sources
│   ├── local.rs    external-local sources
│   └── workspace.rs live workspace-member sources
├── graph/          build and identify one exact dependency closure
│   ├── resolve/    traverse declared dependency edges
│   ├── reconcile/  reject conflicting or incomplete closures
│   └── subject/    canonical closure question and fingerprint
├── package/        names, requester aliases, and source-qualified package keys
└── review/         compiler evidence, source audit, comparison, and decisions
    ├── candidate/  compile exact custody into compiler-issued evidence
    ├── compare/    compare candidate evidence with a retained baseline
    ├── audit/      triage and bounded hostile-source review input
    ├── baseline/   non-admitting restart-stable review baseline
    ├── decision/   root-owned review policy records
    └── reconstruction/ exact local reconstruction questions
```

`workflows/local_project.rs` is the ordinary compiler entrance;
`workflows/audit_source/` is the first explicit package command. Install and update
orchestration remain intentionally absent until accepted-lock and atomic
transaction prerequisites are implemented. Current work is tracked in
[`TASKS_PACKAGE_MANAGER.md`](../../../../../TASKS_PACKAGE_MANAGER.md).

## Ownership

`workflows` owns complete command/compiler flows. `project` reads checked
`build.omg` package declarations. `sources` joins those
declarations to immutable source custody. `graph` follows and
reconciles dependency edges. `review` compiles the resulting closure, compares
compiler-issued facts, prepares source audit material, and records the root
owner's decision.

The crate root deliberately does not flatten these APIs. Callers name the owner
they consume, so source paths continue to explain authority and data flow.

Supporting crates have one-way responsibilities:

- [`omega-package-source`](../source/README.md) acquires and
  authenticates immutable local and Git source without selecting or admitting a
  package graph.
- [`omega-package-evidence`](../review/evidence/README.md) projects
  checked compiler state into inert, canonically encoded evidence. It cannot
  make review or admission decisions.
- [`omega-package-advisory`](../review/advisory/README.md) owns optional
  model-facing audit assistance. It cannot alter deterministic manager policy.
- [`omega-resolver-execution`](../source/resolver-execution/README.md) owns
  confined native helper execution used by source acquisition.

## Core invariants

A `PackageKey` is the package-authored name plus canonical source lineage.
Requester aliases belong only to graph edges. Revisions, commits, trees,
materialization commitments, target profiles, and projected dependency rows
identify an exact candidate instance rather than changing its stable package
name.

One selected `build.omg` has one root `build(&mut Build)` declaration.
Dependency projection is static and target-aware; the package manager does not
execute arbitrary package build code while discovering dependencies. Resolution
retains the complete target-condition schema and selects one explicit target
profile.

Source custody, graph identity, compiler evidence, audit recommendation, owner
decision, and eventual admission are distinct states. A review baseline is not
an accepted lock. Evidence that an audit was requested or recorded is not proof
that a human or model audited competently. Projects that care about audit rigor
must enforce that rigor in their own infrastructure.

Initial installs and updates both recommend audit for intrinsically dangerous
authority such as filesystem or network access. Updates do not become
uninteresting merely because the dangerous capability set is unchanged; source
code and behavior may still have changed.

## Security references

- [Source resolver security](../source/SOURCE_RESOLVER_SECURITY.md)
- [Package manager design draft](../../../../../wiki/design_briefs/package_manager_first_draft.md)
- [Build and package model](../../../../../wiki/design_briefs/build_and_package_model.md)
- [Capabilities, effects, and boundaries](../../../../../wiki/language_guide/chapter_19_capabilities_effects_boundaries.md)
