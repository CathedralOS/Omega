# Omega Packages

Start in [`manager`](manager/README.md) for anything the `omega` binary does.
The two supporting areas follow the package lifecycle downward.

```text
packages/
├── README.md                    this entrance
├── manager/                     complete workflows and package policy
├── source/                      immutable local and Git source acquisition
│   └── resolver-execution/      confined native source helper processes
└── review/                      support for package review
    ├── evidence/                checked compiler state as inert evidence
    └── advisory/                optional model-facing review tooling
```

The dependency direction is one-way:

```text
manager ──→ source ──→ source/resolver-execution
        └─→ review/evidence
review/advisory ──→ manager
```

`review/evidence` understands compiler semantics but cannot admit packages.
`source/resolver-execution` understands host confinement but cannot choose
package identity or policy. `manager` composes those results, but mutating
install and update transactions remain gated by
[`TASKS_PACKAGE_MANAGER.md`](../../../../TASKS_PACKAGE_MANAGER.md).

`review/advisory` consumes deterministic bounded manager output. Its
model protocol and recommendations are optional and never participate in
acceptance.

Design references:

- [`source/SOURCE_RESOLVER_SECURITY.md`](source/SOURCE_RESOLVER_SECURITY.md)
- [`package_manager_first_draft.md`](../../../../wiki/design_briefs/package_manager_first_draft.md)
- [`build_and_package_model.md`](../../../../wiki/design_briefs/build_and_package_model.md)
