// `OptimizationFactReference`: 33-byte closed tagged union (`1` = ScalarConstant,
// `2` = AcceptedObligation, `3` = OwnershipFrontier, `4` = ValueRange) over
// 32-byte fact identities consumed by optimization rules.

use crate::{
    AcceptedObligationFactIdentity, OwnershipFrontierFactIdentity, ScalarConstantFactIdentity,
    ValueRangeFactIdentity,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum OptimizationFactReference {
    ScalarConstant(ScalarConstantFactIdentity),
    AcceptedObligation(AcceptedObligationFactIdentity),
    OwnershipFrontier(OwnershipFrontierFactIdentity),
    ValueRange(ValueRangeFactIdentity),
}

impl OptimizationFactReference {
    pub(crate) const ENCODED_LENGTH: usize = 33;

    pub fn encode(self) -> [u8; Self::ENCODED_LENGTH] {
        let (tag, identity) = match self {
            Self::ScalarConstant(identity) => (1, identity.bytes()),
            Self::AcceptedObligation(identity) => (2, identity.bytes()),
            Self::OwnershipFrontier(identity) => (3, identity.bytes()),
            Self::ValueRange(identity) => (4, identity.bytes()),
        };
        let mut encoded = [0; Self::ENCODED_LENGTH];
        encoded[0] = tag;
        encoded[1..].copy_from_slice(&identity);
        encoded
    }

    pub fn decode(encoded: &[u8]) -> Result<Self, OptimizationFactReferenceDecodeError> {
        if encoded.len() != Self::ENCODED_LENGTH {
            return Err(OptimizationFactReferenceDecodeError::WrongLength {
                expected: Self::ENCODED_LENGTH,
                actual: encoded.len(),
            });
        }
        let identity: [u8; 32] = encoded[1..]
            .try_into()
            .expect("the exact fact-reference length leaves a 32-byte identity");
        match encoded[0] {
            1 => Ok(Self::ScalarConstant(
                ScalarConstantFactIdentity::from_bytes(identity),
            )),
            2 => Ok(Self::AcceptedObligation(
                AcceptedObligationFactIdentity::from_bytes(identity),
            )),
            3 => Ok(Self::OwnershipFrontier(
                OwnershipFrontierFactIdentity::from_bytes(identity),
            )),
            4 => Ok(Self::ValueRange(ValueRangeFactIdentity::from_bytes(
                identity,
            ))),
            tag => Err(OptimizationFactReferenceDecodeError::UnknownTag(tag)),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptimizationFactReferenceDecodeError {
    WrongLength { expected: usize, actual: usize },
    UnknownTag(u8),
}

crate::invalid_error_display!(
    OptimizationFactReferenceDecodeError,
    "optimization fact reference encoding"
);
