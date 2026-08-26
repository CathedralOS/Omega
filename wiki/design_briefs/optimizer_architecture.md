# Design Brief: Optimizer Architecture

Status: proposed architecture, 2026-08-25. The implementation queue is
[`TASKS_OPTIMIZER.md`](../../TASKS_OPTIMIZER.md). The earlier
[`verified_gated_ml_optimizer.md`](verified_gated_ml_optimizer.md) records the
research direction this design makes concrete.

## Purpose

Omega needs one optimization system with three cooperating layers:

1. a target-neutral optimizer over a private realization form derived from
   verified Terminal Psi;
2. target-aware lowering, instruction selection, register allocation, and
   machine optimization; and
3. a translation-validation boundary that permits heuristic, search, or learned
   decisions without trusting the decision maker.

The optimizer must exploit facts that ordinary native compilers usually have to
reconstruct or conservatively abandon: exact arithmetic policy, proved value
ranges, ownership and loan separation, effects and service reach, explicit
state-machine topology, cleanup frontiers, content conservation, termination,
and installed provider identity. Those facts are useful only if they remain
joined to the executable operation they justify.

This document owns the durable architecture. It does not track completion or
repeat milestone history.

## Existing boundaries

The repository currently has two backend lanes.

- The development compiler in `source/compiler/rust/omega/` still lowers
  `CheckedTrees -> StateGraph -> ControlFlowPlan -> AbstractOperations`. Its
  state-value planner performs useful expression substitution and constant
  folding, but it still consumes checked-tree expression handles.
- The target architecture lowers canonical Terminal Psi artifact sections
  through `omega-terminal-psi-to-abstract-operations`. That path first decodes
  and verifies the semantic module and proof bundle, then produces
  source-independent operations retaining Terminal Psi value, place, operation,
  edge, claim, and machine identities.
- The legacy assigned-target stage gives computed operands scratch registers by
  cycling through a fixed architecture-specific list. It does not compute live
  ranges, an interference graph, spills, splitting, or coalescing. The clean
  Terminal lane assigns only the deliberately bounded shapes it currently
  supports. Neither is a general register allocator.
- Terminal Psi is immutable, canonical, target-neutral semantic input. It is
  deliberately distinct from mutable optimization representations. Its
  identity, proof artifact, fuel schedule, installation decisions, and debug
  map are separate concerns.
- Omega, not Psi, owns optimization and target realization after the Terminal
  Psi boundary. The phrase **Psi optimizer** in this document means Omega's
  target-neutral optimizer over a verified Terminal-Psi-derived unit. It does
  not move optimization into the Psi frontend or mutate the canonical artifact.

Consequently, no new optimizer should be built over `StateGraph` or
`ControlFlowPlan`. They remain migration scaffolds. Reusable backend work such
as allocation may serve both lanes through adapters, but the durable high-level
optimizer begins only after canonical Terminal Psi verification.

## Semantic contract

### Observational equivalence

An accepted optimization must preserve every observable defined by the source
program and its admitted installation, not merely its ordinary return value:

- normal scalar, structural, and Unit results;
- crash versus normal return, exact crash cause, and reachable crash guards;
- boundary calls, service reach, effects, and their required order;
- placed, atomic, volatile, device, and externally mutable memory events;
- suspension points, ordering events, and the absence of invented safe points;
- affine and linear transfers, claim settlement, cleanup actions, and cleanup
  order;
- arithmetic policy, exact integer failure conditions, float format, signed
  zero, NaN behavior, and named fused versus unfused operations;
- provider and target-semantic dependencies that participate in realization;
- entry/result ABI promises and externally visible layout;
- required termination and progress properties; and
- constant-time or other resource properties when they are part of an admitted
  contract.

The optimizer may change internal state numbers, block layout, temporary
storage, instruction choice, register assignment, code size, or physical
runtime cost when no retained contract makes those choices observable.

### Logical fuel is not native cost

Terminal Psi logical fuel describes semantic work. An optimization may reduce
native instructions or cycles without reducing the charge observed by the
sponsor. Every optimized block therefore retains a source-charge map back to
the exact Terminal Psi operations and edges it realizes. A fixed-work
certificate remains a statement about the immutable Terminal Psi program. A
dynamically metered native image charges the corresponding semantic events even
when several of them collapse into one machine instruction.

### Floats remain exact to their named contract

There is no optimizer-wide, build-wide, or profile-wide fast-math switch.
Omega's float design explicitly forbids ambient relaxation. `a * b + c` cannot
become a fused operation, reassociation cannot change rounding, and a proof that
values are finite is a correctness fact rather than permission to change the
operation's contract.

If a future source operation deliberately permits several results—for example,
a named multiply-add operation with a bounded rounding disjunction—the
optimizer may choose any realization proved to satisfy that operation. That is
ordinary contract-directed lowering, not lossy optimization of a stricter
program.

### Proofs and borrow facts are capabilities, not observables to erase early

Checked proof and borrow records can authorize a transformation. They are not
silently weakened to target pointer aliases or comments. An optimization unit
retains:

- the exact fact or certificate identities it consumed;
- the values, places, versions, paths, and control region to which they apply;
- access polarity and loan restoration boundaries;
- effect, ordering, and cleanup barriers; and
- a source provenance map for every surviving or removed operation.

Proof-only runtime-erased data may disappear from physical layout after every
downstream consumer has recorded the fact it needs. The semantic artifact and
optimization report continue to retain its identity.

## Pipeline

```text
canonical Terminal Psi semantic + proof sections
    -> decode, reconstruct obligations, verify, admit
    -> VerifiedTerminalOptimizationInput
       { TerminalAbstractOperationPlan,
         immutable TerminalModule,
         proof bundle + fingerprint,
         reconstructed obligations,
         accepted facts,
         verifier-owned structural frontier snapshots }
    -> build PsiOptimizationUnit (private SSA/CFG + semantic side tables)
    -> target-neutral analyses and verified rewrites
    -> OptimizationRun
       { final PsiOptimizationUnit,
         exact named selections,
         retained candidate declarations,
         explicit work budget per named pass,
         ordered pass manifests (including zero-commit passes),
         aggregate decision log + TransformationLedger,
         replay/cache identity bundle }
    -> independently replay and validate every committed candidate
    -> ValidatedOptimizedAbstractPlan
       { clean TerminalAbstractOperationPlan (borrowed access only),
         retained OptimizationRun,
         optimized-plan projection validation receipt,
         validated pre-physical structured manifest + text projection }
    -> abstract operations and target-independent storage decisions
    -> lowering optimization
    -> ValidatedOptimizedTargetOperations
       { target operations (borrowed access only; virtual classes remain open),
         retained ValidatedOptimizedAbstractPlan,
         exact native target }
    -> instruction selection / target combines
       -> selected CFG liveness, live ranges, physical-view legality
          -> bounded transition-free physical-home assignment, or
          -> exact named fixed-view copies
             -> complete liveness/range/legality reanalysis
             -> bounded transition-free physical-home assignment
             (custody-only branch; stops before emission)
       -> current transitional bounded scratch assignment
    -> StagedOptimizedAssignedOperations
       { current assigned plan (borrowed access only),
         retained ValidatedOptimizedTargetOperations,
         root/function-provenance custody receipt,
         no allocator-validation or emission authority }
    -> replace/subsume staging with register allocation + frame assignment
    -> independent allocator validation
    -> validated assigned target operations
    -> machine scheduling / peepholes / block layout
    -> symbolic machine instructions
    -> encoding, relocation, image, installation
```

The immutable Terminal Psi identity remains the semantic root throughout. The
optimized plan has its own realization identity derived from:

- Terminal Psi identity;
- selected provider/target closure;
- exact selected-optimization set and normalized rule-set identity;
- accepted decision log;
- complete transformation-ledger identity; and
- backend/target model identities.

An optimized plan is never published as a replacement Terminal Psi artifact.
The Psi reference interpreter continues to execute the original verified
module. Native/optimized agreement is evidence about the realization, not a
new definition of program meaning.

The Rust reference orchestration entry lives in
`orchestration/omega-optimization-pipeline`; it accepts only a nonempty exact
selection set and an explicit per-pass work ceiling. It derives the canonical
named-pass schedule. The currently supported combined schedule is SCCP followed
by copy propagation. Each pass emits a distinct chained manifest, even when it
commits no rewrite. The aggregate decision log and transformation ledger cover
the entire initial-to-final execution, while the replay/cache bundle binds the
exact selections, flattened ordered rule identities, cost model, decision log,
and ledger. Reordering or omitting pass manifests therefore fails independent
projection rather than changing the meaning of a selection.

The custody bridge lives in `optimization/omega-lowering-optimizer`. It
deliberately does not trust the pass manager's final unit or reserialize that
unit as a detached lowering value. It rebuilds the initial verified unit,
replays the retained immutable candidate declarations through
`omega-optimization-validation`, checks exact
commit/ledger/pass-manifest/identity-bundle agreement, projects the final unit
onto the source plan's immutable semantic metadata, and asks the independent
validator to reconstruct that projection again. Its receipt binds custody of
the Terminal Psi, fuel schedule, initial and final unit revisions, exact
selection set, ledger, and composite identity bundle. It is not the later
target/native realization identity. A domain-separated projection identity
binds all of those fields plus the independent validator identity, so later
custody and manifest joins cannot name only the optimizer-authored bundle while
silently losing the translation validator.

That bridge now also retains a validated pre-physical manifest. The structured
record binds the Terminal-Psi root, fuel schedule, initial/final unit revisions,
exact selections, work ceiling and aggregate usage, complete ordered pass and
decision rows, transformation ledger, replay/cache bundle, projection receipt,
and source/final structural statistics under a domain-separated identity. Its
human text renderer projects those same rows, including consumed fact,
validator, provenance, and logical-fuel identities. The record explicitly says
physical data is unavailable before realization; it has no authority to invent
zero code size, zero spills, a zero frame, or allocator success. A later
physical/publication manifest must join this identity to independently
validated target, allocation, emission, and artifact records.

The pre-physical record has a versioned canonical artifact codec. It embeds the
complete transformation ledger rather than merely its digest, while the
established semantic manifest identity continues to bind the ledger by its
content identity so artifact framing does not churn optimization identity.
Nested codecs reconstruct and check decision rows, pass rows, work accounting,
the identity bundle, the ledger's revision chain, and its provenance/fuel map.
The outer decoder rejects identity tampering, truncation, unknown framing, and
trailing bytes. Decoding deliberately returns a plain record, not the validated
wrapper: cache or artifact bytes must re-enter the independent manifest
validator before downstream custody can accept them.

The first selected-instruction custody receipt binds that pre-physical manifest
identity alongside the optimizer bundle and abstract-projection receipt. Every
implemented liveness, live-range, allocation-legality, fixed-view-copy, and
register-home receipt propagates the same identity, while the parallel staged
assignment receipt joins it directly. Downstream physical work therefore
cannot retain the older roots while silently dropping the structured manifest
it is expected to extend.

The first register-allocation slice follows the same custody discipline. It
derives exact physical-view candidates from the selected CFG, target register
environment, reservations, architectural state, and fixed operand sites. Its
bounded home assigner is deterministic and may run only when every VReg has at
least one common legal view at every occupied point, no fixed-view transition
remains, and no spill is needed. It uses exact VReg interference and complete
storage/write footprints to prevent conflicting homes, while allowing
mutually-exclusive leaf values to reuse a view. Production and independent
replay both bind the legality, range, and register-environment identities.

The bounded chooser now executes actual linear-scan mechanics for this base
case. It orders canonical half-open VReg envelopes by start point and stable
VReg ID, expires active homes whose exclusive end is at or before the next
start, and tests candidates only against still-active exact CFG interference
and full physical footprints. The independent validator reconstructs interval
bounds and active expiration separately. After the fixed-view-copy transform,
the condition and forwarded value form one real interference pair and receive
distinct homes, while mutually exclusive leaf result intervals expire and
reuse the same return view on both ISAs. This remains a strict spill-free home
path, not general splitting or a complete linear-scan allocator. Pressure
decisions are represented by the separate bounded artifact below rather than
changing this path's failure semantics.

The allocator core also has a validated miniature two-register pressure model
with no fixed operands. Production and independent replay both assign two
overlapping flexible intervals to stable views 0 and 1, expire them before a
later interval reuses view 0, and reject three pairwise-overlapping intervals
at the same stable VReg because a spill would be required. The production
vertical now reaches the same flexible ranking mechanics from verified
Terminal operations: each leaf of an exact three-block conditional materializes
two u64 constants, performs one proof-bearing exact add through the target-owned
three-address `add_i64` row, and returns the result. Selection retains both the
obligation and its accepted verifier-fact identity. Liveness exposes exactly one
flexible interference pair per leaf; allocation assigns distinct homes and then
reuses the same pair across mutually exclusive leaves on x86-64 and AArch64.
The miniature three-way failure now feeds the first named pressure policy,
`SingleBlockFarthestEndThenHighestVregV1`. It walks the same canonical interval
order but stops at the first supported pressure point. The artifact records the
incoming VReg and common candidates, every active resident and provisional
home, and only those victim contenders whose hypothetical removal actually
recovers a legal incoming view. The incoming value is also a contender, with no
reclaimed view. Ranking chooses the farthest exclusive end and breaks exact
ties by the highest VReg ID. Equal-end three-way pressure therefore keeps the
two existing homes and selects the incoming VReg; a farther-ending resident is
selected when removing it exposes a legal view.

Production and a structurally separate replay implementation reconstruct this
decision under explicit five-axis work accounting. The policy is deliberately
limited to single-block, one-fragment values without edge connectors at the
pressure point; it fails closed rather than ranking flattened CFG layout. Its
domain-separated identity and canonical codec bind legality, ranges, register
environment, named policy, budget, usage, complete function roster, pressure
witnesses, contenders, and chosen victim. Decode returns a plain plan requiring
independent replay validation.

Despite its historical spill-choice name, this is recovery-victim evidence,
not permission to mutate the program. It chooses neither memory nor
rematerialization and has no spill/reload sites, stack slots, frame offsets,
callee-save costs, or emission authority. Actual materialization must first
join selected-value types, ownership/borrow and proof custody, cleanup/address
stability, target frame policy, and a named placement/cost rule. Stable x86-64
ordering currently chooses `rbx` as the second home in the production exact-add
fixture; that remains legality evidence only.

The next sibling artifact joins the chosen victim back to semantic selected-IR
custody rather than guessing from interval shape. The explicit
`SelectedVictimImmediateU64EligibilityV1` policy records whether the victim is
the not-yet-homed incoming value or an active resident (including its current
and hypothetically reclaimed views), then retains its scalar type, register
class, selected origin, definition site, and pressure location. For the current
positive case it requires a fixed unsigned non-address u64 uniquely defined by
one `MaterializeI64`, exact source-value and operation provenance, nonempty
logical-fuel anchors at the original operation, one local unconnected range,
no future fixed-view use, and at least one canonical future flexible use.

Every other value receives a precise `NoAdmittedRecovery` reason such as entry
parameter, unsupported scalar/range, future fixed use, proof-bearing or other
definition, or no future use. This is deliberately not “spill required”:
failure of one named rematerialization policy provides no positive storage
fact. Production and independent replay rescan selected definitions and range
occurrences, bind the selected/choice/range/legality/environment/unit/fuel roots
plus explicit work budget and usage, and compare the entire classification.
The versioned codec again decodes only to an unchecked plain plan.

This classification grants no authority to choose a strategy, move or
duplicate the original semantic instruction, add a native reconstruction,
change logical-fuel placement, allocate private storage, or emit code.

The first transformation consuming that evidence is the separately named
`SelectedIncomingU12ExactAddImmediateV1` policy. It is intentionally not a
generic literal sink or rematerializer. In the production pressure case the
right literal is already immediately before its only use, and moving it cannot
remove the two simultaneous register uses of a three-register `ExactAddI64`.
The admitted transform instead folds one incoming unsigned `0..=4095` literal
into an immediately following exact-add consumer. Both target owners publish a
two-operand `add_i64_immediate` constraint row: x86-64 may realize the exact,
flag-transparent form with LEA and AArch64 with ADD-immediate.

The transformed `ExactAddI64Immediate` retains the original exact-add
obligation and accepted proof-fact identity. Its provenance concatenates the
literal and add operations and logical-fuel settlements exactly once, while
retaining the add's source values, edges, and obligations. The literal
instruction and its VReg disappear; all later instruction and VReg identities
are deterministically redensified throughout the function. A domain-separated
recipe identity and strict codec bind every selected/range/legality/pressure/
classification/environment/availability/unit/fuel root, named policy, work
budget and usage, per-function action, and transformed selected-CFG identity.
Validation reconstructs the complete transformed CFG from retained roots; the
validated carrier alone can feed fresh liveness, ranges, legality, pressure,
and home analysis.

Each invocation applies at most one already selected action per function. It
does not iterate implicitly and is not installed in ordinary builds. In the
one-view production fixture, one explicit invocation folds literal `8`, full
reanalysis exposes the other leaf's literal as the next pressure victim, and a
second explicit invocation plus another full reanalysis reaches deterministic
homes on x86-64 and AArch64. This establishes an exact physical-form pressure
recovery without granting spill, frame, emission, or general rematerialization
authority.

The source-visible `SelectedIncomingU12ExactAddImmediate` family uses a
separate compiler-owned schedule,
`SelectedIncomingU12ExactAddImmediateToNoChangeV1`. It repeatedly invokes the
one-step primitive, with complete reanalysis after every changed selected CFG,
until one final independently validated attempt applies zero actions. Every
changed sweep must remove exactly one virtual register per action; the initial
virtual-register count is therefore a structural bound. The full build suite,
its exact selected-lowering projection, fixed constituent policies, upstream
work budget, aggregate usage, ordered changed steps, final roots, and terminal
no-change attempt remain together in the completion custody record. An enabled
family with no candidate is a verified successful no-op, not an error.

Optimization orchestration retains this as an append-only custody chain rather
than a raw series of core calls. `stage_first_optimized_literal_fold` owns the
source legality/availability chain plus the exact victim choice,
classification, validated fold, and fresh liveness/range/legality artifacts.
`stage_next_optimized_literal_fold` first replays that complete chain and then
appends exactly one more explicitly requested step. A no-action request rejects
instead of adding a meaningless ledger row. The final home carrier replays the
chain again, derives the ordered `LiteralFold` manifest entries internally,
and validates homes only from the last fresh analysis. Callers cannot supply or
reorder that ledger. Each public iteration receipt exposes the exact source
selected/range/legality roots, choice/classification/fold identities, named
policies and work usage, transformed selected identity, and fresh analysis
identities; adjacent receipt rows must join exactly.

That rejection remains the contract of the manually invoked one-step API. The
selected-lowering suite executor does not append the zero-action attempt as a
transformation; it retains it separately as completion evidence. It derives
the suite and budget from upstream optimized custody rather than accepting
detached caller claims. The executor is available to the clean orchestration
lane. Its completion receipt has a domain-separated identity carried through
strict homes, pre-allocation machine effects, and the home-aware post-
allocation machine sidecar. The post-allocation manifest records that
completion identity separately from its ordered literal-fold transformation
ledger. Consequently, an already-fixed-point run records an empty change
ledger and a nonempty suite-completion identity without pretending that a
rewrite occurred. The first function-relative realization manifest now joins
that completion to the independently replayed machine, encoding, and layout
roots. Native compiler publication remains closed until whole-function,
emission, and final publication custody extend that truthful intermediate
record.

Allocator search availability is now a separate compiler-internal validated
artifact. `AllEnvironmentAllocatableViewsV1` derives the complete flexible set
from the exact target-register environment: each retained view must be target-
declared allocatable and its complete storage/write footprint must avoid the
active reservation union. `ExplicitUnconstrainedViewAllowlistV1` accepts only a
canonical subset of that baseline. It may remove flexible candidates, worsen
code, create pressure, or make legality fail; it can never add a physical
capability, revive a reservation, or override architectural state.

This policy is not a reservation overlay. Reservations express mandatory
target/provider/runtime exclusions and participate in the target-environment
identity. Availability controls only allocator search. Fixed ABI and operand
constraints therefore bypass the flexible allowlist, while still undergoing
the exact class, reservation, and point-local architectural conflict checks.
An explicit one-view classification fixture retains `rdi` on x86-64 and `x0`
on AArch64. Both reach the first honest pressure witness at incoming VReg 2 and
classify literal `8` under
`SelectedVictimImmediateU64EligibilityV1`; x86-64's non-allowlisted fixed
`rax` return remains legal.

The availability plan has a domain-separated identity, strict codec, complete
class roster, named-policy provenance, and independent replay against physical,
constraint, reservation, and selected-key roots. Allocation legality binds its
identity directly, as do fixed-view copies, homes, spill choices, recovery
classification, orchestration receipts, and the post-allocation manifest.
Their identity domains and applicable codec versions advance rather than
silently accepting old cache bytes. Default orchestration always materializes
the all-environment policy; a
separate explicit staging entry accepts an already validated artifact for tests
and offline search. It is not currently exposed through `build.omg`, and it is
not part of source-level named optimization selections.

For future ML work, availability is a finite decision action rather than a
correctness fact: a model may select among validator-supplied candidate-policy
identities, but may not manufacture view masks, reservations, fixed-constraint
exceptions, or root optimization selections. Missing model output remains a
valid deterministic baseline.

The resulting register-home plan has its own versioned canonical artifact
codec. It carries those three roots plus the exact ordered machine, VReg,
register-class, and physical-view assignments, and recomputes a stored content
identity during decode. Malformed framing, invalid semantic machine IDs,
identity changes, truncation, and trailing bytes fail closed. As with the
pre-physical manifest codec, decode returns a plain record rather than a
validated carrier; the independent home validator must replay the retained
legality, range, environment, constraint, physical-model, and reservation
inputs before downstream custody accepts it.

Both strict home paths now construct and independently replay a structured
post-allocation manifest. It extends, rather than replaces, the validated
pre-physical manifest identity and binds the target, selected CFG, liveness,
range, allocation-legality, register-environment, and register-home identities.
Transformed routes additionally bind an ordered typed selected-transformation
ledger. Its current variants distinguish fixed-view-copy identities from
literal-fold identities; order, kind, and identity are all canonical inputs,
and exact duplicate identities reject rather than being silently collapsed. A
separate optional selected-lowering completion identity proves execution of a
named suite even when that ledger is empty; it is not itself a transformation.
The selected root is always the final transformed CFG. Exact function,
assignment, distinct-view,
interference, and remaining-transition counts are structured fields. Because
the strict home allocator rejects pressure and unresolved transitions, this
record may truthfully say no spill was required for the admitted plan. Frame
layout, machine emission, and publication remain explicitly unavailable, so
the record grants none of those authorities.

The post-allocation record also has a versioned canonical codec (v4 after the
selected-lowering completion join). It reconstructs
the typed target, ordered transformation roster, every upstream identity,
availability status, and statistic, then recomputes the stored manifest
identity. Unknown stage, target, transformation, spill, or availability tags,
identity tampering, truncation, and trailing bytes reject. Decode still returns
the plain record; only independent replay against the validated range,
legality, and home carriers can produce its validated wrapper.

The direct home path is still not the general allocator and grants no copy
insertion, splitting, spill, frame, emission, or publication authority. An
incompatible ABI-entry to fixed-return view remains an explicit transition
error there.

The separately named `LeafLocalBeforeFixedUseV1` transformation handles only
the admitted scalar-u64 entry-to-leaf-return case. It inserts an explicit
ISA-owned `CopyI64`, creates a fresh split-result VReg, rewrites only the exact
fixed return operand, preserves the return's provenance and logical fuel, and
gives the native copy source-value provenance with zero logical fuel. Its
explicit work budget is checked for the whole plan before any artifact is
published. Independent replay reconstructs the complete transformed selected
CFG and a domain-separated transformation identity.

Because the selected CFG changed, a sealed validated-analysis boundary reruns
liveness, ranges, interference, architectural state, and candidate legality
from scratch and requires zero remaining transitions. Only then may a separate
post-copy custody carrier invoke the unchanged strict home assigner. Both paths
stop before machine emission. General splitting around calls or pressure,
address-stable values, spills, and frames remain future named capabilities.

The carrier exposes the projected abstract plan only by borrow while retaining
the verified input and complete optimization run. A second optimized-only
carrier lowers it to target operations while retaining the first carrier and
exact target. This prevents an optimized consumer from obtaining either plan
by consuming and discarding its evidence. The ordinary bare-plan lowering API
remains for the empty-selection compatibility lane.

A third opaque `StagedOptimizedAssignedOperations` carrier retains the complete
optimized-target carrier beside the output of the current bounded
scratch-cycling assignment stage. It also retains a clean-lane target-register
environment containing the independently validated physical model and
target-semantic constraint catalog. Its independently reconstructed custody
receipt checks Terminal-Psi identity, projection identity, native target,
entry, exact ordered function roster, attachments, and operation provenance;
the environment target must match that same target. It intentionally exposes
plans and model inputs only by borrow. `Staged` is a trust boundary, not a
synonym for validated allocation: the receipt says nothing about liveness,
interference, register-unit conflicts, fixed operands, spills, or frame slots,
and grants no machine-emission or publication authority.

Clean compiler staging branches before constructing optimizer state. Empty
selection takes the prior compatibility route unchanged. Nonempty selection
must enter `omega-optimization-pipeline`, and unsupported named families or any
validation failure reject without fallback. The typed terminal-component route
continues through optimized target lowering, instruction selection, liveness,
ranges, allocation legality, exact phase dispatch, strict spill-free homes,
and independently replayed post-allocation machine facts. Psi-only suites take
the direct home route; mixed or lower-only suites run the selected-lowering
projection derived from retained full-suite custody. Both deliberately stop
before frame/exit validation, machine emission, object/image construction, or
component publication. A source shape outside the currently admitted selected
CFG fails at that named boundary. The legacy compiler's nonempty-selection
firewall remains closed so selected builds can never fall through to its old
backend.

The verified optimizer input is required, not an optional evidence attachment
to a bare plan. Compatibility lowering has a separate bare-plan entry; the
verified carrier exposes no consuming operation that detaches its plan from the
context, and no optimizer constructor accepts the reduced value. Keeping the
complete verified Terminal module beside the lowering seed ensures that later
unit builders can derive call obligations, structural place paths, edge
cleanup, and borrow/claim frontiers even where the current abstract plan does
not yet project them.

Structural frontier snapshots come from Psi's verifier walk itself. They retain
block entry, operation entry/exit, and edge entry/exit states with exact claim
paths, owned-place multiplicities, and partially moved paths. Omega validates
their coverage, projects them into a canonical immutable optimization-unit fact
catalog, and joins them to transformation provenance; it does not reimplement
the borrow checker as a target alias analysis. Every fact has a content-derived
identity over its Terminal-Psi identity, machine, exact source site, and complete
snapshot. The independent validator reconstructs that catalog from the retained
verifier context for both initial and transformed revisions. Return and crash
edges currently contribute edge-entry facts only because Psi's verifier does
not yet publish their post-terminal exit state; control-successor edges publish
both entry and exit.

## Optimization unit

`PsiOptimizationUnit` is a private, reconstructible compiler representation,
not another portable language boundary. It should evolve from the clean
`TerminalAbstractOperationPlan` seed instead of introducing a parallel
source-shaped IR.

Each function contains:

- canonical Terminal Psi machine identity and signature;
- explicit basic blocks, predecessors, successors, and edge identities;
- typed SSA scalar values and block parameters;
- structural places and path-sensitive projections;
- an explicit memory/effect chain for observable or potentially aliasing work;
- calls with exact effect, crash, progress, and provider summaries;
- ownership/claim/cleanup events in semantic order;
- proof and range facts indexed by value/place version and region;
- logical-fuel charge sites;
- source operation/edge provenance, including many-to-one and one-to-many
  mappings; and
- optional target-independent cost features that do not affect validity.

The reconstructible unit owns the exact verified module declarations needed
after the full Terminal module is discarded: structural types, boundary
machines, and the complete checked provider-candidate catalog. A function's
immutable signature is likewise exact, including nominal attachment, ordered
scalar and structural parameters, Unit/scalar/structural result shape, full
ordered entry-claim declarations, and the normalized published service
ceiling. These are not optimizer summaries. Unit content identity encodes every
field, transformed-revision validation compares them with the immutable
verified input, and abstract-plan projection reads them from the unit before an
independent round-trip check. Passes may inspect this custody but cannot rewrite
it.

The unit also owns the exact verifier-projected ownership frontier catalog.
This is immutable source authority, not a mutable reconstruction of the current
CFG: rewrites may remove a source operation or edge but must retain its fact
row. Catalog and nested snapshot ordering are canonical, the complete catalog
is bound by unit content identity, and validation compares it to an independent
projection from `VerifiedTerminalOptimizationInput`. A later analysis may use a
row only at its exact source site unless it proves a new current-region
relationship.

The unit must not contain syntax nodes, `ExpressionHandle`, authored names as
identity, native byte offsets, physical registers, or target instruction
encodings.

Block parameters retain Terminal-Psi declaration order as explicit input data;
they are never reconstructed by sorting parameter identities or by observing
only the incoming edges that happen to survive. Cached definition/use rows are
convenience indices, not authority: the independent representation validator
re-derives them from operation semantics. It also rechecks the complete current
Terminal-Psi CFG contract—parameter-free entry, closed edges, total
reachability, and acyclicity—before any rule may inspect the unit. When Terminal
Psi later admits wider cyclic control flow, that expansion must arrive as an
explicit vocabulary/validator change rather than an optimizer-only exception.

The unit revision is a versioned content identity, not a rewrite-path token.
Its canonical encoding excludes only the identity field and transformation
history while binding the complete retained function metadata, CFG, operation
payloads, facts, effects, ownership, provenance, and logical-fuel state.
Construction and accepted-fact attachment recompute it. Independent rewrite
validation applies a declared patch, recomputes the accepted output identity,
and only then returns the new revision; a rule does not manufacture that
identity by hashing its own candidate. Equal accepted content reached through
different legal histories therefore shares one unit identity, while the
ordered transformation ledger continues to distinguish those histories.

SSA applies naturally to scalar values. Memory, ownership, and cleanup are not
forced into a scalar fiction. They use explicit versioned tokens/frontiers so a
pass can prove that a rewrite preserves all relevant state. Address-stable
places remain address-stable even if scalar values around them are promoted.

## Analysis system

Analyses are deterministic functions of a content-addressed unit revision and
declared context. The analysis manager checks that address before cached, cold,
or revision-commit work, then invalidates only what a committed rewrite
declares invalid.

The baseline set is:

- CFG validation, dominators, post-dominators, loops, and strongly connected
  components;
- use/definition and reachability indices;
- scalar constant propagation and executable-edge discovery;
- proof-backed ranges, congruences, nonzero facts, and case domains;
- effect, service-reach, boundary, crash, and suspension summaries;
- call graph, recursion, termination, and specialization closure;
- place/path alias classes derived from ownership, loans, indices, and layout-
  independent projections;
- memory versions, available loads, and store liveness;
- ownership, claim, cleanup, and conservation frontiers;
- escape, address-stability, and capture analysis;
- value and place liveness;
- target legalization and instruction-cost facts after target selection; and
- physical register units, clobbers, pressure, and live intervals at allocation.

Analyses never broaden a fact beyond its certificate's region or version. An
unknown result is conservative, not an invitation for a rule to guess.

Effect analysis has two deliberately separate views. Node rows classify the
local operation. Function rows form a deterministic call-graph fixed point over
reachable blocks, carrying transitive service/boundary identity sets and
observable, structural-state, crash, and suspension knowledge. A callee absent
from the closed optimization unit produces `May`; recursion converges by
monotone union and never invents a service or a proof of absence.

The Rust bring-up keeps these products together in `omega-psi-optimizer`.
Analysis caches are compilation-local ordered maps keyed by the exact unit
revision; there is no process-global cache. Dependency resolution follows the
closed `AnalysisKind` order. Committing a revision expands declared
invalidation through dependent analyses, and the pass-validation configuration
cold-recomputes supposedly retained rows before changing the manager revision.
A mismatch is an undeclared-invalidation failure and leaves both cache and
revision untouched. Independent cold analyses may run concurrently, but their
published bundle is sorted back into canonical analysis order.

`OwnershipFrontiers` is the first verifier-derived semantic analysis product.
It exposes each immutable catalog row under its exact machine/source-site key
and binds the view to the current optimization-unit revision. Because that
revision is part of the analysis region, the manager always invalidates and
rebinds the product across a commit, even when no rule declares a mutation of
the underlying immutable catalog. It is not yet a current-CFG ownership solver,
and no rewrite may broaden a source-site snapshot into a function-wide fact.
The decision-manifest vocabulary has a separate typed ownership-frontier fact
reference, allowing a future rule to retain the exact consumed capability and
its domain-separated identity. This representational support does not itself
make any ownership rewrite applicable.

Literal-derived constants and ranges name the exact supporting Psi operation
and are valid only for their `(unit revision, machine, value)` region.
Executable-edge facts retain that support on both the selected and rejected
conditional edge; absence of such support remains `Unknown`. This same shape
is the baseline for later verifier-derived range and ownership facts: a useful
answer without an exact support and validity region is not representable.

Effect summaries use explicit `No`/`May`/`Yes` knowledge. Missing callee,
boundary, crash, or suspension detail is `May`, never an inferred pure result.
Scalar liveness is a fixed-point CFG analysis even while the admitted Terminal
Psi slice remains acyclic, so widening the source vocabulary does not require
replacing the optimizer's analysis model.

## Rule and pass model

Squalr's scan-rule architecture supplies a useful small pattern: typed rules
with stable IDs mutate a plan, a registry owns built-ins, a dispatcher applies
them, and an independent scalar scan can validate a specialized result. Omega
should adopt the separation of **rule**, **registry**, **plan**, and
**dispatcher**, but strengthen it for a compiler:

- registries are ordered and deterministic, never iteration-ordered hash maps;
- registries are explicit values passed to a pipeline, never unsafe mutable
  global singletons;
- input is immutable and a rule proposes a patch rather than partially mutating
  shared state;
- each rule declares stage, version, required analyses, invalidations, safety
  class, and compatible semantic vocabulary;
- a proposed patch contains its region boundary, substitutions, provenance,
  and validation witness;
- the pass manager validates before commit and leaves the unit unchanged on
  rejection;
- repeated groups have an explicit convergence metric and iteration budget;
  rule order alone cannot produce an unbounded rewrite loop; and
- registry coverage tests prove every enabled rule phase is actually dispatched
  (registration alone is not execution); and
- diagnostics and a deterministic decision log explain every applied, skipped,
  and rejected candidate when requested.

Conceptually:

```text
OptimizationRule {
    identity() -> RuleIdentity
    requirements() -> AnalysisSet
    propose(context, immutable_unit) -> [RewriteCandidate]
}

RewriteCandidate {
    decision_point
    affected_region
    patch
    provenance_map
    witness
    predicted_cost_delta
}

PassManager:
    analyze -> enumerate -> choose -> validate -> commit -> invalidate
```

Rules are grouped into named, versioned pipelines for deterministic scheduling.
The build selects exact named optimizations, not a pipeline intensity. The pass
manager derives a canonical schedule for precisely that set; analyses and
correctness-required normalization may run as prerequisites, but an unselected
transformation may not be smuggled in as a prerequisite. A convenience helper
may spell several `enable` calls, but its evaluated result and manifest are the
expanded selections, never an opaque suite level. Source code and models cannot
register arbitrary executable compiler extensions.

The ordered registry preserves the pass manager's explicit schedule exactly;
it does not sort cryptographic rule or pass identities. The full ordered list
is itself identity-bearing, duplicates reject, and each opted-in compilation
owns an immutable registry value. Reversing two otherwise identical rules is a
different schedule and therefore a different rule-set identity. Built-in rule
contributions separately carry source-declared contiguous schedule ordinals;
their private assembler sorts by those ordinals before constructing the
ordered registry. Thus contribution arrival order cannot perturb the declared
schedule, while no opaque identity sort silently invents policy. A direct
second sweep of the currently supported SCCP then copy-propagation schedule
must produce an empty delta; composing that delta preserves the first sweep's
ledger exactly.

The first Rust candidate vocabulary is intentionally closed rather than an
opaque callback or byte payload. An exact-integer-evaluation candidate records
its input revision, rule contract, bounded region, analysis contract,
substitutions, provenance/fuel mapping, typed operand-fact identities, cost
estimate, and typed replacement. A literal operand-fact identity binds the
input revision, machine, value, scalar type, exact definition site, constant
payload, and source operation; a raw source operation ID is not sufficient
rewrite evidence. Its candidate identity covers that canonical declaration;
the candidate does not own an output-revision identity. The independent
validator—not the rule—reconstructs each fact identity and the arithmetic,
produces the new unit, recomputes its canonical content identity, and attaches
its own validator identity. A
proof-certified scalar candidate additionally has a distinct witness shape
that names the admitted operation-obligation fact; a goal-free candidate cannot
carry one. That fact identity binds the Terminal-Psi identity, proof-bundle
fingerprint, exact machine/operation/obligation owner, and canonical proposition
bytes. The verified builder derives the sorted fact index from the immutable
verifier carrier, binds it into the initial unit identity, and the independent
validator reconstructs that projection before a public session can run. The
manifest v3 row therefore records the operand facts and the exact admitted fact
that authorized removal of a proof-bearing operation. This establishes the
pattern future patch variants must follow before they become executable.

For a propagated block parameter, the fact identity additionally binds a
canonical snapshot of the entire machine's coupled SCCP result: every block's
reachability, every exact `EdgeId` verdict (including infeasible competitors),
and every scalar definition's lattice state. The validation crate owns a second
fixed-point implementation and reconstructs this snapshot without depending on
the optimizer crate. A digest supplied by the optimizer is therefore only a
claim; it becomes rewrite evidence only when the validator independently
derives the identical snapshot and fact identity.

The initial pass-manager skeleton has a public entry only from
`VerifiedPsiOptimizationUnit`; a bare reconstructible seed cannot start a run.
It retains the complete verified input context, charges every bounded work
axis, restarts canonical rule dispatch after each accepted patch, requires a
strictly decreasing transformation-specific measure, and commits analysis
invalidation only after the independent validator constructs an accepted
output. Exhaustion or rejection returns no optimized session for publication.
This remains a pre-publication vertical slice. Build-level selections may enter
the clean verified optimizer lane when every named family has a complete
schedule, but no selected build can publish output until physical realization
retains the same custody and passes the publication gate.

The next closed candidate vocabulary covers redundant block parameters without
folding it into CFG cleanup. Its witness lists every exact incoming edge and
the typed value bound at that parameter position; two conditional successors
from one source block remain two rows. The independent validator reconstructs
that complete set, requires one common dominating replacement, removes the
parameter and matching binding entries, substitutes uses, and then rechecks the
whole unit.

That structural rewrite also passes a separately reconstructed closed-region
observation gate. The rule does not supply an observation digest. The validator
derives the exact changed-block set, independently normalizes only the typed
`parameter -> replacement` scalar-use slots and the one proved incoming
binding position, and observes the normalized input against the constructed
output. The canonical question includes full operations even for pure scalar
nodes, block parameters, definitions/uses, CFG successor rows and boundary
edges, typed live-ins/outs, effect links, conservative crash/suspension state,
boundary/service/control/normal/crash events, ownership and cleanup events,
proof provenance, logical fuel, and explicit presence or absence of each
retained verifier-frontier identity. Thus an accidental exact-to-wrapping
operation change cannot hide behind equal definitions and uses. Everything
outside the independently derived block set must remain byte-for-byte equal as
unit content. Pre/post unit revisions differ by design; the normalized
semantics must not.

This is deliberately the exact contract for structural scalar substitution
and one block-parameter binding-slot erasure, not a claim of general regional
equivalence. Real memory traces, explicit suspension edges, arbitrary node-set
regions, and path-derived current ownership facts remain future vocabulary.
The pass has its own block-parameter-count convergence measure and its own
explicit `CopyPropagation` selection; it is not a hidden prerequisite of SCCP
or an optimization-level bundle.

Baseline choice lives in `omega-optimization-policy`, outside rule and
validator crates. The pass manager first obtains independently constructed
outputs, projects only their candidate identities and non-authoritative cost
deltas to policy, and rejects any returned identity absent from that admitted
set. The model-free policy selects the lowest improving cost with candidate
identity as the final tie break. Its canonical ordered decision-log codec
recomputes every decision identity during replay and rejects tamper or trailing
data.

## Validation and trust

Optimization remains an untrusted producer. Acceptance has layers.

### Representation validators

Every committed pass must leave its output structurally valid: defined and
dominating uses, typed block arguments, closed CFG, valid place paths, complete
provenance, balanced ownership/cleanup frontiers, legal effect chains, and
consistent fuel mappings. Test and diagnostic builds run the full validator
after every pass. Production may use a staged validator when equivalently
strong checks are fused into construction.

### Local rewrite validators

The first rules use small, rule-independent validators over bounded regions.
The validator compares:

- live-in assumptions and proof premises;
- all normal live-outs;
- crash and suspension exits;
- memory/effect trace;
- ownership and cleanup frontier;
- boundary and provider events; and
- logical-fuel attribution.

Simple canonical rewrites may be correct by a checked constructor plus these
invariants. Algebraic rewrites attach a derivation that the proof kernel can
check where the existing proposition vocabulary is sufficient.

For the first multi-block structural rewrite, the validator owns both the
normalization and the observation question. Candidate-authored affected blocks,
incoming edges, provenance rows, and fuel rows are checked against independent
enumeration before the region is constructed; omitting a block or node cannot
shrink what is observed. The accepted validator identity changes when this
contract changes even if the proposing rule's semantics do not.

### Translation validation

The durable gate validates the optimized realization against its exact
Terminal Psi source. It reconstructs the equivalence obligations; the
optimizer may not choose what needs proving. Proof search, SMT, e-graphs,
superoptimizers, and learned systems may produce candidate certificates but do
not enter the trusted kernel.

Whole-program equivalence is built compositionally from accepted region and
function summaries. Until a transformation class has such a gate, it may be
used only in differential experiments and cannot publish an optimized
executable.

### Differential testing

The Psi interpreter is the primary execution oracle for generated and curated
inputs. Differential equality includes result bytes, stdout/stderr, boundary
traces, crash behavior, and other exposed observations. Differential testing
finds implementation defects but never substitutes for the admission gate.

## Target-neutral pass families

The initial exact pipeline should grow in this order:

1. CFG cleanup: unreachable blocks, jump threading, branch folding, empty-block
   elimination, and unreachable private-machine removal.
2. Sparse conditional constant propagation using the exact integer/Boolean
   operation semantics.
3. copy propagation, constant propagation, local common-subexpression
   elimination, global value numbering, and dead scalar work elimination.
4. proof-backed elimination of redundant range, nonzero, case, and bounds
   checks while preserving their diagnostic/provenance record.
5. effect-aware dead-call and dead-store elimination for calls proven pure and
   non-crashing in the exact retained context.
6. inlining, tail-call formation, specialization, and devirtualization after
   provider and whole-program closure.
7. state-machine simplification: state fusion, transition threading, dispatch
   bypass, unreachable-state pruning, and direct lowering of acyclic regions.
8. ownership-aware scalar replacement, aggregate promotion, move/copy elision,
   return-place forwarding, loop-carried storage coalescing, and cleanup-tail
   sharing.
9. alias/proof-enabled load forwarding, dead-store elimination, loop-invariant
   code motion, and vectorization.

Rules must be operation-semantic, not spelling-semantic. `Exact`, `Wrapping`,
`Saturating`, and `Trapping` operations have different identities and rewrite
laws. Atomic, placed, volatile, boundary, and may-suspend operations are
barriers according to their explicit contracts rather than a blanket list of
names.

That identity remains explicit through target and assigned-target operations.
Two policies may select the same final ISA opcode only at machine realization.
Likewise, constant folding or instruction selection cannot erase a
proof-bearing operation's obligation merely because its operands are currently
known. A proof-certified candidate must consume the exact admitted obligation
fact owned by that operation; the validator rejects a missing, foreign, or
goal-free proof witness, and the manifest retains the fact reference after the
operation becomes a constant. The transformation ledger remains responsible
for the corresponding semantic settlement record as that vocabulary expands.

## Lowering optimization

Lowering optimization begins after provider and target selection and before
physical register assignment. It owns choices that depend on layout, ABI, ISA,
or a target cost model while preserving the optimized Psi ledger:

- legalization and legal-width decomposition;
- immediate and addressing-mode folding;
- strength reduction valid for the exact arithmetic policy;
- compare/branch and select formation;
- target-specific instruction combining;
- call-sequence and tail-call lowering;
- aggregate copy strategy and proven overlap handling;
- vector width and scalar remainder planning;
- target-aware block placement estimates; and
- rematerialization and spill-cost annotations for allocation.

Instruction selection should produce virtual registers in explicit register
classes plus machine-state uses/defs. It must not preassign arbitrary scratch
registers in ways that hide interference from the allocator.

The first production slice makes that boundary concrete without claiming a
general selector. `omega-terminal-selected-instructions` is the data-only
representation owner, while
`omega-terminal-target-operations-to-selected-instructions` produces and
independently validates three exact three-block runtime conditional forms. The
first has leaves that materialize unsigned 64-bit constants and return. The
second carries a shared unsigned 64-bit entry parameter across both branch
edges and returns it directly, exposing genuine virtual interference and
different entry/return fixed sites without inventing a move. The third gives
each leaf two unsigned 64-bit constants and a verifier-admitted exact addition.
Its selected semantic kind retains the exact obligation and accepted-fact
identity while its physical constraint remains the target-owned, flag-neutral
three-address `add_i64` row. Each virtual register retains
its exact Psi value and definition site; each instruction retains its catalog
constraint, explicit and implicit state footprint, and semantic provenance;
branch-edge fuel remains attached to the corresponding selected successor so
only the taken edge is charged. ISA-owned orchestration injects the exact
constraint keys and ABI live-in views instead of asking a target-neutral stage
to infer them from names or coincident numeric variants. The opaque staged
carrier also owns the final optimized unit, independent abstract projection,
target plan, and validated register environment. This is allocator input only:
it grants no physical-home, emission, or publication authority and fails closed
for every other source shape. A nested liveness carrier may consume it, but
cannot weaken or detach that custody.

## Register allocation

Register allocation is the transformation from target operations with virtual
registers to assigned target operations. Its target model includes:

- allocatable, reserved, argument, result, callee-saved, and caller-saved
  registers;
- overlapping register units and width/subregister constraints;
- integer, float/vector, predicate/flags, address, and special register classes;
- instruction constraints, tied operands, early clobbers, and fixed registers;
- call and inline-assembly clobbers;
- stack alignment, red zones where applicable, outgoing arguments, and frame
  limits; and
- registers reserved by dispatch, metering, platform, or installed providers.

The first implemented substrate is deliberately data-only.
`omega-register-model` owns register units, named views/classes, read/write
footprints, preservation conventions, and reservation overlays, then validates
their closure without depending on either ISA. A separate closed Register
Constraint Catalog binds stable family/variant keys to explicit operand use/def
roles, classes, optional fixed views, canonical ties, early clobbers, implicit
uses/defs/clobbers, and an exact required-key inventory. Generic validation
checks catalog structure against a validated physical model; each ISA then
compares every row with its own canonical target semantics so a same-class
register substitution cannot pass. Clean Terminal ISA crates construct those
values, and opt-in orchestration passes both validated artifacts through an
opaque target-register environment rather than discovering a target through
global state. `omega-regalloc` retains its register-model compatibility facade
and now also owns the first production liveness analysis over the opaque
validated selected carrier; allocation logic will consume these facts without
becoming their representation owner. The baseline
x86-64 model uses lane units so `al` and `ah` are disjoint while `ax`/`eax`/`rax`
alias the appropriate union, and records `eax`'s full-register zeroing write.
The baseline AArch64 model gives encoding-number-31 stack and zero-register
views distinct units, aliases `Wn`/`Xn`, and splits vector halves so the AAPCS64
low-half preservation rule is not inflated to all 128 bits. These declarations
currently include closed scalar call/return rows for System V, Microsoft,
AAPCS64, and Darwin plus Linux syscall and conservative inline-assembly rows.
The first ordinary rows cover i64 materialization, i64 copy, three-address i64
addition and subtraction, compare with zero, and conditional branch. AArch64
maps addition and subtraction directly to flag-transparent register ADD/SUB
forms; x86-64 realizes flag-transparent addition with LEA without inventing a
two-address tie. Its subtraction row is instead an alias-safe three-address
pseudo: `SUB` when the result aliases the left input, `NEG; ADD` when it aliases
the right input, and `MOV; SUB` when distinct. Because those realizations do not
produce one common arithmetic-flags value, the row explicitly clobbers RFLAGS.
Compare defines RFLAGS/NZCV, while branch explicitly uses that state and
updates RIP/PC. The selected vocabulary now emits proof-bearing exact-add and
exact-subtract base cases; these rows are not yet a complete ordinary-
instruction or feature-profile inventory. Arithmetic constraints describe
physical shape, not arithmetic policy: exact, wrapping, and trapping semantics
remain distinct in semantic lowering, and an exact row is usable only after
its required overflow obligation has been discharged. The selected-plan
identity roots each obligation and verifier-owned accepted fact, and target-
owned state effects flow unchanged through liveness, legality, and
deterministic homes.
The current target-owned validators also require the supplied physical model
to equal the ISA's canonical declaration before constructing or comparing
constraint rows. This prevents a self-consistent forged model and catalog from
redefining the expected target semantics; fixed-register view resolution fails
closed for the same noncanonical input. Selected staging reruns its independent
projection validator against the exact retained environment before issuing its
cross-stage custody receipt.

The register environment has four distinct replay identities. The physical
identity covers every unit, view, class, convention, and reservation
declaration in stored canonical order. The instruction-catalog identity covers
every required key, operand constraint, tie, early-clobber bit, and implicit
unit effect and is also bound to the physical identity whose local IDs it uses.
An active-reservation-profile identity covers a named, strictly sorted subset
of reservation overlays and the independently recomputed effective unit union;
declaration alone never activates an overlay. Finally, the environment identity
binds the exact native target, the three component identities, and the selected
materialize/copy/add/subtract/compare/branch/return keys. Selection, liveness,
live-range, and
transitional-assignment custody all retain this joined identity. Those receipts
also retain the validated pre-physical optimization-manifest identity required
by a later physical/publication manifest join.

The first active policy is explicitly named
`omega.conservative-baseline-v1`. It reserves all declared overlays except a
known inapplicable platform overlay (currently Darwin AArch64 `x18` on a
non-Mach-O target). This safely over-reserves frame, dispatch, and metering
units while those plans are not yet joined to allocator input. It is not an
optimization level or a hidden build mode, and it does not claim closure over
future provider/backend reservations. A provider-backed allocation must add
its exact reservation requirement to a named active profile or fail closed.
`NativeTarget` currently cannot distinguish Windows x64 from UEFI x64; if their
reservation policies diverge, the environment must additionally bind the
deployment `TargetProfile` (or a successor policy identity) before allocation.

These declarations do not alter the current scratch-cycling assignment lane
and are not allocator output evidence. The selected optimizer lane may retain
that transitional
lane's assigned plan inside `StagedOptimizedAssignedOperations` to prove
cross-stage custody, but cannot treat it as allocator-validated input to
machine emission.

The bounded production liveness slice is deliberately a CFG fact layer, not a
partial allocator. It computes a deterministic reverse fixed point and records
canonical block, instruction, and successor facts. Virtual-register transfer
uses `before = uses union (after - defs)`. Architectural register units use a
separate transfer, `before = implicit_uses union (after - implicit_defs -
clobbers)`, so flags, instruction-pointer state, stack state, and return-address
state never become invented virtual values. Fixed views remain operand-position
constraints; they do not masquerade as implicit unit uses. Branch successor
rows retain source edge, target, and nonzero/zero polarity even when their live
sets happen to be equal.

Production and validation do not share a transfer implementation. The
validator reconstructs selected instruction order, CFG successors, operand
roles, machine-state effects, fixed positions, canonical exact sets, and the
full content identity before an opaque result is issued. Orchestration nests
that result with the complete selected carrier and revalidates both layers.
This first slice intentionally refuses use-def operands, ties, and early
clobbers, and makes no claim about flattened intervals, allocation, spills,
loops, calls, crashes, cleanup, or suspension. Those become admissible only
with their explicit selected-IR frontiers and dedicated validation rules.

The next allocator-input artifact preserves that CFG meaning instead of
collapsing every value to one global numeric interval. Each dense instruction
position has a before and after point. A range is a canonical set of maximal
half-open fragments within exact block domains plus explicit connectors for
the selected successor edges on which the value remains live. Occurrence and
fixed-view rows retain their exact phase. Canonical unordered pairs record VReg
interference only when fragments overlap in the same block; mutually exclusive
leaf values therefore remain noninterfering. The shared forwarded parameter
supplies the first positive cross-edge and condition/result interference case.

Architectural units have their own semantic fragments and edge connectors.
Instruction uses, defs, and clobbers are separate phase-specific action rows:
a dead instruction-pointer write must constrain a future home without falsely
becoming a live architectural value. An independent implementation reconstructs
all domains, fragments, connectors, occurrences, fixed sites, actions, pairs,
and the content identity. The orchestration carrier nests complete liveness
custody and grants no splitting, physical-home, spill, frame, emission, or
publication authority.

Before allocation, a separate legality artifact joins those ranges to the
identity-bound register environment. For every occupied VReg point it publishes
the exact class-compatible views whose storage and canonical-write footprints
avoid the active reservation union, architectural semantic liveness, and
same-phase architectural actions. A fixed operand may name a view that is not a
general allocation candidate, but it must still have the right class and avoid
reserved or architectural units. Production and replay independently derive
the exact nonempty, sorted candidate rows.

The same artifact exposes incompatible entry and operand fixed views as
transition requirements. In the current forwarded-value fixture, the shared
parameter arrives in `RSI`/`X1` and each leaf return requires `RAX`/`X0`.
Recording that mismatch is not permission to change homes silently: a later
named copy/split transformation must choose whether to hoist a transition or
duplicate it on paths, materialize the selected CFG change with provenance,
and pass independent validation. Only then may linear scan consume the legality
rows and virtual interference to publish physical homes.

The first allocator should be deterministic linear scan with interval splitting,
spills, reloads, rematerialization of cheap constants, and verified frame-slot
assignment. Subsequent policies may add optimistic coalescing, greedy coloring,
pressure-aware scheduling, live-range splitting around calls, and profile-guided
decisions.

An allocator verifier independently replays liveness and checks that:

- simultaneously live values never occupy overlapping register units or stack
  slots;
- fixed/tied constraints and calling conventions hold;
- every clobbered live value is preserved;
- spill/reload and rematerialization sequences reconstruct the right value;
- stack slots have sufficient size/alignment and disjoint live occupancy;
- address-stable loans retain stable homes;
- cleanup, suspension, boundary, and debug frontiers can recover required
  values; and
- emitted state-footprint evidence matches actual register and machine-state
  writes.

## Machine optimization

Machine optimization runs on symbolic instructions before encoding. Separate
pre-allocation and post-allocation groups avoid asking one representation to do
both jobs.

- Pre-allocation: machine CSE, copy coalescing opportunities, instruction
  alternatives, latency-aware scheduling, and block layout hints.
- Post-allocation: redundant move/spill removal, load/store forwarding within
  proven stack slots, peephole combines, branch inversion/fallthrough, concrete
  scheduling where safe, and size/latency instruction choice.
- Encoding remains responsible for branch displacement resolution and exact
  bytes. A machine rule may request a short form but may not guess final
  placement.

Machine instructions carry explicit register and machine-state uses/defs,
memory effects, trap behavior, and Terminal Psi provenance. A peephole cannot
erase an instruction merely because its register result is dead when it also
has an effect, trap, fuel charge, cleanup, or observable state transition.

The first implemented machine layer is deliberately a sidecar over the sealed
selected CFG, not a second mutable instruction IR. Target-owned catalogs name
all eight currently admitted selected semantics and bind each to its exact
register-constraint row, explicit effect classifications, control-barrier
status, ordered target alternatives, and size/latency knowledge. The generic
structural validator checks roster and applicability closure; x86-64 and
AArch64 validators then compare the whole catalog to their canonical target
facts.

`omega-machine-optimizer` walks ordinary selected instructions followed by
each selected terminator and independently reconstructs the same per-program
effect plan. The plan retains full proof-bearing instruction payloads,
architectural unit uses/defs/clobbers, Terminal Psi operation/value/edge/
obligation provenance, logical-fuel settlements, and every currently legal
target alternative. It is rooted in the selected-plan, optimization-unit,
fuel-schedule, target-register-environment, register-constraint-catalog, and
machine-effect-catalog identities. This is pre-allocation analysis authority
only: explicit VReg operands have no physical write footprint until homes are
joined, and no alternative is chosen here.

The sidecar crosses process or cache boundaries only through its strict,
versioned codec. The envelope carries an explicit magic, version, and content
identity;
the decoder consumes the complete closed vocabulary and rejects truncation,
trailing bytes, unknown tags, and stale identities. Orchestration admits the
same analysis over four separately validated selected forms: original
selection, fixed-view-copy output, the final result of an explicitly invoked
literal-fold sequence, and a completed named selected-lowering suite. Its
custody receipt names which form was used and retains that form's independently
replayed receipt, so a transformed CFG cannot inherit the source CFG's machine
facts by shape or convention. The named-suite route remains meaningful when
its independently validated result contains zero rewrites.

Target alternatives state uncertainty instead of guessing. AArch64 arbitrary
i64 materialization is encoder-resolved because its current canonical variant
may expand from one zero-seeded MOVZ through three ascending nonzero MOVKs. A
future MOVN/minimal-seed materializer must use a separately named/versioned
policy. x86 branch and register-dependent forms remain encoder-resolved.
The x86 exact-subtract pseudo exposes ordered alias-safe cases, including the
right-alias case only when the left input is distinct, and carries the RFLAGS
clobber from its constraint row. AArch64 exposes one flag-transparent SUB
alternative. A later home-aware choice must select exactly one applicable
alternative and prove its concrete physical footprint before encoding.

That home-aware boundary is now a separate immutable post-allocation sidecar.
It joins the selected identity, pre-allocation effect identity, live ranges,
allocation legality, homes, post-allocation manifest, target environment,
physical register model, constraint catalog, and machine-effect catalog. For
each operand it records the physical view, storage units, access-qualified read
and write units, and the model's exact write semantics. Per-instruction rows
retain the implicit effects separately and also publish canonical complete
use/def/clobber sets.

The initial choice rule is named `UniqueApplicableInCatalogOrderV1`. It is a
legality partition, not a performance heuristic: exactly one declared form
must apply, while zero or multiple forms reject. All four x86 subtraction
alias cases are tested as a disjoint partition. x86 LEA addition has one
always-applicable alternative for allocator-produced GPR64 homes: R12 is a
valid SIB index when `REX.X=1`, while reserved RSP is the no-index encoding.
Thus R12+R12 remains legal without silently changing to flag-writing ADD. Once
a target has multiple simultaneously legal forms with a performance tradeoff,
choosing among them becomes an explicit named policy decision with its own
receipt.

An alternative also carries an encoded-realization contract distinct from the
selected instruction's semantic and ABI custody. It names which selected
operands actually feed the encoding, which physical units the realization uses,
defines, or clobbers, and its memory, stack, trap, and control behavior. The
distinction is material: x86's XOR-zero subtraction reads no external source
operand even though it realizes a semantic subtraction, while a return's ABI
result remains live in RAX/X0 even though the return opcode does not read that
operand. Catalog, pre-allocation, and post-allocation identities bind this
contract; the strict pre-allocation codec is v3 for the same reason.

Orchestration reconstructs this sidecar for ordinary selected homes,
fixed-view-copy output, an explicit literal-fold sequence, and named selected-
lowering completion, always retaining the matching transformed or verified-
no-change custody and validated post-allocation manifest. This still grants no
emission authority.

The next boundary is implemented for layout-independent scalar forms in each
clean ISA owner's `selected_form_encoding` module. Physical `RegisterViewId`s
are resolved by exhaustive target-owned architectural-name tables rather than
model ordering. Encoders emit one canonical schema: AArch64 fixed words plus
zero-seeded materialization, and x86 exact-width moves/tests, deterministic
LEAs, and alias-partitioned subtraction sequences. Separate limited decoders
parse opcode and operand fields, reconstruct immediates, reject trailing or
noncanonical bytes, and publish decoded register/flag footprints.

`omega-optimization-pipeline` then builds an immutable pre-layout fragment
artifact rooted in both the selected plan and post-allocation machine sidecar.
Every row binds its instruction, chosen alternative, canonical bytes, resolved
size, decoded footprint, and encoded-realization effects under its v2 identity.
Replayed validation compares external physical reads/writes, implicit units,
memory, stack, trap, control, and catalog size bounds with the chosen catalog
alternative. x86-64 near return is independently decoded as canonical `C3` and
records RSP use/def, RIP def, an eight-byte activation-stack read/pop, possible
architectural fault, and return control. AArch64 independently decodes canonical
`RET X30` (`C0 03 5F D6`) and records X30 use, PC def, unchanged stack, no memory
access, and possible architectural fault. Neither form falsely reports the ABI
result operand as an encoded read. Conditional branches alone remain explicit
layout-deferred rows. Consequently this artifact is neither a concatenated code
section nor emission/publication authority.

A separate immutable v1 artifact resolves those branch rows only after choosing
the explicit required-stage layout policy
`EntryThenZeroFallthroughThenNonzeroV1`. For the admitted three-block diamond it
orders entry, zero successor, then nonzero successor; independently proves that
the zero edge begins at branch end; and retains the exact selected edge/block
identity and offset for both paths. The branch remains a direct realization of
the selected nonzero predicate rather than silently inverting it. x86-64 emits
one canonical six-byte `JNE rel32`, using a signed displacement from instruction
end. AArch64 emits one canonical four-byte `B.NE imm19`, using a four-byte-scaled
signed displacement from the branch word. Target-owned validators independently
decode predicate and displacement and reproduce the catalog's flags/PC/control
effects. The new identity binds the selected, post-allocation, and pre-layout
roots, target, named policy, all function/block/instruction spans, successor
custody, bytes, displacement, and decoded effects. It still owns only separate
function-relative fragments—not section placement, symbols, object relocations,
executable bytes, or publication.

Selected-lowering orchestration seals those products in a structured v2
realization manifest. The record joins the full named build suite, its exact
selected-lowering projection and completion identity, the pre-physical and
post-allocation manifests, final selected CFG, pre- and post-allocation machine
roots, pre-layout encoding, resolved layout, exact target, named layout policy,
and a validated whole-function exit-contract identity. It derives function,
block, instruction, byte, and resolved-conditional-branch counts from the
validated layout. A strict binary codec recomputes its domain-separated
identity and rejects old versions, unknown vocabulary, identity changes,
truncation, and trailing bytes; custody replay reconstructs every joined root
and both encoded artifacts.

The current exit contract is an exact named frameless-leaf policy, not a broad
backend mode. Target-owned resolution selects System V AMD64, Microsoft x64,
AAPCS64, or Darwin AAPCS64 from the exact target. For this policy,
unconstrained allocation is deterministically restricted to views whose full
read/write storage lies in the selected convention's caller-saved set; fixed
ABI operands still bypass that search restriction. Independent replay then
requires every non-return instruction to leave the stack unchanged and avoid
activation-stack memory, rejects any callee-saved definition/clobber, and joins
each exact Psi return edge to its final encoded row. x86 returns must be
canonical `C3`, retain the result in RAX separately from encoded reads, read and
pop exactly eight bytes at RSP, and return through the activation stack.
AArch64 returns must be canonical `RET X30`, retain the result in X0 separately,
leave SP unchanged, preserve X30, and transfer through that exact link register.
Both retain possible architectural return faults. The contract records the
caller's aligned-stack/return-state precondition; it does not pretend to prove
the external entry bridge supplied it or that an absolute SP value was
observed. A deliberately unrestricted x86 allocation that writes RBX now fails
at this boundary rather than acquiring imaginary save/restore evidence.

The manifest scope is therefore function-relative fragments with a validated
whole-function exit discipline. Frame construction, machine emission, section
placement, symbols, object relocations, executable image, installation, and
publication remain explicitly unavailable. This is an honest realization
checkpoint, not an object or native-final manifest.

Choosing x86 `rel8` is a separate prospective named transformation,
`X86RelaxConditionalBranchesToRel8V1`, with monotone fixed-point layout replay
and explicit work accounting. It is not an implicit “higher optimization
level” behavior of the baseline encoder.

Before the legacy assigned-operation emitter can be bypassed, the clean lane
still needs general CFG layout/non-fallthrough terminator bundles, framed and
calling exit policies with proved save/restore and unwind behavior,
authoritative entry-bridge and enabled-hardening identities, whole-program
spans and relocations, and publication enforcement of the independent verifier
receipt.

## Decision policy, search, and ML

Mechanism answers **what transformations are legal**. Policy answers **which
legal candidate to choose**. Every choice with a meaningful performance
tradeoff becomes a stable decision point with:

- a versioned feature schema;
- a finite legal action set or candidate list;
- target and cost-model identities;
- the selected action and deterministic tie break;
- optional workload/profile identity; and
- measured or predicted cost, excluded from correctness.

The baseline policy is deterministic and model-free. An offline search or model
may later emit a decision log, which the ordinary compiler replays. Every
resulting rewrite still passes the same validators. Models do not load into the
compiler by default, do not define legality, do not enter the proof kernel, and
do not become runtime dependencies.

Training/evaluation records use a clear functional shape:

```text
input:
    canonical decision features
    target/cost-model identity
    legal candidate summaries
    optional workload-profile digest

output:
    chosen candidate identity
    validator verdict
    realization identity
    code-size and performance measurements
```

Raw user source, authored names, absolute paths, nondeterministic arena
addresses, and unstable debug formatting are not model features. A workload
profile is a separately declared build input with a digest and custody record.
Absence of a workload profile must be a valid deterministic path.

## Build hook and rollout

Optimization is initially enabled only by the root package's authoritative
`build.omg`. There are no `debug`/`release` compiler modes and no `O1`/`O2`/`O3`
intensity levels. Debug information, compiler assertions, diagnostic/report
detail, and optimization selection are independent axes.

The toolchain-provided zero-initializable build vocabulary grows an empty-by-
default optimization selection value. Each opt-in names the actual
transformation family:

```omega
data Optimization {
    case ControlFlowCleanup;
    case SparseConditionalConstantPropagation;
    case CopyPropagation;
    case GlobalValueNumbering;
    case DeadPureScalarElimination;
    case ProofCheckElision;
    case SelectedIncomingU12ExactAddImmediate;
}

data Optimizations {
}

data Build {
    subsystem: Subsystem;
    freestanding: bool;
    optimizations: Optimizations;
}

machine Optimizations::enable(&mut self, optimization: Optimization) {
}

machine build(builder: &mut Build) {
    builder.optimizations.enable(Optimization::ControlFlowCleanup);
    builder.optimizations.enable(
        Optimization::SparseConditionalConstantPropagation,
    );
    builder.optimizations.enable(Optimization::DeadPureScalarElimination);
}
```

The spelling above is illustrative; the landed build vocabulary must follow
Omega's ordinary method and build-evaluation rules. Its semantic requirement is
an exact set of named selections. Empty means disabled. Registry metadata may
mark an individual optimization experimental, preview, or stable, but that is
an admission/support label on that optimization—not a broad compiler mode.

Each named transformation also has one closed execution phase. Phase routing
projects the full requested suite into exact subsets; it does not invent a
level, preset, or implied companion optimization. Custody records retain both
the full build request and the subset completed at that stage. For example,
`SelectedIncomingU12ExactAddImmediate` belongs to selected lowering, so a
pre-physical Psi receipt may retain it in the requested suite while recording
that it completed no Psi pass. A later selected-lowering receipt must bind the
same full request before the suite can be considered complete.

Rules:

- no `build.omg`, no assignment, dependency build file, API default, or legacy
  consumer enables optimization;
- only the root build result can enable named optimizations;
- an empty optimization selection takes the pre-optimizer lowering/emission
  control path and does not
  even instantiate optimizer registries or models; the build-vocabulary and
  manifest schema extension may change compiler metadata, but not program
  semantics, optimizer decisions, or executable bytes attributable to an
  optimization;
- unknown or duplicate selections, unavailable passes, unsupported Terminal
  Psi slices, and
  failed validation reject the optimized build rather than silently falling
  back to a different artifact;
- check-only compilation may validate the setting but does not require native
  optimization work unless it is explicitly asked to emit an optimization
  report;
- the exact selected set, normalized rule set, decision log, target cost model,
  and transformation-ledger identities enter cache and artifact/rebuild
  metadata;
- dependencies cannot add selections or inject rules; and
- every selectable optimization is semantics-preserving. There is no lossy
  float selection.

The provisional escape hatch that lets a program author its own `Build` data
shape must remain compatible: an authored legacy shape with no `optimizations`
field is disabled. Once toolchain-owned build vocabulary becomes exclusive,
the ordinary field is required and zero-initialized like the rest of `Build`.

Legalization, ABI lowering, register assignment, relocation, and encoding are
required compiler stages rather than optional optimizations. Their concrete
policy/algorithm identities remain reportable. Optional transformations alone
use the selection surface above.

Making one named optimization implicit by default is a separate owner decision
for that optimization after both its disabled-path compatibility gate and its
assurance gate close. There is never a promotion from one opaque optimization
level to another.

## Reports and diagnostics

An opt-enabled build emits a deterministic optimization manifest containing:

- source Terminal Psi and optimized realization identities;
- exact selected optimizations and complete ordered rule-set identities;
- per-pass input/output fingerprints and structural statistics;
- applied, skipped, and rejected candidates with reason codes;
- consumed proof/borrow/effect facts;
- transformation-certificate and validator identities;
- source-operation/edge and logical-fuel maps;
- allocator statistics, spills, frame size, and verifier result;
- code-size deltas and optional non-authoritative cost estimates; and
- decision-log and workload-profile digests when present.

Human text/HTML views are projections of a structured compiler-owned record.
They do not enter semantic identity. The report must be suppressible without
changing optimization decisions.

The current Rust slice implements structured/text projections through the
validated abstract-plan and strict spill-free register-home boundaries, plus a
selected-lowering-only function-relative/whole-exit realization projection.
The pre-physical manifest's versioned standalone codec serializes that whole
earlier record and strict nested codecs; the post-allocation record adds
truthful home statistics while marking frame, emission, and publication
unavailable. The function-relative v2 record then binds suite completion to the
validated final selected CFG, machine effects, post-allocation machine,
canonical encoding, named layout policy, resolved fragments, exact code-size
statistics, and the frameless whole-function exit contract. It explicitly
marks frame, section, relocation, image, installation, and publication fields
unavailable. All three records have strict self-authenticating codecs, but none
is yet wired into a compiler-owned artifact or rebuild-metadata section.
`OPT-MANIFEST-SCHEMA` remains open until later manifests join frame/emission/
publication records, enter that metadata path, and the compiler exposes a
suppressible report request without entering native optimization during
ordinary check-only builds.

The decision-row substrate is self-authenticating rather than caller-stamped.
Each row derives its identity from the exact input unit, candidate, rule,
verdict, consumed analyses, canonical typed fact references, and validator; its
codec recomputes that identity. This prevents a future top-level manifest or
human projection from faithfully rendering a row whose evidence was never
actually identity-bound.

## Folder ownership

The final Omega-written product source belongs under
`source/compiler/omega/omega/`:

```text
source/compiler/omega/omega/
  optimization/
    core/                 # selections, rule IDs, decisions, reports, ledgers
    psi/                  # target-neutral Terminal-Psi-derived optimizer
      analyses/
      passes/
      validation/
    lowering/             # target/legalization and target-operation passes
    regalloc/             # target model, liveness, allocation, verifier
    machine/              # symbolic-machine passes and verifier
    policy/               # deterministic baseline and decision replay
    cost/                 # non-authoritative cost models and feature schemas
  pipeline/               # optimizer orchestration hooks
```

During the Rust migration/reference phase, mirror responsibility rather than
files under `source/compiler/rust/omega/`:

```text
omega/
  foundation/omega-optimization-core/
  representations/omega-optimization-unit/
  representations/omega-register-model/
  representations/omega-terminal-selected-instructions/
  pipeline/omega-terminal-target-operations-to-selected-instructions/
  optimization/
    omega-psi-optimizer/
    omega-lowering-optimizer/
    omega-regalloc/
    omega-machine-optimizer/
    omega-optimization-validation/
    omega-optimization-policy/
  orchestration/omega-optimization-pipeline/
```

Keep each crate broad enough to own a coherent responsibility. Individual
analyses and rules are modules, not one-crate-per-pass. Existing representation
crates remain representation owners; pipeline crates transform but do not
become dumping grounds for shared types. Target ISA crates provide declarative
register/instruction facts and encodings, not global optimization policy.

## Testing strategy

Each rule has positive, negative, boundary, and fact-expiry tests. The full
system adds:

- representation-validator corruption tests;
- transformation-certificate mutation tests;
- allocator interference, clobber, spill, and frame corruption tests;
- exact float, trap, atomic, placed-memory, cleanup, suspension, and effect
  barriers;
- randomized Terminal Psi interpreter versus optimized-native differential
  tests;
- unoptimized-native versus optimized-native differential tests on every
  supported target;
- metamorphic pass-order, idempotence, fixed-point, and deterministic-rebuild
  tests;
- full canary/sample suites with no build opt-in, proving unchanged behavior;
- opt-in optimizer canaries with structured manifest expectations; and
- benchmark suites for compile time, peak memory, code size, throughput, and
  target-specific kernels.

Performance regression tests rank decisions but never authorize an otherwise
invalid transformation.

## Non-goals

- Rewriting or republishing canonical Terminal Psi as an optimized semantic
  artifact.
- Adding optimization to the legacy `StateGraph`/`ControlFlowPlan` boundary.
- Treating agreement with LLVM, a model, an SMT solver, or the Rust compiler as
  authority.
- Letting target packages teach the compiler arbitrary encodings through
  optimization rules.
- Runtime adaptive recompilation in the first architecture.
- Ambient fast math, implicit suspension points, or optimizer-created effects.
- Making ML, workload profiles, or benchmark corpora necessary for a correct
  baseline build.

## Resolved architecture decisions

- The semantic root is immutable Terminal Psi; optimized plans are realization
  artifacts with separate identities.
- The target-neutral optimizer is Omega-owned even when called the Psi
  optimizer.
- Correctness gates are independent of rule and policy implementations.
- The initial build surface is an empty-by-default exact set of named
  optimizations, with no broad optimization level or build mode.
- Float relaxation is expressed only by named source operations/contracts, not
  optimization configuration.
- ML/search may choose among or propose candidates but never defines legality.
- Register allocation is a first-class verified stage, replacing fixed scratch
  cycling.
- Durable work targets the Terminal lane; legacy adapters are allowed only for
  genuinely reusable backend machinery.
