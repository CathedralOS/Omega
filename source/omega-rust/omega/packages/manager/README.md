# Omega Package Manager

This is the operation-owning `omega-package-manager` crate. Start at
[`src/lib.rs`](src/lib.rs), then enter the operation you are following.

```text
manager/
├── Cargo.toml
├── README.md
├── src/
│   ├── lib.rs             Rust entrance
│   ├── operations/        complete package-aware operations
│   ├── admission/         consumer-owned evidence promotion gates
│   ├── declarations/      checked package declarations from build.omg
│   ├── resolution/        exact source selection and dependency closure
│   └── review/            compile, compare, audit, and decide
└── tests/                 manager integration tests
```

`operations` is the only owner of complete user or compiler workflows.
`declarations` reads the statically checked package declarations in
`build.omg`; Omega has no second package manifest. `resolution` binds those
declarations to immutable sources and reconciles one exact closure. `review`
turns that closure into compiler-issued facts and root-owned decisions. Its
`reconstruction/root_policy.rs` gate rederives fresh obligations and conflicts
from the same closure, requires exact conflict bijections for open accepted
claims, dangerous authorities, and external executable supplies, then accepts
blocking rows only through their exact candidate-bound root policy. The result
remains review and policy state. `admission` owns the stronger consumer-side
boundary: it rechecks live source custody and the complete reconstruction and
policy replay before producing in-memory accepted ordinary evidence. That
evidence still has no codec, `omega.lock` mutation route, `PackageInstance`, or
transaction authority.

Install and update belong in `operations/` when their remaining acceptance and
transaction gates are closed. The source and review crates remain subordinate
and cannot admit packages independently.

Return to the [package subsystem map](../README.md), or consult:

- [`package_manager_first_draft.md`](../../../../../wiki/design_briefs/package_manager_first_draft.md)
- [`build_and_package_model.md`](../../../../../wiki/design_briefs/build_and_package_model.md)
- [`TASKS_PACKAGE_MANAGER.md`](../../../../../TASKS_PACKAGE_MANAGER.md)
