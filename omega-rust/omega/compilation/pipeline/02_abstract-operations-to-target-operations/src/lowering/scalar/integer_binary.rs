//! Integer binary-operation kinds, folding, and target-expression construction.
use super::{KnownInteger, KnownScalar};
use crate::LoweringError;
use abstract_operations::{IntegerArithmetic, IntegerArithmeticOperator, IntegerOverflowPolicy};
use semantic_vocabulary::{IntegerType, IntegerValue, ValueId};
use std::collections::BTreeMap;
use target_operations::TargetIntegerExpression;
/// A binary integer operation over two operands of one carrier: a bitwise
/// operator, or an arithmetic policy pair with the obligation it carries.
#[derive(Clone, Copy)]
pub(in crate::lowering) enum IntegerBinaryKind {
    BitwiseAnd,
    BitwiseOr,
    BitwiseXor,
    Arithmetic(IntegerArithmetic),
}
pub(in crate::lowering) fn lower_integer_binary(
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
        use IntegerArithmeticOperator as Operator;
        use IntegerOverflowPolicy as Policy;
        let arithmetic = match self {
            Self::BitwiseAnd | Self::BitwiseOr | Self::BitwiseXor => {
                return LoweringError::IntegerBitwiseOperandTypeMismatch(result);
            }
            Self::Arithmetic(arithmetic) => arithmetic,
        };
        // Exact add, subtract and multiply share their wrapping sibling's
        // rejection.
        match (arithmetic.policy(), arithmetic.operator()) {
            (Policy::Wrapping | Policy::Exact, Operator::Add) => {
                LoweringError::WrappingAddOperandTypeMismatch(result)
            }
            (Policy::Saturating, Operator::Add) => {
                LoweringError::SaturatingAddOperandTypeMismatch(result)
            }
            (Policy::Wrapping | Policy::Exact, Operator::Subtract) => {
                LoweringError::WrappingSubtractOperandTypeMismatch(result)
            }
            (Policy::Saturating, Operator::Subtract) => {
                LoweringError::SaturatingSubtractOperandTypeMismatch(result)
            }
            (Policy::Wrapping | Policy::Exact, Operator::Multiply) => {
                LoweringError::WrappingMultiplyOperandTypeMismatch(result)
            }
            (Policy::Saturating, Operator::Multiply) => {
                LoweringError::SaturatingMultiplyOperandTypeMismatch(result)
            }
            (Policy::Exact, Operator::Divide) => {
                LoweringError::ExactDivideOperandTypeMismatch(result)
            }
            (Policy::Exact, Operator::Remainder) => {
                LoweringError::ExactRemainderOperandTypeMismatch(result)
            }
            (Policy::Wrapping, Operator::Divide) => {
                LoweringError::WrappingDivideOperandTypeMismatch(result)
            }
            (Policy::Wrapping, Operator::Remainder) => {
                LoweringError::WrappingRemainderOperandTypeMismatch(result)
            }
            (Policy::Saturating, Operator::Divide) => {
                LoweringError::SaturatingDivideOperandTypeMismatch(result)
            }
            (Policy::Saturating, Operator::Remainder) => {
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
            Self::Arithmetic(arithmetic) => arithmetic.evaluate(scalar_type, left, right),
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
            Self::Arithmetic(arithmetic) => TargetIntegerExpression::Arithmetic {
                psi_operation,
                arithmetic,
                left,
                right,
            },
        }
    }

    /// A proof-bearing pair is never folded here: its verifier-discharged
    /// obligation stays attached to the runtime expression.
    fn is_proof_bearing(self) -> bool {
        match self {
            Self::BitwiseAnd | Self::BitwiseOr | Self::BitwiseXor => false,
            Self::Arithmetic(arithmetic) => arithmetic.obligation().is_some(),
        }
    }
}
