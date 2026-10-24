//! Captured value identity and owned storage are different associations.
//!
//! A saved subject keeps its anchor for proof epochs and executable reads. An
//! owned, noncopyable subject lends its existing home to that observation;
//! moving a payload consumes that home's claims, not a newly invented anchor
//! owner. Copyable subjects instead own a separate captured value. This query
//! selects a storage identity only: availability, transfers and cleanup still
//! require the existing permission and claim receipts for that exact home.

use super::canonical_projection::{CanonicalPlace, canonical_place_from_expression_in_state};
use symbols::SymbolHandle;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};

pub fn ownership_place_from_expression_in_state(
    program: &dyn crate::CheckingInput,
    state_symbol: SymbolHandle,
    statement_index: usize,
    expression: ExpressionHandle,
) -> Option<CanonicalPlace> {
    let mut place = canonical_place_from_expression_in_state(
        program,
        state_symbol,
        statement_index,
        expression,
    )?;
    let mut active = Vec::new();
    while let Some(anchor) = place.root.as_expression()
        && let ExpressionNode::SavedSubject(saved) = program.expression_table.expression(anchor)
    {
        if active.contains(&saved.capture) {
            return None;
        }
        active.push(saved.capture);
        let source = program.expression_table.saved_subject_source(saved)?;
        let (machine, state) = crate::find_state(program, state_symbol)?;
        let reference = crate::expression_result_type_reference(program, machine, state, source)?;
        // Source use multiplicity, not carrier disposal multiplicity: a
        // [copy] record containing shared descriptors still owns a distinct
        // captured copy even though each materialized descriptor dies once.
        if program.type_multiplicity(reference) == language_semantics::Multiplicity::Unrestricted {
            break;
        }
        let mut owner = canonical_place_from_expression_in_state(
            program,
            state_symbol,
            statement_index,
            source,
        )?;
        owner.segments.extend(place.segments);
        place = owner;
    }
    Some(place)
}
