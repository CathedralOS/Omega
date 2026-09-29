use abstract_operations::{
    AbstractOperation, IntegerArithmetic, IntegerArithmeticOperator, IntegerOverflowPolicy,
};
use semantic_vocabulary::{OperationId, ScalarType};
use terminal_psi::{Operation, OperationKind};

use crate::lowering::LoweringError;

/// Lower one Terminal integer arithmetic operation. Terminal Psi names each
/// admitted policy pair as its own operation; Omega carries the pair as data,
/// so this is the one place that reads the per-pair Terminal vocabulary.
pub(super) fn lower(operation: &Operation) -> Result<AbstractOperation, LoweringError> {
    use IntegerArithmeticOperator as Operator;
    use IntegerOverflowPolicy as Policy;
    let (policy, operator, obligation, left, right) = match operation.kind.clone() {
        OperationKind::WrappingIntegerAdd { left, right } => {
            (Policy::Wrapping, Operator::Add, None, left, right)
        }
        OperationKind::ExactIntegerAdd {
            left,
            right,
            obligation,
        } => (Policy::Exact, Operator::Add, Some(obligation), left, right),
        OperationKind::SaturatingIntegerAdd { left, right } => {
            (Policy::Saturating, Operator::Add, None, left, right)
        }
        OperationKind::WrappingIntegerSubtract { left, right } => {
            (Policy::Wrapping, Operator::Subtract, None, left, right)
        }
        OperationKind::ExactIntegerSubtract {
            left,
            right,
            obligation,
        } => (
            Policy::Exact,
            Operator::Subtract,
            Some(obligation),
            left,
            right,
        ),
        OperationKind::SaturatingIntegerSubtract { left, right } => {
            (Policy::Saturating, Operator::Subtract, None, left, right)
        }
        OperationKind::WrappingIntegerMultiply { left, right } => {
            (Policy::Wrapping, Operator::Multiply, None, left, right)
        }
        OperationKind::ExactIntegerMultiply {
            left,
            right,
            obligation,
        } => (
            Policy::Exact,
            Operator::Multiply,
            Some(obligation),
            left,
            right,
        ),
        OperationKind::SaturatingIntegerMultiply { left, right } => {
            (Policy::Saturating, Operator::Multiply, None, left, right)
        }
        OperationKind::WrappingIntegerDivide {
            left,
            right,
            obligation,
        } => (
            Policy::Wrapping,
            Operator::Divide,
            Some(obligation),
            left,
            right,
        ),
        OperationKind::ExactIntegerDivide {
            left,
            right,
            obligation,
        } => (
            Policy::Exact,
            Operator::Divide,
            Some(obligation),
            left,
            right,
        ),
        OperationKind::SaturatingIntegerDivide {
            left,
            right,
            obligation,
        } => (
            Policy::Saturating,
            Operator::Divide,
            Some(obligation),
            left,
            right,
        ),
        OperationKind::WrappingIntegerRemainder {
            left,
            right,
            obligation,
        } => (
            Policy::Wrapping,
            Operator::Remainder,
            Some(obligation),
            left,
            right,
        ),
        OperationKind::ExactIntegerRemainder {
            left,
            right,
            obligation,
        } => (
            Policy::Exact,
            Operator::Remainder,
            Some(obligation),
            left,
            right,
        ),
        OperationKind::SaturatingIntegerRemainder {
            left,
            right,
            obligation,
        } => (
            Policy::Saturating,
            Operator::Remainder,
            Some(obligation),
            left,
            right,
        ),
        _ => unreachable!("arithmetic router is exhaustive"),
    };
    let ScalarType::Integer(scalar_type) = operation.result.expect_scalar().scalar_type else {
        return Err(malformed(policy, operator, operation.id));
    };
    let arithmetic = IntegerArithmetic::new(policy, operator, obligation)
        .ok_or_else(|| malformed(policy, operator, operation.id))?;
    Ok(AbstractOperation::IntegerArithmetic {
        psi_operation: operation.id,
        result: operation.result.expect_scalar().id,
        scalar_type,
        arithmetic,
        left,
        right,
    })
}

/// The malformed-operation rejection each pair has always reported. Exact
/// add, subtract and multiply share their wrapping sibling's rejection.
fn malformed(
    policy: IntegerOverflowPolicy,
    operator: IntegerArithmeticOperator,
    operation: OperationId,
) -> LoweringError {
    use IntegerArithmeticOperator as Operator;
    use IntegerOverflowPolicy as Policy;
    match (policy, operator) {
        (Policy::Wrapping | Policy::Exact, Operator::Add) => {
            LoweringError::VerifiedWrappingAddMalformed(operation)
        }
        (Policy::Wrapping | Policy::Exact, Operator::Subtract) => {
            LoweringError::VerifiedWrappingSubtractMalformed(operation)
        }
        (Policy::Wrapping | Policy::Exact, Operator::Multiply) => {
            LoweringError::VerifiedWrappingMultiplyMalformed(operation)
        }
        (Policy::Saturating, Operator::Add) => {
            LoweringError::VerifiedSaturatingAddMalformed(operation)
        }
        (Policy::Saturating, Operator::Subtract) => {
            LoweringError::VerifiedSaturatingSubtractMalformed(operation)
        }
        (Policy::Saturating, Operator::Multiply) => {
            LoweringError::VerifiedSaturatingMultiplyMalformed(operation)
        }
        (Policy::Exact, Operator::Divide) => LoweringError::VerifiedExactDivideMalformed(operation),
        (Policy::Exact, Operator::Remainder) => {
            LoweringError::VerifiedExactRemainderMalformed(operation)
        }
        (Policy::Wrapping, Operator::Divide) => {
            LoweringError::VerifiedWrappingDivideMalformed(operation)
        }
        (Policy::Wrapping, Operator::Remainder) => {
            LoweringError::VerifiedWrappingRemainderMalformed(operation)
        }
        (Policy::Saturating, Operator::Divide) => {
            LoweringError::VerifiedSaturatingDivideMalformed(operation)
        }
        (Policy::Saturating, Operator::Remainder) => {
            LoweringError::VerifiedSaturatingRemainderMalformed(operation)
        }
    }
}
