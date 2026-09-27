//! Loop-invariant scalar computations: member computations reaching their
//! anchor through transitive member parameters, obligated computations, the
//! non-speculative gate over bypassed and conditional-entry members, and
//! multi-entry preheaders.

use crate::VerifiedPsiOptimizationSession;
use crate::{
    apply_loop_invariant_scalar_motion, propose_loop_invariant_scalar_motion,
    validate_loop_invariant_scalar_motion,
};
use abstract_operations::AbstractOperation;
use optimization_unit::{PsiProvenance, recompute_psi_optimization_unit_identity};
use optimization_unit_semantics::OptimizationUnitValidationError;
use semantic_vocabulary::ValueId;

use super::{
    find_operation_mut, lowered_session, lowered_session_entry, member_addition, operation_of,
    refresh_coordinates_and_effects, take_operation,
};

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
        crate::validation::member_blocks::shared_entry_source(component),
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
        !crate::validation::member_blocks::invariant_member_parameters(function, component)
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
    let preheader = crate::validation::member_blocks::shared_entry_source(component)
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
    let anchor = crate::validation::member_blocks::invariant_member_parameters(function, component)
        [&member_parameter];
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
