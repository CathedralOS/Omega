# Optimization Phases

[Pipeline](pipeline.md) | [Optimizer architecture](../../design_briefs/optimizer_architecture.md)

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
  -> function-relative layout
  -> selected relaxation phase
  -> machine code
  -> object and image
```

This is one pipeline. Empty selections do not choose another backend. Mandatory
normalization and legalization remain lowering responsibilities and run whether
or not an optimization is selected. Optional profitability-changing rewrites
run only when their exact names are selected.

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
explicitly. `lower_machine` constructs an unsealed `LoweredTerminalPsi`;
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
layer out: `NativePhysicalStageResult` still carries assigned operations in its
ordinary and ranked identity arms but a completed optimizer-owned physical result
in its optimized arm. Every optimizer-owned arm then enters one function-fragment
emission stage; the first native projection admits an exact return-only Unit
shape and does not inspect which physical optimization variant produced it. The
public request surface uses the closed post-Terminal selection type, so this
transitional branch cannot reopen an earlier phase. Ranked-countdown native
authority currently rejects a nonempty post-Terminal selection: the ordinary
optimized target route cannot substitute for the independently admitted ranked
native route. Supporting that combination requires a ranked-aware optimizer
carrier through the same physical postcondition.

Migration proceeds in dependency order:

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
   outer Terminal-to-native physical result still mixes assigned ordinary/ranked
   identity plans with the optimized result. Optimizer-owned routes already
   converge through function-fragment emission and the bounded native projection
   is shape-driven rather than variant-driven. Replace the remaining mixed-depth
   outer carrier with one common physical postcondition, then extend native
   projection to the remaining validated fragment shapes. Preserve role-specific
   authority carriers such as ranked execution without using them as optimization
   bypasses.
6. Add checked-tree pruning only after its product-root identity, ownership,
   proof, effect, and boundary-retention rules are independently reconstructible.
7. Remove transitional names, branches, and documentation only after ordinary,
   selected, standalone-Terminal, resumed-lowering, and multi-target controls all
   pass through the same graph.

No migration step may make an unsupported selected route silently fall back to
an unoptimized route. It rejects at the first unimplemented phase boundary.
