//! Arrival requirements consume the guard's evaluated observations.
//!
//! This transports the same ordered environment used for arithmetic admission
//! through the exact target telescope. No whole-guard emptiness requirement,
//! callee precondition assumption, or spelling-based predicate reconstruction
//! can decide whether a post-mutation observation is current.
use super::{ExpressionHandle, Machine, State, ValueEnvironment, ordered_values, place_path};
use crate::machine_calls::calls::CallSite;
use typed_trees::statement::{StatementNode, TransitionGuardNode, TransitionTargetNode};

#[allow(clippy::too_many_arguments)]
pub fn guarded_call_requires_proven(
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
    let Some(StatementNode::Transition(transition)) =
        program.statement_table.statement_at(state, statement_index)
    else {
        return false;
    };
    let TransitionGuardNode::When(condition) = transition.guard else {
        return false;
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
        _ => return false,
    };
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
    let has_calls = crate::machine_calls::calls::expression_has_calls(program, condition)
        || arguments
            .iter()
            .any(|argument| crate::machine_calls::calls::expression_has_calls(program, *argument));
    let frames = has_calls
        .then(|| crate::CallFrameResolver::new(program))
        .flatten();
    let mut evaluated = super::guard_narrowing::evaluated_condition_environment(
        program,
        caller,
        Some(state),
        condition,
        positive,
        &ValueEnvironment::new(),
    );
    let mut bindings = Vec::new();
    let mut exact = Vec::new();
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
        let Some(path) = place_path(program, place) else {
            return false;
        };
        bindings.push((path, parameter.name.as_str().to_owned()));
        exact.push((operand, ordered_values::Operand::parameter(parameter)));
    }
    let mut rebound = evaluated.rebind(&bindings);
    rebound.ordered_values = ordered_values::rebind_relations(&evaluated.ordered_values, &exact);
    super::live_integer_constraints::proves_condition(
        program,
        callee,
        callee_state,
        &rebound,
        expression,
    )
}
