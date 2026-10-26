//! The terminator of one state in a composed Unit control graph.
//!
//! Tried in order: a closed-sum dispatch over the state's final case; a
//! scalar return's checked completion; then the state's authored tail --
//! a Unit or structural return, a crash exit, a jump, conditional or guarded
//! successors, or a terminal machine call. Every successor edge is built by
//! `successor` against the state signatures; a terminal call completes the
//! machine instead (`terminal_calls`); an unsupported tail names its shape.
//! Normal returns do not independently require whole-root no-code cleanup:
//! `cleanup::normal_edges` retains each selected exit's exact disposition
//! after the graph is built, before it can enter the published call closure.
//! That owner admits residual and nominal actions and refuses missing plans.
use super::{
    CheckFacts, CheckedComposedUnitControlTerminatorPlan, CheckedGuardedJumpPlan,
    CheckedScalarExpressionRole, CheckedStructuralControlSuccessorPlan,
    CheckedUnitEffectOperationPlan, CheckedUnitStructuralParameterPlan, EdgeCalls, FusedStates,
    ScalarCalleeClosure, ShapeCollector, Signature, StatementNode, SuccessorEdge, SuccessorGuard,
    TransitionExit, TransitionGuardNode, TransitionTargetNode, closed_sum, control,
    guard_is_boolean, retained_guard, returns, successor, terminal_calls,
};

/// The terminator one state's authored tail plans, after its operations.
/// `fused` carries the whole roster this graph spans: a successor naming a
/// foreign machine's entry resolves to that owner's fused states, so the
/// edge stays inside the one dispatch loop.
#[allow(clippy::too_many_arguments)]
pub(super) fn build(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    scalar_callees: ScalarCalleeClosure,
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
    if operations.iter().any(|operation| {
        matches!(
            operation,
            CheckedUnitEffectOperationPlan::EvaluateStructuralValue { .. }
        )
    }) {
        return Some(CheckedComposedUnitControlTerminatorPlan::EvaluateNoNormal);
    }
    if let Some(call) = operations
        .iter()
        .filter_map(CheckedUnitEffectOperationPlan::call)
        .find(|call| *call.result() == checked_trees::CheckedUnitCallResult::NoNormal)
    {
        return Some(CheckedComposedUnitControlTerminatorPlan::InvokeNoNormal {
            coordinate: call.coordinate,
        });
    }
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
    // Bare expressions and unconditional value arms are the same completion.
    // Keep one producer/operand schedule instead of making the bare spelling
    // admit only literals and case constructors while transitions admit loans.
    let complete_expression =
        |expression,
         shapes: &mut ShapeCollector<'_>,
         operations: &mut Vec<CheckedUnitEffectOperationPlan>| {
            trace.phase("state graph: terminator: return expression");
            let evaluation = match statements.get(terminator_index)? {
                StatementNode::Expression(_) => checked_trees::FlowPermissionEvaluation::Statement,
                StatementNode::Transition(transition) => {
                    checked_trees::FlowPermissionEvaluation::TransitionTarget(transition.target)
                }
                _ => return None,
            };
            let completion = checked_trees::CheckedStructuralReturnCompletion::Value {
                statement_index: ordinal,
                expression,
                evaluation,
            };
            if let Some(mut result) = structural_result.cloned() {
                result.completion = completion;
                return Some(CheckedComposedUnitControlTerminatorPlan::ReturnStructural { result });
            }
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
                expression,
                trace,
            )?;
            if !operation.has_normal_continuation() {
                operations.extend(operand_calls);
                operations.push(operation);
                return Some(CheckedComposedUnitControlTerminatorPlan::EvaluateNoNormal);
            }
            let result = checked_trees::CheckedUnitStructuralReturnPlan::from_binding(
                operation.structural_result_binding()?.clone(),
                completion,
            );
            operations.extend(operand_calls);
            operations.push(operation);
            Some(CheckedComposedUnitControlTerminatorPlan::ReturnStructural { result })
        };
    let before_closed_sum = trace.mark();
    let closed_sum = closed_sum::build(
        program,
        facts,
        shapes,
        fused,
        state_index,
        signatures,
        edge_calls,
        &result,
        next_scalar_binding,
        operations,
        terminator_index,
        trace,
    );
    // A case route that entered a specific obligation retains its refusal if
    // every later route also fails. An inapplicable case route leaves the mark
    // unchanged and must not hide the ordinary tail's own diagnostic.
    let closed_sum_failure =
        (closed_sum.is_none() && trace.mark() != before_closed_sum).then(|| trace.mark());
    let terminator = if let Some(terminator) = closed_sum {
        terminator
    } else if let checked_trees::CheckedControlResultPlan::Scalar { primitive_type } = result
        && let Some(completion) = returns::scalar_completion(
            program,
            fused,
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
        CheckedComposedUnitControlTerminatorPlan::ReturnScalar { completion }
    } else if let Some((terminator, operand_calls)) = returns::guarded(
        program,
        fused,
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
                CheckedComposedUnitControlTerminatorPlan::ReturnUnit
            }
            [StatementNode::Expression(expression)]
                if result != checked_trees::CheckedControlResultPlan::Unit =>
            {
                complete_expression(*expression, shapes, operations)?
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
                    // Explicit `{}` is the same normal Unit completion as an
                    // implicit tail. It is not a named state. The shared
                    // normal-edge cleanup owner still retains its exact exit
                    // disposition before this graph can be published.
                    TransitionTargetNode::Terminal
                        if result == checked_trees::CheckedControlResultPlan::Unit
                            && transition.exit == TransitionExit::Ordinary
                            && !transition.continuation.is_valid() =>
                    {
                        trace.phase("state graph: terminator: explicit unit completion");
                        CheckedComposedUnitControlTerminatorPlan::ReturnUnit
                    }
                    // An unconditional `(expression)` arm closes the state
                    // with its value: the same establishment a bare tail
                    // expression plans, produced at the transition's ordinal.
                    TransitionTargetNode::Value(_)
                        if validation::completing_transition_transfer(
                            program,
                            transition.target,
                        )
                        .is_some_and(|transfer| {
                            super::fused::fused_target_index(program, fused, transfer.target)
                                .is_some()
                        }) =>
                    {
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
                    // An unconditional terminal machine call already ran as
                    // this state's last operation (the sequence planned and
                    // counted it and bound its value); the callee's normal
                    // result completes this machine here.
                    TransitionTargetNode::Named { .. } | TransitionTargetNode::Value(_)
                        if terminal_calls::external_callable(
                            program, machine, fused, transition,
                        )
                        .is_some() =>
                    {
                        trace.phase("state graph: terminator: terminal call completion");
                        terminal_calls::completion(
                            operations,
                            ordinal,
                            &result,
                            scalar_result,
                            structural_result,
                        )?
                    }
                    TransitionTargetNode::Value(expression)
                        if result != checked_trees::CheckedControlResultPlan::Unit
                            && transition.exit == TransitionExit::Ordinary
                            && !transition.continuation.is_valid() =>
                    {
                        complete_expression(*expression, shapes, operations)?
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
                    if matches!(transition.exit, TransitionExit::Crash(_)) {
                        return crash_arm(program, transition, edge_ordinal)
                            .map(|arm| Err((arm, Vec::new())));
                    }
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
                        return Some(Err((
                            checked_trees::CheckedConditionalReturnArm::Unit {
                                statement_ordinal: edge_ordinal,
                            },
                            Vec::new(),
                        )));
                    }
                    if validation::completing_transition_transfer(program, transition.target)
                        .is_some_and(|transfer| {
                            super::fused::fused_target_index(program, fused, transfer.target)
                                .is_some()
                        })
                    {
                        return successor(
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
                        .map(Ok);
                    }
                    // Another machine in target position is an invocation
                    // that completes this machine on its own arm only.
                    if terminal_calls::external_callable(program, machine, fused, transition)
                        .is_some()
                    {
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
                    // Both selected outcomes complete this activation. A
                    // crash has no result or cleanup, independently of the
                    // ordinary completion selected on the other edge.
                    (Err((when_true, true_operands)), Err((when_false, false_operands))) => {
                        operations.extend(true_operands);
                        operations.extend(false_operands);
                        CheckedComposedUnitControlTerminatorPlan::ConditionalCompletion {
                            guard,
                            when_true,
                            when_false,
                        }
                    }
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
                    .guarded_exits
                    .tails
                    .iter()
                    .filter(|tail| tail.state == state.symbol)
                    .collect::<Vec<_>>();
                let [retained] = retained.as_slice() else {
                    return None;
                };
                let exits = facts.flow.guarded_exits.exits.span(retained.arms)?;
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
                    let transition = statement.as_transition()?;
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
                        fused,
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
                    fused,
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
                if let Some(mark) = closed_sum_failure {
                    trace.restore(&mark);
                }
                return None;
            }
        }
    };
    Some(terminator)
}

/// Rejoin the shared roster's authored destination, distinguishing a local
/// state edge from completion through a value or another machine's result.
/// Only the composed graph's explicit fused roster makes a foreign machine
/// entry an internal edge; the scalar branch roster alone cannot do so.
fn internal_destination(
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    fused: &FusedStates<'_>,
    transition: &typed_trees::statement::TableTransition,
    destination: &checked_trees::CheckedScalarBranchDestination,
    ordinal: u32,
) -> Option<symbols::SymbolHandle> {
    if let checked_trees::CheckedScalarBranchDestination::Crash { statement_ordinal } = destination
    {
        crash_arm(program, transition, ordinal)?;
        return (*statement_ordinal == ordinal).then_some(symbols::SymbolHandle::default());
    }
    if transition.exit != TransitionExit::Ordinary || transition.continuation.is_valid() {
        return None;
    }
    if let Some(transfer) = validation::completing_transition_transfer(program, transition.target)
        && super::fused::fused_target_index(program, fused, transfer.target).is_some()
    {
        return Some(transfer.state.symbol);
    }
    match destination {
        checked_trees::CheckedScalarBranchDestination::Jump(selected)
            if selected.statement_ordinal == ordinal && !selected.is_continuation =>
        {
            match terminal_calls::external_callable(program, machine, fused, transition) {
                Some(target) => {
                    (target == selected.target_state).then_some(symbols::SymbolHandle::default())
                }
                None => Some(selected.target_state),
            }
        }
        checked_trees::CheckedScalarBranchDestination::Return {
            statement_ordinal,
            is_continuation: false,
        }
        | checked_trees::CheckedScalarBranchDestination::InvokeNoNormal {
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
/// structural arm mints the `StructuralValue` local-binding producer (its operand
/// calls join the state's operations; the producer itself stays inside the
/// arm so `operation_dependencies` reports it exactly once).
#[allow(clippy::too_many_arguments)]
fn guarded_return_arm(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    scalar_callees: ScalarCalleeClosure,
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
    if matches!(transition.exit, TransitionExit::Crash(_)) {
        return crash_arm(program, transition, statement_ordinal);
    }
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
        .guarded_exits
        .tails
        .iter()
        .filter(|tail| tail.state == state)
        .collect::<Vec<_>>();
    let [retained] = retained.as_slice() else {
        return None;
    };
    let exits = facts.flow.guarded_exits.exits.span(retained.arms)?;
    (retained.fallback.is_none()
        && exits.len() == 2
        && exits[0].guard_statement_ordinal == ordinal
        && exits[1].guard_statement_ordinal == ordinal + 1)
        .then_some(())
}

/// Crash completion retains its authored occurrence without fabricating a
/// value, successor, or ordinary return-cleanup obligation.
fn crash_arm(
    program: &dyn ::validation::CheckingInput,
    transition: &typed_trees::statement::TableTransition,
    statement_ordinal: u32,
) -> Option<checked_trees::CheckedConditionalReturnArm> {
    (matches!(transition.exit, TransitionExit::Crash(_))
        && !transition.continuation.is_valid()
        && matches!(
            program.statement_table.transition_target(transition.target),
            TransitionTargetNode::Terminal
        ))
    .then_some(checked_trees::CheckedConditionalReturnArm::Crash { statement_ordinal })
}
