//! Exact source custody for a byte-content query over a structural operand.
//! Preparation never changes which operand the authored comparison observes.
use super::{CheckedTrees, ExpressionHandle, ExpressionNode, LoweringError, unsupported};
use checked_trees::{
    CheckedStructuralAccess, CheckedSubsliceSite, CheckedUnitStructuralArgumentPlan,
    CheckedUnitStructuralArgumentSourcePlan as Source,
};

pub(crate) fn operands(
    checked: &CheckedTrees,
    expression: ExpressionHandle,
) -> Result<(ExpressionHandle, &[u8], bool), LoweringError> {
    use checked_trees::expression::BinaryOperator;
    let ExpressionNode::Binary(binary) = checked.expression_table.expression(expression) else {
        return unsupported("byte comparison lost its authored operation");
    };
    if !matches!(
        binary.operator,
        BinaryOperator::Equal | BinaryOperator::NotEqual
    ) || checked
        .facts
        .operators
        .expression_use(expression)
        .is_some_and(|operator| {
            operator.selected_operator_symbol.is_valid()
                || operator.candidate_count != 0
                || !matches!(
                    operator.status,
                    checked_trees::CheckedOperatorResolutionStatus::Missing
                        | checked_trees::CheckedOperatorResolutionStatus::BuiltinFallback
                )
        })
    {
        return unsupported("byte comparison changed its selected meaning");
    }
    let negated = binary.operator == BinaryOperator::NotEqual;
    match (
        checked.expression_table.expression(binary.left),
        checked.expression_table.expression(binary.right),
    ) {
        (_, ExpressionNode::String(bytes)) => Ok((binary.left, bytes, negated)),
        (ExpressionNode::String(bytes), _) => Ok((binary.right, bytes, negated)),
        _ => unsupported("byte comparison lost its finite literal"),
    }
}

pub(crate) fn validate(
    checked: &CheckedTrees,
    state: symbols::SymbolHandle,
    statement: u32,
    expression: ExpressionHandle,
    operand_type: checked_trees::types::TypeReferenceHandle,
    subject: &CheckedUnitStructuralArgumentPlan,
) -> Result<(), LoweringError> {
    let (operand, _, _) = operands(checked, expression)?;
    validate_operand_provenance(checked, state, statement, operand, operand_type, subject)
}

/// Every byte-content query re-derives its operand's authored provenance the
/// same way: the comparison or predicate cannot substitute the operand's
/// type reference, storage root, projection path, carrier identity, or
/// access attenuation.
pub(crate) fn validate_operand_provenance(
    checked: &CheckedTrees,
    state: symbols::SymbolHandle,
    statement: u32,
    operand: ExpressionHandle,
    operand_type: checked_trees::types::TypeReferenceHandle,
    subject: &CheckedUnitStructuralArgumentPlan,
) -> Result<(), LoweringError> {
    let (machine, state) = super::authored_state(checked, state)?;
    let target =
        validation::expression_result_type_reference(&checked.typed, machine, state, operand)
            .ok_or(LoweringError::Unsupported(
                "byte comparison operand lost its result type",
            ))?;
    if target != operand_type {
        return unsupported("byte comparison substituted its operand type reference");
    }
    if matches!(
        subject.source,
        Source::ByteSequenceSubslice { .. } | Source::ElementViewSubslice { .. }
    ) {
        if !matches!(subject.source, Source::ByteSequenceSubslice { .. }) {
            return unsupported("byte comparison substituted an element view");
        }
        return super::mixed_arguments::validate_range_operand(
            checked,
            state,
            statement,
            target,
            operand,
            subject,
            CheckedSubsliceSite::Expression {
                expression: operand,
            },
        );
    }
    let source = crate::emission::call_source_custody::projected_receivers::source(
        checked,
        machine.symbol,
        state.symbol,
        statement as usize,
        operand,
    )?;
    let root = match subject.source {
        Source::Parameter { parameter_index } => {
            super::structural_parameter_at(
                checked,
                checked.state_parameters(state),
                parameter_index,
                "byte comparison lost its source parameter",
            )?
            .symbol
        }
        Source::StructuralLocal { symbol } => {
            super::preceding_local(
                checked,
                state,
                symbol,
                statement as usize,
                "byte comparison lost its local",
                "byte comparison local is not established",
            )?;
            symbol
        }
        _ => return unsupported("byte comparison substituted its operand storage"),
    };
    let mut carrier = target;
    loop {
        match checked.type_reference_table.type_reference(carrier) {
            checked_trees::types::TypeReferenceNode::Reference { referee, .. } => {
                carrier = *referee
            }
            checked_trees::types::TypeReferenceNode::Constrained { base_type, .. } => {
                carrier = *base_type
            }
            _ => break,
        }
    }
    if subject.access != CheckedStructuralAccess::SharedBorrow
        || root != source.root
        || subject.path != source.path
        || checked.normalized_type_identity(carrier).as_str() != subject.type_identity
    {
        return unsupported("byte comparison substituted its operand root, path, type or access");
    }
    Ok(())
}
