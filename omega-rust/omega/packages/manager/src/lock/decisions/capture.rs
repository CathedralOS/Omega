use super::model::{
    HistoricalPackagePolicyDecision, HistoricalPackagePolicyDecisionSubject,
    HistoricalPackagePolicyDecisions, HistoricalPackagePolicyError as Error,
    HistoricalPackagePolicyLimits,
};
use crate::resolution::graph::CanonicalSourceClosureSubject;
use crate::review::{ReviewOnlyCapabilityConflictSet, ReviewOnlyRootPolicyResolution};

impl HistoricalPackagePolicyDecisions {
    /// Record a completed fresh decision set against its exact source subject.
    /// No source or compiler replay data enters the resulting history.
    pub fn capture(
        subject: &CanonicalSourceClosureSubject,
        conflicts: &ReviewOnlyCapabilityConflictSet,
        resolution: Option<&ReviewOnlyRootPolicyResolution>,
        limits: HistoricalPackagePolicyLimits,
    ) -> Result<Self, Error> {
        let limits = limits.bounded();
        if conflicts.source_subject() != subject.fingerprint() {
            return Err(Error::SourceSubjectMismatch);
        }
        let supplied = resolution.map_or(&[][..], ReviewOnlyRootPolicyResolution::decisions);
        if supplied.len() > limits.maximum_decisions {
            return Err(Error::DecisionLimitExceeded);
        }
        if supplied.len() > limits.maximum_bytes / 83 {
            return Err(Error::ByteLimitExceeded);
        }
        let mut decisions = Vec::new();
        decisions
            .try_reserve_exact(supplied.len())
            .map_err(|_| Error::AllocationFailed)?;
        for package in conflicts.packages() {
            let package_index = subject
                .packages()
                .binary_search_by(|source| source.key().cmp(package.key()))
                .map_err(|_| Error::UnknownPackage)?;
            if subject.packages()[package_index].resolution() != package.candidate_resolution() {
                return Err(Error::SourceSubjectMismatch);
            }
            for conflict in package
                .conflicts()
                .iter()
                .filter(|conflict| conflict.is_blocking())
            {
                let Some(resolution) = resolution else {
                    return Err(Error::ResolutionMismatch);
                };
                if resolution.candidate_closure() != package.candidate_closure() {
                    return Err(Error::ResolutionMismatch);
                }
                let index = supplied
                    .binary_search_by_key(&conflict.fingerprint(), |decision| decision.conflict())
                    .map_err(|_| Error::ResolutionMismatch)?;
                let decision = supplied[index];
                if decisions.len() >= supplied.len() {
                    return Err(Error::ResolutionMismatch);
                }
                decisions.push(HistoricalPackagePolicyDecision {
                    subject: HistoricalPackagePolicyDecisionSubject::LegacyConflict {
                        package_index,
                        conflict: decision.conflict().digest(),
                    },
                    disposition: decision.disposition(),
                });
            }
        }
        if decisions.len() != supplied.len() {
            return Err(Error::ResolutionMismatch);
        }
        decisions.sort_unstable_by_key(|decision| decision.subject);
        let historical = Self {
            source_subject: subject.fingerprint().clone(),
            baseline_source_subject: None,
            comparison: None,
            decisions,
        };
        // Apply the same byte ceiling at capture as at persistence.
        historical.canonical_text(subject, limits)?;
        Ok(historical)
    }
}
