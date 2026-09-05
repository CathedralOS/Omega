use crate::declarations::{AliasName, BuildDeclarationKind, PackageKey};
use crate::resolution::graph::CanonicalSourceClosureSubjectFingerprint;
use crate::review::ReviewOnlyRootPolicyDisposition;
use crate::review::ReviewOnlyRootRoleContract;
use std::fmt;

/// One recorded project choice, not a fresh candidate-bound authorization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoricalPackagePolicyDecision {
    pub(super) subject: HistoricalPackagePolicyDecisionSubject,
    pub(super) conflict: [u8; 32],
    pub(super) disposition: ReviewOnlyRootPolicyDisposition,
}

impl HistoricalPackagePolicyDecision {
    /// Document reference into the associated source subject's sorted packages.
    /// This is not a compiler handle or an index into a later updated graph.
    pub const fn package_index(&self) -> Option<usize> {
        match &self.subject {
            HistoricalPackagePolicyDecisionSubject::CandidatePackage { package_index }
            | HistoricalPackagePolicyDecisionSubject::RootRole { package_index, .. }
            | HistoricalPackagePolicyDecisionSubject::SourceReplacement { package_index, .. } => {
                Some(*package_index)
            }
            HistoricalPackagePolicyDecisionSubject::RemovedPackage { .. } => None,
        }
    }

    pub const fn subject(&self) -> &HistoricalPackagePolicyDecisionSubject {
        &self.subject
    }

    /// V1 conflict fingerprint or V2 normalized obligation fingerprint.
    pub const fn obligation(&self) -> [u8; 32] {
        self.conflict
    }

    pub const fn conflict(&self) -> [u8; 32] {
        self.conflict
    }

    pub const fn disposition(&self) -> ReviewOnlyRootPolicyDisposition {
        self.disposition
    }
}

/// Exact history subject. A removed key is not an index into the candidate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HistoricalPackagePolicyDecisionSubject {
    CandidatePackage {
        package_index: usize,
    },
    RemovedPackage {
        key: PackageKey,
    },
    RootRole {
        package_index: usize,
        baseline_role: BuildDeclarationKind,
        candidate_role: BuildDeclarationKind,
        broken_contract: ReviewOnlyRootRoleContract,
    },
    SourceReplacement {
        baseline: PackageKey,
        package_index: usize,
        site: HistoricalPackagePolicyReplacementSite,
    },
}

/// One exact root or requester-local binding, never an inferred name pairing.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum HistoricalPackagePolicyReplacementSite {
    Root,
    Dependency {
        requester_index: usize,
        alias: AliasName,
    },
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
    /// None is the original V1 review-conflict history, never inferred V2.
    pub(super) comparison: Option<[u8; 32]>,
}

impl HistoricalPackagePolicyDecisions {
    pub const fn source_subject(&self) -> &CanonicalSourceClosureSubjectFingerprint {
        &self.source_subject
    }

    pub fn decisions(&self) -> &[HistoricalPackagePolicyDecision] {
        &self.decisions
    }

    pub const fn version(&self) -> u16 {
        if self.comparison.is_some() { 2 } else { 1 }
    }
    pub const fn comparison(&self) -> Option<[u8; 32]> {
        self.comparison
    }
}

/// Requested recovery storage and retained decision count for an enclosing
/// lock budget. Storage includes duplicate-fingerprint validation scratch;
/// input text and the already recovered source subject remain borrowed.
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

    pub(super) fn charge(
        &mut self,
        bytes: usize,
        maximum: usize,
    ) -> Result<(), HistoricalPackagePolicyError> {
        self.owned_bytes = self
            .owned_bytes
            .checked_add(bytes)
            .filter(|bytes| *bytes <= maximum)
            .ok_or(HistoricalPackagePolicyError::AllocationLimitExceeded)?;
        Ok(())
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
    InvalidSubject,
    SourceKey,
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
            Self::InvalidSubject => "historical policy subject contradicts its candidate graph or directional root role",
            Self::SourceKey => "historical policy has an invalid source-qualified package key",
        })
    }
}

impl std::error::Error for HistoricalPackagePolicyError {}
