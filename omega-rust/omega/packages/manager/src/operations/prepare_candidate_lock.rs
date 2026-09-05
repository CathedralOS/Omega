//! Prepare actual lock content from checked findings and current project policy.

mod error;
mod validation;

pub use error::PrepareCandidateLockError;

use crate::lock::{
    HistoricalPackagePolicyDecisions, HistoricalPackagePolicyLimits, PackageLockTarget,
};
use crate::resolution::graph::{
    CanonicalSourceClosureSubject, CanonicalSourceClosureSubjectLimits,
    ExactTargetPackageSourceClosure,
};
use crate::review::{
    CompilerIssuedPackageReviewSet, PackagePolicyChangeLimits, PackagePolicyDecisionLimits,
    PackagePolicyDecisionResolution, compare_package_policy_changes,
    resolve_package_policy_decisions,
};

/// Each existing owner retains its own hard ceilings. The history storage
/// ceiling covers retained decisions and temporary key/codec storage, not all
/// compiler review memory. Policy payloads are moved rather than cloned.
#[derive(Debug, Clone, Copy)]
pub struct PrepareCandidateLockLimits {
    pub comparison: PackagePolicyChangeLimits,
    pub decisions: PackagePolicyDecisionLimits,
    pub source: CanonicalSourceClosureSubjectLimits,
    pub history: HistoricalPackagePolicyLimits,
    pub maximum_history_owned_bytes: usize,
}
impl Default for PrepareCandidateLockLimits {
    fn default() -> Self {
        Self {
            comparison: Default::default(),
            decisions: Default::default(),
            source: Default::default(),
            history: Default::default(),
            maximum_history_owned_bytes: 256 * 1024 * 1024,
        }
    }
}

/// Prepare one target's concrete source pins, complete policy, and decision
/// history. This consumes final compiler findings without recompiling them.
///
/// Explicit project acceptance cannot discharge an open proof obligation.
/// Accepted assumptions and other disclosed trust are instead governed by the
/// exact normalized delta; unchanged accepted policy is not fresh admission.
///
/// Old source is never acquired. No project file is written, no native artifact
/// is produced, and no intermediate acceptance certificate is constructed.
/// Final transaction publication must still guard project-file versions and
/// revalidate candidate custody at its own immediate commit boundary.
pub fn prepare_candidate_lock_target(
    accepted: Option<&PackageLockTarget>,
    candidate_sources: &ExactTargetPackageSourceClosure<'_>,
    reviews: CompilerIssuedPackageReviewSet,
    resolution: &PackagePolicyDecisionResolution,
    limits: PrepareCandidateLockLimits,
) -> Result<PackageLockTarget, PrepareCandidateLockError> {
    let changes =
        compare_package_policy_changes(accepted, &reviews, candidate_sources, limits.comparison)
            .map_err(PrepareCandidateLockError::Comparison)?;
    let current =
        resolve_package_policy_decisions(&changes, resolution.decisions(), limits.decisions)
            .map_err(PrepareCandidateLockError::Decisions)?;
    if &current != resolution {
        return Err(PrepareCandidateLockError::ResolutionMismatch);
    }
    if !current.all_required_changes_accepted() {
        return Err(PrepareCandidateLockError::RejectedDecision);
    }
    validation::source_custody(candidate_sources)?;
    validation::obligations(&reviews, candidate_sources)?;
    let source = CanonicalSourceClosureSubject::from_resolved(candidate_sources, limits.source)
        .map_err(PrepareCandidateLockError::SourceSubject)?;
    let (history, _) = HistoricalPackagePolicyDecisions::capture_policy_changes_with_usage(
        &source,
        &changes,
        &current,
        limits.history,
        limits.maximum_history_owned_bytes.min(256 * 1024 * 1024),
    )
    .map_err(PrepareCandidateLockError::History)?;
    let baselines = reviews
        .into_policies_by_key()
        .map_err(|_| PrepareCandidateLockError::AllocationFailed)?;
    let target = PackageLockTarget::from_parts(source, baselines, history)
        .map_err(PrepareCandidateLockError::Lock)?;
    // Preparation can include significant bounded policy processing. Recheck
    // custody again before returning, without pretending this closes a future
    // filesystem transaction's concurrent-change window.
    validation::source_custody(candidate_sources)?;
    Ok(target)
}
