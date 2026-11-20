use crate::reasons::{PreparationReason, RefusalCause};
use num_bigint::BigInt;
use omega_typed::{IntegerOperationError, ValueType, evaluate_integer_operation};

/// Most operations fit without constructing either endpoint of the carrier.
pub(crate) struct IntegerRange(ValueType);

impl IntegerRange {
    pub(crate) fn for_type(carrier: &ValueType) -> Option<Self> {
        match carrier {
            ValueType::Integer { width: Some(0), .. } => None,
            ValueType::Integer { .. } => Some(Self(carrier.clone())),
            _ => None,
        }
    }

    pub(crate) fn fits(
        &self,
        value: &BigInt,
    ) -> bool {
        self.0.contains_integer(value)
    }

    pub(crate) fn apply(
        &self,
        token: &str,
        left: BigInt,
        right: BigInt,
    ) -> Result<BigInt, RefusalCause> {
        evaluate_integer_operation(token, &[left, right], &self.0).map_err(|error| {
            RefusalCause::Preparation(match error {
                IntegerOperationError::ZeroDivisor if token.ends_with('!') => PreparationReason::ZeroDivisorTrapPossible,
                IntegerOperationError::ZeroDivisor => PreparationReason::ZeroDivisor,
                IntegerOperationError::QuotientOutsideCarrier if token.ends_with('!') => PreparationReason::QuotientTrapPossible,
                IntegerOperationError::QuotientOutsideCarrier => PreparationReason::RemainderQuotientOutsideCarrier,
                IntegerOperationError::OutsideCarrier(_) if token == "/!" => PreparationReason::QuotientTrapPossible,
                IntegerOperationError::OutsideCarrier(_) if token.ends_with('!') => PreparationReason::OverflowTrapPossible,
                IntegerOperationError::OutsideCarrier(_) => match token {
                    "+" => PreparationReason::AdditionOutsideCarrier,
                    "-" => PreparationReason::SubtractionOutsideCarrier,
                    "*" => PreparationReason::MultiplicationOutsideCarrier,
                    "/" => PreparationReason::DivisionOutsideCarrier,
                    _ => PreparationReason::OperationOutsideCarrier,
                },
                IntegerOperationError::InvalidOperation => PreparationReason::IntegerPrimitiveWithoutRule,
                // The shared primitive owns this text, such as its allocation budget.
                IntegerOperationError::Refused(reason) => return RefusalCause::IntegerPrimitive(reason),
            })
        })
    }
}
