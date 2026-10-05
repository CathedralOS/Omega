//! Rejoin constructor fields before their computations enter the ordinary
//! operand-scope walk. A matching final type cannot replace source identity.
use crate::expression_preparation::computation_graph::fields;
use crate::lowering_error::{LoweringError, unsupported};
use checked_trees::CheckedScalarCaseConstruction;
use checked_trees::CheckedTrees;
use language_semantics::Multiplicity;
type Computation = checked_trees::CheckedScalarComputationHandle;
use checked_trees::expression::{ExpressionHandle, ExpressionNode};

pub(crate) fn construction(
    checked: &CheckedTrees,
    subject: &CheckedScalarCaseConstruction,
) -> Result<Vec<(ExpressionHandle, Computation)>, LoweringError> {
    let source = validation::scalar_case_constructor(&checked.typed, subject.expression).ok_or(
        LoweringError::Unsupported("computed case lost its exact authored constructor"),
    )?;
    if source.type_reference != subject.type_reference || source.case != subject.case {
        return unsupported("computed case constructor type or case changed");
    }
    // This is the existing no-code ownership classifier, not an inference
    // from the currently selected payload being scalar.
    if !validation::has_plain_owned_contents_with_numeric_constraints(
        &checked.typed,
        subject.type_reference,
    ) || !matches!(
        checked.type_multiplicity(subject.type_reference),
        Multiplicity::Affine | Multiplicity::Unrestricted
    ) {
        return unsupported("computed case has no eligible no-code disposition");
    }
    let retained = fields(checked, subject)?;
    if retained.len() != source.fields.len() {
        return unsupported("computed case omitted or duplicated authored fields");
    }
    Ok(source
        .fields
        .into_iter()
        .zip(retained)
        .map(|((_, expression, _), field)| (expression, field.value))
        .collect())
}

/// Whether statement `index` is an erased runtime-indexed dispatch subject: an
/// immutable shared borrow followed by nothing but transitions. It plans no
/// operation of its own.
pub(crate) fn is_erased_dispatch_subject(
    checked: &CheckedTrees,
    statements: &[checked_trees::statement::StatementNode],
    index: usize,
) -> bool {
    let Some(checked_trees::statement::StatementNode::LocalData(local)) = statements.get(index)
    else {
        return false;
    };
    !local.is_mutable
        && local.symbol.is_valid()
        && matches!(
            checked.expression_table.expression(local.initial_value),
            ExpressionNode::Borrow(borrow)
                if borrow.access == language_core::ReferenceAccess::Shared
                    && matches!(
                        checked.expression_table.expression(borrow.target),
                        ExpressionNode::Indexed(indexed)
                            if checked
                                .expression_table
                                .constant_integer_value(indexed.index)
                                .is_none()
                    )
        )
        && statements.get(index + 1..).is_some_and(|arms| {
            !arms.is_empty()
                && arms.iter().all(|statement| {
                    matches!(
                        statement,
                        checked_trees::statement::StatementNode::Transition(_)
                    )
                })
        })
}
