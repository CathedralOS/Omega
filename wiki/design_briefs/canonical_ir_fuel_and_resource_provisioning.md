# Design Brief: Canonical IR Fuel And Resource Provisioning

Status: architectural direction settled 2026-07-30. The hard-root accounting
precursor is schedule-keyed and uses logical-fuel provisions; canonical portable
IR, metering, and general fixed-work segment checking remain implementation
work. The concrete v1 portable-IR contract is owner-blocked on
`OWNER_QUESTIONS.md` #4; the current evaluator-step schedule is telemetry
precursor evidence, not canonical-IR fuel.

## Context

WCSU proves a spatial fact: a closed activation needs at most a derived amount
of stack. Logical-work accounting has three distinct customers:

- deterministic metering of work that actually executes;
- a restricted theorem for paths that must fit a fixed logical budget; and
- attributed response reporting for waits and edges with no finite guarantee.

General parametric work functions, arbitrary recurrence solving, and WCET are
not prerequisites for those facilities.

## Canonical portable IR

Omega's distributable, interpreter-defined artifact is a versioned
**canonical IR**. It is distinct from mutable compiler optimization
representations. The reference oracle executes this IR; native code is an
acceleration produced from it.

IR semantics and accounting are independently versioned:

```text
PortableIrIdentity {
    semantic_version;
    program_fingerprint;
}

FuelScheduleIdentity {
    schedule_version;
}
```

Changing the fuel schedule changes accounting, not program meaning. Cached
semantic results therefore key on IR semantics and program identity; cost
records additionally key on the fuel schedule.

## Logical fuel

The fuel schedule assigns deterministic logical cost to canonical IR
instructions or normalized blocks. Fuel is not native instruction count,
cycles, energy, or wall-clock time.

The execution sponsor supplies a budget. Executed code cannot inspect its
remaining fuel, branch on budget policy, catch exhaustion as a machine result,
or distinguish interpreted from natively metered execution. Exhaustion is a
sponsor event: the host may replenish and resume, cancel, or terminate
according to installation policy.

The same denomination serves:

- build-time evaluation by executing canonical IR in the evaluator;
- portable interpreted artifacts through direct metering; and
- native realizations whose trusted lowering inserts counters that charge the
  corresponding canonical IR blocks.

Optimization may reduce physical work without reducing logical fuel. A
compiler release may not silently change budget behavior merely because its
native lowering improved.

Build usage remains deterministic for the concrete invocation, target
description, evaluator/IR semantics, and fuel schedule. It never depends on
host load or elapsed time. Long terminating builds remain legal; progress,
warnings, cache accounting, and optional root-selected ceilings consume the
meter without making the ceiling program semantics.

## Restricted fixed-work checking

A restricted checker may prove:

> Entry `E`, under preconditions `P`, executes at most `K` units under fuel
> schedule `S`.

The supported fragment has constant-bounded iteration, bounded call
multiplicity, acyclic or explicitly measured call structure, and no unresolved
blocking or foreign-completion edge. The checker applies to a whole hard-root
entry or to a selected path segment ending at the next semantic safe point.

The public certificate keys the canonical IR, entry, relevant preconditions,
fuel schedule, and scalar ceiling. Private proof or optional diagnostic
evidence may retain the maximizing path; it has no semantic identity and does
not seed target WCET analysis. An edge without a finite response contract
retains the exact attribution that prevented closure.

Static premises may be discharged at installation. Invocation-dependent
premises are ordinary call obligations and must hold at each meter-free call.

A sponsor may execute a fixed-work entry natively without runtime metering when
trusted lowering and installation establish that the executing bytes came
from the certified IR and the proved ceiling fits the granted fuel. IR without
such a theorem remains safely executable under interpreter metering or trusted
inserted native metering. A certificate that arbitrary native bytes refine the
IR is a separate future proof-carrying-code chain.

Provider-local `FixedFuelProviderSummary` and `LogicalFuelResourceColumn` are
the current implementation precursor for hard roots. Each summary and
provision names a nonzero `FuelScheduleIdentity`; composition rejects mixed
schedules, and the external-root artifact publishes the schedule version,
provision, ceiling, and composed units. These units are provider-authored
logical-fuel summaries, not a derivation from canonical IR. The precursor still
must migrate to IR-derived entry/segment certificates and does not grow into
general symbolic complexity analysis.

## Response and physical time

Logical compute and response are separate:

```text
pure fixed path       work: Bounded(K)   response: finite under a timing model
block mutex.lock()    work: local bound  response: NoFiniteGuarantee(mutex.lock)
suspend io.read()     work: local bound  response: NoFiniteGuarantee(io.read)
```

A selected-point report has three honest outcomes:

- `Bounded(K, evidence)` when restricted fixed-work checking closes;
- `Unknown(reason)` when the checker cannot prove a bound; and
- `NoFiniteGuarantee(edge)` when a reachable wait or foreign edge publishes no
  finite response contract.

A hard-control profile requiring bounded response rejects `Unknown` and
`NoFiniteGuarantee` at its roots. Force-terminating a blocked holder is not a
substitute for a response theorem.

A monotonic clock or performance counter may report one observed execution
under a target provider. Observation is not a future guarantee. Converting a
logical or target-work ceiling to a statement such as `<= 850 us` requires a
separate derived or admitted worst-case timing model.

Fuel and target WCET optimize different cost functions, so their maximizing
paths may differ. A future real-time analysis re-searches target paths. It may
reuse structural enabling facts from the IR certificate, but lowering must
also show that helper calls, expansions, and other target realization choices
introduce no unbounded structure.

A strict real-time profile needs analyzable evidence for every dependency:
canonical IR, a separately verifiable native WCET certificate, or an admitted
target-specific summary when policy permits one. Portable IR is the preferred
distribution form, not the only mathematically possible evidence source.

## Spatial resources are provisioned

Omega does not add one flat `memory_budget` meter. Allocation and storage
already require authority. A sponsor provisions the concrete resources an
execution may use:

- independently sized `Extent` or allocator capabilities;
- WCSU-derived stacks and activation-stack pools;
- static image/code storage admitted at installation; and
- qualified extents for pinned, shared, physical, DMA-visible, persistent, or
  other provider-defined memory.

Multiple heaps are multiple allocator or `Extent` values. A component receives
bounded child storage authority instead of ambient access to a global allocator.
External retained storage remains ordinary claim and custody accounting.

Infallible allocation in the package-level bump canary is the first concrete
customer for a `CountedQuantity<Bytes>` content algebra. Allocation consumes
normalized size, alignment padding, and metadata from a proof-level natural
residual magnitude keyed by the `Bytes` unit identity. The residual tail
`Extent` supplies placement; released extents become retired content and do not
restore bump capacity until reset recomposes the original backing. A scalar
free-byte count does not prove placement in a fragmented general heap. Such
allocators remain fallible or require an exact free-extent/reservation theorem.

## Contracts, installation, and proof-carrying code

Fuel and spatial provisions normally belong to an execution sponsor or
installation profile, not API/ABI identity. A replacement may require more
fuel or provision while remaining semantically compatible; installation
rejects or reprovisions it. A deadline or fixed resource ceiling enters the
interface contract only when an API deliberately promises it.

The proof-carrying-code scope in this brief is canonical IR. Its verifier may
check memory safety, ownership and resource conservation, reach, termination,
and fixed-fuel certificates without trusting the producing compiler. Native
lowering/refinement certificates have a different subject and TCB and remain a
separate future lane.

## Implementation sequence

1. Define and version the canonical portable IR independently from optimizer
   representations.
2. Define the separately versioned logical fuel schedule and interpreter meter.
3. Feed build-time evaluation usage, progress, warnings, and optional policy
   from that meter.
4. Migrate provider-local fixed-work summaries to IR fuel and generalize them
   to selected safe-point segments.
5. Preserve `Bounded`, `Unknown`, and attributed no-finite-guarantee outcomes
   in artifacts and diagnostics.
6. Add trusted native block metering; defer a separate IR-to-native PCC chain.
7. Add `CountedQuantity<Bytes>` with the package-level bump-allocation canary;
   retain exact tail placement and keep general fragmented allocators fallible
   unless they supply placement/reservation evidence.
