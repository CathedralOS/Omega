//! Independent proof-certified scalar identity classification.
use abstract_operations::{
    AbstractOperation as O, IntegerArithmeticOperator, IntegerOverflowPolicy,
    IntegerShiftDirection, IntegerShiftPolicy,
};
use optimization_unit::ProofCertifiedScalarIdentityKind;
use semantic_vocabulary::{
    IntegerCarrier, IntegerSign, IntegerType, IntegerValue, OperationId, ValueId,
};

#[derive(Debug, Clone, Copy)]
pub(crate) struct IndependentProofCertifiedScalarIdentity {
    pub(super) source_operation: OperationId,
    pub(super) obligation: semantic_vocabulary::ObligationId,
    pub(super) result: ValueId,
    pub(super) replacement: ValueId,
    pub(super) identity_operand: ValueId,
    pub(super) result_type: IntegerType,
    pub(super) identity_type: IntegerType,
    pub(super) identity_constant: IntegerValue,
}

pub(crate) fn independent_proof_certified_scalar_identity(
    operation: &O,
    identity: ProofCertifiedScalarIdentityKind,
) -> Option<IndependentProofCertifiedScalarIdentity> {
    use IntegerArithmeticOperator as Operator;
    use IntegerOverflowPolicy as Policy;
    use ProofCertifiedScalarIdentityKind as Kind;
    let row = match operation {
        O::IntegerArithmetic {
            psi_operation,
            result,
            scalar_type,
            arithmetic,
            left,
            right,
        } => {
            let obligation = arithmetic.obligation()?;
            let zero = independent_integer_zero(*scalar_type);
            let one = independent_integer_one(*scalar_type);
            // (replacement, identity operand, identity constant)
            let (replacement, identity_operand, identity_constant) =
                match (arithmetic.policy(), arithmetic.operator(), identity) {
                    (Policy::Exact, Operator::Add, Kind::ExactIntegerAddZeroLeft) => {
                        (*right, *left, zero)
                    }
                    (Policy::Exact, Operator::Add, Kind::ExactIntegerAddZeroRight)
                    | (Policy::Exact, Operator::Subtract, Kind::ExactIntegerSubtractZeroRight) => {
                        (*left, *right, zero)
                    }
                    (Policy::Exact, Operator::Multiply, Kind::ExactIntegerMultiplyOneLeft) => {
                        (*right, *left, one)
                    }
                    (Policy::Exact, Operator::Multiply, Kind::ExactIntegerMultiplyOneRight)
                    | (Policy::Exact, Operator::Divide, Kind::ExactIntegerDivideOneRight)
                    | (Policy::Wrapping, Operator::Divide, Kind::WrappingIntegerDivideOneRight)
                    | (
                        Policy::Saturating,
                        Operator::Divide,
                        Kind::SaturatingIntegerDivideOneRight,
                    ) => (*left, *right, one),
                    (Policy::Exact, Operator::Multiply, Kind::ExactIntegerMultiplyZeroLeft)
                    | (Policy::Exact, Operator::Divide, Kind::ExactIntegerDivideZeroLeft)
                    | (Policy::Wrapping, Operator::Divide, Kind::WrappingIntegerDivideZeroLeft)
                    | (
                        Policy::Saturating,
                        Operator::Divide,
                        Kind::SaturatingIntegerDivideZeroLeft,
                    )
                    | (Policy::Exact, Operator::Remainder, Kind::ExactIntegerRemainderZeroLeft)
                    | (
                        Policy::Wrapping,
                        Operator::Remainder,
                        Kind::WrappingIntegerRemainderZeroLeft,
                    )
                    | (
                        Policy::Saturating,
                        Operator::Remainder,
                        Kind::SaturatingIntegerRemainderZeroLeft,
                    ) => (*left, *left, zero),
                    (Policy::Exact, Operator::Multiply, Kind::ExactIntegerMultiplyZeroRight) => {
                        (*right, *right, zero)
                    }
                    _ => return None,
                };
            (
                *psi_operation,
                obligation,
                *result,
                replacement,
                identity_operand,
                *scalar_type,
                *scalar_type,
                identity_constant,
            )
        }
        O::IntegerShift {
            psi_operation,
            result,
            value_type,
            count_type,
            policy: IntegerShiftPolicy::Exact { obligation },
            direction,
            value,
            count,
        } => match (direction, identity) {
            (IntegerShiftDirection::Left, Kind::ExactIntegerShiftLeftZeroCount)
            | (IntegerShiftDirection::Right, Kind::ExactIntegerShiftRightZeroCount) => (
                *psi_operation,
                *obligation,
                *result,
                *value,
                *count,
                *value_type,
                *count_type,
                independent_integer_zero(*count_type),
            ),
            (IntegerShiftDirection::Left, Kind::ExactIntegerShiftLeftZeroValue)
            | (IntegerShiftDirection::Right, Kind::ExactIntegerShiftRightZeroValue) => (
                *psi_operation,
                *obligation,
                *result,
                *value,
                *value,
                *value_type,
                *value_type,
                independent_integer_zero(*value_type),
            ),
            (IntegerShiftDirection::Right, Kind::ExactIntegerShiftRightNegativeOneValue)
                if value_type.carrier() == IntegerCarrier::Fixed
                    && value_type.sign() == IntegerSign::Signed =>
            {
                (
                    *psi_operation,
                    *obligation,
                    *result,
                    *value,
                    *value,
                    *value_type,
                    *value_type,
                    IntegerValue::Signed(-1),
                )
            }
            _ => return None,
        },
        _ => return None,
    };
    Some(IndependentProofCertifiedScalarIdentity {
        source_operation: row.0,
        obligation: row.1,
        result: row.2,
        replacement: row.3,
        identity_operand: row.4,
        result_type: row.5,
        identity_type: row.6,
        identity_constant: row.7,
    })
}

pub(crate) fn independent_integer_zero(scalar_type: IntegerType) -> IntegerValue {
    match scalar_type.sign() {
        IntegerSign::Signed => IntegerValue::Signed(0),
        IntegerSign::Unsigned => IntegerValue::Unsigned(0),
    }
}

pub(crate) fn independent_integer_one(scalar_type: IntegerType) -> IntegerValue {
    match scalar_type.sign() {
        IntegerSign::Signed => IntegerValue::Signed(1),
        IntegerSign::Unsigned => IntegerValue::Unsigned(1),
    }
}
