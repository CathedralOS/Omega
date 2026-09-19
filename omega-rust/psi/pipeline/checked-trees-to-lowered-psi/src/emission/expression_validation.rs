use crate::emission::operation_emission::boolean::LoweredBooleanReturnExpression;
use crate::emission::operation_emission::expressions::LoweredDirectExpression;
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
    }
}

pub(crate) fn validate_direct_parameter_types(
    expression: &LoweredDirectExpression,
    parameter_types: &[ScalarType],
) -> Result<(), LoweringError> {
    match expression {
        LoweredDirectExpression::ErasedParameter { .. } => {
            return unsupported("erased formal is proof-only and has no runtime parameter type");
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
        | LoweredDirectExpression::ByteSequenceFieldLength { .. } => Ok(()),
        LoweredDirectExpression::ByteSequenceRead { index, .. } => {
            validate_direct_parameter_types(index, parameter_types)
        }
        LoweredDirectExpression::IntegerBinary { left, right, .. } => {
            validate_direct_parameter_types(left, parameter_types)?;
            validate_direct_parameter_types(right, parameter_types)
        }
        LoweredDirectExpression::IntegerBitwiseNot { operand, .. } => {
            validate_direct_parameter_types(operand, parameter_types)
        }
        LoweredDirectExpression::IntegerWiden { operand, .. } => {
            validate_direct_parameter_types(operand, parameter_types)
        }
        LoweredDirectExpression::IntegerExactCast { operand, .. } => {
            validate_direct_parameter_types(operand, parameter_types)
        }
        LoweredDirectExpression::Boolean { expression } => {
            validate_boolean_parameter_types(expression, parameter_types)
        }
    }
}

pub(crate) fn contains_short_circuit(expression: &LoweredBooleanReturnExpression) -> bool {
    match expression {
        LoweredBooleanReturnExpression::Constant { .. }
        | LoweredBooleanReturnExpression::StructuralCaseMembership { .. }
        | LoweredBooleanReturnExpression::PrimitiveRead { .. }
        | LoweredBooleanReturnExpression::Parameter { .. }
        | LoweredBooleanReturnExpression::Local { .. }
        | LoweredBooleanReturnExpression::UnresolvedStructuralParameterField { .. }
        | LoweredBooleanReturnExpression::StructuralField { .. }
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
        | LoweredBooleanReturnExpression::PrimitiveRead { .. }
        | LoweredBooleanReturnExpression::Parameter { .. }
        | LoweredBooleanReturnExpression::StructuralField { .. }
        | LoweredBooleanReturnExpression::Local { .. }
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
        contains_short_circuit, direct_expression_contains_short_circuit,
        validate_boolean_parameter_types, validate_direct_parameter_types,
        validate_short_circuit_expression,
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

    #[test]
    fn shared_short_circuit_queries_keep_boolean_control_visible() {
        let leaf = LoweredBooleanReturnExpression::Constant { value: true };
        assert!(!contains_short_circuit(&leaf));
        let conjunction = LoweredBooleanReturnExpression::And {
            left: Box::new(leaf.clone()),
            right: Box::new(leaf),
        };
        let nested = LoweredBooleanReturnExpression::Not {
            operand: Box::new(conjunction),
        };
        assert!(contains_short_circuit(&nested));
        assert!(validate_short_circuit_expression(&nested).is_ok());
        assert!(direct_expression_contains_short_circuit(
            &LoweredDirectExpression::Boolean {
                expression: Box::new(nested),
            }
        ));
    }
}
