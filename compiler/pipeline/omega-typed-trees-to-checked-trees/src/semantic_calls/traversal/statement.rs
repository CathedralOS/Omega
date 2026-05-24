use super::*;
use crate::lookup::statement_call_can_dispatch_to_machine;
use expression::find_call_site_in_expression;

pub(crate) fn find_call_site_in_statement<'program>(
    program: &'program omega_typed_trees::TypedTrees,
    machine: &'program omega_typed_trees::machine::Machine,
    state: &'program omega_typed_trees::state::State,
    statement: &'program StatementNode,
    current_statement_index: usize,
    target_statement_index: usize,
    target_call_ordinal: usize,
    current_ordinal: &mut usize,
) -> Option<CallSite<'program>> {
    match statement {
        StatementNode::Assignment(assignment) => find_call_site_in_expression(
            program,
            machine,
            state,
            assignment.value,
            current_statement_index,
            target_statement_index,
            target_call_ordinal,
            current_ordinal,
        ),
        StatementNode::Call(call) => {
            let is_machine_call =
                statement_call_can_dispatch_to_machine(program, machine, state, call)
                    || call.target_symbol.is_valid();
            if is_machine_call {
                if current_statement_index == target_statement_index
                    && *current_ordinal == target_call_ordinal
                {
                    return Some(CallSite::Statement(call));
                }
                *current_ordinal = current_ordinal.saturating_add(1);
            }

            for argument in program.statement_table.expression_handles(call.arguments) {
                if let Some(call_site) = find_call_site_in_expression(
                    program,
                    machine,
                    state,
                    *argument,
                    current_statement_index,
                    target_statement_index,
                    target_call_ordinal,
                    current_ordinal,
                ) {
                    return Some(call_site);
                }
            }

            None
        }
        StatementNode::Expression(expression) => find_call_site_in_expression(
            program,
            machine,
            state,
            *expression,
            current_statement_index,
            target_statement_index,
            target_call_ordinal,
            current_ordinal,
        ),
        StatementNode::LocalData(local_data) => {
            if !local_data.initial_value.is_valid() {
                return None;
            }
            find_call_site_in_expression(
                program,
                machine,
                state,
                local_data.initial_value,
                current_statement_index,
                target_statement_index,
                target_call_ordinal,
                current_ordinal,
            )
        }
        StatementNode::Transition(transition) => {
            if let TransitionGuardNode::When(expression) = transition.guard
                && let Some(call_site) = find_call_site_in_expression(
                    program,
                    machine,
                    state,
                    expression,
                    current_statement_index,
                    target_statement_index,
                    target_call_ordinal,
                    current_ordinal,
                )
            {
                return Some(call_site);
            }

            if let Some(call_site) = find_call_site_in_transition_target(
                program,
                machine,
                state,
                transition.target,
                current_statement_index,
                target_statement_index,
                target_call_ordinal,
                current_ordinal,
            ) {
                return Some(call_site);
            }

            if transition.continuation.is_valid() {
                return find_call_site_in_transition_target(
                    program,
                    machine,
                    state,
                    transition.continuation,
                    current_statement_index,
                    target_statement_index,
                    target_call_ordinal,
                    current_ordinal,
                );
            }

            None
        }
    }
}

fn find_call_site_in_transition_target<'program>(
    program: &'program omega_typed_trees::TypedTrees,
    machine: &'program omega_typed_trees::machine::Machine,
    state: &'program omega_typed_trees::state::State,
    target: omega_typed_trees::statement::TransitionTargetHandle,
    current_statement_index: usize,
    target_statement_index: usize,
    target_call_ordinal: usize,
    current_ordinal: &mut usize,
) -> Option<CallSite<'program>> {
    match program.statement_table.transition_target(target) {
        TransitionTargetNode::Named { arguments, .. } => {
            for argument in program.statement_table.expression_handles(*arguments) {
                if let Some(call_site) = find_call_site_in_expression(
                    program,
                    machine,
                    state,
                    *argument,
                    current_statement_index,
                    target_statement_index,
                    target_call_ordinal,
                    current_ordinal,
                ) {
                    return Some(call_site);
                }
            }
            None
        }
        TransitionTargetNode::Value(expression) => find_call_site_in_expression(
            program,
            machine,
            state,
            *expression,
            current_statement_index,
            target_statement_index,
            target_call_ordinal,
            current_ordinal,
        ),
        TransitionTargetNode::SelfTarget | TransitionTargetNode::Terminal => None,
    }
}
