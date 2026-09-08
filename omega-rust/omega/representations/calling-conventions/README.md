# Calling conventions

The [public calling-plan contract](../../../../wiki/spec/build/calling_plans.md)
owns source semantics. This crate owns normalized `CallPlan`, `StatePlan`,
`BoundaryEntryPlan`, physical shape/placement vocabulary, built-in evaluators,
and validation. See [plans.rs](src/plans.rs) and the exported vocabulary in
[lib.rs](src/lib.rs). Callback parameter and destination identities live in
[callback_materializations.rs](src/callback_materializations.rs).

## Source-policy boundary

[provider-planning](../../build/provider-planning/src/calling_policy_plans.rs)
materializes a public signature graph, evaluates source-authored policy,
range-checks its output, and invokes the normalized validator. Current graph
nodes cover integers, floats, references, fixed arrays, and fixed records.
Opaque by-value uses close through representation selection first.

The source vocabulary is [std/calling.omg](../../../../source/library/std/calling.omg).
Its existing `Calling<C>` declaration remains behind the specified
`Calling<C, Policy>` explicit named-evidence surface. Policy evaluation itself
works; implementing the named-evidence selection is distinct from adding machine
parameters or runtime function values. Standard target evaluators still include
Rust implementations; source policies replace them only with differential
coverage and the same validator.

Source quantities are `u64`. Decoder capacities and narrower normalized fields
are private bounds, checked before conversion. Current capacities are named in
the materializer; do not copy them into another source of truth.

## Consumer obligations

Outbound imports, syscalls, vtables, service tables, and callbacks must retain
their exact plans rather than rederive target defaults. Composite adapters retain
each native subcall plan. Discarding a native status/count result removes no
result from ABI validation or footprint accounting and creates no semantic result
storage merely to represent the discarded value.

Structural parameter construction and caller materialization follow
[structural access](../../../../wiki/spec/terminal-psi/structural_access.md).
The existing tagged reference shape is not a proof that all structural producers
use it. Current native limits belong beside the
[lowerer](../../pipeline/abstract-operations-to-target-operations/README.md).

Final state-footprint certificates are a separate realization check; their
[production status](../../backend/images/image/footprint_replay.md) must not be
inferred from a passing plan evaluator.
