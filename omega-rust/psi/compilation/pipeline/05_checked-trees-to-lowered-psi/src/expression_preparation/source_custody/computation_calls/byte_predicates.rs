//! Exact source custody for a byte-sequence predicate over a structural
//! operand. The recognized intrinsic is read-only: preparation reads the
//! operand the authored call names.
use super::{CheckedTrees, ExpressionHandle, LoweringError};

pub(crate) fn operand(
    checked: &CheckedTrees,
    expression: ExpressionHandle,
) -> Result<ExpressionHandle, LoweringError> {
    let (_, operand) =
        typed_trees::byte_predicates::byte_predicate_call(&checked.typed, expression).ok_or(
            LoweringError::Unsupported("byte predicate lost its authored call"),
        )?;
    Ok(operand)
}
