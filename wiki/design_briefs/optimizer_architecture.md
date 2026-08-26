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
    -> OptimizedPsiPlan + TransformationLedger
    -> abstract operations and target-independent storage decisions
    -> lowering optimization
    -> target operations with virtual register classes
    -> instruction selection / target combines
    -> register allocation + frame assignment
    -> assigned target operations
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
their coverage and joins them to transformation provenance; it does not
reimplement the borrow checker as a target alias analysis.

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

SSA applies naturally to scalar values. Memory, ownership, and cleanup are not
forced into a scalar fiction. They use explicit versioned tokens/frontiers so a
pass can prove that a rewrite preserves all relevant state. Address-stable
places remain address-stable even if scalar values around them are promoted.

## Analysis system

Analyses are deterministic functions of a unit revision and declared context.
The analysis manager caches them and invalidates only what a committed rewrite
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

The Rust bring-up keeps these products together in `omega-psi-optimizer`.
Analysis caches are compilation-local ordered maps keyed by the exact unit
revision; there is no process-global cache. Dependency resolution follows the
closed `AnalysisKind` order. Committing a revision expands declared
invalidation through dependent analyses, and the pass-validation configuration
cold-recomputes supposedly retained rows before changing the manager revision.
A mismatch is an undeclared-invalidation failure and leaves both cache and
revision untouched. Independent cold analyses may run concurrently, but their
published bundle is sorted back into canonical analysis order.

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
different schedule and therefore a different rule-set identity.

The first Rust candidate vocabulary is intentionally closed rather than an
opaque callback or byte payload. An exact-integer-evaluation candidate records
its input revision, rule contract, bounded region, analysis contract,
substitutions, provenance/fuel mapping, typed operand-fact identities, cost
estimate, and typed replacement. A literal operand-fact identity binds the
input revision, machine, value, scalar type, exact definition site, constant
payload, and source operation; a raw source operation ID is not sufficient
rewrite evidence. Its candidate identity covers that canonical declaration;
the output revision chains the input and candidate identities. The independent
validator—not the rule—reconstructs each fact identity and the arithmetic,
produces the new unit, and attaches its own validator identity. This establishes
the pattern future patch variants must follow before they become executable.

The initial pass-manager skeleton has a public entry only from
`VerifiedPsiOptimizationUnit`; a bare reconstructible seed cannot start a run.
It retains the complete verified input context, charges every bounded work
axis, restarts canonical rule dispatch after each accepted patch, requires a
strictly decreasing transformation-specific measure, and commits analysis
invalidation only after the independent validator constructs an accepted
output. Exhaustion or rejection returns no optimized session for publication.
This is still an internal vertical slice: build-level optimization selections
remain rejected until their complete named pass schedules and publication gate
exist.

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
known; an accepted transformation must first settle the obligation and record
that settlement in the transformation ledger.

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
