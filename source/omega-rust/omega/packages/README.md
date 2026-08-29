# Omega Packages

Start in [`manager`](manager/README.md) for a user operation. Follow
[`source`](source/README.md) when the operation acquires hostile input, and
[`review`](review/README.md) when it turns checked compiler state into a local
admission decision.

```text
packages/
├── README.md       this entrance
├── manager/        command workflows, package graph, and local admission
├── source/         immutable acquisition and hostile process execution
└── review/         compiler evidence and optional advisory review
```

The dependency direction is one-way:

```text
manager ──→ source/acquisition ──→ source/execution
        └─→ review/evidence
review/advisory ──→ manager
```

`review/evidence` understands compiler semantics but cannot admit packages.
`source/execution` understands host confinement but cannot choose package
identity or policy. `manager` composes those results, but mutating install and
update transactions remain gated by [`TASKS_PACKAGE_MANAGER.md`](../../../../TASKS_PACKAGE_MANAGER.md).

`review/advisory` consumes deterministic bounded manager output. Its model
protocol and recommendations are optional and never participate in acceptance.

Design references:

- [`manager/SOURCE_RESOLVER_SECURITY.md`](manager/SOURCE_RESOLVER_SECURITY.md)
- [`package_manager_first_draft.md`](../../../../wiki/design_briefs/package_manager_first_draft.md)
- [`build_and_package_model.md`](../../../../wiki/design_briefs/build_and_package_model.md)
