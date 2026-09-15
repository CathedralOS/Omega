//! Optimizer module role: test leaf. Transitive member-parameter invariant discovery and place-observation admission.

use super::super::super::VerifiedPsiOptimizationSession;
use crate::{
    LoopInvariantScalarMotionError, LoopInvariantScalarRelocation,
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
                [definition] => definition.value,
                _ => panic!("one defined result"),
            },
            scalar_type: match addition.definitions.as_slice() {
                [definition] => definition.scalar_type,
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
    assert!(
        crate::validation::invariant_place_observation_admission(function, component, read),
        "the field observation passes the shared admission"
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
    assert_eq!(relocation.node().result(), read_result);
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
            !crate::validation::invariant_place_observation_admission(function, component, read),
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
