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
    find_operation_mut, lowered_session, lowered_session_entry, member_call, member_literal,
    member_primitive_local, member_structural_scalar_call, member_structural_scalar_calls,
    operation_of, refresh_coordinates_and_effects, take_operation,
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
            let bumped: u32 in Wrapping = Root::bump(pending as u32 in Wrapping);
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
            let bumped: u64 in Wrapping = Root::bump(pending as u64 in Wrapping);
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

/// A `CallUnit` carrying crash-route custody: `sink(b, s)` inside `step`
/// borrows `b` — the member structural parameter every reaching edge
/// resolves to the machine's `buf` root — and reads `s`, the member scalar
/// parameter resolving to `scale`'s preheader anchor, while `sink`'s
/// verifier-owned contract publishes a conditional `Abort` route over its
/// scalar formal alone. The relocated call rebinds the borrowed root,
/// rebinds `s` to the anchor, and re-derives its `crash_continuations` from
/// the callee's published routes at the substituted arguments rather than
/// carrying the member-spelled roster byte-exact.
const CRASH_CONTINUATION_UNIT_CALL_SOURCE: &str = r#"
    data Root {}
    machine sink(v: &[u8], bound: u32) crashes Abort bound == 0 {}
    machine Root::scan(buf: &[u8], scale: u32, remaining: u32 [0..=5])
    crashes Abort
    {
        transition { _ -> step(buf, scale, remaining) }
        state step(b: &[u8], s: u32, pending: u32 [0..=5]) {
            sink(b, s);
            transition pending > 0 {
                true -> scan(b, s, pending - 1)
                _ -> finish()
            }
        }
        state finish() {}
    }
"#;

/// A `CallStructuralScalar` carrying crash-route custody: `measure(&scratch,
/// scale)` inside the cyclic member borrows the member-produced primitive
/// local `scratch` — whose `7` initializer relocates ahead of it in the same
/// run — and reads `scale`, the member scalar parameter resolving to its
/// preheader anchor, while `measure`'s verifier-owned contract publishes a
/// conditional `Abort` route over its scalar formal alone. The relocated
/// call keeps the borrowed place byte-exact behind its moved producer,
/// rebinds `scale` to the anchor, re-derives its `crash_continuations` at
/// the substituted actual, and preserves its scalar result so `combined`
/// relocates behind it. The cyclic machine keeps the tail shape — named
/// `state` bodies cannot establish primitive storage — while carrying its
/// own `crashes Abort` roster.
const CRASH_CONTINUATION_STRUCTURAL_SCALAR_CALL_SOURCE: &str = r#"
    machine measure(value: &u64, bias: u64) -> u64 crashes Abort bias == 0 { bias }

    machine scan(remaining: u64 [0..=5], scale: u64) -> u64
    crashes Abort
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

/// A `CallStructural` carrying crash-route custody: `pick(b, s)` inside
/// `step` borrows `b` — resolving to `buf` — and reads `s` — resolving to
/// `scale`'s anchor — while `pick`'s verifier-owned contract publishes an
/// unconditional `Abort` route. The roster reads no formal at all, so its
/// substituted spelling coincides with the contract's own — the only roster
/// shape the optimization-unit contract replay admits on a
/// structural-result call — and the relocated call still re-derives it
/// through the crash lane rather than carrying the member-spelled payload
/// byte-exact, keeping the affine result's dispatch custody under the same
/// member-edge rewrite the plain structural call performs.
const CRASH_CONTINUATION_STRUCTURAL_CALL_SOURCE: &str = r#"
    data Root {}
    data Step { case More(rest: u64); case Halt(tag: u64); }

    machine pick(view: &[u8], seed: u64) -> Step crashes Abort { Step::More { rest: view.len } }

    machine Root::scan(scale: u64, buf: &[u8], remaining: u64 [0..=5])
    crashes Abort
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

/// The same `sink(b, s)` crash-custody call, but the scalar argument is the
/// loop-carried countdown: `pending` never resolves to a preheader
/// representative, so the call stays inside even though its callee is pure
/// and the roster would derive at any actuals.
const CARRIED_CRASH_UNIT_CALL_SOURCE: &str = r#"
    data Root {}
    machine sink(v: &[u8], bound: u32) crashes Abort bound == 0 {}
    machine Root::scan(buf: &[u8], remaining: u32 [0..=5])
    crashes Abort
    {
        transition { _ -> step(buf, remaining) }
        state step(b: &[u8], pending: u32 [0..=5]) {
            sink(b, pending);
            transition pending > 0 {
                true -> scan(b, pending - 1)
                _ -> finish()
            }
        }
        state finish() {}
    }
"#;

/// A `CallUnit` whose callee publishes a crash predicate over its owned
/// record parameter's field — `bound == packet.capacity` — so the roster
/// carries a rooted structural term the optimizer-side reconstruction cannot
/// replay: the scalar-only gate refuses, and the call stays inside even
/// though callee purity, member observability, and every argument landing
/// hold. `packet` is `[copy]`, so the `Owned` argument the member spells is
/// the block parameter the `owned_argument` eligibility arm admits.
const STRUCTURAL_PREDICATE_CRASH_UNIT_CALL_SOURCE: &str = r#"
    data Root {}
    data Packet [copy] { capacity: u64; }
    machine sink(packet: Packet, bound: u64) crashes Abort bound == packet.capacity {}
    machine Root::scan(packet: Packet, scale: u64, remaining: u64 [0..=5])
    crashes Abort
    {
        transition { _ -> step(packet, scale, remaining) }
        state step(packet: Packet, s: u64, pending: u64 [0..=5]) {
            sink(packet, s);
            transition pending > 0 {
                true -> scan(packet, s, pending - 1)
                _ -> finish()
            }
        }
        state finish() {}
    }
"#;

/// The same `sink(b, s)` crash-custody call, but the entry state's `done`
/// arm can leave the component before `step` ever runs: relocating the call
/// would perform callee work a bypassed traversal never performs, so the
/// non-speculative gate keeps it inside.
const BYPASSED_CRASH_UNIT_CALL_SOURCE: &str = r#"
    data Root {}
    machine sink(v: &[u8], bound: u32) crashes Abort bound == 0 {}
    machine Root::scan(buf: &[u8], scale: u32, remaining: u32 [0..=5])
    crashes Abort
    {
        transition remaining > 0 {
            true -> step(buf, scale, remaining - 1)
            _ -> done()
        }
        state step(b: &[u8], s: u32, pending: u32 [0..=5]) {
            sink(b, s);
            transition pending > 0 {
                true -> scan(b, s, pending - 1)
                _ -> finish()
            }
        }
        state done() {}
        state finish() {}
    }
"#;

#[test]
fn crash_continuation_unit_call_relocates_rederiving_its_roster() {
    let session = lowered_session(
        CRASH_CONTINUATION_UNIT_CALL_SOURCE,
        "crash custody unit call loop",
    );
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
    let (call_block, call) = member_unit_call(function, component);
    let member = call_block.id;
    let (call_operation, argument, callee, argument_place, seed_continuations) =
        match &call.operation {
            AbstractOperation::CallUnit {
                psi_operation,
                callee,
                arguments,
                structural_arguments,
                crash_continuations,
                ..
            } => {
                assert!(
                    !crash_continuations.is_empty(),
                    "the member call carries crash-route custody"
                );
                let [structural] = structural_arguments.as_slice() else {
                    panic!("one structural argument")
                };
                (
                    *psi_operation,
                    arguments[0],
                    *callee,
                    structural.place,
                    crash_continuations.as_slice(),
                )
            }
            operation => panic!("the member node is a unit call: {operation:?}"),
        };
    assert_eq!(
        crash_roster_references(seed_continuations),
        std::collections::BTreeSet::from([argument]),
        "the seed roster predicates read the member-parameter argument"
    );
    let anchor = crate::validation::member_blocks::invariant_member_parameters(function, component)
        [&argument];
    let representative = crate::validation::place_observations::invariant_member_place_parameters(
        function,
        component,
        &std::collections::BTreeSet::new(),
    )[&argument_place];
    assert_ne!(representative, argument_place);

    // The lane split is exact: the empty-roster lane refuses the
    // custody-carrying node, and the crash lane admits it under the shared
    // scalar substitution and borrow-root rewrites.
    let effects = crate::validation::invariant_calls::unit_effect_summaries(session.unit());
    assert!(
        crate::validation::invariant_calls::invariant_unit_call_admission(
            function,
            component,
            call,
            &std::collections::BTreeSet::new(),
            &std::collections::BTreeSet::new(),
            &effects,
        )
        .is_none(),
        "the empty-roster lane keeps refusing the crash-custody call"
    );
    let (substitution, rewrites) =
        crate::validation::invariant_calls::invariant_crash_continuation_unit_call_admission(
            &session.unit().functions,
            function,
            component,
            call,
            &std::collections::BTreeSet::new(),
            &std::collections::BTreeSet::new(),
            &effects,
        )
        .expect("the crash-custody lane admits the unit call");
    assert_eq!(substitution.get(&argument), Some(&anchor));
    assert_eq!(
        rewrites.as_slice(),
        &[(argument_place, representative)],
        "the borrowed root rebinds to its preheader-visible representative"
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
        .expect("the crash-custody unit call is a planned relocation");
    assert!(
        matches!(relocation.node().result(), LoopInvariantNodeResult::Unit),
        "the unit call relocation preserves the invocation"
    );
    assert_eq!(
        relocation.node().operand_rewrites(),
        &[(argument, anchor)],
        "the call's member-parameter argument rebinds to its preheader anchor"
    );
    assert_eq!(
        relocation.node().argument_rewrites(),
        &[(argument_place, representative)],
        "the call's borrowed root rebinds to its preheader-visible representative"
    );
    assert_eq!(relocation.node().location().block, member);
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
    let moved_continuations = match &moved.operation {
        AbstractOperation::CallUnit {
            callee: moved_callee,
            arguments,
            structural_arguments,
            crash_continuations,
            ..
        } => {
            assert_eq!(arguments.as_slice(), &[anchor]);
            assert_eq!(*moved_callee, callee, "callee identity is byte-exact");
            let [structural] = structural_arguments.as_slice() else {
                panic!("one structural argument")
            };
            assert_eq!(
                structural.place, representative,
                "the relocated call borrows the representative root"
            );
            crash_continuations
        }
        operation => panic!("relocated node keeps its unit-call operation: {operation:?}"),
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
        crate::validation::relocation_rewrites::structural_call_crash_continuations(
            callee_function,
            &[anchor],
        )
        .expect("the callee contract re-derives the roster")
        .as_slice(),
        "the moved roster is the callee contract instantiated at the anchor"
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
    assert!(
        propose_loop_invariant_scalar_motion(applied.session(), 8)
            .expect("relocated session is an exact fixed point")
            .is_empty()
    );
}

#[test]
fn crash_continuation_structural_scalar_call_relocates_rederiving_its_roster() {
    let session = lowered_session_entry(
        CRASH_CONTINUATION_STRUCTURAL_SCALAR_CALL_SOURCE,
        "crash custody structural-scalar call loop",
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
    let (_, local) = member_primitive_local(function, component);
    let (local_operation, local_place) = match &local.operation {
        AbstractOperation::EstablishPrimitiveLocal {
            psi_operation,
            result,
            ..
        } => (*psi_operation, result.place),
        operation => panic!("the member node is a primitive-local establishment: {operation:?}"),
    };
    let (call_block, call) = member_structural_scalar_call(function, component);
    let member = call_block.id;
    let (call_operation, argument, callee, borrowed_place, result, seed_continuations) =
        match &call.operation {
            AbstractOperation::CallStructuralScalar {
                psi_operation,
                result,
                callee,
                arguments,
                structural_arguments,
                crash_continuations,
                ..
            } => {
                assert!(
                    !crash_continuations.is_empty(),
                    "the member call carries crash-route custody"
                );
                let [structural] = structural_arguments.as_slice() else {
                    panic!("one structural argument")
                };
                assert_eq!(
                    structural.access,
                    terminal_psi::StructuralAccess::SharedBorrow,
                    "the call shares the member-produced local"
                );
                (
                    *psi_operation,
                    arguments[0],
                    *callee,
                    structural.place,
                    result.value,
                    crash_continuations.as_slice(),
                )
            }
            operation => panic!("the member node is a structural-scalar call: {operation:?}"),
        };
    assert_eq!(
        borrowed_place, local_place,
        "the call's shared borrow names the local's declared place"
    );
    assert_eq!(
        crash_roster_references(seed_continuations),
        std::collections::BTreeSet::from([argument]),
        "the seed roster predicates read the member-parameter argument"
    );
    let anchor = crate::validation::member_blocks::invariant_member_parameters(function, component)
        [&argument];

    // The empty-roster lane refuses the custody-carrying node outright; the
    // crash lane admits it once the same run covers the borrowed root's
    // producer, and the moved local keeps its declared place byte-exact so
    // the call needs no structural rewrite at all.
    let effects = crate::validation::invariant_calls::unit_effect_summaries(session.unit());
    assert!(
        crate::validation::invariant_calls::invariant_structural_scalar_call_admission(
            function,
            component,
            call,
            &std::collections::BTreeSet::new(),
            &std::collections::BTreeSet::new(),
            &effects,
        )
        .is_none(),
        "the empty-roster lane keeps refusing the crash-custody call"
    );
    assert!(
        crate::validation::invariant_calls::invariant_crash_continuation_structural_scalar_call_admission(
            &session.unit().functions,
            function,
            component,
            call,
            &std::collections::BTreeSet::new(),
            &std::collections::BTreeSet::new(),
            &effects,
        )
        .is_none(),
        "the borrowed root stays inside until its producer relocates in the same run"
    );
    let (substitution, rewrites) =
        crate::validation::invariant_calls::invariant_crash_continuation_structural_scalar_call_admission(
            &session.unit().functions,
            function,
            component,
            call,
            &std::collections::BTreeSet::new(),
            &std::collections::BTreeSet::from([local_place]),
            &effects,
        )
        .expect("the crash-custody lane admits the structural-scalar call");
    assert_eq!(substitution.get(&argument), Some(&anchor));
    assert!(
        rewrites.is_empty(),
        "the moved establishment preserves the borrowed place identity byte-exact"
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
        .expect("the primitive-local establishment is a planned relocation");
    let relocation = candidate
        .relocations()
        .iter()
        .find(|relocation| relocation.node().psi_operation() == call_operation)
        .expect("the crash-custody structural-scalar call is a planned relocation");
    let LoopInvariantNodeResult::Scalar { value, .. } = relocation.node().result() else {
        panic!("the call relocates its preserved scalar result")
    };
    assert_eq!(
        *value, result,
        "the preserved result is the call's declared scalar"
    );
    assert_eq!(
        relocation.node().operand_rewrites(),
        &[(argument, anchor)],
        "the call's member-parameter argument rebinds to its preheader anchor"
    );
    assert!(
        relocation.node().argument_rewrites().is_empty(),
        "the borrowed root relocates with its producer, keeping the place byte-exact"
    );
    assert_eq!(relocation.node().location().block, member);
    assert_eq!(relocation.destination().block, entry.source);
    assert!(
        local_relocation.destination().node < relocation.destination().node,
        "the relocated establishment lands ahead of the call borrowing its root"
    );

    // The `measured & s` computation consumes the call's preserved result,
    // so it relocates behind the call in the same run.
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
    let moved_continuations = match &moved.operation {
        AbstractOperation::CallStructuralScalar {
            callee: moved_callee,
            arguments,
            structural_arguments,
            crash_continuations,
            ..
        } => {
            assert_eq!(arguments.as_slice(), &[anchor]);
            assert_eq!(*moved_callee, callee, "callee identity is byte-exact");
            let [structural] = structural_arguments.as_slice() else {
                panic!("one structural argument")
            };
            assert_eq!(
                structural.place, local_place,
                "the relocated call still borrows the moved local's declared place"
            );
            crash_continuations
        }
        operation => {
            panic!("relocated node keeps its structural-scalar operation: {operation:?}")
        }
    };
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
        crate::validation::relocation_rewrites::structural_call_crash_continuations(
            callee_function,
            &[anchor],
        )
        .expect("the callee contract re-derives the roster")
        .as_slice(),
        "the moved roster is the callee contract instantiated at the anchor"
    );
    assert_eq!(moved.provenance, relocation.node().provenance());
    assert_eq!(moved.fuel, relocation.node().fuel());
    let member_block = output_function
        .blocks
        .iter()
        .find(|block| block.id == member)
        .expect("member block exists");
    assert!(
        member_block.nodes.iter().all(|node| !matches!(
            node.operation,
            AbstractOperation::CallStructuralScalar { .. }
        )),
        "the structural-scalar call exists once, at the destination"
    );
    assert!(
        propose_loop_invariant_scalar_motion(applied.session(), 8)
            .expect("relocated session is an exact fixed point")
            .is_empty()
    );
}

#[test]
fn crash_continuation_structural_call_relocates_rederiving_its_roster() {
    let session = lowered_session_entry(
        CRASH_CONTINUATION_STRUCTURAL_CALL_SOURCE,
        "crash custody structural call loop",
        "Root::scan",
    );
    let [component] = session.cycle_components().components() else {
        panic!("one cyclic component")
    };
    let [entry] = component.entries.as_slice() else {
        panic!("one entry edge")
    };
    let member_targets: std::collections::BTreeSet<_> = component.members.iter().copied().collect();
    let machine = component.id.machine;
    let function = session
        .unit()
        .functions
        .iter()
        .find(|function| function.machine == machine)
        .expect("component machine exists");
    let calls = member_structural_calls(function, component);
    let [(call_block, call)] = calls.as_slice() else {
        panic!("one member structural call")
    };
    let member = call_block.id;
    let (call_operation, picked, argument, callee, argument_place, seed_continuations) =
        match &call.operation {
            AbstractOperation::CallStructural {
                psi_operation,
                result,
                callee,
                arguments,
                structural_arguments,
                crash_continuations,
                ..
            } => {
                assert!(
                    !crash_continuations.is_empty(),
                    "the member call carries crash-route custody"
                );
                assert_eq!(
                    result.multiplicity,
                    terminal_psi::StructuralMultiplicity::Affine,
                    "the result is the confined affine sum"
                );
                let [structural] = structural_arguments.as_slice() else {
                    panic!("one structural argument")
                };
                (
                    *psi_operation,
                    result.place,
                    arguments[0],
                    *callee,
                    structural.place,
                    crash_continuations.as_slice(),
                )
            }
            operation => panic!("the member node is a structural call: {operation:?}"),
        };
    assert_eq!(
        crash_roster_references(seed_continuations),
        std::collections::BTreeSet::new(),
        "the unconditional route carries no predicate payload"
    );
    let anchor = crate::validation::member_blocks::invariant_member_parameters(function, component)
        [&argument];
    let representative = crate::validation::place_observations::invariant_member_place_parameters(
        function,
        component,
        &std::collections::BTreeSet::new(),
    )[&argument_place];
    assert_ne!(representative, argument_place);

    let effects = crate::validation::invariant_calls::unit_effect_summaries(session.unit());
    assert!(
        crate::validation::invariant_calls::invariant_structural_call_admission(
            function,
            component,
            call,
            &std::collections::BTreeSet::new(),
            &std::collections::BTreeSet::new(),
            &effects,
        )
        .is_none(),
        "the empty-roster lane keeps refusing the crash-custody call"
    );
    let (substitution, rewrites) =
        crate::validation::invariant_calls::invariant_crash_continuation_structural_call_admission(
            &session.unit().functions,
            function,
            component,
            call,
            &std::collections::BTreeSet::new(),
            &std::collections::BTreeSet::new(),
            &effects,
        )
        .expect("the crash-custody lane admits the structural call");
    assert_eq!(substitution.get(&argument), Some(&anchor));
    assert_eq!(
        rewrites.as_slice(),
        &[(argument_place, representative)],
        "the borrowed root rebinds to its preheader-visible representative"
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
        .expect("the crash-custody structural call is a planned relocation");
    let LoopInvariantNodeResult::Structural(result) = relocation.node().result() else {
        panic!("the structural call relocates its structural result")
    };
    assert_eq!(result.place, picked, "the declared place is byte-exact");
    assert_eq!(
        relocation.node().operand_rewrites(),
        &[(argument, anchor)],
        "the call's member-parameter argument rebinds to its preheader anchor"
    );
    assert_eq!(
        relocation.node().argument_rewrites(),
        &[(argument_place, representative)],
        "the call's borrowed root rebinds to its preheader-visible representative"
    );
    assert_eq!(relocation.node().location().block, member);
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
    let moved_continuations = match &moved.operation {
        AbstractOperation::CallStructural {
            result,
            callee: moved_callee,
            arguments,
            structural_arguments,
            crash_continuations,
            ..
        } => {
            assert_eq!(result.place, picked, "the declared place is byte-exact");
            assert_eq!(arguments.as_slice(), &[anchor]);
            assert_eq!(*moved_callee, callee, "callee identity is byte-exact");
            let [structural] = structural_arguments.as_slice() else {
                panic!("one structural argument")
            };
            assert_eq!(
                structural.place, representative,
                "the relocated call borrows the representative root"
            );
            crash_continuations
        }
        operation => panic!("relocated node keeps its structural call operation: {operation:?}"),
    };
    assert_eq!(
        crash_roster_references(moved_continuations),
        std::collections::BTreeSet::new(),
        "the unconditional route carries no predicate payload"
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
        crate::validation::relocation_rewrites::structural_call_crash_continuations(
            callee_function,
            &[anchor],
        )
        .expect("the callee contract re-derives the roster")
        .as_slice(),
        "the moved roster is the callee contract instantiated at the anchor"
    );
    // The affine result's dispatch custody re-expresses exactly as the plain
    // lane's: member-internal edges keep the persistent place live while
    // every member exit edge disposes it.
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
fn carried_argument_crash_unit_call_stays_inside() {
    let session = lowered_session(
        CARRIED_CRASH_UNIT_CALL_SOURCE,
        "carried crash custody unit call loop",
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
    let (_, call) = member_unit_call(function, component);
    let (call_operation, argument) = match &call.operation {
        AbstractOperation::CallUnit {
            psi_operation,
            arguments,
            ..
        } => (*psi_operation, arguments[0]),
        operation => panic!("the member node is a unit call: {operation:?}"),
    };
    assert!(
        !crate::validation::member_blocks::invariant_member_parameters(function, component)
            .contains_key(&argument),
        "the back edge advances the call's argument, so it stays loop-carried"
    );
    let effects = crate::validation::invariant_calls::unit_effect_summaries(session.unit());
    assert!(
        crate::validation::invariant_calls::invariant_crash_continuation_unit_call_admission(
            &session.unit().functions,
            function,
            component,
            call,
            &std::collections::BTreeSet::new(),
            &std::collections::BTreeSet::new(),
            &effects,
        )
        .is_none(),
        "the carried-argument crash call fails admission at the substitution half"
    );
    let candidates =
        propose_loop_invariant_scalar_motion(&session, 8).expect("exact relocation candidates");
    assert!(
        candidates
            .iter()
            .flat_map(|candidate| candidate.relocations().iter())
            .all(|relocation| relocation.node().psi_operation() != call_operation),
        "the carried-argument crash unit call is not a planned relocation"
    );
}

#[test]
fn drifted_structural_crash_roster_stays_inside() {
    let session = lowered_session(
        CRASH_CONTINUATION_UNIT_CALL_SOURCE,
        "drifted structural crash roster loop",
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
    let (_, call) = member_unit_call(function, component);
    // Forge the carried roster into the unconditional form: it no longer
    // equals what the callee's contract derives at the seed arguments, so
    // the crash-custody lane refuses — the moved node never inherits a
    // roster the contract did not produce.
    let mut drifted = call.clone();
    let AbstractOperation::CallUnit {
        crash_continuations,
        ..
    } = &mut drifted.operation
    else {
        panic!("the member node is a unit call")
    };
    crash_continuations[0].alternatives = vec![terminal_psi::CrashRouteGuard::Truth];
    let effects = crate::validation::invariant_calls::unit_effect_summaries(session.unit());
    assert!(
        crate::validation::invariant_calls::admissible_invariant_crash_continuation_unit_call(
            &drifted
        )
        .is_some(),
        "the drifted node still spells the crash-custody call shape"
    );
    assert!(
        crate::validation::invariant_calls::invariant_crash_continuation_unit_call_admission(
            &session.unit().functions,
            function,
            component,
            &drifted,
            &std::collections::BTreeSet::new(),
            &std::collections::BTreeSet::new(),
            &effects,
        )
        .is_none(),
        "a roster the contract does not derive refuses the relocation"
    );
}

#[test]
fn structural_predicate_crash_roster_stays_inside() {
    let session = lowered_session(
        STRUCTURAL_PREDICATE_CRASH_UNIT_CALL_SOURCE,
        "structural predicate crash custody loop",
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
    let (_, call) = member_unit_call(function, component);
    let call_operation = operation_of(call);
    // The seed roster spells a rooted structural term — the callee's
    // `packet.capacity` field — so the scalar-only gate refuses even though
    // every other admission half is intact.
    let AbstractOperation::CallUnit {
        crash_continuations,
        ..
    } = &call.operation
    else {
        panic!("the member node is a unit call")
    };
    assert!(
        !crash_continuations.is_empty(),
        "the member call carries crash-route custody"
    );
    let effects = crate::validation::invariant_calls::unit_effect_summaries(session.unit());
    assert!(
        crate::validation::invariant_calls::admissible_invariant_crash_continuation_unit_call(call)
            .is_some(),
        "the node spells the crash-custody call shape"
    );
    assert!(
        crate::validation::invariant_calls::invariant_crash_continuation_unit_call_admission(
            &session.unit().functions,
            function,
            component,
            call,
            &std::collections::BTreeSet::new(),
            &std::collections::BTreeSet::new(),
            &effects,
        )
        .is_none(),
        "a roster reading a callee structural parameter refuses the relocation"
    );
    let candidates =
        propose_loop_invariant_scalar_motion(&session, 8).expect("exact relocation candidates");
    assert!(
        candidates
            .iter()
            .flat_map(|candidate| candidate.relocations().iter())
            .all(|relocation| relocation.node().psi_operation() != call_operation),
        "the structural-predicate crash call is not a planned relocation"
    );
}

#[test]
fn bypassed_crash_unit_call_is_speculation_and_stays_inside() {
    let session = lowered_session(
        BYPASSED_CRASH_UNIT_CALL_SOURCE,
        "bypassed crash custody unit call loop",
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
    let (call_block, call) = member_unit_call(function, component);
    let call_operation = operation_of(call);
    // Crash-lane admission itself is intact — the refusal is the
    // non-speculative gate alone: the call's member block does not dominate
    // the entry state's own `done` exit.
    let effects = crate::validation::invariant_calls::unit_effect_summaries(session.unit());
    assert!(
        crate::validation::invariant_calls::invariant_crash_continuation_unit_call_admission(
            &session.unit().functions,
            function,
            component,
            call,
            &std::collections::BTreeSet::new(),
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
        "the speculated crash-custody unit call is not a planned relocation"
    );
}
