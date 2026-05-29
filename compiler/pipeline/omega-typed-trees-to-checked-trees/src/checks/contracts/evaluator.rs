use omega_checked_trees::{FlowCallFact, FlowStateFact};
use omega_typed_trees::expression::ExpressionHandle;
use omega_typed_trees::state::State;

mod booleans;
mod collections;
mod integers;
mod resolution;

pub(super) fn call_site_proves_boolean_contract_expression(
    program: &omega_typed_trees::TypedTrees,
    state_flow: &FlowStateFact,
    call_flow: &FlowCallFact,
    call_site: &crate::CallSite<'_>,
    target_state: &State,
    expression: ExpressionHandle,
) -> bool {
    let Some(caller_state) =
        crate::find_state_in_machine(program, state_flow.machine_symbol, state_flow.state_symbol)
    else {
        return false;
    };

    ContractExpressionEvaluator {
        program,
        caller_state,
        statement_index: call_flow.statement_index,
        call_site,
        target_state,
    }
    .boolean_value(expression)
    .unwrap_or(false)
}

pub(super) struct ContractExpressionEvaluator<'program, 'call> {
    program: &'program omega_typed_trees::TypedTrees,
    caller_state: &'program State,
    statement_index: usize,
    call_site: &'call crate::CallSite<'program>,
    target_state: &'program State,
}
