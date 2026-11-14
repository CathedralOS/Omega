//! Published integer descriptors contain a concrete carrier, never a width
//! binder. Closing describes the exact type already judged; it cannot select
//! a width, substitute an argument, or change the module's meaning.

use crate::{
    IeeeFloatFormat, IntegerCarrier, IntegerSign, IntegerType, IntegerValue, IntegerWidth,
    PropositionError, ScalarType, WidthParameter,
};
use numerics::integer_carrier::FixedInteger;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntegerWidthClosureError {
    Parameter(WidthParameter),
    InvalidWidth(u16),
    SignedAddress,
}

impl std::fmt::Display for IntegerWidthClosureError {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Parameter(parameter) => write!(
                out,
                "unresolved integer width parameter {} in machine {}",
                parameter.value.get(),
                parameter.owner.get()
            ),
            Self::InvalidWidth(bits) => write!(
                out,
                "integer width {bits} is outside the current host integer representation"
            ),
            Self::SignedAddress => out.write_str("an address carrier must be unsigned"),
        }
    }
}

impl std::error::Error for IntegerWidthClosureError {}

/// Closed descriptor for the current host-backed integer engine. Its 128-bit
/// backing limit does not limit the language's positive-u64 width family.
/// `FixedInteger` remains the sole owner of numerical sign and positive width;
/// the separate address kind retains its exact address-model width.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ClosedIntegerType {
    carrier: IntegerCarrier,
    fixed: FixedInteger,
}

impl TryFrom<IntegerType> for ClosedIntegerType {
    type Error = IntegerWidthClosureError;

    fn try_from(integer: IntegerType) -> Result<Self, Self::Error> {
        let bits = match integer.width() {
            IntegerWidth::Bits(bits) => bits,
            IntegerWidth::Parameter(parameter) => {
                return Err(IntegerWidthClosureError::Parameter(parameter));
            }
        };
        if !(1..=128).contains(&bits) {
            return Err(IntegerWidthClosureError::InvalidWidth(bits));
        }
        if integer.is_address() && integer.sign() != IntegerSign::Unsigned {
            return Err(IntegerWidthClosureError::SignedAddress);
        }
        Ok(Self {
            carrier: integer.carrier(),
            fixed: FixedInteger::new(integer.sign(), u64::from(bits))
                .expect("validated positive width"),
        })
    }
}

impl ClosedIntegerType {
    pub fn new(sign: IntegerSign, bits: u16) -> Result<Self, PropositionError> {
        Ok(Self::try_from(IntegerType::new(sign, bits)?).expect("validated closed integer type"))
    }

    pub fn address(bits: u16) -> Result<Self, PropositionError> {
        Ok(Self::try_from(IntegerType::address(bits)?).expect("validated closed address type"))
    }

    pub fn from_fixed(fixed: FixedInteger) -> Option<Self> {
        IntegerType::from_fixed(fixed)?.try_into().ok()
    }

    pub const fn supported_address_representation() -> Self {
        Self {
            carrier: IntegerCarrier::Address,
            fixed: FixedInteger::unsigned(64),
        }
    }

    pub const fn carrier(self) -> IntegerCarrier {
        self.carrier
    }

    pub const fn sign(self) -> IntegerSign {
        self.fixed.sign()
    }

    pub const fn is_address(self) -> bool {
        matches!(self.carrier, IntegerCarrier::Address)
    }

    pub fn bits(self) -> u16 {
        u16::try_from(self.fixed.width()).expect("closed host width")
    }

    pub fn as_checking(self) -> IntegerType {
        match self.carrier {
            IntegerCarrier::Fixed => {
                IntegerType::from_fixed(self.fixed).expect("closed host-backed fixed integer")
            }
            IntegerCarrier::Address => {
                IntegerType::address(self.bits()).expect("closed address width")
            }
        }
    }

    pub fn minimum_value(self) -> IntegerValue {
        self.as_checking()
            .minimum_value()
            .expect("closed host width has an endpoint")
    }

    pub fn maximum_value(self) -> IntegerValue {
        self.as_checking()
            .maximum_value()
            .expect("closed host width has an endpoint")
    }
}

impl From<ClosedIntegerType> for IntegerType {
    fn from(integer: ClosedIntegerType) -> Self {
        integer.as_checking()
    }
}

impl std::hash::Hash for ClosedIntegerType {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        std::hash::Hash::hash(&self.as_checking(), state);
    }
}

/// A view of a scalar after width closure, with no independent type identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ClosedScalarType {
    Boolean,
    Integer(ClosedIntegerType),
    IeeeFloat(IeeeFloatFormat),
}

impl TryFrom<ScalarType> for ClosedScalarType {
    type Error = IntegerWidthClosureError;

    fn try_from(scalar: ScalarType) -> Result<Self, Self::Error> {
        Ok(match scalar {
            ScalarType::Boolean => Self::Boolean,
            ScalarType::Integer(integer) => Self::Integer(integer.try_into()?),
            ScalarType::IeeeFloat(format) => Self::IeeeFloat(format),
        })
    }
}

impl ClosedScalarType {
    pub const fn as_integer(self) -> Option<ClosedIntegerType> {
        match self {
            Self::Integer(integer) => Some(integer),
            _ => None,
        }
    }

    pub fn as_checking(self) -> ScalarType {
        match self {
            Self::Boolean => ScalarType::Boolean,
            Self::Integer(integer) => ScalarType::Integer(integer.as_checking()),
            Self::IeeeFloat(format) => ScalarType::IeeeFloat(format),
        }
    }
}

impl From<ClosedScalarType> for ScalarType {
    fn from(scalar: ClosedScalarType) -> Self {
        scalar.as_checking()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{MachineId, ValueId};
    use std::hash::{Hash, Hasher};

    #[derive(Default)]
    struct HashBytes(Vec<u8>);

    impl Hasher for HashBytes {
        fn finish(&self) -> u64 {
            0
        }

        fn write(&mut self, bytes: &[u8]) {
            self.0.extend_from_slice(bytes);
        }
    }

    #[test]
    fn closed_descriptors_preserve_legacy_hash_and_width_identity() {
        for sign in [IntegerSign::Signed, IntegerSign::Unsigned] {
            for bits in [1, 8, 13, 32, 64, 128] {
                let raw = IntegerType::new(sign, bits).unwrap();
                let closed = ClosedIntegerType::try_from(raw).unwrap();
                assert_eq!(closed.as_checking(), raw);
                assert_eq!(closed.bits(), bits);
                let mut expected = HashBytes::default();
                IntegerCarrier::Fixed.hash(&mut expected);
                sign.hash(&mut expected);
                bits.hash(&mut expected);
                let mut actual = HashBytes::default();
                raw.hash(&mut actual);
                assert_eq!(actual.0, expected.0);
                let mut projected = HashBytes::default();
                closed.hash(&mut projected);
                assert_eq!(projected.0, expected.0);
                let mut identity = Vec::new();
                raw.width()
                    .write_identity(|part| identity.extend_from_slice(part));
                assert_eq!(identity, bits.to_le_bytes());
            }
        }
    }

    #[test]
    fn rigid_widths_keep_owner_identity_and_grant_no_concrete_authority() {
        let parameter = WidthParameter {
            owner: MachineId::new(1).unwrap(),
            value: ValueId::new(2).unwrap(),
        };
        let other_owner = WidthParameter {
            owner: MachineId::new(2).unwrap(),
            ..parameter
        };
        for sign in [IntegerSign::Signed, IntegerSign::Unsigned] {
            let raw = IntegerType::parameterized(sign, parameter);
            assert_ne!(raw, IntegerType::parameterized(sign, other_owner));
            assert_eq!(raw.bits(), None);
            assert_eq!(raw.minimum_value(), None);
            assert_eq!(raw.maximum_value(), None);
            assert!(!raw.admits(raw.zero()));
            let closed = IntegerType::new(sign, 64).unwrap();
            assert!(!raw.can_widen_to(closed));
            assert!(!closed.can_widen_to(raw));
            assert_eq!(raw.exact_add(raw.zero(), raw.zero()), None);
            assert_eq!(raw.wrapping_add(raw.zero(), raw.zero()), None);
            assert_eq!(
                ClosedIntegerType::try_from(raw),
                Err(IntegerWidthClosureError::Parameter(parameter))
            );
            assert_eq!(
                ClosedScalarType::try_from(ScalarType::Integer(raw)),
                Err(IntegerWidthClosureError::Parameter(parameter))
            );
        }
    }

    #[test]
    fn closed_address_width_and_one_bit_signed_range_are_exact() {
        for bits in [1, 24, 64, 128] {
            let address = ClosedIntegerType::address(bits).unwrap();
            let count = ClosedIntegerType::new(IntegerSign::Unsigned, bits).unwrap();
            assert_eq!(address.bits(), bits);
            assert!(address.is_address());
            assert!(!count.is_address());
            assert_ne!(address, count);
        }
        let signed = ClosedIntegerType::new(IntegerSign::Signed, 1).unwrap();
        assert_eq!(signed.minimum_value(), IntegerValue::Signed(-1));
        assert_eq!(signed.maximum_value(), IntegerValue::Signed(0));
        assert!(!signed.as_checking().admits(IntegerValue::Signed(1)));
        assert_eq!(
            signed
                .as_checking()
                .exact_mul(IntegerValue::Signed(-1), IntegerValue::Signed(-1)),
            None
        );
        let wider = FixedInteger::unsigned(257);
        assert_eq!(wider.width(), 257);
        assert_eq!(ClosedIntegerType::from_fixed(wider), None);
    }
}
