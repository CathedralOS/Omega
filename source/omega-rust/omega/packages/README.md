# Omega Packages

Start in [`omega-package-manager`](omega-package-manager/README.md) for a user
operation. Each remaining folder is one supporting crate, named exactly as it
appears in Cargo metadata.

```text
packages/
├── README.md                    this entrance
├── omega-package-manager/       commands, package graph, review, and admission
├── omega-package-source/        immutable local and Git source acquisition
├── omega-resolver-execution/    confined native resolver processes
├── omega-package-review/        checked compiler state as inert review evidence
└── omega-package-advisory/      optional model-facing review tooling
```

The dependency direction is one-way:

```text
omega-package-manager ──→ omega-package-source ──→ omega-resolver-execution
                      └─→ omega-package-review
omega-package-advisory ──→ omega-package-manager
```

`omega-package-review` understands compiler semantics but cannot admit
packages. `omega-resolver-execution` understands host confinement but cannot
choose package identity or policy. `omega-package-manager` composes those
results, but mutating install and update transactions remain gated by
[`TASKS_PACKAGE_MANAGER.md`](../../../../TASKS_PACKAGE_MANAGER.md).

`omega-package-advisory` consumes deterministic bounded manager output. Its
model protocol and recommendations are optional and never participate in
acceptance.

Design references:

- [`omega-package-source/SOURCE_RESOLVER_SECURITY.md`](omega-package-source/SOURCE_RESOLVER_SECURITY.md)
- [`package_manager_first_draft.md`](../../../../wiki/design_briefs/package_manager_first_draft.md)
- [`build_and_package_model.md`](../../../../wiki/design_briefs/build_and_package_model.md)
