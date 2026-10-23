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
allocates. Result publication must reserve copied live storage and charge
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

The Terminal engine currently has no live-storage insertion/readback hooks.
This foundation's unit controls establish accounting atomicity and lifetime
behavior only; they do not establish interpreter budgeting or enable sponsored
Terminal Build execution. The ordinary package-aware CLI remains on the checked
Build evaluator until real engine accounting and host adapters are connected.

`BuildEvaluationSponsor::charge_pure_usage` atomically pays validated recorded
cost for an already admitted pure result. It checks the exact engine/schedule,
refuses filesystem-observing usage, spends cumulative fuel/log/result quotas,
and admits recorded peak demand beside existing reservations as an immediately
retired logical reservation. It returns current-account telemetry without
retaining the producing account. The caller still proves result identity,
current admission and absence of effects; a usage record proves none of those.
