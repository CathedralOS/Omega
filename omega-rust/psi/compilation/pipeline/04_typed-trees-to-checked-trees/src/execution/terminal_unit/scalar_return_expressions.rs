//! Classification of scalar and boolean return expressions.

pub(crate) fn checked_boolean_contains_short_circuit(
    expression: &checked_trees::CheckedBooleanExpression,
) -> bool {
    match expression {
        checked_trees::CheckedBooleanExpression::StorageRead { .. } => false,
        checked_trees::CheckedBooleanExpression::And { .. }
        | checked_trees::CheckedBooleanExpression::Or { .. } => true,
        checked_trees::CheckedBooleanExpression::Not(operand) => {
            checked_boolean_contains_short_circuit(operand)
        }
        checked_trees::CheckedBooleanExpression::Equal { left, right } => {
            checked_boolean_contains_short_circuit(left)
                || checked_boolean_contains_short_circuit(right)
        }
        checked_trees::CheckedBooleanExpression::Constant(_)
        | checked_trees::CheckedBooleanExpression::Parameter { .. }
        | checked_trees::CheckedBooleanExpression::ErasedParameter { .. }
        | checked_trees::CheckedBooleanExpression::Local { .. }
        | checked_trees::CheckedBooleanExpression::StructuralParameterField { .. }
        | checked_trees::CheckedBooleanExpression::IntegerComparison { .. }
        | checked_trees::CheckedBooleanExpression::IntegerMathComparison { .. }
        | checked_trees::CheckedBooleanExpression::IeeeFloatComparison { .. }
        | checked_trees::CheckedBooleanExpression::StructuralIdentity { .. }
        | checked_trees::CheckedBooleanExpression::ScalarIeeeFloatComparison { .. }
        | checked_trees::CheckedBooleanExpression::ByteSequenceEqual { .. }
        | checked_trees::CheckedBooleanExpression::ByteSequencePredicate { .. }
        | checked_trees::CheckedBooleanExpression::StructuralCaseMembership { .. } => false,
    }
}
