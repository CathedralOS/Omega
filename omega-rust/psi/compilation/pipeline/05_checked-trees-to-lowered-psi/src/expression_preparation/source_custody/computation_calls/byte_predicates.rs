//! Exact source custody for a byte-sequence predicate over a structural
//! operand. The recognized intrinsic is read-only: preparation re-derives
//! that the authored call named a declared predicate over the operand the
//! subject plan still describes.
use super::byte_comparisons;
use super::{CheckedTrees, ExpressionHandle, LoweringError, unsupported};
use checked_trees::CheckedUnitStructuralArgumentPlan;

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

pub(crate) fn validate(
    checked: &CheckedTrees,
    state: symbols::SymbolHandle,
    statement: u32,
    expression: ExpressionHandle,
    operand_type: checked_trees::types::TypeReferenceHandle,
    subject: &CheckedUnitStructuralArgumentPlan,
) -> Result<(), LoweringError> {
    let (_, operand) = operands(checked, expression)?;
    byte_comparisons::validate_operand_provenance(
        checked,
        state,
        statement,
        operand,
        operand_type,
        subject,
    )
}
