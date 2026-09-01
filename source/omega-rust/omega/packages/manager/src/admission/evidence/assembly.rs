use super::{
    AcceptedOrdinaryClosureEvidence, AcceptedOrdinaryEvidenceError,
    AcceptedOrdinaryEvidenceSchemaIdentity, AcceptedOrdinaryPackageEvidence,
};
use crate::resolution::graph::ResolvedPackageSourceClosure;
use crate::review::{
    CanonicalPackageReconstructionQuestionLimits, CompilerIssuedPackageReviewSet,
    ReviewOnlyCapabilityConflictLimits, ReviewOnlyRootPolicyResolution,
    bind_fresh_package_root_policy,
};
use std::collections::BTreeMap;

/// Rederive and accept the complete current ordinary evidence closure.
///
/// Decoded questions, baselines, fingerprints, and already-bound policy values
/// cannot enter as evidence. The gate starts from live resolver custody, fresh
/// compiler review, and root decisions, then independently reruns every
/// implemented admission join.
pub fn accept_ordinary_closure_evidence(
    closure: &ResolvedPackageSourceClosure,
    reviews: &CompilerIssuedPackageReviewSet,
    reconstruction_limits: CanonicalPackageReconstructionQuestionLimits,
    conflict_limits: ReviewOnlyCapabilityConflictLimits,
    root_policy: Option<&ReviewOnlyRootPolicyResolution>,
) -> Result<AcceptedOrdinaryClosureEvidence, AcceptedOrdinaryEvidenceError> {
    super::validation::revalidate_source_custody(closure)?;
    let acceptance = bind_fresh_package_root_policy(
        closure,
        reviews,
        reconstruction_limits,
        conflict_limits,
        root_policy,
    )
    .map_err(AcceptedOrdinaryEvidenceError::RootPolicy)?;

    let mut reviews_by_package = BTreeMap::new();
    for review in reviews.reviews() {
        if reviews_by_package.insert(review.key(), review).is_some() {
            return Err(AcceptedOrdinaryEvidenceError::ReviewAssociationMismatch(
                review.key().clone(),
            ));
        }
    }

    let obligation_entries = acceptance.obligations().entries();
    let question_entries = acceptance.obligations().question().entries();
    if obligation_entries.len() != question_entries.len() {
        return Err(AcceptedOrdinaryEvidenceError::AllocationFailed);
    }
    let mut packages = Vec::new();
    packages
        .try_reserve_exact(question_entries.len())
        .map_err(|_| AcceptedOrdinaryEvidenceError::AllocationFailed)?;
    for (question_entry, obligation_entry) in question_entries.iter().zip(obligation_entries) {
        let package = question_entry.package();
        let review = reviews_by_package
            .remove(package)
            .ok_or_else(|| AcceptedOrdinaryEvidenceError::MissingReview(package.clone()))?;
        let selected = acceptance
            .obligations()
            .question()
            .source_closure()
            .packages()
            .iter()
            .find(|selected| selected.key() == package)
            .ok_or_else(|| {
                AcceptedOrdinaryEvidenceError::ReviewAssociationMismatch(package.clone())
            })?;
        let generated_sources = review.generated_source_bundle();
        if review.resolution() != selected.resolution()
            || question_entry.obligations() != review.obligations()
            || obligation_entry.package() != package
            || obligation_entry.results() != review.obligation_results()
            || generated_sources.package() != package.identity()
            || generated_sources.target() != question_entry.obligations().target()
            || generated_sources.dependency_closure()
                != question_entry.obligations().dependency_closure()
            || generated_sources.source_consumption_commitment()
                != review.source_consumption_commitment()
        {
            return Err(AcceptedOrdinaryEvidenceError::ReviewAssociationMismatch(
                package.clone(),
            ));
        }
        packages.push(AcceptedOrdinaryPackageEvidence {
            package: package.clone(),
            resolution: selected.resolution().clone(),
            source_consumption: review.source_consumption_commitment(),
            selected_build_machine_identity: review.selected_build_machine_identity().to_owned(),
            build_evaluation_usage: review.build_evaluation_usage(),
            build_observation: review.build_observation_summary().cloned(),
            semantic_bindings: review.semantic_bindings().to_vec(),
            generated_sources: generated_sources.clone(),
            artifact: question_entry.obligations().clone(),
            results: obligation_entry.results().clone(),
        });
    }
    if !reviews_by_package.is_empty() {
        return Err(AcceptedOrdinaryEvidenceError::ReviewAssociationMismatch(
            reviews_by_package
                .into_keys()
                .next()
                .expect("nonempty map has a first key")
                .clone(),
        ));
    }

    Ok(AcceptedOrdinaryClosureEvidence {
        schema: AcceptedOrdinaryEvidenceSchemaIdentity::current(),
        packages,
        acceptance,
    })
}
