use crate::{IntegerCarrier, IntegerType, IntegerValue, PropositionError};

/// A fixed integer carrier restricted to one closed, inclusive interval.
/// Retaining this declaration does not establish that any supplied value inhabits it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BoundedIntegerType {
    integer_type: IntegerType,
    minimum: IntegerValue,
    maximum: IntegerValue,
}

impl BoundedIntegerType {
    pub fn new(
        integer_type: IntegerType,
        minimum: IntegerValue,
        maximum: IntegerValue,
    ) -> Result<Self, PropositionError> {
        if integer_type.carrier() != IntegerCarrier::Fixed
            || !integer_type.admits(minimum)
            || !integer_type.admits(maximum)
            || minimum > maximum
        {
            return Err(PropositionError::InvalidIntegerRange {
                integer_type,
                minimum,
                maximum,
            });
        }
        Ok(Self {
            integer_type,
            minimum,
            maximum,
        })
    }

    pub const fn integer_type(self) -> IntegerType {
        self.integer_type
    }
    pub const fn minimum(self) -> IntegerValue {
        self.minimum
    }
    pub const fn maximum(self) -> IntegerValue {
        self.maximum
    }

    pub fn contains(self, value: IntegerValue) -> bool {
        self.integer_type.admits(value) && self.minimum <= value && value <= self.maximum
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::IntegerSign;

    #[test]
    fn bounded_integer_type_preserves_full_width_endpoints() {
        let integer = IntegerType::new(IntegerSign::Unsigned, 128).unwrap();
        let range = BoundedIntegerType::new(
            integer,
            IntegerValue::Unsigned(1 << 127),
            IntegerValue::Unsigned(u128::MAX),
        )
        .unwrap();
        assert!(range.contains(IntegerValue::Unsigned(u128::MAX)));
        assert!(!range.contains(IntegerValue::Unsigned(0)));
        assert!(!range.contains(IntegerValue::Signed(1)));
        assert_eq!(range.integer_type(), integer);
        assert_eq!(range.minimum(), IntegerValue::Unsigned(1 << 127));
        assert_eq!(range.maximum(), IntegerValue::Unsigned(u128::MAX));
    }

    #[test]
    fn bounded_integer_type_rejects_invalid_endpoints_and_address_carriers() {
        let integer = IntegerType::new(IntegerSign::Signed, 8).unwrap();
        for (minimum, maximum) in [
            (IntegerValue::Signed(1), IntegerValue::Signed(0)),
            (IntegerValue::Signed(-129), IntegerValue::Signed(0)),
            (IntegerValue::Signed(0), IntegerValue::Signed(128)),
            (IntegerValue::Unsigned(0), IntegerValue::Unsigned(1)),
        ] {
            assert!(BoundedIntegerType::new(integer, minimum, maximum).is_err());
        }
        assert!(
            BoundedIntegerType::new(
                IntegerType::address(64).unwrap(),
                IntegerValue::Unsigned(0),
                IntegerValue::Unsigned(1)
            )
            .is_err()
        );
        let singleton = BoundedIntegerType::new(
            integer,
            IntegerValue::Signed(-128),
            IntegerValue::Signed(-128),
        )
        .unwrap();
        assert!(singleton.contains(IntegerValue::Signed(-128)));
        assert!(!singleton.contains(IntegerValue::Signed(-127)));
    }
}
