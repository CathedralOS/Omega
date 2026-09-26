//! The terminator of one state in a composed Unit control graph.
//!
//! Tried in order: a closed-sum dispatch over the state's final case; a
//! scalar return's checked completion; then the state's authored tail --
//! a Unit or structural return, a crash exit, a jump, conditional or guarded
//! successors, or a tail call. Every successor edge is built by `successor`
//! against the state signatures; an unsupported tail names its shape.
use super::{
    CheckFacts, CheckedComposedUnitControlTerminatorPlan, CheckedGuardedJumpPlan,
    CheckedScalarExpressionRole, CheckedStructuralControlSuccessorPlan,
    CheckedUnitEffectOperationPlan, ScalarCalleePlans, ShapeCollector, Signature, StatementNode,
    SuccessorEdge, SuccessorGuard, TransitionExit, TransitionGuardNode, TransitionTargetNode,
    TypedTrees, closed_sum, control, guard_is_boolean, retained_guard, return_cleanup_is_whole,
    returns, successor,
};

/// The terminator one state's authored tail plans, after its operations.
#[allow(clippy::too_many_arguments)]
pub(super) fn build(
    program: &TypedTrees,
    facts: &CheckFacts,
    scalar_callees: ScalarCalleePlans<'_>,
    shapes: &mut ShapeCollector<'_>,
    machine: &symbol_resolved_trees_to_typed_trees::typed_trees::machine::Machine,
    state_index: usize,
    state: &symbol_resolved_trees_to_typed_trees::typed_trees::state::State,
    result: &crate::checked_trees::CheckedControlResultPlan,
    signatures: &[Signature],
    entry_claims: &[crate::checked_trees::CheckedUnitEntryClaimPlan],
    scalar_result: Option<&crate::checked_trees::CheckedUnitScalarResultBindingPlan>,
    structural_result: Option<&crate::checked_trees::CheckedUnitStructuralReturnPlan>,
    operations: &mut Vec<CheckedUnitEffectOperationPlan>,
    terminator_index: usize,
    trace: &control::LocalConstructionTrace,
) -> Option<CheckedComposedUnitControlTerminatorPlan> {
    let result = result.clone();
    let (structural, _) = &signatures[state_index];
    let statements = program.statement_table.statements(state.statement_nodes);
    trace.phase("state graph: terminator");
    let ordinal = u32::try_from(terminator_index).ok()?;
    let edge = |operations: &[CheckedUnitEffectOperationPlan], transition, edge_ordinal, kind| {
        successor(
            program,
            facts,
            machine,
            state_index,
            signatures,
            operations,
            transition,
            edge_ordinal,
            kind,
            trace,
        )
    };
    let terminator = if let Some(terminator) = closed_sum::build(
        program,
        facts,
        machine,
        state_index,
        signatures,
        operations,
        terminator_index,
        trace,
    ) {
        terminator
    } else if let crate::checked_trees::CheckedControlResultPlan::Scalar { primitive_type } = result
        && let Some(completion) = returns::scalar_completion(
            program,
            facts,
            machine,
            state,
            terminator_index,
            primitive_type,
            scalar_result,
            trace,
        )
    {
        trace.phase("state graph: terminator: scalar return cleanup");
        return_cleanup_is_whole(
            program, facts, machine, state, structural, operations, shapes,
        )?;
        CheckedComposedUnitControlTerminatorPlan::ReturnScalar { completion }
    } else if let Some((terminator, operand_calls)) = returns::guarded(
        program,
        facts,
        scalar_callees,
        shapes,
        machine,
        state,
        structural,
        entry_claims,
        operations,
        terminator_index,
        trace,
    ) {
        operations.extend(operand_calls);
        terminator
    } else {
        match &statements[terminator_index..] {
            [] if result == crate::checked_trees::CheckedControlResultPlan::Unit => {
                trace.phase("state graph: terminator: unit tail cleanup");
                return_cleanup_is_whole(
                    program, facts, machine, state, structural, operations, shapes,
                )?;
                CheckedComposedUnitControlTerminatorPlan::ReturnUnit
            }
            [StatementNode::Expression(expression)]
                if result != crate::checked_trees::CheckedControlResultPlan::Unit =>
            {
                trace.phase("state graph: terminator: return expression");
                if let Some(result) = structural_result.cloned() {
                    CheckedComposedUnitControlTerminatorPlan::ReturnStructural { result }
                } else {
                    CheckedComposedUnitControlTerminatorPlan::ReturnCase {
                        result: returns::constructor(program, facts, state, ordinal, *expression)?,
                    }
                }
            }
            [StatementNode::Transition(transition)]
                if matches!(transition.exit, TransitionExit::Crash(_)) =>
            {
                trace.phase("state graph: terminator: crash exit");
                CheckedComposedUnitControlTerminatorPlan::Crash {
                    statement_ordinal: ordinal,
                }
            }
            // Typing already lowered a run-closing constant-true arm
            // (`transition true { true -> done(v) }`) to `Always`.
            [StatementNode::Transition(transition)]
                if transition.guard == TransitionGuardNode::Always =>
            {
                trace.phase("state graph: terminator: jump successor");
                CheckedComposedUnitControlTerminatorPlan::Jump {
                    successor: edge(operations, transition, ordinal, SuccessorEdge::Jump)?,
                }
            }
            [
                StatementNode::Transition(when_true),
                StatementNode::Transition(when_false),
            ] if matches!(when_true.guard, TransitionGuardNode::When(_))
                && (when_false.guard == TransitionGuardNode::Always
                    || crate::execution::guard_complement::complementary(
                        program,
                        &facts.values.scalar_expressions,
                        state,
                        ordinal,
                    )) =>
            {
                trace.phase("state graph: terminator: conditional successors: guard expression");
                let guard = retained_guard(facts, machine.symbol, state.symbol, ordinal)?;
                if !guard_is_boolean(facts, &guard) {
                    trace.phase("state graph: terminator: conditional successors: guard type");
                    return None;
                }
                // An authored `(expression)` arm returns the established
                // value instead of transferring to a named state; a named
                // arm keeps the ordinary successor custody plan.
                let mut return_count = returns::next_result_ordinal(operations)?;
                let mut branch = |transition: &symbol_resolved_trees_to_typed_trees::typed_trees::statement::TableTransition,
                                  edge_ordinal: u32|
                 -> Option<
                    Result<
                        CheckedStructuralControlSuccessorPlan,
                        (
                            crate::checked_trees::CheckedConditionalReturnArm,
                            Vec<CheckedUnitEffectOperationPlan>,
                        ),
                    >,
                > {
                    if let TransitionTargetNode::Value(expression) =
                        program.statement_table.transition_target(transition.target)
                    {
                        if transition.exit != TransitionExit::Ordinary
                            || transition.continuation.is_valid()
                        {
                            trace.phase(
                                SuccessorEdge::Conditional.phase(SuccessorGuard::TargetState),
                            );
                            return None;
                        }
                        trace.phase(
                            "state graph: terminator: conditional successors: return arm value",
                        );
                        trace.statement(Some(edge_ordinal));
                        // A scalar result is the value checking retained
                        // under the arm's `Return` role; the arm alone
                        // evaluates it.
                        if let crate::checked_trees::CheckedControlResultPlan::Scalar { primitive_type } =
                            result
                        {
                            let role = CheckedScalarExpressionRole::Return;
                            let retained = facts
                                .values
                                .scalar_expressions
                                .expression_at(state.symbol, edge_ordinal, role)
                                .is_some()
                                || facts
                                    .values
                                    .scalar_computations
                                    .root_at(state.symbol, edge_ordinal, role)
                                    .is_some_and(|root| root.machine == machine.symbol);
                            return retained.then_some(Err((
                                crate::checked_trees::CheckedConditionalReturnArm::Scalar {
                                    statement_ordinal: edge_ordinal,
                                    primitive_type,
                                },
                                Vec::new(),
                            )));
                        }
                        return returns::return_value_operation(
                            program,
                            facts,
                            scalar_callees,
                            shapes,
                            machine,
                            state,
                            structural,
                            entry_claims,
                            &mut return_count,
                            edge_ordinal,
                            *expression,
                            trace,
                        )
                        .map(|(operation, operand_calls)| {
                            Err((
                                crate::checked_trees::CheckedConditionalReturnArm::Structural(operation),
                                operand_calls,
                            ))
                        });
                    }
                    successor(
                        program,
                        facts,
                        machine,
                        state_index,
                        signatures,
                        operations,
                        transition,
                        edge_ordinal,
                        SuccessorEdge::Conditional,
                        trace,
                    )
                    .map(Ok)
                };
                match (
                    branch(when_true, ordinal)?,
                    branch(when_false, ordinal.checked_add(1)?)?,
                ) {
                    (Ok(when_true), Ok(when_false)) => {
                        CheckedComposedUnitControlTerminatorPlan::Conditional {
                            guard,
                            when_true,
                            when_false,
                        }
                    }
                    (Ok(jump), Err((return_arm, operand_calls))) => {
                        operations.extend(operand_calls);
                        CheckedComposedUnitControlTerminatorPlan::ConditionalReturn {
                            guard,
                            jump,
                            return_arm,
                            return_when_true: false,
                        }
                    }
                    (Err((return_arm, operand_calls)), Ok(jump)) => {
                        operations.extend(operand_calls);
                        CheckedComposedUnitControlTerminatorPlan::ConditionalReturn {
                            guard,
                            jump,
                            return_arm,
                            return_when_true: true,
                        }
                    }
                    // Both arms `(expression)` targets check as Guarded.
                    (Err(_), Err(_)) => return None,
                }
            }
            tail @ [
                StatementNode::Transition(_),
                StatementNode::Transition(_),
                StatementNode::Transition(_),
                ..,
            ] if tail
                .iter()
                .all(|statement| matches!(statement, StatementNode::Transition(_)))
                && tail[..tail.len() - 1].iter().all(|statement| {
                    matches!(statement, StatementNode::Transition(transition)
                        if matches!(transition.guard, TransitionGuardNode::When(_)))
                })
                && matches!(tail.last(), Some(StatementNode::Transition(transition))
                    if transition.guard == TransitionGuardNode::Always) =>
            {
                trace.phase("state graph: terminator: guarded jump successors: roster");
                let retained = facts
                    .flow
                    .terminal_scalar_graphs
                    .guarded_tails
                    .iter()
                    .filter(|tail| tail.state == state.symbol)
                    .collect::<Vec<_>>();
                let [retained] = retained.as_slice() else {
                    return None;
                };
                let exits = facts
                    .flow
                    .terminal_scalar_graphs
                    .guarded_exits
                    .span(retained.arms)?;
                // The shared scalar roster owns this tail's guard order,
                // coverage and selected destinations; the composed edges
                // must agree with it exactly.
                if exits.len() != tail.len() - 1 {
                    return None;
                }
                let mut arms = Vec::with_capacity(exits.len());
                for (index, (statement, exit)) in
                    tail[..tail.len() - 1].iter().zip(exits.iter()).enumerate()
                {
                    let StatementNode::Transition(transition) = statement else {
                        return None;
                    };
                    let arm_ordinal = ordinal.checked_add(u32::try_from(index).ok()?)?;
                    let crate::checked_trees::CheckedScalarBranchDestination::Jump(selected) =
                        &exit.destination
                    else {
                        return None;
                    };
                    if exit.guard_statement_ordinal != arm_ordinal
                        || selected.statement_ordinal != arm_ordinal
                    {
                        return None;
                    }
                    trace.phase(
                        "state graph: terminator: guarded jump successors: guard expression",
                    );
                    trace.statement(Some(arm_ordinal));
                    let guard = retained_guard(facts, machine.symbol, state.symbol, arm_ordinal)?;
                    if !guard_is_boolean(facts, &guard) {
                        return None;
                    }
                    let successor = edge(
                        operations,
                        transition,
                        arm_ordinal,
                        SuccessorEdge::GuardedJump,
                    )?;
                    if successor.target_state != selected.target {
                        return None;
                    }
                    arms.push(CheckedGuardedJumpPlan { guard, successor });
                }
                let Some(StatementNode::Transition(fallback_transition)) = tail.last() else {
                    return None;
                };
                let fallback_ordinal = ordinal.checked_add(u32::try_from(exits.len()).ok()?)?;
                let Some(crate::checked_trees::CheckedScalarBranchDestination::Jump(selected)) =
                    &retained.fallback
                else {
                    return None;
                };
                if selected.statement_ordinal != fallback_ordinal {
                    return None;
                }
                let fallback = edge(
                    operations,
                    fallback_transition,
                    fallback_ordinal,
                    SuccessorEdge::GuardedJump,
                )?;
                if fallback.target_state != selected.target {
                    return None;
                }
                CheckedComposedUnitControlTerminatorPlan::GuardedJumps { arms, fallback }
            }
            _ => {
                // Name the tail shape the general route lacks: the arms
                // above admit an empty unit tail, one return expression,
                // one unconditional jump, an exact when/else pair, and an
                // ordered guarded chain ending in its authored `_` arm.
                trace.phase(match &statements[terminator_index..] {
                    [] => "state graph: terminator: unsupported tail: missing return value",
                    [StatementNode::Expression(_)] => {
                        "state graph: terminator: unsupported tail: expression statement with unit result"
                    }
                    [StatementNode::Transition(transition)] => match transition.exit {
                        TransitionExit::Ordinary => {
                            "state graph: terminator: unsupported tail: single guarded transition"
                        }
                        TransitionExit::Crash(_) => {
                            "state graph: terminator: unsupported tail: single guarded crash exit"
                        }
                    },
                    [StatementNode::Transition(first), StatementNode::Transition(_)]
                        if first.guard == TransitionGuardNode::Always =>
                    {
                        "state graph: terminator: unsupported tail: jump followed by a transition"
                    }
                    [StatementNode::Transition(_), StatementNode::Transition(_)] => {
                        "state graph: terminator: unsupported tail: guarded pair without exact false fallback"
                    }
                    tail @ [
                        StatementNode::Transition(_),
                        StatementNode::Transition(_),
                        StatementNode::Transition(_),
                        ..,
                    ] if tail
                        .iter()
                        .all(|statement| matches!(statement, StatementNode::Transition(_))) =>
                    {
                        "state graph: terminator: unsupported tail: transition chain"
                    }
                    [StatementNode::Transition(_), ..] => {
                        "state graph: terminator: unsupported tail: transition followed by statements"
                    }
                    [StatementNode::Expression(_), ..] => {
                        "state graph: terminator: unsupported tail: expression followed by statements"
                    }
                    _ => "state graph: terminator: unsupported tail: other statement tail",
                });
                return None;
            }
        }
    };
    Some(terminator)
}
