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

- deterministic metering inside bounded compiler and bootstrap services;
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

An evaluator or other bounded compiler service may supply a budget. Evaluated
code cannot inspect the remaining budget, branch on budget policy, or catch
exhaustion as a machine result. Exhaustion is an incomplete compiler-service
outcome, never Omega program semantics.

The documentation calls the greatest total charged along any one admitted
path the **maximum logical work**, measured in fuel units. Sequential work
adds, while mutually exclusive branches take their maximum:

```text
sequential A then B        work = A + B
exclusive A or B          work = max(A, B)
```

This is intentionally unlike worst-case simultaneous stack use, where two
sequential calls normally contribute their maximum because the first frame is
reclaimed before the second. Logical work is consumed and is not returned.
The maximum is neither native instruction count nor elapsed time.

The denomination serves deterministic build-time evaluation, bounded
bootstrap tools, static work reports, and optional WCET analysis. It is not a
native runtime mechanism. Native lowering inserts no fuel counter, hot charge,
cold dispatcher, transfer stub, resume stub, sponsor route, or hidden allowance
context. Once admitted native code starts, it executes according to its machine
semantics and ordinary boundary contracts.

The target-neutral admission floor, checked zero-argument/fixed-array
evaluator, ownership-taking const-generic pre-resolution evaluation, and
machine-backed concrete const-domain fact discharge are Psi services
(`psi-build-time-evaluation`). Omega orchestration may schedule those services,
but it does not own or reinterpret their language semantics. Target-specific
compilation consumes the resulting syntax/checked values and terminal Psi.

The public build-time service exposes ownership-taking pre-resolution and
pre-check conveyors for these target-neutral phases. Omega may interpose target
machine selection between them and performs calling-policy ABI, provider,
artifact, and native realization afterward; those target decisions are not
folded back into Psi language elaboration.

Optimization may reduce physical work without changing this portable logical
work measure. The measure constrains only services or analyses that explicitly
consume it; it never constrains a native activation implicitly.

A value-less `ReturnUnit` is still one taken normal-return edge. It has the
same edge charge as a scalar return and no invented value-producing operation.
Likewise, a finite retained exact-add, left-associated exact-subtract,
left-associated mixed exact-add/subtract, left-associated exact-multiply, mixed
exact-divide/remainder, exact-right-shift, or exact-left-shift chain charges
every ordered arithmetic operation and its ordinary value leaves.
Reconstructing each operation's proof adds no executable operation and therefore
no fuel charge.
The same accounting applies when a retained chain interleaves exact
add/subtract with exact multiply: every original arithmetic operation remains
charged, while verifier replay of cumulative affine coefficients and offsets
adds no executable work.
When that mixed affine chain feeds a partial exact cast, the cast is charged in
addition to every arithmetic operation; target-interval replay through the
cumulative coefficient and offset adds no executable work.
A finite chain of direct integer widens followed by an exact narrowing back to
the original carrier likewise charges every retained operation; the
verifier-derived self-proof adds no executable work.
Likewise, a finite retained exact-add/subtract literal-offset chain followed by
one exact fixed-native cast charges every arithmetic operation and the cast;
the verifier's shifted target-interval reconstruction adds no executable work.
Likewise, a finite retained exact-multiply literal chain followed by one partial
fixed-native exact cast charges every multiply operation and the cast; the
verifier's cumulative-product inverse-interval reconstruction adds no
executable work.
The signed-product widening uses identical accounting for its direct,
pre-cast, and post-cast homogeneous chains: every multiply and cast remains
charged separately, while checked sign/magnitude accumulation and reversed
negative-product interval replay add no executable work.
The signed-affine widening likewise charges every retained add, subtract,
multiply, and one-sided partial cast separately in its direct, pre-cast, and
post-cast placements. Checked sign/magnitude coefficient-and-offset
composition, reversed negative-coefficient preimages, and carrier intersection
are verifier work only and add no executable fuel.
The two-sided signed-affine sandwich likewise charges every retained source
add, subtract, and multiply, the one partial exact cast, and every retained
target add, subtract, and multiply separately. Checked sign/magnitude replay on
both sides, either reversed negative preimage, zero-coefficient decisions, and
the exact cast-carrier intersection are verifier-only work and add no
executable operation or fuel charge.
A same-root affine fork/join likewise charges every exact operation in the
complete left branch, every exact operation in the complete right branch, and
the outer exact add or subtract separately. Replaying both correlated affine
forms, checking their disjoint source-ordered definitions, and reconstructing
the combined root preimage are verifier-only work and add no executable
operation or fuel charge. Cancellation never removes a retained branch charge.
A distinct-root signature-bounded affine fork/join uses the same accounting:
every exact operation in both complete branches and the outer exact add or
subtract is retained and charged separately. Selecting tightest unary
signature bounds, intersecting both root intervals with their carrier, mapping
the two checked signed affine forms forward, and computing their Minkowski sum
or difference are verifier-only work and add no executable operation or fuel
charge. A containment or falsehood decision never removes a branch charge.
A distinct-root signed affine product join likewise charges every exact
operation in both complete branches and the outer exact multiply separately.
Selecting four unary signature endpoints, replaying both signed affine forms,
and computing the four-corner product hull are verifier-only work and add no
executable operation or fuel charge. A zero branch, containment decision, or
falsehood result never removes a retained branch charge.
A same-root signed affine quadratic product join likewise charges every exact
operation in both complete branches and the outer exact multiply separately.
Composing the correlated integer quadratic, selecting the two unary signature
endpoints, and evaluating endpoint and vertex-adjacent lattice candidates are
verifier-only work and add no executable operation or fuel charge. A constant
collapse fence, containment decision, or falsehood result never removes a
retained branch charge.
A same-root signed affine divide/remainder safety join likewise charges every
exact operation in both complete affine branches and the outer exact divide or
remainder separately. Selecting the two unary signature endpoints, solving the
divisor-zero and correlated signed `MIN / -1` lattice equations, and deciding
complete safety, complete unsafety, or partial safety are verifier-only work
and add no executable operation or fuel charge. A correlation decision never
removes a retained branch charge.
A finite partial exact-cast chain likewise charges every retained cast and its
ordinary value leaves. Ordered carrier-intersection replay is verifier work and
adds no executable operation or fuel charge; every cast obligation remains
independent.
When that finite cast chain follows an admitted affine, signed-product, shift,
or carrier-total divide/remainder prefix, every computed operation and every
cast is still charged separately. Replaying the complete carrier intersection
through the prefix's existing verifier-owned inverse algebra adds no executable
operation or fuel charge.
The converse finite-cast-chain rule has the same accounting: every cast and
every affine, signed-product, shift, or divide/remainder suffix operation is
retained and charged. Full carrier-intersection validation plus the selected
post-cast inverse replay are verifier work only and add no executable operation
or fuel charge.
Composing both sides does not change that accounting: a nonempty computed
prefix, every cast in an at-least-two partial-cast chain, and every operation in
the nonempty computed suffix are each retained and charged separately. Full
ordered carrier-intersection plus the selected target inverse and source
inverse/hull replay remain verifier-only work and add no executable fuel.
A computed-prefix/widen-chain/computed-suffix composition charges every source
exact operation, every retained `IntegerWiden`, and every target exact operation
separately. Validating ordered strict widening edges, intersecting the target
preimage with the source carrier, and replaying the selected source algebra are
verifier-only work and add no executable operation or fuel charge.
A heterogeneous computed-prefix/conversion-spine/computed-suffix composition
likewise charges every source exact operation, every retained widening, every
partial exact cast, and every target exact operation separately. Walking the
mixed conversion word, validating adjacent carriers, intersecting representable
intervals, and replaying existing source or target algebra remain verifier-only
work. No cast evidence or fuel charge is supplied by an earlier cast or widen.
Likewise, a finite retained exact-left-shift literal chain followed by one
partial fixed-native exact cast charges every shift operation and the cast;
the verifier's cumulative-count inverse-interval reconstruction adds no
executable work.
The same rule applies to a finite exact-right-shift literal chain followed by a
partial fixed-native exact cast: every shift and the cast are charged, while
the verifier's target-preimage reconstruction adds no executable work.
The same rule applies to a carrier-total finite exact-divide/remainder literal
chain followed by a partial fixed-native exact cast: every arithmetic operation
and the cast are charged, while verifier-owned interval-hull replay adds no
executable work.
The converse finite retained chain—one direct partial exact cast followed by a
nonempty left-associated same-target-carrier exact-add/subtract literal-offset
chain—likewise charges the cast and every arithmetic operation separately;
the cast proof and every independently reconstructed prefix proof add no fuel.
A direct partial exact cast followed by a finite nonempty same-target-carrier
exact-multiply literal chain follows the same rule: the cast and every multiply
operation are charged separately, while cumulative-product interval proofs add
no executable work.
A direct partial exact cast followed by the unified finite same-target-carrier
mixed affine chain likewise charges the cast and every retained add, subtract,
and multiply separately. Replaying checked cumulative coefficients and offsets
for each prefix adds no executable work.
A direct partial exact cast followed by a finite nonempty exact-left-shift
literal chain also charges the cast and every shift operation separately;
cumulative-count source-interval proofs add no executable work.
A direct partial exact cast followed by a finite nonempty exact-right-shift
literal chain likewise charges the cast and every shift operation separately;
each shift's independently reconstructed legal-count proof adds no executable
work.
A direct partial exact cast followed by a finite nonempty exact-divide/remainder
literal chain likewise charges the cast and every retained divide or remainder
separately; each operation's independently reconstructed safe-divisor proof
adds no executable work.
The unified runtime-divisor widening uses the same accounting for either a
direct-root chain or a chain rooted at one partial exact cast: every retained
divide, remainder, and cast is charged, while each independently reconstructed
runtime-divisor proposition adds no executable work.
A finite mixed exact-left/exact-right chain also charges every retained shift
and ordinary value leaf. Verifier replay of each left prefix's safe interval
through the prior canonical mixed-shift definitions adds no executable work.
When that mixed chain feeds a partial exact cast, the cast is charged in
addition to every retained shift; replay of the target/source interval through
the ordered mixed definitions adds no executable work.
The converse mixed-only family charges one direct partial exact cast and every
retained post-cast shift separately. Replaying each left prefix through the
ordered target-carrier definitions and intersecting with the source carrier is
verifier work and adds no executable fuel.
A retained finite exact-arithmetic prefix followed by a finite shift suffix
charges every add, subtract, multiply, and shift operation separately. Replaying
each left prefix through prior shifts and the checked affine form is verifier
work and adds no executable fuel.
The converse finite shift-prefix/exact-arithmetic-suffix family uses the same
accounting: every retained shift, add, subtract, and multiply is charged, while
affine preimage reconstruction and ordered shift replay add no executable fuel.
The finite affine/cast/affine sandwich charges every source arithmetic
operation, the partial cast, and every target arithmetic operation separately.
Two-stage affine preimage reconstruction and the source/target carrier
intersection are verifier work and add no executable fuel.
The finite shift/cast/shift sandwich follows the same rule: every source shift,
the partial cast, and every target shift is retained and charged separately.
Ordered target replay, carrier intersection, and ordered source replay are
verifier work and add no executable fuel.
The consolidated heterogeneous affine/shift cast sandwiches likewise charge
every retained source operation, the partial cast, and every retained target
operation separately. Checked affine composition, ordered shift replay, and
source/target carrier intersection are verifier work and add no executable
fuel.
The consolidated divide/remainder cross-cast family charges every retained
source operation, the partial cast, and every retained target operation.
Carrier-total quotient/remainder hull replay, target safe-interval replay, and
hull containment or disjointness checks are verifier work and add no executable
fuel.
The direct same-carrier divide/remainder-to-affine/shift compositions likewise
charge every retained operation and ordinary value leaf separately. Replaying
the carrier-total divide/remainder hull, affine safe interval, or ordered shift
preimage and comparing hull containment or disjointness are verifier work and
add no executable fuel.
A finite nonempty exact-divide/remainder chain crossing one partial exact cast
into another finite nonempty exact-divide/remainder chain charges every source
operation, the cast, and every target operation separately. Replaying the
carrier-total source hull for the cast and checking each independently safe
target divisor adds no executable work.

Build usage remains deterministic for the concrete invocation, target
description, evaluator/Psi semantics, and fuel schedule. It never depends on
host load or elapsed time. Long terminating builds remain legal; progress,
warnings, cache accounting, and optional root-selected ceilings consume the
meter without making the ceiling program semantics.

## Restricted maximum-logical-work checking

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
premises are ordinary call obligations and must hold whenever a consumer uses
the certificate.

External-root work composition retains the complete canonical provider summary
graph and every admitted opaque-provider receipt. Its compact FNV is a
non-authoritative report/cache coordinate only; certificate consumers compare
the retained demand and evidence rather than accepting fingerprint equality.

An installed-code certificate may bind a proved maximum-logical-work theorem to
the exact bytes it describes. That is PCC/report evidence only: it grants no
execution authority, reserves no runtime allowance, and changes no native
control flow. A certificate that arbitrary native bytes refine Terminal Psi is
a separate proof-carrying-code chain.

The live hard-root precursor composes recomputable entry/segment certificates
and admitted opaque-provider summaries under one `FuelScheduleIdentity`.
Installation rechecks whole-entry evidence against the exact terminal identity,
architecture, frozen code, entry stub, and function offset; a segment
certificate additionally exposes borrowed replay against its exact installed
code context, artifact, and selected entry while preserving its semantic
machine/block/edge coordinates. Its distinct type still cannot authorize a
whole root, and ordinary whole-entry graph composition rejects segment-local
evidence even when it appears beneath an opaque or entry summary. Psi also
seals the complete canonically ordered safe-point partition only after replaying
every row as one sequence; installation binds that non-clonable catalog to one
exact code, artifact, and entry occurrence. The catalog exposes only borrowed
segment rows and supplies no whole-entry, composition, bulk-charge,
native-meter, or publication authority. Cathedral migration and general
loop/build-time coverage remain in `TASKS.md`. This path does not grow into a
symbolic complexity language.

The first native WCSU precursor is deliberately narrower than installed-root
admission. For fully lowered Unit closures, Omega's emitter retains exact
code-positioned frame, argument/shadow, and link evidence. Object construction
validates those instructions, derives numeric local and caller-live peaks, and
composes the acyclic call-closure peak by maximum over sequential calls. The
branch-free scalar slice similarly retains exact ordered frame and
temporary-stack mutations. Its typed direct calls additionally retain exact
outbound and AArch64 link-register evidence; object construction decodes and
replays the instructions, derives caller-live bytes even with pending
temporaries, and composes the same acyclic closure. It rejects missing, forged,
untyped, cyclic, or unaccounted evidence. Conditional control-flow joins remain
outside this scalar slice. The one admitted conditional shape is not a join: a
top-level Boolean-parameter or bounded Boolean-expression condition leading to
two direct accountable integer return arms. Object construction distinguishes
the condition form, validates its exact branch target (`JZ` on x86-64, `CBZ` or
expression `B.EQ` on AArch64), and recognizes the exact x86 flag-preserving
`LEA` used to release an expression frame after comparison. It replays the
balanced expression prefix and each arm independently, then takes their
maximum peak. Typed scalar calls in the prefix or either arm reuse exact
outbound/link validation and closure composition. Direct branch-free division
and remainder in Boolean condition operands or either return arm reuse the
same replay and maximum. All AArch64 forms qualify; branch-free x86 uses the
same region replay, while signed Wrapping/Saturating x86 forms carry composite
outer-conditional and ordered inner-diamond evidence. Object construction
partitions those diamonds by prefix/arm and replays both paths independently.
The same division forms in typed call arguments in the condition or either arm
retain exact relocation and closure evidence through installation. Accountable
acyclic conditional-control trees use one depth-independent physical decision
list and DFS terminal bitmap. Object construction reconstructs every nested
region, replays its expression prefix and return/crash leaves, and partitions
the ordered x86 division-diamond ledger across those exact regions; AArch64
retains the branch-free evidence. One bounded genuine-reconvergence shape is
also retained: a finite nested Boolean tree whose immediate-value returning
leaves have the same complete affine cleanup frontier. Its true-before-false
source return edges are bound independently to the native jump leaves and final
fallthrough; all leaves materialize only their result, then reach one physical
preservation, cleanup, and return suffix. Object construction reconstructs the
leaf roster, validates every actual unconditional join and the final
fallthrough, replays the common cleanup suffix/actions and stack mutations once
per reachable path, and takes the maximum rather than summing mutually
exclusive leaves. Executable nominal cleanup calls continue to use the generic
cleanup-call relocation replay. Multiple semantic edge-attribution rows
deliberately name that same interval in DFS roster order.
Parameter, negated-parameter, and expression leaves retain the existing
per-control cleanup carrier rather than claiming this shared tail. Partial
duplicate rosters, mismatched edge/attribution rows, nonuniform cleanup
frontiers, crash leaves, and arbitrary CFG joins remain excluded. Source
edge-role classification remains upstream carried Terminal provenance and
artifact custody: object replay has no Terminal control-flow graph and does not
independently reject a fully coordinated source-identity retarget. The
conditional theorem also admits crash leaves. Evidence binds their exact set and
object construction validates every exact native `UD2`/`BRK` terminal before
installation; any returning arm stays directly accountable. Ordered x86
division diamonds from the condition or returning arm use the same independent
path replay.
The result intentionally excludes external entry adapter and architectural
arrival state. Root installation joins it into the selected context-indexed
`EntryStackRealization`: body WCSU contributes only to the `Body` epoch's
execution domain, while target-derived arrival and generated-or-admitted
adapter epochs retain their own domains and nesting allowances. Until that
separate join is implemented, this evidence alone is not an external-root
`StackPlan` or provider receipt.

## Response and physical time

Logical compute and response are separate:

```text
pure fixed path       work: Bounded(K)   response: finite under a timing model
block mutex.lock()    work: local bound  response: NoFiniteGuarantee(Edge(mutex.lock),
                                                    UnboundedWait)
suspend io.read()     work: local bound  response: NoFiniteGuarantee(Edge(io.read),
                                                    UnboundedWait)
```

A selected-point report has three honest outcomes:

- `Bounded(K, evidence)` when restricted maximum-logical-work checking closes;
- `Unknown(reason)` when the checker cannot prove a bound; and
- `NoFiniteGuarantee(subject, cause)` when a reachable wait, foreign edge, or
  cyclic component publishes no finite bound. The subject is an exact edge or a
  verifier-derived `CycleComponentId`; component causes distinguish `Unranked`,
  `UnboundedRank`, and the directed edge that prevents closure.

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

Program-local content supply has the same explicit shape. A content-bearing
domain authorizes one exact entry requirement; a fresh local lineage may appear
only at one of that requirement's statically enumerable installed parameter
positions. The requirement contract states exact finite capacity per occurrence
or constrains a selected const family. Ordinary invocation of the requirement
must supply an existing root and cannot reset the account.

The portable verifier reconstructs every introduction schema rather than
trusting a producer summary. Installation verification supplies the exact
finite occurrence set and derives the aggregate for one installed artifact
instance and lifecycle epoch. System admission composes those verified totals
across concurrently live components and replacement eras. A shared cap within
one assembly is one parent root divided among children; a machine-lifetime cap
must persist across epochs instead of being recreated as fresh local authority.
No separate provision declaration or data annotation participates.

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

Logical-work budgets belong to the compiler or bootstrap service that consumes
them. Spatial provisions may belong to an installation profile. Neither is
API/ABI identity unless an API deliberately promises a deadline or fixed
resource ceiling.

The proof-carrying-code scope in this brief is terminal Psi. Its verifier may
check memory safety, ownership and resource conservation, reach, termination,
and fixed-fuel certificates without trusting the producing compiler. Native
lowering/refinement certificates have a different subject and TCB and remain a
separate future lane.

The verifier/kernel split and its final trust placement are settled. A total
low-rung canonical semantic-ledger definition consumes terminal-Psi bytes,
validates the exact structure, directly denotes each operation, and emits the
complete ordered goals and validity-scoped premise introductions. Deployment
establishes that ledger either by executing the low definition or by checking a
derivation of the same result. At that endpoint the Rust verifier becomes an
optimized untrusted certificate producer and differential oracle; agreement
grants no authority and disagreement rejects. Until then its exact implementation
and version remain an explicit trusted dependency.

The low generator carries only local equations, authored contracts, primitive
denotations, and checked call substitution. Its closed declarative operation
schemas do not perform multi-operation interval reduction or algebraic
normalization. Those procedures must derive the canonical goal with a checked
certificate. Logical-justification order, path availability, state/place
versions, invalidation, all-predecessor merge evidence, and call-requirement
enumeration are part of the canonical ledger. Cyclic control requires explicit
invariant establishment and preservation rather than ordinary merge evidence.
The first ranked unsigned-countdown representation implements that distinction
for structural custody: its acyclic preheader establishes the header frontier,
and the exact covered backedge must reconstruct the identical live-claim,
owned-place, and partial-custody frontier after one complete cycle body. The
reference interpreter separately admits only that exact one-machine structural
Unit countdown through an opaque interpreter-specific verifier carrier. Its
proof walk removes the validated backedge from the deterministic schedule,
reconstructs the positive guard as the discrete unsigned subtraction premise,
and checks the decrement evidence before execution. Interpreter budget
accounting grants no ordinary verified carrier used by fixed-work checking,
Omega lowering, native installation, or provider dispatch. Whole-entry fixed
work for this exact slice uses another
opaque verifier carrier: it derives actual preheader, header, decrement, exit,
and return costs from the current schedule and combines them as `entry +
maximum_iterations * cycle + exit`. The certificate binds the canonical
terminal identity and fails closed when that exact all-input ceiling cannot fit
its scalar denomination. Omega's ranked-native path retains an opaque
verifier-issued owned semantic subject through machine code and independently
rejoins its public ranked graph, structural signature, type declarations,
provenance, and logical-fuel rows at object emission. This prevents coordinated
rewrites of otherwise self-consistent projected coordinates from replacing the
verified subject. Ordinary publication rejoins canonical installation plus
source-free native-artifact custody without changing the countdown body.
A distinct non-authorizing semantic-analysis carrier now seals the complete
safe-point partition for only that exact verified `u32` subject. It retains the
preheader edge, both header outcomes, decrement backedge, and return as five
block-local per-traversal certificates in canonical block/terminator order,
with current-schedule ceilings `1, 3, 3, 3, 1`. Independent whole-roster replay
binds terminal identity, schedule, machine, start block, edge, empty current
preconditions, and ceiling. Omission, duplication, reordering, semantic or
schedule drift, and a wider rank carrier reject. This catalog is neither a
whole-entry theorem nor execution, composition, or bulk-charge authority. A
distinct non-clonable installed correspondence now binds the complete roster
to one exact Terminal identity, ranked machine, emitted function offset,
frozen installed-code context, artifact, and selected entry. That carrier is
PCC/report evidence only and rejects occurrence, artifact, or entry
substitution; it grants no runtime meter, execution, root, or publication
authority. Its row and catalog types are not accepted by the acyclic segment or
whole-entry composition binders. Acyclic segment checking is not widened.

Every verifier, reduction-family, denotation-row, composition theorem, and
irreducible semantic dependency has an exact versioned node in a closed trust
graph whose leaves are registered roots. Portable denotation is relative to the
terminal-Psi abstract execution model; native ISA and hardware assumptions stay
in the separate native-refinement closure. A Psi-hosted generic kernel may
accelerate or cross-check proofs, but it emits no ledger and supplies no
reconstruction assurance.

## Implementation constraints

- Keep legacy interpreters or lowerers only as differential oracles while their
  consumer moves; they never define a second semantic path.
- Bind evidence to exact semantic and reconstructed-obligation identities. A
  certificate-provided proposition is never authoritative.
- Preserve accounting provenance through build-time evaluation and analysis, and
  retain `Bounded`, `Unknown`, and attributed no-finite-guarantee outcomes.
- Keep spatial authority concrete: content frontier rows and allocator canaries
  must retain exact placement, custody, and recomposition evidence.

`TASKS.md` owns the live implementation sequence and acceptance criteria.
