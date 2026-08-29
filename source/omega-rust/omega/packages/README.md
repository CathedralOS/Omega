# Omega Packages

Start in [`manager`](manager/README.md). It owns the package workflow and is the
only package crate called by the `omega` command.

```text
packages/
├── README.md             this entrance
├── advisory-tooling/     optional model-facing source-review protocol
├── manager/              workflow → manifest/source/package/graph/review
├── package-review/       checked compiler state → inert review evidence
└── resolver-execution/   confined native source-resolution processes
```

The dependency direction is one-way:

```text
manager ──→ package-review
        └─→ resolver-execution
advisory-tooling ──→ manager
```

`package-review` understands compiler semantics but cannot admit packages.
`resolver-execution` understands host confinement but cannot choose package
identity or policy. `manager` composes those results, but mutating install and
update transactions remain gated by [`TASKS_PACKAGE_MANAGER.md`](../../../../TASKS_PACKAGE_MANAGER.md).

`advisory-tooling` consumes deterministic bounded manager output. Its model
protocol and recommendations are optional and never participate in acceptance.

Design references:

- [`manager/SOURCE_RESOLVER_SECURITY.md`](manager/SOURCE_RESOLVER_SECURITY.md)
- [`package_manager_first_draft.md`](../../../../wiki/design_briefs/package_manager_first_draft.md)
- [`build_and_package_model.md`](../../../../wiki/design_briefs/build_and_package_model.md)
