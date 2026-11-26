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

[Boundary values](src/boundary_values.rs) defines those shapes. Today the
backend chooses how to classify an aggregate from its ABI tag, including SysV
eightbyte and AAPCS64 homogeneous-float rules. It applies that choice again
when checking the class returned by a source policy.

The source vocabulary is [system/calling.omg](../../../../../source/library/system/calling.omg).
`Calling<Policy>` selects the named implementation argument declared
`Policy satisfies CallingPolicy`. Psi checks the implementation slot in order;
[policy implementations](../../realization/provider-planning/src/calling_policy_plans/policy_implementations.rs)
closes its application and selects the exact `CallingPolicy::plan` row. The
row's machine is called by symbol, not through an attached `Data::plan` name.

The [plan decoder](../../realization/provider-planning/src/calling_policy_plans/build_time_decoding.rs)
currently requires one of six foreign ABI tags. The [Rust planners](src/plans/call_plan_evaluation.rs)
also repeat placement rules, including the 32 bytes of shadow space already
declared by the [Windows x64 policy](../../../../../source/library/system/targets/windows_x86_64/calling.omg).
The [authorship contract](../../../../../wiki/spec/build/calling_plans.md#authorship-and-evaluation)
requires each foreign policy to own these choices. Retire the tag and the Rust
classification and placement rules as the source policies cover those signatures.
The compiler still checks each value against its public shape, validates
registers, stack ranges and entry control for the target, and emits the plan.

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
[lowerer](../../pipeline/04_abstract-operations-to-target-operations/README.md).

Plan evaluation establishes the permitted machine-state use. The backend must
honor that ceiling when selecting and emitting instructions, including generated
entry and exit code. See [machine-state realization](../../../../../wiki/spec/build/machine_state_evidence.md).

## Cross-check against clang

[clang_crosscheck.rs](src/tests/clang_crosscheck.rs) compares where the Rust
foreign ABI planners and clang put each argument and result byte for 1,440
fixed generated C signatures on all four hosted targets. The platform
ABI documents stay the answer key. Every admitted signature must agree;
refused signatures are counted separately, and any mismatch or unreadable
clang output fails the test. It needs clang, so it is ignored by
default; run it by hand on a LAN VM that has clang:
`mbx test -p calling-conventions --lib clang_crosscheck -- --ignored --nocapture`.
