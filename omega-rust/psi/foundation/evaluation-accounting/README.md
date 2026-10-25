# Evaluation accounting

This foundation owns source-neutral resource accounts, movable storage leases,
and tagged measurement records. It contains no interpreter, source admission,
Build execution, or filesystem implementation.

`BuildEvaluationSponsor::new` retains the checked-tree schedule for existing
clients. `new_with_measurement` binds an explicit supported identity immutably.
The same account is shared across a package closure and retries; another engine
or storage schedule cannot spend that account's ceilings. Equal numeric fuel
markers never imply equal work or storage units.

`EvaluationInvocationAccount::reserve_site` checks fuel and live storage in one
aggregate-before-local transaction before engine execution. Refusal changes
neither counter; dropping the returned lease releases only live storage. Engine
storage owners move leases across frames/maps and share an owning backing lease
through `Arc`; copy-on-write must reserve the overlapping replacement before it
allocates. `EvaluationStorageLease::absorb` joins existing reservations only
within the exact invocation, without charging again. `truncate_cells` retires
that owner's removed rows without changing its payload reservation. Result publication must reserve copied live storage and charge
cumulative result custody before allocating or exposing a result. Custody is not
refunded on teardown or retry.

The checked-tree measurement preserves schema 7, state/statement/expression
fuel schedule 1, semantic cells and Text backing bytes. Terminal Psi measurement
uses schema 8, semantic operation/edge fuel schedule 1 and distinct logical
storage schedule 1. Terminal storage includes logical runtime rows, frame
headers, array/case payload elements and full retained byte backings once per
owning backing. This is not heap capacity, allocator overhead or process memory.

Usage records are telemetry, not admission or transferable sponsorship.
In-process `EvaluationInvocationUsage` retains exact account custody for issuer
checks. Cache records retain the explicit engine, fuel and storage identities
and reject unknown versions. The tree projection remains in checked-interpreter
and cannot manufacture a Terminal observation.

The common Terminal fuel meter can retain an exact invocation account and pay
it at each operation/selected edge before execution. Local resumable allowance
and the shared invocation/aggregate ceilings are independent: replenishment or
meter cloning cannot enlarge that shared account. A refused shared charge
commits neither local attribution nor account counters and remains a host error.
The source-produced Build integration test checks both exact-budget completion
and one-unit-short refusal across two activations with ordered binding effects.

Terminal byte backings now carry leases selected before entry allocation.
Aliases share the full backing reservation; copy-on-write reserves the overlapping
replacement, and boundary buffers transfer the same owner at writeback. Actual
Build execution tests exact and one-byte-short live payload budgets. Scalar and
structural runtime tables now reserve each new row and overlapping copy; frame
moves and joins transfer leases, and retiring rows releases their cells. The same
Build control checks exact and one-cell-short budgets through fuel pauses.
Root activations and saved call headers reserve one cell each before creation;
saved headers retire on return, while the root remains through readback until
instance close. Source-produced empty calls check peak depth rather than cumulative
call count. Packed scalar-array elements now own reservations through construction,
independent guest copies, frame transfers and retained result inspection. Inspection
aliases share one reservation; mutation funds any detached replacement first.
Descriptor maps, placed/case storage and cumulative result custody still need
their real lifetime hooks.
These partial snapshots are not complete Build usage or storage budgeting; do
not publish them as production Build accounting receipts. The ordinary
package-aware CLI remains on the checked Build evaluator until accounting and
host adapters are complete.

`BuildEvaluationSponsor::charge_pure_usage` atomically pays validated recorded
cost for an already admitted pure result. It checks the exact engine/schedule,
refuses filesystem-observing usage, spends cumulative fuel/log/result quotas,
and admits recorded peak demand beside existing reservations as an immediately
retired logical reservation. It returns current-account telemetry without
retaining the producing account. The caller still proves result identity,
current admission and absence of effects; a usage record proves none of those.
