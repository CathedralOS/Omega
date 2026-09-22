//! Construction and decoding failures shared by manifest record families.

use std::fmt;

use crate::CoreContractDecodeError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InvalidOptimizationManifestRecord {
    AppliedWithoutValidator,
    NonCanonicalConsumedFacts,
    DuplicateRuleIdentity,
    RuleSetIdentityMismatch,
    DecisionNamesUnscheduledRule,
    DuplicateDecisionIdentity,
    DuplicateCandidateIdentity,
}

impl fmt::Display for InvalidOptimizationManifestRecord {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid optimization manifest record: {self:?}")
    }
}

impl std::error::Error for InvalidOptimizationManifestRecord {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptimizationManifestDecodeError {
    Truncated,
    WrongLength { expected: usize, actual: usize },
    WrongMagic,
    UnsupportedVersion(u32),
    InvalidOptionalTag(u8),
    UnknownFactReference(u8),
    DecisionIdentityMismatch,
    TrailingBytes,
    CoreContract(CoreContractDecodeError),
    InvalidRecord(InvalidOptimizationManifestRecord),
}

impl fmt::Display for OptimizationManifestDecodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid optimization manifest encoding: {self:?}"
        )
    }
}

impl std::error::Error for OptimizationManifestDecodeError {}
