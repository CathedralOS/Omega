# Calling conventions

The [public calling-plan contract](../../../../../wiki/spec/build/calling_plans.md)
owns source semantics. This crate owns normalized `CallPlan`, `StatePlan`,
`BoundaryEntryPlan`, physical shape/placement vocabulary, built-in evaluators,
and validation. See [plans.rs](src/plans/mod.rs) and the exported vocabulary in
[lib.rs](src/lib.rs). Callback parameter and destination identities live in
[callback_materializations.rs](src/callback_materializations/mod.rs).

## Source-policy boundary

[provider-planning](../../realization/provider-planning/src/calling_policy_plans/mod.rs)
materializes a public signature graph, evaluates source-authored policy,
range-checks its output, and invokes the normalized validator. Current graph
nodes cover integers, floats, references, fixed arrays, and fixed records.
Opaque by-value uses close through representation selection first.

[Boundary values](src/boundary_values.rs) owns that source-free shape vocabulary
and policy-specific ABI classification. Signature materialization borrows the
same graph for compatibility calling plans and authored-plan class checking;
SysV eightbyte and AAPCS64 homogeneous-float rules are backend decisions.
Class compatibility does not replace placement or full calling-plan validation.

The source vocabulary is [std/calling.omg](../../../../../source/library/std/calling.omg).
`Calling<Policy>` selects the named implementation argument declared
`Policy satisfies CallingPolicy`. Psi checks the ordered implementation slot;
[policy implementations](../../realization/provider-planning/src/calling_policy_plans/policy_implementations.rs)
closes its application and selects the exact `CallingPolicy::plan` row.
The row's machine is invoked by symbol, not by an attached `Data::plan` name.
Standard target evaluators still include
Rust implementations; source policies replace them only with differential
coverage and the same validator.

Source quantities are `u64`. Decoder capacities and narrower normalized fields
are private bounds, checked before conversion. Current capacities are named in
the materializer; do not copy them into another source of truth.

## Consumer obligations

[NativeParameterTelescope](src/boundary_application/native_telescope.rs) constructs
the native argument roster for source preparation and canonical replay. It
preserves semantic identities/shapes, inserts target-sized direct callback
arguments, and checks one contiguous native order. Direct destination demands
come from those emitted rows, not a second parallel list. Source layout
correspondence stays with the source-owned adapter.

Native callback argument/binder correspondence is reconstructed by
[TerminalCallingTelescope](src/boundary_application/terminal_telescope.rs) from
the exact reached Terminal boundary declaration. Its ordinary argument shape
roots still belong to the proposed value-layout graph; native-only pointer
shape, parameter identities/order, and callback binder contracts do not.
The proposed application's telescope must agree before its plan is consumed.

Outbound imports, syscalls, vtables, service tables, and callbacks must retain
their exact plans rather than rederive target defaults. Composite adapters retain
each native subcall plan. Discarding a native status/count result removes no
result from ABI validation or footprint accounting and creates no semantic result
storage merely to represent the discarded value.

Structural parameter construction and caller materialization follow
[structural access](../../../../../wiki/spec/terminal-psi/structural_access.md).
The existing tagged reference shape is not a proof that all structural producers
use it. Current native limits belong beside the
[lowerer](../../pipeline/02_abstract-operations-to-target-operations/README.md).

Final state-footprint certificates are a separate realization check; their
[production status](../images/image/src/lib.rs) must not be
inferred from a passing plan evaluator.
