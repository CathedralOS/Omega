//! Review the Product selected by this package Build.
use super::{
    BuildSnapshotRequest, CompileResolvedPackageReviewsError, CompilerIssuedPackageReview,
    CompletedOccurrence, OccurrenceFacts, PackageCompilationInputs, PackageReviewOccurrence,
    PackageReviewRound,
};

impl PackageReviewRound<'_> {
    pub(super) fn retain_product(
        &mut self,
        occurrence: PackageReviewOccurrence<'_>,
        inputs: PackageCompilationInputs,
        build_snapshot: BuildSnapshotRequest,
        product: project_build::PlannedProduct,
    ) -> Result<(), CompileResolvedPackageReviewsError> {
        let checked = self.compile_occurrence(&occurrence, &product)?;
        self.retain_occurrence(&occurrence, &inputs, &build_snapshot, checked)
    }

    fn compile_occurrence(
        &mut self,
        occurrence: &PackageReviewOccurrence<'_>,
        product: &project_build::PlannedProduct,
    ) -> Result<CompletedOccurrence, CompileResolvedPackageReviewsError> {
        let key = occurrence.key;
        let profile = occurrence.checked_target;
        let invalid = || {
            CompileResolvedPackageReviewsError::new(
                crate::review::CompileResolvedPackageReviewCause::IdentityMismatch {
                    package: key.clone(),
                },
            )
        };
        let compilation_error =
            |failure| occurrence.compilation_error(self.execution_profile, failure);
        let stage = product
            .product_stage(&[profile])
            .map_err(|diagnostics| compilation_error(diagnostics.into()))?;
        let checked = stage.review_policy(profile).map_err(compilation_error)?;
        self.accounting.append(checked.source_completion_accounting);
        let subject = checked.sources.package_subject().ok_or_else(invalid)?;
        let build_handoff = product.build_handoff(profile).map_err(compilation_error)?;
        let evaluated = product.evaluated();
        let build = &evaluated.configuration;
        let source_consumption_commitment = subject.source_consumption_commitment();
        if subject.root() != key.identity()
            || build_handoff.package() != subject.root()
            || build_handoff.source_consumption_commitment() != source_consumption_commitment
            || build_handoff.target() != profile
            || build_handoff.purpose() != occurrence.purpose
            || build_handoff.build_execution_profile() != self.execution_profile
            || build_handoff
                .selection_observation()
                .is_some_and(|selected| {
                    Some(selected)
                        != build
                            .observation_summary
                            .as_ref()
                            .map(|summary| summary.identity().digest())
                })
        {
            return Err(invalid());
        }
        let semantic_bindings = checked.sources.accepted_semantic_bindings().to_vec();
        let encoding = |error| {
            CompileResolvedPackageReviewsError::new(
                crate::review::CompileResolvedPackageReviewCause::Encoding {
                    package: key.clone(),
                    error,
                },
            )
        };
        let policy =
            super::super::super::review_set::encode_policy(&checked.policy).map_err(encoding)?;
        let policy_bytes = policy.bytes.len();
        let facts = OccurrenceFacts {
            consumed_units: subject.consumed_units().to_vec(),
            component_description: checked.component_description,
        };
        let preparations = evaluated
            .preparations
            .iter()
            .map(|(successor, stage)| {
                let executed = stage.executed_entry.as_ref().ok_or_else(invalid)?;
                let source = executed.declaration.source();
                let file = source.logical_filename().map_err(|_| invalid())?;
                Ok(super::super::super::review_set::ReviewedPreparation {
                    file,
                    callable: executed.callable_identity.clone(),
                    successor: successor.clone(),
                    observation: stage.observation_summary.clone().ok_or_else(invalid)?,
                })
            })
            .collect::<Result<_, CompileResolvedPackageReviewsError>>()?;
        let review = CompilerIssuedPackageReview {
            preparations,
            key: key.clone(),
            resolution: occurrence.custody.resolution().clone(),
            source_consumption_commitment,
            selected_build_machine_identity: checked.selected_build_machine_identity,
            build_evaluation_usage: build.evaluation_usage,
            build_observation_summary: build.observation_summary.clone(),
            restricted_build_requests: evaluated.restricted_build_requests.clone(),
            semantic_bindings,
            semantic_binding_candidates: checked.semantic_binding_candidates,
            build_handoff,
            policy: checked.policy,
            policy_rows: policy.rows,
            policy_commitment: policy.commitment,
        };
        Ok(CompletedOccurrence {
            review,
            facts: std::sync::Arc::new(facts),
            policy_bytes,
        })
    }
}
