use crate::context::*;
mod lookup;
mod traversal;

use crate::lookup::machine_by_symbol;
use traversal::find_call_site_in_statement;

pub(crate) enum CallSite<'program> {
    Statement(&'program omega_typed_trees::statement::TableCall),
    Expression(&'program omega_typed_trees::expression::TableCallExpression),
    TransitionNamed(omega_core::arena::HandleSpan<ExpressionHandle>),
}

pub(crate) fn find_call_site<'program>(
    program: &'program omega_typed_trees::TypedTrees,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    statement_index: usize,
    call_ordinal: usize,
) -> Option<CallSite<'program>> {
    let state = find_state_in_machine(program, machine_symbol, state_symbol)?;
    let machine = machine_by_symbol(program, machine_symbol)?;

    for (current_statement_index, statement) in program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .enumerate()
    {
        let mut current_ordinal = 0usize;
        if let Some(call_site) = find_call_site_in_statement(
            program,
            machine,
            state,
            statement,
            current_statement_index,
            statement_index,
            call_ordinal,
            &mut current_ordinal,
        ) {
            return Some(call_site);
        }
    }

    None
}

pub(crate) fn call_site_argument_expressions<'program>(
    program: &'program omega_typed_trees::TypedTrees,
    call_site: &CallSite<'program>,
) -> &'program [ExpressionHandle] {
    lookup::call_site_argument_expressions(program, call_site)
}

pub(crate) use lookup::{find_state, find_state_in_machine};
