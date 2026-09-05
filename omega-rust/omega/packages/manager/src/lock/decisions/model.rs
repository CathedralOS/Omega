use crate::resolution::graph::CanonicalSourceClosureSubjectFingerprint;
use crate::review::ReviewOnlyRootPolicyDisposition;
use std::fmt;

/// One recorded project choice, not a fresh candidate-bound authorization.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HistoricalPackagePolicyDecision {
    pub(super) package_index: usize,
    pub(super) conflict: [u8; 32],
    pub(super) disposition: ReviewOnlyRootPolicyDisposition,
}

impl HistoricalPackagePolicyDecision {
    /// Document reference into the associated source subject's sorted packages.
    /// This is not a compiler handle or an index into a later updated graph.
    pub const fn package_index(&self) -> usize {
        self.package_index
    }

    pub const fn conflict(&self) -> [u8; 32] {
        self.conflict
    }

    pub const fn disposition(&self) -> ReviewOnlyRootPolicyDisposition {
        self.disposition
    }
}

/// Inert policy history for one exact source subject, including its target.
///
/// Recovery trusts the project's authors to record decisions. It validates
/// structure and source association, not whether an audit occurred. There is
/// deliberately no conversion to a fresh root-policy resolution. Rejections
/// remain rejections; merely recording one cannot permit its publication.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoricalPackagePolicyDecisions {
    pub(super) source_subject: CanonicalSourceClosureSubjectFingerprint,
    pub(super) decisions: Vec<HistoricalPackagePolicyDecision>,
}

impl HistoricalPackagePolicyDecisions {
    pub const fn source_subject(&self) -> &CanonicalSourceClosureSubjectFingerprint {
        &self.source_subject
    }

    pub fn decisions(&self) -> &[HistoricalPackagePolicyDecision] {
        &self.decisions
    }
}

/// Callers may lower these ceilings but cannot raise the format's hard limits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HistoricalPackagePolicyLimits {
    pub(super) maximum_bytes: usize,
    pub(super) maximum_decisions: usize,
}

impl HistoricalPackagePolicyLimits {
    pub const fn new(maximum_bytes: usize, maximum_decisions: usize) -> Self {
        Self {
            maximum_bytes,
            maximum_decisions,
        }
    }

    pub(super) fn bounded(self) -> Self {
        Self::new(
            self.maximum_bytes.min(8 * 1024 * 1024),
            self.maximum_decisions.min(65_536),
        )
    }
}

impl Default for HistoricalPackagePolicyLimits {
    fn default() -> Self {
        Self::new(8 * 1024 * 1024, 65_536)
    }
}

/// Fixed diagnostics never echo authored record text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HistoricalPackagePolicyError {
    ByteLimitExceeded,
    DecisionLimitExceeded,
    AllocationFailed,
    InvalidFraming,
    UnsupportedVersion,
    SourceSubjectMismatch,
    UnknownPackage,
    NonCanonicalDecisions,
    ResolutionMismatch,
}

impl fmt::Display for HistoricalPackagePolicyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::ByteLimitExceeded => "historical policy exceeds the record-byte limit",
            Self::DecisionLimitExceeded => "historical policy exceeds the decision limit",
            Self::AllocationFailed => "historical policy allocation failed",
            Self::InvalidFraming => "historical policy has invalid canonical framing",
            Self::UnsupportedVersion => "unsupported historical policy version; retain the existing pins and recover with a compatible toolchain",
            Self::SourceSubjectMismatch => "historical policy belongs to a different source graph, root, or target",
            Self::UnknownPackage => "historical policy references a package outside its source graph",
            Self::NonCanonicalDecisions => "historical policy repeats or misorders decisions",
            Self::ResolutionMismatch => "historical policy capture requires the exact complete current resolution",
        })
    }
}

impl std::error::Error for HistoricalPackagePolicyError {}
