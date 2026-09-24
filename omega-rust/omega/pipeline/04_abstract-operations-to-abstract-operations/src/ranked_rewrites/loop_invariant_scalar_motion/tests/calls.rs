//! Loop-invariant calls: scalar, unit, structural-scalar and structural
//! calls, their borrowed and owned argument roots, mutable-borrow producers,
//! callee purity, and the dispatch custody a relocated structural call
//! re-expresses.

use crate::VerifiedPsiOptimizationSession;
use crate::ranked_rewrites::LoopInvariantNodeResult;
use crate::{
    apply_loop_invariant_scalar_motion, propose_loop_invariant_scalar_motion,
    validate_loop_invariant_scalar_motion,
};
use abstract_operations::AbstractOperation;
use optimization_unit::{
    ProvenanceDisposition, PsiRealizationSite, recompute_psi_optimization_unit_identity,
};
use optimization_unit_semantics::OptimizationUnitValidationError;

use super::{
    BYPASSED_LITERAL_SOURCE, INVARIANT_LITERAL_SOURCE, MEMBER_SCALAR_ARRAY_SOURCE,
    find_operation_mut, lowered_session, lowered_session_entry, member_call, member_field_reads,
    member_literal, member_primitive_local, member_structural_scalar_call,
    member_structural_scalar_calls, operation_of, refresh_coordinates_and_effects, take_operation,
};

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
    let anchor = crate::validation::member_blocks::invariant_member_parameters(function, component)
        [&argument];

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
        !crate::validation::member_blocks::invariant_member_parameters(function, component)
            .contains_key(&argument),
        "the back edge advances the call's argument, so it stays loop-carried"
    );
    let effects = crate::validation::invariant_calls::unit_effect_summaries(session.unit());
    assert!(
        crate::validation::invariant_calls::invariant_scalar_call_admission(
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
    let effects = crate::validation::invariant_calls::unit_effect_summaries(session.unit());
    assert!(
        crate::validation::invariant_calls::invariant_scalar_call_admission(
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
        !crate::validation::member_blocks::guaranteed_executed_member_blocks(component)
            .contains(&call_block.id),
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
    let effects = crate::validation::invariant_calls::unit_effect_summaries(session.unit());
    // The real summary admits the call: the refusals below isolate the
    // callee-purity half of admission — the member roster is unchanged and
    // the argument substitution still resolves.
    assert!(
        crate::validation::invariant_calls::invariant_scalar_call_admission(
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
            crate::validation::invariant_calls::invariant_scalar_call_admission(
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
        crate::validation::invariant_calls::invariant_scalar_call_admission(
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
    let effects = crate::validation::invariant_calls::unit_effect_summaries(session.unit());
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
        crate::validation::invariant_calls::invariant_scalar_call_admission(
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
    let representative = crate::validation::place_observations::invariant_member_place_parameters(
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
        !crate::validation::place_observations::invariant_member_place_parameters(
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
        crate::validation::member_blocks::invariant_member_parameters(function, component)
            [&scalar_argument];

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

/// A `CallStructuralScalar` spelling an `Owned` whole-root argument over the
/// machine's own unrestricted record parameter: `first(row)` inside `scan`
/// copies the parameter's payload into the callee — an observation, not
/// custody movement — and `row`'s member spelling resolves transitively to
/// the machine parameter's preheader-visible root, so the relocated call
/// rebinds its argument to that root. The member-produced counterpart is
/// [`MEMBER_SCALAR_ARRAY_SOURCE`]'s relocated consumer.
const OWNED_PARAMETER_SCALAR_CALL_SOURCE: &str = r#"
    data Pair [copy] { a: u64; b: u64; }
    machine first(pair: Pair) -> u64 { 0 }
    machine scan(row: Pair, remaining: u64 [0..=5], scale: u64 [0..=10]) -> u64
    terminates by remaining -> Nat::Descending in 0..6;
    {
        let v: u64 = first(row);
        transition remaining > 0 {
            true -> scan(row, remaining - 1, scale)
            _ -> v
        }
    }
"#;

#[test]
fn invariant_owned_parameter_scalar_call_relocates_rebinding_its_root() {
    let session = lowered_session_entry(
        OWNED_PARAMETER_SCALAR_CALL_SOURCE,
        "owned parameter scalar-call loop",
        "scan",
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
    // `row` is the machine's unrestricted owned parameter — the copyable
    // shape the `Owned` whitelist admits — spelled inside the member block
    // through its own parameter place.
    let row_root = function
        .structural_parameters
        .iter()
        .find(|parameter| {
            !parameter.is_self
                && parameter.access == terminal_psi::StructuralAccess::Owned
                && parameter.multiplicity == terminal_psi::StructuralMultiplicity::Unrestricted
        })
        .map(|parameter| parameter.place)
        .expect("the machine's `row` parameter declares the copyable owned shape");
    let member_root = component
        .members
        .iter()
        .flat_map(|member| {
            function
                .blocks
                .iter()
                .find(|block| block.id == *member)
                .into_iter()
                .flat_map(|block| block.structural_parameters.iter())
        })
        .find(|parameter| {
            !parameter.is_self
                && parameter.access == terminal_psi::StructuralAccess::Owned
                && parameter.multiplicity == terminal_psi::StructuralMultiplicity::Unrestricted
        })
        .map(|parameter| parameter.place)
        .expect("the member's `row` parameter declares the copyable owned shape");
    let (call_operation, call_result) = component
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
            AbstractOperation::CallStructuralScalar {
                psi_operation,
                result,
                structural_arguments,
                ..
            } => {
                let [argument] = structural_arguments.as_slice() else {
                    panic!("the owned-argument call carries one structural argument")
                };
                assert_eq!(
                    argument.access,
                    terminal_psi::StructuralAccess::Owned,
                    "the call copies the machine's record parameter"
                );
                assert_eq!(
                    argument.place, member_root,
                    "the owned argument names the member's parameter spelling"
                );
                Some((*psi_operation, result.value))
            }
            _ => None,
        })
        .expect("the member block holds the owned-argument scalar call");
    let representative = crate::validation::place_observations::invariant_member_place_parameters(
        function,
        component,
        &std::collections::BTreeSet::new(),
    )
    .get(&member_root)
    .copied()
    .expect("the member owned parameter resolves to a preheader-visible root");
    assert_eq!(
        representative, row_root,
        "the member parameter resolves to the machine's `row` root"
    );

    let candidates =
        propose_loop_invariant_scalar_motion(&session, 8).expect("exact relocation candidates");
    let [candidate] = candidates.as_slice() else {
        panic!("one component yields one atomic candidate")
    };
    let relocation = candidate
        .relocations()
        .iter()
        .find(|relocation| relocation.node().psi_operation() == call_operation)
        .expect("the owned-parameter call is a planned relocation");
    let LoopInvariantNodeResult::Scalar { value, .. } = relocation.node().result() else {
        panic!("the call relocates its scalar result")
    };
    assert_eq!(
        *value, call_result,
        "the relocated call preserves its scalar result identity"
    );
    assert_eq!(
        relocation.node().argument_rewrites(),
        &[(member_root, row_root)],
        "the relocation records the member parameter's rebind to `row`"
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
    let AbstractOperation::CallStructuralScalar {
        structural_arguments,
        ..
    } = &moved.operation
    else {
        panic!("the relocated node keeps its call operation")
    };
    let [argument] = structural_arguments.as_slice() else {
        panic!("the moved call keeps one structural argument")
    };
    assert_eq!(
        argument.access,
        terminal_psi::StructuralAccess::Owned,
        "the owned access rides byte-exact"
    );
    assert_eq!(
        argument.place, row_root,
        "the moved call still copies the machine parameter root"
    );
    assert!(
        applied
            .session()
            .unit()
            .functions
            .iter()
            .flat_map(|function| &function.blocks)
            .filter(|block| member_targets.contains(&block.id))
            .flat_map(|block| &block.nodes)
            .all(|node| !matches!(
                node.operation,
                AbstractOperation::CallStructuralScalar { .. }
            )),
        "the member roster keeps no copy of the relocated call"
    );
    assert!(
        propose_loop_invariant_scalar_motion(applied.session(), 1)
            .expect("relocated session is an exact fixed point")
            .is_empty()
    );
}

#[test]
fn owned_argument_call_moved_without_its_producer_is_rejected_by_the_freeze_fence() {
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
    let machine = component.id.machine;
    let function = session
        .unit()
        .functions
        .iter()
        .find(|function| function.machine == machine)
        .expect("component machine exists");
    let (call_block, call_operation) = component
        .members
        .iter()
        .flat_map(|member| {
            function
                .blocks
                .iter()
                .find(|block| block.id == *member)
                .into_iter()
                .flat_map(|block| block.nodes.iter().map(move |node| (block.id, node)))
        })
        .find_map(|(block, node)| match &node.operation {
            AbstractOperation::CallStructuralScalar { psi_operation, .. } => {
                Some((block, *psi_operation))
            }
            _ => None,
        })
        .expect("the member block holds the owned-argument scalar call");
    let member = call_block;
    let preheader = entry.source;
    let (input, mut unit) = session.into_parts();
    // Hand-move only the call: its `Owned` argument still names the array's
    // member-produced place, but the producer stayed inside — the freeze
    // fence re-derives the admission and refuses because no node in the
    // component's relocated run covered the copied root.
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

/// A `CallStructural` carrying a shared borrow of an invariant member
/// structural parameter: `pick(b, s)` inside `step` borrows `b`, which every
/// reaching edge resolves to the machine's `buf` parameter root, and returns
/// the affine `picked` the same block dispatches — the cyclic-eligibility
/// fence already confines that result, so the relocated call rebinds the
/// borrowed root to `buf`, rebinds `s` to `scale`'s anchor, keeps its
/// vacuous claim-transfer row byte-exact, and leaves the persistent result's
/// re-expressed custody to the member-edge rewrite.
const SHARED_BORROW_STRUCTURAL_CALL_SOURCE: &str = r#"
    data Root {}
    data Step { case More(rest: u64); case Halt(tag: u64); }

    machine pick(view: &[u8], seed: u64) -> Step { Step::More { rest: view.len } }

    machine Root::scan(scale: u64, buf: &[u8], spare: &[u8], remaining: u64 [0..=5])
    {
        transition { _ -> step(scale, buf, remaining) }
        state step(s: u64, b: &[u8], pending: u64 [0..=5]) {
            let picked: Step = pick(b, s);
            transition picked {
                Step::More { rest } -> check(rest, s, b, pending)
                Step::Halt { tag } -> check(tag, s, b, pending)
            }
        }
        state check(v: u64, s: u64, b: &[u8], pending: u64 [0..=5]) {
            transition pending > 0 {
                true -> step(s, b, pending - 1)
                _ -> finish(v)
            }
        }
        state finish(r: u64) {}
    }
"#;

/// Same `pick(b, s)` shape, but `b`'s reaching edges resolve to two different
/// preheader-visible roots — `buf` on the entry edge, `fallback` through
/// `alt`'s `spare` on the back edge — so the borrowed root is loop-carried
/// and the call stays inside: the carried-view counterpart for a
/// structural-result call.
const CARRIED_BORROW_STRUCTURAL_CALL_SOURCE: &str = r#"
    data Root {}
    data Step { case More(rest: u64); case Halt(tag: u64); }

    machine pick(view: &[u8], seed: u64) -> Step { Step::More { rest: view.len } }

    machine Root::scan(scale: u64, buf: &[u8], fallback: &[u8], remaining: u64 [0..=5])
    {
        transition { _ -> step(scale, buf, remaining, fallback) }
        state step(s: u64, b: &[u8], pending: u64 [0..=5], spare: &[u8]) {
            let picked: Step = pick(b, s);
            transition picked {
                Step::More { rest } -> check(rest, s, pending, spare)
                Step::Halt { tag } -> check(tag, s, pending, spare)
            }
        }
        state check(v: u64, s: u64, pending: u64 [0..=5], spare: &[u8]) {
            transition pending > 0 {
                true -> alt(pending - 1, s, spare)
                _ -> finish(v)
            }
        }
        state alt(pending: u64 [0..=5], s: u64, spare: &[u8]) {
            transition { _ -> step(s, spare, pending, spare) }
        }
        state finish(r: u64) {}
    }
"#;

/// Same `pick(b, s)` shape, but `check` stores through the machine's
/// `&mut self` receiver: a member mutates a place every traversal, so the
/// whole-component place-custody bound the borrow argument needs refuses —
/// the relocated invocation could not reproduce the traversal's observed
/// storage — and the call stays inside.
const MUTATED_MEMBER_STRUCTURAL_CALL_SOURCE: &str = r#"
    data Root { ticks: u32 in Wrapping }
    data Step { case More(rest: u64); case Halt(tag: u64); }

    machine pick(view: &[u8], seed: u64) -> Step { Step::More { rest: view.len } }

    machine Root::scan(&mut self, scale: u64, buf: &[u8], remaining: u64 [0..=5])
    {
        transition { _ -> step(scale, buf, remaining) }
        state step(&mut self, s: u64, b: &[u8], pending: u64 [0..=5]) {
            let picked: Step = pick(b, s);
            transition picked {
                Step::More { rest } -> check(rest, s, b, pending)
                Step::Halt { tag } -> check(tag, s, b, pending)
            }
        }
        state check(&mut self, v: u64, s: u64, b: &[u8], pending: u64 [0..=5]) {
            self.ticks = self.ticks + 1;
            transition pending > 0 {
                true -> step(s, b, pending - 1)
                _ -> finish(v)
            }
        }
        state finish(&mut self, r: u64) {}
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
fn invariant_borrow_structural_call_relocates_rebinding_its_borrowed_root() {
    let session = lowered_session_entry(
        SHARED_BORROW_STRUCTURAL_CALL_SOURCE,
        "shared borrow structural-call loop",
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
    let [(call_block, call)] = calls.as_slice() else {
        panic!("one member structural call")
    };
    let (call_operation, picked, scalar_argument, borrowed_place) = match &call.operation {
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
                claim_transfers.is_empty()
                    && returned_claim_transfers.is_empty()
                    && requirement_obligations.is_empty()
                    && crash_continuations.is_empty()
                    && selected_evidence.is_empty(),
                "the admitted call shape carries no evidence surface beyond its result and arguments"
            );
            assert_eq!(
                result.multiplicity,
                terminal_psi::StructuralMultiplicity::Affine,
                "the result is the confined affine sum"
            );
            let [structural_argument] = structural_arguments.as_slice() else {
                panic!("the call carries one structural argument")
            };
            assert_eq!(
                structural_argument.access,
                terminal_psi::StructuralAccess::SharedBorrow,
                "the call borrows the member view"
            );
            assert!(
                structural_argument.path.is_empty(),
                "the borrow names the member parameter root"
            );
            let [scalar_argument] = arguments.as_slice() else {
                panic!("the call carries one scalar argument")
            };
            (
                *psi_operation,
                result.place,
                *scalar_argument,
                structural_argument.place,
            )
        }
        operation => panic!("the member node is a structural call: {operation:?}"),
    };
    // The borrowed place is `step`'s `b` member parameter; every edge binding
    // it resolves to the machine's `buf` root, so the representative is the
    // preheader-visible `buf` place.
    let [member_parameter] = call_block.structural_parameters.as_slice() else {
        panic!("the call's member carries one view structural parameter")
    };
    assert_eq!(
        member_parameter.place, borrowed_place,
        "the borrowed argument names the member parameter"
    );
    let representative = crate::validation::place_observations::invariant_member_place_parameters(
        function,
        component,
        &std::collections::BTreeSet::new(),
    )
    .get(&borrowed_place)
    .copied()
    .expect("the member view parameter resolves to a preheader-visible root");
    let buf_root = function
        .structural_parameters
        .iter()
        .map(|parameter| parameter.place)
        .find(|place| *place == representative)
        .expect("the representative is the machine's `buf` parameter root");
    assert!(
        function
            .structural_parameters
            .iter()
            .any(|parameter| parameter.place != buf_root),
        "the machine retains a second structural root for forging"
    );
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
        .expect("the borrow-carrying structural call is a planned relocation");
    let LoopInvariantNodeResult::Structural(result) = relocation.node().result() else {
        panic!("the structural call relocates its structural result")
    };
    assert_eq!(result.place, picked, "the declared place is byte-exact");
    let rewrites = relocation.node().operand_rewrites();
    let [(rewritten_argument, _)] = rewrites else {
        panic!("the scalar argument carries one member-parameter rewrite")
    };
    assert_eq!(
        *rewritten_argument, scalar_argument,
        "the rewrite spells the call's scalar argument"
    );
    let anchor = function
        .parameters
        .iter()
        .find(|parameter| parameter.value == rewrites[0].1)
        .expect("the scalar representative is the machine's `scale` parameter")
        .value;
    assert_eq!(
        relocation.node().argument_rewrites(),
        &[(borrowed_place, buf_root)],
        "the relocation records the borrowed root's rebind"
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
        AbstractOperation::CallStructural {
            result,
            arguments,
            structural_arguments,
            ..
        } => {
            assert_eq!(result.place, picked, "the declared place is byte-exact");
            for argument in arguments {
                assert_eq!(
                    *argument, anchor,
                    "the moved call rebinds its scalar argument to the preheader anchor"
                );
            }
            let [structural_argument] = structural_arguments.as_slice() else {
                panic!("the moved call keeps one structural argument")
            };
            assert_eq!(
                structural_argument.place, buf_root,
                "the moved call borrows the preheader-visible root"
            );
            assert_eq!(
                structural_argument.access,
                terminal_psi::StructuralAccess::SharedBorrow,
                "the borrow's access rides byte-exact"
            );
        }
        operation => panic!("relocated node keeps its call operation: {operation:?}"),
    }
    assert_eq!(moved.provenance, relocation.node().provenance());
    assert_eq!(moved.fuel, relocation.node().fuel());
    // The retained dispatch keeps the persistent result live on
    // member-internal edges and disposes it on the component's exit.
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
fn carried_borrow_structural_call_stays_inside() {
    let session = lowered_session_entry(
        CARRIED_BORROW_STRUCTURAL_CALL_SOURCE,
        "carried borrow structural-call loop",
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
    match &call.operation {
        AbstractOperation::CallStructural {
            structural_arguments,
            ..
        } => assert_eq!(
            structural_arguments[0].access,
            terminal_psi::StructuralAccess::SharedBorrow,
            "the member call borrows its carried view"
        ),
        operation => panic!("the member node is a structural call: {operation:?}"),
    }

    let candidates =
        propose_loop_invariant_scalar_motion(&session, 8).expect("exact relocation candidates");
    assert!(
        candidates
            .iter()
            .flat_map(|candidate| candidate.relocations().iter())
            .all(|relocation| relocation.node().psi_operation() != call_operation),
        "a structural call borrowing a carried root stays inside"
    );
}

#[test]
fn member_mutation_keeps_borrow_structural_call_inside() {
    let session = lowered_session_entry(
        MUTATED_MEMBER_STRUCTURAL_CALL_SOURCE,
        "mutated member structural-call loop",
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
    assert!(
        !crate::validation::place_observations::component_preserves_place_observations(
            function, component
        ),
        "the member's `self.ticks` store ends the component's place custody"
    );
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
        "a borrow call cannot be replayed once per traversal's observed storage"
    );
}

#[test]
fn forged_structural_call_borrow_root_is_rejected_by_the_freeze_fence() {
    let session = lowered_session_entry(
        SHARED_BORROW_STRUCTURAL_CALL_SOURCE,
        "shared borrow structural-call loop",
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
    let (call_operation, borrowed_place) = match &call.operation {
        AbstractOperation::CallStructural {
            psi_operation,
            structural_arguments,
            ..
        } => (*psi_operation, structural_arguments[0].place),
        operation => panic!("the member node is a structural call: {operation:?}"),
    };
    // `spare` is the machine's second structural parameter: a
    // preheader-visible place that is not the derived representative, so a
    // moved call spelling it is a well-formed place reference that fails
    // admission replay.
    let representative = crate::validation::place_observations::invariant_member_place_parameters(
        function,
        component,
        &std::collections::BTreeSet::new(),
    )
    .get(&borrowed_place)
    .copied()
    .expect("the member view parameter resolves to the `buf` root");
    let stale_root = function
        .structural_parameters
        .iter()
        .map(|parameter| parameter.place)
        .find(|place| *place != representative)
        .expect("the machine retains a second structural root");
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
    // Forging the moved borrow's root to the machine's other structural
    // parameter leaves a legal place reference that skips the seed-derived
    // substitution — the replayed admission derives `buf`, so the byte-exact
    // operation comparison rejects the drifted spelling.
    let forged = find_operation_mut(&mut unit, call_operation);
    if let AbstractOperation::CallStructural {
        structural_arguments,
        ..
    } = &mut forged.operation
    {
        structural_arguments[0].place = stale_root;
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
fn kept_internal_borrow_call_discard_is_rejected_by_the_freeze_fence() {
    let session = lowered_session_entry(
        SHARED_BORROW_STRUCTURAL_CALL_SOURCE,
        "shared borrow structural-call loop",
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
    // Restoring the seed's per-traversal discard on the dispatch's
    // member-internal edges ends the one preheader place the next traversal
    // dispatches — the freeze replay's normalized custody keeps it live, so
    // the kept discard rejects byte-exact even though admission tolerated
    // the call's own result while the borrow ran.
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
        arguments.fill(member_argument);
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

/// A `CallStructural` whose unrestricted claim-free record result is a copy
/// payload: `pick(s)` inside `step` returns the unrestricted `picked` that
/// `v` reads once in the same member. The cyclic eligibility fence admits
/// the producer in its plain-source form — the copy payload never enters
/// `owned_places`, so no disposal roster exists — so the relocation needs
/// neither the scalar-case containment bound nor the member-edge custody
/// rewrite: it rebinds `s` to `scale`'s anchor and leaves the persistent
/// result's declared place for the member read to keep spelling byte-exact.
const UNRESTRICTED_STRUCTURAL_CALL_SOURCE: &str = r#"
    data Root {}
    data Pair [copy] { a: u64; b: u64; }

    machine pick(seed: u64) -> Pair { Pair { a: seed, b: seed } }

    machine Root::scan(scale: u64, remaining: u64 [0..=5])
    {
        transition { _ -> step(scale, remaining) }
        state step(s: u64, pending: u64 [0..=5]) {
            let picked: Pair = pick(s);
            let v: u64 = picked.a;
            transition pending > 0 {
                true -> step(s, pending - 1)
                _ -> finish(v)
            }
        }
        state finish(r: u64) {}
    }
"#;

/// Same `pick(s)` shape, but `step` stores through the machine's `&mut self`
/// receiver: a member mutates a place every traversal, so the whole-component
/// place-custody bound the call's relocation replays refuses — the persistent
/// preheader result could not reproduce the traversal's observed storage —
/// and the call stays inside.
const MUTATED_MEMBER_UNRESTRICTED_STRUCTURAL_CALL_SOURCE: &str = r#"
    data Root { ticks: u32 in Wrapping }
    data Pair [copy] { a: u64; b: u64; }

    machine pick(seed: u64) -> Pair { Pair { a: seed, b: seed } }

    machine Root::scan(&mut self, scale: u64, remaining: u64 [0..=5])
    {
        transition { _ -> step(scale, remaining) }
        state step(&mut self, s: u64, pending: u64 [0..=5]) {
            let picked: Pair = pick(s);
            let v: u64 = picked.a;
            self.ticks = self.ticks + 1;
            transition pending > 0 {
                true -> step(s, pending - 1)
                _ -> finish(v)
            }
        }
        state finish(&mut self, r: u64) {}
    }
"#;

#[test]
fn unrestricted_structural_call_result_relocates_without_custody_rewrite() {
    let session = lowered_session_entry(
        UNRESTRICTED_STRUCTURAL_CALL_SOURCE,
        "unrestricted structural-call loop",
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
                terminal_psi::StructuralMultiplicity::Unrestricted,
                "the result is the unrestricted copy payload"
            );
            (*psi_operation, result.place, arguments.clone())
        }
        operation => panic!("the member node is a structural call: {operation:?}"),
    };
    assert_eq!(arguments.len(), 1, "the call carries one scalar argument");
    // The unrestricted result carries no disposal roster anywhere in the
    // seed: the copy payload never enters `owned_places`.
    for node in function.blocks.iter().flat_map(|block| &block.nodes) {
        for edge in &node.successors {
            assert!(
                !edge.trivial_affine_discards.contains(&picked),
                "the copy payload is never disposal custody"
            );
        }
    }
    let reads = member_field_reads(function, component);
    let [(_, read)] = reads.as_slice() else {
        panic!("one member field read of the unrestricted result")
    };
    let read_operation = operation_of(read);
    if let AbstractOperation::IntegerStructuralField { source, .. } = &read.operation {
        assert_eq!(*source, picked, "the member read observes the result place");
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
    assert_eq!(relocation.destination().block, entry.source);
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
    // The member read of the persistent copy payload relocates behind its
    // producer in the same run.
    let read_relocation = candidate
        .relocations()
        .iter()
        .find(|relocation| relocation.node().psi_operation() == read_operation)
        .expect("the member field read relocates behind the call");
    assert_eq!(read_relocation.destination().block, entry.source);

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
            assert_eq!(
                result.multiplicity,
                terminal_psi::StructuralMultiplicity::Unrestricted,
                "the moved result keeps its unrestricted multiplicity"
            );
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
    // No custody rewrite applies to the copy payload: every retained edge —
    // member-internal and exit alike — keeps its seed discard roster
    // byte-exact, and no roster ever names `picked`.
    for block in applied
        .session()
        .unit()
        .functions
        .iter()
        .flat_map(|function| {
            function
                .blocks
                .iter()
                .filter(|block| member_targets.contains(&block.id))
        })
    {
        for node in &block.nodes {
            for edge in &node.successors {
                assert!(
                    !edge.trivial_affine_discards.contains(&picked),
                    "no edge disposes the custody-free result"
                );
            }
        }
    }
    assert!(
        propose_loop_invariant_scalar_motion(applied.session(), 1)
            .expect("relocated session is an exact fixed point")
            .is_empty()
    );
}

#[test]
fn member_mutation_keeps_unrestricted_structural_call_inside() {
    let session = lowered_session_entry(
        MUTATED_MEMBER_UNRESTRICTED_STRUCTURAL_CALL_SOURCE,
        "mutated member unrestricted structural-call loop",
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
        "a member mutating a place keeps the copy-payload call inside"
    );
}

#[test]
fn forged_unrestricted_result_multiplicity_is_rejected_by_the_freeze_fence() {
    let session = lowered_session_entry(
        UNRESTRICTED_STRUCTURAL_CALL_SOURCE,
        "unrestricted structural-call loop",
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
    let call_operation = operation_of(call);
    let candidate = propose_loop_invariant_scalar_motion(&session, 8)
        .expect("exact candidate")
        .pop()
        .expect("one candidate");
    let relocation = candidate
        .relocations()
        .iter()
        .find(|relocation| relocation.node().psi_operation() == call_operation)
        .expect("the unrestricted structural call is a planned relocation");
    let member = relocation.node().location().block;
    let validated = validate_loop_invariant_scalar_motion(&session, &candidate)
        .expect("validated exact candidate");
    let applied =
        apply_loop_invariant_scalar_motion(session, validated).expect("applied exact candidate");
    let (input, mut unit) = applied.into_session().into_parts();
    // Forging the moved result's multiplicity to linear invents custody the
    // relocation never re-expressed — the replayed operation comparison
    // retains every source-owned field, so the drifted spelling rejects
    // byte-exact.
    let forged = find_operation_mut(&mut unit, call_operation);
    if let AbstractOperation::CallStructural { result, .. } = &mut forged.operation {
        result.multiplicity = terminal_psi::StructuralMultiplicity::Linear;
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

/// A two-state cycle whose member invokes a pure callee whose published
/// contract declares a crash route its body never exercises: `Root::bump`'s
/// transitive effect summary proves no observable effect, crash, or
/// suspension even though `crashes Abort` publishes a conditional route, so
/// the member call carries crash-route custody — and the relocation
/// re-derives the moved node's `crash_continuations` from the callee's
/// verifier-owned routes instantiated at the substituted `scale` anchor
/// rather than carrying the member-spelled roster byte-exact.
const CRASH_CONTINUATION_CALL_SOURCE: &str = r#"
    data Root {}
    machine Root::scan(scale: u64 in Wrapping, remaining: u64 [0..=5])
    crashes Abort
    {
        transition { _ -> step(scale, remaining) }
        state step(s: u64 in Wrapping, pending: u64 [0..=5]) {
            let bumped: u64 in Wrapping = Root::bump(s);
            let doubled: u64 in Wrapping = bumped + s;
            transition pending > 0 {
                true -> scan(s, pending - 1)
                _ -> finish(doubled)
            }
        }
        state finish(r: u64 in Wrapping) {}
    }
    machine Root::bump(v: u64 in Wrapping) -> u64 in Wrapping
    crashes Abort
        v == 0
    { v + 1 }
"#;

/// The same crash-custody call, but its argument is the loop-carried
/// countdown: the member parameter never resolves to a preheader
/// representative, so the call stays inside even though its callee is pure
/// and both crash rosters are derivable.
const CARRIED_CRASH_CALL_SOURCE: &str = r#"
    data Root {}
    machine Root::scan(scale: u64 in Wrapping, remaining: u64 [0..=5])
    crashes Abort
    {
        transition { _ -> step(scale, remaining) }
        state step(s: u64 in Wrapping, pending: u64 [0..=5]) {
            let bumped: u64 in Wrapping = Root::bump(pending);
            let doubled: u64 in Wrapping = bumped + s;
            transition pending > 0 {
                true -> scan(s, pending - 1)
                _ -> finish(doubled)
            }
        }
        state finish(r: u64 in Wrapping) {}
    }
    machine Root::bump(v: u64 in Wrapping) -> u64 in Wrapping
    crashes Abort
        v == 0
    { v + 1 }
"#;

/// The same crash-custody call, but the entry state's `done` arm can leave
/// the component before `step` ever runs: the non-speculative gate keeps the
/// call inside — relocating it would perform callee work a bypassed
/// traversal never performs.
const BYPASSED_CRASH_CALL_SOURCE: &str = r#"
    data Root {}
    machine Root::scan(scale: u64 in Wrapping, remaining: u64 [0..=5])
    crashes Abort
    {
        transition remaining > 0 {
            true -> step(scale, remaining - 1)
            _ -> done()
        }
        state step(s: u64 in Wrapping, pending: u64 [0..=5]) {
            let bumped: u64 in Wrapping = Root::bump(s);
            let doubled: u64 in Wrapping = bumped + s;
            transition pending > 0 {
                true -> scan(s, pending - 1)
                _ -> finish(doubled)
            }
        }
        state done() {}
        state finish(r: u64 in Wrapping) {}
    }
    machine Root::bump(v: u64 in Wrapping) -> u64 in Wrapping
    crashes Abort
        v == 0
    { v + 1 }
"#;

/// Every value identity a crash-continuation roster's predicates read.
fn crash_roster_references(
    continuations: &[terminal_psi::CrashRouteBucket],
) -> std::collections::BTreeSet<semantic_vocabulary::ValueId> {
    let mut references = std::collections::BTreeSet::new();
    for continuation in continuations {
        for guard in &continuation.alternatives {
            let terminal_psi::CrashRouteGuard::Predicate(term) = guard else {
                continue;
            };
            assert!(
                term.proposition().visit_value_ids(|value| {
                    references.insert(value);
                }),
                "the roster's predicates read only scalar value identities"
            );
        }
    }
    references
}

#[test]
fn crash_continuation_call_relocates_rederiving_its_roster() {
    let session = lowered_session(CRASH_CONTINUATION_CALL_SOURCE, "crash custody call loop");
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
    let member = call_block.id;
    let (call_operation, argument, callee, seed_continuations) = match &call.operation {
        AbstractOperation::Call {
            psi_operation,
            callee,
            arguments,
            crash_continuations,
            ..
        } => {
            assert!(
                !crash_continuations.is_empty(),
                "the member call carries crash-route custody"
            );
            (
                *psi_operation,
                arguments[0],
                *callee,
                crash_continuations.as_slice(),
            )
        }
        operation => panic!("the member node is a scalar call: {operation:?}"),
    };
    assert_eq!(
        crash_roster_references(seed_continuations),
        std::collections::BTreeSet::from([argument]),
        "the seed roster predicates read the member-parameter argument"
    );
    let anchor = crate::validation::member_blocks::invariant_member_parameters(function, component)
        [&argument];

    // The lane split is exact: the pure scalar-call lane refuses the
    // custody-carrying node, and the crash lane admits it under the shared
    // substitution — callee purity and member observability are intact.
    let effects = crate::validation::invariant_calls::unit_effect_summaries(session.unit());
    assert!(
        crate::validation::invariant_calls::invariant_scalar_call_admission(
            function,
            component,
            call,
            &std::collections::BTreeSet::new(),
            &effects,
        )
        .is_none(),
        "the pure lane keeps refusing the crash-custody call"
    );
    let substitution =
        crate::validation::invariant_calls::invariant_crash_continuation_call_admission(
            &session.unit().functions,
            function,
            component,
            call,
            &std::collections::BTreeSet::new(),
            &effects,
        )
        .expect("the crash-custody lane admits the invariant call");
    assert_eq!(substitution.get(&argument), Some(&anchor));

    let candidates =
        propose_loop_invariant_scalar_motion(&session, 8).expect("exact relocation candidates");
    let [candidate] = candidates.as_slice() else {
        panic!("one component yields one atomic candidate")
    };
    let relocation = candidate
        .relocations()
        .iter()
        .find(|relocation| relocation.node().psi_operation() == call_operation)
        .expect("the crash-custody call is a planned relocation");
    assert_eq!(
        relocation.node().operand_rewrites(),
        &[(argument, anchor)],
        "the call's member-parameter argument rebinds to its preheader anchor"
    );
    assert_eq!(relocation.node().location().block, member);
    assert_eq!(relocation.destination().block, entry.source);

    // The `bumped + s` computation consumes the call's preserved result, so
    // it relocates behind the call in the same run.
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
    let moved_continuations = match &moved.operation {
        AbstractOperation::Call {
            callee: moved_callee,
            arguments,
            crash_continuations,
            ..
        } => {
            assert_eq!(arguments.as_slice(), &[anchor]);
            assert_eq!(*moved_callee, callee, "callee identity is byte-exact");
            crash_continuations
        }
        operation => panic!("relocated node keeps its call operation: {operation:?}"),
    };
    // The roster is re-derived, not moved: every predicate reads the
    // preheader anchor — never the member parameter the seed roster spelled.
    assert_eq!(
        crash_roster_references(moved_continuations),
        std::collections::BTreeSet::from([anchor]),
        "the moved roster's predicates read the substituted anchor"
    );
    let callee_function = applied
        .session()
        .unit()
        .functions
        .iter()
        .find(|function| function.machine == callee)
        .expect("callee function exists");
    assert_eq!(
        moved_continuations.as_slice(),
        crate::validation::relocation_rewrites::call_crash_continuations(
            callee_function,
            &[anchor],
        )
        .expect("the callee contract re-derives the roster")
        .as_slice(),
        "the moved roster is the callee contract instantiated at the anchor"
    );
    assert_eq!(moved.provenance, relocation.node().provenance());
    assert_eq!(moved.fuel, relocation.node().fuel());
    let member_block = applied
        .session()
        .unit()
        .functions
        .iter()
        .flat_map(|function| &function.blocks)
        .find(|block| block.id == member)
        .expect("member block exists");
    assert!(
        member_block
            .nodes
            .iter()
            .all(|node| !matches!(node.operation, AbstractOperation::Call { .. })),
        "the call exists once, at the destination"
    );
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
fn carried_argument_crash_call_stays_inside() {
    let session = lowered_session(CARRIED_CRASH_CALL_SOURCE, "carried crash custody call loop");
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
    let (call_operation, argument) = match &call.operation {
        AbstractOperation::Call {
            psi_operation,
            arguments,
            ..
        } => (*psi_operation, arguments[0]),
        operation => panic!("the member node is a scalar call: {operation:?}"),
    };
    assert!(
        !crate::validation::member_blocks::invariant_member_parameters(function, component)
            .contains_key(&argument),
        "the back edge advances the call's argument, so it stays loop-carried"
    );
    let effects = crate::validation::invariant_calls::unit_effect_summaries(session.unit());
    assert!(
        crate::validation::invariant_calls::invariant_crash_continuation_call_admission(
            &session.unit().functions,
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
    assert!(
        candidates
            .iter()
            .flat_map(|candidate| candidate.relocations().iter())
            .all(|relocation| relocation.node().psi_operation() != call_operation),
        "the carried-argument crash call is not a planned relocation"
    );
}

#[test]
fn bypassed_crash_continuation_call_stays_inside() {
    let session = lowered_session(
        BYPASSED_CRASH_CALL_SOURCE,
        "bypassed crash custody call loop",
    );
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
    // Crash-lane admission itself is intact — the refusal is the
    // non-speculative gate alone: the call's member block does not dominate
    // the entry state's own `done` exit.
    let effects = crate::validation::invariant_calls::unit_effect_summaries(session.unit());
    assert!(
        crate::validation::invariant_calls::invariant_crash_continuation_call_admission(
            &session.unit().functions,
            function,
            component,
            call,
            &std::collections::BTreeSet::new(),
            &effects,
        )
        .is_some(),
        "crash-custody admission is intact; the member gate is the only refusal"
    );
    assert!(
        !crate::validation::member_blocks::guaranteed_executed_member_blocks(component)
            .contains(&call_block.id),
        "the bypassed member block is outside the non-speculative gate"
    );
    let candidates =
        propose_loop_invariant_scalar_motion(&session, 8).expect("exact relocation candidates");
    assert!(
        candidates
            .iter()
            .flat_map(|candidate| candidate.relocations().iter())
            .all(|relocation| relocation.node().psi_operation() != call_operation),
        "the speculated crash-custody call is not a planned relocation"
    );
}

#[test]
fn impure_crash_callee_fails_crash_continuation_admission() {
    let session = lowered_session(CRASH_CONTINUATION_CALL_SOURCE, "impure crash callee loop");
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
    let effects = crate::validation::invariant_calls::unit_effect_summaries(session.unit());
    assert!(
        crate::validation::invariant_calls::invariant_crash_continuation_call_admission(
            &session.unit().functions,
            function,
            component,
            call,
            &std::collections::BTreeSet::new(),
            &effects,
        )
        .is_some(),
        "the verified summary admits the crash-custody call"
    );
    // Forge each impure axis on the callee's transitive summary in turn: a
    // callee that could actually crash — rather than publishing a ceiling its
    // body never exercises — is `observable`-`May` too, and each forged axis
    // independently refuses the relocation.
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
            crate::validation::invariant_calls::invariant_crash_continuation_call_admission(
                &session.unit().functions,
                function,
                component,
                call,
                &std::collections::BTreeSet::new(),
                &forged,
            )
            .is_none(),
            "callee effect axis {axis} refuses the crash-custody relocation"
        );
    }
}

#[test]
fn observable_member_keeps_the_crash_continuation_call_inside() {
    let session = lowered_session(
        CRASH_CONTINUATION_CALL_SOURCE,
        "observable member crash custody loop",
    );
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
    let effects = crate::validation::invariant_calls::unit_effect_summaries(session.unit());
    // Forge one non-call member node's summary to observable work: hoisting
    // the call's possible non-return ahead of member work reorders an
    // observable boundary, so the member roster gate refuses.
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
        crate::validation::invariant_calls::invariant_crash_continuation_call_admission(
            &session.unit().functions,
            function,
            component,
            call,
            &std::collections::BTreeSet::new(),
            &forged,
        )
        .is_none(),
        "an observable member node refuses the crash-custody relocation"
    );
}

#[test]
fn drifted_crash_roster_stays_inside() {
    let session = lowered_session(CRASH_CONTINUATION_CALL_SOURCE, "drifted crash roster loop");
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
    // Forge the carried roster into the unconditional form: it no longer
    // equals what the callee's contract derives at the seed arguments, so
    // the crash-custody lane refuses — the moved node never inherits a
    // roster the contract did not produce.
    let mut drifted = call.clone();
    let AbstractOperation::Call {
        crash_continuations,
        ..
    } = &mut drifted.operation
    else {
        panic!("the member node is a scalar call")
    };
    crash_continuations[0].alternatives = vec![terminal_psi::CrashRouteGuard::Truth];
    let effects = crate::validation::invariant_calls::unit_effect_summaries(session.unit());
    assert!(
        crate::validation::invariant_calls::admissible_invariant_crash_continuation_call(&drifted)
            .is_some(),
        "the drifted node still spells the crash-custody call shape"
    );
    assert!(
        crate::validation::invariant_calls::invariant_crash_continuation_call_admission(
            &session.unit().functions,
            function,
            component,
            &drifted,
            &std::collections::BTreeSet::new(),
            &effects,
        )
        .is_none(),
        "a roster the contract does not derive refuses the relocation"
    );
}

#[test]
fn forged_moved_crash_roster_is_rejected_by_the_freeze_fence() {
    let session = lowered_session(CRASH_CONTINUATION_CALL_SOURCE, "crash roster forge loop");
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
    let (_, call) = member_call(function, component);
    let call_operation = operation_of(call);
    let candidate = propose_loop_invariant_scalar_motion(&session, 8)
        .expect("exact candidate")
        .pop()
        .expect("one candidate");
    let relocation = candidate
        .relocations()
        .iter()
        .find(|relocation| relocation.node().psi_operation() == call_operation)
        .expect("the crash-custody call is a planned relocation");
    let member = relocation.node().location().block;
    let validated = validate_loop_invariant_scalar_motion(&session, &candidate)
        .expect("validated exact candidate");
    let applied =
        apply_loop_invariant_scalar_motion(session, validated).expect("applied exact candidate");
    let (input, mut unit) = applied.into_session().into_parts();
    // Forge the moved roster's predicates so they no longer spell what the
    // callee's contract derives at the substituted anchor — the freeze
    // replay re-derives the roster and compares byte-exact, so the drifted
    // custody payload rejects.
    let forged = find_operation_mut(&mut unit, call_operation);
    if let AbstractOperation::Call {
        crash_continuations,
        ..
    } = &mut forged.operation
    {
        for continuation in crash_continuations.iter_mut() {
            for guard in continuation.alternatives.iter_mut() {
                let terminal_psi::CrashRouteGuard::Predicate(term) = guard else {
                    continue;
                };
                let proposition = term.proposition().clone();
                *term = terminal_psi::CrashPredicateTerm::new(
                    semantic_vocabulary::Proposition::Conjunction(vec![proposition]),
                );
            }
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
        ) if rejected_machine == machine && block == member
    ));
}
