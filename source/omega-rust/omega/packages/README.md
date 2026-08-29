# Omega Packages

Start in [`manager`](manager/README.md). It owns the package workflow and is the
only package crate called by the `omega` command.

```text
packages/
├── README.md             this entrance
├── manager/              manifest → source → graph → review
├── compiler-review/      checked compiler state → inert review evidence
└── resolver-execution/   confined native source-resolution processes
```

The dependency direction is one-way:

```text
manager ──→ compiler-review
        └─→ resolver-execution
```

`compiler-review` understands compiler semantics but cannot admit packages.
`resolver-execution` understands host confinement but cannot choose package
identity or policy. `manager` composes those results, but mutating install and
update transactions remain gated by [`TASKS_PACKAGE_MANAGER.md`](../../../../TASKS_PACKAGE_MANAGER.md).

Design references:

- [`manager/SOURCE_RESOLVER_SECURITY.md`](manager/SOURCE_RESOLVER_SECURITY.md)
- [`package_manager_first_draft.md`](../../../../wiki/design_briefs/package_manager_first_draft.md)
- [`build_and_package_model.md`](../../../../wiki/design_briefs/build_and_package_model.md)
