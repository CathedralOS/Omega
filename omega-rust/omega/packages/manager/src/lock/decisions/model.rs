use crate::resolution::graph::CanonicalSourceClosureSubjectFingerprint;
use crate::review::ReviewOnlyRootPolicyDisposition;
use std::fmt;

/// A retained change coordinate. Complete-policy subjects do not index the
/// candidate graph: removed packages and replaced roots need no invented owner.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum HistoricalPackagePolicyDecisionSubject {
    LegacyConflict {
        package_index: usize,
        conflict: [u8; 32],
    },
    RootRole,
    SourceReplacement([u8; 32]),
    Row([u8; 32]),
}

/// One recorded project choice, not a fresh candidate-bound authorization.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HistoricalPackagePolicyDecision {
    pub(super) subject: HistoricalPackagePolicyDecisionSubject,
    pub(super) disposition: ReviewOnlyRootPolicyDisposition,
}

impl HistoricalPackagePolicyDecision {
    pub const fn subject(&self) -> HistoricalPackagePolicyDecisionSubject {
        self.subject
    }

    /// Version 1 document reference only, never an index for a modern change.
    pub const fn package_index(&self) -> Option<usize> {
        match self.subject {
            HistoricalPackagePolicyDecisionSubject::LegacyConflict { package_index, .. } => {
                Some(package_index)
            }
            _ => None,
        }
    }

    /// Version 1 conflict only. Modern records expose their typed `subject`.
    pub const fn conflict(&self) -> Option<[u8; 32]> {
        match self.subject {
            HistoricalPackagePolicyDecisionSubject::LegacyConflict { conflict, .. } => {
                Some(conflict)
            }
            _ => None,
        }
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
    pub(super) baseline_source_subject: Option<[u8; 32]>,
    pub(super) comparison: Option<[u8; 32]>,
    pub(super) decisions: Vec<HistoricalPackagePolicyDecision>,
}

impl HistoricalPackagePolicyDecisions {
    pub const fn source_subject(&self) -> &CanonicalSourceClosureSubjectFingerprint {
        &self.source_subject
    }

    pub fn decisions(&self) -> &[HistoricalPackagePolicyDecision] {
        &self.decisions
    }

    /// Exact full-policy comparison retained by version 2. Version 1 history
    /// did not store this comparison and is never upgraded by inventing one.
    pub const fn comparison(&self) -> Option<[u8; 32]> {
        self.comparison
    }

    /// Prior source association, absent for fresh review and version 1 history.
    /// Read this alongside `comparison` to distinguish those cases.
    pub const fn baseline_source_subject(&self) -> Option<[u8; 32]> {
        self.baseline_source_subject
    }
}

/// Requested recovery storage and retained decision count for an enclosing
/// lock budget. Version 1 also counts duplicate-fingerprint validation scratch;
/// version 2 needs no scratch. Input text and the source subject remain borrowed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HistoricalPackagePolicyRecoveryUsage {
    pub(super) owned_bytes: usize,
    pub(super) decisions: usize,
}

impl HistoricalPackagePolicyRecoveryUsage {
    pub const fn owned_bytes(&self) -> usize {
        self.owned_bytes
    }

    pub const fn decisions(&self) -> usize {
        self.decisions
    }

    pub(super) fn for_decisions(
        decisions: usize,
        maximum_owned_bytes: usize,
    ) -> Result<Self, HistoricalPackagePolicyError> {
        let owned_bytes = decisions
            .checked_mul(std::mem::size_of::<HistoricalPackagePolicyDecision>() + 32)
            .filter(|bytes| *bytes <= maximum_owned_bytes)
            .ok_or(HistoricalPackagePolicyError::AllocationLimitExceeded)?;
        Ok(Self {
            owned_bytes,
            decisions,
        })
    }

    pub(super) fn for_policy_decisions(
        decisions: usize,
        maximum_owned_bytes: usize,
    ) -> Result<Self, HistoricalPackagePolicyError> {
        // Version 2 checks strict subject order in one pass, without scratch.
        let owned_bytes = decisions
            .checked_mul(std::mem::size_of::<HistoricalPackagePolicyDecision>())
            .filter(|bytes| *bytes <= maximum_owned_bytes)
            .ok_or(HistoricalPackagePolicyError::AllocationLimitExceeded)?;
        Ok(Self {
            owned_bytes,
            decisions,
        })
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
    AllocationLimitExceeded,
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
            Self::AllocationLimitExceeded => "historical policy exceeds the owned-storage limit",
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
