//! Call and arrival requirements consume their evaluated observations.
//!
//! This transports the same ordered environment used for arithmetic admission
//! through the exact target telescope. No whole-guard emptiness requirement,
//! callee precondition assumption, or spelling-based predicate reconstruction
//! can decide whether a post-mutation observation is current.
use super::{ExpressionHandle, Machine, State, ValueEnvironment, ordered_values};
use crate::machine_calls::calls::CallSite;
use typed_trees::statement::{StatementNode, TransitionGuardNode, TransitionTargetNode};

#[allow(clippy::too_many_arguments)]
pub fn evaluated_call_requires_proven(
    program: &dyn crate::CheckingInput,
    caller: &Machine,
    state: &State,
    statement_index: usize,
    site: CallSite<'_>,
    callee_owner: symbols::SymbolHandle,
    parameters: &[typed_trees::signature::StateParameter],
    arguments: &[ExpressionHandle],
    expression: ExpressionHandle,
) -> bool {
    let selected = program
        .machine_holding_state(callee_owner)
        .and_then(|machine| {
            program
                .state_in_machine(machine, callee_owner)
                .map(|state| (machine, state))
        })
        .or_else(|| {
            program.machine_by_symbol(callee_owner).and_then(|machine| {
                program
                    .machine_states(machine)
                    .first()
                    .map(|state| (machine, state))
            })
        });
    let Some((callee, callee_state)) = selected else {
        return false;
    };
    if parameters.len() != arguments.len() {
        return false;
    }
    let has_calls = arguments
        .iter()
        .any(|argument| crate::machine_calls::calls::expression_has_calls(program, *argument));
    let frames = has_calls
        .then(|| crate::CallFrameResolver::new(program))
        .flatten();
    // Pure actuals preserve the exact occurrence environment prepared by the
    // shared arrival/effect walk. This transports predicate consequences and
    // saved values at ordinary calls as well as state edges; source entry
    // assumptions are never replayed after an intervening write.
    let mut evaluated = if !arguments
        .iter()
        .any(|argument| crate::machine_calls::calls::expression_has_calls(program, *argument))
    {
        arguments.last().and_then(|argument| {
            super::guard_narrowing::occurrence_value_environment(
                program,
                caller.symbol,
                state.symbol,
                *argument,
            )
        })
    } else {
        None
    };
    if evaluated.is_none() {
        evaluated = selected_guard_environment(program, caller, state, statement_index, site);
    }
    let Some(mut evaluated) = evaluated else {
        return false;
    };

    let mut bindings = Vec::new();
    for (parameter, argument) in parameters.iter().zip(arguments) {
        if crate::machine_calls::calls::expression_has_calls(program, *argument) {
            match frames
                .as_ref()
                .and_then(|frames| frames.expression_may_write_paths(caller, *argument))
            {
                Some(writes) => evaluated.invalidate_written_paths(&writes),
                None => evaluated.invalidate_unknown_writes(program, Some(state)),
            }
        }
        let place = match program.expression_table.expression(*argument) {
            typed_trees::expression::ExpressionNode::Borrow(borrow) => borrow.target,
            _ => *argument,
        };
        let Some(operand) = ordered_values::operand(program, caller, state, place, frames.as_ref())
        else {
            return false;
        };
        bindings.push((ordered_values::Operand::parameter(parameter), operand));
    }
    super::live_integer_constraints::proves_condition(
        program,
        callee,
        callee_state,
        &evaluated,
        expression,
        &bindings,
    )
}

/// The selected transition guard retains its existing effect-aware route when
/// actuals cannot be projected as pure observations. Sibling arm facts never
/// become premises, and later argument writes still invalidate them below.
fn selected_guard_environment(
    program: &dyn crate::CheckingInput,
    caller: &Machine,
    state: &State,
    statement_index: usize,
    site: CallSite<'_>,
) -> Option<ValueEnvironment> {
    let Some(StatementNode::Transition(transition)) =
        program.statement_table.statement_at(state, statement_index)
    else {
        return None;
    };
    let TransitionGuardNode::When(condition) = transition.guard else {
        return None;
    };
    let matches_site = |target| match (program.statement_table.transition_target(target), site) {
        (
            TransitionTargetNode::Named {
                path, arguments, ..
            },
            CallSite::TransitionNamed {
                path: selected,
                arguments: actuals,
                ..
            },
        ) => std::ptr::eq(path, selected) && *arguments == actuals,
        (TransitionTargetNode::Value(value), CallSite::Expression { expression, .. }) => {
            *value == expression
        }
        _ => false,
    };
    let positive = match (
        transition.target.is_valid() && matches_site(transition.target),
        transition.continuation.is_valid() && matches_site(transition.continuation),
    ) {
        (true, false) => true,
        (false, true) => false,
        // A shared expression handle cannot identify which sibling observed
        // the call. It supplies no selected-path premise.
        _ => return None,
    };
    Some(super::guard_narrowing::evaluated_condition_environment(
        program,
        caller,
        Some(state),
        condition,
        positive,
        &ValueEnvironment::new(),
    ))
}
