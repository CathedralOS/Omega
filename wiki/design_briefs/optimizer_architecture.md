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

- The installed-output compatibility path in
  `source/omega-rust/omega/` still lowers `CheckedTrees -> StateGraph ->
  ControlFlowPlan -> AbstractOperations`. Its state-value planner performs
  useful expression substitution and constant folding, but it still consumes
  checked-tree expression handles. The retained-native product no longer uses
  this path.
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
         reconstructed obligations and their complete ordered proof questions,
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
    -> named physical realization and independently replayed encoding
    -> relocation-free function fragments
    -> validated dense text-section placement
    -> private-symbol Omega object container
    -> canonical Terminal semantic/proof-to-object artifact + rebuild record
    -> entry-specific semantic classification
       -> validated ordinary-callable entry classification, or
       -> retained ProgramEntry signature/calling plans
          -> checked-source signature / Terminal `MachineId` settlement
          -> validated declaration-only ProgramStorage semantic contract
          -> address-free semantic ProgramStorage wrapper plan
          -> claim-preserving call-aware Unit legalization
          -> atomic Unit-call selection + pre-allocation effects/liveness
          -> zero-VReg ranges/legality/empty homes + post-allocation effects
          -> typed internal-call fixup encoding
          -> function-relative custody and balanced whole-function exit
          -> fixup-preserving fragments
          -> whole-section resolution
          -> zero-relocation two-Machine object (generic codegen proof)
          -> compiler-owned wrapper encoding selection and replay
          -> synthetic one-function structural Unit object (encoding proof)
          -> exact settled claim-consuming continuation object/artifact
             (engineering prerequisite)
          -> composite semantic ProgramStorage wrapper object (next)
       (physical process entry, native image, install, and publication closed)
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
named-pass schedule. The currently supported combined schedule is SCCP,
constant-conditional CFG cleanup, then copy propagation. Each pass emits a
distinct chained manifest, even when it
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

This classification alone grants no authority to choose a strategy, move or
duplicate the original semantic instruction, add a native reconstruction,
change logical-fuel placement, allocate private storage, or emit code. The
first exact consumer is
`SelectedActiveResidentImmediateU64BeforeSingleFutureFlexibleUseV1`. It admits
only an active-resident literal with one local unconnected range, exactly one
later same-block flexible use, and no later fixed use. The original
`MaterializeI64`, operation provenance, and logical fuel remain unchanged for
the already-executed segment. Immediately before the sole future use, the
transform inserts one deterministic fresh VReg and `MaterializeI64`, rewrites
only that operand, and gives the new instruction zero logical fuel plus only
the original source-value lineage—never operation, edge, or obligation
custody.

The sibling
`SelectedActiveResidentImmediateU64BeforeFirstOfMultipleFutureFlexibleUsesV1`
admits the same exact active-resident literal only when it has at least two
strictly later same-block flexible uses, one local unconnected range, and no
later fixed use. It inserts one fresh zero-fuel, value-lineage-only
`MaterializeI64` before the first future-use instruction and rewrites every
classified future use to that same fresh VReg in canonical
instruction/operand order. Earlier and unrelated uses are untouched. Both
policies use one ordered rewrite-row action schema. Its strict v2 recipe codec
roots the sealed selected analysis,
spill-choice/classification evidence, availability, environment, budget, and
transformed identity. A structurally independent validator replays the recipe;
fresh liveness, ranges, interference, legality, and home assignment prove the
admitted two-view pressure point closes. The bounded API performs no implicit
loop and grants no memory spill, stack, frame, emission, or general
rematerialization authority.

That multi-use policy now has one production-reachable vertical rather than
only hand-built allocator fixtures. The seventh closed legalization recipe has
one leaf materialize `resident`, `left`, and `right`, compute
`inner = exact(left + right)`, `middle = exact(resident + inner)`, and
`result = exact(resident + middle)`, then return `result`; the other leaf is a
single immediate return. With an explicitly validated two-view availability,
materializing `right` creates pressure before the first exact addition while
`resident` is active, and its farther exclusive end makes it the canonical
victim. Its two later flexible uses are therefore the exact multi-use suffix
admitted by the rule.

`stage_optimized_active_resident_rematerialization` is a deliberately one-step
opaque carrier over that case. It fixes the spill-choice, recovery-
classification, and multi-use rematerialization policies, requires one shared
work budget and at least one applied action, and performs no hidden fixed-point
loop. After the selected CFG changes it recomputes liveness, ranges,
interference, availability-bound legality, and homes from scratch, then emits
one typed pressure-rematerialization manifest row. Independent replay starts
from the retained source legality custody and reconstructs every decision,
rewrite, fresh analysis, home, manifest, policy, budget, and count before
returning its dedicated full-vertical custody receipt. No source analysis fact
crosses the selected-CFG mutation.

The default-off root-build family
`ActiveResidentImmediateU64MultiUseRematerializationV1` now exposes exactly
that schedule. It belongs to `AllocationRecovery`, implies no companion
transformation, and derives a target-owned two-view pressure policy using
`rax`/`rcx` or `x0`/`x1`. The physical pipeline requires the singleton phase
projection, one sweep, and at least one action, then retains the transformed
program through fresh allocation evidence, post-allocation machine planning,
canonical encoding, resolved layout, and function-relative realization. A
missing required view, no-candidate shape, second allocation-recovery family,
or unfinished physical-phase composition fails closed. Empty ordinary and
dependency builds never enter this route. The physical carrier itself grants
no frame, emission, image, installation, or publication authority; subsequent
generic custody carriers now advance its exact root-build result through
relocation-free fragments and text, private-symbol object serialization,
canonical Terminal semantic/proof-to-object joining, ordinary-callable ABI
classification, and the opaque cumulative report without weakening that
boundary.

Other exact transformations consuming that evidence are the separately named
`SelectedIncomingU12ExactAddImmediateV1` and
`SelectedIncomingU12ExactSubtractImmediateV1` policies. They are intentionally
not generic literal sinks or rematerializers. In the production pressure case
the authored right literal is already immediately before its only use, and
moving it cannot remove the simultaneous register uses of a three-register
exact binary operation. Each admitted transform folds one incoming unsigned
`0..=4095` literal into an immediately following proof-bearing consumer, and
only in operand 1; subtraction never commutes or reorders authored operands.
Both target owners publish distinct two-operand add- and subtract-immediate
constraint rows. x86-64 realizes the flag-transparent forms with LEA, using a
canonical negative displacement for subtraction; AArch64 uses non-S ADD- and
SUB-immediate forms so neither alternative changes architectural flags.

The transformed `ExactAddI64Immediate` or `ExactSubtractI64Immediate` retains
the original arithmetic obligation and accepted proof-fact identity. Its
provenance concatenates the literal and consumer operations and logical-fuel
settlements exactly once, while retaining the consumer's source values, edges,
and obligations. The literal
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
one-view production fixtures, two explicit invocations with full reanalysis
close the mutually exclusive leaf pressure points and reach deterministic
homes on x86-64 and AArch64. This establishes exact add and subtract physical-
form pressure recovery without granting spill, frame, emission, or general
rematerialization authority.

The source-visible `SelectedIncomingU12ExactAddImmediate` and
`SelectedIncomingU12ExactSubtractImmediate` families use separate compiler-
owned schedules, with a third exact schedule for the canonical set containing
both names. The corresponding add-only, subtract-only, or combined policy
repeatedly invokes the one-step primitive, with complete reanalysis after every
changed selected CFG, until one final independently validated attempt applies
zero actions. The combined policy admits only the union of the two named
shapes; it is not a new optimization name or hidden suite identity. Every
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
ledger. Its current variants distinguish fixed-view-copy, literal-fold, and
pressure-rematerialization identities; order, kind, and identity are all
canonical inputs, and exact duplicate identities reject rather than being
silently collapsed. A
separate optional selected-lowering completion identity proves execution of a
named suite even when that ledger is empty; it is not itself a transformation.
The selected root is always the final transformed CFG. Exact function,
assignment, distinct-view,
interference, and remaining-transition counts are structured fields. Because
the strict home allocator rejects pressure and unresolved transitions, this
record may truthfully say no spill was required for the admitted plan. Frame
layout, machine emission, and publication remain explicitly unavailable, so
the record grants none of those authorities.

The post-allocation record also has a versioned canonical codec (v5 after the
pressure-rematerialization transformation join). It reconstructs
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
CFG and a domain-separated transformation identity. A strict `OMGFCV` v3 codec
binds either closed policy, every source root and work counter, insertion and
ordered destination sites, constraints, the transformed selected identity, and
the complete transformed CFG including provenance and fuel. Decoding
authenticates bytes into a plain unchecked plan; the independent validator is
still required before custody can advance.

Because the selected CFG changed, a sealed validated-analysis boundary reruns
liveness, ranges, interference, architectural state, and candidate legality
from scratch and requires zero remaining transitions. Only then may a separate
post-copy custody carrier invoke the unchanged strict home assigner. Both paths
stop before machine emission. General splitting around calls or pressure,
address-stable values, spills, and frames remain future named capabilities.

The allocator core also exposes the distinct
`SharedEntryAfterCompareBeforeBranchV1` policy for that exact forwarded-u64
diamond. Instead of cloning a copy into both leaves, it requires the entry's
`CompareI64Zero` to be immediately followed by its conditional branch and uses
the target catalog's flag-transparent `CopyI64` guarantee to insert one shared
copy between them. Both return sites are retained as ordered destination rows
and rewritten to one fresh VReg. Production and independent replay separately
reconstruct the exact source, insertion point, two successors, fixed views,
provenance, fuel, and flag footprint. The v3 copy identity binds the policy,
insertion block/instruction, and every ordered destination. This is a legal
strategy artifact, not an implicit profitability choice. The exact, default-
off root-build selection
`SharedEntryFixedViewCopyAfterCompareBeforeBranchV1` projects into the closed
`AllocationRecovery` phase and chooses only this policy. Orchestration then
reruns the full selected analyses, requires zero remaining transitions, assigns
fresh homes, and reconstructs post-allocation machine facts. Composition with
other physical phases is fail-closed until an explicit ordering/custody
contract is admitted.

The carrier exposes the projected abstract plan only by borrow while retaining
the verified input and complete optimization run. A second optimized-only
carrier lowers it to target operations while retaining the first carrier and
exact target. When checked providers are installed, this carrier also owns the
opaque admitted installation behind a borrowed projection; downstream selected
and physical carriers retain it transitively rather than copying its facts into
a detachable side channel. This prevents an optimized consumer from obtaining
either plan or an installed-provider projection by consuming and discarding its
evidence. The ordinary bare-plan lowering API remains for the empty-selection
compatibility lane.

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
after the full Terminal module is discarded: structural types and domains, the
complete service declaration graph, boundary machines, and the complete
checked provider-candidate catalog. A
function's immutable signature and structural custody are likewise exact,
including nominal attachment, ordered scalar and structural parameters,
Unit/scalar/structural result shape, the complete structural-place catalog,
ordinary and content entry claims, and the normalized published service
ceiling, exact verified machine contract, and per-machine evidence-contract
lane roster. These are not optimizer summaries. Unit content identity v16
encodes every field, transformed-revision validation compares them with an independent
projection of the immutable verified input, and abstract-plan projection reads
them from the unit before an independent round-trip check. Passes may inspect
this custody but cannot rewrite it.

The unit also owns the exact verifier-projected ownership frontier catalog.
This is immutable source authority, not a mutable reconstruction of the current
CFG: rewrites may remove a source operation or edge but must retain its fact
row. Catalog and nested snapshot ordering are canonical, the complete catalog
is bound by unit content identity, and validation compares it to an independent
projection from `VerifiedTerminalOptimizationInput`. A later analysis may use a
row only at its exact source site unless it proves a new current-region
relationship.

The unit separately retains the verifier's complete ordered proof-question
catalog. Every self-authenticating row binds the Terminal-Psi identity and
proof-bundle fingerprint; exact `Operation`, `CallRequires`,
`NominalCleanupRequires`, or `ContractEnsures` owner coordinates; obligation
identity and admission class; canonical proposition bytes; ordered machine
requirements and verifier-derived semantic axioms; and canonical-certificate
policy. The verified builder projects rows one-for-one in reconstruction order,
and the independent validator reprojects the complete catalog for both initial
and transformed revisions. Rewrites retain rows for removed and pruned source
sites as historical proof custody. This does not make a premise a function-wide
range fact: a future proof-derived range analysis must establish an exact
current-revision site or region before consuming it. The existing narrower
accepted-operation fact index remains the rule-facing capability for current
proof-certified scalar rewrites.

The unit must not contain syntax nodes, `ExpressionHandle`, authored names as
identity, native byte offsets, physical registers, or target instruction
encodings.

Block parameters retain Terminal-Psi declaration order as explicit input data;
they are never reconstructed by sorting parameter identities or by observing
only the incoming edges that happen to survive. Cached definition/use rows are
convenience indices, not authority: the independent representation validator
re-derives them from operation semantics. Re-derived metadata is still not a
semantic type proof: the validator separately checks the closed, wildcard-free
scalar operation vocabulary, including literal domains, operand/result types,
cast and carrier legality, control and return types, and exact scalar signatures
for internal and boundary calls against complete duplicate-free catalogs. Thus
a malformed pass cannot make an ill-typed operation acceptable merely by
refreshing its definitions, uses, fact index, and content identity. Structural
calls are independently replayed against the retained catalogs for arity, root
and path grammar, resolved type, source and destination access, exclusive
overlap, multiplicity, qualifications, structural result custody,
ordinary/content claim transfer, and complete boundary requirement/completion
correspondence. The liveness check mirrors Terminal's one live-claim namespace:
ordinary and content-entry declarations remain distinct authority, while a
content-only claim may be transferred or completed exactly like an ordinary
claim. Focused internal/boundary corruption tests and one source-derived
content-bearing admission canary cover that union. The retained structural-type
catalog is independently traversed as a whole after unknown-target checks;
Record, FixedArray, Sum, and both Mixed edge lanes must be acyclic even for
unused declarations and disconnected graph components. Nested Record, Sum, and
Mixed declarations also replay canonical per-namespace field/case ID order,
unique nonempty identities, nonempty case sets, and Terminal's closed
field-relevance/carrier matrix. A relevant opaque field is legal only with an
exact retained Record provider-attachment field witness; payloadless cases and
independent field namespaces remain legal. Top-level structural type and domain
rosters require canonical ID order and unique nonempty identities; domain
semantic identities are independently unique, carrier references are closed,
shared carriers remain legal, first-class byte sequences are borrowed views,
and fixed arrays are nonempty. Byte-literal place catalogs additionally require
dense declaration ordinals, borrowed-view carriers, and one full typed
executable establishment per row. Witness correspondence is identity-based;
block storage order is not declaration authority. Every executable input of a
producer-defined root independently locates its current `CallStructural`
result, byte-literal, or explicit trivial-affine-local producer and requires it
to occur strictly earlier in the same block or in a CFG-dominating block.
Structural arguments, observations, return sources, and cleanup inputs all
participate; immutable source-frontier rows grant no current-site availability.
Compressed return-tuple locals have no executable producer requirement.
Source-derived content-bearing structural call/return and byte-literal boundary
canaries exercise that admission boundary. Trivial-affine local declarations
additionally require
dense declaration ordinals and empty-Record carriers. Each local has exactly
one full typed witness route: an executable `EstablishTrivialAffineLocal`, or
the compressed tuple on the currently emitted structural parameter return.
That tuple must equal the complete local catalog in declaration order, use
unique hidden source-operation identities disjoint from executable operations,
and pair with the exact parameter-zero/result carrier shape and reverse
local-plus-affine-tail discard sequence. Compressed no-ABI locals deliberately
remain outside executable `declared_places`. The optimization unit also binds
each hidden establishment identity into the structural-return node's
provenance and fuel immediately after the return edge, in tuple/declaration
order. Independent validation requires the matching immutable operation-entry/
operation-exit frontier pair to establish exactly that affine local. This
remains catalog, source-site authority, and current compressed-return
correspondence rather than authority to synthesize locals or a complete
current-CFG consume/cleanup replay.
Provider-backed attachments independently replay Terminal's complete closed
specialization rule. An attached function has exactly one relevant erased
Record provider field, no structural `self`, and a nonempty unique root roster
in boundary-ID order; every root retains the exact attachment/field pair and
names a known unattached boundary. The set of current `BoundaryCall` targets
must equal that roster, while repeated calls remain legal, and provider roots
cannot become runtime structural arguments to either `BoundaryCall` or
`CallUnit`. A source-derived repeated-write Console program exercises this
optimizer-admission boundary. Structural-domain content projections
independently replay Terminal's exact owner identity, algebra/expression shape,
nonempty parameter, canonical naturals, bounded recursive scalar grammar,
Record-only field-identity paths ending in Scalar leaves, and canonical
fingerprint. Retained content-entry claims must reference that exact owner
identity and algebra; forged or unknown owners reject before pass execution.
The service catalog is likewise immutable verifier-owned authority. Independent
validation requires unique service IDs and nonempty identities, strict known
parent order, an acyclic graph, and each declaration's complete transitive
parent closure. Function and boundary service ceilings must be canonical,
known, and parent-closed. Every scalar, unit, structural, and structural-scalar
internal call plus every boundary call must stay within the caller's ceiling;
`PortWrite` must name a declared service in that ceiling. Provider realized
ceilings must exactly equal the candidate function ceiling and refine the
declared boundary ceiling. Verified-input projection rejects replacement
catalogs even when locally self-consistent, while the source-derived Console
canary proves frontend catalog, ceiling, call, and provider custody.
The admitted structural root vocabulary now has an independent role layer as
well. Function and boundary attachments must be known; any `self` parameter is
unique and exactly matches its attachment, while an attachment may legitimately
have no runtime `self`. Logical root keys are unique per function across
parameters, result, operation results, byte literals, provider attachments, and
trivial affine locals. `BooleanStructuralField` is restricted to the entry
machine's exact readable affine, unqualified and unclaimed structural parameter,
one relevant Boolean Record field, one observation pair, no content-entry
claims, a Boolean scalar parameter, and nominal cleanup at every scalar return.
`ReturnStructural` may source only a structural parameter, the exact retained
`CallStructural` operation result, or the exact bounded
`EstablishPayloadlessCase` result admitted below, with structural type,
multiplicity, and qualifications equal to the declared result. Focused
corruption matrices cover the root and signature axes, and the nominal Boolean
convergence source canary crosses verified optimizer admission with its exact
observation roots intact.
Payloadless structural values cross a deliberately asymmetric boundary.
Optimizer-only artifact lowering retains `EstablishPayloadlessCase`, the exact
structural result and case identity, and every bounded `CallStructural` field
including requirements, crash continuations, and selected evidence. The
verified optimization unit additionally retains the callee's complete machine
contract and evidence-contract lane membership. Its independent validator
reconstructs Terminal's exact direct-producer exits, rejects calls or payload
construction in the leaf, requires empty scalar/structural signatures and
custody transfers, confines outcome-proposition roots to the declared result,
and checks the selected-evidence surface separately. Ordinary abstract lowering
still returns `UnsupportedPayloadlessCase`, and target lowering still rejects
materialization; optimizer retention does not silently create a tagged-sum ABI.
Real direct-constructor and guarded selected-evidence source programs exercise
the positive boundary, while independently refreshed corruptions cover every
retained classifier lane.
Root service reach is retained as identity-bound current-revision state rather
than immutable source custody. Independent validation starts at the selected
entry, follows all four internal-call forms, separates installation-bound
boundary identities from concrete boundary ceilings, expands concrete
`PortWrite` services through their normalized parents, and requires exact
canonical equality with the retained row. A constant-conditional rewrite that
removes an effectful dead region refreshes the row before computing the output
identity; any other stale or forged closure fails total validation. This keeps
future effect elimination honest without freezing the original over-approximate
closure forever. Real ordinary-boundary and installation-bound source canaries
cover the concrete and installation axes.
The bounded retained-ownership rung also preserves every Jump and Conditional
successor's exact ordered trivial-affine discard vector. Unit identity binds
both the executable operation and projected edge rows; independent validation
replays Terminal's eligible ordered-subsequence rule against immutable edge
entry/exit frontiers and requires the exit to remove exactly those places while
leaving claims and partial custody unchanged. Constant-condition folding copies
the selected edge's vector. Empty-block threading, block merge, and shared-
terminal fusion currently decline nonempty removed-edge cleanup until
composition has an admitted proof. Target lowering names the already verified
ownership-only erasure explicitly and emits no runtime instruction. This is
exact source-site retention and replay, not yet a current-CFG ownership solver.
Remaining current-CFG ownership/claim/cleanup replay, proof-derived
current-region range facts, and the wider crash/requirement and remaining
effect vocabulary remain explicit unfinished
validator layers. It
also rechecks the complete current
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

Complete proof-question custody is likewise immutable source authority rather
than an analysis conclusion. It preserves call and nominal-cleanup requirements,
contract guarantees, assumptions, semantic axioms, and certificate policy even
where the current abstract-operation vocabulary erases those proof-site fields.
Analyses may derive nonzero, interval, congruence, or case facts only by joining
these rows to an independently proven current site or region; catalog presence
alone never broadens their scope.

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
second sweep of the currently supported SCCP, CFG-cleanup, copy-propagation,
GVN, proof-elision, then dead-scalar schedule must produce an empty delta;
composing that delta preserves the first sweep's ledger exactly. For each
current multi-rule family—SCCP, CFG cleanup, GVN, proof-check elision, and
dead-scalar elimination—thirty-two shuffled pre-assembly contribution orders
reconstruct the same ordered registry identity and contracts, then produce
byte-for-byte equal final units, commits, work usage, decisions, manifests, and
ledgers on a real dependent fixture. The test never shuffles an assembled
public registry, because its order is intentionally semantic and
identity-bearing.

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
derives the identical snapshot and fact identity. The current thirty exact
rules cover every evaluable Boolean and integer operation in the admitted
verified-unit vocabulary. Structural-field and call results stay overdefined
without immutable structural-version or call-summary facts. This is a
current-vocabulary completion boundary, not permission to guess semantics for
future float, trapping, or otherwise extended scalar operations.

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
or an optimization-level bundle. Block parameters and their incoming bindings
are the current Psi vocabulary's only scalar copy form: calls directly define
their result `ValueId`, and there is no scalar `Copy`, `Move`, or debug-value
operation. A verified call-result artifact therefore exercises the existing
generic rule rather than adding a call-specific rewrite. It proves that the
parameter and binding disappear, uses select the dominating call result, and
the complete effectful call node, callee, effect link, provenance, logical
fuel, and function roster remain exact.

Terminal debug maps remain a separate identified input. They may name a block
parameter with `DebugSubject::Value`, so executable-unit correctness does not
by itself authorize publication of an optimized debug map after that parameter
is removed. A debug-projection gate must replay each validated scalar
substitution, retain every authored source span through a many-to-one
alias/recovery relation, prove every surviving projected subject exists in the
optimized module, and carry durable reconstruction custody in the ledger or a
separately validated receipt. This cannot blindly rewrite both subjects to one
debug-map key because the map permits only one primary span per subject. Until
the recovery vocabulary exists, the optimizer may retain the original debug
map as separate source custody but must not claim an optimized debug-map
projection.

The first `ControlFlowCleanup` rule has an equally narrow contract. A
conditional whose Boolean condition has an independently reconstructible SCCP
fact may become its selected unconditional jump and delete exactly the blocks
made unreachable by that choice in one atomic candidate. A fresh CFG walk
independently derives that complement, so shared or reconverged blocks remain.
The candidate identity binds the condition, Boolean value, both edge IDs, the
decision and removed blocks, and every surviving block whose dense effect links
must shift. The selected edge is `RealizedAt` its output successor edge and
retains its runtime logical charge. Every shifted surviving node is likewise
realized at its original input location. The rejected edge and every node or
successor edge in every deleted block are separately `ProvenUnreachableAt`
their exact original input occurrences:
their scheduled units remain durable audit custody but carry no runtime charge.
The v5 rule and v4 validator, registered in the v7 pass, independently
reconstruct the Boolean fact, successors and bindings, reachability, exact
affected roster, all disposition/fuel rows, dense effects, node metadata,
current operation facts, declared places, and output identity before total
validation. `CallGraph` is explicitly
invalidated, while verifier-accepted obligation and ownership-frontier catalogs
remain immutable source custody. The pass manager records only that validator-
accepted accounting. Ledger replay rejects unknown disposition tags, duplicate
cross-disposition sources, noncanonical rows, zero fuel, and source/fuel
mismatches. Projection validation then proves that the initial source/fuel map
is the disjoint union of the final realized map and all cumulative proven-
unreachable rows, with no resurrection. Human reports name both cases and
distinguish source-scheduled fuel from runtime charge.

The convergence measure counts functions, blocks, nodes, and successor edges,
so every accepted atomic fold/prune strictly decreases it. This authority is
specific to the reachability complement created by the proven conditional
choice; it is not a general dead-region eraser. Crash, cleanup,
suspension, call, and boundary-only blocks are never classified as empty: they
may disappear only when the fresh structural traversal proves them unreachable,
with every removed source site tombstoned and independently validated.

The second `ControlFlowCleanup` rule is the deliberately narrower
`linear-empty-block-thread.v2`. A non-entry block may be bypassed only when it
contains exactly one unconditional jump, has exactly one incoming edge, and
that incoming edge belongs to another unconditional jump. The validator
independently composes the removed block's typed parameters and outgoing
bindings and requires the verifier-owned snapshots at incoming-edge entry and
exit, removed-block entry, outgoing-edge entry and exit, and target entry to be
identical. Synthetic units without frontier facts are eligible only when the
function has no structural parameters, entry claims, or declared places.

The two jumps always execute together, so this linear case is an exact
many-to-one realization: the retained successor edge keeps its own identity and
stores its existing provenance/fuel followed by the removed edge's complete
provenance/fuel. No source is called unreachable. Candidate accounting realizes
that fused row at the predecessor and realizes every later surviving node whose
dense effect link shifts. The independent validator deletes the block, rebuilds
effects, facts, and declared places, recomputes identity, and totally validates
the result.

The optimization unit now places CFG-arm custody on `OptimizationEdge`, while
non-successor terminal exits remain node occurrences. Each transformation row
names an exact `PsiRealizationSite` in the input revision and its realized or
proven-unreachable output site. Ledger replay removes each input occurrence and
adds every declared output occurrence before advancing to the next record.
This represents one-to-one moves, many-to-one fusion, and one-to-many fanout
without confusing a reachable source with an unreachable tombstone.

The third rule, `path-qualified-empty-block-thread.v1`, uses that vocabulary to
bypass all incoming edges of a non-entry single-jump block atomically. It
independently composes typed bindings and checks the ownership-frontier chain
for every incoming edge. The outgoing occurrence is copied onto every incoming
output edge. Because Omega's validated Psi CFG is acyclic, those incoming edges
form an antichain; the total validator additionally rejects any duplicated
source whose edge occurrences can execute sequentially. A verified artifact
test fans one source out and then threads both resulting occurrences through
later rewrites, with projection replay reaching the exact final occurrence
map. Candidate v13, optimization-unit content identity v7, ledger v3,
`ControlFlowCleanup` v7, prephysical manifest v6, and optimized-plan projection
validation v7 bind this admission meaning. Marking a reachable outgoing source
`ProvenUnreachableAt` remains invalid. Physical publication for newly admitted
CFG shapes still fails closed until target/selected/native custody supports the
shape.

The fourth rule, `adjacent-single-predecessor-block-merge.v4`, is the first
nonempty redundant-jump elimination contract. It merges only the immediately
following target block and only when the jump is its sole incoming edge. The
target must begin with a real operation having no successor arms, consist of
its conditional terminator, or consist of an exact return/crash terminal with
edge provenance. These restrictions make the transformation block-
boundary erasure rather than non-adjacent code motion. The validator
independently reconstructs typed target-parameter substitutions, requires each
replacement definition to dominate every use (including uses in dominated
successor blocks), rewrites that complete use set, and requires identical ownership snapshots
at incoming-edge entry, incoming-edge exit, and target-block entry. Every moved
node occurrence is renamed to its new block/node location; the removed edge's
source and fuel are additionally realized behind the first operation's direct
provenance at that same node. For a conditional-first target, the incoming
source is instead realized on both mutually exclusive successor edges; the
unit antichain check rejects any sequentially executable duplicate. Later nodes
whose dense effects shift are also accounted for. Corruption tests reject
forged node and fanout realization sites, and full artifact tests replay the
ledger to exact one-block and three-block prephysical projections. Candidate
v20, optimization-unit content identity v16, `ControlFlowCleanup` v11,
prephysical manifest v14, and optimized-plan projection validation v15 bind this
admission meaning; ledger v4 expresses both the many-to-one move and one-to-many
fanout. Direct terminal fusion retains the terminal edge and removed jump edge
at the fused node, so return, cleanup, structural-return, crash, and fuel work
are not classified as empty. Nonadjacent merges and native publication
for inherited node-edge custody remain fail-closed.

The fifth rule, `shared-terminal-jump-fusion.v1`, removes one unconditional
jump into a shared terminal-only target without moving or deleting that target.
The target is non-entry, has at least two incoming edges, and contains exactly
one `Return`, `ReturnUnit`, `ReturnStructural`, or `Crash`. The chosen jump's
typed bindings replace target parameters only in the cloned terminal; the
retained target and its parameter declarations remain byte-for-byte unchanged.
Ownership snapshots at incoming-edge entry, incoming-edge exit, and target
entry must be identical.

Custody is an exact one-to-many plus many-to-one relation. The incoming edge is
realized at the cloned terminal, while the original terminal node is realized
both at the clone and at its unchanged target site with identical source and
fuel vectors. Total-unit validation permits duplicated edge provenance at node
sites only when every occurrence is an exact no-successor terminal and their
blocks are pairwise incomparable in the output CFG. Duplicate operation
sources, node/edge cross-kind duplication, same-block duplication, and
sequentially executable occurrences remain invalid. The rewrite preserves the
jump node's effect link, rebuilds metadata/facts/places and identity, and
strictly decreases successor count. Candidate v15, optimization-unit content
identity v9, `ControlFlowCleanup` v10, prephysical manifest v9, and optimized-
plan projection validation v10 bind the admitted fanout; ledger v4 already
expresses it. Full artifact replay reaches two exact mutually exclusive
terminal occurrences without classifying return, cleanup, crash, or fuel as
empty work.

The sixth rule, `unreachable-private-machine-pruning.v1`, is a module-roster
rewrite rather than a node rewrite. Its candidate decision point is the exact
canonical machine set, never a surrogate first node. The root set contains the
module entry, every provider candidate, every nominally attached function, and
their transitive internal-call and nominal-cleanup-machine closure. The rule
atomically removes the entire active complement, including disconnected call
chains and recursive islands; the independent validator reconstructs that root
closure without trusting the optimizer's call graph.

Function removal retains more custody than ordinary link-time dead stripping.
`PrunedMachineCustody` binds every removed machine to its verified source-roster
ordinal. Active and pruned machines must form an exact, order-preserving
partition of the source roster, while accepted-obligation and verifier-owned
ownership-frontier catalogs remain complete historical facts. Ledger v4
records roster dispositions independently of its node/edge occurrence rows;
every source-bearing node and successor edge in a removed function is also
tombstoned with its original fuel. Projection replay requires the cumulative
ledger roster to equal the final unit roster custody and rejects an omitted,
duplicated, reordered, resurrected, provider-root, attachment-root, call-root,
or cleanup-root machine. Candidate v14, optimization-unit content identity v8,
`ControlFlowCleanup` v9, prephysical manifest v8, and optimized-plan projection
validation v9 bind this admission meaning.

The seventh rule,
`non-adjacent-unique-predecessor-block-merge.v1`, owns the code-motion contract
that the adjacent rule intentionally excludes. The target must be a non-entry,
nonempty block with one exact incoming edge, and the predecessor must end in
that edge's unconditional jump. A target consisting only of an empty jump is
left to the earlier empty-block rules. “Non-adjacent” means outside the
immediately-following roster relation admitted by the fourth rule; a target
serialized before the predecessor is necessarily handled here even when their
roster slots happen to touch.

Source-roster order is never execution authority. The producer explicitly
requires CFG, dominators, use-definition, and ownership-frontier products. It
proves the predecessor dominates the target, reconstructs the sole incoming
edge and exact typed parameter bindings, and proves each replacement dominates
every parameter use. Those uses are rewritten across the complete function,
including calls, scalar operations, return values, conditional operands, and
successor bindings in dominated blocks. Values defined by the moved target
keep their `ValueId`s; their definition sites move to the predecessor and total
validation re-proves every downstream use.

The typed patch separately names the predecessor location, incoming `EdgeId`,
and removed target. Candidate accounting includes the predecessor and target,
every block changed by global substitution, and every block whose dense effect
links shift when target nodes cross the serialized roster. Each sourced target
node moves from `target:i` to `predecessor:(jump-index+i)`. Incoming-edge
custody is appended to the first moved direct node, or is realized on every
exact successor edge when that first node is a source-less control terminator.
No reachable target work is tombstoned. The validator independently rebuilds
dominators, substitutions, affected blocks, provenance/fuel rows, the roster
mutation in either direction, node metadata, dense effects, facts, declared
places, and content identity before total validation.

A verified non-topological Boolean artifact exercises two consecutive merges,
global parameter substitution into a descendant serialized before its
definition, a moved definition still used by that descendant, exact ledger and
prephysical projection replay, and successful x64/arm64 lowering. Candidate
v20, `ControlFlowCleanup` v11, prephysical manifest v14, and optimized-plan
projection validator v15 bind the new admission. Optimization-unit identity
v10 and ledger v4 require no schema bump because they already encode block
removal, moved definition sites, one-to-one occurrence moves, fusion, and
fanout.

There is deliberately no standalone “prune blocks already unreachable from
entry” rule over `PsiOptimizationUnit`. Terminal verification and independent
total-unit validation reject `UnreachableBlock`, including disconnected SCCs,
before any optimizer rule can propose a candidate. Dead-region pruning must
therefore remain atomic with the reachability-changing rewrite that proves the
region dead. Admitting a partially reachable pre-verification IR would be a
new layer and a separately versioned design, not an omitted cleanup rule here.

### Dead pure scalar work

`DeadPureScalarElimination` is an explicit named suite that currently expands
to two exact rules. `dead-unused-scalar-literal-elimination.v1` contains only
`BooleanConstant` and `IntegerConstant`.
`dead-unused-unconditionally-total-scalar-elimination.v1` contains only:

- Boolean not/equality and integer equality/order comparisons;
- integer bitwise not/and/or/xor and widening;
- wrapping integer shifts; and
- wrapping or saturating integer add, subtract, and multiply.

The second list is deliberately closed. Each admitted operation is pure,
unconditionally total for already verified typed operands, and carries no
operation obligation. Exact casts, exact arithmetic and shifts, every
divide/remainder policy, calls, structural work, and boundary/service/control
operations remain excluded. In particular, an admitted proof obligation is
not evidence that dead-code elimination may silently discard the operation;
proof-bearing elimination needs its own explicit rule and custody contract.

Both rules require value liveness and effect summaries. The independent
validator checks the candidate rule identity against an independent copy of
the corresponding closed operation list, reconstructs the exact
definition/type, rejects any operation-obligation reference, and proves
absence from live-out and every use site. A valid block always retains a
following terminator, so deletion realizes the removed operation's provenance
and fuel at the immediately following, co-executed node rather than marking
reachable work unreachable. Later sourced nodes shifted by deletion receive
exact ledger relocation rows, while dense effects, definitions/uses, literal
facts, places, and unit identity are rebuilt.

Node provenance retains the receiver's primary source as an exact prefix and
may then carry inherited operation or edge sources. A jump or conditional may
therefore hold unconditional inherited node custody before its arm-specific
successor edges. Global uniqueness, fuel equality, and terminal-antichain rules
still reject duplicated or co-executable occurrences. A verified wrapping-add
artifact first removes the unused total arithmetic node, then revisits the
earlier rule and removes both newly dead literals; artifact replay leaves the
return with all original source/fuel sites. Candidate v19, optimization-unit
content identity v11, the named v2 pass, prephysical manifest v13, and
optimized-plan projection validation v14 bind this meaning; ledger v4 already
represents the many-to-one moves.

### Local and dominator common-subexpression elimination

The exact `GlobalValueNumbering` suite expands in canonical order to
`same-block-obligation-free-total-scalar-cse.v1`,
`same-block-proof-certified-total-scalar-cse.v1`,
`dominator-obligation-free-total-scalar-gvn.v1`,
`dominator-proof-certified-total-scalar-gvn.v1`,
`phi-translated-obligation-free-total-scalar-gvn.v1`, and
`phi-translated-proof-certified-total-scalar-gvn.v1`, followed by
`same-block-proof-certified-compatible-policy-scalar-cse.v1` and
`dominator-proof-certified-compatible-policy-scalar-gvn.v1`, then
`phi-translated-proof-certified-compatible-policy-scalar-gvn.v1`. Each local
rule scans a block in node order and replaces a later equivalent result with
the earliest admitted leader. The obligation-free vocabulary contains
literals,
Boolean operations,
integer comparisons/bitwise/widening, wrapping shifts, and wrapping or
saturating add/subtract/multiply. The proof-certified vocabulary contains exact
integer casts, shifts, add/subtract/multiply/divide/remainder, and wrapping or
saturating divide/remainder. Calls, structural work, boundary/service work, and
control operations remain excluded.

The three compatible-policy rules are directional equal-value joins. A redundant
proof-certified exact add, subtract, or multiply may reuse an earlier
obligation-free wrapping or saturating operation with the same signed/unsigned
fixed-width family, domain, and operands. A redundant proof-certified exact
left or right shift may reuse the matching wrapping shift. Add and multiply
canonicalize swapped operands; subtraction and shifts preserve order. Division
and remainder are excluded because their proof-bearing leaders require a
different two-fact contract, and the reverse obligation-free-to-exact rewrite
is not admitted. The redundant exact operation must retain its own active
accepted obligation, which the rewrite consumes without mutating the historical
accepted-fact catalog. Local/dominator leader selection, dominance,
substitution, and forward provenance/fuel settlement are otherwise identical
to the corresponding same-policy rule and are independently reconstructed.

An expression key binds the exact operation family and policy, all literal
payload, complete source/result/count integer domains, and operand identities.
Only operations whose exact semantics are commutative canonicalize their two
operands: Boolean and integer equality, bitwise and/or/xor, wrapping or
saturating add/multiply, and proof-certified exact add/multiply. Ordered
comparisons, casts, shifts, widening, subtraction, division, and remainder
retain operand order. Each proof-bearing leader and redundant operation must
independently match an active operation-obligation reference and verifier-owned
accepted fact. The candidate names only the redundant fact as consumed
transformation authority; the leader fact remains active and the whole accepted
catalog remains immutable historical custody. Missing evidence makes an
expression ineligible. A proposed candidate with a foreign redundant fact or a
leader whose fact is absent fails independent replay.

The local rules require use-definition and effect summaries. The cross-block
rules additionally require the CFG and dominators, select the earliest admitted
outer expression in a strictly dominating block, and prove the selected result
dominates every rewritten use. They never substitute canonical BlockId roster
order for execution dominance. The validators independently reconstruct the
closed key vocabulary, proof facts, reachable CFG/dominator sets, exact typed
definitions, the redundant result's uses, and the one redundant-to-leader
substitution.

The redundant node's provenance and fuel move forward to the next co-executed
node, never backward to the leader. Rebuilding rewrites scalar operands,
call/boundary arguments, successor binding arguments, branch conditions, and
return values without removing bindings; successor edge custody is preserved
by edge identity. Definitions, uses, dense effects, fact/place indexes, unit
identity, and affected-region custody are reconstructed independently. A
parameter-fed wrapping-add fixture admits swapped commutative operands, removes
one add, rewrites its integer comparison consumer, and reaches a ledger fixed
point. Exact-add fixtures prove the same behavior locally and across a
non-topological dominator boundary while retaining both accepted facts and
manifesting only the redundant fact as consumed. A separately verified
Terminal artifact proves return-use substitution and optimized-plan projection
when the dominating leader appears later in the serialized block roster. A
diamond fixture rejects sibling-only equivalence and reaches a two-rewrite
cascading fixed point at its join.

The phi-translated rule is deliberately narrower than general PRE. A candidate
join expression must reference at least one typed block parameter. The rule
enumerates every incoming edge, translates those parameter operands through
the edge's existing binding positions, and requires an exact matching
obligation-free total scalar leader available before the source terminator or
in a strict dominator of that source. Each arm uses the same canonical
outermost-depth then `NodeLocation` choice as dominator GVN. Missing arms,
binding/type disagreement, and nonavailable sibling expressions decline.

The rewrite does not invent a value identity and does not globally substitute
the redundant result. It appends that result `ValueId` as a new typed join
parameter, appends a binding from each incoming edge to that arm's leader, and
removes the redundant join node. The independent validator reconstructs the
complete incoming set, translations, dominators, leaders, parameter position,
edge custody, definitions/uses, dense effects, facts/places, and provenance
relocation before acceptance. Corruption tests cover reordered incoming rows
and detached leaders; a verified Terminal diamond exercises publication
projection with both appended bindings. Candidate encoding v24,
optimization-unit content identity v16, the named v7 pass, prephysical
manifest v23, and optimized-plan projection validation v24 bind the current
meaning; ledger v4 already represents both node relocation and edge custody.
The proof-certified phi rule uses the same closed proof-bearing scalar
vocabulary as proof-certified local and dominator GVN. Every translated arm
leader must have its own active verifier-accepted obligation fact; the
candidate consumes only the redundant join operation's fact, preserves the
accepted catalog as immutable history, and removes only that redundant active
reference. Independent replay rejects a foreign redundant witness and any
missing exact leader fact. Partial redundancy elimination and cyclic-CFG GVN
remain separate future rules; current admitted optimization units reject
control cycles.

The compatible-policy phi rule applies the same directional equality at an
acyclic join. The redundant exact add/subtract/multiply/left-shift/right-shift
must reference at least one typed join parameter and retain its own active
verifier-accepted obligation. Every incoming translation must find the
canonical available obligation-free compatible leader before the source
terminator or in a strict dominator: wrapping or saturating for arithmetic and
wrapping for the same shift direction. Arm policies may differ because the
redundant exact operation's proof certifies their common exact value.
Add/multiply canonicalize swapped operands; subtraction and shifts retain
order. The existing phi rewrite appends the redundant result as a join
parameter and every leader binding, removes the computation without global
substitution, moves custody forward, and consumes only the redundant active
reference. Independent validation reconstructs the compatible relation,
complete translated incoming set, canonical leader choice, proof fact, edge
bindings, provenance/fuel, and immutable accepted catalog. Division/remainder
and the reverse obligation-free-to-exact relation remain excluded.

### Proof-certified dead scalar work

The first exact `ProofCheckElision` rule is
`dead-unused-proof-certified-scalar-elimination.v1`. It admits only exact
integer cast, exact shifts, exact add/subtract/multiply/divide/remainder, and
wrapping or saturating divide/remainder. These operations are pure but carry an
operation obligation, so apparent result liveness alone never authorizes their
removal.

Every candidate instead carries the exact `AcceptedObligationFactIdentity` in
a dedicated proof witness. Candidate construction fails closed when the active
operation-reference fact has no matching accepted row. The independent
validator duplicates the closed operation vocabulary, reconstructs its
obligation and result type, and requires the accepted row to match identity,
machine, operation, and obligation. The decision manifest records that row as
the transformation's consumed fact.

Deletion then uses the same co-executed provenance/fuel relocation and metadata
rebuild as obligation-free dead scalar work. The active
`OperationObligationReference` leaves the function fact index with its removed
owner, but the verifier-owned accepted-obligation catalog remains byte-for-byte
intact as historical proof custody. A verified artifact carries the exact
accepted fact through projection after removing an unused exact add. The same
selection also owns
`live-proof-certified-integer-identity-elimination.v1`. It rewrites a live exact
result to the non-identity operand only for add zero on either side, subtract
zero on the right, multiply one on either side, and left/right shift by zero.
The candidate carries both the exact literal constant fact and accepted
obligation fact. Independent replay reconstructs the operation, identity kind,
integer types, literal definition and fact, active obligation reference,
accepted row, substitution, accounting, output, provenance, and fuel. The
active obligation reference is removed with the operation while the accepted
catalog remains immutable history. The third rule,
`live-proof-certified-integer-divide-by-one-elimination.v1`, admits only
`x / 1 -> x` for exact, wrapping, and saturating integer division. It requires
the direct typed literal-one fact and the exact accepted-obligation identity;
absence declines the candidate, while mismatched or corrupted evidence fails
independent validation. The fourth rule,
`live-proof-certified-exact-integer-multiply-by-zero-elimination.v1`, rewrites
`0 * x` or `x * 0` to the existing direct typed zero operand for exact fixed-
width signed/unsigned multiplication only, with canonical left selection for
`0 * 0`. Wrapping/saturating multiplication is obligation-free and deliberately
not hidden under `ProofCheckElision`; float multiplication remains outside this
exact law. The fifth rule,
`live-proof-certified-integer-zero-dividend-elimination.v1`, handles the closed
fixed-width signed/unsigned family `0 / x -> 0` and `0 % x -> 0` for exact,
wrapping, and saturating operations. The zero must be the direct typed left
literal and the operation must retain its exact accepted obligation; production
and replay consume both fact identities while retaining the historical accepted
catalog. The sixth rule,
`live-proof-certified-exact-integer-zero-value-shift-elimination.v1`, rewrites
exact fixed-width signed/unsigned `0 << count` and `0 >> count` to the existing
direct typed zero operand. The count need not be literal, but the operation must
retain the verifier-accepted obligation proving that its authored shift is
defined. Wrapping shifts are obligation-free and remain outside this proof-
bearing family. The seventh rule,
`live-proof-certified-exact-integer-self-subtract-elimination.v1`, replaces a
live signed/unsigned exact `x - x` node in place with typed zero. It preserves
the result `ValueId`, source location, operation/provenance/fuel custody, and an
empty substitution set. The eighth rule,
`live-proof-certified-integer-self-remainder-elimination.v1`, uses the same
in-place contract for signed/unsigned exact, wrapping, and saturating `x % x`.
In both cases the exact accepted obligation proves the authored operation
defined; the active operation-reference fact is removed, the historical
accepted catalog remains immutable, and independent replay reconstructs the
new literal-zero fact. The ninth rule,
`live-proof-certified-integer-self-divide-elimination.v1`, uses the same
in-place custody contract to replace fixed-width signed/unsigned exact,
wrapping, or saturating `x / x` with typed one. Its exact accepted obligation
proves the authored divisor nonzero. Signed one-bit and address carriers decline
because positive one is not representable in those domains. Independent replay
reconstructs the operands, policy, representable-one domain, fact, constant,
accounting, and output while removing only the active operation reference.
The tenth rule,
`live-proof-certified-integer-remainder-by-one-elimination.v1`, replaces live
fixed-width signed/unsigned exact, wrapping, or saturating `x % 1` with typed
zero at the same operation site. It requires the right operand's direct typed
literal-one fact as well as the exact verifier-accepted operation obligation;
a propagated-one analysis result cannot authorize the rewrite. The result
`ValueId`, operation identity/location, provenance, fuel, and historical
accepted catalog remain intact, while the active operation reference becomes a
literal-zero fact supported by that original operation. The rule explicitly
declines `left == right`, preserving the earlier self-remainder rule's ownership
of `1 % 1`. Independent replay reconstructs the operator policy, direct literal
definition and fact, accepted obligation, live/observation boundary, constant,
accounting, provenance/fuel, and output. Verified projection and x86-64/AArch64
lowering retain the resulting zero.
The eleventh rule,
`live-proof-certified-signed-integer-remainder-by-negative-one-elimination.v1`,
replaces live fixed-width signed exact, wrapping, or saturating `x % -1` with
typed zero. It requires the right operand's direct typed negative-one fact and
the exact active verifier-accepted obligation, excludes unsigned and address
carriers, and declines `left == right` so the established self-remainder rule
keeps ownership of `-1 % -1`. Independent replay reconstructs the type,
operator policy, operands, facts, observation/liveness boundary, provenance,
fuel, accounting, and output. Verified projection and x86-64/AArch64 lowering
retain the typed-zero realization.
Candidate schema remains v24. Optimization-unit identity v16 intentionally
rekeys revision-bound candidate identities because ordered edge cleanup and
compressed hidden-operation custody now join the already retained root service
reach, payloadless-call surface, verified machine contracts, and evidence-
contract lane rosters as input-revision content; deterministic tie breaks
between otherwise equivalent candidates can therefore change at this schema
migration.
The named v11 pass, prephysical manifest identity v26, and optimized-plan
projection validation v27 bind the expanded eleven-rule schedule.
Ledger v4 already represents the relocation. Runtime
policy events, other live proof-bearing identities, and physical checks not
represented by these exact Psi contracts remain open.

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

1. CFG cleanup: constant branch folding first, then observation-supported
   unreachable blocks, jump threading, empty-block elimination, and unreachable
   private-machine removal.
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
general selector. `omega-terminal-legalized-operations` is a data-only,
target-bound representation below raw target operations. A mandatory checked
canonicalizer reconstructs one exact V5 projection from the target plan,
optimized abstract plan, and verified optimization unit. Its canonical identity
binds Terminal-Psi, optimization-unit and fuel-schedule roots, exact native
target, entry/function roster, attachments, one closed recipe, source
blocks/values/definition sites, target provenance, branch/return edges and
bindings, accepted exact-operation facts, and every operation/edge fuel
settlement. Identity recipes report zero decompositions; the first non-identity
recipe reports its independently replayed occurrence count.

A separate plan-driven replay validates that projection without calling the
canonicalizer, importing its producer helpers, or constructing a second
expected plan. It checks the proposed fields directly against the raw target,
abstract-plan, and verified optimization-unit custody. A domain-separated
validator identity is retained by the legalized receipt, selected receipt, and
staged selection custody, so a legal-plan content identity cannot be detached
from the implementation that independently admitted it. Architecture tests
enforce the producer/replay dependency boundary.

Instruction selection accepts only that opaque legal carrier, so its public
producer and validator cannot freely recombine raw target, abstract, and unit
inputs. Selection constraints are derived from legal ABI source locations,
the selected receipt binds the legal-plan identity, and orchestration replays
and retains the same identity through later liveness/effects custody. The
existing `omega-terminal-target-operations-to-selected-instructions` pipeline
currently owns both checked legalization and selection mechanics, but the type
boundary is mandatory and explicit.

The closed V5 has seven exact three-block runtime conditional forms plus one
separate zero-VReg Unit-return form. The first conditional form has leaves that
materialize unsigned 64-bit constants and return. The second
carries a shared unsigned 64-bit entry parameter across both branch edges and
returns it directly, exposing genuine virtual interference and different
entry/return fixed sites without inventing a move. The third and fourth give
each leaf two unsigned 64-bit constants and a verifier-admitted exact addition
or subtraction. Their selected semantic kinds retain the exact obligation and
accepted-fact identity. Addition uses the target-owned flag-neutral
three-address `add_i64` row. AArch64 subtraction uses flag-transparent `SUB`;
x86-64 retains the honest RFLAGS clobber of its alias-safe subtraction pseudo.
The fifth and sixth accept two immediate unsigned-u8 exact additions or exact
subtractions followed by u8-to-u64 widening. Closed theorems record that zero-
extension commutes with the overflow- or underflow-proven unsigned operation;
independent replay checks the concrete arithmetic instance as well as the
original u8 accepted-fact owner. Subtraction preserves the authored left/right
order rather than applying addition's commutative reasoning. Each arm's narrow
operation and widen become one selected i64 operation with ordered two-
operation/two-fuel custody. Its promoted operands receive dense function-local
legalization-temporary identities, and selected virtual-register origins
preserve those identities instead of attributing a u64 value directly to a u8
Psi definition.
The seventh is the heterogeneous active-resident pressure recipe described
above: one leaf retains three literal definitions and three proof-bearing exact
adds in dependency order, while the sibling leaf returns one immediate. It is
an identity legalization whose purpose is to make the already validated
multi-use allocation-recovery policy reachable from ordinary selected custody;
it adds no selected instruction kind or target constraint. The selected-plan
identity advances to v7 because its closed recipe vocabulary changes.
The receipt therefore reports two non-identity legalization groups for either
two-arm recipe. Source-derived virtual registers retain their exact Psi value
and definition site; legalized temporaries retain their function-local identity
plus exact source lineage and definition site. Each
instruction retains its catalog constraint, explicit and implicit state
footprint, and semantic provenance; branch-edge fuel remains attached to the
corresponding selected successor so only the taken edge is charged. ISA-owned
orchestration injects the exact constraint keys and ABI live-in views instead
of asking a target-neutral stage to infer them from names or coincident numeric
variants. The opaque staged carrier also owns the final optimized unit,
independent abstract projection, target plan, legal plan, and validated
register environment. This is allocator input only: it grants no physical-home,
emission, or publication authority and fails closed for every other source
shape. A nested liveness carrier may consume it, but cannot weaken or detach
that custody.

This is not completion of target legalization. The general representation must
become CFG-shaped, apply the new non-Psi temporary identity model across a
general legalized value/operation vocabulary, record an exact source-
occurrence-to-legal-program-point expansion map, and partition proof/effect/
provenance/fuel custody across general one-to-many recipes. The legality profile
must eventually bind target profile, ABI, ISA feature/capability, and applicable
semantic catalogs rather than only `NativeTarget`. A plain constant widen would
only be a many-to-one canonicalization, while a direct narrow return would
silently choose ABI extension semantics and therefore remains unsupported.
This mandatory normalization is not a named build optimization; optional target
combines remain explicit selections.

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
The current slice admits distinct earlier-Use-to-later-Def ties in the same
instruction as canonical edge rows through liveness and block-domain ranges.
Any number of edges may form transitive same-home components: multiple Defs may
target one Use, and a VReg may participate in later ties, admitting both chains
and forks. `MultipleSingleDistinctDefAgainstUsesEarlyClobberV1` permits any
number of disjoint instruction rows, each with exactly one early Def and one or
more distinct Uses, excluding UseDef, other Defs, and repeated participants.
The separately closed `SingleEarlyDefTiedComponentAgainstUntiedUsesV1` form
permits the early Def to be tied directly to exactly one earlier distinct same-
class Use while that edge participates in a larger ordinary transitive chain or
fork. The component contains exactly one tied early Def/row, and at least one
unrelated same-instruction Use remains outside all tied components. Multiple
such rows are valid only in disjoint components. A second early row in one
component, a tied hazard Use, UseDef, additional Def, repeated participant, or
cross-class tie rejects. Spills, loops, calls, crashes, cleanup, suspension, and
other compositions remain refused until they have explicit selected-IR
frontiers and dedicated validation rules.

A two-machine disconnected fixture exercises the complete current vertical on
x86-64 and AArch64. Selection intentionally restarts dense block and VReg IDs
per function, liveness restarts dense positions, ranges remain block-domain and
machine local, and the same-shaped functions receive the same deterministic
local homes without cross-function interference. The fixture proceeds through
legality and post-allocation machine reconstruction. Independent liveness,
range, legality, home, and machine replays each reject a second-function machine
identity substitution even though its local numeric IDs match the first.

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

Early clobber is not modeled by moving the Def's semantic live range backward.
The Def still begins at the after point, while a separate canonical phase-
hazard row binds its before point and every unrelated same-instruction Use. In
the admitted `SingleEarlyDefTiedComponentAgainstUntiedUsesV1` form, the early
Def is tied directly to one earlier distinct same-class Use, and that edge may
belong to a larger ordinary transitive tie chain or fork. Exactly one tied early
Def/row may inhabit the component. At least one unrelated same-instruction Use
must remain outside every tied component; only those unrelated Uses enter the
phase-hazard list, while the tied source remains represented by its ordinary
tie row. Multiple rows are admitted only in disjoint tied components. Legality
derives an additional before-phase Def candidate row under the same fixed-view,
reservation, architectural-state, and availability checks. Home assignment and
its independent replay then require the selected Def view's complete write
footprint to be disjoint from every Use view, including a dying Use already
expired from the ordinary active set. Spill-choice rejects this topology until
it has an equally phase-aware recovery strategy.

Before allocation, a separate legality artifact joins those ranges to the
identity-bound register environment. For every occupied VReg point it publishes
the exact class-compatible views whose storage and canonical-write footprints
avoid the active reservation union, architectural semantic liveness, and
same-phase architectural actions. A fixed operand may name a view that is not a
general allocation candidate, but it must still have the right class and avoid
reserved or architectural units. Production and replay independently derive
the exact nonempty, sorted candidate rows.

For each admitted tied component, home assignment takes the union envelope,
intersects every member's ordinary and early-clobber point candidates, chooses
the lowest stable common view, and assigns that view to every member. Production
uses union-find while independent replay uses separately coded set merging.
Every unordered component member pair must be noninterfering. Replay rejects
component interference, disjoint candidates, malformed topology, or unequal
homes. Spill-choice fails closed while any tied component is present rather
than separating tied values without a proved copy/storage strategy. The
complete transitive component shares one home, while the early Def's complete
write footprint remains disjoint from every unrelated Use home. Production
uses graph closure and union-find; independent replay separately merges the
same components and rejects a second early member, a tied hazard Use,
interference, disjoint candidate sets, or write/storage aliasing. Liveness and
live-range identities are v7; the strict legality/home identities and home
codec remain unchanged because their schemas did not change and already bind
the new v7 roots.

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
all nine currently admitted selected semantics and bind each to its exact
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
same analysis over five separately validated selected forms: original
selection, fixed-view-copy output, the final result of an explicitly invoked
literal-fold sequence, production active-resident multi-use rematerialization,
and a completed named selected-lowering suite. Its custody receipt names which
form was used and retains that form's independently replayed receipt, so a
transformed CFG cannot inherit the source CFG's machine facts by shape or
convention. The rematerialization route replays the complete one-step policy
carrier before analyzing its transformed selected CFG. The named-suite route
remains meaningful when its independently validated result contains zero
rewrites.

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
contract; the strict pre-allocation codec is v4 after exact-subtract immediate
selection joined the encoded-realization vocabulary.

The post-allocation sidecar crosses a durable boundary through a distinct
strict v1 codec. Its framed content binds all joined roots, the named choice
rule, the complete selected roster, chosen alternatives, physical views and
access-qualified footprints, and canonical unit actions. The decoder rejects
unknown closed-vocabulary tags, invalid machine identities, truncation,
trailing bytes, and stale content identities. It returns only a plain unchecked
plan: content authentication proves which bytes were stored, not that their
machine claims are legal. Orchestration therefore independently reconstructs
decoded plans against selected instructions, effects, allocation facts, homes,
the register model, and target catalogs on both x86-64 and AArch64. Even a
tampered plan with a freshly recomputed content identity is rejected by replay.

Orchestration reconstructs this sidecar for ordinary selected homes,
fixed-view-copy output, an explicit literal-fold sequence, production active-
resident rematerialization, and named selected-lowering completion, always
retaining the matching transformed or verified-no-change custody and validated
post-allocation manifest. The rematerialization path consumes only the
transformed selected plan and its fresh ranges, legality, homes, and typed v5
manifest while retaining the original target/environment/catalog/unit/fuel
roots. Both machine-layer source receipts contain the exact full-vertical
rematerialization receipt, and independent replay rejects upstream corruption,
original-selected detachment, and cross-source substitution on both ISAs.
Machine-effect v4 and post-allocation-machine v1 need no schema change because
their identities already cover every transformed root and row. This still
grants no encoding, layout, emission, or publication authority.

The first post-allocation transformation is the exact, default-off
`Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1` rule in the closed
`PostAllocationMachine` phase. It accepts only adjacent selected
`CompareI64Zero` and `ConditionalBranchNonZero` variant-zero forms after homes
are fixed. The compare must carry no logical fuel; its source must resolve to a
canonical allocatable 64-bit X view; NZCV must flow from compare to branch and
independent liveness must prove it dead at the branch output, block output, and
both successors. The bounded v1 symbolic artifact retains the source machine,
selected and liveness roots, exact target/model, ordered attempts and actions,
source-qualified physical read, compare elision, fused-branch disposition,
both successor identities, work usage, and revision history. A strict `OMGCNZ`
v1 codec authenticates that complete content, while a separately implemented
validator reconstructs it from the retained inputs.

The symbolic rule owns no displacement, byte layout, or emission authority.
The AArch64 ISA owner separately encodes and independently decodes the exact
64-bit `CBNZ` imm19 form, reporting the qualified X-register read and PC
use/definition without inventing an NZCV dependency. Direct and selected-
lowering physical-pipeline routes now return owning function-relative
realization carriers instead of symbolic sidecars. Each carrier retains homes,
selected machine, symbolic fusion, baseline and fused encoding/layout roots,
and CBNZ-aware whole-function exit custody. The compare remains in the
instruction roster as a zero-byte row; the fused branch is target-encoded and
independently decoded as `CBNZ`; source-register, successor, PC, and absent-NZCV
effects are replayed; and the function shrinks by exactly four bytes. The
function-relative manifest/codec v6 binds the exact post-allocation selection,
an explicit allocation-recovery phase projection,
baseline/final pre-layout identities, optional fusion identity, and both
resolved layouts; whole-function exit identity v4 independently admits only
the exact elided-compare custody. Emission, relocation, image, installation,
and publication remain later boundaries.

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
size, decoded footprint, and encoded-realization effects under its v3 identity.
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

The production active-resident vertical has a separate owning pre-layout
carrier around that generic artifact. It consumes the full rematerialization
carrier and its rematerialization-sourced post-allocation machine plan by value,
replays both source-specific receipts, derives the transformed selected plan
and physical model only from retained custody, and independently reconstructs
the generic encoding. Its in-process receipt binds both upstream receipts, the
transformed selected root, unchanged v3 encoding identity, and exact encoded
and deferred row counts. Both ISA tests locate the fresh rematerialization
instruction and require its scalar bytes; corrupt source custody, cross-source
machine substitution, and byte mutation fail closed. This advances no resolved
layout, frame, section, emission, or publication authority and adds no durable
schema.

A sibling owning carrier advances that exact route through required resolved
layout. It first replays the entire rematerialization pre-layout wrapper, then
derives the transformed selected plan, source-specific post-allocation machine,
and physical model only from retained custody before invoking the generic
`EntryThenZeroFallthroughThenNonzeroV1` resolver. Independent validation repeats
the same full upstream replay before reconstructing offsets, branch bytes, and
effects. Its receipt binds the upstream custody, selected, machine, pre-layout,
physical-model, and resolved-layout roots, target, named policy, and exact
function/block/instruction/byte/branch counts. Both ISA paths retain the fresh
rematerialization row and resolve one branch; corruption at pre-layout bytes,
resolved bytes, or receipt custody fails at its own layer. The generic layout
identity remains v2 and no durable schema changes. Branch relaxation,
whole-function exit, realization manifests, frame, emission, sections, objects,
images, installation, and publication remain unavailable.

One carrier now closes the next function-relative custody join for this route.
It owns the resolved wrapper, validates the generic `BaselineNearLayoutV1`
frameless exit, and independently reconstructs the v5 realization manifest.
Direct staging preserves the original source-visible selection and keeps every
unexecuted phase projection empty. Compiler routing instead requires the exact
`ActiveResidentImmediateU64MultiUseRematerializationV1` selection in the
allocation-recovery projection; selected-lowering completion and the post-
allocation-machine and function-relative-layout projections remain absent. The
v5
post-allocation manifest truthfully carries the sole typed
`PressureRematerialization` row and transformed selected root. Production and
replay derive every exit, layout, machine, manifest, and statistic field only
after replaying the full upstream wrapper. Both targets use caller-saved
pressure views (`rax`/`rcx` and `x0`/`x1`) rather than weakening callee-save
validation. Whole-function-exit v3 remains sufficient, while realization-
manifest v5 makes allocation-recovery execution replayable. The compiler
physical pipeline accepts only the exact singleton family and rejects
unsupported physical-phase composition. Frame, emission, sections, objects,
images, installation, and publication remain unavailable.

A separate immutable v2 artifact resolves those branch rows only after choosing
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

Function-relative orchestration seals those products in a structured v5
realization manifest. Separate custody routes admit ordinary homes when only a
function-relative family is selected, selected-lowering homes when that phase
also ran, and the active-resident rematerialization carrier for its exact
allocation-recovery family. The record joins the full named build suite, every
exact phase projection, optional selected-lowering completion, pre-physical and
post-allocation manifests, final selected CFG, pre- and post-allocation machine
roots, pre-layout encoding, baseline and final layouts, optional layout-
transformation receipt, exact target, named layout policy, and a validated
whole-function exit-contract identity. It derives function, block, instruction,
byte, and resolved-conditional-branch counts from the final validated layout. A
strict binary codec recomputes its domain-separated identity and rejects old
versions, unknown vocabulary, identity changes, truncation, and trailing bytes;
custody replay reconstructs every joined root and both encoded artifacts.

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
whole-function exit discipline. Frame construction, section placement, symbols,
object relocations, executable image, installation, and publication remain
explicitly unavailable. This is an honest realization checkpoint, not an
object or native-final manifest.

Choosing x86 `rel8` is the separate named transformation
`X86RelaxConditionalBranchesToRel8V1`; it is not an implicit “higher
optimization level” behavior of the baseline encoder. The target owner exposes
a distinct canonical two-byte `JNE rel8` encoder and validator with exact signed
byte bounds and the same declared control/effect footprint as the required
near form. The optimizer's immutable result binds the required near-layout
source, the rewritten layout, exact budget and usage, ordered scan attempts,
each four-byte shrink, and domain-separated revision and artifact identities.
Production deterministically commits at most one branch per iteration and
stops only after a complete no-change sweep. Independent replay reconstructs
the scan, dense offsets, both successor offsets, displacements, re-encodings,
work accounting, and the terminal fixed point. The rule rejects non-x86 targets
and never widens a branch.

The exact rule name is present in the v3 build vocabulary under the separate
function-relative-layout phase and remains default-off. Verified physical
orchestration projects that phase separately from selected lowering. A
branch-only suite takes frameless legality through ordinary register homes and
records an absent selected-lowering completion; a combined suite retains both
positive completion receipts. The strict v5 realization manifest binds the
full suite, all phase projections, baseline and final layout identities, the
optional relaxation identity, final statistics, and the final-layout exit
contract. Whole-function exit identity v2 names either the required near-layout
custody or the exact independently replayed relaxation receipt. Baseline exit
validation remains strict and cannot admit short bytes by accident.

A later relocation-free boundary now consumes completed x86 rel8, AArch64 CBNZ,
or active-resident rematerialization realization carriers and materializes dense
bytes independently per function. Its immutable function-fragment artifact
retains the function, block, and instruction spans—including the CBNZ route's
zero-byte compare and the rematerializer's fresh nonempty `MaterializeI64`—plus
the chosen alternatives, decoded branch evidence, full Psi/selected provenance,
successor bindings, and path-specific fuel settlements. The architecture-
neutral rematerialization source kind replays the complete owning carrier,
requires its exact singleton allocation-recovery projection, and retains the
baseline `JNE rel32` or `B.NE imm19` branch rather than claiming rel8 or CBNZ.
It never concatenates functions into a globally placed section. Production and
an independently coded replay reconstruct source custody, offsets, row bytes,
aggregate bytes, statistics, receipt, and the content identity. The strict
`OMGFFE` v5 manifest binds the generalized five-kind source vocabulary, including
Unit baseline and structural-Unit custody, source realization, and every selected, post-allocation,
layout, exit, target, and fragment root while explicitly marking section
placement, symbols, object
relocations, executable image, installation, and publication unavailable.

The following required boundary places those fragments into one immutable
relocation-free text section under
`DenseValidatedFragmentOrderNoPaddingV1`. It preserves validated source
function order, inserts no padding, uses section alignment one for x86-64 and
four for AArch64, and retains both function-relative and section-relative
coordinates for every function, block, and instruction—including zero-byte
CBNZ compare spans. The semantic entry machine and section offset remain
custody facts, not an invented `main`/`_main` process symbol. An exhaustive
classification of the current alternative families proves scalar/fallthrough/
return forms contain no address-bearing target and every relative conditional
branch is already resolved to blocks in its owned function. Production and a
separately coded replay reconstruct the byte concatenation, coordinates,
alignment, entry, statistics, and closed no-relocation conclusion. The strict
`OMGTSP` v5 manifest binds the generalized five-kind source vocabulary and all upstream
roots while marking symbols, object container and serialization, external entry
bridge, executable image,
installation, and publication unavailable.

The next object-owned boundary consumes that validated text section by value.
`ValidatedRelocationFreeTerminalObjectContainerV1` creates exactly one private
object-local symbol per placed function in source order, with canonical one-
based symbol IDs and `MachineId`-rooted names. Dense symbol intervals cover the
text bytes exactly. Precisely one symbol carries the semantic-entry role, even
when that function is second and begins at a nonzero offset; this is not a
`main`/`_main` process-entry claim. The strict `OMGTRO` v1 container and
`OMGTOM` v1 manifest bind the canonical target, source text-section identity,
symbol roster, semantic entry, exact bytes, and independently replayed zero-
relocation conclusion. This clean sibling does not acquire authority from the
legacy object plan. External/process entry bridging, native image construction,
installation, and publication remain unavailable.

A following canonical semantic/proof-to-object boundary owns both the verified
Terminal artifact and clean object carrier by value.
`ValidatedOptimizedTerminalObjectArtifactV1` requires exact decoded module and
proof-bundle equality, then binds the Terminal artifact, Psi, obligation and
proof roots, optional debug fingerprint, explicit selections, target, semantic
entry, every pre-physical through object manifest, canonical object/container
roots, exact statistics, and the zero-relocation result. Production and an
independently coded replay reconstruct the retained custody through different
access paths. The strict `OMGOTA` and `OMGOTM` v1 codecs provide the canonical
artifact and rebuild-metadata records. They still mark external entry bridging,
native image construction, installation, and publication unavailable.

The following boundary is
`ValidatedOptimizedTerminalOrdinaryCallableEntryV1`. It consumes that artifact
by value and classifies the current exact unattached, frameless scalar semantic
entry as an ordinary target-ABI callable. For each semantic scalar parameter,
its identity binds the ValueId and ABI shape to the selected VReg, fixed
assigned physical view, canonical storage units, and target ABI register.
Terminal scalar results are pseudo-result declarations, not one universal
selected value: the record therefore binds the result declaration separately
from ordered per-return-edge rows naming each actual returned ValueId, selected
VReg, assigned view, and units. It also retains the selected plan, homes,
physical model, private semantic-entry symbol and span, object/container roots,
whole-function exit contract, target-native calling policy, hardening, stack
alignment, red-zone facts, and caller-created return-state assumption.
Production and an independently coded validator recompute the call plan and
every Terminal/selection/home/symbol/exit join. Its explicit disposition is
`ExternalProcessEntryBridgeRequiredV1`; strict `OMGOER`/`OMGOEM` v1 records
grant no process symbol, wrapper bytes, relocations, image, installation, or
publication. The cumulative report gains this record only from the opaque
carrier that owns the source artifact.

The compiler-selected active-resident route now proves that these generic
downstream joins accept a newly transformed allocation source without a new
schema or route-specific shortcut. Tests cover System V AMD64, Microsoft x64,
Linux AAPCS64, and Darwin AAPCS64 from the exact singleton build selection.
They retain the fresh nonempty `MaterializeI64` section span, transformed
selected root and homes, sole `PressureRematerialization` ledger row, Terminal
semantic/proof roots, private entry symbol, both return-edge result rows, and
target ABI parameter/result registers through independent replay. The
route-specific source tag remains authenticated by `OMGFFE`/`OMGTSP` v5;
`OMGTRO`/`OMGTOM` and `OMGOTA`/`OMGOTM` remain v1 because their generic
child-identity vocabularies did not change. `OMGOER`/`OMGOEM` advance to v3
because their closed exit-policy codec now names the structural leaf. Artifact reporting
still omits callable metadata until the opaque callable carrier owns the
artifact. The final disposition remains
`ExternalProcessEntryBridgeRequiredV1`.

The clean backend now has an exact Unit-return vocabulary alongside the scalar
callable vocabulary. Its original baseline roster admits the receiver-free,
zero-VReg, one-block shape whose sole terminator is `ReturnUnit`; the distinct
v6 structural roster described below does not weaken that shape. Selection of
the baseline uses distinct Unit semantic, constraint, and
alternative identities; it does not encode Unit as a scalar return with a fake
value. x86-64 owns canonical `C3` encoding/effects and AArch64 owns canonical
`RET X30` encoding/effects. Because this changes closed replay vocabularies,
the affected legalized, selected, effect, encoding, layout, fragment, text,
and relaxation schemas advance and old records fail closed. Whole-function
exit v4 distinguishes `UnitV1` from scalar-return evidence; resolved-layout v4
owns `SingleEntryBlockV1`; function-relative v6 and fragment/text v4 bind the
new route. A dedicated baseline carrier now proves the exact one-function Unit
shape through zero-VReg liveness, ranges, legality, empty homes,
post-allocation-machine replay, encoding/layout, Unit exit evidence,
relocation-free fragments and text, private-symbol object serialization, and
the canonical Terminal semantic/proof-to-object artifact on x86-64 and
AArch64. It grants no ProgramStorage wrapper, process-entry, image,
installation, or publication authority.

The source-side join is no longer reduced to an entry machine name.
`CheckedCompilation` retains the complete `SelectedCompilerProgramEntry`, and
native realization accepts a coupled request containing the exact target,
admission profile, selections, provider settlements, source entry signature,
and optional paired semantic/physical calling plans. Validation rejects target
or pairing drift before lowering. A domain-separated
`ProgramEntrySourceSignatureIdentity` binds the normalized callable, complete
target slot, receiver form, ordered visible roles and indices, normalized type
identities, exact value/Extent layouts and modes, and Unit result. Arena symbol
handles are excluded because they are replay coordinates rather than stable
semantic identity. This is declaration-only custody: it cannot authorize
runtime roots, bootstrap conversion, image construction, or publication.

Receipt-coupled checked-to-Terminal production now preserves the selected
source signature's opaque identity and checked symbol/name beside the canonical
Terminal-Psi identity and unique Unit entry `MachineId`. Native orchestration
independently replays the canonical artifact and rejoins the complete source
signature, target, and paired calling plans before producing an owned
settlement. Source, target, Terminal-Psi, and entry substitution all fail
closed. This receipt is association evidence, not entry-bridge authority.

The settled semantic declaration layer is also concrete in
`omega-program-storage`. Its optimized data-only contract admits only the
receiver-free UEFI x86-64 ProgramStorage slot; Image then InitialStorage with
exact `Extent in Granted` source shape, role, and carry; Unit/no result; and the
Microsoft x64 semantic call plan/fingerprint. It retains the separately
validated physical plan only as `PlannedNotInvokedV1`. It deliberately contains
no Terminal `MachineId`, wrapper bytes, runtime values, bootstrap, image,
installation, or publication authority.

The next semantic layer is also concrete and deliberately address-free. The
pure wrapper plan derives only from that contract and independently replays the
exact Microsoft-x64 action sequence: preserve the indirect `RCX`/`RDX` roots,
reserve a balanced 72-byte outgoing frame, copy the four Extent words into its
ABI copy area, bind the copied Image and InitialStorage addresses back to
`RCX`/`RDX`, call one unresolved compiler-private continuation, release the
frame, and return Unit. It retains a symbolic function-relative `rel32`
requirement and `PlannedNotInvokedV1`, but no `MachineId`, symbol, bytes,
runtime values, bootstrap, process-entry, image, installation, or publication
authority.

The entry path separates a design-settled semantic wrapper from an unsettled
physical bootstrap. The distinct receiver-free, straight-line Unit selected
shape, complete zero-VReg-to-object artifact route, retained target-owned
`ProgramEntry` signature/calling plans, checked-source-to-Terminal entry
settlement, declaration-only semantic ProgramStorage contract, and address-free
wrapper plan now exist. Legalized-plan and independent-replay v6 add a distinct
structural Unit roster rather than weakening the original zero-VReg roster. It
admits exactly a structural `ReturnUnit`, or one whole-root `CallUnit` followed
by `ReturnUnit`, and retains the structural type closure, paired semantic and
target parameter/argument declarations, native call plans, places, claims,
service ceiling, provenance, fuel, effects, and ownership events. Replay
recomputes both caller and callee call plans and resolves the raw roster by
unique `MachineId`. Any placement, ordering, plan, effect, ownership, cleanup,
or roster drift fails closed.

This is the generic claim-preserving backend vocabulary needed by the wrapper,
not the ProgramStorage wrapper object itself. On Microsoft x64 COFF only, the
validated register environment now exposes an explicit structural-call key and
selected-plan v9 lowers the bounded form to one atomic zero-VReg call pseudo
plus `ReturnUnit`. Independent replay retains the exact `RCX`/`RDX` indirect
inputs, caller-copy offsets 32/48, balanced 72-byte frame, internal callee,
caller-saved clobbers, claim transfers, effects, ownership, and provenance.
Effect-catalog v4 declares this pseudo separately from ordinary encoded
alternatives. Preallocation-effect v5/`OMGMFX` v6 retains a parallel structural
machine-effect roster, and liveness v8 retains its architectural unit uses,
defs, and clobbers without fabricating VRegs. Live-range v8 retains exact block
domains and architectural actions while proving the structural roster has no
virtual ranges, ties, early clobbers, or interference. Allocation-legality v5
and register-home v6 retain exact empty structural rows. Postallocation-
manifest v6 reports structural and ordinary functions separately, and
postallocation-machine v4/`OMGPMX` v3 rejoins the exact call effects and
selected `ReturnUnit` alternative under empty-home and MachineId custody.

The target-owned x86 encoder produces and independently decodes the canonical
89-byte call template, including root reads, four caller-copy writes, argument
pointer rebinding, balanced frame, flags, scratch register, fault, and call
effects. Its zero `E8` displacement is explicitly owned by a typed unresolved
internal-Machine fixup at opcode offset 80/field 81/next-IP 85; it is not
executable-byte authority. Layout-independent selected-form encoding v5 now
retains a parallel structural-function roster with that exact template,
decoded footprint, typed fixup, separately encoded `C3` return, and exact
ordinary/structural counts under independent replay. Resolved-layout v5 retains
the exact caller call/return spans `[0, 89)`/`[89, 90)` and leaf return span
`[0, 1)` without resolving the fixup. Whole-function-exit v5 validates a
distinct balanced Microsoft-x64 structural-call policy, and a separate owning
function-relative carrier publishes `OMGFRM` v7 statistics with zero ordinary
rows, structural `2/2/3/91`, and one unresolved internal-Machine fixup.
Machine-code fragment schema v3 and `OMGFFE` v5 now retain that exact parallel
structural roster, 90/1-byte aggregate spans, call/return provenance, and a
target-neutral typed fixup. That call-bearing fixture remains explicitly
distinct from the relocation-free leaf stage. The target-owned x86 resolver
computes checked signed rel32 fields and independently replays the complete
patched call, including the canonical `+5` forward fixture and negative calls.
Two-pass whole-text placement now constructs the complete `MachineId` offset
map before resolving calls. Text-section schema v3 and `OMGTSP` v5 retain the
patched 91-byte section, caller/callee spans `0/90` and `90/1`, exact opcode,
field, next-IP and callee coordinates, the canonical `+5` displacement, and
zero remaining internal fixups. Independent replay reconstructs the target
template and resolution before granting relocation-free custody. The unchanged
object v1 path then emits two exact private symbols and zero relocation records.
That two-Machine object is a generic code-generation proof fixture, not the
ProgramStorage wrapper object: its caller is the Terminal module entry, every
function and symbol is `MachineId`-rooted, and object validation deliberately
forbids relabelling one of them as a compiler wrapper. The checked settlement
also binds the source entry to that same Terminal module entry, so the leaf in
the structural fixture cannot be substituted as the settled continuation.

The owning join has now landed in `omega-terminal-native-realization`, which
owns the settlement and depends on both `omega-program-storage` and the
optimization pipeline. Its opaque stage consumes the validated settlement, the
canonical optimized object artifact for the actual settled Terminal entry, and
the selected compact encoding by value. Independent replay reconstructs those
three inputs, the semantic contract, and the wrapper plan before synthesizing a
distinct compiler-owned composite object. The object places the wrapper before
a byte-for-byte retained copy of the canonical child text, shifts every child
symbol coordinate with checked arithmetic, classifies the child semantic-entry
`MachineId` symbol as the private Terminal continuation, and resolves the
wrapper call to that shifted object-local symbol. It retains the complete child
artifact and has zero relocations. Dedicated plan, container, and manifest
identities plus `OMGPSO`/`OMGPSM` v1 codecs make the join replayable. This is
intentionally not an unresolved cross-object call: an object-local continuation
is not a legal linkage target from a sibling object. The compiler-owned wrapper
symbol is distinct from every Terminal `MachineId`, and the manifest explicitly
withholds physical bridge, image, installation, and publication authority.

The encoding split is now explicit. The semantic wrapper plan owns no byte
coordinates and requires a later target encoding. Native realization consumes
that plan by value and projects all eleven semantic steps into the named compact
Microsoft-x64 caller-saved-only/no-control-state-mutation policy. The
target-owned encoder independently parses its complete 90-byte function,
retains root reads, copy writes, argument-register rewrites, caller-saved
clobbers, stack/IP/flags effects, balanced frame, fault/call/cleanup behavior,
and Unit return, and owns opcode/field/next-IP offsets `80/81/85`. Its resolver
checks the signed target equation and every byte outside the field. The older
compatibility emitter remains a distinct 143-byte full-frame encoding with
coordinates `113/114/118`; neither layout impersonates the other.

The one-function structural two-Extent `ReturnUnit` code-generation fixture has
landed. Whole-function
exit v6 independently admits exactly either the existing two-Machine
entry-caller-to-leaf graph or one call-free entry leaf, and assigns the latter
the separately named `MicrosoftX64FramelessStructuralUnitLeafV1` policy.
Generalized `OMGFFE` v5 structural fragments classify its zero-fixup output as
relocation-free; `OMGTSP` v5 places one `C3` with no resolution rows; the object
owns one semantic-entry private symbol and no relocations; and canonical
artifact replay rejoins the exact semantic and proof inputs. No fabricated
wrapper `MachineId` or parameterless signature substitution is involved. This
fixture proves the structural encoding/object route only. Its unrestricted
parameters and empty claim roster do not make it an honest ProgramStorage
continuation.

The wrapper identity remains distinct from every Terminal `MachineId`; only
the settled internal continuation is MachineId-rooted. The checked-source
ProgramStorage regression also establishes that the existing generic source
lowering preserves two linear owned `Extent in Granted` roots, two entry claims,
and completion receipts while expressing its handoff as `BoundaryCall`. It must
not be misrepresented as the positive compiler-private `CallUnit` wrapper
fixture, and the synthetic leaf must not be paired with that receipt. Provider
catalog construction and Terminal verification admit and exactly replay a
linear, owned, domain-qualified structural provider signature, its positional
and domain refinement, and its checked claim-consuming provider body. Structural
access is part of that canonical signature as of Terminal vocabulary 36; owned
and borrowed candidates are therefore not interchangeable catalog rows.

Provider selection produces an opaque Omega-owned installation carrier only
after replaying the exact artifact, abstract plan, full candidate conformance
row, structural signature, and claim correspondence. The Psi reference
interpreter uses that admission to execute the provider through the same
structural Unit-call transition as an authored call, including ownership
transfer and completion of both ProgramStorage roots. The canonical source
operation nevertheless remains `BoundaryCall`. Installation-aware target
lowering instead emits the distinct target-only `InstalledProviderCall`, which
retains the boundary occurrence, complete provider row, original and physical
arguments, caller completion sources and receipts, and their exact claim-
transfer interpretation. It rejects external-settlement overlap, partial
installation, and identity or structural substitution. This is not permission
to relabel the occurrence as source-authored `CallUnit`.

Production native realization derives this admission privately from the sealed
selected provider closure. It projects only checked adapters whose exact
requirements occur in the replayed Terminal catalog, rejects duplicate or
identity-drifted projections, and immediately consumes the rows through opaque
installation admission. An empty checked projection leaves ordinary external
provider realization unchanged. Once any checked adapter is selected, partial
catalog installation remains invalid. Optimized realization transfers the
owned admission into the physical pipeline rather than reconstructing it from
names; the compatibility lane uses installation-aware target projection but
still cannot silently emit the optimizer-only operation.

Legalization retains that distinction in `TerminalLegalizedCallUnitSource` and
independently replays the installed provider row, candidate ABI, physical
receipt-derived transfers, original `BoundaryCall`, and `ClaimCompletion`
ownership. Selected structural calls reuse the same source vocabulary and bind
it in selected identity v10 while reusing the ordinary direct-internal-call
physical recipe. An authored-call substitution therefore changes identity and
fails replay. The compatibility assigned-operations lane rejects
`InstalledProviderCall` explicitly; consumers that have not selected the
optimizer cannot acquire it accidentally.

Pure claim completion is a separate target mechanism. The provider body's
`Extent::settle` operations may use the named `ClaimCompletionOnly` realization
only when exact whole-root owned linear arguments, canonical entry-claim
sources, receipts, and admitted provider execution all replay. It owns no
scalar/result/byte payload and emits zero bytes, while legacy machine/object/
installation custody retains the complete settlement row. Installation format
41 binds the added realization tag. It is not a fake call, port effect, or
selected instruction.

Optimized legalization v8 and selected identity v11 retain these settlements as
ordered metadata rows. Independent derivation and replay reconstruct the exact
boundary declaration, whole-root owned linear ABI, canonical caller entry-
claim sources (including unrelated still-live sources), receipt set, admitted
provider execution, fuel, sequential effect links, and `ClaimCompletion`
ownership. The rows receive no selected instruction IDs; a settlement-only
provider body selects only its `ReturnUnit`, numbered 0.

The first honest checked-source ProgramStorage path now opts into the explicitly
named copy-propagation pass, admits the opaque installation, lowers the
installed call and both `ClaimCompletionOnly` settlements, and reaches
independently replayed optimized legalization and selection. It retains the
installation in the optimized-target carrier, both structural functions, the
installed-call `ClaimCompletion`, both ordered settlement rows, zero virtual
registers, and exactly three physical instructions (the call and two Unit
returns). This regression also binds the canonical qualification union from a
boundary parameter plus its `requires` domains and the full entry-claim source
roster at each settlement while each receipt completes only its selected claim.
The positive three-way object/wrapper join now reaches canonical custody. The
same honest checked continuation proceeds through liveness, empty allocation,
structural realization, resolved fragments/text, a 91-byte two-symbol
zero-relocation Terminal child object, and the canonical semantic/proof object
artifact while retaining the opaque installation by value. The semantic
wrapper join borrows that installation and independently matches its exact
candidate and installed occurrence to the selected root call, boundary,
operation, structural arguments, completion evidence, root claims, provider
callee, and the provider body's two ordered settlement rows. It then prefixes
the 90-byte compiler-owned wrapper and resolves its private continuation into a
181-byte three-symbol zero-relocation composite. The wrapper symbol has no
`MachineId`; both copied child symbols retain theirs. A diagnostic-only pure
replay now accepts the retained opaque installation, a borrowed selected plan,
and the semantic entry without returning any authority. The checked-source
matrix mutates cloned evidence and rejects provider/candidate identity,
installed/authored source kind, structural access and qualification/domain,
semantic call arguments, completion sources and receipts, entry claims,
callee/function rosters, and provider settlement claims/order/count while the
unmodified evidence continues through wrapper composition. Source-entry
identity substitution still rejects before composition. This is not a
language-design question.
The scalar-result conditional fixture above remains an ordinary callable. The
eventual semantic wrapper is not yet an authoritative firmware/process entry:
the UEFI surface is explicitly planned and non-invoked, and no target/runtime
contract maps `(EfiImageHandle, &EfiSystemTable)` into those two semantic
extents or maps Unit completion and failure into `EfiStatus`. Hosted targets
publish no physical entry contract yet. `OWNER_QUESTIONS.md` Q17 owns this
genuine design decision;
the semantic-wrapper slice may proceed while physical bridge, image,
installation, and publication authority remain closed.

The build vocabulary still does not grant native publication authority:
explicit selection currently fails closed without installing output. The
compiler gate can be removed only after the Unit semantic wrapper, the Q17
physical contract, native-image adaptation, independent final-image validation,
and selected-build publication tests all join the same custody chain.

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

The first external schema now records current Psi decision points without
consulting them. Its v1 context roots the source optimization unit, full and
Psi-phase selections, explicit target-neutral identity, ordered rule set, and
structural cost model. A point contains only its input revision, rule identity,
canonical candidate identities with signed cost deltas, and the selected member
of the finite candidate-or-skip action set. Those types cannot represent raw
paths, authored names, pointers, arena order, diagnostics, or debug strings.
The strict codec recomputes every point and log identity. Optimized-plan
projection independently derives the expected trace from the ordinary baseline
log and pass manifests and rejects a detached valid trace. The baseline log,
not this recording, remains in the optimization identity bundle, so recording
does not change policy or output.

Psi now also exposes a strict byte-boundary replay API without changing its
ordinary model-free entry points. Replay first decodes the v1 log and exactly
matches every context root before dispatching a rule. One ordered cursor spans
all selected Psi pass groups. For each nonempty decision point, the ordinary
rule proposes candidates and all normal candidate validators finish before the
cursor is consulted; only then may the supplied action select one member of the
exact canonical roster or the explicit skip. Missing, duplicate, foreign,
reordered, roster-mismatched, and leftover points fail closed. The chosen action
is recorded in the existing baseline decision-log format and follows the same
manifest, commit, fixed-point, analysis, and transformation-ledger path as an
ordinary choice. At completion the compiler independently reconstructs the
external log from the resulting baseline records and validated manifests and
requires exact equality with the supplied bytes; optimized-plan projection
performs a second reconstruction. Thus the replay mechanism can change
profitability policy but cannot bypass legality or acquire a separate artifact
identity. Other compiler phases still need their own schemas, and build-file
custody, workload inputs, training export, and offline search remain later
boundaries.

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

Named suites are only readable expansion helpers over exact transformation
names. They are not compiler modes, cannot carry hidden intensity semantics,
and have no identity apart from their canonical expanded selection and ordered
rule schedule. Build reports and cache/replay identities always expose that
expanded set.

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
    case X86RelaxConditionalBranchesToRel8V1;
    case SelectedIncomingU12ExactSubtractImmediate;
    case Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1;
    case SharedEntryFixedViewCopyAfterCompareBeforeBranchV1;
    case ActiveResidentImmediateU64MultiUseRematerializationV1;
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
The canonical `OMGOPT` selection codec and domain are v7 after adding the
twelfth exact family; the version change prevents an older vocabulary from
silently interpreting the new build request.

Each named transformation also has one closed execution phase. Phase routing
projects the full requested suite into exact subsets; it does not invent a
level, preset, or implied companion optimization. Custody records retain both
the full build request and the subset completed at that stage. For example,
`SelectedIncomingU12ExactAddImmediate` and
`SelectedIncomingU12ExactSubtractImmediate` belong to selected lowering, so a
pre-physical Psi receipt may retain them in the requested suite while recording
that they completed no Psi pass. A later selected-lowering receipt must bind the
same full request before the suite can be considered complete.
`Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1` similarly belongs only to
post-allocation machine transformation: it is absent unless named by the root
build, and its custody receipt binds both the complete suite and that exact
phase projection. This is an individual transformation selection, not an
optimization level or a debug/release profile.
`SharedEntryFixedViewCopyAfterCompareBeforeBranchV1` belongs only to
`AllocationRecovery`; naming it chooses that exact copy policy and no hidden
allocator intensity or companion transformation. Until a composition contract
is admitted, orchestration rejects suites that combine it with another physical
phase.
`ActiveResidentImmediateU64MultiUseRematerializationV1` is a separate
`AllocationRecovery` family. Naming it chooses the exact two-view,
farthest-end-victim, immediate-u64 eligibility, and multi-use reconstruction
schedule under one shared budget. It does not choose the fixed-view-copy family
or any optimization level. Until their joint ordering and custody contract are
admitted, selecting both allocation-recovery families—or combining this route
with another unfinished physical phase—rejects.

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
- check-only compilation validates and retains the report request but does not
  enter native optimization merely to satisfy a report-only build;
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

The first compiler-facing report switch is the exact root-build call
`builder.optimizations.emit_report()`. It requests only the human text
projection and is stored independently from `OptimizationSelections`; a
report-only build still has the empty transformation set. Absence and legacy
authored `Build` shapes suppress it, duplicate requests reject, and dependency
build machines cannot set the root request. The optimization pipeline derives
one cumulative carrier from its opaque staged result, joining the complete
validated pre-physical and post-allocation records and, for the selected-
lowering or function-relative-layout route, the function-relative realization
record. Suppression returns
no text from that same carrier after all decisions, so it cannot affect a
candidate, register home, layout, or emitted byte.

The current Rust slice implements structured/text projections through the
validated abstract-plan and strict spill-free register-home boundaries, plus
direct-layout and selected-lowering function-relative/whole-exit realization
projections.
The pre-physical manifest's versioned standalone codec serializes that whole
earlier record and strict nested codecs; the post-allocation record adds
truthful home statistics while marking frame, emission, and publication
unavailable. The function-relative v6 record then binds exact phase projections
and optional completions to the validated final selected CFG, machine effects,
post-allocation machine, canonical encoding, baseline/final layout roots,
optional layout-transform receipt, named layout policy, final code-size
statistics, and the frameless whole-function exit contract. It explicitly
marks frame, section, relocation, image, installation, and publication fields
unavailable. The separate strict v3 function-fragment manifest additionally
binds completed x86 rel8, AArch64 CBNZ, active-resident rematerialization, or
Unit-baseline realization custody to relocation-free per-function bytes and
exact span/provenance/fuel statistics while retaining the same later-boundary
unavailability. The strict v3 text-section manifest then binds deterministic
no-padding placement, section coordinates, aggregate bytes, the semantic-entry
coordinate, and the independently proved absence of
relocation requirements for the current inventory. It still declares symbols,
object container/serialization, external entry bridge, image, installation,
and publication unavailable. The pre-physical, post-allocation, and function-
relative records have strict self-authenticating codecs. A third strict v1
object-container manifest binds the clean object plan, private function symbols,
object-local semantic-entry binding, strict `OMGTRO` bytes, and independently
replayed zero relocation-record count while keeping external entry, native
image, installation, and publication unavailable. The strict `OMGOTA` and
`OMGOTM` v1 records now join the exact Terminal semantic/proof artifact to that
clean object, every preceding manifest, and its rebuild roots. The pipeline-
owned cumulative report can gain fragment, text-section, object-container, and
artifact records only from the opaque artifact carrier that owns all of them by
value. The strict `OMGOER` and `OMGOEM` v1 records then add the independently
reconstructed native calling plan, parameter rows, scalar pseudo-result and
per-return bindings, private semantic-entry symbol, homes/units, and exact exit
contract while retaining the external-bridge-required disposition. The report
gains that record only from the new opaque callable-entry carrier; suppression
still changes no decision or byte. `OPT-MANIFEST-SCHEMA`
remains open until later process-bridge, native-image, and
publication manifests enter compiler custody. Successful native compiler
publication must then materialize the already-retained report request.

The decision-row substrate is self-authenticating rather than caller-stamped.
Each row derives its identity from the exact input unit, candidate, rule,
verdict, consumed analyses, canonical typed fact references, and validator; its
codec recomputes that identity. This prevents a future top-level manifest or
human projection from faithfully rendering a row whose evidence was never
actually identity-bound.

## Folder ownership

The final Omega-written product source belongs under
`source/omega/`:

```text
source/omega/
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
files under `source/omega-rust/omega/`:

```text
omega/
  foundation/omega-optimization-core/
  representations/omega-optimization-unit/
  representations/omega-register-model/
  representations/omega-terminal-legalized-operations/
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
