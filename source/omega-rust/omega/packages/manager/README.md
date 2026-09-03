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
Its package-aware native operation consumes a `PreparedLocalProject`, selects
one exact target child, compiles the final production review, reconstructs its
current initial conflict set, and recovers a caller-selected root-policy record
only against that set. Accepted evidence then enters the manager-owned retained
native route with a distinct receiving permission policy. Preparation retains
the source closure needed for this handoff but neither discovers policy nor
infers permissions. A root-policy file is required exactly when the fresh
review has blocking rows.
`declarations` reads the statically checked package declarations in
`build.omg`; Omega has no second package manifest. `resolution` binds those
declarations to immutable sources and reconciles one exact closure. `review`
turns that closure into compiler-issued facts and root-owned decisions. Its
`reconstruction/root_policy.rs` gate rederives fresh obligations and conflicts
from the same closure, requires exact conflict bijections for open accepted
claims, dangerous authorities, and external executable supplies, then accepts
blocking rows only through their exact candidate-bound root policy. Contract-
entailment obligations remain unadmittable while open; canonically recorded,
locally rechecked assumption discharges compose separately across the complete
closure with their original package owners. The result remains review and
policy state.
`admission` owns the stronger consumer-side
boundary: it rechecks live source custody and the complete reconstruction and
policy replay before producing in-memory accepted ordinary evidence. That
evidence retains exact compiler-consumed semantic bindings scoped to their
consuming package, while every resulting blocker still requires fresh root
policy. It still has no codec, `omega.lock` mutation route, `PackageInstance`,
or transaction authority.

`compile_resolved_package_candidate_reviews` is the install/update candidate
entrance. It uses one preliminary compiler review only to discover supported
package-owned semantic surfaces, then recompiles with exact consumer-scoped
bindings. Only that final review may proceed to conflicts and admission; the
discovery pass is neither policy nor evidence that an audit occurred.
For a requirement-only service candidate, the preliminary review exposes the
exact checked `ServiceSchema` beside the proposed binding. This is
non-authoritative review material: a consumer may use complete requirement
identities from that schema to author terminal-permission rows, but neither the
candidate nor the manager assigns classes from service paths or method names.
The resulting binding must still survive the complete final recompilation and
ordinary root-policy admission.

Production-bearing operations use
`compile_resolved_package_candidate_for_production`. Its non-clonable result
exposes the same final review rows while retaining the exact checked
application root that produced them. Admission consumes that root directly
into unpublished Terminal/native production after fresh evidence comparison;
it does not rerun `build.omg` or recover generated source from staging.

Install and update belong in `operations/` when their remaining acceptance and
transaction gates are closed. The source and review crates remain subordinate
and cannot admit packages independently.

Return to the [package subsystem map](../README.md), or consult:

- [`package_manager_first_draft.md`](../../../../../wiki/design_briefs/package_manager_first_draft.md)
- [`build_and_package_model.md`](../../../../../wiki/design_briefs/build_and_package_model.md)
- [`TASKS_PACKAGE_MANAGER.md`](../../../../../TASKS_PACKAGE_MANAGER.md)
