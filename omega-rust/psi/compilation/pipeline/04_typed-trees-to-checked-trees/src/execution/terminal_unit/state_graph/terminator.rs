//! The terminator of one state in a composed Unit control graph.
//!
//! Tried in order: a closed-sum dispatch over the state's final case; a
//! scalar return's checked completion; then the state's authored tail --
//! a Unit or structural return, a crash exit, a jump, conditional or guarded
//! successors, or a terminal machine call. Every successor edge is built by
//! `successor` against the state signatures; a terminal call completes the
//! machine instead (`terminal_calls`); an unsupported tail names its shape.
use super::{
    CheckFacts, CheckedComposedUnitControlTerminatorPlan, CheckedGuardedJumpPlan,
    CheckedScalarExpressionRole, CheckedStructuralControlSuccessorPlan,
    CheckedUnitEffectOperationPlan, CheckedUnitStructuralParameterPlan, EdgeCalls, FusedStates,
    ScalarCalleePlans, ShapeCollector, Signature, StatementNode, SuccessorEdge, SuccessorGuard,
    TransitionExit, TransitionGuardNode, TransitionTargetNode, TypedTrees, closed_sum, control,
    guard_is_boolean, retained_guard, return_cleanup_is_whole, returns, successor, terminal_calls,
};

/// The terminator one state's authored tail plans, after its operations.
/// `fused` carries the whole roster this graph spans: a successor naming a
/// foreign machine's entry resolves to that owner's fused states, so the
/// edge stays inside the one dispatch loop.
#[allow(clippy::too_many_arguments)]
pub(super) fn build(
    program: &TypedTrees,
    facts: &CheckFacts,
    scalar_callees: ScalarCalleePlans<'_>,
    shapes: &mut ShapeCollector<'_>,
    fused: &FusedStates<'_>,
    state_index: usize,
    state: &typed_trees::state::State,
    result: &checked_trees::CheckedControlResultPlan,
    signatures: &[Signature],
    entry_claims: &[checked_trees::CheckedUnitEntryClaimPlan],
    local_results: &[(
        checked_trees::CheckedUnitStructuralResultBindingPlan,
        facts::PlaceRoot,
    )],
    scalar_result: Option<&checked_trees::CheckedUnitScalarResultBindingPlan>,
    structural_result: Option<&checked_trees::CheckedUnitStructuralReturnPlan>,
    next_scalar_binding: usize,
    operations: &mut Vec<CheckedUnitEffectOperationPlan>,
    terminator_index: usize,
    trace: &control::LocalConstructionTrace,
) -> Option<CheckedComposedUnitControlTerminatorPlan> {
    let result = result.clone();
    // The state's own owner answers its machine-scoped facts; the dispatch
    // machine's identity only names the composed plan itself.
    let &(machine, _) = fused.get(state_index)?;
    let (structural, _) = &signatures[state_index];
    let statements = program.statement_table.statements(state.statement_nodes);
    trace.phase("state graph: terminator");
    let ordinal = u32::try_from(terminator_index).ok()?;
    let edge_calls = EdgeCalls {
        scalar_callees,
        entry_claims,
        local_results,
    };
    let edge = |operations: &[CheckedUnitEffectOperationPlan],
                transition,
                edge_ordinal,
                kind,
                shapes: &mut ShapeCollector<'_>| {
        successor(
            program,
            facts,
            fused,
            state_index,
            signatures,
            operations,
            transition,
            edge_ordinal,
            kind,
            edge_calls,
            shapes,
            trace,
        )
    };
    let terminator = if let Some(terminator) = closed_sum::build(
        program,
        facts,
        shapes,
        fused,
        state_index,
        signatures,
        edge_calls,
        &result,
        operations,
        terminator_index,
        trace,
    ) {
        terminator
    } else if let checked_trees::CheckedControlResultPlan::Scalar { primitive_type } = result
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
        match typed_trees::statement::reachable_transition_prefix(&statements[terminator_index..]) {
            [] if result == checked_trees::CheckedControlResultPlan::Unit => {
                trace.phase("state graph: terminator: unit tail cleanup");
                return_cleanup_is_whole(
                    program, facts, machine, state, structural, operations, shapes,
                )?;
                CheckedComposedUnitControlTerminatorPlan::ReturnUnit
            }
            [StatementNode::Expression(expression)]
                if result != checked_trees::CheckedControlResultPlan::Unit =>
            {
                trace.phase("state graph: terminator: return expression");
                if let Some(result) = structural_result.cloned() {
                    CheckedComposedUnitControlTerminatorPlan::ReturnStructural { result }
                } else if let Some(operation) = returns::literal_result_operation(
                    program,
                    shapes,
                    state,
                    &mut returns::next_result_ordinal(operations)?,
                    ordinal,
                    *expression,
                ) {
                    let result = operation.structural_result_binding()?.clone().into();
                    operations.push(operation);
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
                match program.statement_table.transition_target(transition.target) {
                    // An unconditional `(expression)` arm closes the state
                    // with its value: the same establishment a bare tail
                    // expression plans, produced at the transition's ordinal.
                    TransitionTargetNode::Value(expression)
                        if result != checked_trees::CheckedControlResultPlan::Unit
                            && transition.exit == TransitionExit::Ordinary
                            && !transition.continuation.is_valid() =>
                    {
                        trace.phase("state graph: terminator: return expression");
                        if let Some(result) = structural_result.cloned() {
                            CheckedComposedUnitControlTerminatorPlan::ReturnStructural { result }
                        } else {
                            let mut count = returns::next_result_ordinal(operations)?;
                            let (operation, operand_calls) = returns::return_value_operation(
                                program,
                                facts,
                                scalar_callees,
                                shapes,
                                machine,
                                state,
                                structural,
                                entry_claims,
                                &mut count,
                                ordinal,
                                *expression,
                                trace,
                            )?;
                            let result = match &operation {
                                CheckedUnitEffectOperationPlan::EstablishStructuralValue {
                                    result,
                                    ..
                                }
                                | CheckedUnitEffectOperationPlan::EstablishReference {
                                    result,
                                    ..
                                }
                                | CheckedUnitEffectOperationPlan::EstablishByteSequence {
                                    result,
                                    ..
                                }
                                | CheckedUnitEffectOperationPlan::EstablishViewSubslice {
                                    result,
                                    ..
                                } => checked_trees::CheckedUnitStructuralReturnPlan::from(
                                    result.clone(),
                                ),
                                _ => return None,
                            };
                            operations.extend(operand_calls);
                            operations.push(operation);
                            CheckedComposedUnitControlTerminatorPlan::ReturnStructural { result }
                        }
                    }
                    // An unconditional terminal machine call already ran as
                    // this state's last operation (the sequence planned and
                    // counted it); its normal Unit result completes here.
                    TransitionTargetNode::Named { .. }
                        if result == checked_trees::CheckedControlResultPlan::Unit
                            && control::terminal_named_call(
                                program,
                                machine,
                                state,
                                terminator_index,
                            )
                            .is_some() =>
                    {
                        trace.phase("state graph: terminator: terminal call completion");
                        if !operations.iter().any(|operation| {
                            operation.call().is_some_and(|call| {
                                call.coordinate.statement_index == ordinal
                                    && call.coordinate.call_ordinal == 0
                                    && *call.result() == checked_trees::CheckedUnitCallResult::Unit
                            })
                        }) {
                            return None;
                        }
                        return_cleanup_is_whole(
                            program, facts, machine, state, structural, operations, shapes,
                        )?;
                        CheckedComposedUnitControlTerminatorPlan::ReturnUnit
                    }
                    _ => {
                        trace.phase("state graph: terminator: jump successor");
                        CheckedComposedUnitControlTerminatorPlan::Jump {
                            successor: edge(
                                operations,
                                transition,
                                ordinal,
                                SuccessorEdge::Jump,
                                shapes,
                            )?,
                        }
                    }
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
                    )
                    || covering_pair(facts, state.symbol, ordinal).is_some()) =>
            {
                trace.phase("state graph: terminator: conditional successors: guard expression");
                let guard = retained_guard(facts, machine.symbol, state.symbol, ordinal)?;
                if !guard_is_boolean(facts, &guard) {
                    trace.phase("state graph: terminator: conditional successors: guard type");
                    return None;
                }
                // A return arm completes with Unit or an established value;
                // a named arm keeps the ordinary successor custody plan.
                let mut return_count = returns::next_result_ordinal(operations)?;
                let mut branch = |transition: &typed_trees::statement::TableTransition,
                                  edge_ordinal: u32|
                 -> Option<
                    Result<
                        CheckedStructuralControlSuccessorPlan,
                        (
                            checked_trees::CheckedConditionalReturnArm,
                            Vec<CheckedUnitEffectOperationPlan>,
                        ),
                    >,
                > {
                    // Typing already classifies normal `{}` completion as
                    // Terminal. It returns Unit on this edge; it must not be
                    // looked up as a named successor or assigned a dummy value.
                    if matches!(
                        program.statement_table.transition_target(transition.target),
                        TransitionTargetNode::Terminal
                    ) && result == checked_trees::CheckedControlResultPlan::Unit
                        && transition.exit == TransitionExit::Ordinary
                        && !transition.continuation.is_valid()
                    {
                        return_cleanup_is_whole(
                            program, facts, machine, state, structural, operations, shapes,
                        )?;
                        return Some(Err((
                            checked_trees::CheckedConditionalReturnArm::Unit {
                                statement_ordinal: edge_ordinal,
                            },
                            Vec::new(),
                        )));
                    }
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
                        if let checked_trees::CheckedControlResultPlan::Scalar { primitive_type } =
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
                                checked_trees::CheckedConditionalReturnArm::Scalar {
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
                                checked_trees::CheckedConditionalReturnArm::Structural(operation),
                                operand_calls,
                            ))
                        });
                    }
                    // Another machine in target position is an invocation
                    // that completes this machine on its own arm only.
                    if terminal_calls::invoked_callable(program, machine, transition).is_some() {
                        return terminal_calls::arm_call(
                            program,
                            facts,
                            scalar_callees,
                            shapes,
                            machine,
                            state,
                            structural,
                            entry_claims,
                            local_results,
                            operations,
                            &result,
                            next_scalar_binding,
                            transition,
                            edge_ordinal,
                            trace,
                        )
                        .map(|arm| Err((arm, Vec::new())));
                    }
                    successor(
                        program,
                        facts,
                        fused,
                        state_index,
                        signatures,
                        operations,
                        transition,
                        edge_ordinal,
                        SuccessorEdge::Conditional,
                        edge_calls,
                        shapes,
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
                    // Both arms complete: at least one through a terminal
                    // machine call. Two `(expression)` targets check as
                    // Guarded instead.
                    (Err((when_true, true_operands)), Err((when_false, false_operands)))
                        if matches!(
                            (&when_true, &when_false),
                            (checked_trees::CheckedConditionalReturnArm::Call { .. }, _)
                                | (_, checked_trees::CheckedConditionalReturnArm::Call { .. })
                        ) =>
                    {
                        operations.extend(true_operands);
                        operations.extend(false_operands);
                        CheckedComposedUnitControlTerminatorPlan::ConditionalCompletion {
                            guard,
                            when_true,
                            when_false,
                        }
                    }
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
                if matches!(
                    transition.guard,
                    TransitionGuardNode::Always | TransitionGuardNode::When(_)
                )) =>
            {
                trace.phase("state graph: terminator: guarded jump successors: roster");
                let last_guarded = matches!(
                    tail.last(),
                    Some(StatementNode::Transition(transition))
                        if matches!(transition.guard, TransitionGuardNode::When(_))
                );
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
                // must agree with it exactly. A guarded final arm is the
                // roster's proven else: coverage keeps it, so no authored
                // fallback is recorded for it.
                if exits.len() != tail.len() - usize::from(!last_guarded)
                    || last_guarded != retained.fallback.is_none()
                {
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
                    // A mid-chain arm transfers to its named state or completes
                    // with its authored Unit/value result under the guard, just
                    // like the closing fallback.
                    if exit.guard_statement_ordinal != arm_ordinal {
                        return None;
                    }
                    let jump_target = internal_destination(
                        program,
                        machine,
                        transition,
                        &exit.destination,
                        arm_ordinal,
                    )?;
                    trace.phase(
                        "state graph: terminator: guarded jump successors: guard expression",
                    );
                    trace.statement(Some(arm_ordinal));
                    let guard = retained_guard(facts, machine.symbol, state.symbol, arm_ordinal)?;
                    if !guard_is_boolean(facts, &guard) {
                        return None;
                    }
                    let arm = if jump_target.is_valid() {
                        let target = jump_target;
                        let successor = edge(
                            operations,
                            transition,
                            arm_ordinal,
                            SuccessorEdge::GuardedJump,
                            shapes,
                        )?;
                        if successor.target_state != target {
                            trace.phase(
                                SuccessorEdge::GuardedJump.phase(SuccessorGuard::TargetState),
                            );
                            trace.statement(Some(arm_ordinal));
                            return None;
                        }
                        checked_trees::CheckedGuardedJumpArmPlan::Jump(CheckedGuardedJumpPlan {
                            guard,
                            successor,
                        })
                    } else {
                        let return_arm = guarded_return_arm(
                            program,
                            facts,
                            scalar_callees,
                            shapes,
                            machine,
                            state,
                            structural,
                            entry_claims,
                            local_results,
                            &result,
                            next_scalar_binding,
                            operations,
                            arm_ordinal,
                            transition,
                            trace,
                        )?;
                        checked_trees::CheckedGuardedJumpArmPlan::Return(
                            checked_trees::CheckedGuardedJumpReturnPlan {
                                guard,
                                statement_ordinal: arm_ordinal,
                                return_arm,
                            },
                        )
                    };
                    arms.push(arm);
                }
                let Some(StatementNode::Transition(fallback_transition)) = tail.last() else {
                    return None;
                };
                let fallback_ordinal = ordinal.checked_add(u32::try_from(tail.len() - 1).ok()?)?;
                let fallback_destination = if last_guarded {
                    exits.last().map(|exit| &exit.destination)
                } else {
                    retained.fallback.as_ref()
                };
                if last_guarded
                    && exits
                        .last()
                        .is_none_or(|exit| exit.guard_statement_ordinal != fallback_ordinal)
                {
                    return None;
                }
                // The roster's proven else may be a named-state edge or the
                // authored `(expression)` arm that returns its value — the
                // same outcome split a `ConditionalReturn` arm carries.
                let fallback = match internal_destination(
                    program,
                    machine,
                    fallback_transition,
                    fallback_destination?,
                    fallback_ordinal,
                )? {
                    target if target.is_valid() => {
                        let fallback = edge(
                            operations,
                            fallback_transition,
                            fallback_ordinal,
                            SuccessorEdge::GuardedJump,
                            shapes,
                        )?;
                        if fallback.target_state != target {
                            trace.phase(
                                SuccessorEdge::GuardedJump.phase(SuccessorGuard::TargetState),
                            );
                            trace.statement(Some(fallback_ordinal));
                            return None;
                        }
                        checked_trees::CheckedGuardedJumpFallbackPlan::Successor(fallback)
                    }
                    _ => {
                        trace.phase(
                            "state graph: terminator: guarded jump successors: return completion",
                        );
                        trace.statement(Some(fallback_ordinal));
                        checked_trees::CheckedGuardedJumpFallbackPlan::Return(guarded_return_arm(
                            program,
                            facts,
                            scalar_callees,
                            shapes,
                            machine,
                            state,
                            structural,
                            entry_claims,
                            local_results,
                            &result,
                            next_scalar_binding,
                            operations,
                            fallback_ordinal,
                            fallback_transition,
                            trace,
                        )?)
                    }
                };
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

/// Rejoin the shared roster's authored destination, distinguishing a local
/// state edge from completion through a value or another machine's result.
/// A machine-named destination is not a jump inside this activation merely
/// because the scalar roster retains its target alongside state targets.
fn internal_destination(
    program: &TypedTrees,
    machine: &typed_trees::machine::Machine,
    transition: &typed_trees::statement::TableTransition,
    destination: &checked_trees::CheckedScalarBranchDestination,
    ordinal: u32,
) -> Option<symbols::SymbolHandle> {
    if transition.exit != TransitionExit::Ordinary || transition.continuation.is_valid() {
        return None;
    }
    match destination {
        checked_trees::CheckedScalarBranchDestination::Jump(selected)
            if selected.statement_ordinal == ordinal && !selected.is_continuation =>
        {
            match terminal_calls::invoked_callable(program, machine, transition) {
                Some(target) => {
                    (target == selected.target).then_some(symbols::SymbolHandle::default())
                }
                None => Some(selected.target),
            }
        }
        checked_trees::CheckedScalarBranchDestination::Return {
            statement_ordinal,
            is_continuation: false,
        } if *statement_ordinal == ordinal => Some(symbols::SymbolHandle::default()),
        _ => None,
    }
}

/// The completion an ordered-chain arm selects: a mid-chain arm and the closing
/// fallback use this same join. Unit exits preserve ordinary return cleanup,
/// without manufacturing a value. A scalar arm rejoins the
/// `Return`-role value checking retained at its exact statement coordinate; a
/// structural arm mints the `EstablishStructuralValue` producer (its operand
/// calls join the state's operations; the producer itself stays inside the
/// arm so `operation_dependencies` reports it exactly once).
#[allow(clippy::too_many_arguments)]
fn guarded_return_arm(
    program: &TypedTrees,
    facts: &CheckFacts,
    scalar_callees: ScalarCalleePlans<'_>,
    shapes: &mut ShapeCollector<'_>,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    structural: &[CheckedUnitStructuralParameterPlan],
    entry_claims: &[checked_trees::CheckedUnitEntryClaimPlan],
    local_results: &[(
        checked_trees::CheckedUnitStructuralResultBindingPlan,
        facts::PlaceRoot,
    )],
    result: &checked_trees::CheckedControlResultPlan,
    next_scalar_binding: usize,
    operations: &mut Vec<CheckedUnitEffectOperationPlan>,
    statement_ordinal: u32,
    transition: &typed_trees::statement::TableTransition,
    trace: &control::LocalConstructionTrace,
) -> Option<checked_trees::CheckedConditionalReturnArm> {
    if transition.exit != TransitionExit::Ordinary || transition.continuation.is_valid() {
        return None;
    }
    if terminal_calls::invoked_callable(program, machine, transition).is_some() {
        return terminal_calls::arm_call(
            program,
            facts,
            scalar_callees,
            shapes,
            machine,
            state,
            structural,
            entry_claims,
            local_results,
            operations,
            result,
            next_scalar_binding,
            transition,
            statement_ordinal,
            trace,
        );
    }
    let expression = match program.statement_table.transition_target(transition.target) {
        TransitionTargetNode::Terminal
            if *result == checked_trees::CheckedControlResultPlan::Unit =>
        {
            return_cleanup_is_whole(
                program, facts, machine, state, structural, operations, shapes,
            )?;
            return Some(checked_trees::CheckedConditionalReturnArm::Unit { statement_ordinal });
        }
        TransitionTargetNode::Value(expression) => *expression,
        _ => return None,
    };
    if let checked_trees::CheckedControlResultPlan::Scalar { primitive_type } = result {
        let role = CheckedScalarExpressionRole::Return;
        let retained = facts
            .values
            .scalar_expressions
            .expression_at(state.symbol, statement_ordinal, role)
            .is_some()
            || facts
                .values
                .scalar_computations
                .root_at(state.symbol, statement_ordinal, role)
                .is_some_and(|root| root.machine == machine.symbol);
        if !retained {
            return None;
        }
        Some(checked_trees::CheckedConditionalReturnArm::Scalar {
            statement_ordinal,
            primitive_type: *primitive_type,
        })
    } else {
        let mut return_count = returns::next_result_ordinal(operations)?;
        let (operation, operand_calls) = returns::return_value_operation(
            program,
            facts,
            scalar_callees,
            shapes,
            machine,
            state,
            structural,
            entry_claims,
            &mut return_count,
            statement_ordinal,
            expression,
            trace,
        )?;
        operations.extend(operand_calls);
        Some(checked_trees::CheckedConditionalReturnArm::Structural(
            operation,
        ))
    }
}

/// The shared scalar roster's coverage evidence for a two-statement guarded
/// tail at `ordinal`: a retained pair records only when its guards provably
/// close the dispatch (an exact complement or complete case coverage of one
/// subject), so the second guard is the vacuous else the terminator admits.
fn covering_pair(facts: &CheckFacts, state: symbols::SymbolHandle, ordinal: u32) -> Option<()> {
    let retained = facts
        .flow
        .terminal_scalar_graphs
        .guarded_tails
        .iter()
        .filter(|tail| tail.state == state)
        .collect::<Vec<_>>();
    let [retained] = retained.as_slice() else {
        return None;
    };
    let exits = facts
        .flow
        .terminal_scalar_graphs
        .guarded_exits
        .span(retained.arms)?;
    (retained.fallback.is_none()
        && exits.len() == 2
        && exits[0].guard_statement_ordinal == ordinal
        && exits[1].guard_statement_ordinal == ordinal + 1)
        .then_some(())
}

#[cfg(test)]
mod tests {
    use super::{ScalarCalleePlans, ShapeCollector, control};

    #[test]
    fn ordered_terminal_calls_keep_selected_completion() {
        let checked = crate::tests::front_end::checked_program(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../../../tests/omega/pass/calls/guarded_terminal_call_chain_exit/main.omg"
        )));
        let program = &checked.typed;
        let machine = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "choose")
            .unwrap();
        let trace = control::LocalConstructionTrace::default();
        let plan = super::super::build_traced(
            program,
            &checked.facts,
            ScalarCalleePlans {
                boundary_returns: &checked.facts.flow.terminal_boundary_scalar_returns,
                structural_returns: &checked.facts.flow.terminal_structural_scalar_returns,
            },
            &mut ShapeCollector::new(program),
            machine,
            None,
            &trace,
        )
        .unwrap_or_else(|| panic!("ordered call completion: {:?}", trace.stage()));
        assert_eq!(
            plan.states[0]
                .operation_dependencies()
                .filter(|operation| operation.call().is_some())
                .count(),
            3
        );
    }
}
