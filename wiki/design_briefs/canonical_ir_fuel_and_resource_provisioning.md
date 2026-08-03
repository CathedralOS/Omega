# Design Brief: Terminal Psi, Fuel, And Resource Provisioning

Status: canonical Psi architecture settled 2026-08-02. The hard-root accounting
precursor is schedule-keyed and uses logical-fuel provisions. The first
terminal-Psi schedule, interpreter meter, and straight-line fixed-entry checker
are live; build-time migration, general safe-point segment checking, response
outcomes, and native metering remain implementation work. The current
TypedTrees evaluator-step schedule is
telemetry precursor evidence, not canonical-Psi fuel. The implementation cut
and migration are detailed in
[`terminal_psi.md`](../architecture/pipeline/terminal_psi.md).

Foundation checkpoint (2026-08-02): Psi-owned core and proof-kernel crates now
carry stable semantic identities, typed scalar propositions checked against a
module value table, total primitive judgments, explicit structural proof
certificates, and sealed admission evidence bound to an exact authorized site,
authority identity, evidence identity, and installation-profile decision. The
kernel rejects admission for a primitively derivable proposition. That
foundation was the prerequisite evidence substrate; the executable checkpoints
below extend it.

Executable checkpoint (2026-08-02): the in-memory terminal semantic module now
carries stable machine/block/value/operation/edge identities, representable
integer constants, v2 Boolean constants, v3 exact-width wrapping integer
addition, v4 exact-width saturating integer addition, v5 exact-width
wrapping integer subtraction, v6 exact-width saturating integer subtraction,
v7 exact-width wrapping integer multiplication, and v8 exact-width saturating
integer multiplication, unconditional jump/return control, and bodyful
contracts. Semantic v9 adds proof-only structural places and content
conservation, v10 adds identity-preserving claim reshuffles, v11 adds stable
sum-case content-path segments, and current v12 adds exact authored-partition
substitution rows without adding executable operations. The
verifier reconstructs operation, edge-binding, and return-binding axioms from
the executable path, rejects unreachable fact sources and out-of-scope contract
values, and requires evidence for every `ensures`; the proof kernel checks
semantic-axiom citations, equality composition, and closed integer relations
over all six arithmetic terms. Wrapping addition reduces modulo the declared
1–128-bit width and interprets signed reduced bits as two's complement;
saturating addition clamps at the declared signed or unsigned bounds. Wrapping
and saturating subtraction apply the same policies to `left - right`;
wrapping multiplication reduces the product at the declared width, while
saturating multiplication clamps it at the declared bounds. All six are total
and create no overflow obligation. Omega's interpreter executes the same
verified module object and rejects out-of-range integer arguments before
execution.

The first Psi-owned checked-tree producer, `psi-checked-trees-to-terminal`,
lowers three exact closed-contract source forms: a Boolean literal or exact
named parameter from ordinary Boolean parameters; a recursively nested
expression over exact parameter/literal operands using builtin
wrapping/saturating add, subtract, or multiply from a nonempty sequence of
ordinary primitive-integer parameters; or an
integer-constant/unconditional-jump whose return is the matching literal or a
builtin parameter-plus-literal wrapping/saturating add, subtract, or multiply.
It emits the semantic module and proof bundle separately and fails closed on
all other shapes. Its canaries drop the frontend trees before terminal
verification and interpretation; ninth-parameter `bool` and `u8` machines
additionally cross the selected host incoming-stack ABI, while runtime wrapping
add combines its ninth stack argument with its first register argument and a
nested add-then-multiply expression reaches the same native lane. Parsing
through checked semantics and this first terminal producer are now Psi-owned;
general terminal vocabulary must extend the same direction. The same producer
independently revalidates checked content
conservation fingerprints, exact claim-preserving reshuffles, and direct
partition-composition substitutions before emitting their canonical terminal
v9-v12 evidence rows; the executable canary remains content-free. A
source-independent Omega abstract-operation consumer accepts only the verified
module and emits owned scalar-materialization, wrapping-add, saturating-add,
wrapping-subtract, saturating-subtract, wrapping-multiply,
saturating-multiply, jump-binding, and return requirements
with stable Psi provenance and no source
handles. The clean
target continuation resolves the current compile-known stream to a
provenance-retaining immediate return, emits AArch64 and x86-64 machine code,
and executes the emitted host entry in a linker harness with the same result as
terminal interpretation. The first runtime-parameter continuation now retains
declared parameter/result types, selects their AAPCS64, System V AMD64, or
Microsoft x64 register/incoming-stack locations through the established call
planner, and emits direct scalar parameter returns on both architectures. A
nine-`u8` canary forces a stack argument and agrees with interpretation at 77.
The next continuation lowers recursive parameter-fed wrapping/saturating
integer expressions and emits signed or unsigned 8/16/32/64-bit native
arithmetic on both architectures. A nested `u8` register/stack canary and a
signed `i64` two-bound canary agree with interpretation through real C ABI
calls. General register assignment remains separate implementation work; this
terminal slice uses bounded scratch registers and an aligned AArch64 argument
spill frame.
Semantic v5 and proof format v4 add recursive wrapping-subtract vocabulary;
semantic v6 and proof format v5 add recursive saturating-subtract vocabulary;
semantic v7 and proof format v6 add recursive wrapping-multiply vocabulary;
semantic v8 and proof format v7 add recursive saturating-multiply
vocabulary without changing fuel schedule v1. Parameter-fed canaries
round-trip, verify, cost two units, and agree with native execution: wrapping
`u8` computes 5-10 = 251, while signed `i64` saturating subtraction reaches
both bounds and wrapping `u8` multiplication computes 20*13 = 4.
The signed `i64` saturating-multiply canary reaches both bounds and covers the
`MIN * -1` edge plus an ordinary negative product.

The vocabulary has canonical semantic bytes and a domain-separated semantic
fingerprint; the source, wrapping, and saturating canaries round-trip after
discarding producer state. Proof format v1 retains its frozen original bytes,
minimal format v2 adds recursive wrapping-add terms, minimal format v3 adds
recursive saturating-add terms, minimal format v4 adds recursive wrapping-subtract
terms, minimal format v5 adds recursive saturating-subtract terms, and minimal
format v6 adds recursive wrapping-multiply terms, minimal format v7 adds
recursive saturating-multiply terms, minimal format v8 adds content-conservation
terms, and minimal format v9 adds sum-case structural paths. The proof section has its own
golden fingerprint, and a role-separated manifest binds semantic, proof,
installation, and debug sections without folding replaceable evidence into
program identity. The clean terminal lane owns a semantic-identity-bound object
artifact, emits the compatibility object container and standalone images for
the four supported architecture/format pairs, and validates exact
relocation-free text plus complete executable-region coverage. The macOS
canaries execute the emitted Mach-O image directly after producer state is
dropped. A canonical typed installation payload separately binds the terminal
identity, exact target facts, PE subsystem, profile decision, selected
provider-plan set, complete image digest, and compiler text-validation evidence;
its exact bytes enter the installation role of the artifact manifest. The
provider-free scalar canaries use an empty selected set. This metadata does not
replace the native executable admission/placement state machine. Canonical
typed debug-map v1 now binds the exact semantic identity to ordered source-file
metadata and bounded spans for stable terminal subjects, rejects unknown
subjects and wrong-module attachment, and remains replaceable presentation
evidence. Producer span population remains. General register assignment remains
on the legacy backend.

Semantic v1 integer, v2 Boolean, v3 wrapping-add, v4 saturating-add, v5
wrapping-subtract, v6 saturating-subtract, v7 wrapping-multiply, v8
saturating-multiply, v9 content, and v10 reshuffle modules retain their frozen
bytes and execution semantics; explicit migration produces a new current-v12
fingerprint. The v3 wrapping slice round-trips, verifies,
meters, lowers, emits,
and executes `u8` 200+100 as 44. The v4 saturating slice traverses the
same path and clamps that sum to 255. The checkpoint still has no branching.
`psi-terminal-fuel` defines schedule v1 as one unit per executed terminal
operation and one unit per taken terminal edge. The verified interpreter returns
exact schedule-keyed usage attributed to stable operation/edge identities; a
finite sponsor allowance fails atomically before an unpaid site. Explicit
in-memory execution state resumes at that exact site after checked allowance
replenishment without replaying prior work. The current acyclic single-path
vocabulary also has an exact entry-to-return certificate keyed by semantic
identity, entry, return edge, and fuel schedule; consumers recompute every field
without trusting the producer. The same checker now derives exact selected
block-to-edge segment certificates, including the endpoint charge, so adjacent
segments neither omit nor double-charge a jump. The current-vocabulary semantic
safe-point selector now returns the complete ordered partition at every
explicit jump/return edge; validation rejects omitted or reordered segments.
Build-time migration, branch/loop certificates, and native metering remain.
Attributed response reporting additionally waits on executable terminal
wait/foreign-edge variants carrying their response-contract status. The current
total operation plus unconditional jump/return vocabulary can close a bounded
report, but it cannot recompute `NoFiniteGuarantee(edge)` from semantics; a
producer-authored edge attribution would not satisfy the verification model.

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
is the one versioned portable execution representation consumed by Omega.
Omega begins with terminal Psi and owns provider installation, target
realization, optimization, ABI lowering, native emission, and execution.

```text
Omega files
    -> Psi parse / resolve / type / check / lower / canonicalize
    -> terminal Psi
    -> Omega interpret or realize for a target
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

Psi semantics and accounting are independently versioned:

```text
TerminalPsiIdentity {
    semantic_version;
    program_fingerprint;
}

FuelScheduleIdentity {
    schedule_version;
}
```

Changing the fuel schedule changes accounting, not program meaning. Cached
semantic results therefore key on Psi semantics and program identity; cost
records additionally key on the fuel schedule.

### The representation cut

No current representation is terminal Psi. `CheckedTrees`, `StateGraph`, and
`ControlFlowPlan` all retain `TypedTrees` expression tables and
`ExpressionHandle` as executable content. `StateGraph` and `ControlFlowPlan`
provide useful machine, state, transition, contract, borrow, and ownership
topology, but the latter is presently mostly a remap of the former rather than
expression lowering. `AbstractOperations` is already an Omega representation:
runtime storage regions, calling conventions, ABI aggregate classes, native
offsets, and related target-realization choices are its job.

Terminal Psi therefore replaces the hollow state-graph/control-flow pair with
one self-contained form at that altitude. It keeps their semantic skeleton but
replaces every source-tree reference with lowered values, operations,
predicates, typed places, blocks, and obligation-carrying edges. Short-circuit
evaluation, calls, guards, cleanup, suspension, and fallible operations may
create blocks not present in today's source-derived state segmentation; the
current arena topology is not the public schema.

The boundary is based on provenance, not on whether a number resembles an
offset. Author-declared device offsets, transfer widths, alignment demands,
and placement schema are semantic and remain in Psi. Target-selected native
field offsets, stack slots, register assignments, ABI classes, and concrete
storage regions belong to Omega.

### Semantic module

The fingerprinted semantic module contains:

- concrete machines, states, typed block parameters, values, calls,
  transitions, and terminals;
- a closed semantic operation vocabulary and statically visible variants for
  choices that change execution or generated obligations;
- structural places rooted in ordinary or provider-backed storage, including
  field, index-by-value, dereference, and range/subextent projections;
- contracts, author-declared premises, generated structural obligations,
  cleanup/transfer actions, conservation equations, work attribution, trust
  classes, and authorized admission sites;
- target-neutral provider requirements and scoped ordering operations; and
- stable identities shared by execution, propositions, proof evidence, fuel,
  diagnostics, and lowering provenance.

A choice that changes execution semantics or generated obligations must be
distinguishable without constant propagation. For example, trapping,
wrapping, and saturating integer addition are closed instruction variants, not
an ordinary runtime policy value. Several sound proof lemmas of differing
precision may describe one transition; lemma selection and later proof-library
improvements do not change operation or program identity.

The proposition vocabulary may differ from the executable vocabulary, but both
refer to the same canonical values, places, operations, and edges. Each
operation definition owns one normative execution transition and one generated
obligation schema. Its logical rules must be proven sound with respect to that
transition under those obligations. These per-operation proofs, plus the
global soundness obligations for control-flow composition, place algebra,
admission binding, canonical decoding, and ordering, form an enumerable Psi
language-verification backlog rather than an amorphous trusted compiler.

The normative Psi semantics is not whatever the current interpreter happens to
do. The interpreter and native lowering implement the versioned operation
semantics. Differential execution compares those two implementations; it does
not replace the per-operation soundness work or prove that both implementations
did not share a mistaken reading.

### Obligations, evidence, and admission

The verifier reconstructs the required obligation set from executable Psi and
its fingerprinted contracts. A proof bundle cannot omit an inconvenient
obligation, weaken a contract, or relabel a derivable fact as admitted.
Admission is legal only at sealed positions whose truth cannot be structurally
derived, such as a foreign boundary, provider fact, or checked assembly claim.
The verifier validates each admission's kind, provider/evidence identity, and
profile acceptance even though it cannot prove the admitted fact true.

Every accepted fact follows exactly one route:

```text
kernel-derived       a specified total judgment re-decided by the verifier
certificate-derived  explicit evidence checked by the proof kernel
admitted             an authorized unverifiable assertion accepted by policy
```

`requires` and published guarantees are program semantics and remain in the
module. Call sites must establish requirements. A bodyful guarantee must be
derived; only a bodyless or foreign guarantee at an authorized site may depend
on admission.

Primitive kernel judgments are minimized. Each is a normative, total,
specified decision procedure with its own soundness obligation. Other solvers
remain outside the trusted base by producing certificates checked by the small
kernel. A total, guaranteed certificate reconstruction may run locally in a
consumer; any search that may time out, return unknown, or otherwise fail must
ship its certificate when portable verification is required. An external
non-certifying answer is admitted evidence, not derived proof.

Terminal Psi plus its content-addressed semantic dependencies is sufficient to
state and check replacement evidence without source or the producing compiler.
That makes evidence replacement possible, not necessarily cheap: proprietary
or expensive proof search may still be required to find a new certificate.

### Artifact sections and identity

One installed execution selects one complete semantic version. Translating an
older module creates a new module and fingerprint; a verifier may not approve
one representation and execute another. A distribution container may carry
several separately fingerprinted variants, but selection occurs before
verification and execution.

The artifact separates four concerns:

```text
semantic module       executable Psi, contracts, obligations, admissions
proof bundle          replaceable derivations and carried certificates
installation record   selected providers, target facts, profile decisions
debug/source maps      presentation and diagnostics
```

The semantic fingerprint covers only the semantic module. The containing
artifact manifest hashes every attached section so evidence or installation
records cannot be silently replaced. Improving a proof changes the proof-bundle
and container identities, not the program's semantic identity. Supply-chain
attestation may separately state which producer created a module; it grants no
semantic authority to the verifier.

Canonical decoding rejects alternate encodings rather than silently
normalizing them. Numbering, ordering, algebraic normal forms, and serialization
are deterministic, so byte identity and semantic-module identity coincide for
one Psi version. Proof-system and evidence versions are separate from Psi
semantics. A bug in a Psi operation definition requires a semantic-version
response; a bug in a proof checker or trusted decision procedure revokes that
evidence version and may allow a replacement proof bundle over the unchanged
semantic module.

### Vocabulary construction

The proposition IR, proof kernel, and their extension/versioning discipline are
established before operations depend on them, then vocabulary grows through
vertical slices. For each operation class, specify together:

1. canonical encoding and typed operands/results;
2. execution transition;
3. generated obligations and authorized admissions;
4. proof rule plus its soundness obligation;
5. interpreter behavior and Omega lowering requirement; and
6. fuel identity under a separately versioned schedule.

Two operations require distinct static identities when their execution
semantics or generated obligations differ. Proof-lemma choice is not an
operation distinction. Proposition expressiveness is not limited to automatic
decidability: closed total fragments may discharge automatically, while richer
claims require explicit checkable evidence. Unsupported entailment refuses; it
never triggers unbounded proof search during verification.

The canonical operation vocabulary must retain scoped ordering events rather
than flattening them into opaque calls or one universal fence. CPU atomic
fences, same-context compiler/interruption fences, DMA publication, device
acquisition, MMIO completion, cache maintenance, and checked ISA barriers name
different participants and guarantees. A cross-device event retains its exact
range, mapping, observer/device instance, and ordering scope so the verifier can
check composition and target lowering can discharge the same requirement.

Erased proof evidence does not create runtime ordering. For example, a DMA
publication result may authorize a later doorbell in source, but the
publication operation itself must contribute the Psi event that forbids sinking
covered writes past publication or hoisting notification before it. On a
coherent target the verified realization may emit no instruction; on another
target it may expand into bounded cache maintenance, barriers, or an admitted
OS provider call. Those realizations retain distinct work and trust evidence
while implementing the same scoped semantic event.

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

Provider-local `FixedFuelProviderSummary` and `LogicalFuelResourceColumn` are
the current implementation precursor for hard roots. Each summary and
provision now names the `psi-core`-owned nonzero `FuelScheduleIdentity` directly;
composition rejects mixed schedules, and the external-root artifact publishes
the schedule version, provision, ceiling, and composed units. A summary's local
evidence now distinguishes a sealed recomputable terminal-Psi entry/segment
certificate from an admitted opaque-provider unit claim. Certificate-backed
units derive from the certificate, contribute no provider-validation receipt,
and retain exact terminal identity in the external-root artifact. The real
source canary composes its four-unit entry certificate through this path after
the generic installation ladder freezes and validates its code. The sealed
binding checks terminal semantic identity, architecture, exact
relocation-free artifact/frozen bytes, installed-code context, and selected
function offset. External-root installation rechecks the whole-entry
certificate against the exact root code and stub; a segment certificate alone
cannot authorize a whole hard-root entry. Opaque provider leaves remain
admitted summaries, and the Cathedral hard-root graph still needs migration.
This precursor does not grow into general symbolic complexity analysis.

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
original backing. A scalar
free-byte count does not prove placement in a fragmented general heap. Such
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

## Implementation sequence

1. Establish the proposition IR, small proof kernel, closed total judgments,
   evidence envelope, and authorized admission taxonomy.
2. Replace the current state-graph/control-flow remap with terminal Psi vertical
   slices, lowering expressions and predicates together and retaining
   structural places and edge obligations.
3. Re-root the reference interpreter on terminal Psi. During migration, keep
   the old interpreter only as comparison evidence; the established oracle
   claim is rebuilt rather than assumed to survive the change.
4. Re-root abstract-operation construction on terminal Psi and remove its
   dependency on `CheckedTrees` expression substitution. This moves
   monomorphization-shaped substitution above the boundary and unblocks generic
   backend work on explicit instantiated values.
5. Define deterministic serialization, semantic/module fingerprints, proof and
   installation section hashes, and version migration tests.
6. Define the separately versioned logical fuel schedule and interpreter meter.
7. Feed build-time evaluation usage, progress, warnings, and optional policy
   from that meter.
8. Migrate provider-local fixed-work summaries to Psi fuel and generalize them
   to selected safe-point segments.
9. Preserve `Bounded`, `Unknown`, and attributed no-finite-guarantee outcomes
   in artifacts and diagnostics.
10. Add trusted native block metering while preserving accounting provenance
   through optimization; canonical block topology itself need not survive.
   Defer a separate Psi-to-native PCC chain.
11. Add entry/current structural-place proposition terms, canonical
   `IntervalSet<CoordinateSpace>`, partial n-ary separation, canonical residual
   difference, and sealed introduction/custody-exit frontier rows. Migrate the
   current transitional single-interval content carrier before this vocabulary
   enters terminal Psi identity.
12. Add `CountedQuantity<Bytes>` with the package-level bump-allocation canary;
   retain exact tail placement and keep general fragmented allocators fallible
   unless they supply placement/reservation evidence.
