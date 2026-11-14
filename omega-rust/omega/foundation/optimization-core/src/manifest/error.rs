// Construction (`InvalidOptimizationManifestRecord`) and wire decoding
// (`OptimizationManifestDecodeError`) errors shared across decision and pass
// manifest records.

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

crate::invalid_error_display!(
    InvalidOptimizationManifestRecord,
    "optimization manifest record"
);

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

crate::invalid_error_display!(
    OptimizationManifestDecodeError,
    "optimization manifest encoding"
);
