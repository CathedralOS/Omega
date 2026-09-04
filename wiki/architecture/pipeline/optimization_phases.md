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
accept/reject decision.

The producer exposes the construction, optimization, and publication seam
explicitly. `lower_machine` constructs an unsealed `LoweredTerminalPsi`;
`run_psi_optimization` consumes that complete carrier and returns a validated
`PsiOptimizationStageResult`; `finalize_terminal_artifact` accepts only that
stage result. The identity execution is live and validates both sides. Named
passes fail closed there until their rewrites and independent validators are
ported. The existing post-Terminal optimizer unit is reconstructed from a
verified Terminal module and an Omega abstract operation plan, so it cannot be
inserted at this seam unchanged: its target-neutral state must move to Psi or
its target/lowering-shaped fields must be replaced.

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
consumer never learns names such as an x86 materialization rule.

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

The current implementation constructs Terminal Psi first, lowers it into an
Omega-owned optimization unit, and dispatches empty and nonempty selections
through `Unoptimized` and `ExplicitOptimization` native branches. That is the
known transitional shape, not the target contract.

Migration proceeds in dependency order:

1. Make the phase model and phase-specific selections canonical. Empty
   selection becomes identity execution at every phase.
2. Retarget existing target-neutral passes and their validators to the live
   pre-Terminal entrance. Reuse existing optimization-unit vocabulary where
   sound; move ownership to Psi or replace target/lowering-shaped fields instead
   of preserving an Omega dependency for convenience.
3. Make build-selected Psi passes use that entrance. Canonical Terminal encoding
   then necessarily contains the selected Psi result; no selected pass may
   continue only in the post-Terminal compatibility route.
4. Make Terminal-to-abstract lowering unconditional and move any genuinely
   abstract-operation rewrites to their own later phase.
5. Replace the native `Unoptimized | ExplicitOptimization` entrance with one
   phase result. Preserve role-specific authority carriers such as ranked
   execution without using them as optimization bypasses.
6. Add checked-tree pruning only after its product-root identity, ownership,
   proof, effect, and boundary-retention rules are independently reconstructible.
7. Remove transitional names, branches, and documentation only after ordinary,
   selected, standalone-Terminal, resumed-lowering, and multi-target controls all
   pass through the same graph.

No migration step may make an unsupported selected route silently fall back to
an unoptimized route. It rejects at the first unimplemented phase boundary.
