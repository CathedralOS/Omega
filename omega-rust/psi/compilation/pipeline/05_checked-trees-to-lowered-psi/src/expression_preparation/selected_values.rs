//! Stage 04 publishes the completed structural copies in evaluation order.
//! Lowering consumes those independent value roots before target operands.
use crate::checked_facts::value_facts;

pub(crate) fn roots(
    checked: &checked_trees::CheckedTrees,
    owner: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
    statement: u32,
) -> Vec<&checked_trees::CheckedStructuralValueRoot> {
    value_facts(checked)
        .structural_values
        .roots_for_statement(state, statement)
        .into_iter()
        .filter(|root| root.machine == owner && root.captured_copy.is_valid())
        .collect()
}
