# Optimization Phases

[Pipeline](pipeline.md) | [Optimizer architecture](../../design_briefs/optimizer_architecture.md)
| [Ownership cleanup plan](ownership_cleanup.md)

Status: target architecture and migration contract.

Optimization is an explicit pipeline phase. It is not hidden inside parsing,
checking, lowering, instruction selection, allocation, layout, or emission. A
build selects exact optimization names in `build.omg`; an empty selected set
executes the same phase as the identity transformation.

## Representation rule

An optimization normally consumes and produces the same representation. A new
representation is justified only when a transformation changes at least one of:

- the vocabulary available to later stages;
- the interpretation or invariants of the represented program; or
- the identity of the product being published.

Consequently `CheckedTrees -> CheckedTrees`, `PsiOptimizationUnit ->
PsiOptimizationUnit`, and `AbstractOperationPlan -> AbstractOperationPlan` are
valid optimization-stage shapes. They still have explicit selection, bounded
execution, validation, and transformation custody. Types such as
`PreOptimizedPsi` and `PostOptimizedPsi` are not created merely to record that
some bytes changed.

Pipeline folders name both endpoints literally, including identical endpoints:
`abstract-operations-to-abstract-operations` and
`post-allocation-machine-to-post-allocation-machine`. There is no
`-optimizer` naming exception. The visible sequence is `X-to-Y`, `Y-to-Y`,
`Y-to-Z`; individual passes and analyses are modules within the owning phase,
not additional apparent program routes. Empty selections preserve that sequence.

Before Terminal publication the current Rust sequence is
`checked-trees-to-lowered-psi` → `lowered-psi-to-lowered-psi` →
`lowered-psi-to-terminal-psi`. The intermediate `LoweredPsi` representation
owns the unsealed semantics and proof/debug/source companions. The compiler
coordinator owns product receipts; the stages own only their transformations.

Real boundaries retain distinct representations. Checked-tree lowering changes
vocabulary when it constructs Psi. Terminalization strengthens the contract by
sealing an immutable, self-contained Psi product. Terminal-to-abstract lowering
changes vocabulary again. Target legalization, instruction selection, physical
assignment, and machine encoding likewise keep their existing distinct
products.

## Target pipeline

```text
source
  -> tokens
  -> syntax trees
  -> symbol-resolved trees
  -> typed trees
  -> checked trees
  -> selected checked-tree optimization phase
  -> Psi optimization unit
  -> selected Psi optimization phase
  -> Terminal Psi
  -> abstract operations
  -> selected abstract-operation optimization phase
  -> target operations
  -> selected target-operation optimization phase
  -> selected instructions
  -> liveness and live ranges
  -> selected pre-allocation optimization phase
  -> allocation and physical homes
  -> selected post-allocation optimization phase
  -> selected-form encoding
  -> resolved function-relative layout
  -> selected resolved-layout optimization phase
  -> machine code
  -> object and image
```

This is one pipeline. Empty selections do not choose another backend. Mandatory
normalization and legalization remain lowering responsibilities and run whether
or not an optimization is selected. Optional profitability-changing rewrites
run only when their exact names are selected.

The layout portion has separate transform and optimization owners:
`post-allocation-machine-to-selected-form-encoding` produces encoding,
`selected-form-encoding-to-resolved-layout` constructs and independently checks
the baseline layout, and `resolved-layout-to-resolved-layout` executes the
explicit identity or selected relaxation phase. The latter owns the x86 rel8
catalog, production, and replay; the baseline owner does not re-export them.
Both phase outcomes expose the same `machine_code::ResolvedMachineLayout` data.
Sharing that immutable output does not mint a baseline-validation receipt.
Phase replay joins the exact baseline, encoding, machine, optional preceding
machine optimization, phase selections, and relaxation evidence before a
consumer accepts the current layout. Selected relaxation combined with a
preceding machine optimization remains unsupported; naming the phase does not
expand its composition contract.

## Allocation and frame ownership

Selected-program analysis owns machine effects; allocation owns ABI-preserved-register discovery.
Machine effects describe the current selected program and are not a competing
program stage; both construction and replay live in the selected-instruction
X-to-X stage's analysis modules.
Frame layout owns abstract
callee-save storage and spill requirements; machine emission owns packing the
resulting prologue/epilogue bytes. These calculations are modules of their
consuming phases, not separately scheduled public pipeline stages. Retained
requirements and receipts remain available for replay. Frame geometry is
checked by bounds and congruences, and emitted frame spans by exact order,
extent and target encoding; neither check re-enters the producing calculation.

Scalar-bearing internal calls retain their outgoing stack extent and ordered
register snapshot slots before byte emission. Assignment owns those choices;
emission checks the exact incoming-register roster, disjoint slot geometry and
minimal ABI alignment, then executes the retained transport without repairing
it. This includes scalar-result, mixed scalar/structural, and structural-result
calls. Aggregate-only Unit calls carry no scalar transport and retain their
separate aggregate transport handling until physical-route convergence.
These are current physical facts, not producer-history wrappers or a new
public pipeline stage.

## Terminal boundary

Terminal Psi is the output of target-neutral Psi optimization, not its mutable
input. Once published, Terminal Psi is immutable. Later native optimization may
transform abstract, target, selected-instruction, allocated, or layout
representations while retaining the Terminal identity and semantic relation;
it does not mint a replacement Terminal module.

A standalone Terminal product therefore contains the result of every selected
Psi-side optimization. A receiving interpreter or lowerer does not rerun those
passes. Target- or deployment-specific selections that have not run travel as
strongly bound companion policy and remain subject to the receiving authority's
accept/reject decision. For a target-constrained Terminal product, the retained
native proposal carries the exact post-Terminal selection and the identity of
the complete build selection. Proposal validation rejoins that identity to the
Psi selection recorded by the Terminal artifact, and a lowerer accepting the
proposal must accept exactly the pending selection or reject it. This is a
proposal, not optimization authority: a different lowerer may refuse it. A raw
target-neutral Terminal artifact has no such target policy and leaves later
physical selection to its receiving authority.

The producer exposes the construction, optimization, and publication seam
explicitly. `lower_machine` constructs an unsealed `LoweredPsi`;
`run_psi_optimization` consumes that complete carrier and returns a validated
`PsiOptimizationStageResult`; `finalize_terminal_artifact` accepts only that
stage result. The identity execution is live and validates both sides. Named
passes fail closed there until their rewrites and independent validators are
ported. The remaining post-Terminal optimizer unit is reconstructed from a
verified Terminal module and an Omega abstract-operation plan and is now
reachable only for phases owned after Terminal publication. Its target-neutral
passes must move to Psi rather than being reselected by a receiving lowerer.

The canonical Terminal artifact retains the complete Psi optimization
execution record: the exact selected pass set plus the semantic and proof
identities before and after the stage. The artifact manifest binds the strong
execution identity, and decoding independently checks that the record's output
identities equal the decoded semantic and proof sections. An empty selection
may claim only identical input and output products. Thus stopping compilation
at Psi does not discard which target-neutral phase produced the portable
artifact, and a later lowerer does not need frontend state to recover it.

## Checked-tree pruning

Whole-program selection and unreachable-declaration pruning may earn a
checked-tree optimization phase because checked trees already contain the facts
needed to determine a product closure. This phase runs only after all authored
source has been parsed, resolved, typed, and checked. It cannot make invalid
authored source valid by deleting it before checking.

When root selection changes which program is published, that selection is a
product boundary and its identity is retained explicitly. The pruning mechanics
may still operate on `CheckedTrees`; the selected product is not inferred from
the shape of the remaining vectors.

Shared frontend work precedes target fan-out. A multi-target build can reuse
source, token, syntax, resolution, typing, and target-independent checking, then
derive one target/root-selected Psi product per requested profile. Platform
details do not enter target-neutral Psi merely because product selection has
branched.

## Selection ownership

Native phase sequencing and report assembly live in
`native-realization/src/native_pipeline`. Function realization,
fragment emission and placement admission live in `machine-emission`;
object publication lives in `object-file`, and callable-entry admission
in `native-artifact`. These owners retain replay inputs explicitly without
reopening source/frontend state. Build evaluation consumes report-request data
from `optimization-core`, not an executable optimization coordinator.

The pass manager owns baseline candidate selection. Baseline decision logs and
the external decision schema live in `optimization-core/src/decisions`;
their record-only builder and codecs never choose a rewrite. Offline policy
tools consume this data directly, without depending on the executing optimizer.

Abstract optimization includes its own publication step. The native coordinator
calls `optimize_abstract_operations`; it does not schedule the pass manager and
projection separately. The output exposes the current abstract program directly
and retains replay inputs and `AbstractOptimizationEvidence` separately. It
does not retain the executing session or its analysis cache. Publication replay
remains independent of the pass execution algorithm.

`build.omg` remains the source of exact opt-in selections. Its one ergonomic
selection surface is projected into phase-specific closed sets:

- checked-tree selections;
- Psi selections;
- abstract-operation selections;
- target-operation and instruction-selection selections;
- pre-allocation selections;
- post-allocation selections; and
- layout/relaxation selections.

Every optimization belongs to exactly one phase. A stage consumes only its own
projection. No later coordinator rescans the global set and invents a second
schedule.

The unified build vocabulary is not imported into Psi. Target-neutral Psi pass
identities and their canonical selection encoding are Psi-owned; target,
instruction, allocation, machine, and layout identities remain Omega-owned. The
build coordinator performs one exhaustive structural projection and retains the
complete build-selection identity beside the Psi-local selection. Adding a pass
therefore forces its owning phase to be classified, while a standalone Psi
consumer never learns names such as an x86 materialization rule. Conversely,
the Terminal-to-native API accepts `PostTerminalOptimizationSelections`, whose
constructor excludes checked-tree and Psi names. Earlier phases are therefore
unrepresentable at resumed lowering rather than global names accepted and then
rejected by convention.

## Stage contract

Every optimization phase, including the identity case, has one entrance that:

1. receives one validated input representation and its exact phase selection;
2. executes the selected rules under explicit bounds in canonical order;
3. independently validates every applied transformation;
4. validates the final representation;
5. publishes the selected set and transformation/identity record; and
6. returns one typed stage result to the next pipeline stage.

An empty set publishes canonical identity execution: no pass manifests, no
transformation records, and an output equal to the input. It does not bypass
input or output validation.

## Migration from the current implementation

Rematerialized programs use the ordinary encoding and resolved-layout stages.
The separate active-resident encoding crate and its layout/realization wrappers
have been removed; their byte and corruption controls exercise the shared paths.

Physical coordination performs instruction selection, selected-instruction
optimization, register allocation, and post-allocation machine construction
once. The selected-instruction X-to-X stage owns selected-lowering execution,
including liveness and pressure analyses and their reconstruction after a fold.
Liveness and live-range plans, subordinate rows, and canonical identities live
under the `selected-instructions` representation root: they describe selected
virtual instructions before any physical homes exist. Allocation and later
machine rewrites consume that data directly. Computation, independent replay,
errors, and sealed validation receipts remain in the selected X-to-X stage;
constructing or hashing a raw plan grants no validation authority.
Allocation consumes that result and owns assignment and pressure recovery.
Required frameless contracts are explicit in the analysis for their consuming
rewrite or layout policy. Function-relative realization consumes the same
allocation and machine outputs and independently checks their join. It cannot
rerun either earlier stage or substitute a machine from another allocation.
Optional later layout execution is read from its phase
selection, not encoded as a second selected-lowering route variant.

Crate extraction alone does not satisfy this migration. Several physical
stages still own `StagedOptimized...` wrappers containing preceding stage
objects, and the coordinator still branches by optimization history. Those
wrappers and branches must be replaced at coherent representation boundaries;
they are not additional canonical pipeline phases.

The selected-program effect boundary now consumes one validated selected
program and an explicit target environment. Its single analysis/replay entrance
returns effects bound to that current program, without a five-way optimizer
lineage sum. Post-allocation construction replays the upstream transformation
before entering the common analysis. Persisted effect rows, identities, and
encoding are selected-representation-owned; analysis and admission remain
pipeline-owned.

Layout-independent selected-form bytes are current representation data in
`machine-code::SelectedFormEncoding`, reached through the machine-code root's
encoding owner. Rows, physical footprints, call templates, counts, and normalized
optimization custody do not belong to the encoding transform. That transform
constructs and independently validates the raw program before returning its
sealed admission wrapper. Layout reads representation-owned rows, and fragment
emission retains the same immutable encoding program beside its other current
inputs. Only replay retains the producing stage's admission token; sharing or
rehashing the raw program grants no encoding or publication authority.

Selected-lowering rewrites and selected-program analysis algorithms belong to
`selected-instructions-to-selected-instructions`; they do not assign register
homes. Raw allocation facts belong to `register-homes`, under its constraints,
storage, and recovery owners: allocator availability, legality, fixed-precolored
intervals, split requirements, segment homes, spill choices, and recovery
classifications. Their canonical identities and raw codecs live beside those
records; compute/replay code and sealed validation receipts stay in the stage.
The result retains one current selected program and separate replay
inputs. The successor `selected-instructions-to-register-homes` consumes it
without re-executing the selected-lowering suite. Its sealed
allocation boundary independently reconstructs retained evidence and exposes
one immutable current-program view, with policy and evidence separate from the
program's identity. Both machine-plan construction and post-allocation machine
optimization consume that boundary without inspecting stage ancestry or
selecting an `after_*` route. Post-allocation function realization now retains
allocation-owned immutable replay inputs and exposes their current facts to
fragment emission. Its construction, replay, and manifest join no longer switch
between baseline, selected lowering, and allocation recovery. Other
layout/realization adapters, allocation's retained internal histories, and outer
identity/selected routing remain to be
consolidated; they are not additional canonical stages.

The current implementation projects the complete effective build selection in
two directions. The Psi projection runs at the checked-to-Terminal entrance and
is retained in the sealed artifact. For a target-constrained Terminal product,
the post-Terminal projection is retained in the native proposal and excludes
checked-tree and Psi phases before native realization. The proposal preserves
the complete build-selection identity so its two phase projections cannot be
silently recombined from different builds. A standalone receiving lowerer
rejects either earlier-phase selection instead of rerunning it. Terminal-to-
abstract native admission and lowering now run unconditionally before
selection presence is inspected. A closed `Identity | Selected` optimization
continuation makes empty execution explicit instead of encoding it as a missing
context. Both cases first enter one explicit post-Terminal optimization stage.
That stage alone consumes the continuation and publishes ordinary identity,
ranked identity, or validated optimized-ordinary custody. Empty selection still
retains the verified optimizer input, executes the abstract-stage identity
validation, and rejects a changed plan. Target lowering
consumes that closed result and cannot invoke the optimizer or inspect the
earlier continuation. Machine emission sends the target result through one
physical-routing stage and consumes its closed output. It no longer schedules
abstract optimization, target lowering, physical assignment, or physical
optimization itself. The physical entrance
projects its closed post-Terminal selection once into exact phase-local inputs;
composition does not rescan the global set, and a post-Terminal phase without
an implemented stage rejects rather than disappearing. Executable physical
catalogs accept only `OptimizationPhaseSelections`, validate the owning phase,
and cannot rediscover policy by scanning the global selection. Inside the
selected optimizer pipeline, every physical route now reaches a validated
function-relative realization. Empty physical selection is classified from the
already selected representation into unit, structural-unit, or fixed-frame
identity execution before custody is consumed; it never means try a specialized
route and silently fall back. Consequently every selected-pipeline result owns a
non-optional function-relative manifest. The remaining transitional split is one
layer out: `NativePhysicalStageResult` still carries assigned operations for
ranked programs and ordinary identity programs beyond the supported fragment
publication shape. The selection stage owns publication-input classification,
using the same input predicates and ordered form catalog as legalization.
The coordinator contains no separate scalar grammar and does not try legalization
or emission before choosing a route. Classification grants no proof or physical
authority; construction and independent replay still check the exact program.
Return-only Unit programs, free `u64` scalar leaves, and the catalog's
integer-ABI conditionals use the shared fragment stages even with empty selections.
Scalar expression leaves carry ordered constants and proof-checked exact
addition/subtraction steps, with operands referencing earlier definitions or
register-passed ABI inputs. Legalization and selection do not classify these
sequences by tree shape or chain length. The same representation carries the
existing conditional pressure graphs; rematerialization remains an opt-in
rewrite over their ordinary instructions, not a special source-program kind.
Independent replay checks definition order, operand identity, accepted arithmetic
facts and logical fuel. Legalized identity v25 binds these ordered steps; the
retired chain-specific tags are not reused. Incoming stack arguments and other
scalar operations remain outside this sequence contract.
The direct-return conditional forms include unsigned equality and inequality,
signed/unsigned less-than and less-or-equal, and unsigned equality/inequality
against zero, returning `u64` constants from two arms. Boolean-parameter forms
remain outside default fragment publication until the ordinary scalar ABI
carrier represents Boolean explicitly; catalog membership cannot replace that ABI.
Source-generated common-return functions retain all four blocks, both jumps,
the shared result parameter, and every authored successor binding. Selection
does not duplicate the return or manufacture a different source graph.
Liveness substitutes incoming arguments for live destination parameters;
allocation independently checks the same-home constraints that currently
realize those transfers without copies. Conflicting home constraints reject
until edge-copy scheduling exists. Unused incoming parameters retain their
semantic bindings without requiring physical registers.
Scalar leaves retain their target ABI, keep incoming parameter precolors, and explicitly
copy a returned parameter into a separate return-constrained virtual value.
Structural parameters, cleanup, callbacks and provider settlements remain
outside this scalar migration. Direct-return controls start at separately
authored, verified Terminal products; shared-return controls start at Omega
source and pass through ordinary Terminal publication and resumed lowering.
Ordinary straight-line scalar and Unit functions share one legalized function
graph: typed parameters, ordered instructions, and an explicit value or Unit
return. The separate Unit and Unit-caller rosters and their call recipe are
removed. Calls may target other callers; their results may be returned, reused,
or discarded without erasing the call. Executable order comes from the checked
operation stream, not recursive target return expressions. Those expressions
remain source/ABI evidence, not the new graph's executable payload.
Each call retains its exact ABI and source operation; allocation sees explicit
argument/result copies and target-owned clobbers. Entry parameters are copied
out of ABI-fixed registers so they can survive calls, and return transport has
its own result-constrained value. These graphs reach framed fragment text,
object, executable image and installation publication on Linux x64/Arm64,
Windows x64 and macOS Arm64 with empty or selected physical phases.
The multihop scalar-return coordinator regression authors Terminal directly; it does
not establish checked-source lowering of a scalar helper that returns another call.
Default source builds enter the same path by the selection stage's input grammar, before routing;
they do not probe emission or fall back after failure. Selected physical
materialization uses the same frame construction and independent replay as
ordinary realization. Allocated preservation writes and calls determine frame
requirements, not selection presence. The optimized body retains its exact
encoding/layout custody alongside frame geometry and protocol; fragment emission
and frame application carry both through text and object publication. Removing
or substituting either side rejects. Image emission consumes the validated
shared object directly, preserving its bytes, ABI, semantic intervals and exact
frame/call stack prefixes. It does not rebuild an assigned machine plan or
manufacture stack-held result records for values in preserved registers.
Installation replay composes WCSU from those same frame and call facts, taking
the maximum across sequential calls and composing nested callees. Scalar and
Unit callers retain their corresponding checked stack envelopes; neither is
relabeled to bypass publication checks. Multi-block call graphs, narrow and
Boolean call transport, stack arguments, and the remaining structural/ranked
routes are unfinished physical-route convergence work.
Register-call argument counts come from the checked call plan, including zero;
they do not select different source-program recipes. The target catalog supplies
one exact constraint and effect row per admitted ABI register roster. Selection
and independent replay check every argument's source, ordered parameter position,
fixed register and result placement. Encoding checks the same exact register
roster while preserving the target's ordinary call bytes and relocation. The
current System V AMD64, AAPCS64, Microsoft x64 and Darwin Arm64 routes admit
all-register U64 signatures; an argument requiring stack placement remains
outside this contract. Legalized identity v26 binds the ordinary graph and
return role; legalization verifier v27 independently checks its source, ABI,
fuel, effects and ownership. Register-environment and machine-effect identities
bind the exact arity-key roster. Empty physical selection uses the same
admission and encoding route. Microsoft calls reserve their required shadow
area in the same frame calculation used by publication replay.
The existing Microsoft-x64 structural Unit family also reaches this shared
object, image and installation path with empty or nonempty selections: an
owned-indirect pair leaf, or one entry caller passing that pair to one leaf.
The call may be authored or selected from an admitted provider conformance;
claim-completion prefixes retain their execution and receipt evidence as
zero-byte metadata. Publication records incoming pointer locations directly,
not invented local homes or cleanup. Provider calls keep their distinct origin
and exact conformance through installation encoding and independent replay.
The call's 72-byte outbound area and eight-byte return address contribute 80
bytes to stack demand without becoming a persistent frame. Larger structural
graphs, other ABIs and structural executable-entry provisioning remain outside
this bounded family; object/image construction does not admit an entrypoint.
The input classifier enforces the existing singleton or caller-to-leaf topology
before routing, rather than diverting unsupported programs into later rejection.
This boundary is checked before execution; a failure never
selects the old route. Empty and selected fragment publication bind the exact
validated abstract projection, final optimization unit and shared object custody.
Native admission rejects a substituted object, including changed metadata;
the retained object is immutable equality evidence, not a second current-program
access path. Missing application coverage remains explicitly unavailable.
Every optimizer-owned arm then enters one function-fragment
emission stage. Native projection admits Unit returns, ordered calls, the bounded
structural family and the scalar bodies above;
it does not inspect which physical optimization variant produced them. Scalar
publication binds the exact ABI, bytes and semantic intervals, with independent
object/stack replay. Forward scalar graphs retain actual conditional predicates,
unconditional targets and returns. Byte-level replay reconstructs block boundaries
and transfers independently; it rejects hidden branches, backward edges, missing
blocks and inconsistent incoming stack depths at joins. A shared frame is restored
at the actual return, not at each jump. The
taken edge owns the branch interval; the fallthrough edge has a zero-width
coordinate at the next instruction. Both retain their one semantic conditional
ordinal. Microsoft-x64 leaf frames retain the incoming stack when
empty and align any allocated storage; outgoing calls still need a home-area
contract and are not admitted by that frame policy. The
public request surface uses the closed post-Terminal selection type, so this
transitional branch cannot reopen an earlier phase. Ranked-countdown native
authority currently rejects a nonempty post-Terminal selection: the ordinary
optimized target route cannot substitute for the independently admitted ranked
native route. Supporting that combination requires a ranked-aware optimizer
carrier through the same physical postcondition.

Migration proceeds in dependency order:

The optimizer-owned physical result is one struct carrying the retained
function-relative realization into emission, not a second seven-way route enum.
It exposes the current machine and manifests without walking allocation ancestry
or cloning program data. Fragment construction selects only scalar versus
structural program shape. Recovery, selected-lowering, and machine/layout
optimization histories remain distinct replay inputs and evidence identities;
their validators run before emission but do not choose its algorithm. The
remaining realization replay carriers are transitional, not new canonical IRs.

1. **Complete.** Make the phase model and phase-specific selections canonical.
   Empty selection becomes identity execution at every phase.
2. Retarget existing target-neutral passes and their validators to the live
   pre-Terminal entrance. Reuse existing optimization-unit vocabulary where
   sound; move ownership to Psi or replace target/lowering-shaped fields instead
   of preserving an Omega dependency for convenience.
3. **Complete as a routing boundary.** Build-selected Psi passes use that
   entrance and canonical Terminal encoding contains the selected Psi result.
   The currently unported named rewrites fail closed at that entrance. No
   selected Psi pass continues in the post-Terminal compatibility route.
4. **Complete as a routing boundary.** Terminal-to-abstract native admission
   and lowering produce one stage result regardless of selection presence. An
   explicit post-Terminal optimization stage consumes its closed continuation
   before target lowering; target lowering cannot schedule abstract optimization.
   Identity continuation executes and validates the empty abstract-operation
   phase rather than passing its plan through unchecked.
   Ranked execution remains role-specific authority and selected ranked input
   fails closed until a ranked-aware optimizer carrier exists. There are
   currently no cataloged abstract-operation rewrites to move.
5. **Optimizer-owned physical routes now converge at function-relative
   realization; outer convergence remains.** Identity and selected execution
   enter one target-lowering stage and then one physical-routing stage. The
   selected optimizer pipeline classifies identity input by its already selected
   representation and produces a validated unit, structural-unit, or fixed-frame
   realization; it does not try one route and fall back to another. Its public
   function-relative manifest is therefore non-optional for every route. The
   outer Terminal-to-native physical result still mixes assigned ranked and
   richer ordinary identity plans with the shared fragment result. Return-only
   identity programs already reach that result without selecting an optimization.
   Optimizer-owned routes already
   converge through function-fragment emission and the bounded native projection
   is shape-driven rather than variant-driven. Replace the remaining mixed-depth
   outer carrier with one common physical postcondition, then extend native
   projection to the remaining validated fragment shapes. Preserve role-specific
   authority carriers such as ranked execution without using them as optimization
   bypasses. Register allocation and the post-allocation machine optimizer are
   now ordinary pipeline stage crates rather than children of the transitional
   `pipeline/optimization` island. That directory is now removed rather than
   preserved as an architectural layer. The remaining
   `native-realization` crate is a transitional cross-stage
   coordinator to split and delete, not the replacement layer. Deterministic
   optimization policy likewise lives at pipeline rank beside its consumers,
   as does the independent optimization-unit validator. The former
   `omega-psi-optimizer` is now named `abstract-operations-to-abstract-operations`:
   its units are reconstructed from Terminal Psi and it runs after publication,
   so it cannot stand in for the still-distinct portable Psi phase.
6. Add checked-tree pruning only after its product-root identity, ownership,
   proof, effect, and boundary-retention rules are independently reconstructible.
7. Remove transitional names, branches, and documentation only after ordinary,
   selected, standalone-Terminal, resumed-lowering, and multi-target controls all
   pass through the same graph.

No migration step may make an unsupported selected route silently fall back to
an unoptimized route. It rejects at the first unimplemented phase boundary.
