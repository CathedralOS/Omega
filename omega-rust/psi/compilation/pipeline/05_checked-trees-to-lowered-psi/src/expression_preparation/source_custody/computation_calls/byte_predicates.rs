//! Exact source custody for a byte-sequence predicate over a structural
//! operand. The recognized intrinsic is read-only: preparation re-derives
//! that the authored call named a declared predicate over the operand the
//! subject plan still describes.
use super::{CheckedTrees, ExpressionHandle, LoweringError, unsupported};

pub(crate) fn operands(
    checked: &CheckedTrees,
    expression: ExpressionHandle,
) -> Result<(terminal_psi::ByteSequencePredicate, ExpressionHandle), LoweringError> {
    let (predicate, operand) =
        typed_trees::byte_predicates::byte_predicate_call(&checked.typed, expression).ok_or(
            LoweringError::Unsupported("byte predicate lost its authored call"),
        )?;
    if checked
        .facts
        .operators
        .expression_use(expression)
        .is_some_and(|operator| {
            operator.selected_operator_symbol.is_valid() || operator.candidate_count != 0
        })
    {
        return unsupported("byte predicate changed its selected meaning");
    }
    Ok((predicate, operand))
}
