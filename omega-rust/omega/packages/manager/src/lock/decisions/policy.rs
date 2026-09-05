//! Complete-policy history: capture project choices without candidate indices.

mod text;
pub(super) use text::{HEADER, encode, recover};

use super::model::{
    HistoricalPackagePolicyDecision, HistoricalPackagePolicyDecisionSubject as Subject,
    HistoricalPackagePolicyDecisions, HistoricalPackagePolicyError as Error,
    HistoricalPackagePolicyLimits,
};
use crate::resolution::graph::CanonicalSourceClosureSubject;
use crate::review::{
    PackagePolicyChangeSet, PackagePolicyDecisionSubject, PackagePolicyResolution,
};

impl HistoricalPackagePolicyDecisions {
    /// Retain the exact completed choices and both source associations. Removed
    /// packages, root roles, and replacements refer to comparison subjects, not
    /// candidate-only package indices. A rejection is preserved, not published.
    /// No source/IR replay or proof-of-review stage is needed here.
    pub fn capture_policy(
        source: &CanonicalSourceClosureSubject,
        changes: &PackagePolicyChangeSet,
        resolution: &PackagePolicyResolution,
        limits: HistoricalPackagePolicyLimits,
    ) -> Result<Self, Error> {
        let limits = limits.bounded();
        if changes.candidate_source_subject() != source.fingerprint() {
            return Err(Error::SourceSubjectMismatch);
        }
        if resolution.comparison() != changes.fingerprint() {
            return Err(Error::ResolutionMismatch);
        }
        let supplied = resolution.decisions();
        if supplied.len() > limits.maximum_decisions {
            return Err(Error::DecisionLimitExceeded);
        }
        if supplied.len() > limits.maximum_bytes / "decision root-role accept\n".len() {
            return Err(Error::ByteLimitExceeded);
        }
        let mut decisions = Vec::new();
        decisions
            .try_reserve_exact(supplied.len())
            .map_err(|_| Error::AllocationFailed)?;
        decisions.extend(
            supplied
                .iter()
                .map(|decision| HistoricalPackagePolicyDecision {
                    subject: match decision.subject {
                        PackagePolicyDecisionSubject::RootRole => Subject::RootRole,
                        PackagePolicyDecisionSubject::SourceReplacement(digest) => {
                            Subject::SourceReplacement(digest)
                        }
                        PackagePolicyDecisionSubject::Row(digest) => Subject::Row(digest),
                    },
                    disposition: decision.disposition,
                }),
        );
        let history = Self {
            source_subject: source.fingerprint().clone(),
            baseline_source_subject: changes
                .baseline_source_subject()
                .map(|source| *source.as_bytes()),
            comparison: Some(resolution.comparison().digest()),
            decisions,
        };
        // Enforce the same representation ceiling at capture and persistence.
        encode(&history, limits)?;
        Ok(history)
    }
}
