use std::fmt;

/// Fixed diagnostics do not echo hostile record text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackagePolicyDecisionError {
    ChangeLimitExceeded,
    DecisionLimitExceeded,
    ByteLimitExceeded,
    OwnedLimitExceeded,
    LengthOverflow,
    AllocationFailed,
    WrongChangeSet,
    ForeignPackage,
    StaleOrForeignObligation,
    NonBlockingRow,
    DuplicateObligation,
    DuplicateDecision,
    MissingDecision,
    UnsupportedVersion,
    InvalidFraming,
    InvalidFingerprint,
    InvalidDisposition,
    NonCanonicalDecisions,
    ResolutionFingerprintMismatch,
}
impl fmt::Display for PackagePolicyDecisionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::ChangeLimitExceeded => "policy decisions exceed the comparison-scan limit",
            Self::DecisionLimitExceeded => "policy decisions exceed the decision limit",
            Self::ByteLimitExceeded => "policy decisions exceed the record-byte limit",
            Self::OwnedLimitExceeded => "policy decisions exceed the requested-storage limit",
            Self::LengthOverflow => "policy decision size overflow",
            Self::AllocationFailed => "policy decision allocation failed",
            Self::WrongChangeSet => "policy decision belongs to a different exact comparison",
            Self::ForeignPackage => "policy decision does not belong to this package",
            Self::StaleOrForeignObligation => {
                "policy decision references a stale or foreign obligation"
            }
            Self::NonBlockingRow => "an audit-only or unchanged row is not a policy obligation",
            Self::DuplicateObligation => "comparison repeats an exact policy obligation",
            Self::DuplicateDecision => "policy decisions repeat an obligation",
            Self::MissingDecision => "policy decisions omit a required obligation",
            Self::UnsupportedVersion => "unsupported normalized policy decision version",
            Self::InvalidFraming => "invalid canonical policy decision framing",
            Self::InvalidFingerprint => "invalid canonical policy decision fingerprint",
            Self::InvalidDisposition => "invalid closed policy decision disposition",
            Self::NonCanonicalDecisions => "policy decisions are not in canonical order",
            Self::ResolutionFingerprintMismatch => "policy decision resolution fingerprint differs",
        })
    }
}
impl std::error::Error for PackagePolicyDecisionError {}
