//! The place a value expression denotes inside one state.
//!
//! Checking asks this question at every operand of every comparison, write and
//! transfer, so the answer carries one spelling here instead of repeating the
//! full state-local projection call at each site.

use symbols::SymbolHandle;
use typed_trees::expression::ExpressionHandle;

/// The canonical place `expression` denotes in `state_symbol` at
/// `statement_index`, when it denotes one at all.
pub(crate) fn place_at(
    program: &dyn ::validation::CheckingInput,
    state_symbol: SymbolHandle,
    statement_index: usize,
    expression: ExpressionHandle,
) -> Option<validation::CanonicalPlace> {
    validation::canonical_place_from_expression_in_state(
        program,
        state_symbol,
        statement_index,
        expression,
    )
}
