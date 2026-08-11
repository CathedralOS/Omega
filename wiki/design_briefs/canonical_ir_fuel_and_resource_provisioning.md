# Design Brief: Terminal Psi, Fuel, And Resource Provisioning

Status: canonical Psi architecture settled 2026-08-02. This brief states the
current semantic contract; incomplete implementation work is tracked in
`TASKS.md`. The representation cut is detailed in
[`terminal_psi.md`](../architecture/pipeline/terminal_psi.md).

Terminal Psi is pre-release. Its producer, verifier, interpreter, and Omega
consumers move as one vocabulary; stale artifacts reject. Git history, not this
brief, retains superseded encodings and implementation checkpoints.

## Context

WCSU proves a spatial fact: a closed activation needs at most a derived amount
of stack. Logical-work accounting has three distinct customers:

- deterministic metering of work that actually executes;
- a restricted theorem for paths that must fit a fixed logical budget; and
- attributed response reporting for waits and edges with no finite guarantee.

General parametric work functions, arbitrary recurrence solving, and WCET are
not prerequisites for those facilities.

## Terminal Psi

Psi operates on Omega-branded source files and owns the complete target-neutral
pipeline: parsing, resolution, typing, semantic checking, proof and obligation
construction, expression lowering, and canonicalization. Its terminal product
is the one canonical portable execution representation consumed by Omega.
Omega begins with terminal Psi and owns provider installation, target
realization, optimization, ABI lowering, native emission, and execution.

```text
Omega files
    -> Psi parse / resolve / type / check / lower / canonicalize
    -> terminal Psi
    -> Psi reference interpreter (oracle)
       or Omega realization for a target
```

There is no Omega-to-Psi-to-Omega pipeline and no separate public source
language called Psi. The names mark an implementation and trust boundary:
Omega is the user-facing language and platform brand; Psi owns its checked
portable semantics.

Terminal Psi is distinct from mutable compiler optimization representations.
The reference oracle executes it directly; native code is an acceleration
lowered from the same module. Terminal artifacts are concrete and
post-instantiation. Generic parsing, checking, and instantiation may occur in
nonterminal Psi forms, but the interpreter, verifier, and Omega lowering do not
need generic execution semantics.

Psi semantics and accounting have independent identities:

```text
TerminalPsiIdentity {
    vocabulary_marker;
    program_fingerprint;
}

FuelScheduleIdentity {
    schedule_marker;
}
```

Changing the fuel schedule changes accounting, not program meaning. Cached
semantic results therefore key on Psi semantics and program identity; cost
records additionally key on the fuel schedule.

### Semantic and proof boundary

The detailed representation, operation-slice discipline, verifier split,
canonical bytes, and artifact identities are specified once in
[`terminal_psi.md`](../architecture/pipeline/terminal_psi.md). The constraints
that matter to fuel and resource provisioning are:

- terminal Psi is immutable, self-contained, concrete, and target-neutral;
- every executable choice that changes behavior or generated obligations has a
  closed static identity;
- execution, propositions, evidence, fuel, diagnostics, and lowering refer to
  the same stable values, places, operations, and edges;
- author-declared hardware geometry remains semantic, while target-selected
  layout, ABI classes, registers, storage regions, and instructions belong to
  Omega; and
- the reference interpreter and native lowering implement the normative
  operation semantics; agreement between them is a test, not the definition.

The artifact verifier reconstructs the complete obligation set from the
semantic module and its fingerprinted contracts. The proof kernel checks
evidence for that reconstructed set. A proof bundle cannot choose what must be
proved, and an admission is valid only at a sealed site accepted by the active
profile. Each accepted fact is re-decided by a total kernel judgment, proved by
checked evidence, or explicitly admitted; unsupported entailment rejects.

Semantic bytes, replaceable proof evidence, installation decisions, and debug
maps have separate identities under one manifest. Proof improvement does not
change program identity. Canonical decoding accepts only the current
pre-release vocabulary, so producers and consumers change together and stale
artifacts reject.

The vocabulary grows only through complete vertical slices: encoding,
execution, reconstructed obligations and authorized admissions, proof rule and
soundness argument, interpretation, Omega lowering, and fuel identity. Scoped
ordering operations remain distinct semantic events; proof evidence alone
cannot create runtime ordering. Their participant and realization rules are
specified in [`concurrency_atomics.md`](concurrency_atomics.md) and the hardware
foundation briefs
([freestanding](freestanding_boot_and_hardware_facts.md),
[memory and devices](os_memory_and_hardware_foundation.md)).

## Logical fuel

The fuel schedule assigns deterministic logical cost to terminal Psi
instructions or normalized blocks. Fuel is not native instruction count,
cycles, energy, or wall-clock time.

The execution sponsor supplies a budget. Executed code cannot inspect its
remaining fuel, branch on budget policy, catch exhaustion as a machine result,
or distinguish interpreted from natively metered execution. Exhaustion is a
sponsor event: the host may replenish and resume, cancel, or terminate
according to installation policy.

The same denomination serves:

- build-time evaluation by executing terminal Psi in the evaluator;
- portable interpreted artifacts through direct metering; and
- native realizations whose trusted lowering inserts counters that charge the
  corresponding terminal-Psi blocks.

Optimization may reduce physical work without reducing logical fuel. A
compiler release may not silently change budget behavior merely because its
native lowering improved.

A value-less `ReturnUnit` is still one taken normal-return edge. It has the
same edge charge as a scalar return and no invented value-producing operation.

Build usage remains deterministic for the concrete invocation, target
description, evaluator/Psi semantics, and fuel schedule. It never depends on
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

The public certificate keys terminal Psi, the entry, relevant preconditions,
fuel schedule, and scalar ceiling. Private proof or optional diagnostic
evidence may retain the maximizing path; it has no semantic identity and does
not seed target WCET analysis. An edge without a finite response contract
retains the exact attribution that prevented closure.

Static premises may be discharged at installation. Invocation-dependent
premises are ordinary call obligations and must hold at each meter-free call.

A sponsor may execute a fixed-work entry natively without runtime metering when
trusted lowering and installation establish that the executing bytes came
from the certified Psi module and the proved ceiling fits the granted fuel. Psi
without such a theorem remains safely executable under interpreter metering or
trusted inserted native metering. A certificate that arbitrary native bytes
refine terminal Psi is a separate future proof-carrying-code chain.

The live hard-root precursor composes recomputable entry/segment certificates
and admitted opaque-provider summaries under one `FuelScheduleIdentity`.
Installation rechecks whole-entry evidence against the exact terminal identity,
architecture, frozen code, entry stub, and function offset; a segment
certificate cannot authorize a whole root. Cathedral migration and general
loop/build-time coverage remain in `TASKS.md`. This path does not grow into a
symbolic complexity language.

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
reuse structural enabling facts from the Psi certificate, but lowering must
also show that helper calls, expansions, and other target realization choices
introduce no unbounded structure.

A strict real-time profile needs analyzable evidence for every dependency:
terminal Psi, a separately verifiable native WCET certificate, or an admitted
target-specific summary when policy permits one. Terminal Psi is the preferred
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
`Extent` supplies placement; released extents leave the allocation's live
custody frontier but do not restore bump capacity until reset recomposes the
original backing. A scalar free-byte count does not prove placement in a
fragmented general heap. Such
allocators remain fallible or require an exact free-extent/reservation theorem.

## Contracts, installation, and proof-carrying code

Fuel and spatial provisions normally belong to an execution sponsor or
installation profile, not API/ABI identity. A replacement may require more
fuel or provision while remaining semantically compatible; installation
rejects or reprovisions it. A deadline or fixed resource ceiling enters the
interface contract only when an API deliberately promises it.

The proof-carrying-code scope in this brief is terminal Psi. Its verifier may
check memory safety, ownership and resource conservation, reach, termination,
and fixed-fuel certificates without trusting the producing compiler. Native
lowering/refinement certificates have a different subject and TCB and remain a
separate future lane.

The verifier/kernel split above is settled; its final trust placement is not.
The Psi-aware verifier may gain a low-rung reference implementation, emit a
reconstruction derivation checked by the low kernel, or remain explicitly
trusted. A Psi-hosted generic kernel may accelerate or cross-check proofs, but
does not establish that the verifier reconstructed the right obligations.

## Implementation constraints

- Keep legacy interpreters or lowerers only as differential oracles while their
  consumer moves; they never define a second semantic path.
- Bind evidence to exact semantic and reconstructed-obligation identities. A
  certificate-provided proposition is never authoritative.
- Preserve accounting provenance through build-time and native metering, and
  retain `Bounded`, `Unknown`, and attributed no-finite-guarantee outcomes.
- Keep spatial authority concrete: content frontier rows and allocator canaries
  must retain exact placement, custody, and recomposition evidence.

`TASKS.md` owns the live implementation sequence and acceptance criteria.
