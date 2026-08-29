# Omega Package Manager

Start in [`omega-package-manager`](omega-package-manager/README.md). It owns the
package workflow: read project declarations, acquire immutable sources, resolve
the dependency closure, compile that closure for review, compare a candidate
with an accepted baseline, and eventually publish an install or update.

The siblings are narrower trust boundaries, not alternate entry points:

```text
packages/
|-- README.md                    # This map and ownership boundary.
|-- omega-package-manager/       # Workflow entrance and public Rust API.
|   `-- src/
|       |-- manifest/            # Read and conservatively edit build.omg.
|       |-- source/              # Identify, resolve, acquire, and audit sources.
|       |-- closure/             # Reconcile and identify the dependency graph.
|       |-- review/              # Compiler review, comparison, triage, and policy.
|       `-- records/             # Internal bounded retained records.
|-- omega-package-review/        # Compiler-owned, non-admitting semantic projection.
|   `-- src/
|       |-- model/               # Stable review vocabulary.
|       |-- projection/          # Checked compiler state into that vocabulary.
|       `-- encoding/            # Canonical rows and strict recovery.
`-- omega-resolver-execution/    # OS process/network confinement for acquisition.
```

The dependency direction is deliberate:

```text
omega-package-manager
    |-- uses --> omega-package-review
    `-- uses --> omega-resolver-execution

omega-package-review             # knows compiler semantics, not package policy
omega-resolver-execution         # knows host confinement, not package identity
```

Source-layout rules for this subsystem:

- a crate or responsibility directory has one obvious `README.md`, `lib.rs`, or
  `mod.rs` entrance;
- entrance modules map and reexport responsibilities rather than accumulating
  behavior;
- child names describe what they own (`identity`, `custody`, `comparison`,
  `encoding`), and tests live beside the behavior they exercise;
- production modules import their dependencies explicitly instead of inheriting
  a parent prelude.

Package-authored source never decides admission, capability classification,
resolver policy, or accepted lock state. `omega-package-review` produces
compiler review evidence but cannot admit it. `omega-resolver-execution`
executes bounded acquisition helpers but cannot choose package identity or
policy. The manager is the only layer that composes those results, and its
mutating install/update commands remain gated by `TASKS_PACKAGE_MANAGER.md`.

Design and security references:

- [`omega-package-manager/README.md`](omega-package-manager/README.md)
- [`omega-package-manager/SOURCE_RESOLVER_SECURITY.md`](omega-package-manager/SOURCE_RESOLVER_SECURITY.md)
- [`../../../../wiki/design_briefs/package_manager_first_draft.md`](../../../../wiki/design_briefs/package_manager_first_draft.md)
- [`../../../../TASKS_PACKAGE_MANAGER.md`](../../../../TASKS_PACKAGE_MANAGER.md)
