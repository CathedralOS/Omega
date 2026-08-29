# Omega Packages

Start in [`manager`](manager/README.md) for a user operation. The directory
names describe responsibilities; Cargo metadata retains the fully qualified
crate names used by Rust.

```text
packages/
├── README.md                    this entrance
├── manager/                     operations, declarations, graph, and review policy
├── source/                      immutable local and Git source acquisition
├── resolver-execution/          confined native resolver processes
├── evidence/                    checked compiler state as inert package evidence
└── advisory/                    optional model-facing review tooling
```

The dependency direction is one-way:

```text
omega-package-manager ──→ omega-package-source ──→ omega-resolver-execution
                      └─→ omega-package-evidence
omega-package-advisory ──→ omega-package-manager
```

`omega-package-evidence` understands compiler semantics but cannot admit
packages. `omega-resolver-execution` understands host confinement but cannot
choose package identity or policy. `omega-package-manager` composes those
results, but mutating install and update transactions remain gated by
[`TASKS_PACKAGE_MANAGER.md`](../../../../TASKS_PACKAGE_MANAGER.md).

`omega-package-advisory` consumes deterministic bounded manager output. Its
model protocol and recommendations are optional and never participate in
acceptance.

Design references:

- [`source/SOURCE_RESOLVER_SECURITY.md`](source/SOURCE_RESOLVER_SECURITY.md)
- [`package_manager_first_draft.md`](../../../../wiki/design_briefs/package_manager_first_draft.md)
- [`build_and_package_model.md`](../../../../wiki/design_briefs/build_and_package_model.md)
