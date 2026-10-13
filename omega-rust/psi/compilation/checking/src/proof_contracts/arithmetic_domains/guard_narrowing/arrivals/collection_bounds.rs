//! Collection ordinals consume the same live, all-arrival integer observations
//! as arithmetic formation. A second state-local guard replay would lose
//! predecessor bounds and copied scalar equalities at the index occurrence.
//! The environment is after child effects: operand admission must therefore
//! retain its recursive purity fence. Effectful endpoints need captured child
//! values, not reconstruction from their later storage identities.

use symbols::SymbolHandle;
use typed_trees::expression::ExpressionHandle;

/// Prove the bounds of a builtin scalar index or window at its evaluation
/// occurrence. The range owner retains selected operator admission;
/// this judgment grants neither a loan nor a selected indexing implementation.
pub fn occurrence_collection_bounds_are_proven(
    program: &dyn crate::CheckingInput,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    statement_index: usize,
    expression: ExpressionHandle,
) -> bool {
    let Some(indexed) = program.expression_table.indexed_node(expression) else {
        return false;
    };
    let Some((machine, state)) = program.machine_state(machine_symbol, state_symbol) else {
        return false;
    };
    let Some(environment) = super::expression_observation(
        program,
        machine_symbol,
        state_symbol,
        Some(statement_index),
        expression,
    )
    .and_then(|observation| observation.environment) else {
        return false;
    };
    crate::proof_contracts::arithmetic_domains::live_integer_constraints::proves_collection_bounds(
        program,
        machine,
        state,
        &environment,
        indexed.collection,
        indexed.index,
    )
}
