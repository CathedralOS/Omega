//! A read's root is current only in its exact authored state prefix.
//!
//! Parameters and preceding local declarations already belong to typed spans.
//! Reading those relationships needs no thread scope, copied symbol roster or
//! program-identity invalidation. Duplicate local declarations preserve prefix
//! membership here; the separate declaration-resolution query requires uniqueness.

use super::super::{Machine, State};

pub(in crate::checks::ranges::facts::dependencies) fn root_is_current(
    program: &dyn ::validation::CheckingInput,
    machine: &Machine,
    state: &State,
    statement_index: usize,
    root: facts::PlaceRoot,
) -> bool {
    let Some(symbol) = root.as_symbol() else {
        return false;
    };
    symbol == machine.symbol
        || program.state_parameter_by_symbol(state, symbol).is_some()
        || crate::flow::state_statements(program, state.statement_nodes)
            .iter()
            .take(statement_index)
            .any(|statement| {
                matches!(statement,
                typed_trees::statement::StatementNode::LocalData(local) if local.symbol == symbol)
            })
}
