//! Closed scalar contracts use only the supplied entry/result namespace.
//! A nested body local cannot borrow an equally numbered parameter slot.

use super::super::{
    CheckedBooleanExpression, CheckedScalarExpression, LoweringError, PrimitiveType,
};

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
        CheckedBooleanExpression::IntegerComparison { left, right, .. }
        | CheckedBooleanExpression::ScalarIeeeFloatComparison { left, right, .. } => {
            scalar(left)?;
            scalar(right)
        }
        CheckedBooleanExpression::Parameter { .. }
        | CheckedBooleanExpression::ErasedParameter { .. }
        | CheckedBooleanExpression::Constant(_) => Ok(()),
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
        CheckedScalarExpression::IntegerWrappingCast { .. } => Err(LoweringError::Unsupported(
            "wrapping conversion is not supported scalar contract arithmetic",
        )),
        CheckedScalarExpression::StructuralParameterIndexedRead { .. } => {
            Err(LoweringError::Unsupported(
                "indexed structural scalar reads are not supported contract operands",
            ))
        }
        CheckedScalarExpression::IntegerTrappingCast { .. } => Err(LoweringError::Unsupported(
            "trapping conversion is not total scalar contract arithmetic",
        )),
        CheckedScalarExpression::IntegerSaturatingCast { .. } => Err(LoweringError::Unsupported(
            "saturating conversion is not supported scalar contract arithmetic",
        )),
        CheckedScalarExpression::Local { .. } | CheckedScalarExpression::StorageRead { .. } => {
            Err(LoweringError::Unsupported(
                "scalar contract operand contains body-local or mutable storage",
            ))
        }
        CheckedScalarExpression::ErasedParameter { .. } => Ok(()),
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
        // A whole-view extent reads the parameter's own view descriptor — an
        // entry observation the closed namespace admits once the lowering
        // names the retained place. A nested byte-field extent still fails
        // closed at the term: its carrier path needs canonical field ids the
        // contract namespace does not mint.
        CheckedScalarExpression::StructuralParameterByteLength { path, .. } if path.is_empty() => {
            Ok(())
        }
        // A member leaf roots at a declared structural parameter; the
        // structural signature lowering names its retained place and field
        // path. An empty path would name the whole parameter — never a
        // scalar leaf — and an address member has no fixed-integer carrier.
        CheckedScalarExpression::StructuralParameterField {
            path,
            primitive_type,
            ..
        } => {
            if path.is_empty() || *primitive_type == PrimitiveType::Addr {
                return Err(LoweringError::Unsupported(
                    "closed scalar contract operand requires a scalar namespace",
                ));
            }
            crate::emission::scalar_types::integer_scalar_type(*primitive_type).map(|_| ())
        }
        CheckedScalarExpression::StructuralParameterByteLength { .. } => {
            Err(LoweringError::Unsupported(
                "closed scalar contract operand requires a scalar namespace",
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{CheckedBooleanExpression, CheckedScalarExpression, validate};
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
    fn structural_member_leaves_join_the_closed_scalar_namespace() {
        // A Boolean member leaf still needs an occurrence-bound crash
        // predicate; the closed namespace admits integer leaves only.
        assert!(
            validate(&CheckedBooleanExpression::StructuralParameterField {
                parameter_position: 0,
                path: Vec::new(),
            })
            .is_err()
        );
        // An empty member path names the whole parameter, never a scalar leaf.
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
        assert!(
            validate(&comparison(
                CheckedScalarExpression::StructuralParameterField {
                    parameter_position: 0,
                    path: vec![checked_trees::CheckedStructuralPredicatePathSegment::Field(
                        "rows".to_owned(),
                    )],
                    primitive_type: PrimitiveType::I32,
                }
            ))
            .is_ok()
        );
        // Address members have no fixed-integer contract carrier.
        assert!(
            validate(&comparison(
                CheckedScalarExpression::StructuralParameterField {
                    parameter_position: 0,
                    path: vec![checked_trees::CheckedStructuralPredicatePathSegment::Field(
                        "address".to_owned(),
                    )],
                    primitive_type: PrimitiveType::Addr,
                }
            ))
            .is_err()
        );
    }
}
