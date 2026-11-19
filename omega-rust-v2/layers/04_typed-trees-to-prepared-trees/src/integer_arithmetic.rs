use num_bigint::BigInt;
use omega_typer::{IntegerOperationError, ValueType, evaluate_integer_operation};

/// Most operations fit without constructing either endpoint of the carrier.
pub(super) struct IntegerRange(ValueType);

impl IntegerRange {
    pub(super) fn for_type(carrier: &ValueType) -> Option<Self> {
        match carrier {
            ValueType::Integer { width: Some(0), .. } => None,
            ValueType::Integer { .. } => Some(Self(carrier.clone())),
            _ => None,
        }
    }

    pub(super) fn fits(
        &self,
        value: &BigInt,
    ) -> bool {
        self.0.contains_integer(value)
    }

    pub(super) fn apply(
        &self,
        token: &str,
        left: BigInt,
        right: BigInt,
    ) -> Result<BigInt, &'static str> {
        evaluate_integer_operation(token, &[left, right], &self.0).map_err(|error| match error {
            IntegerOperationError::ZeroDivisor if token.ends_with('!') => "a zero divisor leaves the selected Trap route possible",
            IntegerOperationError::ZeroDivisor => "integer division or remainder requires a nonzero divisor",
            IntegerOperationError::QuotientOutsideCarrier if token.ends_with('!') => "an unrepresentable quotient leaves the selected Trap route possible",
            IntegerOperationError::QuotientOutsideCarrier => "exact remainder requires a representable integer quotient",
            IntegerOperationError::OutsideCarrier(_) if token == "/!" => "an unrepresentable quotient leaves the selected Trap route possible",
            IntegerOperationError::OutsideCarrier(_) if token.ends_with('!') => "overflow leaves the selected Trap route possible",
            IntegerOperationError::OutsideCarrier(_) => match token {
                "+" => "exact addition exceeds its selected integer carrier",
                "-" => "exact subtraction exceeds its selected integer carrier",
                "*" => "exact multiplication exceeds its selected integer carrier",
                "/" => "exact division exceeds its selected integer carrier",
                _ => "exact operation exceeds its selected integer carrier",
            },
            IntegerOperationError::InvalidOperation => "the selected integer primitive has no execution rule",
            IntegerOperationError::Refused(reason) => reason,
        })
    }
}
