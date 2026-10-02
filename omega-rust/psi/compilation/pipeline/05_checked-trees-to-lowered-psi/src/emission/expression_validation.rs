use crate::emission::operation_emission::boolean::LoweredBooleanReturnExpression;
use crate::emission::operation_emission::expressions::{
    LoweredDirectExpression, LoweredElementPathSegment,
};
use crate::lowering_error::{LoweringError, unsupported};
use semantic_vocabulary::ScalarType;

pub(crate) fn validate_boolean_parameter_types(
    expression: &LoweredBooleanReturnExpression,
    parameter_types: &[ScalarType],
) -> Result<(), LoweringError> {
    match expression {
        LoweredBooleanReturnExpression::Constant { .. } => Ok(()),
        LoweredBooleanReturnExpression::StructuralField { .. }
        | LoweredBooleanReturnExpression::StructuralCaseMembership { .. }
        | LoweredBooleanReturnExpression::PrimitiveRead { .. } => Ok(()),
        LoweredBooleanReturnExpression::UnresolvedStructuralParameterField { .. } => {
            unsupported("unresolved structural field crossed Boolean type validation")
        }
        LoweredBooleanReturnExpression::Parameter { position }
        | LoweredBooleanReturnExpression::Local { position } => {
            if parameter_types.get(*position) == Some(&ScalarType::Boolean) {
                Ok(())
            } else {
                unsupported("scalar graph guard parameters must be Boolean")
            }
        }
        LoweredBooleanReturnExpression::Not { operand } => {
            validate_boolean_parameter_types(operand, parameter_types)
        }
        LoweredBooleanReturnExpression::Equal { left, right }
        | LoweredBooleanReturnExpression::And { left, right }
        | LoweredBooleanReturnExpression::Or { left, right } => {
            validate_boolean_parameter_types(left, parameter_types)?;
            validate_boolean_parameter_types(right, parameter_types)
        }
        LoweredBooleanReturnExpression::IntegerComparison { left, right, .. } => {
            validate_direct_parameter_types(left, parameter_types)?;
            validate_direct_parameter_types(right, parameter_types)
        }
        LoweredBooleanReturnExpression::IndexedCaseMembership { index, .. } => {
            validate_direct_parameter_types(index, parameter_types)
        }
        LoweredBooleanReturnExpression::ByteSequencePredicate { path, .. } => {
            for segment in path {
                if let LoweredElementPathSegment::RuntimeIndex { selector, extent } = segment {
                    validate_direct_parameter_types(selector, parameter_types)?;
                    if let Some(extent) = extent {
                        validate_direct_parameter_types(extent, parameter_types)?;
                    }
                }
            }
            Ok(())
        }
    }
}

pub(crate) fn validate_direct_parameter_types(
    expression: &LoweredDirectExpression,
    parameter_types: &[ScalarType],
) -> Result<(), LoweringError> {
    match expression {
        LoweredDirectExpression::ErasedParameter { .. } => {
            unsupported("erased formal is proof-only and has no runtime parameter type")
        }
        LoweredDirectExpression::Parameter {
            position,
            scalar_type,
        }
        | LoweredDirectExpression::Local {
            position,
            scalar_type,
        } => {
            if parameter_types.get(*position) == Some(scalar_type) {
                Ok(())
            } else {
                unsupported("scalar graph integer guard parameter type does not match")
            }
        }
        LoweredDirectExpression::IntegerLiteral { .. }
        | LoweredDirectExpression::PrimitiveRead { .. }
        | LoweredDirectExpression::StructuralField { .. }
        | LoweredDirectExpression::IeeeFloatLiteral { .. }
        | LoweredDirectExpression::ByteSequenceLength { .. }
        | LoweredDirectExpression::ElementViewLength { .. } => Ok(()),
        LoweredDirectExpression::IndexedPrimitiveRead {
            index,
            inner_indexes,
            element_path,
            ..
        } => {
            validate_direct_parameter_types(index, parameter_types)?;
            inner_indexes
                .iter()
                .try_for_each(|inner| validate_direct_parameter_types(inner, parameter_types))?;
            validate_path_parameter_types(element_path, parameter_types)
        }
        LoweredDirectExpression::ByteSequenceFieldLength { path, .. } => {
            validate_path_parameter_types(path, parameter_types)
        }
        LoweredDirectExpression::ByteSequenceFieldRead { path, index, .. } => {
            validate_path_parameter_types(path, parameter_types)?;
            validate_direct_parameter_types(index, parameter_types)
        }
        LoweredDirectExpression::ByteSequenceRead { index, .. }
        | LoweredDirectExpression::IndexedElementFieldRead { index, .. }
        | LoweredDirectExpression::ElementViewRead { index, .. } => {
            validate_direct_parameter_types(index, parameter_types)
        }
        LoweredDirectExpression::IntegerBinary { left, right, .. }
        | LoweredDirectExpression::SaturatingShiftLeft { left, right, .. } => {
            validate_direct_parameter_types(left, parameter_types)?;
            validate_direct_parameter_types(right, parameter_types)
        }
        LoweredDirectExpression::IntegerBitwiseNot { operand, .. } => {
            validate_direct_parameter_types(operand, parameter_types)
        }
        LoweredDirectExpression::IntegerWiden { operand, .. } => {
            validate_direct_parameter_types(operand, parameter_types)
        }
        LoweredDirectExpression::IntegerExactCast { operand, .. }
        | LoweredDirectExpression::RepresentationView { operand, .. }
        | LoweredDirectExpression::IntegerTrappingCast { operand, .. } => {
            validate_direct_parameter_types(operand, parameter_types)
        }
        LoweredDirectExpression::Boolean { expression } => {
            validate_boolean_parameter_types(expression, parameter_types)
        }
    }
}

fn validate_path_parameter_types(
    path: &[crate::emission::operation_emission::expressions::LoweredElementPathSegment],
    parameter_types: &[ScalarType],
) -> Result<(), LoweringError> {
    use crate::emission::operation_emission::expressions::{
        LoweredElementPathSegment, TERMINAL_NESTED_SELECTOR_LIMIT,
    };
    let selectors = path.iter().filter_map(|segment| match segment {
        LoweredElementPathSegment::RuntimeIndex { selector, extent } => Some((selector, extent)),
        _ => None,
    });
    if selectors.clone().count() > TERMINAL_NESTED_SELECTOR_LIMIT {
        return unsupported("field observation exceeds its selector namespace");
    }
    for (selector, extent) in selectors {
        if let Some(extent) = extent {
            validate_direct_parameter_types(extent, parameter_types)?;
        }
        validate_direct_parameter_types(selector, parameter_types)?;
        if !matches!(selector.scalar_type(), ScalarType::Integer(integer)
            if integer.sign() == semantic_vocabulary::IntegerSign::Unsigned && integer.bits() == 64 && !integer.is_address())
        {
            return unsupported("field observation selector requires an exact u64 value");
        }
    }
    Ok(())
}

pub(crate) fn contains_short_circuit(expression: &LoweredBooleanReturnExpression) -> bool {
    match expression {
        LoweredBooleanReturnExpression::Constant { .. }
        | LoweredBooleanReturnExpression::StructuralCaseMembership { .. }
        | LoweredBooleanReturnExpression::IndexedCaseMembership { .. }
        | LoweredBooleanReturnExpression::PrimitiveRead { .. }
        | LoweredBooleanReturnExpression::Parameter { .. }
        | LoweredBooleanReturnExpression::Local { .. }
        | LoweredBooleanReturnExpression::UnresolvedStructuralParameterField { .. }
        | LoweredBooleanReturnExpression::StructuralField { .. }
        | LoweredBooleanReturnExpression::ByteSequencePredicate { .. }
        | LoweredBooleanReturnExpression::IntegerComparison { .. } => false,
        LoweredBooleanReturnExpression::Not { operand } => contains_short_circuit(operand),
        LoweredBooleanReturnExpression::Equal { left, right } => {
            contains_short_circuit(left) || contains_short_circuit(right)
        }
        LoweredBooleanReturnExpression::And { .. } | LoweredBooleanReturnExpression::Or { .. } => {
            true
        }
    }
}

pub(crate) fn direct_expression_contains_short_circuit(
    expression: &LoweredDirectExpression,
) -> bool {
    matches!(
        expression,
        LoweredDirectExpression::Boolean { expression }
            if contains_short_circuit(expression)
    )
}

pub(crate) fn validate_short_circuit_expression(
    expression: &LoweredBooleanReturnExpression,
) -> Result<(), LoweringError> {
    match expression {
        LoweredBooleanReturnExpression::Constant { .. }
        | LoweredBooleanReturnExpression::StructuralCaseMembership { .. }
        | LoweredBooleanReturnExpression::IndexedCaseMembership { .. }
        | LoweredBooleanReturnExpression::PrimitiveRead { .. }
        | LoweredBooleanReturnExpression::Parameter { .. }
        | LoweredBooleanReturnExpression::StructuralField { .. }
        | LoweredBooleanReturnExpression::Local { .. }
        | LoweredBooleanReturnExpression::ByteSequencePredicate { .. }
        | LoweredBooleanReturnExpression::IntegerComparison { .. } => Ok(()),
        LoweredBooleanReturnExpression::UnresolvedStructuralParameterField { .. } => {
            unsupported("unresolved structural field crossed Boolean validation")
        }
        LoweredBooleanReturnExpression::Not { operand } => {
            validate_short_circuit_expression(operand)
        }
        LoweredBooleanReturnExpression::Equal { left, right } => {
            validate_short_circuit_expression(left)?;
            validate_short_circuit_expression(right)
        }
        LoweredBooleanReturnExpression::And { left, right }
        | LoweredBooleanReturnExpression::Or { left, right } => {
            validate_short_circuit_expression(left)?;
            validate_short_circuit_expression(right)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        LoweredBooleanReturnExpression, LoweredDirectExpression, ScalarType,
        validate_boolean_parameter_types, validate_direct_parameter_types,
    };

    #[test]
    fn shared_operand_validation_retains_position_and_type_requirements() {
        let boolean = LoweredBooleanReturnExpression::Parameter { position: 0 };
        assert!(validate_boolean_parameter_types(&boolean, &[ScalarType::Boolean]).is_ok());
        assert!(validate_boolean_parameter_types(&boolean, &[]).is_err());
        let integer_type = ScalarType::Integer(
            semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 64)
                .unwrap(),
        );
        assert!(validate_boolean_parameter_types(&boolean, &[integer_type]).is_err());
        let integer = LoweredDirectExpression::Parameter {
            position: 0,
            scalar_type: integer_type,
        };
        assert!(validate_direct_parameter_types(&integer, &[integer_type]).is_ok());
        assert!(validate_direct_parameter_types(&integer, &[ScalarType::Boolean]).is_err());
        assert!(validate_direct_parameter_types(&integer, &[]).is_err());
    }
}
