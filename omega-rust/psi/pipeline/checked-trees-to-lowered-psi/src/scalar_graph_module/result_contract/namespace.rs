//! Closed scalar contracts use only the supplied entry/result namespace.
//! A nested body local cannot borrow an equally numbered parameter slot.

use super::{CheckedBooleanExpression, CheckedScalarExpression, LoweringError};

pub(super) fn validate(predicate: &CheckedBooleanExpression) -> Result<(), LoweringError> {
    match predicate {
        CheckedBooleanExpression::Local { .. } | CheckedBooleanExpression::StorageRead { .. } => {
            Err(LoweringError::Unsupported(
                "scalar contract predicate contains body-local or mutable storage",
            ))
        }
        CheckedBooleanExpression::Not(operand) => validate(operand),
        CheckedBooleanExpression::Equal { left, right }
        | CheckedBooleanExpression::And { left, right }
        | CheckedBooleanExpression::Or { left, right } => {
            validate(left)?;
            validate(right)
        }
        CheckedBooleanExpression::IntegerComparison { left, right, .. } => {
            scalar(left)?;
            scalar(right)
        }
        CheckedBooleanExpression::Parameter { .. } | CheckedBooleanExpression::Constant(_) => {
            Ok(())
        }
        CheckedBooleanExpression::StructuralParameterField { .. }
        | CheckedBooleanExpression::IeeeFloatComparison { .. }
        | CheckedBooleanExpression::ByteSequenceEqual { .. }
        | CheckedBooleanExpression::PayloadlessSumEqual { .. }
        | CheckedBooleanExpression::StructuralCaseMembership { .. } => {
            Err(LoweringError::Unsupported(
                "closed scalar contract predicate requires a scalar namespace",
            ))
        }
    }
}

fn scalar(expression: &CheckedScalarExpression) -> Result<(), LoweringError> {
    match expression {
        CheckedScalarExpression::Local { .. } | CheckedScalarExpression::StorageRead { .. } => {
            Err(LoweringError::Unsupported(
                "scalar contract operand contains body-local or mutable storage",
            ))
        }
        CheckedScalarExpression::IntegerBinary { left, right, .. } => {
            scalar(left)?;
            scalar(right)
        }
        CheckedScalarExpression::IntegerBitwiseNot { operand, .. }
        | CheckedScalarExpression::IntegerWiden { operand, .. }
        | CheckedScalarExpression::IntegerExactCast { operand, .. } => scalar(operand),
        CheckedScalarExpression::Boolean(predicate) => validate(predicate),
        CheckedScalarExpression::Parameter { .. }
        | CheckedScalarExpression::IntegerLiteral { .. }
        | CheckedScalarExpression::IeeeFloatLiteral { .. } => Ok(()),
        CheckedScalarExpression::StructuralParameterField { .. } => {
            Err(LoweringError::Unsupported(
                "closed scalar contract operand requires a scalar namespace",
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use checked_trees::{CheckedIntegerBinaryKind, CheckedIntegerComparisonKind};
    use typed_trees::types::PrimitiveType;

    fn scalar_parameter() -> CheckedScalarExpression {
        CheckedScalarExpression::Parameter {
            position: 0,
            primitive_type: PrimitiveType::I32,
        }
    }

    fn comparison(left: CheckedScalarExpression) -> CheckedBooleanExpression {
        CheckedBooleanExpression::IntegerComparison {
            kind: CheckedIntegerComparisonKind::Equal,
            left: Box::new(left),
            right: Box::new(scalar_parameter()),
        }
    }

    #[test]
    fn closed_entry_predicates_keep_boolean_and_integer_composition() {
        let arithmetic = CheckedScalarExpression::IntegerBinary {
            kind: CheckedIntegerBinaryKind::ExactAdd,
            primitive_type: PrimitiveType::I32,
            left: Box::new(scalar_parameter()),
            right: Box::new(CheckedScalarExpression::IntegerLiteral {
                literal: numerics::literals::IntegerLiteral::from_value(1),
            }),
        };
        let predicate = CheckedBooleanExpression::And {
            left: Box::new(CheckedBooleanExpression::Not(Box::new(
                CheckedBooleanExpression::Parameter { position: 1 },
            ))),
            right: Box::new(comparison(arithmetic)),
        };
        assert!(validate(&predicate).is_ok());
        assert!(validate(&CheckedBooleanExpression::Constant(false)).is_ok());
        // Bounds/carrier validation remains the actual namespace consumer's
        // responsibility; this query never invents an entry or result slot.
        assert!(
            validate(&CheckedBooleanExpression::Parameter {
                position: usize::MAX
            })
            .is_ok()
        );
    }

    #[test]
    fn boolean_local_and_storage_cannot_hide_under_connectives_or_equality() {
        for forbidden in [
            CheckedBooleanExpression::Local { position: 0 },
            CheckedBooleanExpression::StorageRead {
                symbol: symbols::SymbolHandle::from_arena_index(1),
            },
        ] {
            for wrapped in [
                forbidden.clone(),
                CheckedBooleanExpression::Not(Box::new(forbidden.clone())),
                CheckedBooleanExpression::Equal {
                    left: Box::new(forbidden.clone()),
                    right: Box::new(CheckedBooleanExpression::Constant(true)),
                },
                CheckedBooleanExpression::And {
                    left: Box::new(CheckedBooleanExpression::Constant(false)),
                    right: Box::new(CheckedBooleanExpression::Or {
                        left: Box::new(CheckedBooleanExpression::Parameter { position: 0 }),
                        right: Box::new(forbidden.clone()),
                    }),
                },
            ] {
                assert!(validate(&wrapped).is_err(), "{wrapped:?}");
            }
        }
    }

    #[test]
    fn nested_integer_operands_cannot_read_body_storage() {
        for forbidden in [
            CheckedScalarExpression::Local {
                position: 0,
                primitive_type: PrimitiveType::I32,
            },
            CheckedScalarExpression::StorageRead {
                symbol: symbols::SymbolHandle::from_arena_index(1),
                primitive_type: PrimitiveType::I32,
            },
            CheckedScalarExpression::Boolean(Box::new(CheckedBooleanExpression::Local {
                position: 0,
            })),
        ] {
            let wrapped = CheckedScalarExpression::IntegerBinary {
                kind: CheckedIntegerBinaryKind::ExactAdd,
                primitive_type: PrimitiveType::I32,
                left: Box::new(scalar_parameter()),
                right: Box::new(CheckedScalarExpression::IntegerWiden {
                    primitive_type: PrimitiveType::I32,
                    operand: Box::new(CheckedScalarExpression::IntegerBitwiseNot {
                        primitive_type: PrimitiveType::I16,
                        operand: Box::new(forbidden),
                    }),
                }),
            };
            assert!(validate(&comparison(wrapped)).is_err());
        }
    }

    #[test]
    fn structural_predicates_do_not_acquire_a_closed_scalar_namespace() {
        assert!(
            validate(&CheckedBooleanExpression::StructuralParameterField {
                parameter_position: 0,
                path: Vec::new(),
            })
            .is_err()
        );
        assert!(
            validate(&comparison(
                CheckedScalarExpression::StructuralParameterField {
                    parameter_position: 0,
                    path: Vec::new(),
                    primitive_type: PrimitiveType::I32,
                }
            ))
            .is_err()
        );
    }
}
