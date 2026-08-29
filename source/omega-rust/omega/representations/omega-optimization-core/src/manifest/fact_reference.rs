//! Canonical closed vocabulary for evidence consumed by optimization rules.

use std::fmt;

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
    /// One closed fact-family tag followed by its exact 32-byte identity.
    pub const ENCODED_LENGTH: usize = 33;

    /// Encode one fact reference in the canonical representation shared by
    /// decision manifests and external policy schemas.
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

    /// Decode exactly one canonical fact reference. Framing belongs to the
    /// caller, so truncated and trailing inputs both reject by exact length.
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

impl fmt::Display for OptimizationFactReferenceDecodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid optimization fact reference encoding: {self:?}"
        )
    }
}

impl std::error::Error for OptimizationFactReferenceDecodeError {}
