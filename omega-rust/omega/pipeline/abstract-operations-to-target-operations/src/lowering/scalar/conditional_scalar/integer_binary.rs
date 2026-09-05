//! Integer binary-operation kinds, folding, and target-expression construction.
use super::*;
#[derive(Clone, Copy)]
pub(in crate::lowering::scalar) enum IntegerBinaryKind {
    BitwiseAnd,
    BitwiseOr,
    BitwiseXor,
    WrappingAdd,
    ExactAdd(semantic_vocabulary::ObligationId),
    SaturatingAdd,
    WrappingSubtract,
    ExactSubtract(semantic_vocabulary::ObligationId),
    SaturatingSubtract,
    WrappingMultiply,
    ExactMultiply(semantic_vocabulary::ObligationId),
    SaturatingMultiply,
    ExactDivide(semantic_vocabulary::ObligationId),
    ExactRemainder(semantic_vocabulary::ObligationId),
    WrappingDivide(semantic_vocabulary::ObligationId),
    WrappingRemainder(semantic_vocabulary::ObligationId),
    SaturatingDivide(semantic_vocabulary::ObligationId),
    SaturatingRemainder(semantic_vocabulary::ObligationId),
}
pub(in crate::lowering::scalar) fn lower_conditional_integer_binary(
    values: &BTreeMap<ValueId, KnownScalar>,
    result: ValueId,
    scalar_type: IntegerType,
    left_id: ValueId,
    right_id: ValueId,
    kind: IntegerBinaryKind,
    psi_operation: semantic_vocabulary::OperationId,
) -> Result<KnownInteger, LoweringError> {
    let operand = |id| match values.get(&id).cloned() {
        Some(KnownScalar::Integer {
            scalar_type: operand_type,
            value,
        }) if operand_type == scalar_type => Ok(value),
        Some(_) => Err(kind.mismatch(result)),
        None => Err(LoweringError::UnknownValue(id)),
    };
    let left = operand(left_id)?;
    let right = operand(right_id)?;
    if kind.is_proof_bearing() {
        return Ok(KnownInteger::Runtime(kind.expression(
            psi_operation,
            left.into_expression(left_id),
            right.into_expression(right_id),
        )));
    }
    Ok(match (left, right) {
        (KnownInteger::Immediate(left), KnownInteger::Immediate(right)) => KnownInteger::Immediate(
            kind.fold(scalar_type, left, right)
                .ok_or(kind.mismatch(result))?,
        ),
        (left, right) => KnownInteger::Runtime(kind.expression(
            psi_operation,
            left.into_expression(left_id),
            right.into_expression(right_id),
        )),
    })
}

impl IntegerBinaryKind {
    fn mismatch(self, result: ValueId) -> LoweringError {
        match self {
            Self::BitwiseAnd | Self::BitwiseOr | Self::BitwiseXor => {
                LoweringError::IntegerBitwiseOperandTypeMismatch(result)
            }
            Self::WrappingAdd | Self::ExactAdd(_) => {
                LoweringError::WrappingAddOperandTypeMismatch(result)
            }
            Self::SaturatingAdd => LoweringError::SaturatingAddOperandTypeMismatch(result),
            Self::WrappingSubtract | Self::ExactSubtract(_) => {
                LoweringError::WrappingSubtractOperandTypeMismatch(result)
            }
            Self::SaturatingSubtract => {
                LoweringError::SaturatingSubtractOperandTypeMismatch(result)
            }
            Self::WrappingMultiply | Self::ExactMultiply(_) => {
                LoweringError::WrappingMultiplyOperandTypeMismatch(result)
            }
            Self::SaturatingMultiply => {
                LoweringError::SaturatingMultiplyOperandTypeMismatch(result)
            }
            Self::ExactDivide(_) => LoweringError::ExactDivideOperandTypeMismatch(result),
            Self::ExactRemainder(_) => LoweringError::ExactRemainderOperandTypeMismatch(result),
            Self::WrappingDivide(_) => LoweringError::WrappingDivideOperandTypeMismatch(result),
            Self::WrappingRemainder(_) => {
                LoweringError::WrappingRemainderOperandTypeMismatch(result)
            }
            Self::SaturatingDivide(_) => LoweringError::SaturatingDivideOperandTypeMismatch(result),
            Self::SaturatingRemainder(_) => {
                LoweringError::SaturatingRemainderOperandTypeMismatch(result)
            }
        }
    }

    fn fold(
        self,
        scalar_type: IntegerType,
        left: IntegerValue,
        right: IntegerValue,
    ) -> Option<IntegerValue> {
        match self {
            Self::BitwiseAnd => scalar_type.bitwise_and(left, right),
            Self::BitwiseOr => scalar_type.bitwise_or(left, right),
            Self::BitwiseXor => scalar_type.bitwise_xor(left, right),
            Self::WrappingAdd => scalar_type.wrapping_add(left, right),
            Self::ExactAdd(_) => scalar_type.exact_add(left, right),
            Self::SaturatingAdd => scalar_type.saturating_add(left, right),
            Self::WrappingSubtract => scalar_type.wrapping_sub(left, right),
            Self::ExactSubtract(_) => scalar_type.exact_sub(left, right),
            Self::SaturatingSubtract => scalar_type.saturating_sub(left, right),
            Self::WrappingMultiply => scalar_type.wrapping_mul(left, right),
            Self::ExactMultiply(_) => scalar_type.exact_mul(left, right),
            Self::SaturatingMultiply => scalar_type.saturating_mul(left, right),
            Self::ExactDivide(_) => scalar_type.exact_div(left, right),
            Self::ExactRemainder(_) => scalar_type.exact_rem(left, right),
            Self::WrappingDivide(_) => scalar_type.wrapping_div(left, right),
            Self::WrappingRemainder(_) => scalar_type.wrapping_rem(left, right),
            Self::SaturatingDivide(_) => scalar_type.saturating_div(left, right),
            Self::SaturatingRemainder(_) => scalar_type.saturating_rem(left, right),
        }
    }

    fn expression(
        self,
        psi_operation: semantic_vocabulary::OperationId,
        left: TargetIntegerExpression,
        right: TargetIntegerExpression,
    ) -> TargetIntegerExpression {
        let left = Box::new(left);
        let right = Box::new(right);
        match self {
            Self::BitwiseAnd => TargetIntegerExpression::BitwiseAnd {
                psi_operation,
                left,
                right,
            },
            Self::BitwiseOr => TargetIntegerExpression::BitwiseOr {
                psi_operation,
                left,
                right,
            },
            Self::BitwiseXor => TargetIntegerExpression::BitwiseXor {
                psi_operation,
                left,
                right,
            },
            Self::WrappingAdd => TargetIntegerExpression::WrappingAdd {
                psi_operation,
                left,
                right,
            },
            Self::ExactAdd(obligation) => TargetIntegerExpression::ExactAdd {
                psi_operation,
                obligation,
                left,
                right,
            },
            Self::SaturatingAdd => TargetIntegerExpression::SaturatingAdd {
                psi_operation,
                left,
                right,
            },
            Self::WrappingSubtract => TargetIntegerExpression::WrappingSubtract {
                psi_operation,
                left,
                right,
            },
            Self::ExactSubtract(obligation) => TargetIntegerExpression::ExactSubtract {
                psi_operation,
                obligation,
                left,
                right,
            },
            Self::SaturatingSubtract => TargetIntegerExpression::SaturatingSubtract {
                psi_operation,
                left,
                right,
            },
            Self::WrappingMultiply => TargetIntegerExpression::WrappingMultiply {
                psi_operation,
                left,
                right,
            },
            Self::ExactMultiply(obligation) => TargetIntegerExpression::ExactMultiply {
                psi_operation,
                obligation,
                left,
                right,
            },
            Self::SaturatingMultiply => TargetIntegerExpression::SaturatingMultiply {
                psi_operation,
                left,
                right,
            },
            Self::ExactDivide(obligation) => TargetIntegerExpression::ExactDivide {
                psi_operation,
                obligation,
                left,
                right,
            },
            Self::ExactRemainder(obligation) => TargetIntegerExpression::ExactRemainder {
                psi_operation,
                obligation,
                left,
                right,
            },
            Self::WrappingDivide(obligation) => TargetIntegerExpression::WrappingDivide {
                psi_operation,
                obligation,
                left,
                right,
            },
            Self::WrappingRemainder(obligation) => TargetIntegerExpression::WrappingRemainder {
                psi_operation,
                obligation,
                left,
                right,
            },
            Self::SaturatingDivide(obligation) => TargetIntegerExpression::SaturatingDivide {
                psi_operation,
                obligation,
                left,
                right,
            },
            Self::SaturatingRemainder(obligation) => TargetIntegerExpression::SaturatingRemainder {
                psi_operation,
                obligation,
                left,
                right,
            },
        }
    }
}

impl IntegerBinaryKind {
    fn is_proof_bearing(self) -> bool {
        matches!(
            self,
            Self::ExactAdd(_)
                | Self::ExactSubtract(_)
                | Self::ExactMultiply(_)
                | Self::ExactDivide(_)
                | Self::ExactRemainder(_)
                | Self::WrappingDivide(_)
                | Self::WrappingRemainder(_)
                | Self::SaturatingDivide(_)
                | Self::SaturatingRemainder(_)
        )
    }
}
