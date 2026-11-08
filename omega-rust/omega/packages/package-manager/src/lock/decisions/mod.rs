/*
Source-scoped historical policy decisions and their bounded text representation.

`HistoricalPackagePolicyDecisions` records inert policy history for one exact
source subject, including its target. Recovery trusts the project's authors to
record decisions and validates structure and source association rather than
whether an audit occurred. Complete-policy subjects (`RootRole`,
`SourceReplacement`, and `Row`) do not index the candidate graph so removed
packages and replaced roots need no invented owner. There is deliberately no
conversion to a fresh root-policy resolution: rejections remain rejections and
merely recording one cannot permit its publication.
*/

mod policy;
mod text;

use crate::resolution::graph::CanonicalSourceClosureSubjectFingerprint;
use crate::review::ReviewOnlyRootPolicyDisposition;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum HistoricalPackagePolicyDecisionSubject {
    LegacyConflict {
        package_index: usize,
        conflict: [u8; 32],
    },
    RootRole,
    SourceReplacement([u8; 32]),
    Row([u8; 32]),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct HistoricalPackagePolicyDecision {
    subject: HistoricalPackagePolicyDecisionSubject,
    disposition: ReviewOnlyRootPolicyDisposition,
}

impl HistoricalPackagePolicyDecision {
    pub const fn package_index(&self) -> Option<usize> {
        match self.subject {
            HistoricalPackagePolicyDecisionSubject::LegacyConflict { package_index, .. } => {
                Some(package_index)
            }
            _ => None,
        }
    }

    pub const fn conflict(&self) -> Option<[u8; 32]> {
        match self.subject {
            HistoricalPackagePolicyDecisionSubject::LegacyConflict { conflict, .. } => {
                Some(conflict)
            }
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct HistoricalPackagePolicyDecisions {
    source_subject: CanonicalSourceClosureSubjectFingerprint,
    baseline_source_subject: Option<[u8; 32]>,
    comparison: Option<[u8; 32]>,
    decisions: Vec<HistoricalPackagePolicyDecision>,
}

impl HistoricalPackagePolicyDecisions {
    pub const fn source_subject(&self) -> &CanonicalSourceClosureSubjectFingerprint {
        &self.source_subject
    }

    pub fn decisions(&self) -> &[HistoricalPackagePolicyDecision] {
        &self.decisions
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct HistoricalPackagePolicyRecoveryUsage {
    owned_bytes: usize,
    decisions: usize,
}

impl HistoricalPackagePolicyRecoveryUsage {
    pub const fn owned_bytes(&self) -> usize {
        self.owned_bytes
    }

    pub const fn decisions(&self) -> usize {
        self.decisions
    }

    fn for_decisions(
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

    fn for_policy_decisions(
        decisions: usize,
        maximum_owned_bytes: usize,
    ) -> Result<Self, HistoricalPackagePolicyError> {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct HistoricalPackagePolicyLimits {
    maximum_bytes: usize,
    maximum_decisions: usize,
}

impl HistoricalPackagePolicyLimits {
    pub const fn new(maximum_bytes: usize, maximum_decisions: usize) -> Self {
        Self {
            maximum_bytes,
            maximum_decisions,
        }
    }

    fn bounded(self) -> Self {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HistoricalPackagePolicyError {
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
