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

/// A two-state cycle entered through a two-arm dispatch on `pick`: both arms
/// of the entry terminator re-enter the component — `header` directly, and
/// `side` unconditionally rejoining `header` — so the anonymous entry block
/// is a shared preheader whose every successor enters. `header` dominates
/// the component's only exit, so `s + s` there relocates under the same
/// non-speculative custody a single-entry component obeys. `side` is an
/// entry target yet a traversal can enter through `header` and leave without
/// ever running it, so its `s * s` — whose `s` resolves to the same `scale`
/// representative — stays inside under the member half of the gate.
const MULTI_ENTRY_SOURCE: &str = r#"
    data Root {}

    machine Root::scan(scale: u32 in Wrapping, seed: u32 in Wrapping, pick: bool, remaining: u32 [0..=5])
    {
        transition pick {
            true -> header(scale, seed, remaining)
            _ -> side(scale, remaining)
        }
        state side(s: u32 in Wrapping, pending: u32 [0..=5]) {
            let w: u32 in Wrapping = s * s;
            transition { _ -> header(s, w, pending) }
        }
        state header(s: u32 in Wrapping, extra: u32 in Wrapping, pending: u32 [0..=5]) {
            let doubled: u32 in Wrapping = s + s;
            transition pending > 0 {
                true -> side(s, pending - 1)
                _ -> finish(doubled)
            }
        }
        state finish(r: u32 in Wrapping) {}
    }
"#;

/// The same cycle entered through edges departing two different blocks — the
/// anonymous entry dispatch and `outer`'s unconditional rejoin — so the
/// component has no unique preheader block at all. `header`'s `s` is bound
/// to `scale` on one entry and to `outer`'s own parameter on the other, so
/// invariance fails too; the boundary declines the whole component even
/// though `header` still dominates the exit.
const MULTI_SOURCE_ENTRY_SOURCE: &str = r#"
    data Root {}

    machine Root::scan(scale: u32 in Wrapping, pick: bool, remaining: u32 [0..=5])
    {
        transition pick {
            true -> header(scale, remaining)
            _ -> outer(scale, remaining)
        }
        state outer(s: u32 in Wrapping, pending: u32 [0..=5]) {
            transition { _ -> header(s, pending) }
        }
        state header(s: u32 in Wrapping, pending: u32 [0..=5]) {
            let doubled: u32 in Wrapping = s + s;
            transition pending > 0 {
                true -> side(s, pending - 1)
                _ -> finish(doubled)
            }
        }
        state side(s: u32 in Wrapping, pending: u32 [0..=5]) {
            transition { _ -> header(s, pending) }
        }
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

/// The `s * s` computation inside a member block, its block, and the member
/// parameter it reads twice — the bypassed-member counterpart of
/// [`member_addition`].
fn member_multiplication<'function>(
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
            if let AbstractOperation::WrappingIntegerMultiply { left, right, .. } = &node.operation
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
    panic!("the `s * s` computation lives in a member block")
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
            argument_rewrites: Vec::new(),
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
        crate::validation::invariant_place_observation_admission(
            function,
            component,
            read,
            &std::collections::BTreeSet::new(),
        ),
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
            crate::validation::invariant_place_observation_admission(
                function,
                component,
                read,
                &std::collections::BTreeSet::new(),
            )
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
        crate::validation::invariant_member_place_parameters(
            function,
            component,
            &std::collections::BTreeSet::new()
        )
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
        crate::validation::invariant_place_observation_admission(
            function,
            component,
            read,
            &std::collections::BTreeSet::new(),
        ),
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
            crate::validation::invariant_place_observation_admission(
                function,
                component,
                read,
                &std::collections::BTreeSet::new(),
            ),
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

fn find_member_node(
    function: &optimization_unit::PsiOptimizationFunction,
    site: NodeLocation,
) -> &optimization_unit::OptimizationNode {
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
        !crate::validation::invariant_member_place_parameters(
            function,
            component,
            &std::collections::BTreeSet::new()
        )
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
                crate::validation::invariant_place_observation_admission(
                    function,
                    component,
                    read,
                    &std::collections::BTreeSet::new(),
                )
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
        crate::validation::invariant_member_place_parameters(
            function,
            component,
            &std::collections::BTreeSet::new()
        )
        .get(&read_source),
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
        crate::validation::invariant_byte_read_admission(
            function,
            component,
            read,
            &relocating,
            &std::collections::BTreeSet::new(),
        ),
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
        crate::validation::invariant_member_place_parameters(
            function,
            component,
            &std::collections::BTreeSet::new()
        )
        .contains_key(&read_source),
        "the member view parameter still resolves to `entries`"
    );
    let relocating = std::collections::BTreeSet::from([read_length]);
    assert!(
        crate::validation::invariant_byte_read_admission(
            function,
            component,
            read,
            &relocating,
            &std::collections::BTreeSet::new(),
        )
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
        !crate::validation::invariant_member_place_parameters(
            function,
            component,
            &std::collections::BTreeSet::new()
        )
        .contains_key(&read_source),
        "a member-produced back-edge binding keeps the member view parameter loop-carried"
    );
    let relocating = std::collections::BTreeSet::from([read_length]);
    assert!(
        crate::validation::invariant_byte_read_admission(
            function,
            component,
            read,
            &relocating,
            &std::collections::BTreeSet::new(),
        )
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
        crate::validation::invariant_member_place_parameters(
            function,
            component,
            &std::collections::BTreeSet::new()
        )
        .get(&subslice_source),
        Some(&entries_place),
        "the subslice's member view parameter resolves to `entries`"
    );
    // `start` is the member scalar-constant leaf's result and `end`/`length`
    // are the member `ByteSequenceLength` result — all run-internal producers
    // the same run relocates, so the substitution is empty.
    let relocating = std::collections::BTreeSet::from([start, end, length]);
    assert_eq!(
        crate::validation::invariant_subslice_admission(
            function,
            component,
            subslice,
            &relocating,
            &std::collections::BTreeSet::new(),
        ),
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

/// Two entry edges departing the same dispatch block are one preheader: the
/// guaranteed member's `s + s` relocates with its operand rebound to the
/// agreed representative, while `side`'s `s * s` — invariant yet bypassable —
/// stays inside under the member half of the gate.
#[test]
fn multi_entry_preheader_relocates_the_guaranteed_member_computation() {
    let session = lowered_session(MULTI_ENTRY_SOURCE, "multi entry loop");
    let [component] = session.cycle_components().components() else {
        panic!("one multi-entry component")
    };
    assert_eq!(
        component.entries.len(),
        2,
        "both dispatch arms enter the component"
    );
    let preheader = crate::validation::shared_entry_source(component)
        .expect("every entry edge departs the one dispatch block");
    let function = session
        .unit()
        .functions
        .iter()
        .find(|function| function.machine == component.id.machine)
        .expect("component machine exists");
    let (member_block, addition, member_parameter) = member_addition(function, component);
    let (bypassed_block, multiplication, _) = member_multiplication(function, component);
    let guaranteed = crate::validation::guaranteed_executed_member_blocks(component);
    assert!(
        guaranteed.contains(&member_block.id),
        "the exit-dominating member is guaranteed to execute"
    );
    assert!(
        !guaranteed.contains(&bypassed_block.id),
        "a traversal can enter and leave without ever running `side`"
    );
    // `s` resolves transitively to the representative the entry edge into
    // `header` binds — the dispatch block's own parameter — so the relocated
    // computation carries that exact operand rewrite.
    let preheader_block = function
        .blocks
        .iter()
        .find(|block| block.id == preheader)
        .expect("the shared preheader block exists");
    let entry_edge = preheader_block
        .nodes
        .last()
        .expect("the preheader owns a terminator")
        .successors
        .iter()
        .find(|edge| edge.target == member_block.id)
        .expect("one entry edge targets the guaranteed member");
    let anchor = entry_edge
        .bindings
        .iter()
        .find(|binding| binding.parameter == member_parameter)
        .expect("the entry edge binds the carried parameter")
        .argument;
    assert_eq!(
        crate::validation::invariant_member_parameters(function, component).get(&member_parameter),
        Some(&anchor),
        "the member parameter resolves to its shared-preheader anchor"
    );
    let addition_operation = match addition.provenance.first() {
        Some(PsiProvenance::Operation(operation)) => *operation,
        _ => panic!("computation carries its operation identity"),
    };
    let multiplication_operation = match multiplication.provenance.first() {
        Some(PsiProvenance::Operation(operation)) => *operation,
        _ => panic!("computation carries its operation identity"),
    };

    let candidates =
        propose_loop_invariant_scalar_motion(&session, 1).expect("one exact relocation candidate");
    let [candidate] = candidates.as_slice() else {
        panic!("the component yields one atomic candidate")
    };
    let relocation = candidate
        .relocations()
        .iter()
        .find(|relocation| relocation.node().psi_operation() == addition_operation)
        .expect("the guaranteed member computation is a planned relocation");
    assert_eq!(relocation.node().location().block, member_block.id);
    assert_eq!(relocation.destination().block, preheader);
    assert_eq!(
        relocation.node().operand_rewrites(),
        &[(member_parameter, anchor)],
        "the moved computation rebinds to the agreed representative"
    );
    assert!(
        candidate
            .relocations()
            .iter()
            .all(|relocation| relocation.node().psi_operation() != multiplication_operation),
        "the bypassed member's computation is not a planned relocation"
    );

    let component_id = component.id.clone();
    let component_members = component.members.clone();
    let validated = validate_loop_invariant_scalar_motion(&session, candidate)
        .expect("independent relocation validation");
    let applied = apply_loop_invariant_scalar_motion(session, validated)
        .expect("atomic relocation application");
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

/// Entry edges departing different blocks leave the component without a
/// shared preheader — there is no block a relocation could target that every
/// traversal passes through, so the boundary declines the component outright.
#[test]
fn multi_source_entries_decline_the_whole_component() {
    let session = lowered_session(MULTI_SOURCE_ENTRY_SOURCE, "multi source entry loop");
    let [component] = session.cycle_components().components() else {
        panic!("one multi-source component")
    };
    assert!(
        component.entries.len() >= 2,
        "the component is entered through edges of different blocks"
    );
    assert_eq!(
        crate::validation::shared_entry_source(component),
        None,
        "entries departing different blocks share no preheader"
    );
    let function = session
        .unit()
        .functions
        .iter()
        .find(|function| function.machine == component.id.machine)
        .expect("component machine exists");
    let (_, _, member_parameter) = member_addition(function, component);
    assert!(
        !crate::validation::invariant_member_parameters(function, component)
            .contains_key(&member_parameter),
        "the member parameter binds different representatives on the two entries"
    );
    assert!(
        propose_loop_invariant_scalar_motion(&session, 1)
            .expect("proposing over a declined component")
            .is_empty(),
        "without a shared preheader even work-free leaves stay inside"
    );
}

/// A forged relocation out of the bypassed member into the shared preheader:
/// the substitution and destination replay cleanly, so only the
/// independently re-derived member gate can reject the move.
#[test]
fn multi_entry_bypassed_member_moved_by_hand_is_rejected_by_the_freeze_fence() {
    let session = lowered_session(MULTI_ENTRY_SOURCE, "multi entry loop");
    let [component] = session.cycle_components().components() else {
        panic!("one multi-entry component")
    };
    let machine = component.id.machine;
    let preheader = crate::validation::shared_entry_source(component)
        .expect("every entry edge departs the one dispatch block");
    let function = session
        .unit()
        .functions
        .iter()
        .find(|function| function.machine == machine)
        .expect("component machine exists");
    let (member_block, multiplication, member_parameter) =
        member_multiplication(function, component);
    let member = member_block.id;
    let operation = match multiplication.provenance.first() {
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
    if let AbstractOperation::WrappingIntegerMultiply { left, right, .. } = &mut moved.operation {
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

/// A forged relocation that lands in a block other than the shared preheader:
/// every authenticated entry edge still departs the dispatch block, so the
/// destination check rejects the move before substitution replay.
#[test]
fn multi_entry_relocation_outside_the_shared_preheader_is_rejected_by_the_freeze_fence() {
    let session = lowered_session(MULTI_ENTRY_SOURCE, "multi entry loop");
    let [component] = session.cycle_components().components() else {
        panic!("one multi-entry component")
    };
    let machine = component.id.machine;
    let preheader = crate::validation::shared_entry_source(component)
        .expect("every entry edge departs the one dispatch block");
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
    let anchor =
        crate::validation::invariant_member_parameters(function, component)[&member_parameter];
    let destination = function
        .blocks
        .iter()
        .find(|block| block.id != preheader && !component.members.contains(&block.id))
        .expect("the exit state owns a non-member block")
        .id;
    let (input, mut unit) = session.into_parts();
    let mut moved = take_operation(&mut unit, operation);
    if let AbstractOperation::WrappingIntegerAdd { left, right, .. } = &mut moved.operation {
        *left = anchor;
        *right = anchor;
    }
    for value_use in &mut moved.uses {
        value_use.value = anchor;
    }
    let destination_block = unit
        .functions
        .iter_mut()
        .flat_map(|function| &mut function.blocks)
        .find(|candidate| candidate.id == destination)
        .expect("the forged destination exists");
    let terminator = destination_block.nodes.len() - 1;
    destination_block.nodes.insert(terminator, moved);
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

/// A two-state cycle whose member invokes a pure internal callee on an
/// invariant argument: `Root::bump` is a leaf machine whose transitive
/// effect summary proves no observable effect, crash, or suspension, every
/// member node is unobservable, and `s` resolves transitively to `scale`'s
/// preheader anchor — so the call relocates into the preheader rebinding its
/// argument to the anchor, and the `bumped + s` computation chained on the
/// call's preserved result relocates behind it through the same run.
const INVARIANT_CALL_SOURCE: &str = r#"
    data Root {}
    machine Root::scan(scale: u32 in Wrapping, remaining: u32 [0..=5])
    {
        transition { _ -> step(scale, remaining) }
        state step(s: u32 in Wrapping, pending: u32 [0..=5]) {
            let bumped: u32 in Wrapping = Root::bump(s);
            let doubled: u32 in Wrapping = bumped + s;
            transition pending > 0 {
                true -> scan(s, pending - 1)
                _ -> finish(doubled)
            }
        }
        state finish(r: u32 in Wrapping) {}
    }
    machine Root::bump(x: u32 in Wrapping) -> u32 in Wrapping { x + 1 }
"#;

/// Same component shape, but the call's argument is the loop-carried
/// countdown: the member parameter the call reads never resolves to a
/// preheader representative, so the call stays inside even though its callee
/// is pure and its member block is guaranteed to execute.
const CARRIED_ARGUMENT_CALL_SOURCE: &str = r#"
    data Root {}
    machine Root::scan(scale: u32 in Wrapping, remaining: u32 [0..=5])
    {
        transition { _ -> step(scale, remaining) }
        state step(s: u32 in Wrapping, pending: u32 [0..=5]) {
            let bumped: u32 in Wrapping = Root::bump(pending);
            let doubled: u32 in Wrapping = bumped + s;
            transition pending > 0 {
                true -> scan(s, pending - 1)
                _ -> finish(doubled)
            }
        }
        state finish(r: u32 in Wrapping) {}
    }
    machine Root::bump(x: u32 in Wrapping) -> u32 in Wrapping { x + 1 }
"#;

/// Same call shape inside `step`, but the entry state's `done` arm can leave
/// the component before `step` ever runs: callee purity and argument
/// invariance are intact, yet relocating the call would speculate callee work
/// a bypassed traversal never performs, so the non-speculative gate keeps it
/// inside while the header's own invariant leaves still relocate.
const BYPASSED_CALL_SOURCE: &str = r#"
    data Root {}
    machine Root::scan(scale: u32 in Wrapping, remaining: u32 [0..=5])
    {
        transition remaining > 0 {
            true -> step(scale, remaining - 1)
            _ -> done()
        }
        state step(s: u32 in Wrapping, pending: u32 [0..=5]) {
            let bumped: u32 in Wrapping = Root::bump(s);
            let doubled: u32 in Wrapping = bumped + s;
            transition pending > 0 {
                true -> scan(s, pending - 1)
                _ -> finish(doubled)
            }
        }
        state done() {}
        state finish(r: u32 in Wrapping) {}
    }
    machine Root::bump(x: u32 in Wrapping) -> u32 in Wrapping { x + 1 }
"#;

/// The callee `Root::spin` never returns: its transitive summary still proves
/// no observable effect, crash, or suspension, and every member node is
/// unobservable, so the relocation is admitted — moving the call's certain
/// divergence ahead of the member run changes nothing anyone can observe.
/// The gate is unobservability of the member roster, not a termination proof
/// of the callee.
const DIVERGING_CALLEE_SOURCE: &str = r#"
    data Root {}
    machine Root::scan(scale: u32 in Wrapping, remaining: u32 [0..=5])
    {
        transition { _ -> step(scale, remaining) }
        state step(s: u32 in Wrapping, pending: u32 [0..=5]) {
            let bumped: u32 in Wrapping = Root::spin(s);
            let doubled: u32 in Wrapping = bumped + s;
            transition pending > 0 {
                true -> scan(s, pending - 1)
                _ -> finish(doubled)
            }
        }
        state finish(r: u32 in Wrapping) {}
    }
    machine Root::spin(x: u32 in Wrapping) -> u32 in Wrapping {
        transition { _ -> spin(x) }
    }
"#;

/// The `Root::bump`/`Root::spin` scalar call inside a member block and its
/// block — the caller-side counterpart of [`member_addition`].
fn member_call<'function>(
    function: &'function optimization_unit::PsiOptimizationFunction,
    component: &optimization_unit::OptimizerCycleComponent,
) -> (
    &'function optimization_unit::OptimizationBlock,
    &'function optimization_unit::OptimizationNode,
) {
    for member in &component.members {
        let block = function
            .blocks
            .iter()
            .find(|block| block.id == *member)
            .expect("member block exists");
        for node in &block.nodes {
            if let AbstractOperation::Call { .. } = &node.operation {
                return (block, node);
            }
        }
    }
    panic!("the scalar call lives in a member block")
}

#[test]
fn invariant_scalar_call_relocates_rebinding_its_argument() {
    let session = lowered_session(INVARIANT_CALL_SOURCE, "invariant call loop");
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
    let (call_block, call) = member_call(function, component);
    let (call_operation, argument, callee) = match &call.operation {
        AbstractOperation::Call {
            psi_operation,
            callee,
            arguments,
            ..
        } => (*psi_operation, arguments[0], *callee),
        operation => panic!("the member node is a scalar call: {operation:?}"),
    };
    let anchor = crate::validation::invariant_member_parameters(function, component)[&argument];

    let candidates =
        propose_loop_invariant_scalar_motion(&session, 8).expect("exact relocation candidates");
    let [candidate] = candidates.as_slice() else {
        panic!("one component yields one atomic candidate")
    };
    let relocation = candidate
        .relocations()
        .iter()
        .find(|relocation| relocation.node().psi_operation() == call_operation)
        .expect("the invariant scalar call is a planned relocation");
    assert_eq!(
        relocation.node().operand_rewrites(),
        &[(argument, anchor)],
        "the call's member-parameter argument rebinds to its preheader anchor"
    );
    assert_eq!(relocation.node().location().block, call_block.id);
    assert_eq!(relocation.destination().block, entry.source);

    // The `bumped + s` computation consumes the call's result through a block
    // parameter every reaching edge binds to that result: once the run covers
    // the producer, the forwarded parameter is the preserved result on every
    // traversal, so the consumer relocates behind the call in the same
    // candidate.
    assert!(
        candidate.relocations().iter().any(|relocation| {
            let source = function
                .blocks
                .iter()
                .find(|block| block.id == relocation.node().location().block)
                .expect("source block exists");
            matches!(
                source.nodes[usize::try_from(relocation.node().location().node).unwrap()].operation,
                AbstractOperation::WrappingIntegerAdd { .. }
            )
        }),
        "the computation chained on the call result relocates in the same run"
    );

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
        AbstractOperation::Call {
            callee: moved_callee,
            arguments,
            ..
        } => {
            assert_eq!(arguments.as_slice(), &[anchor]);
            assert_eq!(*moved_callee, callee, "callee identity is byte-exact");
        }
        operation => panic!("relocated node keeps its call operation: {operation:?}"),
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
        .expect("the relocated call has exact ledger custody");
    assert_eq!(
        row.disposition,
        ProvenanceDisposition::RealizedAt(PsiRealizationSite::Node(relocation.destination()))
    );
    assert!(
        propose_loop_invariant_scalar_motion(applied.session(), 8)
            .expect("relocated session is an exact fixed point")
            .is_empty()
    );
}

#[test]
fn carried_argument_call_stays_inside() {
    let session = lowered_session(CARRIED_ARGUMENT_CALL_SOURCE, "carried call loop");
    let [component] = session.cycle_components().components() else {
        panic!("one two-state component")
    };
    let function = session
        .unit()
        .functions
        .iter()
        .find(|function| function.machine == component.id.machine)
        .expect("component machine exists");
    let (call_block, call) = member_call(function, component);
    let (call_operation, argument) = match &call.operation {
        AbstractOperation::Call {
            psi_operation,
            arguments,
            ..
        } => (*psi_operation, arguments[0]),
        operation => panic!("the member node is a scalar call: {operation:?}"),
    };
    // Callee purity and member observability are intact — the refusal is
    // exactly the carried argument: `pending` never resolves to a preheader
    // representative, so no substitution exists.
    assert!(
        !crate::validation::invariant_member_parameters(function, component)
            .contains_key(&argument),
        "the back edge advances the call's argument, so it stays loop-carried"
    );
    let effects = crate::validation::unit_effect_summaries(session.unit());
    assert!(
        crate::validation::invariant_scalar_call_admission(
            function,
            component,
            call,
            &std::collections::BTreeSet::new(),
            &effects,
        )
        .is_none(),
        "the carried-argument call fails admission at the substitution half"
    );

    let candidates =
        propose_loop_invariant_scalar_motion(&session, 8).expect("exact relocation candidates");
    let [candidate] = candidates.as_slice() else {
        panic!("the component still yields one atomic candidate")
    };
    assert!(
        !candidate.relocations().is_empty(),
        "invariant work still relocates"
    );
    assert!(
        candidate
            .relocations()
            .iter()
            .all(|relocation| relocation.node().psi_operation() != call_operation),
        "the carried-argument call is not a planned relocation"
    );
    assert!(
        candidate
            .relocations()
            .iter()
            .all(|relocation| relocation.node().location().block != call_block.id),
        "nothing relocates out of the call's member block"
    );
}

#[test]
fn bypassed_member_call_is_speculation_and_stays_inside() {
    let session = lowered_session(BYPASSED_CALL_SOURCE, "bypassed call loop");
    let [component] = session.cycle_components().components() else {
        panic!("one two-state component")
    };
    let function = session
        .unit()
        .functions
        .iter()
        .find(|function| function.machine == component.id.machine)
        .expect("component machine exists");
    let (call_block, call) = member_call(function, component);
    let call_operation = operation_of(call);
    // The call itself still qualifies under call admission — callee purity,
    // member observability, and the invariant-argument substitution all hold.
    // The rejection is the non-speculative gate alone: the call's member
    // block does not dominate the entry state's own `done` exit.
    let effects = crate::validation::unit_effect_summaries(session.unit());
    assert!(
        crate::validation::invariant_scalar_call_admission(
            function,
            component,
            call,
            &std::collections::BTreeSet::new(),
            &effects,
        )
        .is_some(),
        "call admission is intact; the member gate is the only rejection"
    );
    assert!(
        !crate::validation::guaranteed_executed_member_blocks(component).contains(&call_block.id),
        "the bypassed member block is outside the non-speculative gate"
    );

    let candidates =
        propose_loop_invariant_scalar_motion(&session, 8).expect("exact relocation candidates");
    let [candidate] = candidates.as_slice() else {
        panic!("the component still yields one atomic candidate")
    };
    assert!(
        candidate
            .relocations()
            .iter()
            .all(|relocation| relocation.node().psi_operation() != call_operation),
        "the speculated call is not a planned relocation"
    );
}

#[test]
fn diverging_pure_callee_relocates_when_members_are_unobservable() {
    let session = lowered_session(DIVERGING_CALLEE_SOURCE, "diverging callee loop");
    // `Root::spin` carries its own never-exiting component; the caller's
    // component is the one whose members contain the `Call`.
    let caller = session
        .unit()
        .functions
        .iter()
        .find(|function| {
            function
                .blocks
                .iter()
                .flat_map(|block| &block.nodes)
                .any(|node| matches!(node.operation, AbstractOperation::Call { .. }))
        })
        .expect("the caller machine exists");
    let caller_components: Vec<_> = session
        .cycle_components()
        .components()
        .iter()
        .filter(|component| component.id.machine == caller.machine)
        .collect();
    let [component] = caller_components.as_slice() else {
        panic!("the caller's one component")
    };
    let function = session
        .unit()
        .functions
        .iter()
        .find(|function| function.machine == component.id.machine)
        .expect("component machine exists");
    let (_, call) = member_call(function, component);
    let call_operation = operation_of(call);

    let candidates =
        propose_loop_invariant_scalar_motion(&session, 8).expect("exact relocation candidates");
    let relocation = candidates
        .iter()
        .flat_map(|candidate| candidate.relocations().iter())
        .find(|relocation| relocation.node().psi_operation() == call_operation)
        .expect("the diverging pure call is a planned relocation");
    let candidate = candidates
        .iter()
        .find(|candidate| {
            candidate
                .relocations()
                .iter()
                .any(|relocation| relocation.node().psi_operation() == call_operation)
        })
        .expect("the caller's candidate");
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
    assert!(
        matches!(moved.operation, AbstractOperation::Call { .. }),
        "the never-returning pure call occupies its preheader destination"
    );
}

#[test]
fn impure_callee_fails_scalar_call_admission() {
    let session = lowered_session(INVARIANT_CALL_SOURCE, "impure callee loop");
    let [component] = session.cycle_components().components() else {
        panic!("one two-state component")
    };
    let function = session
        .unit()
        .functions
        .iter()
        .find(|function| function.machine == component.id.machine)
        .expect("component machine exists");
    let (_, call) = member_call(function, component);
    let callee = match &call.operation {
        AbstractOperation::Call { callee, .. } => *callee,
        operation => panic!("the member node is a scalar call: {operation:?}"),
    };
    let effects = crate::validation::unit_effect_summaries(session.unit());
    // The real summary admits the call: the refusals below isolate the
    // callee-purity half of admission — the member roster is unchanged and
    // the argument substitution still resolves.
    assert!(
        crate::validation::invariant_scalar_call_admission(
            function,
            component,
            call,
            &std::collections::BTreeSet::new(),
            &effects,
        )
        .is_some(),
        "the verified summary admits the invariant scalar call"
    );
    // Forge each impure axis on the callee's transitive summary in turn:
    // observable effects, a possible crash, and a possible suspension each
    // independently refuse the relocation. The forged table stands in for a
    // callee performing observable work the scalar-call dialect cannot
    // spell — a port write, an atomic event, or a boundary call.
    for axis in 0..3 {
        let mut forged = effects.clone();
        let summary = forged
            .functions
            .iter_mut()
            .find(|summary| summary.machine == callee)
            .expect("the callee has a transitive effect summary");
        match axis {
            0 => summary.observable = crate::EffectKnowledge::May,
            1 => summary.crash = crate::EffectKnowledge::May,
            _ => summary.suspension = crate::EffectKnowledge::May,
        }
        assert!(
            crate::validation::invariant_scalar_call_admission(
                function,
                component,
                call,
                &std::collections::BTreeSet::new(),
                &forged,
            )
            .is_none(),
            "callee effect axis {axis} refuses the relocation"
        );
    }
    // An absent callee row fails closed rather than trusting a drifted table.
    let mut absent = effects.clone();
    absent.functions.retain(|summary| summary.machine != callee);
    assert!(
        crate::validation::invariant_scalar_call_admission(
            function,
            component,
            call,
            &std::collections::BTreeSet::new(),
            &absent,
        )
        .is_none(),
        "a callee missing from the summary table refuses the relocation"
    );
}

#[test]
fn observable_member_keeps_the_scalar_call_inside() {
    let session = lowered_session(INVARIANT_CALL_SOURCE, "observable member loop");
    let [component] = session.cycle_components().components() else {
        panic!("one two-state component")
    };
    let function = session
        .unit()
        .functions
        .iter()
        .find(|function| function.machine == component.id.machine)
        .expect("component machine exists");
    let (_, call) = member_call(function, component);
    let effects = crate::validation::unit_effect_summaries(session.unit());
    // Forge one non-call member node's summary to observable work — member
    // calls are judged by their callee's purity, so the forged row must be a
    // node whose own observable axis the member scan consults. Relocating
    // the call would move its possible non-return ahead of member work the
    // source traversal performed, so the divergence-custody half of
    // admission refuses the whole call even though the callee is pure and
    // the argument is invariant.
    let (observable_block, observable_index) = component
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
                .enumerate()
                .map(|(index, node)| (*member, index, node))
        })
        .find(|(_, _, node)| !matches!(node.operation, AbstractOperation::Call { .. }))
        .map(|(block, index, _)| (block, u32::try_from(index).expect("node index is u32")))
        .expect("a non-call member node exists");
    let mut forged = effects.clone();
    forged
        .nodes
        .iter_mut()
        .find(|summary| {
            summary.machine == function.machine
                && summary.block == observable_block
                && summary.node == observable_index
        })
        .expect("the member node has a summary row")
        .observable = crate::EffectKnowledge::May;
    assert!(
        crate::validation::invariant_scalar_call_admission(
            function,
            component,
            call,
            &std::collections::BTreeSet::new(),
            &forged,
        )
        .is_none(),
        "an observable member node refuses the call's relocation"
    );
}

#[test]
fn carried_argument_call_moved_by_hand_is_rejected_by_the_freeze_fence() {
    let session = lowered_session(CARRIED_ARGUMENT_CALL_SOURCE, "carried call loop");
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
    let (call_block, call) = member_call(function, component);
    let member = call_block.id;
    let preheader = entry.source;
    let call_operation = operation_of(call);
    let (input, mut unit) = session.into_parts();
    // Hand-move the call whose argument is a loop-carried member parameter:
    // the relocation fence must reject it because the callee-purity and
    // member-observability replay still pass but no invariant substitution
    // exists for the argument — the refusal is exact, not a shape artifact.
    let moved = take_operation(&mut unit, call_operation);
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

/// A two-state cycle whose member materializes an immutable byte literal for
/// the `sink` call's argument: the `EstablishByteSequenceLiteral` declares a
/// fresh borrowed-view root over constant bytes every traversal — no scalar
/// uses, no observed root, no custody events — so the whole operation
/// relocates into the preheader byte-exact while the consuming `CallUnit`
/// stays inside the loop still spelling the same place identity.
const INVARIANT_LITERAL_SOURCE: &str = r#"
    data Root {}
    machine sink(v: &[u8]) {}
    machine Root::scan(remaining: u32 [0..=5])
    {
        transition { _ -> step(remaining) }
        state step(pending: u32 [0..=5]) {
            sink("lit");
            transition pending > 0 {
                true -> scan(pending - 1)
                _ -> finish()
            }
        }
        state finish() {}
    }
"#;

/// Same literal establishment inside `step`, but the entry state's `done` arm
/// can leave the component before `step` ever runs: relocating the literal
/// would perform its establishment work on traversals the source never
/// charged, so the non-speculative gate keeps it inside.
const BYPASSED_LITERAL_SOURCE: &str = r#"
    data Root {}
    machine sink(v: &[u8]) {}
    machine Root::scan(remaining: u32 [0..=5])
    {
        transition remaining > 0 {
            true -> step(remaining - 1)
            _ -> done()
        }
        state step(pending: u32 [0..=5]) {
            sink("lit");
            transition pending > 0 {
                true -> scan(pending - 1)
                _ -> finish()
            }
        }
        state done() {}
        state finish() {}
    }
"#;

/// The `EstablishByteSequenceLiteral` inside a member block and its block —
/// the byte-literal counterpart of [`member_call`].
fn member_literal<'function>(
    function: &'function optimization_unit::PsiOptimizationFunction,
    component: &optimization_unit::OptimizerCycleComponent,
) -> (
    &'function optimization_unit::OptimizationBlock,
    &'function optimization_unit::OptimizationNode,
) {
    for member in &component.members {
        let block = function
            .blocks
            .iter()
            .find(|block| block.id == *member)
            .expect("member block exists");
        for node in &block.nodes {
            if let AbstractOperation::EstablishByteSequenceLiteral { .. } = &node.operation {
                return (block, node);
            }
        }
    }
    panic!("the byte literal establishment lives in a member block")
}

#[test]
fn invariant_byte_literal_relocates_preserving_its_declared_place() {
    let session = lowered_session(INVARIANT_LITERAL_SOURCE, "invariant literal loop");
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
    let (literal_block, literal) = member_literal(function, component);
    let member = literal_block.id;
    let preheader = entry.source;
    let (place, structural_type, bytes) = match &literal.operation {
        AbstractOperation::EstablishByteSequenceLiteral {
            place,
            structural_type,
            bytes,
            ..
        } => (*place, structural_type.clone(), bytes.clone()),
        operation => panic!("the member node is a byte literal: {operation:?}"),
    };
    assert!(crate::validation::admissible_invariant_byte_literal(
        literal
    ));
    let literal_operation = operation_of(literal);

    let candidates =
        propose_loop_invariant_scalar_motion(&session, 1).expect("one exact relocation candidate");
    let [candidate] = candidates.as_slice() else {
        panic!("one component yields one atomic candidate")
    };
    let relocation = candidate
        .relocations()
        .iter()
        .find(|relocation| relocation.node().psi_operation() == literal_operation)
        .expect("the byte literal is a planned relocation");
    assert_eq!(relocation.node().location().block, member);
    assert_eq!(relocation.destination().block, preheader);
    assert!(
        relocation.node().operand_rewrites().is_empty()
            && relocation.node().root_rewrite().is_none(),
        "the literal has no scalar operands or observed root to rebind"
    );
    let LoopInvariantNodeResult::LiteralPlace(declared) = relocation.node().result() else {
        panic!("the literal relocation preserves a declared place")
    };
    assert_eq!(
        *declared, place,
        "the relocation preserves the literal's declared place identity"
    );

    // The consuming `CallUnit` relocates in the same run, right behind the
    // literal whose declared root it borrows: the root is produced by the run
    // itself, so the call moves byte-exact — no argument rewrite — and still
    // spells the same place identity.
    let call = function
        .blocks
        .iter()
        .flat_map(|block| &block.nodes)
        .find(|node| matches!(node.operation, AbstractOperation::CallUnit { .. }))
        .expect("the member unit call exists");
    let call_operation = operation_of(call);
    let call_relocation = candidate
        .relocations()
        .iter()
        .find(|relocation| relocation.node().psi_operation() == call_operation)
        .expect("the literal's consuming call relocates in the same run");
    assert!(
        matches!(
            call_relocation.node().result(),
            LoopInvariantNodeResult::Unit
        ) && call_relocation.node().argument_rewrites().is_empty()
            && call_relocation.node().operand_rewrites().is_empty(),
        "the call relocates byte-exact behind its literal producer"
    );
    assert_eq!(
        call_relocation.destination().node,
        relocation.destination().node + 1,
        "the call lands immediately behind its producer in the run"
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
        AbstractOperation::EstablishByteSequenceLiteral {
            place: moved_place,
            structural_type: moved_type,
            bytes: moved_bytes,
            ..
        } => {
            assert_eq!(
                (*moved_place, moved_type.clone(), moved_bytes.as_slice()),
                (place, structural_type, bytes.as_slice()),
                "the relocated literal keeps its declaration and payload byte-exact"
            );
        }
        operation => panic!("relocated node keeps its literal operation: {operation:?}"),
    }
    assert_eq!(moved.provenance, relocation.node().provenance());
    assert_eq!(moved.fuel, relocation.node().fuel());
    let moved_call =
        &destination.nodes[usize::try_from(call_relocation.destination().node).unwrap()];
    assert!(
        matches!(
            &moved_call.operation,
            AbstractOperation::CallUnit {
                structural_arguments,
                ..
            } if structural_arguments
                .iter()
                .any(|argument| argument.place == place.id)
        ),
        "the relocated call keeps borrowing the literal's preserved place"
    );
    let member_block = output_function
        .blocks
        .iter()
        .find(|block| block.id == member)
        .expect("member block exists");
    assert!(
        member_block.nodes.iter().all(|node| !matches!(
            node.operation,
            AbstractOperation::EstablishByteSequenceLiteral { .. }
        )),
        "the literal exists once, at the destination"
    );
    assert!(
        member_block
            .nodes
            .iter()
            .all(|node| !matches!(node.operation, AbstractOperation::CallUnit { .. })),
        "the call exists once, at the destination behind its producer"
    );
    assert!(
        output_function.declared_places.contains(&place.id),
        "the declared literal place survives the transform"
    );
}

#[test]
fn bypassed_member_byte_literal_stays_inside() {
    let session = lowered_session(BYPASSED_LITERAL_SOURCE, "bypassed literal loop");
    let [component] = session.cycle_components().components() else {
        panic!("one component")
    };
    let function = session
        .unit()
        .functions
        .iter()
        .find(|function| function.machine == component.id.machine)
        .expect("component machine exists");
    let (literal_block, literal) = member_literal(function, component);
    assert!(
        !crate::validation::guaranteed_executed_member_blocks(component)
            .contains(&literal_block.id),
        "the bypassed member block is outside the non-speculative gate"
    );
    let literal_operation = operation_of(literal);
    // The `sink("lit")` call shares the bypassed member: relocating it would
    // speculate the callee work alongside its literal, so it stays inside
    // too.
    let call_operation = operation_of(
        literal_block
            .nodes
            .iter()
            .find(|node| matches!(node.operation, AbstractOperation::CallUnit { .. }))
            .expect("the bypassed member carries the literal's call"),
    );

    let candidates =
        propose_loop_invariant_scalar_motion(&session, 1).expect("one exact relocation candidate");
    let [candidate] = candidates.as_slice() else {
        panic!("the component still yields one atomic candidate")
    };
    assert!(
        candidate
            .relocations()
            .iter()
            .all(|relocation| relocation.node().psi_operation() != literal_operation),
        "the speculated literal establishment is not a planned relocation"
    );
    assert!(
        candidate
            .relocations()
            .iter()
            .all(|relocation| relocation.node().psi_operation() != call_operation),
        "the speculated unit call is not a planned relocation"
    );
}

/// A `CallUnit` borrowing an invariant member structural parameter: `sink(b)`
/// inside `step` borrows `b`, which every reaching edge resolves to the
/// machine's `buf` parameter root — the relocated call rebinds that root and
/// keeps its vacuous claim-transfer row byte-exact.
const MEMBER_BORROW_CALL_SOURCE: &str = r#"
    data Root {}
    machine sink(v: &[u8]) {}
    machine Root::scan(buf: &[u8], remaining: u32 [0..=5])
    {
        transition { _ -> step(buf, remaining) }
        state step(b: &[u8], pending: u32 [0..=5]) {
            sink(b);
            transition pending > 0 {
                true -> scan(b, pending - 1)
                _ -> finish()
            }
        }
        state finish() {}
    }
"#;

/// The same `sink(b)` call, but `alt` re-enters `step` binding `b` to
/// `spare`, whose chain anchors on `fallback` rather than `buf`: `b`'s
/// reaching edges resolve to two different preheader-visible roots, so the
/// parameter is loop-carried and the call stays inside — the carried-view
/// counterpart for structural call arguments.
const CARRIED_BORROW_CALL_SOURCE: &str = r#"
    data Root {}
    machine sink(v: &[u8]) {}
    machine Root::scan(buf: &[u8], fallback: &[u8], remaining: u32 [0..=5])
    {
        transition { _ -> step(buf, remaining, fallback) }
        state step(b: &[u8], pending: u32 [0..=5], spare: &[u8]) {
            sink(b);
            transition pending > 0 {
                true -> alt(pending - 1, spare)
                _ -> finish()
            }
        }
        state alt(pending: u32 [0..=5], spare: &[u8]) {
            transition { _ -> step(spare, pending, spare) }
        }
        state finish() {}
    }
"#;

/// The `CallUnit` inside a member block and its block — the
/// structural-signature counterpart of [`member_call`].
fn member_unit_call<'function>(
    function: &'function optimization_unit::PsiOptimizationFunction,
    component: &optimization_unit::OptimizerCycleComponent,
) -> (
    &'function optimization_unit::OptimizationBlock,
    &'function optimization_unit::OptimizationNode,
) {
    for member in &component.members {
        let block = function
            .blocks
            .iter()
            .find(|block| block.id == *member)
            .expect("member block exists");
        for node in &block.nodes {
            if let AbstractOperation::CallUnit { .. } = &node.operation {
                return (block, node);
            }
        }
    }
    panic!("the unit call lives in a member block")
}

#[test]
fn invariant_unit_call_relocates_rebinding_its_borrowed_root() {
    let session = lowered_session(MEMBER_BORROW_CALL_SOURCE, "member borrow call loop");
    let [component] = session.cycle_components().components() else {
        panic!("one component")
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
    let (call_block, call) = member_unit_call(function, component);
    let member = call_block.id;
    let call_ownership = call.ownership.clone();
    let (call_operation, argument_place, callee) = match &call.operation {
        AbstractOperation::CallUnit {
            psi_operation,
            callee,
            arguments,
            structural_arguments,
            claim_transfers,
            crash_continuations,
            ..
        } => {
            assert!(arguments.is_empty() && claim_transfers.is_empty());
            assert!(crash_continuations.is_empty());
            let [argument] = structural_arguments.as_slice() else {
                panic!("one structural argument")
            };
            (*psi_operation, argument.place, *callee)
        }
        operation => panic!("the member node is a unit call: {operation:?}"),
    };
    // The borrowed root is `step`'s member structural parameter; every
    // reaching edge resolves it to the `buf` parameter root.
    let representative = crate::validation::invariant_member_place_parameters(
        function,
        component,
        &std::collections::BTreeSet::new(),
    )[&argument_place];
    assert_ne!(representative, argument_place);

    let candidates =
        propose_loop_invariant_scalar_motion(&session, 8).expect("exact relocation candidates");
    let [candidate] = candidates.as_slice() else {
        panic!("one component yields one atomic candidate")
    };
    let relocation = candidate
        .relocations()
        .iter()
        .find(|relocation| relocation.node().psi_operation() == call_operation)
        .expect("the invariant unit call is a planned relocation");
    assert!(
        matches!(relocation.node().result(), LoopInvariantNodeResult::Unit),
        "the unit call relocation preserves the invocation"
    );
    assert_eq!(
        relocation.node().argument_rewrites(),
        &[(argument_place, representative)],
        "the call's borrowed root rebinds to its preheader-visible representative"
    );
    assert!(
        relocation.node().operand_rewrites().is_empty()
            && relocation.node().root_rewrite().is_none(),
        "the call carries no scalar operands or observed root to rebind"
    );
    assert_eq!(relocation.node().location().block, call_block.id);
    assert_eq!(relocation.destination().block, entry.source);

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
        AbstractOperation::CallUnit {
            callee: moved_callee,
            structural_arguments,
            claim_transfers,
            crash_continuations,
            ..
        } => {
            assert_eq!(*moved_callee, callee);
            let [argument] = structural_arguments.as_slice() else {
                panic!("one structural argument")
            };
            assert_eq!(
                argument.place, representative,
                "the relocated call borrows the representative root"
            );
            assert!(claim_transfers.is_empty() && crash_continuations.is_empty());
        }
        operation => panic!("relocated node keeps its unit-call operation: {operation:?}"),
    }
    assert_eq!(
        moved.ownership.as_slice(),
        call_ownership.as_slice(),
        "the vacuous claim-transfer row moves byte-exact with the node"
    );
    assert_eq!(moved.provenance, relocation.node().provenance());
    assert_eq!(moved.fuel, relocation.node().fuel());
    let member_block = output_function
        .blocks
        .iter()
        .find(|block| block.id == member)
        .expect("member block exists");
    assert!(
        member_block
            .nodes
            .iter()
            .all(|node| !matches!(node.operation, AbstractOperation::CallUnit { .. })),
        "the unit call exists once, at the destination"
    );
}

#[test]
fn carried_borrow_unit_call_stays_inside() {
    let session = lowered_session(CARRIED_BORROW_CALL_SOURCE, "carried borrow call loop");
    let [component] = session.cycle_components().components() else {
        panic!("one component")
    };
    let function = session
        .unit()
        .functions
        .iter()
        .find(|function| function.machine == component.id.machine)
        .expect("component machine exists");
    let (_, call) = member_unit_call(function, component);
    let call_operation = operation_of(call);
    // `b`'s reaching edges disagree: the entry binds `buf`'s root while `alt`
    // re-binds it through `spare` to `fallback`'s root — two different
    // preheader-visible representatives, so the parameter never resolves.
    let call_argument = match &call.operation {
        AbstractOperation::CallUnit {
            structural_arguments,
            ..
        } => structural_arguments[0].place,
        operation => panic!("the member node is a unit call: {operation:?}"),
    };
    assert!(
        !crate::validation::invariant_member_place_parameters(
            function,
            component,
            &std::collections::BTreeSet::new()
        )
        .contains_key(&call_argument),
        "the carried borrow's parameter has no invariant representative"
    );

    let candidates =
        propose_loop_invariant_scalar_motion(&session, 8).expect("exact relocation candidates");
    let [candidate] = candidates.as_slice() else {
        panic!("the component still yields one atomic candidate")
    };
    assert!(
        candidate
            .relocations()
            .iter()
            .all(|relocation| relocation.node().psi_operation() != call_operation),
        "the call borrowing a loop-carried root is not a planned relocation"
    );
}

#[test]
fn unit_call_moved_without_its_literal_producer_is_rejected_by_the_freeze_fence() {
    let session = lowered_session(INVARIANT_LITERAL_SOURCE, "invariant literal loop");
    let [component] = session.cycle_components().components() else {
        panic!("one component")
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
    let (call_block, call) = member_unit_call(function, component);
    let member = call_block.id;
    let preheader = entry.source;
    let call_operation = operation_of(call);
    let (input, mut unit) = session.into_parts();
    // Hand-move only the call: its argument still borrows the literal's
    // member-produced place, but the producer stayed inside — the freeze
    // fence re-derives the admission and refuses because no node in the
    // component's relocated run covered the borrowed root.
    let moved = take_operation(&mut unit, call_operation);
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
fn unit_call_moved_without_rebinding_its_borrow_is_rejected_by_the_freeze_fence() {
    let session = lowered_session(MEMBER_BORROW_CALL_SOURCE, "member borrow call loop");
    let [component] = session.cycle_components().components() else {
        panic!("one component")
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
    let (call_block, call) = member_unit_call(function, component);
    let member = call_block.id;
    let preheader = entry.source;
    let call_operation = operation_of(call);
    let (input, mut unit) = session.into_parts();
    // Hand-move the call byte-exact: the argument still spells `step`'s
    // member parameter. The fence re-derives the member-parameter resolution
    // from the seed, rebinds the expected operation, and the drifted move
    // mismatches.
    let moved = take_operation(&mut unit, call_operation);
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
fn bypassed_literal_moved_by_hand_is_rejected_by_the_freeze_fence() {
    let session = lowered_session(BYPASSED_LITERAL_SOURCE, "bypassed literal loop");
    let [component] = session.cycle_components().components() else {
        panic!("one component")
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
    let (literal_block, literal) = member_literal(function, component);
    let member = literal_block.id;
    let preheader = entry.source;
    let literal_operation = operation_of(literal);
    let (input, mut unit) = session.into_parts();
    // Hand-move the literal out of a member a bypassing exit can skip: the
    // relocation fence must reject it because the re-derived non-speculative
    // gate never admits the move — the forged node's fuel would charge
    // traversals the source never paid it on.
    let moved = take_operation(&mut unit, literal_operation);
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

/// A cyclic `let mut` primitive local borrowed by a scalar-result structural
/// call: `measure` is a pure callee whose shared borrow of `scratch` the
/// cyclic eligibility fence only admits because `scratch` is a
/// primitive-local establishment result, so the establishment and the call
/// relocate in the same run — the local's declared place identity stays
/// byte-exact, its initializer member parameter rebinds to the relocated `7`
/// constant's preserved result, the call's scalar argument rebinds to
/// `scale`'s preheader anchor, and the `measured & scale` computation chained
/// on the call's result relocates behind it through the same run.
const STRUCTURAL_SCALAR_CALL_SOURCE: &str = r#"
    machine measure(value: &u64, bias: u64) -> u64 { bias }

    machine scan(remaining: u64 [0..=5], scale: u64) -> u64
    terminates by remaining -> Nat::Descending in 0..6;
    {
        let mut scratch: u64 = 7;
        let measured: u64 = measure(&scratch, scale);
        let combined: u64 = measured & scale;
        transition remaining > 0 {
            true -> scan(remaining - 1, scale)
            _ -> combined
        }
    }
"#;

/// Same component shape, but the call's scalar argument is the loop-carried
/// countdown: the member parameter the call reads never resolves to a
/// preheader representative, so the call stays inside even though its callee
/// is pure and its borrowed root relocates — the establishment itself still
/// leaves, because a `let mut` cell no member mutates reads its invariant
/// initializer on every traversal whether it is created there or once in the
/// preheader.
const CARRIED_ARGUMENT_STRUCTURAL_CALL_SOURCE: &str = r#"
    machine measure(value: &u64, bias: u64) -> u64 { bias }

    machine scan(remaining: u64 [0..=5], scale: u64) -> u64
    terminates by remaining -> Nat::Descending in 0..6;
    {
        let mut scratch: u64 = 7;
        let measured: u64 = measure(&scratch, remaining);
        transition remaining > 0 {
            true -> scan(remaining - 1, scale)
            _ -> measured
        }
    }
"#;

/// Same component shape, but the borrowed local's initializer is the
/// loop-carried countdown: the establishment cannot leave (its `value`
/// operand is not invariant), and with its producer staying inside the call's
/// borrowed root has no run-covered landing — the call stays inside too even
/// though its scalar argument is invariant.
const CARRIED_LOCAL_STRUCTURAL_CALL_SOURCE: &str = r#"
    machine measure(value: &u64, bias: u64) -> u64 { bias }

    machine scan(remaining: u64 [0..=5], scale: u64) -> u64
    terminates by remaining -> Nat::Descending in 0..6;
    {
        let mut scratch: u64 = remaining;
        let measured: u64 = measure(&scratch, scale);
        transition remaining > 0 {
            true -> scan(remaining - 1, scale)
            _ -> measured
        }
    }
"#;

/// Same component shape, but a member stores to the borrowed local: the
/// whole-component place-custody bound refuses the establishment — a cell
/// re-initialized per traversal in the source cannot collapse into one
/// persistent preheader cell when its contents are rewritten inside the loop
/// — and with the establishment inside, the call's borrowed root never lands
/// in the relocated run either.
const STORED_LOCAL_STRUCTURAL_CALL_SOURCE: &str = r#"
    machine measure(value: &u64, bias: u64) -> u64 { bias }

    machine scan(remaining: u64 [0..=5], scale: u64) -> u64
    terminates by remaining -> Nat::Descending in 0..6;
    {
        let mut scratch: u64 = 7;
        scratch = scale;
        let measured: u64 = measure(&scratch, scale);
        transition remaining > 0 {
            true -> scan(remaining - 1, scale)
            _ -> measured
        }
    }
"#;

/// Same component shape, but the callee takes `&mut`: the mutably borrowed
/// root is produced inside the component and no other member observes it, so
/// the establishment and the borrowing call relocate together — the moved
/// producer's byte-exact place identity lets the call keep spelling the same
/// root, and the callee's per-traversal view stays identical because the
/// cell is initialized once to the same invariant value.
const MUTABLE_BORROW_STRUCTURAL_CALL_SOURCE: &str = r#"
    machine measure_mut(value: &mut u64, bias: u64) -> u64 { value = bias; bias }

    machine scan(remaining: u64 [0..=5], scale: u64) -> u64
    terminates by remaining -> Nat::Descending in 0..6;
    {
        let mut scratch: u64 = 7;
        let measured: u64 = measure_mut(&mut scratch, scale);
        transition remaining > 0 {
            true -> scan(remaining - 1, scale)
            _ -> measured
        }
    }
"#;

/// Same component shape, but the callee takes `&write`: a write-only borrow
/// hands the callee write authority over the member-produced cell without
/// read authority, so the establishment and the borrowing call relocate
/// together under the same member-produced-root rule.
const WRITE_ONLY_BORROW_STRUCTURAL_CALL_SOURCE: &str = r#"
    machine fill(value: &write u64, bias: u64) -> u64 { value = bias; bias }

    machine scan(remaining: u64 [0..=5], scale: u64) -> u64
    terminates by remaining -> Nat::Descending in 0..6;
    {
        let mut scratch: u64 = 7;
        let measured: u64 = fill(&write scratch, scale);
        transition remaining > 0 {
            true -> scan(remaining - 1, scale)
            _ -> measured
        }
    }
"#;

/// A `CallUnit` counterpart of the mutable-borrow shape: the unit-result
/// call mutates the member-produced cell through `&mut`, and producer and
/// borrower relocate together.
const MUTABLE_BORROW_UNIT_CALL_SOURCE: &str = r#"
    machine poke(value: &mut u64, bias: u64) { value = bias; }

    machine scan(remaining: u64 [0..=5], scale: u64) -> u64
    terminates by remaining -> Nat::Descending in 0..6;
    {
        let mut scratch: u64 = 7;
        poke(&mut scratch, scale);
        transition remaining > 0 {
            true -> scan(remaining - 1, scale)
            _ -> 0
        }
    }
"#;

/// Same mutable-borrow component shape, but a member node reads the borrowed
/// root after the call: the read breaks the borrow's exclusivity — the moved
/// producer's persistent cell would hand the member read accumulated
/// post-write contents where the source traversal re-initialized it — so the
/// whole-component custody bound keeps the establishment, the call, and the
/// read inside.
const MEMBER_READS_MUTABLE_BORROW_SOURCE: &str = r#"
    machine measure_mut(value: &mut u64, bias: u64) -> u64 { value = bias; bias }

    machine scan(remaining: u64 [0..=5], scale: u64) -> u64
    terminates by remaining -> Nat::Descending in 0..6;
    {
        let mut scratch: u64 = 7;
        let measured: u64 = measure_mut(&mut scratch, scale);
        let observed: u64 = scratch;
        let combined: u64 = measured & observed;
        transition remaining > 0 {
            true -> scan(remaining - 1, scale)
            _ -> combined
        }
    }
"#;

/// Same mutable-borrow component shape, but two calls mutably borrow one
/// root: each breaks the other's exclusivity — a moved call's callee could
/// observe the sibling borrower's accumulated writes where the source handed
/// it a freshly initialized cell — so the establishment and both calls stay
/// inside.
const SHARED_MUTABLE_ROOT_SOURCE: &str = r#"
    machine measure_mut(value: &mut u64, bias: u64) -> u64 { value = bias; bias }

    machine scan(remaining: u64 [0..=5], scale: u64) -> u64
    terminates by remaining -> Nat::Descending in 0..6;
    {
        let mut scratch: u64 = 7;
        let first: u64 = measure_mut(&mut scratch, scale);
        let second: u64 = measure_mut(&mut scratch, scale);
        let combined: u64 = first & second;
        transition remaining > 0 {
            true -> scan(remaining - 1, scale)
            _ -> combined
        }
    }
"#;

/// Same mutable-borrow component shape, but the borrower's scalar argument
/// is the loop-carried countdown: the call can never relocate, so the
/// move-together coupling keeps the producer's establishment inside too — a
/// persistent preheader cell would let the staying borrower read accumulated
/// writes where the source traversal re-initialized it.
const CARRIED_ARGUMENT_MUTABLE_BORROW_SOURCE: &str = r#"
    machine measure_mut(value: &mut u64, bias: u64) -> u64 { value = bias; bias }

    machine scan(remaining: u64 [0..=5], scale: u64) -> u64
    terminates by remaining -> Nat::Descending in 0..6;
    {
        let mut scratch: u64 = 7;
        let measured: u64 = measure_mut(&mut scratch, remaining);
        transition remaining > 0 {
            true -> scan(remaining - 1, scale)
            _ -> measured
        }
    }
"#;

/// Two primitive locals each borrowed by their own structural scalar call:
/// both establishments and both calls relocate, and each call's borrowed
/// root keeps its producer's declared place byte-exact. The second live
/// place is what a forgery can claim as a substituted argument root.
const TWO_LOCAL_STRUCTURAL_CALL_SOURCE: &str = r#"
    machine measure(value: &u64, bias: u64) -> u64 { bias }

    machine scan(remaining: u64 [0..=5], scale: u64) -> u64
    terminates by remaining -> Nat::Descending in 0..6;
    {
        let mut scratch: u64 = 7;
        let mut other: u64 = 9;
        let first: u64 = measure(&scratch, scale);
        let second: u64 = measure(&other, scale);
        let combined: u64 = first & second;
        transition remaining > 0 {
            true -> scan(remaining - 1, scale)
            _ -> combined
        }
    }
"#;

/// The `EstablishPrimitiveLocal` node inside a member block and its block —
/// the primitive-local counterpart of [`member_call`].
fn member_primitive_local<'function>(
    function: &'function optimization_unit::PsiOptimizationFunction,
    component: &optimization_unit::OptimizerCycleComponent,
) -> (
    &'function optimization_unit::OptimizationBlock,
    &'function optimization_unit::OptimizationNode,
) {
    for member in &component.members {
        let block = function
            .blocks
            .iter()
            .find(|block| block.id == *member)
            .expect("member block exists");
        for node in &block.nodes {
            if let AbstractOperation::EstablishPrimitiveLocal { .. } = &node.operation {
                return (block, node);
            }
        }
    }
    panic!("the primitive-local establishment lives in a member block")
}

/// The `CallStructuralScalar` node inside a member block and its block —
/// the shared-borrow counterpart of [`member_call`]. With more than one
/// structural call in the roster, [`member_structural_scalar_calls`] lists
/// them all.
fn member_structural_scalar_call<'function>(
    function: &'function optimization_unit::PsiOptimizationFunction,
    component: &optimization_unit::OptimizerCycleComponent,
) -> (
    &'function optimization_unit::OptimizationBlock,
    &'function optimization_unit::OptimizationNode,
) {
    let calls = member_structural_scalar_calls(function, component);
    let [(block, node)] = calls.as_slice() else {
        panic!("exactly one structural scalar call lives in a member block")
    };
    (block, node)
}

fn member_structural_scalar_calls<'function>(
    function: &'function optimization_unit::PsiOptimizationFunction,
    component: &optimization_unit::OptimizerCycleComponent,
) -> Vec<(
    &'function optimization_unit::OptimizationBlock,
    &'function optimization_unit::OptimizationNode,
)> {
    let mut calls = Vec::new();
    for member in &component.members {
        let block = function
            .blocks
            .iter()
            .find(|block| block.id == *member)
            .expect("member block exists");
        for node in &block.nodes {
            if let AbstractOperation::CallStructuralScalar { .. } = &node.operation {
                calls.push((block, node));
            }
        }
    }
    calls
}

#[test]
fn structural_scalar_call_relocates_with_its_primitive_local() {
    let session = lowered_session_entry(
        STRUCTURAL_SCALAR_CALL_SOURCE,
        "structural scalar call loop",
        "scan",
    );
    let [component] = session.cycle_components().components() else {
        panic!("one cyclic component")
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
    let (local_block, local) = member_primitive_local(function, component);
    let (call_block, call) = member_structural_scalar_call(function, component);
    let (local_operation, local_place, local_value) = match &local.operation {
        AbstractOperation::EstablishPrimitiveLocal {
            psi_operation,
            result,
            value,
        } => (*psi_operation, result.place, value.value),
        operation => panic!("the member node is a primitive-local establishment: {operation:?}"),
    };
    let (call_operation, scalar_argument, borrowed_place, callee) = match &call.operation {
        AbstractOperation::CallStructuralScalar {
            psi_operation,
            arguments,
            structural_arguments,
            callee,
            ..
        } => (
            *psi_operation,
            arguments[0],
            structural_arguments[0].place,
            *callee,
        ),
        operation => panic!("the member node is a structural scalar call: {operation:?}"),
    };
    assert_eq!(
        borrowed_place, local_place,
        "the call's shared borrow names the local's declared place"
    );
    let scalar_anchor =
        crate::validation::invariant_member_parameters(function, component)[&scalar_argument];

    let candidates =
        propose_loop_invariant_scalar_motion(&session, 8).expect("exact relocation candidates");
    let [candidate] = candidates.as_slice() else {
        panic!("one component yields one atomic candidate")
    };
    let local_relocation = candidate
        .relocations()
        .iter()
        .find(|relocation| relocation.node().psi_operation() == local_operation)
        .expect("the primitive-local establishment is a planned relocation");
    // The declared place relocates byte-exact inside the moved operation, so
    // the borrowing call keeps spelling the same root — no argument rewrite.
    let LoopInvariantNodeResult::Structural(result) = local_relocation.node().result() else {
        panic!("the establishment relocates its structural result")
    };
    assert_eq!(result.place, local_place);
    let [(initializer, representative)] = local_relocation.node().operand_rewrites() else {
        panic!("the establishment carries exactly its initializer rewrite")
    };
    assert_eq!(*initializer, local_value);
    // The initializer member parameter rebinds to the relocated `7`
    // constant's preserved result — a producer the same run already covers.
    let initializer_producer = candidate
        .relocations()
        .iter()
        .find(|relocation| {
            matches!(
                relocation.node().result(),
                LoopInvariantNodeResult::Scalar { value, .. } if value == representative
            )
        })
        .expect("the initializer's producer relocates in the same run");
    let producer_block = function
        .blocks
        .iter()
        .find(|block| block.id == initializer_producer.node().location().block)
        .expect("producer block exists");
    assert!(
        matches!(
            producer_block.nodes
                [usize::try_from(initializer_producer.node().location().node).unwrap()]
            .operation,
            AbstractOperation::IntegerConstant {
                value: semantic_vocabulary::IntegerValue::Unsigned(7),
                ..
            }
        ),
        "the initializer rebinds to the relocated `7` constant"
    );

    let call_relocation = candidate
        .relocations()
        .iter()
        .find(|relocation| relocation.node().psi_operation() == call_operation)
        .expect("the structural scalar call is a planned relocation");
    assert_eq!(
        call_relocation.node().operand_rewrites(),
        &[(scalar_argument, scalar_anchor)],
        "the call's member-parameter scalar argument rebinds to its preheader anchor"
    );
    assert!(
        call_relocation.node().argument_rewrites().is_empty(),
        "the moved establishment preserves the borrowed place identity byte-exact"
    );
    assert_eq!(call_relocation.node().location().block, call_block.id);
    assert_eq!(local_relocation.node().location().block, local_block.id);
    assert_eq!(call_relocation.destination().block, entry.source);
    assert!(
        local_relocation.destination().node < call_relocation.destination().node,
        "the relocated establishment lands ahead of the call borrowing its root"
    );
    // The `measured & scale` computation consumes the call's preserved result
    // through a member parameter every reaching edge binds to it, so it
    // relocates behind the call in the same candidate.
    assert!(
        candidate.relocations().iter().any(|relocation| {
            let source = function
                .blocks
                .iter()
                .find(|block| block.id == relocation.node().location().block)
                .expect("source block exists");
            matches!(
                source.nodes[usize::try_from(relocation.node().location().node).unwrap()].operation,
                AbstractOperation::IntegerBitwiseAnd { .. }
            )
        }),
        "the computation chained on the call result relocates in the same run"
    );

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
        .find(|block| block.id == call_relocation.destination().block)
        .expect("destination block exists");
    let moved_local =
        &destination.nodes[usize::try_from(local_relocation.destination().node).unwrap()];
    match &moved_local.operation {
        AbstractOperation::EstablishPrimitiveLocal { result, value, .. } => {
            assert_eq!(
                result.place, local_place,
                "the declared place is byte-exact"
            );
            assert_eq!(
                value.value, *representative,
                "the initializer rebinds to the relocated constant"
            );
        }
        operation => panic!("relocated node keeps its establishment operation: {operation:?}"),
    }
    let moved_call =
        &destination.nodes[usize::try_from(call_relocation.destination().node).unwrap()];
    match &moved_call.operation {
        AbstractOperation::CallStructuralScalar {
            callee: moved_callee,
            arguments,
            structural_arguments,
            ..
        } => {
            assert_eq!(arguments.as_slice(), &[scalar_anchor]);
            assert_eq!(
                structural_arguments[0].place, local_place,
                "the borrowed root stays byte-exact"
            );
            assert_eq!(*moved_callee, callee, "callee identity is byte-exact");
        }
        operation => panic!("relocated node keeps its call operation: {operation:?}"),
    }
    assert_eq!(moved_call.provenance, call_relocation.node().provenance());
    assert_eq!(moved_call.fuel, call_relocation.node().fuel());
}

#[test]
fn structural_scalar_call_stays_when_its_scalar_argument_is_carried() {
    let session = lowered_session_entry(
        CARRIED_ARGUMENT_STRUCTURAL_CALL_SOURCE,
        "carried argument structural call loop",
        "scan",
    );
    let [component] = session.cycle_components().components() else {
        panic!("one cyclic component")
    };
    let function = session
        .unit()
        .functions
        .iter()
        .find(|function| function.machine == component.id.machine)
        .expect("component machine exists");
    let (_, local) = member_primitive_local(function, component);
    let (_, call) = member_structural_scalar_call(function, component);
    let local_operation = operation_of(local);
    let call_operation = operation_of(call);

    let candidates =
        propose_loop_invariant_scalar_motion(&session, 8).expect("exact relocation candidates");
    let [candidate] = candidates.as_slice() else {
        panic!("one component yields one atomic candidate")
    };
    // The establishment still relocates — its `7` initializer is invariant
    // and no member mutates the declared place — but the call's scalar
    // argument is the carried countdown, so the call stays inside.
    assert!(
        candidate
            .relocations()
            .iter()
            .any(|relocation| relocation.node().psi_operation() == local_operation),
        "the invariant-initialized establishment relocates"
    );
    assert!(
        candidate
            .relocations()
            .iter()
            .all(|relocation| relocation.node().psi_operation() != call_operation),
        "the call reading a carried scalar argument stays inside"
    );
}

#[test]
fn structural_scalar_call_stays_when_its_borrowed_local_is_carried() {
    let session = lowered_session_entry(
        CARRIED_LOCAL_STRUCTURAL_CALL_SOURCE,
        "carried local structural call loop",
        "scan",
    );
    let [component] = session.cycle_components().components() else {
        panic!("one cyclic component")
    };
    let function = session
        .unit()
        .functions
        .iter()
        .find(|function| function.machine == component.id.machine)
        .expect("component machine exists");
    let (_, local) = member_primitive_local(function, component);
    let (_, call) = member_structural_scalar_call(function, component);
    let local_operation = operation_of(local);
    let call_operation = operation_of(call);

    let candidates =
        propose_loop_invariant_scalar_motion(&session, 8).expect("exact relocation candidates");
    for candidate in &candidates {
        // The carried initializer keeps the establishment inside, and with
        // its producer inside the call's borrowed root has no run-covered
        // landing — no structural-argument root rewrite ever appears.
        assert!(
            candidate
                .relocations()
                .iter()
                .all(
                    |relocation| relocation.node().psi_operation() != local_operation
                        && relocation.node().psi_operation() != call_operation
                ),
            "the carried local and its borrowing call stay inside"
        );
        assert!(
            candidate
                .relocations()
                .iter()
                .all(|relocation| relocation.node().argument_rewrites().is_empty()),
            "no incorrect root rebinding occurs"
        );
    }
}

#[test]
fn primitive_local_and_call_stay_when_a_member_stores_to_it() {
    let session = lowered_session_entry(
        STORED_LOCAL_STRUCTURAL_CALL_SOURCE,
        "stored local structural call loop",
        "scan",
    );
    let [component] = session.cycle_components().components() else {
        panic!("one cyclic component")
    };
    let function = session
        .unit()
        .functions
        .iter()
        .find(|function| function.machine == component.id.machine)
        .expect("component machine exists");
    let (_, local) = member_primitive_local(function, component);
    let (_, call) = member_structural_scalar_call(function, component);
    let local_operation = operation_of(local);
    let call_operation = operation_of(call);
    // The store lands inside the roster as `PrimitiveLocalStore`.
    assert!(
        component.members.iter().any(|member| {
            function
                .blocks
                .iter()
                .find(|block| block.id == *member)
                .expect("member block exists")
                .nodes
                .iter()
                .any(|node| {
                    matches!(
                        node.operation,
                        AbstractOperation::PrimitiveLocalStore { .. }
                    )
                })
        }),
        "the member roster contains the local store"
    );

    let candidates =
        propose_loop_invariant_scalar_motion(&session, 8).expect("exact relocation candidates");
    for candidate in &candidates {
        // The whole-component place-custody bound refuses the establishment —
        // a member rewrites the cell the source creates per traversal — and
        // with the establishment inside, the call's borrowed root never lands
        // in the relocated run.
        assert!(
            candidate
                .relocations()
                .iter()
                .all(
                    |relocation| relocation.node().psi_operation() != local_operation
                        && relocation.node().psi_operation() != call_operation
                ),
            "a member store keeps the local and its borrowing call inside"
        );
    }
}

#[test]
fn mutable_borrow_call_relocates_with_its_primitive_local() {
    let session = lowered_session_entry(
        MUTABLE_BORROW_STRUCTURAL_CALL_SOURCE,
        "mutable borrow structural call loop",
        "scan",
    );
    let [component] = session.cycle_components().components() else {
        panic!("one cyclic component")
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
    let (_, local) = member_primitive_local(function, component);
    let (_, call) = member_structural_scalar_call(function, component);
    let local_operation = operation_of(local);
    let call_operation = operation_of(call);
    let (local_place, callee) = match (&local.operation, &call.operation) {
        (
            AbstractOperation::EstablishPrimitiveLocal { result, .. },
            AbstractOperation::CallStructuralScalar {
                structural_arguments,
                callee,
                ..
            },
        ) => {
            assert_eq!(
                structural_arguments[0].access,
                terminal_psi::StructuralAccess::MutableBorrow,
                "the member call borrows the local mutably"
            );
            assert_eq!(structural_arguments[0].place, result.place);
            (result.place, *callee)
        }
        operations => {
            panic!("member nodes are a local and a structural scalar call: {operations:?}")
        }
    };

    let candidates =
        propose_loop_invariant_scalar_motion(&session, 8).expect("exact relocation candidates");
    let [candidate] = candidates.as_slice() else {
        panic!("one component yields one atomic candidate")
    };
    // The move-together coupling: the establishment relocates only because
    // its sole mutable borrower relocates in the same run, and the moved
    // producer's byte-exact place identity keeps the borrow argument
    // spelling unchanged — no argument rewrite.
    let local_relocation = candidate
        .relocations()
        .iter()
        .find(|relocation| relocation.node().psi_operation() == local_operation)
        .expect("the primitive-local establishment relocates with its borrower");
    let call_relocation = candidate
        .relocations()
        .iter()
        .find(|relocation| relocation.node().psi_operation() == call_operation)
        .expect("the mutable-borrow call relocates behind its producer");
    assert!(
        call_relocation.node().argument_rewrites().is_empty(),
        "the moved establishment preserves the borrowed place identity byte-exact"
    );
    assert_eq!(call_relocation.destination().block, entry.source);
    assert!(
        local_relocation.destination().node < call_relocation.destination().node,
        "the relocated establishment lands ahead of the call mutating its root"
    );

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
        .find(|block| block.id == call_relocation.destination().block)
        .expect("destination block exists");
    let moved_call =
        &destination.nodes[usize::try_from(call_relocation.destination().node).unwrap()];
    match &moved_call.operation {
        AbstractOperation::CallStructuralScalar {
            callee: moved_callee,
            structural_arguments,
            ..
        } => {
            assert_eq!(
                structural_arguments[0].place, local_place,
                "the mutably borrowed root stays byte-exact"
            );
            assert_eq!(
                structural_arguments[0].access,
                terminal_psi::StructuralAccess::MutableBorrow,
                "the mutable-borrow access stays byte-exact"
            );
            assert_eq!(*moved_callee, callee, "callee identity is byte-exact");
        }
        operation => panic!("relocated node keeps its call operation: {operation:?}"),
    }
    assert_eq!(moved_call.provenance, call_relocation.node().provenance());
    assert_eq!(moved_call.fuel, call_relocation.node().fuel());
}

#[test]
fn write_only_borrow_call_relocates_with_its_primitive_local() {
    let session = lowered_session_entry(
        WRITE_ONLY_BORROW_STRUCTURAL_CALL_SOURCE,
        "write-only borrow structural call loop",
        "scan",
    );
    let [component] = session.cycle_components().components() else {
        panic!("one cyclic component")
    };
    let [entry] = component.entries.as_slice() else {
        panic!("one entry edge")
    };
    let preheader = entry.source;
    let function = session
        .unit()
        .functions
        .iter()
        .find(|function| function.machine == component.id.machine)
        .expect("component machine exists");
    let (_, local) = member_primitive_local(function, component);
    let (_, call) = member_structural_scalar_call(function, component);
    let local_operation = operation_of(local);
    let call_operation = operation_of(call);
    match &call.operation {
        AbstractOperation::CallStructuralScalar {
            structural_arguments,
            ..
        } => assert_eq!(
            structural_arguments[0].access,
            terminal_psi::StructuralAccess::WriteOnlyBorrow,
            "the member call borrows the local write-only"
        ),
        operation => panic!("the member node is a structural scalar call: {operation:?}"),
    }

    let candidates =
        propose_loop_invariant_scalar_motion(&session, 8).expect("exact relocation candidates");
    let [candidate] = candidates.as_slice() else {
        panic!("one component yields one atomic candidate")
    };
    let local_relocation = candidate
        .relocations()
        .iter()
        .find(|relocation| relocation.node().psi_operation() == local_operation)
        .expect("the primitive-local establishment relocates with its borrower");
    let call_relocation = candidate
        .relocations()
        .iter()
        .find(|relocation| relocation.node().psi_operation() == call_operation)
        .expect("the write-only-borrow call relocates behind its producer");
    assert!(
        call_relocation.node().argument_rewrites().is_empty(),
        "the moved establishment preserves the borrowed place identity byte-exact"
    );
    assert!(
        local_relocation.destination().node < call_relocation.destination().node,
        "the relocated establishment lands ahead of the call writing its root"
    );

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
        .find(|block| block.id == preheader)
        .expect("destination block exists");
    let moved_call =
        &destination.nodes[usize::try_from(call_relocation.destination().node).unwrap()];
    match &moved_call.operation {
        AbstractOperation::CallStructuralScalar {
            structural_arguments,
            ..
        } => assert_eq!(
            structural_arguments[0].access,
            terminal_psi::StructuralAccess::WriteOnlyBorrow,
            "the write-only-borrow access stays byte-exact"
        ),
        operation => panic!("relocated node keeps its call operation: {operation:?}"),
    }
}

#[test]
fn mutable_borrow_unit_call_relocates_with_its_primitive_local() {
    let session = lowered_session_entry(
        MUTABLE_BORROW_UNIT_CALL_SOURCE,
        "mutable borrow unit call loop",
        "scan",
    );
    let [component] = session.cycle_components().components() else {
        panic!("one cyclic component")
    };
    let [entry] = component.entries.as_slice() else {
        panic!("one entry edge")
    };
    let preheader = entry.source;
    let function = session
        .unit()
        .functions
        .iter()
        .find(|function| function.machine == component.id.machine)
        .expect("component machine exists");
    let (_, local) = member_primitive_local(function, component);
    let (_, call) = member_unit_call(function, component);
    let local_operation = operation_of(local);
    let call_operation = operation_of(call);
    match &call.operation {
        AbstractOperation::CallUnit {
            structural_arguments,
            ..
        } => assert_eq!(
            structural_arguments[0].access,
            terminal_psi::StructuralAccess::MutableBorrow,
            "the member unit call borrows the local mutably"
        ),
        operation => panic!("the member node is a unit call: {operation:?}"),
    }

    let candidates =
        propose_loop_invariant_scalar_motion(&session, 8).expect("exact relocation candidates");
    let [candidate] = candidates.as_slice() else {
        panic!("one component yields one atomic candidate")
    };
    let local_relocation = candidate
        .relocations()
        .iter()
        .find(|relocation| relocation.node().psi_operation() == local_operation)
        .expect("the primitive-local establishment relocates with its borrower");
    let call_relocation = candidate
        .relocations()
        .iter()
        .find(|relocation| relocation.node().psi_operation() == call_operation)
        .expect("the mutable-borrow unit call relocates behind its producer");
    assert!(
        matches!(
            call_relocation.node().result(),
            LoopInvariantNodeResult::Unit
        ) && call_relocation.node().argument_rewrites().is_empty(),
        "the unit call relocates byte-exact behind its producer"
    );
    assert!(
        local_relocation.destination().node < call_relocation.destination().node,
        "the relocated establishment lands ahead of the call mutating its root"
    );

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
        .find(|block| block.id == preheader)
        .expect("destination block exists");
    let moved_call =
        &destination.nodes[usize::try_from(call_relocation.destination().node).unwrap()];
    assert!(
        matches!(
            &moved_call.operation,
            AbstractOperation::CallUnit {
                structural_arguments,
                ..
            } if structural_arguments[0].access
                == terminal_psi::StructuralAccess::MutableBorrow
        ),
        "the relocated unit call keeps its mutable-borrow argument byte-exact"
    );
}

#[test]
fn mutable_borrow_stays_when_a_member_reads_the_borrowed_root() {
    let session = lowered_session_entry(
        MEMBER_READS_MUTABLE_BORROW_SOURCE,
        "member reads mutable borrow loop",
        "scan",
    );
    let [component] = session.cycle_components().components() else {
        panic!("one cyclic component")
    };
    let function = session
        .unit()
        .functions
        .iter()
        .find(|function| function.machine == component.id.machine)
        .expect("component machine exists");
    let (_, local) = member_primitive_local(function, component);
    let (_, call) = member_structural_scalar_call(function, component);
    let local_operation = operation_of(local);
    let call_operation = operation_of(call);
    let read = component
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
        })
        .find(|node| {
            matches!(
                node.operation,
                AbstractOperation::PrimitiveScalarRead { .. }
            )
        })
        .expect("the member read of the borrowed local exists");
    let read_operation = operation_of(read);

    let candidates =
        propose_loop_invariant_scalar_motion(&session, 8).expect("exact relocation candidates");
    for candidate in &candidates {
        // The member read breaks the mutating borrow's exclusivity: a moved
        // producer would hand the read a persistent cell's accumulated
        // contents where the source traversal re-initialized it, so the
        // custody bound keeps all three inside.
        assert!(
            candidate
                .relocations()
                .iter()
                .all(
                    |relocation| relocation.node().psi_operation() != local_operation
                        && relocation.node().psi_operation() != call_operation
                        && relocation.node().psi_operation() != read_operation
                ),
            "a member read keeps the local, its mutable borrower, and the read inside"
        );
    }
}

#[test]
fn mutable_borrow_stays_when_two_calls_share_the_root() {
    let session = lowered_session_entry(
        SHARED_MUTABLE_ROOT_SOURCE,
        "shared mutable root loop",
        "scan",
    );
    let [component] = session.cycle_components().components() else {
        panic!("one cyclic component")
    };
    let function = session
        .unit()
        .functions
        .iter()
        .find(|function| function.machine == component.id.machine)
        .expect("component machine exists");
    let (_, local) = member_primitive_local(function, component);
    let calls = member_structural_scalar_calls(function, component);
    let local_operation = operation_of(local);
    let [(_, first), (_, second)] = calls.as_slice() else {
        panic!("two calls borrow the same local mutably")
    };
    let call_operations = [operation_of(first), operation_of(second)];

    let candidates =
        propose_loop_invariant_scalar_motion(&session, 8).expect("exact relocation candidates");
    for candidate in &candidates {
        // Each mutable borrower breaks the other's exclusivity: a moved
        // call's callee would read the persistent cell where the source
        // re-initialized it — and the sibling's staying writes would have
        // accumulated into it — so nothing carrying the shared root moves.
        assert!(
            candidate
                .relocations()
                .iter()
                .all(
                    |relocation| relocation.node().psi_operation() != local_operation
                        && !call_operations.contains(&relocation.node().psi_operation())
                ),
            "two mutable borrowers of one root keep the local and both calls inside"
        );
    }
}

#[test]
fn mutable_borrow_producer_stays_when_its_borrower_cannot_relocate() {
    let session = lowered_session_entry(
        CARRIED_ARGUMENT_MUTABLE_BORROW_SOURCE,
        "carried argument mutable borrow loop",
        "scan",
    );
    let [component] = session.cycle_components().components() else {
        panic!("one cyclic component")
    };
    let function = session
        .unit()
        .functions
        .iter()
        .find(|function| function.machine == component.id.machine)
        .expect("component machine exists");
    let (_, local) = member_primitive_local(function, component);
    let (_, call) = member_structural_scalar_call(function, component);
    let local_operation = operation_of(local);
    let call_operation = operation_of(call);

    let candidates =
        propose_loop_invariant_scalar_motion(&session, 8).expect("exact relocation candidates");
    for candidate in &candidates {
        // The borrower's carried scalar argument can never rebind, so the
        // call stays inside — and the move-together coupling then refuses
        // the establishment too: a persistent preheader cell would let the
        // staying borrower read accumulated writes where the source
        // traversal re-initialized it.
        assert!(
            candidate
                .relocations()
                .iter()
                .all(
                    |relocation| relocation.node().psi_operation() != local_operation
                        && relocation.node().psi_operation() != call_operation
                ),
            "a staying mutable borrower keeps its producer inside"
        );
    }
}

#[test]
fn stranded_mutable_borrower_is_rejected_by_the_freeze_fence() {
    let session = lowered_session_entry(
        MUTABLE_BORROW_STRUCTURAL_CALL_SOURCE,
        "mutable borrow structural call loop",
        "scan",
    );
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
    let (_, call) = member_structural_scalar_call(function, component);
    let call_operation = operation_of(call);
    let candidate = propose_loop_invariant_scalar_motion(&session, 8)
        .expect("exact candidate")
        .pop()
        .expect("one candidate");
    let relocation = candidate
        .relocations()
        .iter()
        .find(|relocation| relocation.node().psi_operation() == call_operation)
        .expect("the mutable-borrow call is a planned relocation");
    let member = relocation.node().location().block;
    let validated = validate_loop_invariant_scalar_motion(&session, &candidate)
        .expect("validated exact candidate");
    let applied =
        apply_loop_invariant_scalar_motion(session, validated).expect("applied exact candidate");
    let (input, mut unit) = applied.into_session().into_parts();
    // Forge the unsound half of the coupling: leave the relocated
    // establishment in the preheader but hand the mutable borrower back to
    // its member block. The staying callee reads the persistent cell where
    // the source traversal re-initialized it, so the coverage replay rejects
    // the forged unit.
    let moved = take_operation(&mut unit, call_operation);
    let member_block = unit
        .functions
        .iter_mut()
        .flat_map(|function| &mut function.blocks)
        .find(|block| block.id == member)
        .expect("member block exists");
    let terminator = member_block.nodes.len() - 1;
    member_block.nodes.insert(terminator, moved);
    refresh_coordinates_and_effects(&mut unit);
    let outcome = VerifiedPsiOptimizationSession::from_transformed(input, unit);
    assert!(
        matches!(
            outcome,
            Err(
                OptimizationUnitValidationError::RankedCycleFrozenBlockMismatch {
                    machine: rejected_machine,
                    ..
                }
            ) if rejected_machine == machine
        ),
        "the stranded-borrower forgery is rejected: {outcome:?}"
    );
}

#[test]
fn forged_mutable_borrow_access_is_rejected_by_the_freeze_fence() {
    let session = lowered_session_entry(
        MUTABLE_BORROW_STRUCTURAL_CALL_SOURCE,
        "mutable borrow structural call loop",
        "scan",
    );
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
    let (_, call) = member_structural_scalar_call(function, component);
    let call_operation = operation_of(call);
    let candidate = propose_loop_invariant_scalar_motion(&session, 8)
        .expect("exact candidate")
        .pop()
        .expect("one candidate");
    let relocation = candidate
        .relocations()
        .iter()
        .find(|relocation| relocation.node().psi_operation() == call_operation)
        .expect("the mutable-borrow call is a planned relocation");
    let member = relocation.node().location().block;
    let validated = validate_loop_invariant_scalar_motion(&session, &candidate)
        .expect("validated exact candidate");
    let applied =
        apply_loop_invariant_scalar_motion(session, validated).expect("applied exact candidate");
    let (input, mut unit) = applied.into_session().into_parts();
    // Forging the borrow's access to `SharedBorrow` rewrites a structural
    // field the relocation preserves byte-exact — the seed-derived operation
    // comparison rejects the drifted spelling.
    let forged = find_operation_mut(&mut unit, call_operation);
    if let AbstractOperation::CallStructuralScalar {
        structural_arguments,
        ..
    } = &mut forged.operation
    {
        structural_arguments[0].access = terminal_psi::StructuralAccess::SharedBorrow;
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
fn forged_structural_scalar_call_argument_root_is_rejected_by_the_freeze_fence() {
    let session = lowered_session_entry(
        TWO_LOCAL_STRUCTURAL_CALL_SOURCE,
        "two-local structural call loop",
        "scan",
    );
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
    // The two calls borrow `scratch` and `other` respectively; the first's
    // root is the place the forgery swaps for the second live place.
    let calls = member_structural_scalar_calls(function, component);
    let mut call_roots = calls
        .iter()
        .map(|(_, node)| match &node.operation {
            AbstractOperation::CallStructuralScalar {
                psi_operation,
                structural_arguments,
                ..
            } => (*psi_operation, structural_arguments[0].place),
            operation => panic!("member node is a structural scalar call: {operation:?}"),
        })
        .collect::<Vec<_>>();
    call_roots.sort();
    let [(forged_operation, forged_from), (_, forged_to)] = call_roots.as_slice() else {
        panic!("two calls borrow two distinct local places")
    };
    let candidate = propose_loop_invariant_scalar_motion(&session, 8)
        .expect("exact candidate")
        .pop()
        .expect("one candidate");
    let relocation = candidate
        .relocations()
        .iter()
        .find(|relocation| relocation.node().psi_operation() == *forged_operation)
        .expect("the structural scalar call is a planned relocation");
    let member = relocation.node().location().block;
    let validated = validate_loop_invariant_scalar_motion(&session, &candidate)
        .expect("validated exact candidate");
    let applied =
        apply_loop_invariant_scalar_motion(session, validated).expect("applied exact candidate");
    let (input, mut unit) = applied.into_session().into_parts();
    // Swapping the borrowed root to the other live local is not a planned
    // rewrite — the call's structural argument stays byte-exact because the
    // run preserves its producer's declared place identity — so the
    // seed-derived operation comparison rejects the moved node.
    let forged = find_operation_mut(&mut unit, *forged_operation);
    if let AbstractOperation::CallStructuralScalar {
        structural_arguments,
        ..
    } = &mut forged.operation
    {
        assert_eq!(structural_arguments[0].place, *forged_from);
        structural_arguments[0].place = *forged_to;
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
fn forged_primitive_local_initializer_is_rejected_by_the_freeze_fence() {
    let session = lowered_session_entry(
        STRUCTURAL_SCALAR_CALL_SOURCE,
        "structural scalar call loop",
        "scan",
    );
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
    let (_, local) = member_primitive_local(function, component);
    let (local_operation, local_value) = match &local.operation {
        AbstractOperation::EstablishPrimitiveLocal {
            psi_operation,
            value,
            ..
        } => (*psi_operation, value.value),
        operation => panic!("the member node is a primitive-local establishment: {operation:?}"),
    };
    let candidate = propose_loop_invariant_scalar_motion(&session, 8)
        .expect("exact candidate")
        .pop()
        .expect("one candidate");
    let relocation = candidate
        .relocations()
        .iter()
        .find(|relocation| relocation.node().psi_operation() == local_operation)
        .expect("the primitive-local establishment is a planned relocation");
    let member = relocation.node().location().block;
    let validated = validate_loop_invariant_scalar_motion(&session, &candidate)
        .expect("validated exact candidate");
    let applied =
        apply_loop_invariant_scalar_motion(session, validated).expect("applied exact candidate");
    let (input, mut unit) = applied.into_session().into_parts();
    // Forging the moved initializer back to the member parameter skips the
    // seed-derived substitution — the establishment's `value` operand rebinds
    // to the relocated constant's preserved result, so the replayed
    // operation comparison rejects the drifted spelling.
    let forged = find_operation_mut(&mut unit, local_operation);
    if let AbstractOperation::EstablishPrimitiveLocal { value, .. } = &mut forged.operation {
        value.value = local_value;
    }
    forged.uses[0].value = local_value;
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

/// A cyclic `let mut` primitive local kept alive by a shared borrow to a
/// pure structural scalar call, then read by a member `PrimitiveScalarRead`:
/// `scratch`'s root is produced inside the component, but the establishment
/// relocates and the run covers the root — so the read relocates behind its
/// producer spelling the same member-produced root byte-exact, the call's
/// shared-borrow argument does the same, and the `measured & observed`
/// computation chained on both results relocates through the scalar path.
const MEMBER_READ_LOCAL_SOURCE: &str = r#"
    machine measure(value: &u64, bias: u64) -> u64 { bias }

    machine scan(remaining: u64 [0..=5], scale: u64) -> u64
    terminates by remaining -> Nat::Descending in 0..6;
    {
        let mut scratch: u64 = 7;
        let measured: u64 = measure(&scratch, scale);
        let observed: u64 = scratch;
        let combined: u64 = measured & observed;
        transition remaining > 0 {
            true -> scan(remaining - 1, scale)
            _ -> combined
        }
    }
"#;

/// Same component shape, but the borrowed local's initializer is the
/// loop-carried countdown: the establishment cannot leave, so the run never
/// covers `scratch`'s root — the member read, the borrowing call, and the
/// chained computation all stay inside even though the call's scalar
/// argument is invariant.
const CARRIED_LOCAL_READ_SOURCE: &str = r#"
    machine measure(value: &u64, bias: u64) -> u64 { bias }

    machine scan(remaining: u64 [0..=5], scale: u64) -> u64
    terminates by remaining -> Nat::Descending in 0..6;
    {
        let mut scratch: u64 = remaining;
        let measured: u64 = measure(&scratch, scale);
        let observed: u64 = scratch;
        let combined: u64 = measured & observed;
        transition remaining > 0 {
            true -> scan(remaining - 1, scale)
            _ -> combined
        }
    }
"#;

/// A two-member component whose `swap` member view parameter binds only the
/// subslice `step` establishes fresh each traversal: `view` keeps resolving
/// to the machine's `entries` root through `carry`, while `other`'s single
/// binding edge spells the member-produced subslice root. When the run
/// covers the subslice, `other` resolves to its declared place — so
/// `other.len` relocates out of `swap` rebinding its observed root to the
/// moved subslice's byte-exact result place.
const MEMBER_PRODUCED_VIEW_SOURCE: &str = r#"
    data Root {}

    machine Root::scan(scale: u64, entries: &[u8], remaining: u64 [0..=5])
    {
        transition { _ -> step(scale, entries, remaining) }
        state step(s: u64, view: &[u8], pending: u64 [0..=5]) {
            let length: u64 = view.len;
            transition { _ -> swap(s, view[0..view.len], view, pending) }
        }
        state swap(t: u64, other: &[u8], carry: &[u8], rest: u64 [0..=5]) {
            let width: u64 = other.len;
            transition rest > 0 {
                true -> step(t, carry, rest - 1)
                _ -> finish(width)
            }
        }
        state finish(r: u64) {}
    }
"#;

/// The `PrimitiveScalarRead` node inside a member block and its block —
/// the read counterpart of [`member_primitive_local`].
fn member_scalar_read<'function>(
    function: &'function optimization_unit::PsiOptimizationFunction,
    component: &optimization_unit::OptimizerCycleComponent,
) -> (
    &'function optimization_unit::OptimizationBlock,
    &'function optimization_unit::OptimizationNode,
) {
    for member in &component.members {
        let block = function
            .blocks
            .iter()
            .find(|block| block.id == *member)
            .expect("member block exists");
        for node in &block.nodes {
            if let AbstractOperation::PrimitiveScalarRead { .. } = &node.operation {
                return (block, node);
            }
        }
    }
    panic!("the primitive-scalar read lives in a member block")
}

#[test]
fn member_read_relocates_with_its_member_produced_root() {
    let session = lowered_session_entry(MEMBER_READ_LOCAL_SOURCE, "member read local loop", "scan");
    let [component] = session.cycle_components().components() else {
        panic!("one cyclic component")
    };
    let function = session
        .unit()
        .functions
        .iter()
        .find(|function| function.machine == component.id.machine)
        .expect("component machine exists");
    let (_, local) = member_primitive_local(function, component);
    let (_, call) = member_structural_scalar_call(function, component);
    let (_, read) = member_scalar_read(function, component);
    let (local_operation, local_place) = match &local.operation {
        AbstractOperation::EstablishPrimitiveLocal {
            psi_operation,
            result,
            ..
        } => (*psi_operation, result.place),
        operation => panic!("the member node is a primitive-local establishment: {operation:?}"),
    };
    let call_operation = operation_of(call);
    let (read_operation, read_source, read_result) = match &read.operation {
        AbstractOperation::PrimitiveScalarRead {
            psi_operation,
            result,
            source,
            ..
        } => (*psi_operation, *source, result.value),
        operation => panic!("the member node is a scalar read: {operation:?}"),
    };
    assert_eq!(
        read_source, local_place,
        "the member read observes the local's member-produced root"
    );
    // The admission boundary, both ways: the read's member-produced root is
    // invisible at the preheader and no member parameter resolves it, so an
    // empty run refuses — covering the producer's root admits the read and
    // resolves to the root it already spells.
    assert!(
        crate::validation::invariant_place_observation_admission(
            function,
            component,
            read,
            &std::collections::BTreeSet::new(),
        )
        .is_none(),
        "an uncovered member-produced root keeps the read inside"
    );
    assert_eq!(
        crate::validation::invariant_place_observation_admission(
            function,
            component,
            read,
            &std::collections::BTreeSet::from([local_place]),
        ),
        Some(local_place),
        "the run-covered member-produced root admits the read byte-exact"
    );

    let candidates =
        propose_loop_invariant_scalar_motion(&session, 8).expect("exact relocation candidates");
    let [candidate] = candidates.as_slice() else {
        panic!("one component yields one atomic candidate")
    };
    let local_relocation = candidate
        .relocations()
        .iter()
        .find(|relocation| relocation.node().psi_operation() == local_operation)
        .expect("the primitive-local establishment relocates");
    let call_relocation = candidate
        .relocations()
        .iter()
        .find(|relocation| relocation.node().psi_operation() == call_operation)
        .expect("the shared-borrow call relocates with its producer");
    let read_relocation = candidate
        .relocations()
        .iter()
        .find(|relocation| relocation.node().psi_operation() == read_operation)
        .expect("the member read relocates behind its producer");
    // The member-produced root is already what the moved operations spell:
    // the producer's declared place identity survives byte-exact, so the read
    // needs no root rewrite and the call no argument rewrite.
    assert!(read_relocation.node().root_rewrite().is_none());
    assert!(call_relocation.node().argument_rewrites().is_empty());
    assert!(
        local_relocation.destination().node < read_relocation.destination().node
            && local_relocation.destination().node < call_relocation.destination().node,
        "the relocated producer lands ahead of both member-produced-root consumers"
    );

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
        .find(|block| block.id == read_relocation.destination().block)
        .expect("destination block exists");
    let moved_read =
        &destination.nodes[usize::try_from(read_relocation.destination().node).unwrap()];
    match &moved_read.operation {
        AbstractOperation::PrimitiveScalarRead { source, result, .. } => {
            assert_eq!(
                *source, local_place,
                "the relocated read keeps spelling the member-produced root"
            );
            assert_eq!(result.value, read_result);
        }
        operation => panic!("relocated read keeps its operation: {operation:?}"),
    }
    assert!(
        propose_loop_invariant_scalar_motion(applied.session(), 1)
            .expect("relocated session is an exact fixed point")
            .is_empty()
    );
}

#[test]
fn member_read_stays_when_its_producer_cannot_relocate() {
    let session =
        lowered_session_entry(CARRIED_LOCAL_READ_SOURCE, "carried local read loop", "scan");
    let [component] = session.cycle_components().components() else {
        panic!("one cyclic component")
    };
    let function = session
        .unit()
        .functions
        .iter()
        .find(|function| function.machine == component.id.machine)
        .expect("component machine exists");
    let (_, local) = member_primitive_local(function, component);
    let (_, call) = member_structural_scalar_call(function, component);
    let (_, read) = member_scalar_read(function, component);
    let local_operation = operation_of(local);
    let call_operation = operation_of(call);
    let read_operation = operation_of(read);
    assert!(
        crate::validation::invariant_place_observation_admission(
            function,
            component,
            read,
            &std::collections::BTreeSet::new(),
        )
        .is_none(),
        "the member-produced root is not run-covered while its producer stays"
    );

    let candidates =
        propose_loop_invariant_scalar_motion(&session, 8).expect("exact relocation candidates");
    for candidate in &candidates {
        // The establishment's initializer is the loop-carried countdown, so
        // the producer never enters the run — and with the root uncovered,
        // the read and the borrowing call have no landing either.
        assert!(
            candidate
                .relocations()
                .iter()
                .all(
                    |relocation| relocation.node().psi_operation() != local_operation
                        && relocation.node().psi_operation() != call_operation
                        && relocation.node().psi_operation() != read_operation
                ),
            "a staying producer keeps the local, the call, and the read inside"
        );
    }
}

#[test]
fn member_read_moved_without_its_producer_is_rejected_by_the_freeze_fence() {
    let session = lowered_session_entry(MEMBER_READ_LOCAL_SOURCE, "member read local loop", "scan");
    let [component] = session.cycle_components().components() else {
        panic!("one cyclic component")
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
    let (read_block, read) = member_scalar_read(function, component);
    let member = read_block.id;
    let preheader = entry.source;
    let read_operation = operation_of(read);
    let (input, mut unit) = session.into_parts();
    // Hand-move only the read: its source still spells the local's
    // member-produced root, but the establishment stayed inside — the freeze
    // fence re-derives the admission with an empty relocated-root set for the
    // component and refuses because no moved node covered the observed root.
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
fn member_parameter_rebinds_to_a_run_covered_member_produced_root() {
    let session = lowered_session(MEMBER_PRODUCED_VIEW_SOURCE, "member-produced view loop");
    let [component] = session.cycle_components().components() else {
        panic!("one cyclic component")
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
    // `swap`'s `other` member parameter binds only the subslice root `step`
    // establishes each traversal — member-produced, so the parameter resolves
    // only when the relocation run covers the subslice's declared place.
    let (subslice, lengths) = component
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
        })
        .fold((None, Vec::new()), |(mut subslice, mut lengths), node| {
            match &node.operation {
                AbstractOperation::ByteSequenceSubslice { .. } => subslice = Some(node),
                AbstractOperation::ByteSequenceLength { .. } => lengths.push(node),
                _ => {}
            }
            (subslice, lengths)
        });
    let subslice = subslice.expect("the member subslice exists");
    let (subslice_operation, subslice_place, subslice_source) = match &subslice.operation {
        AbstractOperation::ByteSequenceSubslice {
            psi_operation,
            result,
            source,
            ..
        } => (*psi_operation, result.place, *source),
        operation => panic!("the member node is a subslice: {operation:?}"),
    };
    let [_, other_length] = lengths.as_slice() else {
        panic!("`view.len` and `other.len` are the member length observations")
    };
    let (length_operation, length_source) = match &other_length.operation {
        AbstractOperation::ByteSequenceLength {
            psi_operation,
            source,
            ..
        } => (*psi_operation, *source),
        operation => panic!("the member node is a length observation: {operation:?}"),
    };
    assert_ne!(
        length_source, subslice_source,
        "`other.len` observes `swap`'s own parameter, not `step`'s `view`"
    );
    // The observation root is `swap`'s member parameter; every reaching edge
    // binds it to the member-produced subslice root, so the representative is
    // the covered root — not a preheader-visible place.
    assert!(
        !crate::validation::place_observation_root_visible(
            function,
            function
                .blocks
                .iter()
                .find(|block| block.id == entry.source)
                .expect("preheader exists"),
            subslice_place,
        ),
        "the member-produced subslice root is not preheader-visible"
    );

    let candidates =
        propose_loop_invariant_scalar_motion(&session, 8).expect("exact relocation candidates");
    let [candidate] = candidates.as_slice() else {
        panic!("one component yields one atomic candidate")
    };
    let subslice_relocation = candidate
        .relocations()
        .iter()
        .find(|relocation| relocation.node().psi_operation() == subslice_operation)
        .expect("the member subslice relocates");
    let length_relocation = candidate
        .relocations()
        .iter()
        .find(|relocation| relocation.node().psi_operation() == length_operation)
        .expect("the member observation relocates behind its covered root's producer");
    assert_eq!(
        length_relocation.node().root_rewrite(),
        Some((length_source, subslice_place)),
        "the moved observation rebinds its member parameter to the covered member-produced root"
    );
    assert!(
        subslice_relocation.destination().node < length_relocation.destination().node,
        "the relocated subslice lands ahead of the observation spelling its root"
    );

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
        .find(|block| block.id == length_relocation.destination().block)
        .expect("destination block exists");
    let moved_length =
        &destination.nodes[usize::try_from(length_relocation.destination().node).unwrap()];
    match &moved_length.operation {
        AbstractOperation::ByteSequenceLength { source, .. } => assert_eq!(
            *source, subslice_place,
            "the relocated observation spells the covered member-produced root"
        ),
        operation => panic!("relocated observation keeps its operation: {operation:?}"),
    }
    assert!(
        propose_loop_invariant_scalar_motion(applied.session(), 1)
            .expect("relocated session is an exact fixed point")
            .is_empty()
    );
}

/// A two-state cycle whose member state establishes a record from invariant
/// scalars, copies it through the next member block's structural parameter,
/// and nests that copy inside a second record: `Pair { a: s, b: s }` reads
/// only the `s` member parameter that resolves transitively to `scale`,
/// `pair`'s place flows `Owned` into the copy block's member structural
/// parameter, and `Holder { inner: copy }` copies the parameter's root — so
/// the `pair` establishment relocates rebinding both scalar fields to the
/// preheader anchor, and the `holder` establishment relocates behind it
/// rebinding its copied field root from the member parameter to `pair`'s
/// preserved place.
const MEMBER_RECORD_SOURCE: &str = r#"
    data Pair [copy] { a: u32 in Wrapping; b: u32 in Wrapping; }
    data Holder [copy] { inner: Pair; }
    data Root {}

    machine Root::scan(scale: u32 in Wrapping, remaining: u32 [0..=5])
    {
        transition { _ -> step(scale, remaining) }
        state step(s: u32 in Wrapping, pending: u32 [0..=5]) {
            let pair: Pair = Pair { a: s, b: s };
            let copy: Pair = pair;
            let holder: Holder = Holder { inner: copy };
            transition pending > 0 {
                true -> scan(s, pending - 1)
                _ -> finish(s)
            }
        }
        state finish(r: u32 in Wrapping) {}
    }
"#;

/// Same component shape, but `step` advances `s` on the back edge: the `pair`
/// establishment's scalar fields read a genuinely loop-carried member
/// parameter so it stays inside, and with `pair`'s root uncovered the
/// `holder` copy's member parameter resolves loop-carried — it stays inside
/// too.
const CARRIED_FIELD_RECORD_SOURCE: &str = r#"
    data Pair [copy] { a: u32 in Wrapping; b: u32 in Wrapping; }
    data Holder [copy] { inner: Pair; }
    data Root {}

    machine Root::scan(scale: u32 in Wrapping, remaining: u32 [0..=5])
    {
        transition { _ -> step(scale, remaining) }
        state step(s: u32 in Wrapping, pending: u32 [0..=5]) {
            let pair: Pair = Pair { a: s, b: s };
            let copy: Pair = pair;
            let holder: Holder = Holder { inner: copy };
            transition pending > 0 {
                true -> scan(s + 1, pending - 1)
                _ -> finish(s)
            }
        }
        state finish(r: u32 in Wrapping) {}
    }
"#;

/// Same cycle shape as `BYPASSED_MEMBER_SOURCE`, but `step` establishes the
/// record: the entry state's `done` arm can leave the component before `step`
/// ever runs, so the establishment is speculation and stays inside while the
/// entry state's own invariant leaves still relocate.
const BYPASSED_RECORD_SOURCE: &str = r#"
    data Pair [copy] { a: u32 in Wrapping; b: u32 in Wrapping; }
    data Root {}

    machine Root::scan(scale: u32 in Wrapping, remaining: u32 [0..=5])
    {
        transition remaining > 0 {
            true -> step(scale, remaining - 1)
            _ -> done()
        }
        state step(s: u32 in Wrapping, pending: u32 [0..=5]) {
            let pair: Pair = Pair { a: s, b: s };
            transition pending > 0 {
                true -> scan(s, pending - 1)
                _ -> finish(s)
            }
        }
        state done() {}
        state finish(r: u32 in Wrapping) {}
    }
"#;

/// Every `EstablishRecord` node inside `component`'s member blocks — the
/// record counterpart of [`member_primitive_local`].
fn member_record_establishments<'function>(
    function: &'function optimization_unit::PsiOptimizationFunction,
    component: &optimization_unit::OptimizerCycleComponent,
) -> Vec<(
    &'function optimization_unit::OptimizationBlock,
    &'function optimization_unit::OptimizationNode,
)> {
    let mut records = Vec::new();
    for member in &component.members {
        let block = function
            .blocks
            .iter()
            .find(|block| block.id == *member)
            .expect("member block exists");
        for node in &block.nodes {
            if let AbstractOperation::EstablishRecord { .. } = &node.operation {
                records.push((block, node));
            }
        }
    }
    records
}

#[test]
fn invariant_record_establishments_relocate_preserving_their_places() {
    let session = lowered_session(MEMBER_RECORD_SOURCE, "member record loop");
    let [component] = session.cycle_components().components() else {
        panic!("one cyclic component")
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
    let records = member_record_establishments(function, component);
    let [(_, pair), (_, holder)] = records.as_slice() else {
        panic!("two member record establishments")
    };
    let (pair_operation, pair_place, pair_fields) = match &pair.operation {
        AbstractOperation::EstablishRecord {
            psi_operation,
            result,
            fields,
        } => (*psi_operation, result.place, fields),
        operation => panic!("the member node is a record establishment: {operation:?}"),
    };
    let (holder_operation, holder_place, copied_root) = match &holder.operation {
        AbstractOperation::EstablishRecord {
            psi_operation,
            result,
            fields,
        } => {
            let [field] = fields.as_slice() else {
                panic!("holder declares one field")
            };
            let terminal_psi::RecordFieldValue::Structural(argument) = &field.value else {
                panic!("holder's field copies a place root")
            };
            (*psi_operation, result.place, argument.place)
        }
        operation => panic!("the member node is a record establishment: {operation:?}"),
    };
    assert_ne!(
        copied_root, pair_place,
        "the holder copies the copy block's member structural parameter"
    );

    let candidates =
        propose_loop_invariant_scalar_motion(&session, 8).expect("exact relocation candidates");
    let [candidate] = candidates.as_slice() else {
        panic!("one component yields one atomic candidate")
    };
    let pair_relocation = candidate
        .relocations()
        .iter()
        .find(|relocation| relocation.node().psi_operation() == pair_operation)
        .expect("the pair establishment is a planned relocation");
    let LoopInvariantNodeResult::Structural(result) = pair_relocation.node().result() else {
        panic!("the record establishment relocates its structural result")
    };
    assert_eq!(result.place, pair_place, "the declared place is byte-exact");
    // Both scalar fields resolve to the same `scale` representative — the
    // substitution rewrites every spelled field value to the preheader anchor.
    let rewrites = pair_relocation.node().operand_rewrites();
    assert_eq!(
        rewrites.len(),
        pair_fields.len(),
        "every scalar field value carries one member-parameter rewrite"
    );
    let representative = rewrites[0].1;
    for (rewritten, field) in rewrites.iter().zip(pair_fields.iter()) {
        let terminal_psi::RecordFieldValue::Scalar { value, .. } = &field.value else {
            panic!("pair's fields are scalar")
        };
        assert_eq!(rewritten.0, *value, "the rewrite spells the field value");
        assert_eq!(
            rewritten.1, representative,
            "every field rewrites to the same preheader anchor"
        );
    }
    let anchor = function
        .parameters
        .iter()
        .find(|parameter| parameter.value == representative)
        .expect("the field representative is the machine's `scale` parameter")
        .value;

    let holder_relocation = candidate
        .relocations()
        .iter()
        .find(|relocation| relocation.node().psi_operation() == holder_operation)
        .expect("the holder establishment is a planned relocation");
    assert_eq!(
        holder_relocation.node().argument_rewrites(),
        &[(copied_root, pair_place)],
        "the moved copy rebinds its member parameter root to the run-covered record place"
    );
    assert_eq!(pair_relocation.destination().block, entry.source);
    assert_eq!(holder_relocation.destination().block, entry.source);
    assert!(
        pair_relocation.destination().node < holder_relocation.destination().node,
        "the relocated pair lands ahead of the holder copying its root"
    );

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
        .find(|block| block.id == pair_relocation.destination().block)
        .expect("destination block exists");
    let moved_pair =
        &destination.nodes[usize::try_from(pair_relocation.destination().node).unwrap()];
    match &moved_pair.operation {
        AbstractOperation::EstablishRecord { result, fields, .. } => {
            assert_eq!(result.place, pair_place, "the declared place is byte-exact");
            for field in fields {
                let terminal_psi::RecordFieldValue::Scalar { value, .. } = &field.value else {
                    panic!("pair's fields are scalar")
                };
                assert_eq!(
                    *value, anchor,
                    "the moved initializer rebinds to the preheader anchor"
                );
            }
        }
        operation => panic!("relocated node keeps its record operation: {operation:?}"),
    }
    let moved_holder =
        &destination.nodes[usize::try_from(holder_relocation.destination().node).unwrap()];
    match &moved_holder.operation {
        AbstractOperation::EstablishRecord { result, fields, .. } => {
            assert_eq!(
                result.place, holder_place,
                "the declared place is byte-exact"
            );
            let [field] = fields.as_slice() else {
                panic!("holder declares one field")
            };
            let terminal_psi::RecordFieldValue::Structural(argument) = &field.value else {
                panic!("holder's field copies a place root")
            };
            assert_eq!(
                argument.place, pair_place,
                "the moved copy spells the relocated pair's preserved place"
            );
        }
        operation => panic!("relocated node keeps its record operation: {operation:?}"),
    }
    assert_eq!(moved_pair.provenance, pair_relocation.node().provenance());
    assert_eq!(moved_pair.fuel, pair_relocation.node().fuel());
    assert!(
        propose_loop_invariant_scalar_motion(applied.session(), 1)
            .expect("relocated session is an exact fixed point")
            .is_empty()
    );
}

#[test]
fn carried_field_record_establishments_stay_inside() {
    let session = lowered_session(CARRIED_FIELD_RECORD_SOURCE, "carried field record loop");
    let [component] = session.cycle_components().components() else {
        panic!("one cyclic component")
    };
    let function = session
        .unit()
        .functions
        .iter()
        .find(|function| function.machine == component.id.machine)
        .expect("component machine exists");
    let records = member_record_establishments(function, component);
    assert_eq!(records.len(), 2, "two member record establishments");
    let record_operations = records
        .iter()
        .map(|(_, node)| operation_of(node))
        .collect::<Vec<_>>();

    let candidates =
        propose_loop_invariant_scalar_motion(&session, 8).expect("exact relocation candidates");
    let [candidate] = candidates.as_slice() else {
        panic!("one component yields one atomic candidate")
    };
    for operation in record_operations {
        assert!(
            candidate
                .relocations()
                .iter()
                .all(|relocation| relocation.node().psi_operation() != operation),
            "a record reading a carried field — or copying a root the run never covers — stays inside"
        );
    }
}

#[test]
fn bypassed_member_record_establishment_stays_inside() {
    let session = lowered_session(BYPASSED_RECORD_SOURCE, "bypassed record loop");
    let [component] = session.cycle_components().components() else {
        panic!("one cyclic component")
    };
    let function = session
        .unit()
        .functions
        .iter()
        .find(|function| function.machine == component.id.machine)
        .expect("component machine exists");
    let records = member_record_establishments(function, component);
    assert_eq!(records.len(), 1, "one member record establishment");
    let record_operation = operation_of(records[0].1);

    let candidates =
        propose_loop_invariant_scalar_motion(&session, 8).expect("exact relocation candidates");
    let [candidate] = candidates.as_slice() else {
        panic!("one component yields one atomic candidate")
    };
    assert!(
        candidate
            .relocations()
            .iter()
            .all(|relocation| relocation.node().psi_operation() != record_operation),
        "a record establishment in a member the exit can bypass is speculation"
    );
    assert!(
        !candidate.relocations().is_empty(),
        "the entry state's invariant leaves still relocate"
    );
}

#[test]
fn forged_record_field_initializer_is_rejected_by_the_freeze_fence() {
    let session = lowered_session(MEMBER_RECORD_SOURCE, "member record loop");
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
    let records = member_record_establishments(function, component);
    let [(_, pair), _] = records.as_slice() else {
        panic!("two member record establishments")
    };
    let (pair_operation, member_field_value) = match &pair.operation {
        AbstractOperation::EstablishRecord {
            psi_operation,
            fields,
            ..
        } => {
            let terminal_psi::RecordFieldValue::Scalar { value, .. } = fields[0].value else {
                panic!("pair's fields are scalar")
            };
            (*psi_operation, value)
        }
        operation => panic!("the member node is a record establishment: {operation:?}"),
    };
    let candidate = propose_loop_invariant_scalar_motion(&session, 8)
        .expect("exact candidate")
        .pop()
        .expect("one candidate");
    let relocation = candidate
        .relocations()
        .iter()
        .find(|relocation| relocation.node().psi_operation() == pair_operation)
        .expect("the pair establishment is a planned relocation");
    let member = relocation.node().location().block;
    let validated = validate_loop_invariant_scalar_motion(&session, &candidate)
        .expect("validated exact candidate");
    let applied =
        apply_loop_invariant_scalar_motion(session, validated).expect("applied exact candidate");
    let (input, mut unit) = applied.into_session().into_parts();
    // Forging the moved field initializer back to the member parameter skips
    // the seed-derived substitution — the establishment's field values rebind
    // to the preheader anchor, so the replayed operation comparison rejects
    // the drifted spelling.
    let forged = find_operation_mut(&mut unit, pair_operation);
    if let AbstractOperation::EstablishRecord { fields, .. } = &mut forged.operation {
        for field in fields {
            if let terminal_psi::RecordFieldValue::Scalar { value, .. } = &mut field.value {
                *value = member_field_value;
            }
        }
    }
    for value_use in &mut forged.uses {
        value_use.value = member_field_value;
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
fn forged_record_field_copy_root_is_rejected_by_the_freeze_fence() {
    let session = lowered_session(MEMBER_RECORD_SOURCE, "member record loop");
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
    let records = member_record_establishments(function, component);
    let [(_, pair), (_, holder)] = records.as_slice() else {
        panic!("two member record establishments")
    };
    let holder_place = match &holder.operation {
        AbstractOperation::EstablishRecord { result, .. } => result.place,
        operation => panic!("the member node is a record establishment: {operation:?}"),
    };
    let pair_place = match &pair.operation {
        AbstractOperation::EstablishRecord { result, .. } => result.place,
        operation => panic!("the member node is a record establishment: {operation:?}"),
    };
    let holder_operation = operation_of(holder);
    let candidate = propose_loop_invariant_scalar_motion(&session, 8)
        .expect("exact candidate")
        .pop()
        .expect("one candidate");
    let relocation = candidate
        .relocations()
        .iter()
        .find(|relocation| relocation.node().psi_operation() == holder_operation)
        .expect("the holder establishment is a planned relocation");
    let member = relocation.node().location().block;
    let validated = validate_loop_invariant_scalar_motion(&session, &candidate)
        .expect("validated exact candidate");
    let applied =
        apply_loop_invariant_scalar_motion(session, validated).expect("applied exact candidate");
    let (input, mut unit) = applied.into_session().into_parts();
    // Swapping the copied root to the holder's own declared place is not a
    // planned rewrite — the seed-derived admission rebinds the member
    // parameter to the run-covered pair place, so the replayed operation
    // comparison rejects the forged spelling.
    let forged = find_operation_mut(&mut unit, holder_operation);
    if let AbstractOperation::EstablishRecord { fields, .. } = &mut forged.operation {
        let terminal_psi::RecordFieldValue::Structural(argument) = &mut fields[0].value else {
            panic!("holder's field copies a place root")
        };
        assert_eq!(argument.place, pair_place);
        argument.place = holder_place;
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

/// A ranked scalar-machine cycle whose one member block establishes an
/// unrestricted scalar array from invariant elements and hands the fresh
/// root to `first` as an `Owned` argument: the `scale` member parameter
/// resolves transitively to the machine's `scale` anchor, so the
/// establishment relocates rebinding every element to that representative
/// while its declared place stays byte-exact. The owned-argument call is a
/// copy of the persistent member-produced root — custody-preserving under
/// the whole-component bound — and stays inside the loop spelling the same
/// place.
const MEMBER_SCALAR_ARRAY_SOURCE: &str = r#"
    machine first(row: [u64; 2]) -> u64 { 0 }
    machine scan(remaining: u64 [0..=5], scale: u64 [0..=10]) -> u64
    terminates by remaining -> Nat::Descending in 0..6;
    {
        let v: u64 = first([scale, scale]);
        transition remaining > 0 {
            true -> scan(remaining - 1, scale)
            _ -> v
        }
    }
"#;

/// Same component shape, but the establishment reads the loop-carried
/// `remaining` countdown: the member parameter never resolves to a preheader
/// representative, so the establishment stays inside even though its member
/// block is guaranteed to execute.
const CARRIED_ELEMENT_SCALAR_ARRAY_SOURCE: &str = r#"
    machine first(row: [u64; 2]) -> u64 { 0 }
    machine scan(remaining: u64 [0..=5], scale: u64 [0..=10]) -> u64
    terminates by remaining -> Nat::Descending in 0..6;
    {
        let v: u64 = first([remaining, scale]);
        transition remaining > 0 {
            true -> scan(remaining - 1, scale)
            _ -> v
        }
    }
"#;

/// Every `EstablishScalarArray` node inside `component`'s member blocks —
/// the array counterpart of [`member_record_establishments`].
fn member_scalar_array_establishments<'function>(
    function: &'function optimization_unit::PsiOptimizationFunction,
    component: &optimization_unit::OptimizerCycleComponent,
) -> Vec<(
    &'function optimization_unit::OptimizationBlock,
    &'function optimization_unit::OptimizationNode,
)> {
    let mut arrays = Vec::new();
    for member in &component.members {
        let block = function
            .blocks
            .iter()
            .find(|block| block.id == *member)
            .expect("member block exists");
        for node in &block.nodes {
            if let AbstractOperation::EstablishScalarArray { .. } = &node.operation {
                arrays.push((block, node));
            }
        }
    }
    arrays
}

#[test]
fn invariant_scalar_array_establishment_relocates_preserving_its_place() {
    let session = lowered_session_entry(
        MEMBER_SCALAR_ARRAY_SOURCE,
        "member scalar-array loop",
        "scan",
    );
    let [component] = session.cycle_components().components() else {
        panic!("one cyclic component")
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
    let arrays = member_scalar_array_establishments(function, component);
    let [(_, array)] = arrays.as_slice() else {
        panic!("one member scalar-array establishment")
    };
    let (array_operation, array_place, elements) = match &array.operation {
        AbstractOperation::EstablishScalarArray {
            psi_operation,
            result,
            elements,
        } => (*psi_operation, result.place, elements),
        operation => panic!("the member node is an array establishment: {operation:?}"),
    };
    assert_eq!(elements.len(), 2, "the array declares two scalar elements");

    let candidates =
        propose_loop_invariant_scalar_motion(&session, 8).expect("exact relocation candidates");
    let [candidate] = candidates.as_slice() else {
        panic!("one component yields one atomic candidate")
    };
    let relocation = candidate
        .relocations()
        .iter()
        .find(|relocation| relocation.node().psi_operation() == array_operation)
        .expect("the array establishment is a planned relocation");
    let LoopInvariantNodeResult::Structural(result) = relocation.node().result() else {
        panic!("the array establishment relocates its structural result")
    };
    assert_eq!(
        result.place, array_place,
        "the declared place is byte-exact"
    );
    // Both elements resolve to the same `scale` representative — the
    // substitution rewrites every spelled element to the preheader anchor.
    let rewrites = relocation.node().operand_rewrites();
    assert_eq!(
        rewrites.len(),
        elements.len(),
        "every element carries one member-parameter rewrite"
    );
    let representative = rewrites[0].1;
    for (rewritten, element) in rewrites.iter().zip(elements.iter()) {
        assert_eq!(
            rewritten.0, *element,
            "the rewrite spells the element value"
        );
        assert_eq!(
            rewritten.1, representative,
            "every element rewrites to the same preheader anchor"
        );
    }
    let anchor = function
        .parameters
        .iter()
        .find(|parameter| parameter.value == representative)
        .expect("the element representative is the machine's `scale` parameter")
        .value;
    // The owned-argument call stays inside: it copies the persistent
    // member-produced root each traversal and keeps spelling its preserved
    // place.
    let call_operation = component
        .members
        .iter()
        .flat_map(|member| {
            function
                .blocks
                .iter()
                .find(|block| block.id == *member)
                .into_iter()
                .flat_map(|block| block.nodes.iter())
        })
        .find_map(|node| match &node.operation {
            AbstractOperation::CallStructuralScalar { psi_operation, .. } => Some(*psi_operation),
            _ => None,
        })
        .expect("the member block holds the owned-argument scalar call");
    assert!(
        candidate
            .relocations()
            .iter()
            .all(|relocation| relocation.node().psi_operation() != call_operation),
        "the owned-argument call stays inside spelling the preserved place"
    );
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
        AbstractOperation::EstablishScalarArray {
            result, elements, ..
        } => {
            assert_eq!(
                result.place, array_place,
                "the declared place is byte-exact"
            );
            for element in elements {
                assert_eq!(
                    *element, anchor,
                    "the moved element rebinds to the preheader anchor"
                );
            }
        }
        operation => panic!("relocated node keeps its array operation: {operation:?}"),
    }
    assert_eq!(moved.provenance, relocation.node().provenance());
    assert_eq!(moved.fuel, relocation.node().fuel());
    // The staying call still spells the relocated array's preserved place.
    let staying_call = applied
        .session()
        .unit()
        .functions
        .iter()
        .flat_map(|function| &function.blocks)
        .flat_map(|block| &block.nodes)
        .find(|node| node.provenance.first() == Some(&PsiProvenance::Operation(call_operation)))
        .expect("the owned-argument call survives in the member block");
    let AbstractOperation::CallStructuralScalar {
        structural_arguments,
        ..
    } = &staying_call.operation
    else {
        panic!("the staying node keeps its call operation")
    };
    assert_eq!(
        structural_arguments[0].place, array_place,
        "the staying call copies the relocated array's preserved place"
    );
    assert!(
        propose_loop_invariant_scalar_motion(applied.session(), 1)
            .expect("relocated session is an exact fixed point")
            .is_empty()
    );
}

#[test]
fn carried_element_scalar_array_establishment_stays_inside() {
    let session = lowered_session_entry(
        CARRIED_ELEMENT_SCALAR_ARRAY_SOURCE,
        "carried element scalar-array loop",
        "scan",
    );
    let [component] = session.cycle_components().components() else {
        panic!("one cyclic component")
    };
    let function = session
        .unit()
        .functions
        .iter()
        .find(|function| function.machine == component.id.machine)
        .expect("component machine exists");
    let arrays = member_scalar_array_establishments(function, component);
    let [(_, array)] = arrays.as_slice() else {
        panic!("one member scalar-array establishment")
    };
    let array_operation = operation_of(array);

    let candidates =
        propose_loop_invariant_scalar_motion(&session, 8).expect("exact relocation candidates");
    assert!(
        candidates
            .iter()
            .flat_map(|candidate| candidate.relocations().iter())
            .all(|relocation| relocation.node().psi_operation() != array_operation),
        "an array establishment reading a carried element stays inside"
    );
}

#[test]
fn forged_scalar_array_element_is_rejected_by_the_freeze_fence() {
    let session = lowered_session_entry(
        MEMBER_SCALAR_ARRAY_SOURCE,
        "member scalar-array loop",
        "scan",
    );
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
    let arrays = member_scalar_array_establishments(function, component);
    let [(_, array)] = arrays.as_slice() else {
        panic!("one member scalar-array establishment")
    };
    let (array_operation, member_element) = match &array.operation {
        AbstractOperation::EstablishScalarArray {
            psi_operation,
            elements,
            ..
        } => (*psi_operation, elements[0]),
        operation => panic!("the member node is an array establishment: {operation:?}"),
    };
    let candidate = propose_loop_invariant_scalar_motion(&session, 8)
        .expect("exact candidate")
        .pop()
        .expect("one candidate");
    let relocation = candidate
        .relocations()
        .iter()
        .find(|relocation| relocation.node().psi_operation() == array_operation)
        .expect("the array establishment is a planned relocation");
    let member = relocation.node().location().block;
    let validated = validate_loop_invariant_scalar_motion(&session, &candidate)
        .expect("validated exact candidate");
    let applied =
        apply_loop_invariant_scalar_motion(session, validated).expect("applied exact candidate");
    let (input, mut unit) = applied.into_session().into_parts();
    // Forging the moved elements back to the member parameter skips the
    // seed-derived substitution — the establishment's elements rebind to the
    // preheader anchor, so the replayed operation comparison rejects the
    // drifted spelling.
    let forged = find_operation_mut(&mut unit, array_operation);
    if let AbstractOperation::EstablishScalarArray { elements, .. } = &mut forged.operation {
        for element in elements {
            *element = member_element;
        }
    }
    for value_use in &mut forged.uses {
        value_use.value = member_element;
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

/// Two-state cycle whose `step` member establishes one affine sum — `picked`
/// holds `Step::More { rest: s }` — and dispatches it in the same block, the
/// exact shape the cyclic-eligibility fence admits. `s` is carried
/// unchanged through `check`'s own `s` parameter back onto `step`'s back
/// edge, so the field operand resolves to the machine's `scale` anchor and
/// the establishment relocates; `check` is the only exit source and `step`
/// dominates it, keeping the move inside the non-speculative gate. The
/// relocation is the family's first custody-rewriting one: the persistent
/// preheader result stays live across traversals, so the dispatch's
/// member-internal edges stop discarding it while `check`'s exit edge to
/// `finish` disposes it instead.
const MEMBER_SCALAR_CASE_SOURCE: &str = r#"
    data Root {}
    data Step { case More(rest: u32); case Halt(tag: u32); }

    machine Root::scan(scale: u32, remaining: u32 [0..=5])
    {
        transition { _ -> step(scale, remaining) }
        state step(s: u32, pending: u32 [0..=5]) {
            let picked: Step = Step::More { rest: s };
            transition picked {
                Step::More { rest } -> check(rest, s, pending)
                Step::Halt { tag } -> check(tag, s, pending)
            }
        }
        state check(v: u32, s: u32, pending: u32 [0..=5]) {
            transition pending > 0 {
                true -> step(s, pending - 1)
                _ -> finish(v)
            }
        }
        state finish(r: u32) {}
    }
"#;

/// Same component shape, but the case field reads the loop-carried
/// `pending` countdown: the member parameter never resolves to a preheader
/// representative, so the establishment stays inside even though its member
/// block is guaranteed to execute.
const CARRIED_FIELD_SCALAR_CASE_SOURCE: &str = r#"
    data Root {}
    data Step { case More(rest: u32); case Halt(tag: u32); }

    machine Root::scan(scale: u32, remaining: u32 [0..=5])
    {
        transition { _ -> step(scale, remaining) }
        state step(s: u32, pending: u32 [0..=5]) {
            let picked: Step = Step::More { rest: pending };
            transition picked {
                Step::More { rest } -> check(rest, s, pending)
                Step::Halt { tag } -> check(tag, s, pending)
            }
        }
        state check(v: u32, s: u32, pending: u32 [0..=5]) {
            transition pending > 0 {
                true -> step(s, pending - 1)
                _ -> finish(v)
            }
        }
        state finish(r: u32) {}
    }
"#;

/// Every `EstablishScalarCase` node inside `component`'s member blocks —
/// the sum counterpart of [`member_scalar_array_establishments`].
fn member_scalar_case_establishments<'function>(
    function: &'function optimization_unit::PsiOptimizationFunction,
    component: &optimization_unit::OptimizerCycleComponent,
) -> Vec<(
    &'function optimization_unit::OptimizationBlock,
    &'function optimization_unit::OptimizationNode,
)> {
    let mut cases = Vec::new();
    for member in &component.members {
        let block = function
            .blocks
            .iter()
            .find(|block| block.id == *member)
            .expect("member block exists");
        for node in &block.nodes {
            if let AbstractOperation::EstablishScalarCase { .. } = &node.operation {
                cases.push((block, node));
            }
        }
    }
    cases
}

#[test]
fn invariant_scalar_case_establishment_relocates_re_expressing_dispatch_custody() {
    let session = lowered_session_entry(
        MEMBER_SCALAR_CASE_SOURCE,
        "member scalar-case loop",
        "Root::scan",
    );
    let [component] = session.cycle_components().components() else {
        panic!("one cyclic component")
    };
    let [entry] = component.entries.as_slice() else {
        panic!("one entry edge")
    };
    let member_targets: std::collections::BTreeSet<_> = component.members.iter().copied().collect();
    let function = session
        .unit()
        .functions
        .iter()
        .find(|function| function.machine == component.id.machine)
        .expect("component machine exists");
    let cases = member_scalar_case_establishments(function, component);
    let [(_, establishment)] = cases.as_slice() else {
        panic!("one member scalar-case establishment")
    };
    let (case_operation, picked, fields) = match &establishment.operation {
        AbstractOperation::EstablishScalarCase {
            psi_operation,
            result,
            fields,
            ..
        } => (*psi_operation, result.place, fields),
        operation => panic!("the member node is a scalar-case establishment: {operation:?}"),
    };
    assert_eq!(fields.len(), 1, "the case declares one scalar field");
    // The seed's dispatch discards the fresh affine place on every case
    // edge — the custody the relocation re-expresses.
    let dispatch = component
        .members
        .iter()
        .flat_map(|member| {
            function
                .blocks
                .iter()
                .find(|block| block.id == *member)
                .into_iter()
                .flat_map(|block| block.nodes.iter())
        })
        .find(|node| {
            matches!(
                &node.operation,
                AbstractOperation::StructuralCase { source, .. } if *source == picked
            )
        })
        .expect("the member block holds the dispatch on the fresh sum");
    let AbstractOperation::StructuralCase { cases, .. } = &dispatch.operation else {
        panic!("the dispatch is a structural case")
    };
    for case in cases {
        assert!(
            case.trivial_affine_discards.contains(&picked),
            "the seed's dispatch edge discards the fresh affine sum"
        );
    }

    let candidates =
        propose_loop_invariant_scalar_motion(&session, 8).expect("exact relocation candidates");
    let [candidate] = candidates.as_slice() else {
        panic!("one component yields one atomic candidate")
    };
    let relocation = candidate
        .relocations()
        .iter()
        .find(|relocation| relocation.node().psi_operation() == case_operation)
        .expect("the scalar-case establishment is a planned relocation");
    let LoopInvariantNodeResult::Structural(result) = relocation.node().result() else {
        panic!("the scalar-case establishment relocates its structural result")
    };
    assert_eq!(result.place, picked, "the declared place is byte-exact");
    let rewrites = relocation.node().operand_rewrites();
    assert_eq!(
        rewrites.len(),
        1,
        "the single field carries one member-parameter rewrite"
    );
    assert_eq!(
        rewrites[0].0, fields[0].value,
        "the rewrite spells the field value"
    );
    let anchor = function
        .parameters
        .iter()
        .find(|parameter| parameter.value == rewrites[0].1)
        .expect("the field representative is the machine's `scale` parameter")
        .value;
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
        AbstractOperation::EstablishScalarCase { result, fields, .. } => {
            assert_eq!(result.place, picked, "the declared place is byte-exact");
            for field in fields {
                assert_eq!(
                    field.value, anchor,
                    "the moved field rebinds to the preheader anchor"
                );
            }
        }
        operation => panic!("relocated node keeps its case operation: {operation:?}"),
    }
    assert_eq!(moved.provenance, relocation.node().provenance());
    assert_eq!(moved.fuel, relocation.node().fuel());
    // The retained dispatch keeps the persistent result live on
    // member-internal edges and disposes it on the component's exit: the
    // affine place survives every traversal's case inspection and is
    // discarded exactly once on the way out.
    let staying_dispatch = applied
        .session()
        .unit()
        .functions
        .iter()
        .flat_map(|function| &function.blocks)
        .flat_map(|block| &block.nodes)
        .find(|node| {
            matches!(
                &node.operation,
                AbstractOperation::StructuralCase { source, .. } if *source == picked
            )
        })
        .expect("the dispatch survives in the member block");
    for edge in &staying_dispatch.successors {
        assert!(
            !edge.trivial_affine_discards.contains(&picked),
            "a member-bound dispatch edge keeps the persistent result live"
        );
    }
    let exit_discards: Vec<_> = applied
        .session()
        .unit()
        .functions
        .iter()
        .flat_map(|function| &function.blocks)
        .filter(|block| member_targets.contains(&block.id))
        .flat_map(|block| &block.nodes)
        .flat_map(|node| &node.successors)
        .filter(|edge| !member_targets.contains(&edge.target))
        .map(|edge| edge.trivial_affine_discards.contains(&picked))
        .collect();
    assert!(
        !exit_discards.is_empty() && exit_discards.iter().all(|discard| *discard),
        "every member exit edge disposes the persistent result exactly once"
    );
    assert!(
        propose_loop_invariant_scalar_motion(applied.session(), 1)
            .expect("relocated session is an exact fixed point")
            .is_empty()
    );
}

#[test]
fn carried_field_scalar_case_establishment_stays_inside() {
    let session = lowered_session_entry(
        CARRIED_FIELD_SCALAR_CASE_SOURCE,
        "carried field scalar-case loop",
        "Root::scan",
    );
    let [component] = session.cycle_components().components() else {
        panic!("one cyclic component")
    };
    let function = session
        .unit()
        .functions
        .iter()
        .find(|function| function.machine == component.id.machine)
        .expect("component machine exists");
    let cases = member_scalar_case_establishments(function, component);
    let [(_, establishment)] = cases.as_slice() else {
        panic!("one member scalar-case establishment")
    };
    let case_operation = operation_of(establishment);

    let candidates =
        propose_loop_invariant_scalar_motion(&session, 8).expect("exact relocation candidates");
    assert!(
        candidates
            .iter()
            .flat_map(|candidate| candidate.relocations().iter())
            .all(|relocation| relocation.node().psi_operation() != case_operation),
        "a scalar-case establishment reading a carried field stays inside"
    );
}

#[test]
fn forged_scalar_case_field_is_rejected_by_the_freeze_fence() {
    let session = lowered_session_entry(
        MEMBER_SCALAR_CASE_SOURCE,
        "member scalar-case loop",
        "Root::scan",
    );
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
    let cases = member_scalar_case_establishments(function, component);
    let [(_, establishment)] = cases.as_slice() else {
        panic!("one member scalar-case establishment")
    };
    let (case_operation, member_field) = match &establishment.operation {
        AbstractOperation::EstablishScalarCase {
            psi_operation,
            fields,
            ..
        } => (*psi_operation, fields[0].value),
        operation => panic!("the member node is a scalar-case establishment: {operation:?}"),
    };
    let candidate = propose_loop_invariant_scalar_motion(&session, 8)
        .expect("exact candidate")
        .pop()
        .expect("one candidate");
    let relocation = candidate
        .relocations()
        .iter()
        .find(|relocation| relocation.node().psi_operation() == case_operation)
        .expect("the scalar-case establishment is a planned relocation");
    let member = relocation.node().location().block;
    let validated = validate_loop_invariant_scalar_motion(&session, &candidate)
        .expect("validated exact candidate");
    let applied =
        apply_loop_invariant_scalar_motion(session, validated).expect("applied exact candidate");
    let (input, mut unit) = applied.into_session().into_parts();
    // Forging the moved field back to the member parameter skips the
    // seed-derived substitution — the establishment's field rebinds to the
    // preheader anchor, so the replayed operation comparison rejects the
    // drifted spelling.
    let forged = find_operation_mut(&mut unit, case_operation);
    if let AbstractOperation::EstablishScalarCase { fields, .. } = &mut forged.operation {
        for field in fields {
            field.value = member_field;
        }
    }
    for value_use in &mut forged.uses {
        value_use.value = member_field;
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
fn kept_internal_scalar_case_discard_is_rejected_by_the_freeze_fence() {
    let session = lowered_session_entry(
        MEMBER_SCALAR_CASE_SOURCE,
        "member scalar-case loop",
        "Root::scan",
    );
    let [component] = session.cycle_components().components() else {
        panic!("one cyclic component")
    };
    let machine = component.id.machine;
    let member_targets: std::collections::BTreeSet<_> = component.members.iter().copied().collect();
    let function = session
        .unit()
        .functions
        .iter()
        .find(|function| function.machine == machine)
        .expect("component machine exists");
    let cases = member_scalar_case_establishments(function, component);
    let [(_, establishment)] = cases.as_slice() else {
        panic!("one member scalar-case establishment")
    };
    let picked = match &establishment.operation {
        AbstractOperation::EstablishScalarCase { result, .. } => result.place,
        operation => panic!("the member node is a scalar-case establishment: {operation:?}"),
    };
    let dispatch_block = component
        .members
        .iter()
        .flat_map(|member| {
            function
                .blocks
                .iter()
                .find(|block| block.id == *member)
                .into_iter()
        })
        .find(|block| {
            block.nodes.iter().any(|node| {
                matches!(
                    &node.operation,
                    AbstractOperation::StructuralCase { source, .. } if *source == picked
                )
            })
        })
        .expect("the dispatch's member block")
        .id;
    let candidate = propose_loop_invariant_scalar_motion(&session, 8)
        .expect("exact candidate")
        .pop()
        .expect("one candidate");
    let validated = validate_loop_invariant_scalar_motion(&session, &candidate)
        .expect("validated exact candidate");
    let applied =
        apply_loop_invariant_scalar_motion(session, validated).expect("applied exact candidate");
    let (input, mut unit) = applied.into_session().into_parts();
    // Forging the dispatch's member-internal edges to keep discarding the
    // persistent result restores the source's per-traversal custody: the
    // first traversal would end the one preheader place the next traversal
    // dispatches. The freeze replay normalizes the seed's retained node
    // through the same custody rewrite — internal edges stripped, exits
    // disposing — so the kept discard rejects byte-exact.
    let forged = unit
        .functions
        .iter_mut()
        .flat_map(|function| &mut function.blocks)
        .flat_map(|block| &mut block.nodes)
        .find(|node| {
            matches!(
                &node.operation,
                AbstractOperation::StructuralCase { source, .. } if *source == picked
            )
        })
        .expect("the dispatch survives in the member block");
    if let AbstractOperation::StructuralCase { cases, .. } = &mut forged.operation {
        for case in cases {
            if member_targets.contains(&case.target)
                && !case.trivial_affine_discards.contains(&picked)
            {
                case.trivial_affine_discards.push(picked);
            }
        }
    }
    for edge in &mut forged.successors {
        if member_targets.contains(&edge.target) && !edge.trivial_affine_discards.contains(&picked)
        {
            edge.trivial_affine_discards.push(picked);
        }
    }
    unit.identity = recompute_psi_optimization_unit_identity(&unit);
    assert!(matches!(
        crate::validation::validate_transformed_psi_optimization_unit(&input, &unit),
        Err(
            OptimizationUnitValidationError::RankedCycleFrozenBlockMismatch {
                machine: rejected_machine,
                block
            }
        ) if rejected_machine == machine && block == dispatch_block
    ));
}

#[test]
fn dropped_exit_scalar_case_disposal_is_rejected_by_the_freeze_fence() {
    let session = lowered_session_entry(
        MEMBER_SCALAR_CASE_SOURCE,
        "member scalar-case loop",
        "Root::scan",
    );
    let [component] = session.cycle_components().components() else {
        panic!("one cyclic component")
    };
    let machine = component.id.machine;
    let member_targets: std::collections::BTreeSet<_> = component.members.iter().copied().collect();
    let function = session
        .unit()
        .functions
        .iter()
        .find(|function| function.machine == machine)
        .expect("component machine exists");
    let cases = member_scalar_case_establishments(function, component);
    let [(_, establishment)] = cases.as_slice() else {
        panic!("one member scalar-case establishment")
    };
    let picked = match &establishment.operation {
        AbstractOperation::EstablishScalarCase { result, .. } => result.place,
        operation => panic!("the member node is a scalar-case establishment: {operation:?}"),
    };
    // The exit edge is a member terminator edge departing the roster —
    // `check`'s `finish` arm — which must dispose the persistent result.
    let exit = component
        .members
        .iter()
        .flat_map(|member| {
            function
                .blocks
                .iter()
                .find(|block| block.id == *member)
                .into_iter()
                .flat_map(|block| block.nodes.iter())
        })
        .flat_map(|node| node.successors.iter())
        .find(|edge| !member_targets.contains(&edge.target))
        .expect("the component has an exit edge");
    let exit_edge = exit.psi_edge;
    let candidate = propose_loop_invariant_scalar_motion(&session, 8)
        .expect("exact candidate")
        .pop()
        .expect("one candidate");
    let validated = validate_loop_invariant_scalar_motion(&session, &candidate)
        .expect("validated exact candidate");
    let applied =
        apply_loop_invariant_scalar_motion(session, validated).expect("applied exact candidate");
    let (input, mut unit) = applied.into_session().into_parts();
    // The applied transform disposes `picked` on the exit edge. Forging the
    // edge back to the seed's empty roster leaves the persistent place live
    // outside the component — the freeze replay's normalized custody expects
    // the disposal, so the drop rejects byte-exact.
    let mut forged = false;
    for function in &mut unit.functions {
        for block in &mut function.blocks {
            if !member_targets.contains(&block.id) {
                continue;
            }
            for node in &mut block.nodes {
                for edge in &mut node.successors {
                    if edge.psi_edge == exit_edge
                        && let Some(index) = edge
                            .trivial_affine_discards
                            .iter()
                            .position(|place| *place == picked)
                    {
                        edge.trivial_affine_discards.remove(index);
                        forged = true;
                    }
                }
                match &mut node.operation {
                    AbstractOperation::Jump {
                        trivial_affine_discards,
                        ..
                    } if node
                        .successors
                        .first()
                        .is_some_and(|edge| edge.psi_edge == exit_edge) =>
                    {
                        trivial_affine_discards.retain(|place| *place != picked);
                    }
                    AbstractOperation::Conditional {
                        when_true,
                        when_false,
                        ..
                    } => {
                        for successor in [when_true, when_false] {
                            if successor.psi_edge == exit_edge {
                                forged |= !successor.trivial_affine_discards.is_empty();
                                successor
                                    .trivial_affine_discards
                                    .retain(|place| *place != picked);
                            }
                        }
                    }
                    AbstractOperation::StructuralCase { cases, .. } => {
                        for case in cases {
                            if case.psi_edge == exit_edge {
                                forged |= !case.trivial_affine_discards.is_empty();
                                case.trivial_affine_discards
                                    .retain(|place| *place != picked);
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    assert!(
        forged,
        "the exit edge carried the relocated result's disposal"
    );
    unit.identity = recompute_psi_optimization_unit_identity(&unit);
    assert!(matches!(
        crate::validation::validate_transformed_psi_optimization_unit(&input, &unit),
        Err(
            OptimizationUnitValidationError::RankedCycleFrozenBlockMismatch {
                machine: rejected_machine,
                ..
            }
        ) if rejected_machine == machine
    ));
}

#[test]
fn stale_scalar_case_frontier_catalog_is_rejected() {
    let session = lowered_session_entry(
        MEMBER_SCALAR_CASE_SOURCE,
        "member scalar-case loop",
        "Root::scan",
    );
    let [component] = session.cycle_components().components() else {
        panic!("one cyclic component")
    };
    let machine = component.id.machine;
    // The seed catalog still carries the source's per-traversal custody:
    // `picked` owned between its member establishment and the dispatch-edge
    // discards, dead everywhere else.
    let seed_frontier_facts = session.unit().ownership_frontier_facts.clone();
    let candidate = propose_loop_invariant_scalar_motion(&session, 8)
        .expect("exact candidate")
        .pop()
        .expect("one candidate");
    let validated = validate_loop_invariant_scalar_motion(&session, &candidate)
        .expect("validated exact candidate");
    let applied =
        apply_loop_invariant_scalar_motion(session, validated).expect("applied exact candidate");
    let (input, mut unit) = applied.into_session().into_parts();
    // Forging the frontier catalog back to the seed's spelling leaves the
    // affine-authority replay reading custody the transformed edges no
    // longer execute — internal edges keep the persistent place live where
    // the stale catalog still ends it at dispatch — so the stale
    // membership rejects before the catalog comparison is even reached.
    unit.ownership_frontier_facts = seed_frontier_facts;
    unit.identity = recompute_psi_optimization_unit_identity(&unit);
    assert!(matches!(
        crate::validation::validate_transformed_psi_optimization_unit(&input, &unit),
        Err(
            OptimizationUnitValidationError::StructuralEdgeAffineDiscardsMismatch {
                machine: rejected_machine,
                ..
            }
        ) if rejected_machine == machine
    ));
}

/// Two-state cycle whose member invokes a pure structural-result callee on an
/// invariant argument: `pick` returns the fresh affine sum the member block
/// immediately dispatches, so the cyclic eligibility fence already confined
/// the result to same-block dispatch custody — exactly the shape the
/// relocation re-expresses. `s` resolves transitively to `scale`'s preheader
/// anchor, so the call relocates rebinding its scalar argument while the
/// retained dispatch stops discarding the persistent result on
/// member-internal edges and every exit edge disposes it instead.
const MEMBER_STRUCTURAL_CALL_SOURCE: &str = r#"
    data Root {}
    data Step { case More(rest: u32); case Halt(tag: u32); }

    machine pick(seed: u32) -> Step { Step::More { rest: seed } }

    machine Root::scan(scale: u32, remaining: u32 [0..=5])
    {
        transition { _ -> step(scale, remaining) }
        state step(s: u32, pending: u32 [0..=5]) {
            let picked: Step = pick(s);
            transition picked {
                Step::More { rest } -> check(rest, s, pending)
                Step::Halt { tag } -> check(tag, s, pending)
            }
        }
        state check(v: u32, s: u32, pending: u32 [0..=5]) {
            transition pending > 0 {
                true -> step(s, pending - 1)
                _ -> finish(v)
            }
        }
        state finish(r: u32) {}
    }
"#;

/// Same component shape, but the call's scalar argument reads the
/// loop-carried `pending` countdown: the member parameter never resolves to
/// a preheader representative, so the call stays inside even though its
/// callee is pure and its member block is guaranteed to execute.
const CARRIED_ARGUMENT_STRUCTURAL_RESULT_CALL_SOURCE: &str = r#"
    data Root {}
    data Step { case More(rest: u32); case Halt(tag: u32); }

    machine pick(seed: u32) -> Step { Step::More { rest: seed } }

    machine Root::scan(scale: u32, remaining: u32 [0..=5])
    {
        transition { _ -> step(scale, remaining) }
        state step(s: u32, pending: u32 [0..=5]) {
            let picked: Step = pick(pending);
            transition picked {
                Step::More { rest } -> check(rest, s, pending)
                Step::Halt { tag } -> check(tag, s, pending)
            }
        }
        state check(v: u32, s: u32, pending: u32 [0..=5]) {
            transition pending > 0 {
                true -> step(s, pending - 1)
                _ -> finish(v)
            }
        }
        state finish(r: u32) {}
    }
"#;

/// Every `CallStructural` node inside `component`'s member blocks — the
/// call counterpart of [`member_scalar_case_establishments`].
fn member_structural_calls<'function>(
    function: &'function optimization_unit::PsiOptimizationFunction,
    component: &optimization_unit::OptimizerCycleComponent,
) -> Vec<(
    &'function optimization_unit::OptimizationBlock,
    &'function optimization_unit::OptimizationNode,
)> {
    let mut calls = Vec::new();
    for member in &component.members {
        let block = function
            .blocks
            .iter()
            .find(|block| block.id == *member)
            .expect("member block exists");
        for node in &block.nodes {
            if let AbstractOperation::CallStructural { .. } = &node.operation {
                calls.push((block, node));
            }
        }
    }
    calls
}

#[test]
fn invariant_structural_call_relocates_re_expressing_dispatch_custody() {
    let session = lowered_session_entry(
        MEMBER_STRUCTURAL_CALL_SOURCE,
        "member structural-call loop",
        "Root::scan",
    );
    let [component] = session.cycle_components().components() else {
        panic!("one cyclic component")
    };
    let [entry] = component.entries.as_slice() else {
        panic!("one entry edge")
    };
    let member_targets: std::collections::BTreeSet<_> = component.members.iter().copied().collect();
    let function = session
        .unit()
        .functions
        .iter()
        .find(|function| function.machine == component.id.machine)
        .expect("component machine exists");
    let calls = member_structural_calls(function, component);
    let [(_, call)] = calls.as_slice() else {
        panic!("one member structural call")
    };
    let (call_operation, picked, arguments) = match &call.operation {
        AbstractOperation::CallStructural {
            psi_operation,
            result,
            arguments,
            structural_arguments,
            claim_transfers,
            returned_claim_transfers,
            requirement_obligations,
            crash_continuations,
            selected_evidence,
            ..
        } => {
            assert!(
                structural_arguments.is_empty()
                    && claim_transfers.is_empty()
                    && returned_claim_transfers.is_empty()
                    && requirement_obligations.is_empty()
                    && crash_continuations.is_empty()
                    && selected_evidence.is_empty(),
                "the admitted call shape carries no structural surface beyond its result"
            );
            assert_eq!(
                result.multiplicity,
                terminal_psi::StructuralMultiplicity::Affine,
                "the result is the confined affine sum"
            );
            (*psi_operation, result.place, arguments.clone())
        }
        operation => panic!("the member node is a structural call: {operation:?}"),
    };
    assert_eq!(arguments.len(), 1, "the call carries one scalar argument");
    // The seed's dispatch discards the fresh affine place on every case
    // edge — the custody the relocation re-expresses.
    let dispatch = component
        .members
        .iter()
        .flat_map(|member| {
            function
                .blocks
                .iter()
                .find(|block| block.id == *member)
                .into_iter()
                .flat_map(|block| block.nodes.iter())
        })
        .find(|node| {
            matches!(
                &node.operation,
                AbstractOperation::StructuralCase { source, .. } if *source == picked
            )
        })
        .expect("the member block holds the dispatch on the fresh sum");
    let AbstractOperation::StructuralCase { cases, .. } = &dispatch.operation else {
        panic!("the dispatch is a structural case")
    };
    for case in cases {
        assert!(
            case.trivial_affine_discards.contains(&picked),
            "the seed's dispatch edge discards the fresh affine sum"
        );
    }

    let candidates =
        propose_loop_invariant_scalar_motion(&session, 8).expect("exact relocation candidates");
    let [candidate] = candidates.as_slice() else {
        panic!("one component yields one atomic candidate")
    };
    let relocation = candidate
        .relocations()
        .iter()
        .find(|relocation| relocation.node().psi_operation() == call_operation)
        .expect("the structural call is a planned relocation");
    let LoopInvariantNodeResult::Structural(result) = relocation.node().result() else {
        panic!("the structural call relocates its structural result")
    };
    assert_eq!(result.place, picked, "the declared place is byte-exact");
    let rewrites = relocation.node().operand_rewrites();
    assert_eq!(
        rewrites.len(),
        1,
        "the single scalar argument carries one member-parameter rewrite"
    );
    assert_eq!(
        rewrites[0].0, arguments[0],
        "the rewrite spells the call's scalar argument"
    );
    let anchor = function
        .parameters
        .iter()
        .find(|parameter| parameter.value == rewrites[0].1)
        .expect("the argument representative is the machine's `scale` parameter")
        .value;
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
        AbstractOperation::CallStructural {
            result, arguments, ..
        } => {
            assert_eq!(result.place, picked, "the declared place is byte-exact");
            for argument in arguments {
                assert_eq!(
                    *argument, anchor,
                    "the moved call rebinds its scalar argument to the preheader anchor"
                );
            }
        }
        operation => panic!("relocated node keeps its call operation: {operation:?}"),
    }
    assert_eq!(moved.provenance, relocation.node().provenance());
    assert_eq!(moved.fuel, relocation.node().fuel());
    // The retained dispatch keeps the persistent result live on
    // member-internal edges and disposes it on the component's exit: the
    // affine place survives every traversal's case inspection and is
    // discarded exactly once on the way out.
    let staying_dispatch = applied
        .session()
        .unit()
        .functions
        .iter()
        .flat_map(|function| &function.blocks)
        .flat_map(|block| &block.nodes)
        .find(|node| {
            matches!(
                &node.operation,
                AbstractOperation::StructuralCase { source, .. } if *source == picked
            )
        })
        .expect("the dispatch survives in the member block");
    for edge in &staying_dispatch.successors {
        assert!(
            !edge.trivial_affine_discards.contains(&picked),
            "a member-bound dispatch edge keeps the persistent result live"
        );
    }
    let exit_discards: Vec<_> = applied
        .session()
        .unit()
        .functions
        .iter()
        .flat_map(|function| &function.blocks)
        .filter(|block| member_targets.contains(&block.id))
        .flat_map(|block| &block.nodes)
        .flat_map(|node| &node.successors)
        .filter(|edge| !member_targets.contains(&edge.target))
        .map(|edge| edge.trivial_affine_discards.contains(&picked))
        .collect();
    assert!(
        !exit_discards.is_empty() && exit_discards.iter().all(|discard| *discard),
        "every member exit edge disposes the persistent result exactly once"
    );
    assert!(
        propose_loop_invariant_scalar_motion(applied.session(), 1)
            .expect("relocated session is an exact fixed point")
            .is_empty()
    );
}

#[test]
fn carried_argument_structural_call_stays_inside() {
    let session = lowered_session_entry(
        CARRIED_ARGUMENT_STRUCTURAL_RESULT_CALL_SOURCE,
        "carried argument structural-call loop",
        "Root::scan",
    );
    let [component] = session.cycle_components().components() else {
        panic!("one cyclic component")
    };
    let function = session
        .unit()
        .functions
        .iter()
        .find(|function| function.machine == component.id.machine)
        .expect("component machine exists");
    let calls = member_structural_calls(function, component);
    let [(_, call)] = calls.as_slice() else {
        panic!("one member structural call")
    };
    let call_operation = operation_of(call);

    let candidates =
        propose_loop_invariant_scalar_motion(&session, 8).expect("exact relocation candidates");
    assert!(
        candidates
            .iter()
            .flat_map(|candidate| candidate.relocations().iter())
            .all(|relocation| relocation.node().psi_operation() != call_operation),
        "a structural call reading a carried argument stays inside"
    );
}

#[test]
fn forged_structural_call_argument_is_rejected_by_the_freeze_fence() {
    let session = lowered_session_entry(
        MEMBER_STRUCTURAL_CALL_SOURCE,
        "member structural-call loop",
        "Root::scan",
    );
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
    let calls = member_structural_calls(function, component);
    let [(_, call)] = calls.as_slice() else {
        panic!("one member structural call")
    };
    let (call_operation, member_argument) = match &call.operation {
        AbstractOperation::CallStructural {
            psi_operation,
            arguments,
            ..
        } => (*psi_operation, arguments[0]),
        operation => panic!("the member node is a structural call: {operation:?}"),
    };
    let candidate = propose_loop_invariant_scalar_motion(&session, 8)
        .expect("exact candidate")
        .pop()
        .expect("one candidate");
    let relocation = candidate
        .relocations()
        .iter()
        .find(|relocation| relocation.node().psi_operation() == call_operation)
        .expect("the structural call is a planned relocation");
    let member = relocation.node().location().block;
    let validated = validate_loop_invariant_scalar_motion(&session, &candidate)
        .expect("validated exact candidate");
    let applied =
        apply_loop_invariant_scalar_motion(session, validated).expect("applied exact candidate");
    let (input, mut unit) = applied.into_session().into_parts();
    // Forging the moved argument back to the member parameter skips the
    // seed-derived substitution — the call's argument rebinds to the
    // preheader anchor, so the replayed operation comparison rejects the
    // drifted spelling.
    let forged = find_operation_mut(&mut unit, call_operation);
    if let AbstractOperation::CallStructural { arguments, .. } = &mut forged.operation {
        for argument in arguments {
            *argument = member_argument;
        }
    }
    for value_use in &mut forged.uses {
        value_use.value = member_argument;
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
fn kept_internal_structural_call_discard_is_rejected_by_the_freeze_fence() {
    let session = lowered_session_entry(
        MEMBER_STRUCTURAL_CALL_SOURCE,
        "member structural-call loop",
        "Root::scan",
    );
    let [component] = session.cycle_components().components() else {
        panic!("one cyclic component")
    };
    let machine = component.id.machine;
    let member_targets: std::collections::BTreeSet<_> = component.members.iter().copied().collect();
    let function = session
        .unit()
        .functions
        .iter()
        .find(|function| function.machine == machine)
        .expect("component machine exists");
    let calls = member_structural_calls(function, component);
    let [(_, call)] = calls.as_slice() else {
        panic!("one member structural call")
    };
    let picked = match &call.operation {
        AbstractOperation::CallStructural { result, .. } => result.place,
        operation => panic!("the member node is a structural call: {operation:?}"),
    };
    let dispatch_block = component
        .members
        .iter()
        .flat_map(|member| {
            function
                .blocks
                .iter()
                .find(|block| block.id == *member)
                .into_iter()
        })
        .find(|block| {
            block.nodes.iter().any(|node| {
                matches!(
                    &node.operation,
                    AbstractOperation::StructuralCase { source, .. } if *source == picked
                )
            })
        })
        .expect("the dispatch's member block")
        .id;
    let candidate = propose_loop_invariant_scalar_motion(&session, 8)
        .expect("exact candidate")
        .pop()
        .expect("one candidate");
    let validated = validate_loop_invariant_scalar_motion(&session, &candidate)
        .expect("validated exact candidate");
    let applied =
        apply_loop_invariant_scalar_motion(session, validated).expect("applied exact candidate");
    let (input, mut unit) = applied.into_session().into_parts();
    // Forging the dispatch's member-internal edges to keep discarding the
    // persistent result restores the source's per-traversal custody: the
    // first traversal would end the one preheader place the next traversal
    // dispatches. The freeze replay normalizes the seed's retained node
    // through the same custody rewrite — internal edges stripped, exits
    // disposing — so the kept discard rejects byte-exact.
    let forged = unit
        .functions
        .iter_mut()
        .flat_map(|function| &mut function.blocks)
        .flat_map(|block| &mut block.nodes)
        .find(|node| {
            matches!(
                &node.operation,
                AbstractOperation::StructuralCase { source, .. } if *source == picked
            )
        })
        .expect("the dispatch survives in the member block");
    if let AbstractOperation::StructuralCase { cases, .. } = &mut forged.operation {
        for case in cases {
            if member_targets.contains(&case.target)
                && !case.trivial_affine_discards.contains(&picked)
            {
                case.trivial_affine_discards.push(picked);
            }
        }
    }
    for edge in &mut forged.successors {
        if member_targets.contains(&edge.target) && !edge.trivial_affine_discards.contains(&picked)
        {
            edge.trivial_affine_discards.push(picked);
        }
    }
    unit.identity = recompute_psi_optimization_unit_identity(&unit);
    assert!(matches!(
        crate::validation::validate_transformed_psi_optimization_unit(&input, &unit),
        Err(
            OptimizationUnitValidationError::RankedCycleFrozenBlockMismatch {
                machine: rejected_machine,
                block
            }
        ) if rejected_machine == machine && block == dispatch_block
    ));
}

#[test]
fn dropped_exit_structural_call_disposal_is_rejected_by_the_freeze_fence() {
    let session = lowered_session_entry(
        MEMBER_STRUCTURAL_CALL_SOURCE,
        "member structural-call loop",
        "Root::scan",
    );
    let [component] = session.cycle_components().components() else {
        panic!("one cyclic component")
    };
    let machine = component.id.machine;
    let member_targets: std::collections::BTreeSet<_> = component.members.iter().copied().collect();
    let function = session
        .unit()
        .functions
        .iter()
        .find(|function| function.machine == machine)
        .expect("component machine exists");
    let calls = member_structural_calls(function, component);
    let [(_, call)] = calls.as_slice() else {
        panic!("one member structural call")
    };
    let picked = match &call.operation {
        AbstractOperation::CallStructural { result, .. } => result.place,
        operation => panic!("the member node is a structural call: {operation:?}"),
    };
    // The exit edge is a member terminator edge departing the roster —
    // `check`'s `finish` arm — which must dispose the persistent result.
    let exit = component
        .members
        .iter()
        .flat_map(|member| {
            function
                .blocks
                .iter()
                .find(|block| block.id == *member)
                .into_iter()
                .flat_map(|block| block.nodes.iter())
        })
        .flat_map(|node| node.successors.iter())
        .find(|edge| !member_targets.contains(&edge.target))
        .expect("the component has an exit edge");
    let exit_edge = exit.psi_edge;
    let candidate = propose_loop_invariant_scalar_motion(&session, 8)
        .expect("exact candidate")
        .pop()
        .expect("one candidate");
    let validated = validate_loop_invariant_scalar_motion(&session, &candidate)
        .expect("validated exact candidate");
    let applied =
        apply_loop_invariant_scalar_motion(session, validated).expect("applied exact candidate");
    let (input, mut unit) = applied.into_session().into_parts();
    // The applied transform disposes `picked` on the exit edge. Forging the
    // edge back to the seed's empty roster leaves the persistent place live
    // outside the component — the freeze replay's normalized custody expects
    // the disposal, so the drop rejects byte-exact.
    let mut forged = false;
    for function in &mut unit.functions {
        for block in &mut function.blocks {
            if !member_targets.contains(&block.id) {
                continue;
            }
            for node in &mut block.nodes {
                for edge in &mut node.successors {
                    if edge.psi_edge == exit_edge
                        && let Some(index) = edge
                            .trivial_affine_discards
                            .iter()
                            .position(|place| *place == picked)
                    {
                        edge.trivial_affine_discards.remove(index);
                        forged = true;
                    }
                }
                match &mut node.operation {
                    AbstractOperation::Jump {
                        trivial_affine_discards,
                        ..
                    } if node
                        .successors
                        .first()
                        .is_some_and(|edge| edge.psi_edge == exit_edge) =>
                    {
                        trivial_affine_discards.retain(|place| *place != picked);
                    }
                    AbstractOperation::Conditional {
                        when_true,
                        when_false,
                        ..
                    } => {
                        for successor in [when_true, when_false] {
                            if successor.psi_edge == exit_edge {
                                forged |= !successor.trivial_affine_discards.is_empty();
                                successor
                                    .trivial_affine_discards
                                    .retain(|place| *place != picked);
                            }
                        }
                    }
                    AbstractOperation::StructuralCase { cases, .. } => {
                        for case in cases {
                            if case.psi_edge == exit_edge {
                                forged |= !case.trivial_affine_discards.is_empty();
                                case.trivial_affine_discards
                                    .retain(|place| *place != picked);
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    assert!(
        forged,
        "the exit edge carried the relocated result's disposal"
    );
    unit.identity = recompute_psi_optimization_unit_identity(&unit);
    assert!(matches!(
        crate::validation::validate_transformed_psi_optimization_unit(&input, &unit),
        Err(
            OptimizationUnitValidationError::RankedCycleFrozenBlockMismatch {
                machine: rejected_machine,
                ..
            }
        ) if rejected_machine == machine
    ));
}

#[test]
fn stale_structural_call_frontier_catalog_is_rejected() {
    let session = lowered_session_entry(
        MEMBER_STRUCTURAL_CALL_SOURCE,
        "member structural-call loop",
        "Root::scan",
    );
    let [component] = session.cycle_components().components() else {
        panic!("one cyclic component")
    };
    let machine = component.id.machine;
    // The seed catalog still carries the source's per-traversal custody:
    // `picked` owned between its member call and the dispatch-edge
    // discards, dead everywhere else.
    let seed_frontier_facts = session.unit().ownership_frontier_facts.clone();
    let candidate = propose_loop_invariant_scalar_motion(&session, 8)
        .expect("exact candidate")
        .pop()
        .expect("one candidate");
    let validated = validate_loop_invariant_scalar_motion(&session, &candidate)
        .expect("validated exact candidate");
    let applied =
        apply_loop_invariant_scalar_motion(session, validated).expect("applied exact candidate");
    let (input, mut unit) = applied.into_session().into_parts();
    // Forging the frontier catalog back to the seed's spelling leaves the
    // affine-authority replay reading custody the transformed edges no
    // longer execute — internal edges keep the persistent place live where
    // the stale catalog still ends it at dispatch — so the stale
    // membership rejects before the catalog comparison is even reached.
    unit.ownership_frontier_facts = seed_frontier_facts;
    unit.identity = recompute_psi_optimization_unit_identity(&unit);
    assert!(matches!(
        crate::validation::validate_transformed_psi_optimization_unit(&input, &unit),
        Err(
            OptimizationUnitValidationError::StructuralEdgeAffineDiscardsMismatch {
                machine: rejected_machine,
                ..
            }
        ) if rejected_machine == machine
    ));
}

/// Two-state cycle whose `step` member establishes `marker`, the
/// field-free affine record the composed-control lowering emits for a
/// trivial affine local: the cyclic eligibility fence already confined the
/// fresh place to `step`'s member block, discarding it on every departing
/// edge, and `step` dominates both exit sources — so the establishment
/// relocates into the unique preheader while the retained member edges keep
/// the one persistent place live inside the roster and every exit edge
/// disposes it instead.
const MEMBER_AFFINE_RECORD_SOURCE: &str = r#"
    data Root {}
    data Marker {}

    machine Root::scan(scale: u32, remaining: u32 [0..=5])
    {
        transition { _ -> step(scale, remaining) }
        state step(s: u32, pending: u32 [0..=5]) {
            let marker: Marker = Marker {};
            transition pending > 0 {
                true -> check(s, pending - 1)
                _ -> finish(s)
            }
        }
        state check(v: u32, pending: u32 [0..=5]) {
            transition pending > 0 {
                true -> step(v, pending)
                _ -> finish(v)
            }
        }
        state finish(r: u32) {}
    }
"#;

/// The component's unique entry edge is one of two successors on the
/// preheader's terminator: reaching the preheader does not guarantee
/// entering the loop, so relocating the `marker` establishment would execute
/// it on traversals that never enter the component. The establishment stays
/// inside; work-free scalar-constant leaves still relocate.
const CONDITIONAL_ENTRY_AFFINE_RECORD_SOURCE: &str = r#"
    data Root {}
    data Marker {}

    machine Root::enter(scale: u32, go: bool, remaining: u32 [0..=5])
    {
        transition go {
            true -> scan(scale, remaining)
            _ -> done()
        }
        state scan(s: u32, pending: u32 [0..=5]) {
            let marker: Marker = Marker {};
            transition pending > 0 {
                true -> scan(s, pending - 1)
                _ -> finish(s)
            }
        }
        state done() {}
        state finish(r: u32) {}
    }
"#;

/// Same two-state cycle, but the empty affine record is established inside
/// `check`: a traversal can leave through `step`'s own `finish` arm without
/// ever reaching `check`, so hoisting the establishment would speculate the
/// work and the member stays inside.
const BYPASSED_AFFINE_RECORD_SOURCE: &str = r#"
    data Root {}
    data Marker {}

    machine Root::scan(scale: u32, remaining: u32 [0..=5])
    {
        transition { _ -> step(scale, remaining) }
        state step(s: u32, pending: u32 [0..=5]) {
            transition pending > 0 {
                true -> check(s, pending - 1)
                _ -> finish(s)
            }
        }
        state check(v: u32, pending: u32 [0..=5]) {
            let marker: Marker = Marker {};
            transition pending > 0 {
                true -> step(v, pending)
                _ -> finish(v)
            }
        }
        state finish(r: u32) {}
    }
"#;

/// Every `EstablishRecord` node inside `component`'s member blocks whose
/// result is affine — the empty-declaration custody-rewriting counterpart of
/// [`member_record_establishments`].
fn member_affine_record_establishments<'function>(
    function: &'function optimization_unit::PsiOptimizationFunction,
    component: &optimization_unit::OptimizerCycleComponent,
) -> Vec<(
    &'function optimization_unit::OptimizationBlock,
    &'function optimization_unit::OptimizationNode,
)> {
    member_record_establishments(function, component)
        .into_iter()
        .filter(|(_, node)| {
            matches!(
                &node.operation,
                AbstractOperation::EstablishRecord { result, .. }
                    if result.multiplicity == terminal_psi::StructuralMultiplicity::Affine
            )
        })
        .collect()
}

#[test]
fn affine_empty_record_establishment_relocates_re_expressing_disposal_custody() {
    let session = lowered_session_entry(
        MEMBER_AFFINE_RECORD_SOURCE,
        "member affine empty-record loop",
        "Root::scan",
    );
    let [component] = session.cycle_components().components() else {
        panic!("one cyclic component")
    };
    let [entry] = component.entries.as_slice() else {
        panic!("one entry edge")
    };
    let member_targets: std::collections::BTreeSet<_> = component.members.iter().copied().collect();
    let function = session
        .unit()
        .functions
        .iter()
        .find(|function| function.machine == component.id.machine)
        .expect("component machine exists");
    let records = member_affine_record_establishments(function, component);
    let [(_, establishment)] = records.as_slice() else {
        panic!("one member affine empty-record establishment")
    };
    let (record_operation, picked) = match &establishment.operation {
        AbstractOperation::EstablishRecord {
            psi_operation,
            result,
            fields,
            ..
        } => {
            assert!(
                fields.is_empty(),
                "the composed-control affine record declares no fields"
            );
            assert_eq!(
                result.multiplicity,
                terminal_psi::StructuralMultiplicity::Affine,
                "the result is the confined affine place"
            );
            assert!(
                result.qualifications.is_empty()
                    && result.projected_qualifications.is_empty()
                    && result.claims.is_empty(),
                "the affine result is claim-free"
            );
            (*psi_operation, result.place)
        }
        operation => panic!("the member node is a record establishment: {operation:?}"),
    };
    // The seed disposes the fresh affine place through member-roster discard
    // rosters — the custody the relocation re-expresses — while no edge
    // departing a non-member block ever spells it.
    let (member_roster_discards, foreign_spellings): (usize, usize) = function
        .blocks
        .iter()
        .flat_map(|block| {
            let member = member_targets.contains(&block.id);
            block
                .nodes
                .iter()
                .flat_map(move |node| node.successors.iter().map(move |edge| (member, edge)))
        })
        .fold((0, 0), |(member_discards, foreign), (member, edge)| {
            let spells = edge.trivial_affine_discards.contains(&picked)
                || edge
                    .residual_affine_discards
                    .iter()
                    .any(|discard| discard.place == picked)
                || edge
                    .structural_bindings
                    .iter()
                    .any(|binding| binding.parameter == picked || binding.argument.place == picked);
            match (member, spells) {
                (true, true) => (member_discards + 1, foreign),
                (false, true) => (member_discards, foreign + 1),
                _ => (member_discards, foreign),
            }
        });
    assert!(
        member_roster_discards > 0,
        "the seed's member edges discard the fresh affine record"
    );
    assert_eq!(
        foreign_spellings, 0,
        "the affine place stays inside the member roster"
    );

    let candidates =
        propose_loop_invariant_scalar_motion(&session, 8).expect("exact relocation candidates");
    let [candidate] = candidates.as_slice() else {
        panic!("one component yields one atomic candidate")
    };
    let relocation = candidate
        .relocations()
        .iter()
        .find(|relocation| relocation.node().psi_operation() == record_operation)
        .expect("the affine empty-record establishment is a planned relocation");
    let LoopInvariantNodeResult::Structural(result) = relocation.node().result() else {
        panic!("the affine empty-record establishment relocates its structural result")
    };
    assert_eq!(result.place, picked, "the declared place is byte-exact");
    assert_eq!(
        result.multiplicity,
        terminal_psi::StructuralMultiplicity::Affine,
        "the declared multiplicity is byte-exact"
    );
    assert!(
        relocation.node().operand_rewrites().is_empty()
            && relocation.node().argument_rewrites().is_empty(),
        "the empty declaration carries no operand or root rewrites"
    );
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
        AbstractOperation::EstablishRecord { result, fields, .. } => {
            assert_eq!(result.place, picked, "the declared place is byte-exact");
            assert!(fields.is_empty(), "the moved declaration stays empty");
        }
        operation => panic!("relocated node keeps its record operation: {operation:?}"),
    }
    assert_eq!(moved.provenance, relocation.node().provenance());
    assert_eq!(moved.fuel, relocation.node().fuel());
    // The retained member edges keep the persistent result live across
    // member-internal hops and dispose it on every component exit — the
    // affine place survives every traversal and is discarded exactly once on
    // the way out.
    let internal_discards: Vec<_> = applied
        .session()
        .unit()
        .functions
        .iter()
        .flat_map(|function| &function.blocks)
        .filter(|block| member_targets.contains(&block.id))
        .flat_map(|block| &block.nodes)
        .flat_map(|node| &node.successors)
        .filter(|edge| member_targets.contains(&edge.target))
        .map(|edge| edge.trivial_affine_discards.contains(&picked))
        .collect();
    assert!(
        !internal_discards.is_empty() && internal_discards.iter().all(|discard| !*discard),
        "every member-internal edge keeps the persistent result live"
    );
    let exit_discards: Vec<_> = applied
        .session()
        .unit()
        .functions
        .iter()
        .flat_map(|function| &function.blocks)
        .filter(|block| member_targets.contains(&block.id))
        .flat_map(|block| &block.nodes)
        .flat_map(|node| &node.successors)
        .filter(|edge| !member_targets.contains(&edge.target))
        .map(|edge| edge.trivial_affine_discards.contains(&picked))
        .collect();
    assert!(
        !exit_discards.is_empty() && exit_discards.iter().all(|discard| *discard),
        "every member exit edge disposes the persistent result exactly once"
    );
    assert!(
        propose_loop_invariant_scalar_motion(applied.session(), 1)
            .expect("relocated session is an exact fixed point")
            .is_empty()
    );
}

#[test]
fn conditional_entry_affine_record_establishment_stays_inside() {
    let session = lowered_session_entry(
        CONDITIONAL_ENTRY_AFFINE_RECORD_SOURCE,
        "conditional-entry affine record loop",
        "Root::enter",
    );
    let [component] = session.cycle_components().components() else {
        panic!("one cyclic component")
    };
    let function = session
        .unit()
        .functions
        .iter()
        .find(|function| function.machine == component.id.machine)
        .expect("component machine exists");
    let records = member_affine_record_establishments(function, component);
    let [(_, establishment)] = records.as_slice() else {
        panic!("one member affine empty-record establishment")
    };
    let record_operation = operation_of(establishment);

    let candidates =
        propose_loop_invariant_scalar_motion(&session, 8).expect("exact relocation candidates");
    assert!(
        candidates
            .iter()
            .flat_map(|candidate| candidate.relocations().iter())
            .all(|relocation| relocation.node().psi_operation() != record_operation),
        "an affine record behind a conditional entry stays inside"
    );
}

#[test]
fn bypassed_member_affine_record_establishment_stays_inside() {
    let session = lowered_session_entry(
        BYPASSED_AFFINE_RECORD_SOURCE,
        "bypassed affine record loop",
        "Root::scan",
    );
    let [component] = session.cycle_components().components() else {
        panic!("one cyclic component")
    };
    let function = session
        .unit()
        .functions
        .iter()
        .find(|function| function.machine == component.id.machine)
        .expect("component machine exists");
    let records = member_affine_record_establishments(function, component);
    let [(_, establishment)] = records.as_slice() else {
        panic!("one member affine empty-record establishment")
    };
    let record_operation = operation_of(establishment);

    let candidates =
        propose_loop_invariant_scalar_motion(&session, 8).expect("exact relocation candidates");
    assert!(
        candidates
            .iter()
            .flat_map(|candidate| candidate.relocations().iter())
            .all(|relocation| relocation.node().psi_operation() != record_operation),
        "an affine record a bypassing exit can skip stays inside"
    );
}

#[test]
fn forged_affine_record_result_is_rejected_by_the_freeze_fence() {
    let session = lowered_session_entry(
        MEMBER_AFFINE_RECORD_SOURCE,
        "member affine empty-record loop",
        "Root::scan",
    );
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
    let records = member_affine_record_establishments(function, component);
    let [(_, establishment)] = records.as_slice() else {
        panic!("one member affine empty-record establishment")
    };
    let record_operation = operation_of(establishment);
    let candidate = propose_loop_invariant_scalar_motion(&session, 8)
        .expect("exact candidate")
        .pop()
        .expect("one candidate");
    let relocation = candidate
        .relocations()
        .iter()
        .find(|relocation| relocation.node().psi_operation() == record_operation)
        .expect("the affine empty-record establishment is a planned relocation");
    let member = relocation.node().location().block;
    let validated = validate_loop_invariant_scalar_motion(&session, &candidate)
        .expect("validated exact candidate");
    let applied =
        apply_loop_invariant_scalar_motion(session, validated).expect("applied exact candidate");
    let (input, mut unit) = applied.into_session().into_parts();
    // Forging the moved result's multiplicity to unrestricted drops the
    // affine custody the relocation re-expressed — the replayed operation
    // comparison retains every source-owned field, so the drifted spelling
    // rejects byte-exact.
    let forged = find_operation_mut(&mut unit, record_operation);
    if let AbstractOperation::EstablishRecord { result, .. } = &mut forged.operation {
        result.multiplicity = terminal_psi::StructuralMultiplicity::Unrestricted;
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
fn kept_internal_affine_record_discard_is_rejected_by_the_freeze_fence() {
    let session = lowered_session_entry(
        MEMBER_AFFINE_RECORD_SOURCE,
        "member affine empty-record loop",
        "Root::scan",
    );
    let [component] = session.cycle_components().components() else {
        panic!("one cyclic component")
    };
    let machine = component.id.machine;
    let member_targets: std::collections::BTreeSet<_> = component.members.iter().copied().collect();
    let function = session
        .unit()
        .functions
        .iter()
        .find(|function| function.machine == machine)
        .expect("component machine exists");
    let records = member_affine_record_establishments(function, component);
    let [(_, establishment)] = records.as_slice() else {
        panic!("one member affine empty-record establishment")
    };
    let picked = match &establishment.operation {
        AbstractOperation::EstablishRecord { result, .. } => result.place,
        operation => panic!("the member node is a record establishment: {operation:?}"),
    };
    let candidate = propose_loop_invariant_scalar_motion(&session, 8)
        .expect("exact candidate")
        .pop()
        .expect("one candidate");
    let validated = validate_loop_invariant_scalar_motion(&session, &candidate)
        .expect("validated exact candidate");
    let applied =
        apply_loop_invariant_scalar_motion(session, validated).expect("applied exact candidate");
    let (input, mut unit) = applied.into_session().into_parts();
    // Forging a member-internal edge to keep discarding the persistent
    // result restores the source's per-traversal custody: the first
    // traversal would end the one preheader place the next traversal still
    // owns. The freeze replay normalizes the seed's retained node through
    // the same custody rewrite — internal edges stripped, exits disposing —
    // so the kept discard rejects byte-exact.
    let mut forged = false;
    for function in &mut unit.functions {
        for block in &mut function.blocks {
            if !member_targets.contains(&block.id) {
                continue;
            }
            for node in &mut block.nodes {
                for edge in &mut node.successors {
                    if member_targets.contains(&edge.target)
                        && !edge.trivial_affine_discards.contains(&picked)
                    {
                        edge.trivial_affine_discards.push(picked);
                        forged = true;
                    }
                }
                match &mut node.operation {
                    AbstractOperation::Jump {
                        target,
                        trivial_affine_discards,
                        ..
                    } if member_targets.contains(target) => {
                        if !trivial_affine_discards.contains(&picked) {
                            trivial_affine_discards.push(picked);
                        }
                    }
                    AbstractOperation::Conditional {
                        when_true,
                        when_false,
                        ..
                    } => {
                        for successor in [when_true, when_false] {
                            if member_targets.contains(&successor.target)
                                && !successor.trivial_affine_discards.contains(&picked)
                            {
                                successor.trivial_affine_discards.push(picked);
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    assert!(forged, "a member-internal edge carried the forged discard");
    unit.identity = recompute_psi_optimization_unit_identity(&unit);
    assert!(matches!(
        crate::validation::validate_transformed_psi_optimization_unit(&input, &unit),
        Err(
            OptimizationUnitValidationError::RankedCycleFrozenBlockMismatch {
                machine: rejected_machine,
                ..
            }
        ) if rejected_machine == machine
    ));
}

#[test]
fn dropped_exit_affine_record_disposal_is_rejected_by_the_freeze_fence() {
    let session = lowered_session_entry(
        MEMBER_AFFINE_RECORD_SOURCE,
        "member affine empty-record loop",
        "Root::scan",
    );
    let [component] = session.cycle_components().components() else {
        panic!("one cyclic component")
    };
    let machine = component.id.machine;
    let member_targets: std::collections::BTreeSet<_> = component.members.iter().copied().collect();
    let function = session
        .unit()
        .functions
        .iter()
        .find(|function| function.machine == machine)
        .expect("component machine exists");
    let records = member_affine_record_establishments(function, component);
    let [(_, establishment)] = records.as_slice() else {
        panic!("one member affine empty-record establishment")
    };
    let picked = match &establishment.operation {
        AbstractOperation::EstablishRecord { result, .. } => result.place,
        operation => panic!("the member node is a record establishment: {operation:?}"),
    };
    // The exit edge is a member terminator edge departing the roster —
    // `check`'s `finish` arm — which must dispose the persistent result.
    let exit = component
        .members
        .iter()
        .flat_map(|member| {
            function
                .blocks
                .iter()
                .find(|block| block.id == *member)
                .into_iter()
                .flat_map(|block| block.nodes.iter())
        })
        .flat_map(|node| node.successors.iter())
        .find(|edge| !member_targets.contains(&edge.target))
        .expect("the component has an exit edge");
    let exit_edge = exit.psi_edge;
    let candidate = propose_loop_invariant_scalar_motion(&session, 8)
        .expect("exact candidate")
        .pop()
        .expect("one candidate");
    let validated = validate_loop_invariant_scalar_motion(&session, &candidate)
        .expect("validated exact candidate");
    let applied =
        apply_loop_invariant_scalar_motion(session, validated).expect("applied exact candidate");
    let (input, mut unit) = applied.into_session().into_parts();
    // The applied transform disposes `picked` on the exit edge. Forging the
    // edge back to the seed's empty roster leaves the persistent place live
    // outside the component — the freeze replay's normalized custody expects
    // the disposal, so the drop rejects byte-exact.
    let mut forged = false;
    for function in &mut unit.functions {
        for block in &mut function.blocks {
            if !member_targets.contains(&block.id) {
                continue;
            }
            for node in &mut block.nodes {
                for edge in &mut node.successors {
                    if edge.psi_edge == exit_edge
                        && let Some(index) = edge
                            .trivial_affine_discards
                            .iter()
                            .position(|place| *place == picked)
                    {
                        edge.trivial_affine_discards.remove(index);
                        forged = true;
                    }
                }
                match &mut node.operation {
                    AbstractOperation::Jump {
                        trivial_affine_discards,
                        ..
                    } if node
                        .successors
                        .first()
                        .is_some_and(|edge| edge.psi_edge == exit_edge) =>
                    {
                        trivial_affine_discards.retain(|place| *place != picked);
                    }
                    AbstractOperation::Conditional {
                        when_true,
                        when_false,
                        ..
                    } => {
                        for successor in [when_true, when_false] {
                            if successor.psi_edge == exit_edge {
                                forged |= !successor.trivial_affine_discards.is_empty();
                                successor
                                    .trivial_affine_discards
                                    .retain(|place| *place != picked);
                            }
                        }
                    }
                    AbstractOperation::StructuralCase { cases, .. } => {
                        for case in cases {
                            if case.psi_edge == exit_edge {
                                forged |= !case.trivial_affine_discards.is_empty();
                                case.trivial_affine_discards
                                    .retain(|place| *place != picked);
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    assert!(
        forged,
        "the exit edge carried the relocated result's disposal"
    );
    unit.identity = recompute_psi_optimization_unit_identity(&unit);
    assert!(matches!(
        crate::validation::validate_transformed_psi_optimization_unit(&input, &unit),
        Err(
            OptimizationUnitValidationError::RankedCycleFrozenBlockMismatch {
                machine: rejected_machine,
                ..
            }
        ) if rejected_machine == machine
    ));
}

#[test]
fn stale_affine_record_frontier_catalog_is_rejected() {
    let session = lowered_session_entry(
        MEMBER_AFFINE_RECORD_SOURCE,
        "member affine empty-record loop",
        "Root::scan",
    );
    let [component] = session.cycle_components().components() else {
        panic!("one cyclic component")
    };
    let machine = component.id.machine;
    // The seed catalog still carries the source's per-traversal custody:
    // `picked` owned between its member establishment and the member-edge
    // discards, dead everywhere else.
    let seed_frontier_facts = session.unit().ownership_frontier_facts.clone();
    let candidate = propose_loop_invariant_scalar_motion(&session, 8)
        .expect("exact candidate")
        .pop()
        .expect("one candidate");
    let validated = validate_loop_invariant_scalar_motion(&session, &candidate)
        .expect("validated exact candidate");
    let applied =
        apply_loop_invariant_scalar_motion(session, validated).expect("applied exact candidate");
    let (input, mut unit) = applied.into_session().into_parts();
    // Forging the frontier catalog back to the seed's spelling leaves the
    // affine-authority replay reading custody the transformed edges no
    // longer execute — internal edges keep the persistent place live where
    // the stale catalog still ends it at dispatch — so the stale
    // membership rejects before the catalog comparison is even reached.
    unit.ownership_frontier_facts = seed_frontier_facts;
    unit.identity = recompute_psi_optimization_unit_identity(&unit);
    assert!(matches!(
        crate::validation::validate_transformed_psi_optimization_unit(&input, &unit),
        Err(
            OptimizationUnitValidationError::StructuralEdgeAffineDiscardsMismatch {
                machine: rejected_machine,
                ..
            }
        ) if rejected_machine == machine
    ));
}
