use super::{
    AcceptedOrdinaryClosureEvidence, AcceptedOrdinaryEvidenceError,
    AcceptedOrdinaryEvidenceSchemaIdentity, AcceptedOrdinaryPackageEvidence,
};
use crate::resolution::graph::ExactTargetPackageSourceClosure;
use crate::review::reconstruction::bind_root_policy_with_associated_reviews;
use crate::review::{
    CanonicalPackageReconstructionQuestionLimits, CompilerIssuedPackageReviewSet,
    ReviewOnlyCapabilityConflictLimits, ReviewOnlyRootPolicyResolution,
};

/// Assemble the current checked closure with exact root-owned decisions.
///
/// Revalidate live source custody, then share one exact review association
/// through question, result, policy, and payload assembly. Decoded summaries
/// cannot replace the current source/review inputs or project decisions.
pub fn accept_ordinary_closure_evidence(
    target_closure: &ExactTargetPackageSourceClosure<'_>,
    reviews: &CompilerIssuedPackageReviewSet,
    reconstruction_limits: CanonicalPackageReconstructionQuestionLimits,
    conflict_limits: ReviewOnlyCapabilityConflictLimits,
    root_policy: Option<&ReviewOnlyRootPolicyResolution>,
) -> Result<AcceptedOrdinaryClosureEvidence, AcceptedOrdinaryEvidenceError> {
    let closure = target_closure.source_closure();
    super::validation::revalidate_source_custody(closure)?;
    let (acceptance, associated_reviews) = bind_root_policy_with_associated_reviews(
        target_closure,
        reviews,
        reconstruction_limits,
        conflict_limits,
        root_policy,
    )
    .map_err(AcceptedOrdinaryEvidenceError::RootPolicy)?;

    let mut packages = Vec::new();
    packages
        .try_reserve_exact(associated_reviews.len())
        .map_err(|_| AcceptedOrdinaryEvidenceError::AllocationFailed)?;
    for review in associated_reviews {
        let package = review.key();
        let generated_sources = review.generated_source_bundle();
        if generated_sources.package() != package.identity()
            || generated_sources.target() != review.obligations().target()
            || generated_sources.dependency_closure() != review.obligations().dependency_closure()
            || generated_sources.source_consumption_commitment()
                != review.source_consumption_commitment()
        {
            return Err(AcceptedOrdinaryEvidenceError::ReviewAssociationMismatch(
                package.clone(),
            ));
        }
        packages.push(AcceptedOrdinaryPackageEvidence {
            package: package.clone(),
            resolution: review.resolution().clone(),
            source_consumption: review.source_consumption_commitment(),
            selected_build_machine_identity: review.selected_build_machine_identity().to_owned(),
            build_evaluation_usage: review.build_evaluation_usage(),
            build_observation: review.build_observation_summary().cloned(),
            semantic_bindings: review.semantic_bindings().to_vec(),
            generated_sources: generated_sources.clone(),
            artifact: review.obligations().clone(),
            results: review.obligation_results().clone(),
        });
    }

    Ok(AcceptedOrdinaryClosureEvidence {
        schema: AcceptedOrdinaryEvidenceSchemaIdentity::current(),
        packages,
        acceptance,
    })
}
