# Calling conventions

The [public calling-plan contract](../../../../../wiki/spec/build/calling_plans.md)
defines source meaning. This crate defines normalized `CallPlan`, `StatePlan`,
`BoundaryEntryPlan`, physical shape and placement types, built-in evaluators
and validation. See [plans.rs](src/plans.rs) and the exported vocabulary in
[lib.rs](src/lib.rs). Callback parameter and destination identities live in
[callback_materializations.rs](src/callback_materializations.rs).

## Source-policy boundary

[provider-planning](../../realization/provider-planning/src/calling_policy_plans.rs)
builds a public signature graph, runs the source-written policy, range-checks
its output and calls the normalized validator. Graph nodes cover integers,
floats, references, fixed arrays and fixed records. Opaque by-value uses are
resolved through representation selection first.

[Boundary values](src/boundary_values.rs) defines those source-free shapes and
their per-policy ABI classes. The same graph serves compatibility calling
plans and class checks of authored plans; SysV eightbyte and AAPCS64
homogeneous-float rules are backend decisions. Matching classes does not
replace placement or full calling-plan validation.

The source vocabulary is [std/calling.omg](../../../../../source/library/std/calling.omg).
`Calling<Policy>` selects the named implementation argument declared
`Policy satisfies CallingPolicy`. Psi checks the implementation slot in order;
[policy implementations](../../realization/provider-planning/src/calling_policy_plans/policy_implementations.rs)
closes its application and selects the exact `CallingPolicy::plan` row. The
row's machine is called by symbol, not through an attached `Data::plan` name.
Some standard target evaluators are still Rust; a source policy replaces one
only with differential tests and the same validator.

Source quantities are `u64`. Decoder capacities and narrower normalized
fields are internal limits, checked before conversion. The graph builder names
current capacities; do not copy them elsewhere.

## Consumer obligations

[NativeParameterTelescope](src/boundary_application/native_telescope.rs) builds
the native argument list for source preparation and canonical replay. It
keeps semantic identities and shapes, inserts target-sized direct callback
arguments, and checks one contiguous native order. Direct destinations come
from those rows, not a parallel list. Source layout matching stays with the
source adapter.

[TerminalCallingTelescope](src/boundary_application/terminal_telescope.rs)
rebuilds the pairing of native callback arguments and binders from the exact
reached Terminal boundary declaration. Its ordinary argument shapes belong to
the proposed value-layout graph; native-only pointer shapes, parameter
identities and order, and binder contracts do not. The proposed application's
telescope must agree before its plan is used.

Outbound imports, syscalls, vtables, service tables and callbacks keep their
exact plans instead of rederiving target defaults. Composite adapters keep
each native subcall plan. A discarded native status or count result stays in
ABI validation and footprint accounting, and gets no semantic storage.

Structural parameter construction and caller materialization follow
[structural access](../../../../../wiki/spec/terminal-psi/structural_access.md).
The tagged reference shape existing does not prove every structural producer
uses it. Current native limits belong beside the
[lowerer](../../pipeline/02_abstract-operations-to-target-operations/README.md).

Final state-footprint certificates are a separate realization check; a
passing plan evaluator says nothing about their
[production status](../images/image/src/lib.rs).
