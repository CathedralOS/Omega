//! Optimizer module role: test leaf. Transitive member-parameter invariant discovery and place-observation admission.

use super::super::super::VerifiedPsiOptimizationSession;
use crate::{
    LoopInvariantNodeResult, LoopInvariantScalarMotionError, LoopInvariantScalarRelocation,
    apply_loop_invariant_scalar_motion, propose_loop_invariant_scalar_motion,
    validate_loop_invariant_scalar_motion,
};
use abstract_operations::AbstractOperation;
use optimization_unit::{
    NodeLocation, ProvenanceDisposition, PsiOptimizationUnit, PsiProvenance, PsiRealizationSite,
    recompute_psi_optimization_unit_identity,
};
use optimization_unit_semantics::OptimizationUnitValidationError;
use semantic_vocabulary::ValueId;

/// Two-state unranked cycle: the entry state forwards `scale` to `step`, which
/// carries it back unchanged, and every traversal that leaves the component
/// passes through `step` — the loop's only exit is `step`'s own `finish` arm.
/// `s` is a parameter of a non-entry member block — provably invariant only
/// once member parameters resolve transitively through the component's
/// internal edges — so `s + s` is the relocated computation this family adds
/// over the entry-target-only discovery, and `step` dominating every exit
/// keeps the relocation inside the non-speculative gate.
const TRANSITIVE_MEMBER_SOURCE: &str = r#"
    data Root {}

    machine Root::scan(scale: u32 in Wrapping, remaining: u32 [0..=5])
    {
        transition { _ -> step(scale, remaining) }
        state step(s: u32 in Wrapping, pending: u32 [0..=5]) {
            let doubled: u32 in Wrapping = s + s;
            transition pending > 0 {
                true -> scan(s, pending - 1)
                _ -> finish(doubled)
            }
        }
        state finish(r: u32 in Wrapping) {}
    }
"#;

/// Same component shape, but `step` advances `s` on the back edge, so the
/// member parameter is genuinely loop-carried and `s + s` must stay inside
/// even though its block is guaranteed to execute.
const CARRIED_MEMBER_SOURCE: &str = r#"
    data Root {}

    machine Root::scan(scale: u32 in Wrapping, remaining: u32 [0..=5])
    {
        transition { _ -> step(scale, remaining) }
        state step(s: u32 in Wrapping, pending: u32 [0..=5]) {
            let doubled: u32 in Wrapping = s + s;
            transition pending > 0 {
                true -> scan(s + 1, pending - 1)
                _ -> finish(doubled)
            }
        }
        state finish(r: u32 in Wrapping) {}
    }
"#;

/// Same two-state cycle as `TRANSITIVE_MEMBER_SOURCE`, but the entry state can
/// leave the component through its own `done` arm before `step` ever runs.
/// `s` still resolves to the preheader anchor — invariance is intact — yet
/// `s + s` stays inside because relocating a node out of a member block that
/// does not dominate every exit would speculate executions the traversal may
/// never perform. The header's own invariant leaves still relocate.
const BYPASSED_MEMBER_SOURCE: &str = r#"
    data Root {}

    machine Root::scan(scale: u32 in Wrapping, remaining: u32 [0..=5])
    {
        transition remaining > 0 {
            true -> step(scale, remaining - 1)
            _ -> done()
        }
        state step(s: u32 in Wrapping, pending: u32 [0..=5]) {
            let doubled: u32 in Wrapping = s + s;
            transition pending > 0 {
                true -> scan(s, pending - 1)
                _ -> finish(doubled)
            }
        }
        state done() {}
        state finish(r: u32 in Wrapping) {}
    }
"#;

/// The component's unique entry edge is one of two successors on the
/// preheader's terminator: reaching the preheader does not guarantee entering
/// the loop, so relocating `s + s` would execute it on traversals that never
/// enter the component. The computation stays inside; work-free
/// scalar-constant leaves still relocate.
const CONDITIONAL_ENTRY_SOURCE: &str = r#"
    data Root {}

    machine Root::enter(scale: u32 in Wrapping, go: bool, remaining: u32 [0..=5])
    {
        transition go {
            true -> scan(scale, remaining)
            _ -> done()
        }
        state scan(s: u32 in Wrapping, pending: u32 [0..=5]) {
            let doubled: u32 in Wrapping = s + s;
            transition pending > 0 {
                true -> scan(s, pending - 1)
                _ -> finish(doubled)
            }
        }
        state done() {}
        state finish(r: u32 in Wrapping) {}
    }
"#;

/// A `self`-carried countdown whose member observes a field through the
/// function's structural-parameter root: `self.value` lowers to an
/// `IntegerStructuralField` naming `PlaceId(1)` directly — no member
/// structural parameter stands between the read and the root — the component
/// performs no place mutation or custody movement at all, and `step` is the
/// only member, so every traversal that leaves passes through it. The
/// observation relocates into the preheader byte-exact and the scalar
/// computation chained on the observed result relocates behind it through the
/// same run.
const OBSERVED_FIELD_SOURCE: &str = r#"
    data Root { value: u32 in Wrapping }

    machine Root::scan(&mut self, remaining: u32 [0..=5])
    {
        transition { _ -> step(remaining) }
        state step(&mut self, pending: u32 [0..=5]) {
            let observed: u32 in Wrapping = self.value + 0;
            transition pending > 0 {
                true -> step(pending - 1)
                _ -> finish(observed)
            }
        }
        state finish(&mut self, r: u32 in Wrapping) {}
    }
"#;

/// Same component shape with a member store: `self.ticks = self.ticks + 1`
/// writes through a root the traversal can reach, so the whole-component
/// custody gate refuses every place observation — the next iteration may
/// observe different storage — while scalar-constant leaves still relocate.
const OBSERVED_FIELD_MUTATED_SOURCE: &str = r#"
    data Root { value: u32 in Wrapping; ticks: u32 in Wrapping; }

    machine Root::scan(&mut self, remaining: u32 [0..=5])
    {
        transition { _ -> step(remaining) }
        state step(&mut self, pending: u32 [0..=5]) {
            let observed: u32 in Wrapping = self.value + 0;
            self.ticks = self.ticks + 1;
            transition pending > 0 {
                true -> step(pending - 1)
                _ -> finish(observed)
            }
        }
        state finish(&mut self, r: u32 in Wrapping) {}
    }
"#;

/// The place-custody gate is intact — no member writes any place — but the
/// observation lives in `step`, a member the entry state's `done` arm can
/// bypass. Observing `self.value` performs work a skipped traversal never
/// pays, so the non-speculative gate keeps the read inside even though the
/// observation itself is loop-invariant.
const BYPASSED_OBSERVED_FIELD_SOURCE: &str = r#"
    data Root { value: u32 in Wrapping }

    machine Root::scan(&mut self, scale: u32 in Wrapping, remaining: u32 [0..=5])
    {
        transition remaining > 0 {
            true -> step(scale, remaining - 1)
            _ -> done()
        }
        state step(&mut self, s: u32 in Wrapping, pending: u32 [0..=5]) {
            let observed: u32 in Wrapping = self.value + s;
            transition pending > 0 {
                true -> scan(s, pending - 1)
                _ -> finish(observed)
            }
        }
        state done(&mut self) {}
        state finish(&mut self, r: u32 in Wrapping) {}
    }
"#;

/// A `view` member structural parameter forwarded unchanged on the back edge:
/// `step` binds `view` from the machine's `entries` root on the entry edge and
/// to itself on the back edge, so the member view parameter resolves
/// transitively to the preheader-visible `entries` root — the place analog of
/// `s`'s scalar resolution in `TRANSITIVE_MEMBER_SOURCE`. `view.len` is an
/// invariant place observation through that member parameter: the relocation
/// rebinds the observed root to `entries` rather than moving byte-exact, and
/// `s + s` relocates behind it through the ordinary scalar path in the same
/// run.
const MEMBER_VIEW_SOURCE: &str = r#"
    data Root {}

    machine Root::scan(scale: u32 in Wrapping, entries: &[u8], remaining: u32 [0..=5])
    {
        transition { _ -> step(scale, entries, remaining) }
        state step(s: u32 in Wrapping, view: &[u8], pending: u32 [0..=5]) {
            let length: u64 = view.len;
            let doubled: u32 in Wrapping = s + s;
            transition pending > 0 {
                true -> step(s, view, pending - 1)
                _ -> finish(doubled)
            }
        }
        state finish(r: u32 in Wrapping) {}
    }
"#;

/// A two-member component whose view parameters chain through the internal
/// edge: `step` binds `view` from `entries` on the entry edge and from
/// `swap`'s `other` on the back edge, while `other` binds `step`'s `view` on
/// `step`'s internal edge, so both member view parameters resolve
/// transitively to the `entries` root through each other. `swap` owns the
/// component's only exit, so both members are guaranteed to execute on every
/// traversal that leaves — `view.len` in `step` and `other.len` in `swap`
/// each relocate, rebinding their observed root to `entries`.
const TRANSITIVE_VIEW_SOURCE: &str = r#"
    data Root {}

    machine Root::scan(scale: u32 in Wrapping, entries: &[u8], remaining: u32 [0..=5])
    {
        transition { _ -> step(scale, entries, remaining) }
        state step(s: u32 in Wrapping, view: &[u8], pending: u32 [0..=5]) {
            let length: u64 = view.len;
            let doubled: u32 in Wrapping = s + s;
            transition { _ -> swap(s, view, pending) }
        }
        state swap(t: u32 in Wrapping, other: &[u8], pending: u32 [0..=5]) {
            let width: u64 = other.len;
            transition pending > 0 {
                true -> step(t, other, pending - 1)
                _ -> finish(t)
            }
        }
        state finish(r: u32 in Wrapping) {}
    }
"#;

/// Same `view` member parameter shape, but the back edge forwards a fresh
/// subslice the member itself establishes, so `view` is re-established inside
/// the component on every traversal and stays loop-carried: the observation
/// can rebind to no representative and stays inside. The subslice
/// establishment also ends the component's place-custody preservation, so the
/// custody gate refuses first — the member-produced binding keeps the refusal
/// exact at the resolution level too.
const CARRIED_VIEW_SOURCE: &str = r#"
    data Root {}

    machine Root::scan(scale: u32 in Wrapping, entries: &[u8], remaining: u32 [0..=5])
    {
        transition { _ -> step(scale, entries, remaining) }
        state step(s: u32 in Wrapping, view: &[u8], pending: u32 [0..=5]) {
            let length: u64 = view.len;
            let doubled: u32 in Wrapping = s + s;
            transition pending > 0 && view.len > 0 {
                true -> step(s, view[1..], pending - 1)
                _ -> finish(doubled)
            }
        }
        state finish(r: u32 in Wrapping) {}
    }
"#;

/// A slice-length countdown: the machine retains validated `Natural` ranking
/// evidence, so the optimizer component roster is anchored on the verifier's
/// canonical cyclic-component surface rather than on a private re-derivation
/// of the Terminal body. `scale + scale` is a side-effect-free scalar
/// computation the loop recomputes identically every iteration.
const RANKED_MEMBER_SOURCE: &str = r#"
    data Root {}

    machine Root::scan(scale: u32 in Wrapping, entries: &[u8])
    terminates by entries -> Slice::Length;
    {
        let doubled: u32 in Wrapping = scale + scale;
        transition entries.len > 0 {
            true -> scan(scale, entries[1..])
            _ -> done(doubled)
        }
        state done(r: u32 in Wrapping) {}
    }
"#;

#[test]
fn transitive_member_parameter_computation_hoists_rebinding_to_its_anchor() {
    let session = lowered_session(TRANSITIVE_MEMBER_SOURCE, "transitive member loop");
    let [component] = session.cycle_components().components() else {
        panic!("one two-state component")
    };
    let [entry] = component.entries.as_slice() else {
        panic!("one entry edge")
    };
    let function = session
        .unit()
        .functions
        .iter()
        .find(|function| function.machine == component.id.machine)
        .expect("component machine exists");
    let (member_block, addition, member_parameter) = member_addition(function, component);
    // The anchor is the outside value the entry edge binds to the header
    // parameter `s` chains to: `step` is entered with `s := scale'`, and
    // `scale'` is bound from the machine parameter on the entry edge.
    let header = function
        .blocks
        .iter()
        .find(|block| block.id == entry.target)
        .expect("entry target exists");
    let entry_edge = function
        .blocks
        .iter()
        .flat_map(|block| block.nodes.iter().flat_map(|node| node.successors.iter()))
        .find(|edge| edge.psi_edge == entry.edge && edge.target == entry.target)
        .expect("the preheader terminator owns the entry edge");
    let anchor = entry_edge
        .bindings
        .iter()
        .find(|binding| binding.parameter == header.parameters[0].value)
        .expect("entry binds the carried header parameter")
        .argument;
    assert_eq!(
        crate::validation::invariant_member_parameters(function, component).get(&member_parameter),
        Some(&anchor),
        "the member parameter resolves transitively to its preheader anchor"
    );

    let candidates =
        propose_loop_invariant_scalar_motion(&session, 1).expect("one exact relocation candidate");
    let [candidate] = candidates.as_slice() else {
        panic!("one component yields one atomic candidate")
    };
    let relocation = candidate
        .relocations()
        .iter()
        .find(|relocation| {
            relocation.node().psi_operation()
                == match addition.provenance.first() {
                    Some(PsiProvenance::Operation(operation)) => *operation,
                    _ => panic!("computation carries its operation identity"),
                }
        })
        .expect("the member-parameter computation is a planned relocation");
    assert_eq!(
        relocation.node().operand_rewrites(),
        &[(member_parameter, anchor)],
    );
    assert_eq!(relocation.node().location().block, member_block.id);
    assert_eq!(relocation.destination().block, entry.source);

    let validated = validate_loop_invariant_scalar_motion(&session, candidate)
        .expect("independent relocation validation");
    let applied = apply_loop_invariant_scalar_motion(session, validated)
        .expect("atomic relocation application");
    let destination = applied
        .session()
        .unit()
        .functions
        .iter()
        .flat_map(|function| &function.blocks)
        .find(|block| block.id == relocation.destination().block)
        .expect("destination block exists");
    let moved = &destination.nodes[usize::try_from(relocation.destination().node).unwrap()];
    match &moved.operation {
        AbstractOperation::WrappingIntegerAdd { left, right, .. } => {
            assert_eq!(*left, anchor);
            assert_eq!(*right, anchor);
        }
        operation => panic!("relocated computation keeps its operation: {operation:?}"),
    }
    assert!(moved.uses.iter().all(|value_use| value_use.value == anchor));
    assert_eq!(moved.provenance, relocation.node().provenance());
    assert_eq!(moved.fuel, relocation.node().fuel());

    let [record] = applied.ledger().records() else {
        panic!("one atomic relocation has one ledger record")
    };
    let row = record
        .provenance
        .iter()
        .find(|row| row.input == PsiRealizationSite::Node(relocation.node().location()))
        .expect("the relocated computation has exact ledger custody");
    assert_eq!(
        row.disposition,
        ProvenanceDisposition::RealizedAt(PsiRealizationSite::Node(relocation.destination()))
    );
    assert_eq!(&row.sources, relocation.node().provenance());
    assert_eq!(&row.fuel, relocation.node().fuel());
    assert!(
        propose_loop_invariant_scalar_motion(applied.session(), 1)
            .expect("relocated session is an exact fixed point")
            .is_empty()
    );
}

#[test]
fn loop_carried_member_parameter_computation_stays_inside() {
    let session = lowered_session(CARRIED_MEMBER_SOURCE, "carried member loop");
    let [component] = session.cycle_components().components() else {
        panic!("one two-state component")
    };
    let function = session
        .unit()
        .functions
        .iter()
        .find(|function| function.machine == component.id.machine)
        .expect("component machine exists");
    let (_, addition, member_parameter) = member_addition(function, component);
    assert!(
        !crate::validation::invariant_member_parameters(function, component)
            .contains_key(&member_parameter),
        "the back edge advances the member parameter, so it stays loop-carried"
    );
    let addition_operation = match addition.provenance.first() {
        Some(PsiProvenance::Operation(operation)) => *operation,
        _ => panic!("computation carries its operation identity"),
    };
    let candidates =
        propose_loop_invariant_scalar_motion(&session, 1).expect("one exact relocation candidate");
    let [candidate] = candidates.as_slice() else {
        panic!("one component yields one atomic candidate")
    };
    assert!(
        candidate
            .relocations()
            .iter()
            .all(|relocation| relocation.node().psi_operation() != addition_operation),
        "the loop-carried member computation is not a planned relocation"
    );
}

#[test]
fn carried_member_computation_is_rejected_by_the_freeze_fence() {
    let session = lowered_session(CARRIED_MEMBER_SOURCE, "carried member loop");
    let [component] = session.cycle_components().components() else {
        panic!("one two-state component")
    };
    let [entry] = component.entries.as_slice() else {
        panic!("one entry edge")
    };
    let machine = component.id.machine;
    let function = session
        .unit()
        .functions
        .iter()
        .find(|function| function.machine == machine)
        .expect("component machine exists");
    let (member_block, addition, _) = member_addition(function, component);
    let member = member_block.id;
    let preheader = entry.source;
    let operation = match addition.provenance.first() {
        Some(PsiProvenance::Operation(operation)) => *operation,
        _ => panic!("computation carries its operation identity"),
    };
    let (input, mut unit) = session.into_parts();
    // Hand-move a computation whose operand is a loop-carried member
    // parameter: the relocation fence must reject it because no invariant
    // substitution exists, not merely because the shape differs.
    let moved = take_operation(&mut unit, operation);
    let preheader_block = unit
        .functions
        .iter_mut()
        .flat_map(|function| &mut function.blocks)
        .find(|candidate| candidate.id == preheader)
        .expect("preheader exists");
    let terminator = preheader_block.nodes.len() - 1;
    preheader_block.nodes.insert(terminator, moved);
    refresh_coordinates_and_effects(&mut unit);
    assert!(matches!(
        VerifiedPsiOptimizationSession::from_transformed(input, unit),
        Err(
            OptimizationUnitValidationError::RankedCycleFrozenBlockMismatch {
                machine: rejected_machine,
                block
            }
        ) if rejected_machine == machine && block == member
    ));
}

#[test]
fn forged_member_parameter_rewrite_is_rejected_by_the_freeze_fence() {
    let session = lowered_session(TRANSITIVE_MEMBER_SOURCE, "transitive member loop");
    let [component] = session.cycle_components().components() else {
        panic!("one two-state component")
    };
    let machine = component.id.machine;
    let candidate = propose_loop_invariant_scalar_motion(&session, 1)
        .expect("exact candidate")
        .pop()
        .expect("one candidate");
    let relocation = candidate
        .relocations()
        .iter()
        .find(|relocation| !relocation.node().operand_rewrites().is_empty())
        .expect("the member-parameter computation carries an operand rewrite");
    let member = relocation.node().location().block;
    let validated = validate_loop_invariant_scalar_motion(&session, &candidate)
        .expect("validated exact candidate");
    let applied =
        apply_loop_invariant_scalar_motion(session, validated).expect("applied exact candidate");
    let (input, mut unit) = applied.into_session().into_parts();
    // Forging the rebound operand back to the member parameter must fail the
    // seed-derived substitution, not just dominance bookkeeping.
    let forged = find_operation_mut(&mut unit, relocation.node().psi_operation());
    if let AbstractOperation::WrappingIntegerAdd { right, .. } = &mut forged.operation {
        *right = relocation.node().operand_rewrites()[0].0;
    }
    forged.uses[1].value = relocation.node().operand_rewrites()[0].0;
    unit.identity = recompute_psi_optimization_unit_identity(&unit);
    assert!(matches!(
        crate::validation::validate_transformed_psi_optimization_unit(&input, &unit),
        Err(
            OptimizationUnitValidationError::RankedCycleFrozenBlockMismatch {
                machine: rejected_machine,
                block
            }
        ) if rejected_machine == machine && block == member
    ));
}

/// The `s + s` computation inside a member block, its block, and the member
/// parameter it reads twice.
fn member_addition<'function>(
    function: &'function optimization_unit::PsiOptimizationFunction,
    component: &optimization_unit::OptimizerCycleComponent,
) -> (
    &'function optimization_unit::OptimizationBlock,
    &'function optimization_unit::OptimizationNode,
    ValueId,
) {
    for member in &component.members {
        let block = function
            .blocks
            .iter()
            .find(|block| block.id == *member)
            .expect("member block exists");
        for node in &block.nodes {
            if let AbstractOperation::WrappingIntegerAdd { left, right, .. } = &node.operation
                && left == right
                && block
                    .parameters
                    .iter()
                    .any(|parameter| parameter.value == *left)
            {
                return (block, node, *left);
            }
        }
    }
    panic!("the `s + s` computation lives in a member block")
}

fn take_operation(
    unit: &mut PsiOptimizationUnit,
    operation: semantic_vocabulary::OperationId,
) -> optimization_unit::OptimizationNode {
    for function in &mut unit.functions {
        for block in &mut function.blocks {
            if let Some(index) = block.nodes.iter().position(|node| {
                node.provenance.first() == Some(&PsiProvenance::Operation(operation))
            }) {
                return block.nodes.remove(index);
            }
        }
    }
    panic!("operation {operation:?} exists")
}

fn find_operation_mut(
    unit: &mut PsiOptimizationUnit,
    operation: semantic_vocabulary::OperationId,
) -> &mut optimization_unit::OptimizationNode {
    unit.functions
        .iter_mut()
        .flat_map(|function| &mut function.blocks)
        .flat_map(|block| &mut block.nodes)
        .find(|node| node.provenance.first() == Some(&PsiProvenance::Operation(operation)))
        .expect("operation exists")
}

fn refresh_coordinates_and_effects(unit: &mut PsiOptimizationUnit) {
    for function in &mut unit.functions {
        let mut effect = 0u64;
        for block in &mut function.blocks {
            for (node_index, node) in block.nodes.iter_mut().enumerate() {
                let node_index = u32::try_from(node_index).expect("test fixture fits u32");
                for definition in &mut node.definitions {
                    definition.site = optimization_unit::ValueDefinitionSite::Node {
                        block: block.id,
                        node: node_index,
                    };
                }
                for value_use in &mut node.uses {
                    value_use.block = block.id;
                    value_use.node = node_index;
                }
                node.effect = optimization_unit::EffectLink {
                    input: effect,
                    output: effect + 1,
                };
                effect += 1;
            }
        }
        let operation_order = function
            .blocks
            .iter()
            .flat_map(|block| &block.nodes)
            .enumerate()
            .filter_map(|(position, node)| match node.provenance.first() {
                Some(PsiProvenance::Operation(operation)) => Some((*operation, position)),
                _ => None,
            })
            .collect::<std::collections::BTreeMap<_, _>>();
        function.facts.sort_by_key(|fact| {
            let support = match fact {
                optimization_unit::OptimizationFact::OperationObligationReference {
                    support,
                    ..
                }
                | optimization_unit::OptimizationFact::BooleanConstant { support, .. }
                | optimization_unit::OptimizationFact::IntegerConstant { support, .. } => support,
            };
            operation_order.get(support).copied()
        });
    }
    unit.identity = recompute_psi_optimization_unit_identity(unit);
}

#[test]
fn bypassed_member_computation_is_speculation_and_stays_inside() {
    let session = lowered_session(BYPASSED_MEMBER_SOURCE, "bypassed member loop");
    let [component] = session.cycle_components().components() else {
        panic!("one two-state component")
    };
    let [entry] = component.entries.as_slice() else {
        panic!("one entry edge")
    };
    let function = session
        .unit()
        .functions
        .iter()
        .find(|function| function.machine == component.id.machine)
        .expect("component machine exists");
    let (member_block, addition, member_parameter) = member_addition(function, component);
    // Invariance is intact: `s` still resolves transitively to the preheader
    // anchor. The rejection is the profitability gate alone — `step` does not
    // dominate the entry state's own `done` exit, so relocating `s + s` into
    // the preheader would execute it on traversals that leave the component
    // without ever reaching `step`.
    let header = function
        .blocks
        .iter()
        .find(|block| block.id == entry.target)
        .expect("entry target exists");
    let entry_edge = function
        .blocks
        .iter()
        .flat_map(|block| block.nodes.iter().flat_map(|node| node.successors.iter()))
        .find(|edge| edge.psi_edge == entry.edge && edge.target == entry.target)
        .expect("the preheader terminator owns the entry edge");
    let anchor = entry_edge
        .bindings
        .iter()
        .find(|binding| binding.parameter == header.parameters[0].value)
        .expect("entry binds the carried header parameter")
        .argument;
    assert_eq!(
        crate::validation::invariant_member_parameters(function, component).get(&member_parameter),
        Some(&anchor),
        "the member parameter still resolves transitively to its preheader anchor"
    );
    assert!(
        !crate::validation::guaranteed_executed_member_blocks(component).contains(&member_block.id),
        "the bypassed member block is outside the non-speculative gate"
    );

    let candidates =
        propose_loop_invariant_scalar_motion(&session, 1).expect("one exact relocation candidate");
    let [candidate] = candidates.as_slice() else {
        panic!("the component still yields one atomic candidate")
    };
    let addition_operation = match addition.provenance.first() {
        Some(PsiProvenance::Operation(operation)) => *operation,
        _ => panic!("computation carries its operation identity"),
    };
    assert!(
        !candidate.relocations().is_empty()
            && candidate
                .relocations()
                .iter()
                .all(|relocation| relocation.node().operand_rewrites().is_empty()),
        "work-free scalar-constant leaves still relocate"
    );
    assert!(
        candidate
            .relocations()
            .iter()
            .all(|relocation| relocation.node().psi_operation() != addition_operation),
        "the speculated member computation is not a planned relocation"
    );

    // A forged candidate cannot smuggle the speculated relocation past
    // independent validation: replaying the gated plan yields a different
    // relocation roster and a different candidate identity.
    let mut forged = candidate.clone();
    forged.relocations.push(LoopInvariantScalarRelocation {
        node: crate::LoopInvariantScalarNode {
            psi_operation: addition_operation,
            result: match addition.definitions.as_slice() {
                [definition] => LoopInvariantNodeResult::Scalar {
                    value: definition.value,
                    scalar_type: definition.scalar_type,
                },
                _ => panic!("one defined result"),
            },
            location: NodeLocation {
                machine: component.id.machine,
                block: member_block.id,
                node: u32::try_from(
                    member_block
                        .nodes
                        .iter()
                        .position(|node| std::ptr::eq(node, addition))
                        .expect("the computation lives in its member block"),
                )
                .expect("node index fits u32"),
            },
            operand_rewrites: vec![(member_parameter, anchor)],
            root_rewrite: None,
            provenance: addition.provenance.clone(),
            fuel: addition.fuel.clone(),
        },
        destination: NodeLocation {
            machine: component.id.machine,
            block: entry.source,
            node: 0,
        },
    });
    assert!(matches!(
        validate_loop_invariant_scalar_motion(&session, &forged),
        Err(LoopInvariantScalarMotionError::CandidateMismatch)
            | Err(LoopInvariantScalarMotionError::StaleCandidateRevision { .. })
    ));

    // The gated plan itself still validates and applies atomically, and the
    // applied session is an exact fixed point: the retained computation stays
    // frozen inside the component while every leaf sits in the preheader.
    let validated = validate_loop_invariant_scalar_motion(&session, candidate)
        .expect("gated plan validates independently");
    let applied = apply_loop_invariant_scalar_motion(session, validated)
        .expect("gated plan applies atomically");
    assert!(
        propose_loop_invariant_scalar_motion(applied.session(), 1)
            .expect("relocated session is an exact fixed point")
            .is_empty()
    );
}

#[test]
fn conditional_entry_keeps_computations_inside_while_leaves_still_relocate() {
    let session = lowered_session_entry(
        CONDITIONAL_ENTRY_SOURCE,
        "conditional entry loop",
        "Root::enter",
    );
    let [component] = session.cycle_components().components() else {
        panic!("one self-loop component")
    };
    let function = session
        .unit()
        .functions
        .iter()
        .find(|function| function.machine == component.id.machine)
        .expect("component machine exists");
    // The member gate itself still qualifies the header, and `s` resolves to
    // its entry representative — only the two-successor preheader terminator
    // declines the computation's relocation.
    assert!(
        !crate::validation::guaranteed_executed_member_blocks(component).is_empty(),
        "member blocks qualify; the conditional entry is the only rejection"
    );
    let (_, addition, member_parameter) = member_addition(function, component);
    assert!(
        crate::validation::invariant_member_parameters(function, component)
            .contains_key(&member_parameter),
        "the member parameter is still provably invariant"
    );
    let addition_operation = match addition.provenance.first() {
        Some(PsiProvenance::Operation(operation)) => *operation,
        _ => panic!("computation carries its operation identity"),
    };
    let candidates =
        propose_loop_invariant_scalar_motion(&session, 1).expect("one exact relocation candidate");
    let [candidate] = candidates.as_slice() else {
        panic!("the component still yields one atomic candidate")
    };
    assert!(
        !candidate.relocations().is_empty()
            && candidate
                .relocations()
                .iter()
                .all(|relocation| relocation.node().operand_rewrites().is_empty()),
        "only work-free scalar-constant leaves relocate past a conditional entry"
    );
    assert!(
        candidate
            .relocations()
            .iter()
            .all(|relocation| relocation.node().psi_operation() != addition_operation),
        "the speculated computation is not a planned relocation"
    );
}

/// The roster this boundary iterates is the validated Terminal SCC itself:
/// the retained `Natural` row's rank roster is the component's member set and
/// its edge rows are the component's internal-edge identity, and the same
/// custody still carries the `scale + scale` relocation end to end.
#[test]
fn ranked_natural_component_is_the_validated_terminal_scc() {
    let session = lowered_session_entry(RANKED_MEMBER_SOURCE, "ranked member loop", "Root::scan");
    let [component] = session.cycle_components().components() else {
        panic!("one validated cyclic component")
    };
    let machine = session
        .input()
        .context()
        .module()
        .machines
        .iter()
        .find(|machine| machine.id == component.id.machine)
        .expect("component machine exists in the authenticated module");
    let terminal_psi::TerminalRankedScc::Natural(naturals) = machine
        .ranked_scc
        .as_ref()
        .expect("the source loop retains validated Natural ranking evidence");
    let [natural] = naturals.as_slice() else {
        panic!("one validated natural component")
    };
    assert_eq!(
        component.members,
        natural
            .ranks
            .iter()
            .map(|rank| rank.block)
            .collect::<Vec<_>>(),
        "component membership is the validated Terminal SCC roster"
    );
    assert_eq!(
        component
            .id
            .internal_edges
            .iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>(),
        natural
            .edges
            .iter()
            .map(|edge| optimization_unit::CycleComponentEdge {
                edge: edge.edge,
                source: edge.source,
                target: edge.target,
            })
            .collect::<std::collections::BTreeSet<_>>(),
        "component identity carries the validated internal-edge set"
    );
    assert!(
        session.ranking_certificates().certificates().is_empty(),
        "slice-length Natural custody carries no countdown certificate"
    );

    let [entry] = component.entries.as_slice() else {
        panic!("one entry edge")
    };
    let function = session
        .unit()
        .functions
        .iter()
        .find(|function| function.machine == component.id.machine)
        .expect("component machine exists");
    let (member_block, addition, _) = member_addition(function, component);
    let addition_operation = match addition.provenance.first() {
        Some(PsiProvenance::Operation(operation)) => *operation,
        _ => panic!("computation carries its operation identity"),
    };

    let candidates =
        propose_loop_invariant_scalar_motion(&session, 1).expect("one exact relocation candidate");
    let [candidate] = candidates.as_slice() else {
        panic!("the validated component yields one atomic candidate")
    };
    let relocation = candidate
        .relocations()
        .iter()
        .find(|relocation| relocation.node().psi_operation() == addition_operation)
        .expect("the invariant computation is a planned relocation");
    assert_eq!(relocation.node().location().block, member_block.id);
    assert_eq!(relocation.destination().block, entry.source);

    let component_id = component.id.clone();
    let component_members = component.members.clone();
    let validated = validate_loop_invariant_scalar_motion(&session, candidate)
        .expect("independent relocation validation");
    let applied = apply_loop_invariant_scalar_motion(session, validated)
        .expect("atomic relocation application");
    // Custody survives the transform: the relocated session still iterates
    // the same validated Terminal SCC roster.
    let [applied_component] = applied.session().cycle_components().components() else {
        panic!("the transformed session retains one validated component")
    };
    assert_eq!(applied_component.id, component_id);
    assert_eq!(applied_component.members, component_members);
    assert!(
        propose_loop_invariant_scalar_motion(applied.session(), 1)
            .expect("relocated session is an exact fixed point")
            .is_empty()
    );
}

/// The `IntegerStructuralField` observations inside a component's member
/// blocks, as `(member block, node)` pairs in member order.
fn member_field_reads<'function>(
    function: &'function optimization_unit::PsiOptimizationFunction,
    component: &optimization_unit::OptimizerCycleComponent,
) -> Vec<(
    &'function optimization_unit::OptimizationBlock,
    &'function optimization_unit::OptimizationNode,
)> {
    component
        .members
        .iter()
        .map(|member| {
            function
                .blocks
                .iter()
                .find(|block| block.id == *member)
                .expect("member block exists")
        })
        .flat_map(|block| {
            block
                .nodes
                .iter()
                .filter(|node| {
                    matches!(
                        node.operation,
                        AbstractOperation::IntegerStructuralField { .. }
                    )
                })
                .map(move |node| (block, node))
        })
        .collect()
}

#[test]
fn invariant_place_observation_relocates_with_its_chained_computation() {
    let session = lowered_session(OBSERVED_FIELD_SOURCE, "observed field loop");
    let [component] = session.cycle_components().components() else {
        panic!("one self-loop component")
    };
    let [entry] = component.entries.as_slice() else {
        panic!("one entry edge")
    };
    let machine = component.id.machine;
    let preheader = entry.source;
    let function = session
        .unit()
        .functions
        .iter()
        .find(|function| function.machine == machine)
        .expect("component machine exists");
    let reads = member_field_reads(function, component);
    let [(member_block, read)] = reads.as_slice() else {
        panic!("one member field observation")
    };
    let member = member_block.id;
    let (read_source, read_field) = match &read.operation {
        AbstractOperation::IntegerStructuralField { source, field, .. } => (*source, *field),
        _ => unreachable!("member_field_reads only yields field observations"),
    };
    assert!(
        function
            .structural_parameters
            .iter()
            .any(|parameter| parameter.place == read_source),
        "the observed root is the function's `self` structural parameter"
    );
    assert!(
        crate::validation::component_preserves_place_observations(function, component),
        "no member mutates or moves custody of any place"
    );
    assert_eq!(
        crate::validation::invariant_place_observation_admission(function, component, read),
        Some(read_source),
        "the field observation passes the shared admission and keeps its own root"
    );
    let read_operation = match read.provenance.first() {
        Some(PsiProvenance::Operation(operation)) => *operation,
        _ => panic!("the observation carries its operation identity"),
    };
    let read_result = match read.definitions.as_slice() {
        [definition] => definition.value,
        _ => panic!("one observed result"),
    };
    let addition = member_block
        .nodes
        .iter()
        .find(|node| matches!(node.operation, AbstractOperation::WrappingIntegerAdd { .. }))
        .expect("the observed value feeds a scalar computation");
    let addition_operation = match addition.provenance.first() {
        Some(PsiProvenance::Operation(operation)) => *operation,
        _ => panic!("the computation carries its operation identity"),
    };
    match &addition.operation {
        AbstractOperation::WrappingIntegerAdd { left, .. } => assert_eq!(*left, read_result),
        _ => unreachable!("the add reads the observed result"),
    }
    let expected_parameters = function.structural_parameters.clone();

    let candidates =
        propose_loop_invariant_scalar_motion(&session, 1).expect("one exact relocation candidate");
    let [candidate] = candidates.as_slice() else {
        panic!("one component yields one atomic candidate")
    };
    let relocation = candidate
        .relocations()
        .iter()
        .find(|relocation| relocation.node().psi_operation() == read_operation)
        .expect("the place observation is a planned relocation");
    assert!(
        relocation.node().operand_rewrites().is_empty(),
        "the observed root already names a preheader-visible place"
    );
    assert_eq!(relocation.node().result().scalar_value(), Some(read_result));
    assert_eq!(relocation.node().location().block, member);
    assert_eq!(relocation.destination().block, preheader);
    let chained = candidate
        .relocations()
        .iter()
        .find(|relocation| relocation.node().psi_operation() == addition_operation)
        .expect("the computation chained on the observation relocates in the same run");
    assert!(
        chained.node().operand_rewrites().is_empty(),
        "the run preserves the observed result's value identity"
    );

    let validated = validate_loop_invariant_scalar_motion(&session, candidate)
        .expect("independent relocation validation");
    let applied = apply_loop_invariant_scalar_motion(session, validated)
        .expect("atomic relocation application");
    let output_function = applied
        .session()
        .unit()
        .functions
        .iter()
        .find(|function| function.machine == machine)
        .expect("component machine exists");
    let destination = output_function
        .blocks
        .iter()
        .find(|block| block.id == relocation.destination().block)
        .expect("destination block exists");
    let moved = &destination.nodes[usize::try_from(relocation.destination().node).unwrap()];
    match &moved.operation {
        AbstractOperation::IntegerStructuralField {
            source,
            field,
            result,
            ..
        } => {
            assert_eq!(
                *source, read_source,
                "the relocated observation keeps its root"
            );
            assert_eq!(*field, read_field);
            assert_eq!(result.value, read_result);
        }
        operation => panic!("relocated observation keeps its operation: {operation:?}"),
    }
    assert_eq!(moved.provenance, relocation.node().provenance());
    assert_eq!(moved.fuel, relocation.node().fuel());
    assert!(
        output_function
            .blocks
            .iter()
            .find(|block| block.id == member)
            .expect("member block exists")
            .nodes
            .iter()
            .all(|node| !matches!(
                node.operation,
                AbstractOperation::IntegerStructuralField { .. }
            )),
        "the observation exists once, at the destination"
    );
    // Place custody survives the transform: the root stays declared and the
    // function's structural-parameter roster is unchanged.
    assert!(output_function.declared_places.contains(&read_source));
    assert_eq!(output_function.structural_parameters, expected_parameters);

    let [record] = applied.ledger().records() else {
        panic!("one atomic relocation has one ledger record")
    };
    let row = record
        .provenance
        .iter()
        .find(|row| row.input == PsiRealizationSite::Node(relocation.node().location()))
        .expect("the relocated observation has exact ledger custody");
    assert_eq!(
        row.disposition,
        ProvenanceDisposition::RealizedAt(PsiRealizationSite::Node(relocation.destination()))
    );
    assert_eq!(&row.sources, relocation.node().provenance());
    assert_eq!(&row.fuel, relocation.node().fuel());
    assert!(
        propose_loop_invariant_scalar_motion(applied.session(), 1)
            .expect("relocated session is an exact fixed point")
            .is_empty()
    );
}

#[test]
fn member_store_keeps_place_observations_inside() {
    let session = lowered_session(OBSERVED_FIELD_MUTATED_SOURCE, "mutated field loop");
    let [component] = session.cycle_components().components() else {
        panic!("one self-loop component")
    };
    let function = session
        .unit()
        .functions
        .iter()
        .find(|function| function.machine == component.id.machine)
        .expect("component machine exists");
    assert!(
        !crate::validation::component_preserves_place_observations(function, component),
        "the member store ends the component's place-custody preservation"
    );
    let reads = member_field_reads(function, component);
    assert_eq!(reads.len(), 2, "the member observes both `self` fields");
    let read_operations = reads
        .iter()
        .map(|(_, node)| match node.provenance.first() {
            Some(PsiProvenance::Operation(operation)) => *operation,
            _ => panic!("the observation carries its operation identity"),
        })
        .collect::<Vec<_>>();
    for (_, read) in &reads {
        assert!(
            crate::validation::invariant_place_observation_admission(function, component, read)
                .is_none(),
            "the shared admission refuses the observation"
        );
    }

    let candidates =
        propose_loop_invariant_scalar_motion(&session, 1).expect("exact relocation candidates");
    let [candidate] = candidates.as_slice() else {
        panic!("the component still yields one atomic candidate")
    };
    assert!(
        !candidate.relocations().is_empty()
            && candidate
                .relocations()
                .iter()
                .all(|relocation| relocation.node().operand_rewrites().is_empty()),
        "work-free scalar-constant leaves still relocate"
    );
    assert!(
        candidate
            .relocations()
            .iter()
            .all(|relocation| !read_operations.contains(&relocation.node().psi_operation())),
        "no observation relocates out of a mutating component"
    );
}

#[test]
fn mutated_component_read_is_rejected_by_the_freeze_fence() {
    let session = lowered_session(OBSERVED_FIELD_MUTATED_SOURCE, "mutated field loop");
    let [component] = session.cycle_components().components() else {
        panic!("one self-loop component")
    };
    let [entry] = component.entries.as_slice() else {
        panic!("one entry edge")
    };
    let machine = component.id.machine;
    let preheader = entry.source;
    let function = session
        .unit()
        .functions
        .iter()
        .find(|function| function.machine == machine)
        .expect("component machine exists");
    let (member_block, read) = member_field_reads(function, component)[0];
    let member = member_block.id;
    let read_operation = match read.provenance.first() {
        Some(PsiProvenance::Operation(operation)) => *operation,
        _ => panic!("the observation carries its operation identity"),
    };
    let (input, mut unit) = session.into_parts();
    // Hand-move a field observation out of a component that stores through a
    // reachable root: the relocation fence must refuse it because the
    // seed-derived custody gate fails, not merely because the shape differs.
    let moved = take_operation(&mut unit, read_operation);
    let preheader_block = unit
        .functions
        .iter_mut()
        .flat_map(|function| &mut function.blocks)
        .find(|candidate| candidate.id == preheader)
        .expect("preheader exists");
    let terminator = preheader_block.nodes.len() - 1;
    preheader_block.nodes.insert(terminator, moved);
    refresh_coordinates_and_effects(&mut unit);
    assert!(matches!(
        VerifiedPsiOptimizationSession::from_transformed(input, unit),
        Err(
            OptimizationUnitValidationError::RankedCycleFrozenBlockMismatch {
                machine: rejected_machine,
                block
            }
        ) if rejected_machine == machine && block == member
    ));
}

#[test]
fn bypassed_member_place_observation_stays_inside() {
    let session = lowered_session(BYPASSED_OBSERVED_FIELD_SOURCE, "bypassed read loop");
    let [component] = session.cycle_components().components() else {
        panic!("one two-state component")
    };
    let function = session
        .unit()
        .functions
        .iter()
        .find(|function| function.machine == component.id.machine)
        .expect("component machine exists");
    assert!(
        crate::validation::component_preserves_place_observations(function, component),
        "the custody gate is intact; only speculation refuses the observation"
    );
    let (observing_block, read) = member_field_reads(function, component)[0];
    assert!(
        !crate::validation::guaranteed_executed_member_blocks(component)
            .contains(&observing_block.id),
        "the observing member does not dominate every exit"
    );
    assert!(
        crate::validation::admissible_invariant_place_read(read).is_some(),
        "the observation's own shape is admitted"
    );
    let read_operation = match read.provenance.first() {
        Some(PsiProvenance::Operation(operation)) => *operation,
        _ => panic!("the observation carries its operation identity"),
    };

    let candidates =
        propose_loop_invariant_scalar_motion(&session, 1).expect("exact relocation candidates");
    let [candidate] = candidates.as_slice() else {
        panic!("the component still yields one atomic candidate")
    };
    assert!(
        !candidate.relocations().is_empty(),
        "work-free scalar-constant leaves still relocate"
    );
    assert!(
        candidate
            .relocations()
            .iter()
            .all(|relocation| relocation.node().psi_operation() != read_operation),
        "the speculated observation is not a planned relocation"
    );
}

/// The `ByteSequenceLength` observations inside a component's member blocks,
/// as `(member block, node)` pairs in member order.
fn member_length_reads<'function>(
    function: &'function optimization_unit::PsiOptimizationFunction,
    component: &optimization_unit::OptimizerCycleComponent,
) -> Vec<(
    &'function optimization_unit::OptimizationBlock,
    &'function optimization_unit::OptimizationNode,
)> {
    component
        .members
        .iter()
        .map(|member| {
            function
                .blocks
                .iter()
                .find(|block| block.id == *member)
                .expect("member block exists")
        })
        .flat_map(|block| {
            block
                .nodes
                .iter()
                .filter(|node| {
                    matches!(node.operation, AbstractOperation::ByteSequenceLength { .. })
                })
                .map(move |node| (block, node))
        })
        .collect()
}

#[test]
fn member_view_parameter_observation_relocates_rebinding_its_root() {
    let session = lowered_session(MEMBER_VIEW_SOURCE, "member view loop");
    let [component] = session.cycle_components().components() else {
        panic!("one self-loop component")
    };
    let [entry] = component.entries.as_slice() else {
        panic!("one entry edge")
    };
    let machine = component.id.machine;
    let preheader = entry.source;
    let function = session
        .unit()
        .functions
        .iter()
        .find(|function| function.machine == machine)
        .expect("component machine exists");
    let member = component.members[0];
    let member_block = function
        .blocks
        .iter()
        .find(|block| block.id == member)
        .expect("member block exists");
    let [view_parameter] = member_block.structural_parameters.as_slice() else {
        panic!("the member carries one view structural parameter")
    };
    let [entries_parameter] = function.structural_parameters.as_slice() else {
        panic!("the machine carries one structural parameter")
    };
    assert_eq!(
        crate::validation::invariant_member_place_parameters(function, component)
            .get(&view_parameter.place),
        Some(&entries_parameter.place),
        "the member view parameter resolves transitively to the machine's `entries` root"
    );
    let reads = member_length_reads(function, component);
    let [(_, read)] = reads.as_slice() else {
        panic!("one member length observation")
    };
    assert_eq!(
        crate::validation::admissible_invariant_place_read(read),
        Some(view_parameter.place),
        "the observation reads through the member view parameter"
    );
    assert_eq!(
        crate::validation::invariant_place_observation_admission(function, component, read),
        Some(entries_parameter.place),
        "the admission rebinds the observed root to the preheader-visible representative"
    );
    assert!(
        crate::validation::component_preserves_place_observations(function, component),
        "no member mutates or moves custody of any place"
    );
    let read_operation = match read.provenance.first() {
        Some(PsiProvenance::Operation(operation)) => *operation,
        _ => panic!("the observation carries its operation identity"),
    };
    let read_result = match read.definitions.as_slice() {
        [definition] => definition.value,
        _ => panic!("one observed result"),
    };

    let candidates =
        propose_loop_invariant_scalar_motion(&session, 1).expect("one exact relocation candidate");
    let [candidate] = candidates.as_slice() else {
        panic!("one component yields one atomic candidate")
    };
    let relocation = candidate
        .relocations()
        .iter()
        .find(|relocation| relocation.node().psi_operation() == read_operation)
        .expect("the member view observation is a planned relocation");
    assert_eq!(
        relocation.node().root_rewrite(),
        Some((view_parameter.place, entries_parameter.place)),
        "the relocation records the member parameter's root rebind"
    );
    assert!(
        relocation.node().operand_rewrites().is_empty(),
        "a place observation rebinds its root, not a scalar operand"
    );
    assert_eq!(relocation.node().result().scalar_value(), Some(read_result));
    assert_eq!(relocation.node().location().block, member);
    assert_eq!(relocation.destination().block, preheader);
    // The scalar member parameter still relocates through its own path in the
    // same run: `s + s` rebinds `s` to `scale`, not through a root rewrite.
    let addition = member_block
        .nodes
        .iter()
        .find(|node| matches!(node.operation, AbstractOperation::WrappingIntegerAdd { .. }))
        .expect("the member carries the scalar computation");
    let addition_operation = match addition.provenance.first() {
        Some(PsiProvenance::Operation(operation)) => *operation,
        _ => panic!("the computation carries its operation identity"),
    };
    let chained = candidate
        .relocations()
        .iter()
        .find(|relocation| relocation.node().psi_operation() == addition_operation)
        .expect("the scalar member-parameter computation relocates in the same run");
    assert_eq!(chained.node().root_rewrite(), None);
    assert_eq!(chained.node().operand_rewrites().len(), 1);
    let view_place = view_parameter.place;
    let entries_place = entries_parameter.place;
    let member_structural_parameters = member_block.structural_parameters.clone();

    let validated = validate_loop_invariant_scalar_motion(&session, candidate)
        .expect("independent relocation validation");
    let applied = apply_loop_invariant_scalar_motion(session, validated)
        .expect("atomic relocation application");
    let output_function = applied
        .session()
        .unit()
        .functions
        .iter()
        .find(|function| function.machine == machine)
        .expect("component machine exists");
    let destination = output_function
        .blocks
        .iter()
        .find(|block| block.id == relocation.destination().block)
        .expect("destination block exists");
    let moved = &destination.nodes[usize::try_from(relocation.destination().node).unwrap()];
    match &moved.operation {
        AbstractOperation::ByteSequenceLength { source, .. } => {
            assert_eq!(
                *source, entries_place,
                "the relocated observation reads the representative root"
            );
        }
        operation => panic!("relocated observation keeps its operation: {operation:?}"),
    }
    assert_eq!(moved.provenance, relocation.node().provenance());
    assert_eq!(moved.fuel, relocation.node().fuel());
    assert!(
        output_function
            .blocks
            .iter()
            .find(|block| block.id == member)
            .expect("member block exists")
            .nodes
            .iter()
            .all(|node| !matches!(node.operation, AbstractOperation::ByteSequenceLength { .. })),
        "the observation exists once, at the destination"
    );
    // Place custody survives the transform: both roots stay declared and the
    // member's view parameter roster is unchanged — the moved observation
    // reads the representative, it does not rebind the parameter itself.
    assert!(output_function.declared_places.contains(&view_place));
    assert!(output_function.declared_places.contains(&entries_place));
    assert_eq!(
        output_function
            .blocks
            .iter()
            .find(|block| block.id == member)
            .expect("member block exists")
            .structural_parameters,
        member_structural_parameters,
    );

    let [record] = applied.ledger().records() else {
        panic!("one atomic relocation has one ledger record")
    };
    let row = record
        .provenance
        .iter()
        .find(|row| row.input == PsiRealizationSite::Node(relocation.node().location()))
        .expect("the relocated observation has exact ledger custody");
    assert_eq!(
        row.disposition,
        ProvenanceDisposition::RealizedAt(PsiRealizationSite::Node(relocation.destination()))
    );
    assert_eq!(&row.sources, relocation.node().provenance());
    assert_eq!(&row.fuel, relocation.node().fuel());
    assert!(
        propose_loop_invariant_scalar_motion(applied.session(), 1)
            .expect("relocated session is an exact fixed point")
            .is_empty()
    );
}

#[test]
fn member_view_parameters_resolve_across_member_edges() {
    let session = lowered_session(TRANSITIVE_VIEW_SOURCE, "transitive view loop");
    let [component] = session.cycle_components().components() else {
        panic!("one two-member component")
    };
    let [entry] = component.entries.as_slice() else {
        panic!("one entry edge")
    };
    let machine = component.id.machine;
    let preheader = entry.source;
    let function = session
        .unit()
        .functions
        .iter()
        .find(|function| function.machine == machine)
        .expect("component machine exists");
    let [entries_parameter] = function.structural_parameters.as_slice() else {
        panic!("the machine carries one structural parameter")
    };
    let reads = member_length_reads(function, component);
    let [(_, step_read), (_, swap_read)] = reads.as_slice() else {
        panic!("each member observes its view length once")
    };
    // Both member view parameters resolve through each other to `entries`:
    // `step.view` chains `swap.other` and `swap.other` chains `step.view`,
    // with the entry edge's `entries` anchor breaking the tie.
    for (_, read) in &reads {
        assert_eq!(
            crate::validation::invariant_place_observation_admission(function, component, read),
            Some(entries_parameter.place),
            "each member view observation resolves its root to `entries`"
        );
    }
    let read_operations: Vec<_> = [step_read, swap_read]
        .iter()
        .map(|read| match read.provenance.first() {
            Some(PsiProvenance::Operation(operation)) => *operation,
            _ => panic!("each observation carries its operation identity"),
        })
        .collect();

    let candidates =
        propose_loop_invariant_scalar_motion(&session, 1).expect("one exact relocation candidate");
    let [candidate] = candidates.as_slice() else {
        panic!("one component yields one atomic candidate")
    };
    let relocations: Vec<_> = read_operations
        .iter()
        .map(|operation| {
            candidate
                .relocations()
                .iter()
                .find(|relocation| relocation.node().psi_operation() == *operation)
                .expect("each member view observation is a planned relocation")
        })
        .collect();
    for relocation in &relocations {
        let (parameter, representative) = relocation
            .node()
            .root_rewrite()
            .expect("each relocation carries a root rewrite");
        assert_eq!(representative, entries_parameter.place);
        assert_eq!(
            Some(parameter),
            crate::validation::admissible_invariant_place_read(find_member_node(
                function,
                relocation.node().location()
            )),
            "each relocation rebinds its member parameter root to `entries`"
        );
        assert_eq!(relocation.destination().block, preheader);
    }
    let entries_place = entries_parameter.place;

    let validated = validate_loop_invariant_scalar_motion(&session, candidate)
        .expect("independent relocation validation");
    let applied = apply_loop_invariant_scalar_motion(session, validated)
        .expect("atomic relocation application");
    let output_function = applied
        .session()
        .unit()
        .functions
        .iter()
        .find(|function| function.machine == machine)
        .expect("component machine exists");
    for relocation in relocations {
        let destination = output_function
            .blocks
            .iter()
            .find(|block| block.id == relocation.destination().block)
            .expect("destination block exists");
        let moved = &destination.nodes[usize::try_from(relocation.destination().node).unwrap()];
        match &moved.operation {
            AbstractOperation::ByteSequenceLength { source, .. } => {
                assert_eq!(*source, entries_place);
            }
            operation => panic!("relocated observation keeps its operation: {operation:?}"),
        }
        assert!(
            output_function
                .blocks
                .iter()
                .find(|block| block.id == relocation.node().location().block)
                .expect("member block exists")
                .nodes
                .iter()
                .all(|node| !matches!(
                    node.operation,
                    AbstractOperation::ByteSequenceLength { .. }
                )),
            "the member's observation exists once, at the destination"
        );
    }
    assert!(
        propose_loop_invariant_scalar_motion(applied.session(), 1)
            .expect("relocated session is an exact fixed point")
            .is_empty()
    );
}

fn find_member_node<'function>(
    function: &'function optimization_unit::PsiOptimizationFunction,
    site: NodeLocation,
) -> &'function optimization_unit::OptimizationNode {
    function
        .blocks
        .iter()
        .find(|block| block.id == site.block)
        .expect("member block exists")
        .nodes
        .get(usize::try_from(site.node).unwrap())
        .expect("member node exists")
}

#[test]
fn member_produced_view_parameter_stays_loop_carried() {
    let session = lowered_session(CARRIED_VIEW_SOURCE, "carried view loop");
    let [component] = session.cycle_components().components() else {
        panic!("one self-loop component")
    };
    let function = session
        .unit()
        .functions
        .iter()
        .find(|function| function.machine == component.id.machine)
        .expect("component machine exists");
    let observing = function
        .blocks
        .iter()
        .find(|block| {
            block
                .nodes
                .iter()
                .any(|node| matches!(node.operation, AbstractOperation::ByteSequenceLength { .. }))
        })
        .expect("one member observes the view length");
    let [view_parameter] = observing.structural_parameters.as_slice() else {
        panic!("the observing member carries one view structural parameter")
    };
    assert!(
        !crate::validation::invariant_member_place_parameters(function, component)
            .contains_key(&view_parameter.place),
        "a member-produced back-edge binding keeps the member view parameter loop-carried"
    );
    // The member's own subslice establishment no longer closes the custody
    // gate: it reads the carried root's extent without mutating it and its
    // fresh view is a member-produced root that can never anchor another
    // parameter's invariant representative. The refusal for this component
    // lives entirely in the root resolution — `view` is bound to a
    // member-produced place on the back edge, so it stays loop-carried and
    // every observation through it refuses below.
    assert!(
        crate::validation::component_preserves_place_observations(function, component),
        "establishing a fresh view mutates no established place"
    );
    let reads = member_length_reads(function, component);
    let read_operations: Vec<_> = reads
        .iter()
        .map(|(_, read)| {
            assert!(
                crate::validation::invariant_place_observation_admission(function, component, read)
                    .is_none(),
                "the shared admission refuses each observation on the carried root"
            );
            match read.provenance.first() {
                Some(PsiProvenance::Operation(operation)) => *operation,
                _ => panic!("each observation carries its operation identity"),
            }
        })
        .collect();
    assert!(
        !read_operations.is_empty(),
        "the member observes the carried view"
    );

    let candidates =
        propose_loop_invariant_scalar_motion(&session, 1).expect("exact relocation candidates");
    let [candidate] = candidates.as_slice() else {
        panic!("the component still yields one atomic candidate")
    };
    assert!(
        !candidate.relocations().is_empty(),
        "scalar work still relocates out of the same component"
    );
    assert!(
        candidate
            .relocations()
            .iter()
            .all(|relocation| { !read_operations.contains(&relocation.node().psi_operation()) }),
        "the loop-carried view observations are not planned relocations"
    );
    assert!(
        candidate
            .relocations()
            .iter()
            .all(|relocation| relocation.node().root_rewrite().is_none()),
        "no relocation carries a root rewrite"
    );
    // The scalar member parameter is genuinely invariant — every reaching edge
    // resolves `s` back to `scale` — so `s + s` still relocates through the
    // ordinary scalar path while the carried view stays inside.
    let addition = observing
        .nodes
        .iter()
        .find(|node| matches!(node.operation, AbstractOperation::WrappingIntegerAdd { .. }))
        .expect("the observing member carries the scalar computation");
    let addition_operation = match addition.provenance.first() {
        Some(PsiProvenance::Operation(operation)) => *operation,
        _ => panic!("the computation carries its operation identity"),
    };
    assert!(
        candidate
            .relocations()
            .iter()
            .any(|relocation| relocation.node().psi_operation() == addition_operation),
        "the invariant scalar computation still relocates"
    );
}

#[test]
fn carried_member_view_observation_is_rejected_by_the_freeze_fence() {
    let session = lowered_session(CARRIED_VIEW_SOURCE, "carried view loop");
    let [component] = session.cycle_components().components() else {
        panic!("one self-loop component")
    };
    let [entry] = component.entries.as_slice() else {
        panic!("one entry edge")
    };
    let machine = component.id.machine;
    let preheader = entry.source;
    let function = session
        .unit()
        .functions
        .iter()
        .find(|function| function.machine == machine)
        .expect("component machine exists");
    let (observing_block, read) = member_length_reads(function, component)[0];
    let member = observing_block.id;
    let read_operation = match read.provenance.first() {
        Some(PsiProvenance::Operation(operation)) => *operation,
        _ => panic!("the observation carries its operation identity"),
    };
    let (input, mut unit) = session.into_parts();
    // Hand-move a view observation whose member parameter never resolves: the
    // relocation fence must refuse it because no representative exists, not
    // merely because the shape differs.
    let moved = take_operation(&mut unit, read_operation);
    let preheader_block = unit
        .functions
        .iter_mut()
        .flat_map(|function| &mut function.blocks)
        .find(|candidate| candidate.id == preheader)
        .expect("preheader exists");
    let terminator = preheader_block.nodes.len() - 1;
    preheader_block.nodes.insert(terminator, moved);
    refresh_coordinates_and_effects(&mut unit);
    assert!(matches!(
        VerifiedPsiOptimizationSession::from_transformed(input, unit),
        Err(
            OptimizationUnitValidationError::RankedCycleFrozenBlockMismatch {
                machine: rejected_machine,
                block
            }
        ) if rejected_machine == machine && block == member
    ));
}

#[test]
fn forged_view_root_rewrite_is_rejected_by_the_freeze_fence() {
    let session = lowered_session(MEMBER_VIEW_SOURCE, "member view loop");
    let [component] = session.cycle_components().components() else {
        panic!("one self-loop component")
    };
    let machine = component.id.machine;
    let candidate = propose_loop_invariant_scalar_motion(&session, 1)
        .expect("exact candidate")
        .pop()
        .expect("one candidate");
    let relocation = candidate
        .relocations()
        .iter()
        .find(|relocation| relocation.node().root_rewrite().is_some())
        .expect("the member view observation carries a root rewrite");
    let member = relocation.node().location().block;
    let (parameter, _) = relocation
        .node()
        .root_rewrite()
        .expect("root rewrite exists");
    let validated = validate_loop_invariant_scalar_motion(&session, &candidate)
        .expect("validated exact candidate");
    let applied =
        apply_loop_invariant_scalar_motion(session, validated).expect("applied exact candidate");
    let (input, mut unit) = applied.into_session().into_parts();
    // Forging the rebound root back to the member view parameter must fail the
    // seed-derived resolution, not just dominance bookkeeping.
    let forged = find_operation_mut(&mut unit, relocation.node().psi_operation());
    if let AbstractOperation::ByteSequenceLength { source, .. } = &mut forged.operation {
        *source = parameter;
    }
    unit.identity = recompute_psi_optimization_unit_identity(&unit);
    assert!(matches!(
        crate::validation::validate_transformed_psi_optimization_unit(&input, &unit),
        Err(
            OptimizationUnitValidationError::RankedCycleFrozenBlockMismatch {
                machine: rejected_machine,
                block
            }
        ) if rejected_machine == machine && block == member
    ));
}

/// A byte read whose operands exercise every substitution half at once:
/// `view[i]` reads through the member `view` structural parameter that
/// resolves transitively to the machine's `entries` root, through the member
/// `i` scalar parameter that resolves transitively to the `index` machine
/// parameter, and its `length` operand is the member-internal
/// `ByteSequenceLength` (`view.len`) that must relocate in the same run —
/// its own root rebinds to `entries` so the moved read still validates
/// byte-exact against its preserved bounds obligation. The read lives in a
/// member block that dominates the component's only exit, so the
/// non-speculative gate admits it.
const MEMBER_BYTE_READ_SOURCE: &str = r#"
    data Root {}

    machine Root::scan(scale: u32 in Wrapping, entries: &[u8], index: u64, remaining: u32 [0..=5])
    {
        transition { _ -> step(scale, entries, index, remaining) }
        state step(s: u32 in Wrapping, view: &[u8], i: u64, pending: u32 [0..=5]) {
            transition i < view.len && view[i] == 0 {
                true -> tail(s, view, i, pending)
                _ -> hold(s, view, i, pending)
            }
        }
        state tail(s: u32 in Wrapping, view: &[u8], i: u64, pending: u32 [0..=5]) {
            transition pending > 0 {
                true -> step(s, view, i, pending - 1)
                _ -> finish()
            }
        }
        state hold(s: u32 in Wrapping, view: &[u8], i: u64, pending: u32 [0..=5]) {
            transition { _ -> step(s, view, i, pending) }
        }
        state finish() {}
    }
"#;

/// Same byte-read shape, but the back edge binds `i` to a fresh
/// member-internal constant, so the member `i` parameter stays loop-carried:
/// the read's `index` operand has no invariant representative and the read
/// stays inside even though its root still resolves to `entries`.
const CARRIED_INDEX_BYTE_READ_SOURCE: &str = r#"
    data Root {}

    machine Root::scan(scale: u32 in Wrapping, entries: &[u8], index: u64, remaining: u32 [0..=5])
    {
        transition { _ -> step(scale, entries, index, remaining) }
        state step(s: u32 in Wrapping, view: &[u8], i: u64, pending: u32 [0..=5]) {
            transition i < view.len && view[i] == 0 {
                true -> tail(s, view, i, pending)
                _ -> hold(s, view, i, pending)
            }
        }
        state tail(s: u32 in Wrapping, view: &[u8], i: u64, pending: u32 [0..=5]) {
            transition pending > 0 {
                true -> step(s, view, 0, pending - 1)
                _ -> finish()
            }
        }
        state hold(s: u32 in Wrapping, view: &[u8], i: u64, pending: u32 [0..=5]) {
            transition { _ -> step(s, view, i, pending) }
        }
        state finish() {}
    }
"#;

/// Same byte-read shape, but `tail`'s back edge binds `view` to a different
/// root than the entry edge: `entries` on entry and `fallback` on the back
/// edge disagree, so the member `view` parameter resolves to no single
/// representative and stays loop-carried — the read stays inside even though
/// the component's place-custody preservation is intact.
const CARRIED_VIEW_BYTE_READ_SOURCE: &str = r#"
    data Root {}

    machine Root::scan(scale: u32 in Wrapping, entries: &[u8], fallback: &[u8], index: u64, remaining: u32 [0..=5])
    {
        transition { _ -> step(scale, entries, index, remaining, fallback) }
        state step(s: u32 in Wrapping, view: &[u8], i: u64, pending: u32 [0..=5], spare: &[u8]) {
            transition i < view.len && view[i] == 0 {
                true -> tail(s, view, i, pending, spare)
                _ -> hold(s, view, i, pending, spare)
            }
        }
        state tail(s: u32 in Wrapping, view: &[u8], i: u64, pending: u32 [0..=5], spare: &[u8]) {
            transition pending > 0 {
                true -> step(s, spare, i, pending - 1, spare)
                _ -> finish()
            }
        }
        state hold(s: u32 in Wrapping, view: &[u8], i: u64, pending: u32 [0..=5], spare: &[u8]) {
            transition { _ -> step(s, view, i, pending, spare) }
        }
        state finish() {}
    }
"#;

/// The `ByteSequenceRead` observations inside a component's member blocks,
/// as `(member block, node)` pairs in member order.
fn member_byte_reads<'function>(
    function: &'function optimization_unit::PsiOptimizationFunction,
    component: &optimization_unit::OptimizerCycleComponent,
) -> Vec<(
    &'function optimization_unit::OptimizationBlock,
    &'function optimization_unit::OptimizationNode,
)> {
    component
        .members
        .iter()
        .map(|member| {
            function
                .blocks
                .iter()
                .find(|block| block.id == *member)
                .expect("member block exists")
        })
        .flat_map(|block| {
            block
                .nodes
                .iter()
                .filter(|node| matches!(node.operation, AbstractOperation::ByteSequenceRead { .. }))
                .map(move |node| (block, node))
        })
        .collect()
}

/// The operation identity of a source-owned node.
fn operation_of(node: &optimization_unit::OptimizationNode) -> semantic_vocabulary::OperationId {
    match node.provenance.first() {
        Some(PsiProvenance::Operation(operation)) => *operation,
        _ => panic!("the node carries its operation identity"),
    }
}

#[test]
fn invariant_byte_read_relocates_rebinding_its_root_and_index() {
    let session = lowered_session(MEMBER_BYTE_READ_SOURCE, "member byte read loop");
    let [component] = session.cycle_components().components() else {
        panic!("one cyclic component")
    };
    let [entry] = component.entries.as_slice() else {
        panic!("one entry edge")
    };
    let machine = component.id.machine;
    let preheader = entry.source;
    let function = session
        .unit()
        .functions
        .iter()
        .find(|function| function.machine == machine)
        .expect("component machine exists");
    let [entries_parameter] = function.structural_parameters.as_slice() else {
        panic!("the machine carries one structural parameter")
    };
    let reads = member_byte_reads(function, component);
    let [(read_block, read)] = reads.as_slice() else {
        panic!("one member byte read")
    };
    let member = read_block.id;
    let (read_source, read_index, read_length, read_obligation) = match &read.operation {
        AbstractOperation::ByteSequenceRead {
            source,
            index,
            length,
            obligation,
            ..
        } => (*source, *index, *length, *obligation),
        _ => unreachable!("member_byte_reads only yields byte reads"),
    };
    let read_result = match read.definitions.as_slice() {
        [definition] => definition.value,
        _ => panic!("one observed result"),
    };
    assert_eq!(
        crate::validation::admissible_invariant_byte_read(read),
        Some((read_source, read_index, read_length)),
        "the byte read carries the source-owned admission shape"
    );
    assert!(
        crate::validation::guaranteed_executed_member_blocks(component).contains(&member),
        "the read's member block dominates every exit"
    );
    assert!(
        crate::validation::component_preserves_place_observations(function, component),
        "no member mutates or moves custody of any place"
    );
    let entries_place = entries_parameter.place;
    assert_eq!(
        crate::validation::invariant_member_place_parameters(function, component).get(&read_source),
        Some(&entries_place),
        "the read's member view parameter resolves to `entries`"
    );
    let index_anchor = crate::validation::invariant_member_parameters(function, component)
        .get(&read_index)
        .copied()
        .expect("the read's member index parameter resolves to its preheader anchor");
    // The length operand is the member-internal `ByteSequenceLength` result:
    // the shared admission couples the read to that producer — it must
    // relocate in the same run and measure the read's rebound root.
    let length_reads = member_length_reads(function, component);
    let [(length_block, length_read)] = length_reads.as_slice() else {
        panic!("one member length observation feeds the read")
    };
    assert_eq!(
        match length_read.definitions.as_slice() {
            [definition] => definition.value,
            _ => panic!("one length result"),
        },
        read_length,
        "the read's length operand is the member length observation's result"
    );
    let relocating = std::collections::BTreeSet::from([read_length]);
    assert_eq!(
        crate::validation::invariant_byte_read_admission(function, component, read, &relocating),
        Some((
            entries_place,
            std::collections::BTreeMap::from([(read_index, index_anchor)])
        )),
        "the shared admission rebinds the root and substitutes the index"
    );
    let read_operation = operation_of(read);
    let length_operation = operation_of(length_read);
    let member_structural_parameters = read_block.structural_parameters.clone();

    let candidates =
        propose_loop_invariant_scalar_motion(&session, 1).expect("one exact relocation candidate");
    let [candidate] = candidates.as_slice() else {
        panic!("one component yields one atomic candidate")
    };
    let relocation = candidate
        .relocations()
        .iter()
        .find(|relocation| relocation.node().psi_operation() == read_operation)
        .expect("the byte read is a planned relocation");
    assert_eq!(
        relocation.node().root_rewrite(),
        Some((read_source, entries_place)),
        "the relocation rebinds the member view parameter to `entries`"
    );
    assert_eq!(
        relocation.node().operand_rewrites(),
        &[(read_index, index_anchor)],
        "the relocation substitutes the member index parameter"
    );
    assert_eq!(relocation.node().result().scalar_value(), Some(read_result));
    assert_eq!(relocation.node().location().block, member);
    assert_eq!(relocation.destination().block, preheader);
    let length_relocation = candidate
        .relocations()
        .iter()
        .find(|relocation| relocation.node().psi_operation() == length_operation)
        .expect("the length producer relocates in the same run");
    assert_eq!(
        length_relocation.node().root_rewrite(),
        Some((read_source, entries_place)),
        "the length observation rebinds the same root"
    );
    assert!(
        length_relocation.node().operand_rewrites().is_empty(),
        "the length observation carries no scalar operands"
    );
    assert!(
        length_relocation.destination().node < relocation.destination().node,
        "the run keeps the length producer ahead of the read it defines"
    );
    let view_place = read_source;
    let length_member = length_block.id;

    let validated = validate_loop_invariant_scalar_motion(&session, candidate)
        .expect("independent relocation validation");
    let applied = apply_loop_invariant_scalar_motion(session, validated)
        .expect("atomic relocation application");
    let output_function = applied
        .session()
        .unit()
        .functions
        .iter()
        .find(|function| function.machine == machine)
        .expect("component machine exists");
    let destination = output_function
        .blocks
        .iter()
        .find(|block| block.id == relocation.destination().block)
        .expect("destination block exists");
    let moved = &destination.nodes[usize::try_from(relocation.destination().node).unwrap()];
    match &moved.operation {
        AbstractOperation::ByteSequenceRead {
            source,
            index,
            length,
            obligation,
            result,
            ..
        } => {
            assert_eq!(
                *source, entries_place,
                "the relocated read observes the representative root"
            );
            assert_eq!(*index, index_anchor, "the relocated index is the anchor");
            assert_eq!(
                *length, read_length,
                "the length operand keeps its relocated producer's result"
            );
            assert_eq!(
                *obligation, read_obligation,
                "the bounds obligation stays byte-exact"
            );
            assert_eq!(result.value, read_result);
        }
        operation => panic!("relocated read keeps its operation: {operation:?}"),
    }
    assert_eq!(moved.provenance, relocation.node().provenance());
    assert_eq!(moved.fuel, relocation.node().fuel());
    for (moved_member, kind) in [(member, "read"), (length_member, "length")] {
        assert!(
            output_function
                .blocks
                .iter()
                .find(|block| block.id == moved_member)
                .expect("member block exists")
                .nodes
                .iter()
                .all(|node| {
                    !matches!(
                        node.operation,
                        AbstractOperation::ByteSequenceRead { .. }
                            if kind == "read"
                    ) && !matches!(
                        node.operation,
                        AbstractOperation::ByteSequenceLength { .. }
                            if kind == "length"
                    )
                }),
            "the {kind} exists once, at the destination"
        );
    }
    // Place custody survives the transform: both roots stay declared and the
    // member's view parameter roster is unchanged.
    assert!(output_function.declared_places.contains(&view_place));
    assert!(output_function.declared_places.contains(&entries_place));
    assert_eq!(
        output_function
            .blocks
            .iter()
            .find(|block| block.id == member)
            .expect("member block exists")
            .structural_parameters,
        member_structural_parameters,
    );

    let [record] = applied.ledger().records() else {
        panic!("one atomic relocation has one ledger record")
    };
    let row = record
        .provenance
        .iter()
        .find(|row| row.input == PsiRealizationSite::Node(relocation.node().location()))
        .expect("the relocated read has exact ledger custody");
    assert_eq!(
        row.disposition,
        ProvenanceDisposition::RealizedAt(PsiRealizationSite::Node(relocation.destination()))
    );
    assert_eq!(&row.sources, relocation.node().provenance());
    assert_eq!(&row.fuel, relocation.node().fuel());
    assert!(
        propose_loop_invariant_scalar_motion(applied.session(), 1)
            .expect("relocated session is an exact fixed point")
            .is_empty()
    );
}

#[test]
fn carried_index_keeps_the_byte_read_inside() {
    let session = lowered_session(CARRIED_INDEX_BYTE_READ_SOURCE, "carried index read loop");
    let [component] = session.cycle_components().components() else {
        panic!("one cyclic component")
    };
    let function = session
        .unit()
        .functions
        .iter()
        .find(|function| function.machine == component.id.machine)
        .expect("component machine exists");
    let reads = member_byte_reads(function, component);
    let [(_, read)] = reads.as_slice() else {
        panic!("one member byte read")
    };
    let (read_source, read_index, read_length) = match &read.operation {
        AbstractOperation::ByteSequenceRead {
            source,
            index,
            length,
            ..
        } => (*source, *index, *length),
        _ => unreachable!("member_byte_reads only yields byte reads"),
    };
    assert!(
        !crate::validation::invariant_member_parameters(function, component)
            .contains_key(&read_index),
        "the back edge binds a member-produced constant, so `i` stays loop-carried"
    );
    // The root half still resolves — only the carried index operand refuses.
    assert!(
        crate::validation::invariant_member_place_parameters(function, component)
            .contains_key(&read_source),
        "the member view parameter still resolves to `entries`"
    );
    let relocating = std::collections::BTreeSet::from([read_length]);
    assert!(
        crate::validation::invariant_byte_read_admission(function, component, read, &relocating)
            .is_none(),
        "the shared admission refuses the carried index operand"
    );
    let read_operation = operation_of(read);

    let candidates =
        propose_loop_invariant_scalar_motion(&session, 1).expect("exact relocation candidates");
    let [candidate] = candidates.as_slice() else {
        panic!("the component still yields one atomic candidate")
    };
    assert!(
        candidate
            .relocations()
            .iter()
            .all(|relocation| relocation.node().psi_operation() != read_operation),
        "the carried-index read is not a planned relocation"
    );
}

#[test]
fn carried_view_keeps_the_byte_read_inside() {
    let session = lowered_session(CARRIED_VIEW_BYTE_READ_SOURCE, "carried view read loop");
    let [component] = session.cycle_components().components() else {
        panic!("one cyclic component")
    };
    let function = session
        .unit()
        .functions
        .iter()
        .find(|function| function.machine == component.id.machine)
        .expect("component machine exists");
    assert!(
        crate::validation::component_preserves_place_observations(function, component),
        "no member produces or mutates a place — the refusal is the root's alone"
    );
    let reads = member_byte_reads(function, component);
    let [(_, read)] = reads.as_slice() else {
        panic!("one member byte read")
    };
    let (read_source, _, read_length) = match &read.operation {
        AbstractOperation::ByteSequenceRead {
            source,
            index,
            length,
            ..
        } => (*source, *index, *length),
        _ => unreachable!("member_byte_reads only yields byte reads"),
    };
    assert!(
        !crate::validation::invariant_member_place_parameters(function, component)
            .contains_key(&read_source),
        "a member-produced back-edge binding keeps the member view parameter loop-carried"
    );
    let relocating = std::collections::BTreeSet::from([read_length]);
    assert!(
        crate::validation::invariant_byte_read_admission(function, component, read, &relocating)
            .is_none(),
        "the shared admission refuses the carried view root"
    );
    let read_operation = operation_of(read);

    let candidates =
        propose_loop_invariant_scalar_motion(&session, 1).expect("exact relocation candidates");
    let [candidate] = candidates.as_slice() else {
        panic!("the component still yields one atomic candidate")
    };
    assert!(
        candidate
            .relocations()
            .iter()
            .all(|relocation| relocation.node().psi_operation() != read_operation),
        "the carried-view read is not a planned relocation"
    );
}

#[test]
fn moved_byte_read_without_its_length_is_rejected_by_the_freeze_fence() {
    let session = lowered_session(MEMBER_BYTE_READ_SOURCE, "member byte read loop");
    let [component] = session.cycle_components().components() else {
        panic!("one cyclic component")
    };
    let [entry] = component.entries.as_slice() else {
        panic!("one entry edge")
    };
    let machine = component.id.machine;
    let preheader = entry.source;
    let function = session
        .unit()
        .functions
        .iter()
        .find(|function| function.machine == machine)
        .expect("component machine exists");
    let (read_block, read) = member_byte_reads(function, component)[0];
    let member = read_block.id;
    let read_operation = operation_of(read);
    let (input, mut unit) = session.into_parts();
    // Hand-move the read while its `ByteSequenceLength` producer stays inside:
    // the read's `length` operand names a member-internal result no run
    // relocates, so the seed-derived admission refuses before any root or
    // operand comparison — the coupling is replayed, not trusted.
    let moved = take_operation(&mut unit, read_operation);
    let preheader_block = unit
        .functions
        .iter_mut()
        .flat_map(|function| &mut function.blocks)
        .find(|candidate| candidate.id == preheader)
        .expect("preheader exists");
    let terminator = preheader_block.nodes.len() - 1;
    preheader_block.nodes.insert(terminator, moved);
    refresh_coordinates_and_effects(&mut unit);
    assert!(matches!(
        VerifiedPsiOptimizationSession::from_transformed(input, unit),
        Err(
            OptimizationUnitValidationError::RankedCycleFrozenBlockMismatch {
                machine: rejected_machine,
                block
            }
        ) if rejected_machine == machine && block == member
    ));
}

#[test]
fn forged_byte_read_root_rewrite_is_rejected_by_the_freeze_fence() {
    let session = lowered_session(MEMBER_BYTE_READ_SOURCE, "member byte read loop");
    let [component] = session.cycle_components().components() else {
        panic!("one cyclic component")
    };
    let machine = component.id.machine;
    let function = session
        .unit()
        .functions
        .iter()
        .find(|function| function.machine == machine)
        .expect("component machine exists");
    let (_, read) = member_byte_reads(function, component)[0];
    let read_operation = operation_of(read);
    let candidate = propose_loop_invariant_scalar_motion(&session, 1)
        .expect("exact candidate")
        .pop()
        .expect("one candidate");
    let relocation = candidate
        .relocations()
        .iter()
        .find(|relocation| relocation.node().psi_operation() == read_operation)
        .expect("the byte read carries a root rewrite");
    let member = relocation.node().location().block;
    let (parameter, _) = relocation
        .node()
        .root_rewrite()
        .expect("root rewrite exists");
    let validated = validate_loop_invariant_scalar_motion(&session, &candidate)
        .expect("validated exact candidate");
    let applied =
        apply_loop_invariant_scalar_motion(session, validated).expect("applied exact candidate");
    let (input, mut unit) = applied.into_session().into_parts();
    // Forging the rebound root back to the member view parameter must fail the
    // seed-derived resolution, not just dominance bookkeeping.
    let forged = find_operation_mut(&mut unit, read_operation);
    if let AbstractOperation::ByteSequenceRead { source, .. } = &mut forged.operation {
        *source = parameter;
    }
    unit.identity = recompute_psi_optimization_unit_identity(&unit);
    assert!(matches!(
        crate::validation::validate_transformed_psi_optimization_unit(&input, &unit),
        Err(
            OptimizationUnitValidationError::RankedCycleFrozenBlockMismatch {
                machine: rejected_machine,
                block
            }
        ) if rejected_machine == machine && block == member
    ));
}

#[test]
fn forged_byte_read_index_rewrite_is_rejected_by_the_freeze_fence() {
    let session = lowered_session(MEMBER_BYTE_READ_SOURCE, "member byte read loop");
    let [component] = session.cycle_components().components() else {
        panic!("one cyclic component")
    };
    let machine = component.id.machine;
    let function = session
        .unit()
        .functions
        .iter()
        .find(|function| function.machine == machine)
        .expect("component machine exists");
    let (_, read) = member_byte_reads(function, component)[0];
    let read_operation = operation_of(read);
    let candidate = propose_loop_invariant_scalar_motion(&session, 1)
        .expect("exact candidate")
        .pop()
        .expect("one candidate");
    let relocation = candidate
        .relocations()
        .iter()
        .find(|relocation| relocation.node().psi_operation() == read_operation)
        .expect("the byte read carries an operand rewrite");
    let member = relocation.node().location().block;
    let (parameter, _) = relocation
        .node()
        .operand_rewrites()
        .first()
        .copied()
        .expect("the index substitution exists");
    let validated = validate_loop_invariant_scalar_motion(&session, &candidate)
        .expect("validated exact candidate");
    let applied =
        apply_loop_invariant_scalar_motion(session, validated).expect("applied exact candidate");
    let (input, mut unit) = applied.into_session().into_parts();
    // Forging the rebound index back to the member scalar parameter must fail
    // the seed-derived substitution, not just dominance bookkeeping.
    let forged = find_operation_mut(&mut unit, read_operation);
    if let AbstractOperation::ByteSequenceRead { index, .. } = &mut forged.operation {
        *index = parameter;
    }
    forged.uses[0].value = parameter;
    unit.identity = recompute_psi_optimization_unit_identity(&unit);
    assert!(matches!(
        crate::validation::validate_transformed_psi_optimization_unit(&input, &unit),
        Err(
            OptimizationUnitValidationError::RankedCycleFrozenBlockMismatch {
                machine: rejected_machine,
                block
            }
        ) if rejected_machine == machine && block == member
    ));
}

#[test]
fn forged_byte_read_length_and_obligation_are_rejected_by_the_freeze_fence() {
    let session = lowered_session(MEMBER_BYTE_READ_SOURCE, "member byte read loop");
    let [component] = session.cycle_components().components() else {
        panic!("one cyclic component")
    };
    let machine = component.id.machine;
    let function = session
        .unit()
        .functions
        .iter()
        .find(|function| function.machine == machine)
        .expect("component machine exists");
    let (_, read) = member_byte_reads(function, component)[0];
    let read_operation = operation_of(read);
    let (read_index, read_obligation) = match &read.operation {
        AbstractOperation::ByteSequenceRead {
            index, obligation, ..
        } => (*index, *obligation),
        _ => unreachable!("member_byte_reads only yields byte reads"),
    };
    // A second live obligation the forgery can claim — the member's
    // `pending - 1` descent carries its own exact-subtraction obligation.
    let other_obligation = function
        .blocks
        .iter()
        .flat_map(|block| &block.nodes)
        .filter_map(|node| match &node.operation {
            AbstractOperation::ExactIntegerSubtract { obligation, .. } => Some(*obligation),
            _ => None,
        })
        .find(|obligation| *obligation != read_obligation)
        .expect("the member carries a second obligation to forge");
    let candidate = propose_loop_invariant_scalar_motion(&session, 1)
        .expect("exact candidate")
        .pop()
        .expect("one candidate");
    let relocation = candidate
        .relocations()
        .iter()
        .find(|relocation| relocation.node().psi_operation() == read_operation)
        .expect("the byte read is a planned relocation");
    let member = relocation.node().location().block;
    let validated = validate_loop_invariant_scalar_motion(&session, &candidate)
        .expect("validated exact candidate");
    let applied =
        apply_loop_invariant_scalar_motion(session, validated).expect("applied exact candidate");

    // Forging the `length` operand to another scalar — the rebound index —
    // breaks the coupling the seed replays: the moved read must still name
    // the `ByteSequenceLength` result that measures its rebound root.
    let (input, mut unit) = applied.into_session().into_parts();
    let forged = find_operation_mut(&mut unit, read_operation);
    if let AbstractOperation::ByteSequenceRead { length, .. } = &mut forged.operation {
        *length = read_index;
    }
    forged.uses[1].value = read_index;
    unit.identity = recompute_psi_optimization_unit_identity(&unit);
    assert!(matches!(
        crate::validation::validate_transformed_psi_optimization_unit(&input, &unit),
        Err(
            OptimizationUnitValidationError::RankedCycleFrozenBlockMismatch {
                machine: rejected_machine,
                block
            }
        ) if rejected_machine == machine && block == member
    ));

    // Rebuild the applied unit and forge the obligation instead: the
    // obligation is not a substitutable position, so the replayed operation
    // comparison rejects any other obligation identity.
    let session = lowered_session(MEMBER_BYTE_READ_SOURCE, "member byte read loop");
    let candidate = propose_loop_invariant_scalar_motion(&session, 1)
        .expect("exact candidate")
        .pop()
        .expect("one candidate");
    let validated = validate_loop_invariant_scalar_motion(&session, &candidate)
        .expect("validated exact candidate");
    let applied =
        apply_loop_invariant_scalar_motion(session, validated).expect("applied exact candidate");
    let (input, mut unit) = applied.into_session().into_parts();
    let forged = find_operation_mut(&mut unit, read_operation);
    if let AbstractOperation::ByteSequenceRead { obligation, .. } = &mut forged.operation {
        *obligation = other_obligation;
    }
    unit.identity = recompute_psi_optimization_unit_identity(&unit);
    assert!(matches!(
        crate::validation::validate_transformed_psi_optimization_unit(&input, &unit),
        Err(
            OptimizationUnitValidationError::RankedCycleFrozenBlockMismatch {
                machine: rejected_machine,
                block
            }
        ) if rejected_machine == machine && block == member
    ));
}

/// A `ByteSequenceSubslice` whose evidence is all loop-invariant: `view` is
/// the member structural parameter every reaching edge binds to `entries`,
/// its `start` is a member scalar-constant leaf, and its `end`/`length` pair
/// is the member `ByteSequenceLength` result that relocates in the same run.
/// The subslice's fresh view crosses the `step` → `tail` member edge as
/// `window`, so relocating the producer also exercises a structural result
/// whose member consumers stay inside the loop.
const MEMBER_SUBSLICE_SOURCE: &str = r#"
    data Root {}

    machine Root::scan(scale: u32 in Wrapping, entries: &[u8], index: u64, remaining: u32 [0..=5])
    {
        transition { _ -> step(scale, entries, index, remaining) }
        state step(s: u32 in Wrapping, view: &[u8], i: u64, pending: u32 [0..=5])
        {
            transition { _ -> tail(s, view, view[0..view.len], i, pending) }
        }
        state tail(s: u32 in Wrapping, view: &[u8], window: &[u8], i: u64, pending: u32 [0..=5]) {
            transition pending > 0 && window.len > 0 {
                true -> step(s, view, i, pending - 1)
                _ -> finish()
            }
        }
        state finish() {}
    }
"#;

/// The `ByteSequenceSubslice` operations inside a component's member blocks,
/// as `(member block, node)` pairs in member order.
fn member_subslices<'function>(
    function: &'function optimization_unit::PsiOptimizationFunction,
    component: &optimization_unit::OptimizerCycleComponent,
) -> Vec<(
    &'function optimization_unit::OptimizationBlock,
    &'function optimization_unit::OptimizationNode,
)> {
    component
        .members
        .iter()
        .map(|member| {
            function
                .blocks
                .iter()
                .find(|block| block.id == *member)
                .expect("member block exists")
        })
        .flat_map(|block| {
            block
                .nodes
                .iter()
                .filter(|node| {
                    matches!(
                        node.operation,
                        AbstractOperation::ByteSequenceSubslice { .. }
                    )
                })
                .map(move |node| (block, node))
        })
        .collect()
}

#[test]
fn invariant_byte_subslice_relocates_preserving_its_structural_result() {
    let session = lowered_session(MEMBER_SUBSLICE_SOURCE, "member subslice loop");
    let [component] = session.cycle_components().components() else {
        panic!("one cyclic component")
    };
    let [entry] = component.entries.as_slice() else {
        panic!("one entry edge")
    };
    let machine = component.id.machine;
    let preheader = entry.source;
    let function = session
        .unit()
        .functions
        .iter()
        .find(|function| function.machine == machine)
        .expect("component machine exists");
    let [entries_parameter] = function.structural_parameters.as_slice() else {
        panic!("the machine carries one structural parameter")
    };
    let subslices = member_subslices(function, component);
    let [(subslice_block, subslice)] = subslices.as_slice() else {
        panic!("one member subslice")
    };
    let member = subslice_block.id;
    let (subslice_source, start, end, length, subslice_obligation, result_place) =
        match &subslice.operation {
            AbstractOperation::ByteSequenceSubslice {
                source,
                start,
                end,
                length,
                obligation,
                result,
                ..
            } => (*source, *start, *end, *length, *obligation, result.place),
            _ => unreachable!("member_subslices only yields subslices"),
        };
    assert_eq!(
        crate::validation::admissible_invariant_subslice(subslice),
        Some((subslice_source, start, end, length)),
        "the subslice carries the source-owned admission shape"
    );
    assert!(
        subslice.definitions.is_empty(),
        "the structural result defines no scalar"
    );
    assert!(
        crate::validation::guaranteed_executed_member_blocks(component).contains(&member),
        "the subslice's member block dominates every exit"
    );
    assert!(
        crate::validation::component_preserves_place_observations(function, component),
        "no member mutates or moves custody of any established place"
    );
    let entries_place = entries_parameter.place;
    assert_eq!(
        crate::validation::invariant_member_place_parameters(function, component)
            .get(&subslice_source),
        Some(&entries_place),
        "the subslice's member view parameter resolves to `entries`"
    );
    // `start` is the member scalar-constant leaf's result and `end`/`length`
    // are the member `ByteSequenceLength` result — all run-internal producers
    // the same run relocates, so the substitution is empty.
    let relocating = std::collections::BTreeSet::from([start, end, length]);
    assert_eq!(
        crate::validation::invariant_subslice_admission(function, component, subslice, &relocating),
        Some((entries_place, std::collections::BTreeMap::new())),
        "the shared admission rebinds the root and keeps every operand bound"
    );
    let subslice_operation = operation_of(subslice);
    let member_structural_parameters = subslice_block.structural_parameters.clone();

    let candidates =
        propose_loop_invariant_scalar_motion(&session, 1).expect("one exact relocation candidate");
    let [candidate] = candidates.as_slice() else {
        panic!("one component yields one atomic candidate")
    };
    let relocation = candidate
        .relocations()
        .iter()
        .find(|relocation| relocation.node().psi_operation() == subslice_operation)
        .expect("the subslice is a planned relocation");
    assert_eq!(
        relocation.node().root_rewrite(),
        Some((subslice_source, entries_place)),
        "the relocation rebinds the member view parameter to `entries`"
    );
    assert!(
        relocation.node().operand_rewrites().is_empty(),
        "every scalar operand stays bound to its run-internal producer"
    );
    let LoopInvariantNodeResult::Structural(result) = relocation.node().result() else {
        panic!("the subslice relocation preserves a structural result")
    };
    assert_eq!(result.place, result_place);
    assert_eq!(relocation.node().location().block, member);
    assert_eq!(relocation.destination().block, preheader);
    // The `length` producer must relocate ahead of the subslice it defines.
    let length_reads = member_length_reads(function, component);
    let length_read = length_reads
        .iter()
        .find(|(_, node)| {
            matches!(
                node.definitions.as_slice(),
                [definition] if definition.value == length
            )
        })
        .map(|(_, node)| node)
        .expect("the member length observation feeds the subslice");
    let length_relocation = candidate
        .relocations()
        .iter()
        .find(|relocation| relocation.node().psi_operation() == operation_of(length_read))
        .expect("the length producer relocates in the same run");
    assert!(
        length_relocation.destination().node < relocation.destination().node,
        "the run keeps the length producer ahead of the subslice it defines"
    );

    let validated = validate_loop_invariant_scalar_motion(&session, candidate)
        .expect("independent relocation validation");
    let applied = apply_loop_invariant_scalar_motion(session, validated)
        .expect("atomic relocation application");
    let output_function = applied
        .session()
        .unit()
        .functions
        .iter()
        .find(|function| function.machine == machine)
        .expect("component machine exists");
    let destination = output_function
        .blocks
        .iter()
        .find(|block| block.id == relocation.destination().block)
        .expect("destination block exists");
    let moved = &destination.nodes[usize::try_from(relocation.destination().node).unwrap()];
    match &moved.operation {
        AbstractOperation::ByteSequenceSubslice {
            source,
            start: moved_start,
            end: moved_end,
            length: moved_length,
            obligation,
            result,
            ..
        } => {
            assert_eq!(
                *source, entries_place,
                "the relocated subslice observes the representative root"
            );
            assert_eq!(
                (*moved_start, *moved_end, *moved_length),
                (start, end, length),
                "the scalar operands keep their relocated producers' results"
            );
            assert_eq!(
                *obligation, subslice_obligation,
                "the bounds obligation stays byte-exact"
            );
            assert_eq!(
                result.place, result_place,
                "the fresh view place is preserved, not re-spelled"
            );
        }
        operation => panic!("relocated subslice keeps its operation: {operation:?}"),
    }
    assert_eq!(moved.provenance, relocation.node().provenance());
    assert_eq!(moved.fuel, relocation.node().fuel());
    assert!(
        output_function
            .blocks
            .iter()
            .find(|block| block.id == member)
            .expect("member block exists")
            .nodes
            .iter()
            .all(|node| !matches!(
                node.operation,
                AbstractOperation::ByteSequenceSubslice { .. }
            )),
        "the subslice exists once, at the destination"
    );
    // Place custody survives the transform: the fresh view root stays declared
    // and the member's view parameter roster is unchanged.
    assert!(output_function.declared_places.contains(&result_place));
    assert!(output_function.declared_places.contains(&entries_place));
    assert_eq!(
        output_function
            .blocks
            .iter()
            .find(|block| block.id == member)
            .expect("member block exists")
            .structural_parameters,
        member_structural_parameters,
    );

    let [record] = applied.ledger().records() else {
        panic!("one atomic relocation has one ledger record")
    };
    let row = record
        .provenance
        .iter()
        .find(|row| row.input == PsiRealizationSite::Node(relocation.node().location()))
        .expect("the relocated subslice has exact ledger custody");
    assert_eq!(
        row.disposition,
        ProvenanceDisposition::RealizedAt(PsiRealizationSite::Node(relocation.destination()))
    );
    assert_eq!(&row.sources, relocation.node().provenance());
    assert_eq!(&row.fuel, relocation.node().fuel());
    assert!(
        propose_loop_invariant_scalar_motion(applied.session(), 1)
            .expect("relocated session is an exact fixed point")
            .is_empty()
    );
}

#[test]
fn moved_byte_subslice_without_its_length_is_rejected_by_the_freeze_fence() {
    let session = lowered_session(MEMBER_SUBSLICE_SOURCE, "member subslice loop");
    let [component] = session.cycle_components().components() else {
        panic!("one cyclic component")
    };
    let [entry] = component.entries.as_slice() else {
        panic!("one entry edge")
    };
    let machine = component.id.machine;
    let preheader = entry.source;
    let function = session
        .unit()
        .functions
        .iter()
        .find(|function| function.machine == machine)
        .expect("component machine exists");
    let (subslice_block, subslice) = member_subslices(function, component)[0];
    let member = subslice_block.id;
    let subslice_operation = operation_of(subslice);
    let (input, mut unit) = session.into_parts();
    // Hand-move the subslice while its `ByteSequenceLength` producer stays
    // inside: the subslice's `end`/`length` operands name member-internal
    // results no run relocates, so the seed-derived admission refuses before
    // any root or operand comparison — the coupling is replayed, not trusted.
    let moved = take_operation(&mut unit, subslice_operation);
    let preheader_block = unit
        .functions
        .iter_mut()
        .flat_map(|function| &mut function.blocks)
        .find(|candidate| candidate.id == preheader)
        .expect("preheader exists");
    let terminator = preheader_block.nodes.len() - 1;
    preheader_block.nodes.insert(terminator, moved);
    refresh_coordinates_and_effects(&mut unit);
    assert!(matches!(
        VerifiedPsiOptimizationSession::from_transformed(input, unit),
        Err(
            OptimizationUnitValidationError::RankedCycleFrozenBlockMismatch {
                machine: rejected_machine,
                block
            }
        ) if rejected_machine == machine && block == member
    ));
}

#[test]
fn forged_byte_subslice_root_and_obligation_are_rejected_by_the_freeze_fence() {
    let session = lowered_session(MEMBER_SUBSLICE_SOURCE, "member subslice loop");
    let [component] = session.cycle_components().components() else {
        panic!("one cyclic component")
    };
    let machine = component.id.machine;
    let function = session
        .unit()
        .functions
        .iter()
        .find(|function| function.machine == machine)
        .expect("component machine exists");
    let (_, subslice) = member_subslices(function, component)[0];
    let subslice_operation = operation_of(subslice);
    let subslice_obligation = match &subslice.operation {
        AbstractOperation::ByteSequenceSubslice { obligation, .. } => *obligation,
        _ => unreachable!("member_subslices only yields subslices"),
    };
    // A second live obligation the forgery can claim — the member's
    // `pending - 1` descent carries its own exact-subtraction obligation.
    let other_obligation = function
        .blocks
        .iter()
        .flat_map(|block| &block.nodes)
        .filter_map(|node| match &node.operation {
            AbstractOperation::ExactIntegerSubtract { obligation, .. } => Some(*obligation),
            _ => None,
        })
        .find(|obligation| *obligation != subslice_obligation)
        .expect("the member carries a second obligation to forge");
    let candidate = propose_loop_invariant_scalar_motion(&session, 1)
        .expect("exact candidate")
        .pop()
        .expect("one candidate");
    let relocation = candidate
        .relocations()
        .iter()
        .find(|relocation| relocation.node().psi_operation() == subslice_operation)
        .expect("the subslice is a planned relocation");
    let member = relocation.node().location().block;
    let (parameter, _) = relocation
        .node()
        .root_rewrite()
        .expect("root rewrite exists");
    let validated = validate_loop_invariant_scalar_motion(&session, &candidate)
        .expect("validated exact candidate");
    let applied =
        apply_loop_invariant_scalar_motion(session, validated).expect("applied exact candidate");
    let (input, mut unit) = applied.into_session().into_parts();
    // Forging the rebound root back to the member view parameter must fail the
    // seed-derived resolution, not just dominance bookkeeping.
    let forged = find_operation_mut(&mut unit, subslice_operation);
    if let AbstractOperation::ByteSequenceSubslice { source, .. } = &mut forged.operation {
        *source = parameter;
    }
    unit.identity = recompute_psi_optimization_unit_identity(&unit);
    assert!(matches!(
        crate::validation::validate_transformed_psi_optimization_unit(&input, &unit),
        Err(
            OptimizationUnitValidationError::RankedCycleFrozenBlockMismatch {
                machine: rejected_machine,
                block
            }
        ) if rejected_machine == machine && block == member
    ));

    // Rebuild the applied unit and forge the obligation instead: the
    // obligation is not a substitutable position, so the replayed operation
    // comparison rejects any other obligation identity.
    let session = lowered_session(MEMBER_SUBSLICE_SOURCE, "member subslice loop");
    let candidate = propose_loop_invariant_scalar_motion(&session, 1)
        .expect("exact candidate")
        .pop()
        .expect("one candidate");
    let validated = validate_loop_invariant_scalar_motion(&session, &candidate)
        .expect("validated exact candidate");
    let applied =
        apply_loop_invariant_scalar_motion(session, validated).expect("applied exact candidate");
    let (input, mut unit) = applied.into_session().into_parts();
    let forged = find_operation_mut(&mut unit, subslice_operation);
    if let AbstractOperation::ByteSequenceSubslice { obligation, .. } = &mut forged.operation {
        *obligation = other_obligation;
    }
    unit.identity = recompute_psi_optimization_unit_identity(&unit);
    assert!(matches!(
        crate::validation::validate_transformed_psi_optimization_unit(&input, &unit),
        Err(
            OptimizationUnitValidationError::RankedCycleFrozenBlockMismatch {
                machine: rejected_machine,
                block
            }
        ) if rejected_machine == machine && block == member
    ));
}

#[test]
fn forged_byte_subslice_result_place_is_rejected() {
    let session = lowered_session(MEMBER_SUBSLICE_SOURCE, "member subslice loop");
    let [component] = session.cycle_components().components() else {
        panic!("one cyclic component")
    };
    let machine = component.id.machine;
    let function = session
        .unit()
        .functions
        .iter()
        .find(|function| function.machine == machine)
        .expect("component machine exists");
    let (_, subslice) = member_subslices(function, component)[0];
    let subslice_operation = operation_of(subslice);
    // Another live place the forgery can claim — the `entries` root itself.
    let [entries_parameter] = function.structural_parameters.as_slice() else {
        panic!("the machine carries one structural parameter")
    };
    let forged_place = entries_parameter.place;
    let candidate = propose_loop_invariant_scalar_motion(&session, 1)
        .expect("exact candidate")
        .pop()
        .expect("one candidate");
    assert!(
        candidate
            .relocations()
            .iter()
            .any(|relocation| relocation.node().psi_operation() == subslice_operation),
        "the subslice is a planned relocation"
    );
    let validated = validate_loop_invariant_scalar_motion(&session, &candidate)
        .expect("validated exact candidate");
    let applied =
        apply_loop_invariant_scalar_motion(session, validated).expect("applied exact candidate");
    let (input, mut unit) = applied.into_session().into_parts();
    // Re-spelling the fresh view's place rebinds the structural result to a
    // root it never produced: the moved operation's result is not a
    // substitutable position, so validation must reject the forged unit.
    let forged = find_operation_mut(&mut unit, subslice_operation);
    if let AbstractOperation::ByteSequenceSubslice { result, .. } = &mut forged.operation {
        result.place = forged_place;
    }
    unit.identity = recompute_psi_optimization_unit_identity(&unit);
    assert!(
        crate::validation::validate_transformed_psi_optimization_unit(&input, &unit).is_err(),
        "a forged structural result place is rejected"
    );
}

/// Same two-state component shape, but `step` computes `s - 1` on the
/// invariant member parameter: `s`'s declared `1..=9` range discharges the
/// exact subtraction's totality obligation, so the verifier-obligated
/// computation relocates to the preheader with its `left` operand rebound to
/// the anchor and its obligation preserved byte-exact. The loop-carried
/// `pending - 1` decrement on the back-edge block keeps its own obligation
/// and stays inside — `pending` is advanced by the back edge, and the block
/// does not dominate every exit.
const OBLIGATED_MEMBER_SOURCE: &str = r#"
    data Root {}

    machine Root::scan(scale: u32 [1..=9], remaining: u32 [0..=5])
    {
        transition { _ -> step(scale, remaining) }
        state step(s: u32 [1..=9], pending: u32 [0..=5])
        {
            let shrunk: u32 = s - 1;
            transition pending > 0 {
                true -> scan(s, pending - 1)
                _ -> finish(shrunk)
            }
        }
        state finish(r: u32) {}
    }
"#;

/// The `s - 1` member computation and the member parameter its `left`
/// operand names. The carried `pending - 1` lives in a different member
/// block whose parameters do not include its `left` operand, so the block
/// check selects the invariant computation.
fn member_subtraction<'function>(
    function: &'function optimization_unit::PsiOptimizationFunction,
    component: &optimization_unit::OptimizerCycleComponent,
) -> (
    &'function optimization_unit::OptimizationBlock,
    &'function optimization_unit::OptimizationNode,
    ValueId,
) {
    for member in &component.members {
        let block = function
            .blocks
            .iter()
            .find(|block| block.id == *member)
            .expect("member block exists");
        for node in &block.nodes {
            if let AbstractOperation::ExactIntegerSubtract { left, .. } = &node.operation
                && block
                    .parameters
                    .iter()
                    .any(|parameter| parameter.value == *left)
            {
                return (block, node, *left);
            }
        }
    }
    panic!("the `s - 1` computation lives in a member block")
}

#[test]
fn invariant_obligated_computation_relocates_preserving_its_obligation() {
    let session = lowered_session(OBLIGATED_MEMBER_SOURCE, "obligated member loop");
    let [component] = session.cycle_components().components() else {
        panic!("one two-state component")
    };
    let [entry] = component.entries.as_slice() else {
        panic!("one entry edge")
    };
    let function = session
        .unit()
        .functions
        .iter()
        .find(|function| function.machine == component.id.machine)
        .expect("component machine exists");
    let (member_block, subtraction, member_parameter) = member_subtraction(function, component);
    let (subtraction_operation, obligation, right_operand) = match &subtraction.operation {
        AbstractOperation::ExactIntegerSubtract {
            psi_operation,
            obligation,
            right,
            ..
        } => (*psi_operation, *obligation, *right),
        operation => panic!("the member computation is exact subtraction: {operation:?}"),
    };
    let header = function
        .blocks
        .iter()
        .find(|block| block.id == entry.target)
        .expect("entry target exists");
    let entry_edge = function
        .blocks
        .iter()
        .flat_map(|block| block.nodes.iter().flat_map(|node| node.successors.iter()))
        .find(|edge| edge.psi_edge == entry.edge && edge.target == entry.target)
        .expect("the preheader terminator owns the entry edge");
    let anchor = entry_edge
        .bindings
        .iter()
        .find(|binding| binding.parameter == header.parameters[0].value)
        .expect("entry binds the carried header parameter")
        .argument;
    assert_eq!(
        crate::validation::invariant_member_parameters(function, component).get(&member_parameter),
        Some(&anchor),
        "the member parameter resolves transitively to its preheader anchor"
    );

    let candidates =
        propose_loop_invariant_scalar_motion(&session, 8).expect("exact relocation candidates");
    let [candidate] = candidates.as_slice() else {
        panic!("one component yields one atomic candidate")
    };
    let relocation = candidate
        .relocations()
        .iter()
        .find(|relocation| relocation.node().psi_operation() == subtraction_operation)
        .expect("the obligated member computation is a planned relocation");
    assert_eq!(
        relocation.node().operand_rewrites(),
        &[(member_parameter, anchor)],
    );
    assert_eq!(relocation.node().location().block, member_block.id);
    assert_eq!(relocation.destination().block, entry.source);
    let member_blocks = component.members.clone();

    let validated = validate_loop_invariant_scalar_motion(&session, candidate)
        .expect("independent relocation validation");
    let applied = apply_loop_invariant_scalar_motion(session, validated)
        .expect("atomic relocation application");
    let destination = applied
        .session()
        .unit()
        .functions
        .iter()
        .flat_map(|function| &function.blocks)
        .find(|block| block.id == relocation.destination().block)
        .expect("destination block exists");
    let moved = &destination.nodes[usize::try_from(relocation.destination().node).unwrap()];
    match &moved.operation {
        AbstractOperation::ExactIntegerSubtract {
            left,
            right,
            obligation: moved_obligation,
            ..
        } => {
            assert_eq!(*left, anchor);
            assert_eq!(*moved_obligation, obligation, "obligation moves byte-exact");
            assert_eq!(
                *right, right_operand,
                "the constant-leaf producer relocates in the same run and the \
                 `right` operand stays bound to its preserved result"
            );
        }
        operation => panic!("relocated computation keeps its operation: {operation:?}"),
    }
    assert_eq!(moved.provenance, relocation.node().provenance());
    assert_eq!(moved.fuel, relocation.node().fuel());
    let [record] = applied.ledger().records() else {
        panic!("one atomic relocation has one ledger record")
    };
    let row = record
        .provenance
        .iter()
        .find(|row| row.input == PsiRealizationSite::Node(relocation.node().location()))
        .expect("the relocated computation has exact ledger custody");
    assert_eq!(
        row.disposition,
        ProvenanceDisposition::RealizedAt(PsiRealizationSite::Node(relocation.destination()))
    );
    // The loop-carried decrement keeps its obligation inside the member
    // roster; the moved `s - 1` leaves no further invariant work behind.
    let remaining_subtractions = applied
        .session()
        .unit()
        .functions
        .iter()
        .flat_map(|function| &function.blocks)
        .filter(|block| member_blocks.contains(&block.id))
        .flat_map(|block| &block.nodes)
        .filter(|node| {
            matches!(
                node.operation,
                AbstractOperation::ExactIntegerSubtract { .. }
            )
        })
        .count();
    assert_eq!(
        remaining_subtractions, 1,
        "only the loop-carried `pending - 1` remains inside"
    );
    assert!(
        propose_loop_invariant_scalar_motion(applied.session(), 8)
            .expect("relocated session is an exact fixed point")
            .is_empty()
    );
}

#[test]
fn forged_obligated_computation_obligation_is_rejected_by_the_freeze_fence() {
    let session = lowered_session(OBLIGATED_MEMBER_SOURCE, "obligated member loop");
    let [component] = session.cycle_components().components() else {
        panic!("one two-state component")
    };
    let machine = component.id.machine;
    let function = session
        .unit()
        .functions
        .iter()
        .find(|function| function.machine == machine)
        .expect("component machine exists");
    let (_, subtraction, _) = member_subtraction(function, component);
    let subtraction_operation = operation_of(subtraction);
    let member = component
        .members
        .iter()
        .copied()
        .find(|member| {
            function
                .blocks
                .iter()
                .find(|block| block.id == *member)
                .expect("member block exists")
                .nodes
                .iter()
                .any(|node| {
                    node.provenance.first()
                        == Some(&PsiProvenance::Operation(subtraction_operation))
                })
        })
        .expect("the subtraction's member block");
    let candidate = propose_loop_invariant_scalar_motion(&session, 8)
        .expect("exact candidate")
        .pop()
        .expect("one candidate");
    let validated = validate_loop_invariant_scalar_motion(&session, &candidate)
        .expect("validated exact candidate");
    let applied =
        apply_loop_invariant_scalar_motion(session, validated).expect("applied exact candidate");
    let (input, mut unit) = applied.into_session().into_parts();
    // Forging a different discharged obligation onto the moved subtraction
    // must fail the byte-exact operation comparison the freeze fence re-derives
    // from the seed — the obligation is source-owned evidence, not a
    // coordinate the relocation is free to respell.
    let forged = find_operation_mut(&mut unit, subtraction_operation);
    let AbstractOperation::ExactIntegerSubtract { obligation, .. } = &mut forged.operation else {
        panic!("the relocated node keeps exact subtraction")
    };
    *obligation = semantic_vocabulary::ObligationId::new(obligation.get() + 7)
        .expect("a different obligation id");
    unit.identity = recompute_psi_optimization_unit_identity(&unit);
    assert!(matches!(
        crate::validation::validate_transformed_psi_optimization_unit(&input, &unit),
        Err(
            OptimizationUnitValidationError::RankedCycleFrozenBlockMismatch {
                machine: rejected_machine,
                block
            }
        ) if rejected_machine == machine && block == member
    ));
}

#[test]
fn carried_obligated_computation_is_rejected_by_the_freeze_fence() {
    let session = lowered_session(OBLIGATED_MEMBER_SOURCE, "obligated member loop");
    let [component] = session.cycle_components().components() else {
        panic!("one two-state component")
    };
    let [entry] = component.entries.as_slice() else {
        panic!("one entry edge")
    };
    let machine = component.id.machine;
    let preheader = entry.source;
    let function = session
        .unit()
        .functions
        .iter()
        .find(|function| function.machine == machine)
        .expect("component machine exists");
    // The `pending - 1` decrement: an obligated computation whose operand is
    // loop-carried. Hand-moving it into the preheader must fail admission —
    // no invariant substitution exists for a carried member value — not
    // merely fail a shape check.
    let (member, operation) = component
        .members
        .iter()
        .flat_map(|member| {
            function
                .blocks
                .iter()
                .find(|block| block.id == *member)
                .expect("member block exists")
                .nodes
                .iter()
                .map(move |node| (*member, node))
        })
        .find_map(|(member, node)| {
            if let AbstractOperation::ExactIntegerSubtract { left, .. } = &node.operation
                && !crate::validation::invariant_member_parameters(function, component)
                    .contains_key(left)
            {
                return Some((member, operation_of(node)));
            }
            None
        })
        .expect("the loop-carried decrement lives in a member block");
    let (input, mut unit) = session.into_parts();
    let moved = take_operation(&mut unit, operation);
    let preheader_block = unit
        .functions
        .iter_mut()
        .flat_map(|function| &mut function.blocks)
        .find(|candidate| candidate.id == preheader)
        .expect("preheader exists");
    let terminator = preheader_block.nodes.len() - 1;
    preheader_block.nodes.insert(terminator, moved);
    refresh_coordinates_and_effects(&mut unit);
    assert!(matches!(
        VerifiedPsiOptimizationSession::from_transformed(input, unit),
        Err(
            OptimizationUnitValidationError::RankedCycleFrozenBlockMismatch {
                machine: rejected_machine,
                block
            }
        ) if rejected_machine == machine && block == member
    ));
}

#[test]
fn bypassed_member_computation_moved_by_hand_is_rejected_by_the_freeze_fence() {
    let session = lowered_session(BYPASSED_MEMBER_SOURCE, "bypassed member loop");
    let [component] = session.cycle_components().components() else {
        panic!("one two-state component")
    };
    let [entry] = component.entries.as_slice() else {
        panic!("one entry edge")
    };
    let machine = component.id.machine;
    let preheader = entry.source;
    let function = session
        .unit()
        .functions
        .iter()
        .find(|function| function.machine == machine)
        .expect("component machine exists");
    let (member_block, addition, member_parameter) = member_addition(function, component);
    let member = member_block.id;
    let operation = match addition.provenance.first() {
        Some(PsiProvenance::Operation(operation)) => *operation,
        _ => panic!("computation carries its operation identity"),
    };
    assert!(
        !crate::validation::guaranteed_executed_member_blocks(component).contains(&member),
        "the bypassed member block is outside the non-speculative gate"
    );
    // Rebind the operand to the invariant representative exactly as the
    // proposal would spell it, so the seed-derived substitution replays
    // cleanly and only the non-speculative custody can reject the move.
    let anchor =
        crate::validation::invariant_member_parameters(function, component)[&member_parameter];
    let (input, mut unit) = session.into_parts();
    let mut moved = take_operation(&mut unit, operation);
    if let AbstractOperation::WrappingIntegerAdd { left, right, .. } = &mut moved.operation {
        *left = anchor;
        *right = anchor;
    }
    for value_use in &mut moved.uses {
        value_use.value = anchor;
    }
    let preheader_block = unit
        .functions
        .iter_mut()
        .flat_map(|function| &mut function.blocks)
        .find(|candidate| candidate.id == preheader)
        .expect("preheader exists");
    let terminator = preheader_block.nodes.len() - 1;
    preheader_block.nodes.insert(terminator, moved);
    refresh_coordinates_and_effects(&mut unit);
    assert!(matches!(
        VerifiedPsiOptimizationSession::from_transformed(input, unit),
        Err(
            OptimizationUnitValidationError::RankedCycleFrozenBlockMismatch {
                machine: rejected_machine,
                block
            }
        ) if rejected_machine == machine && block == member
    ));
}

#[test]
fn conditional_entry_computation_moved_by_hand_is_rejected_by_the_freeze_fence() {
    let session = lowered_session_entry(
        CONDITIONAL_ENTRY_SOURCE,
        "conditional entry loop",
        "Root::enter",
    );
    let [component] = session.cycle_components().components() else {
        panic!("one self-loop component")
    };
    let [entry] = component.entries.as_slice() else {
        panic!("one entry edge")
    };
    let machine = component.id.machine;
    let preheader = entry.source;
    let function = session
        .unit()
        .functions
        .iter()
        .find(|function| function.machine == machine)
        .expect("component machine exists");
    let (member_block, addition, member_parameter) = member_addition(function, component);
    let member = member_block.id;
    let operation = match addition.provenance.first() {
        Some(PsiProvenance::Operation(operation)) => *operation,
        _ => panic!("computation carries its operation identity"),
    };
    assert!(
        crate::validation::guaranteed_executed_member_blocks(component).contains(&member),
        "the member qualifies; only the conditional entry can reject the move"
    );
    // Rebind the operand to the invariant representative exactly as the
    // proposal would spell it, so the seed-derived substitution replays
    // cleanly and only the non-speculative custody can reject the move.
    let anchor =
        crate::validation::invariant_member_parameters(function, component)[&member_parameter];
    let (input, mut unit) = session.into_parts();
    let mut moved = take_operation(&mut unit, operation);
    if let AbstractOperation::WrappingIntegerAdd { left, right, .. } = &mut moved.operation {
        *left = anchor;
        *right = anchor;
    }
    for value_use in &mut moved.uses {
        value_use.value = anchor;
    }
    let preheader_block = unit
        .functions
        .iter_mut()
        .flat_map(|function| &mut function.blocks)
        .find(|candidate| candidate.id == preheader)
        .expect("preheader exists");
    let terminator = preheader_block.nodes.len() - 1;
    preheader_block.nodes.insert(terminator, moved);
    refresh_coordinates_and_effects(&mut unit);
    assert!(matches!(
        VerifiedPsiOptimizationSession::from_transformed(input, unit),
        Err(
            OptimizationUnitValidationError::RankedCycleFrozenBlockMismatch {
                machine: rejected_machine,
                block
            }
        ) if rejected_machine == machine && block == member
    ));
}

fn lowered_session(source: &str, label: &str) -> VerifiedPsiOptimizationSession {
    lowered_session_entry(source, label, "Root::scan")
}

fn lowered_session_entry(source: &str, label: &str, entry: &str) -> VerifiedPsiOptimizationSession {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .unwrap_or_else(|error| panic!("tokenize {label}: {error:?}"));
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens)
        .unwrap_or_else(|error| panic!("parse {label}: {error:?}"));
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
    )
    .unwrap_or_else(|error| panic!("resolve {label}: {error:?}"));
    let typed = symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .unwrap_or_else(|error| panic!("type {label}: {error:?}"));
    let checked = typed_trees_to_checked_trees::lower_typed_trees(typed)
        .unwrap_or_else(|error| panic!("check {label}: {error:?}"));
    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, entry)
        .unwrap_or_else(|error| panic!("lower {label}: {error:?}"));
    let input = terminal_psi_to_abstract_operations::lower_artifact_for_optimization(
        terminal_psi_to_abstract_operations::ArtifactSections {
            semantic_bytes: &terminal_codec::encode_module(&lowered.semantic_module)
                .unwrap_or_else(|error| panic!("encode {label} semantics: {error:?}")),
            proof_bytes: &terminal_codec::encode_proof_section(
                &lowered.semantic_module,
                &lowered.proof_bundle,
            )
            .unwrap_or_else(|error| panic!("encode {label} proof: {error:?}")),
            obligation_ledger_bytes: None,
        },
        &proof_admission::AdmissionProfile::default(),
    )
    .and_then(|admitted| admitted.try_into_optimization_input())
    .unwrap_or_else(|error| panic!("optimizer-only {label} admission: {error:?}"));
    let verified = terminal_psi_to_abstract_operations::build_verified_psi_optimization_unit(
        input,
        terminal_fuel::TerminalFuelSchedule::CURRENT.identity(),
    )
    .unwrap_or_else(|error| panic!("build {label} optimizer unit: {error:?}"));
    VerifiedPsiOptimizationSession::new(verified)
        .unwrap_or_else(|error| panic!("verified {label} session: {error:?}"))
}
