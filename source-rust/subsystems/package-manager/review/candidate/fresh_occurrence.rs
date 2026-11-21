//! Compile a cache miss using the Product opened by its original package Build.
use super::{
    AssembledOccurrence, CachedOccurrenceFacts, CompileResolvedPackageReviewsError,
    CompilerIssuedPackageReview, CompletedOccurrence, PackageCompilationInputs,
    PackageReviewOccurrence, PackageReviewRound, TargetEntryDiscovery, compiled_review_key,
};

impl PackageReviewRound<'_> {
    pub(super) fn retain_product(
        &mut self,
        occurrence: PackageReviewOccurrence<'_>,
        inputs: PackageCompilationInputs,
        assembled: AssembledOccurrence,
        product: project_build::PlannedProduct,
    ) -> Result<(), CompileResolvedPackageReviewsError> {
        let (checked, bytes) = self.compile_occurrence(&occurrence, &product)?;
        let cache_write = occurrence
            .custody
            .compiled_review_cache_dir()
            .and_then(|directory| {
                let cache =
                    super::super::super::review_cache::CompiledReviewCache::open(directory)?;
                let key = compiled_review_key(
                    occurrence.key.identity(),
                    occurrence.purpose,
                    occurrence.checked_target,
                    self.execution_profile,
                    &assembled.entry,
                    &inputs,
                    &assembled.build_snapshot,
                )?;
                let record = super::super::super::review_cache::record_compiled_review(
                    &checked.review,
                    (*checked.facts).clone(),
                    bytes,
                )?;
                Some((cache, key, record))
            });
        self.retain_occurrence(&occurrence, &inputs, &assembled, checked)?;
        if let Some((cache, key, record)) = cache_write {
            cache.store(&key, &record, false);
        }
        Ok(())
    }

    fn compile_occurrence(
        &mut self,
        occurrence: &PackageReviewOccurrence<'_>,
        product: &project_build::PlannedProduct,
    ) -> Result<(CompletedOccurrence, Vec<u8>), CompileResolvedPackageReviewsError> {
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
        let mut candidates = checked.semantic_binding_candidates;
        if self.discovery == TargetEntryDiscovery::Dependencies {
            for (purpose, candidate) in &self.target_entry_candidates {
                if *purpose == occurrence.purpose
                    && semantic_bindings.contains(candidate.binding())
                    && !candidates
                        .iter()
                        .any(|previous| previous.binding() == candidate.binding())
                {
                    candidates.push(candidate.clone());
                }
            }
        }
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
        let facts = CachedOccurrenceFacts {
            consumed_units: subject.consumed_units().to_vec(),
            source_consumption_commitment,
            component_description: checked.component_description,
            target_entry_candidate: checked.target_entry_candidate,
        };
        let review = CompilerIssuedPackageReview {
            key: key.clone(),
            resolution: occurrence.custody.resolution().clone(),
            source_consumption_commitment,
            selected_build_machine_identity: checked.selected_build_machine_identity,
            build_evaluation_usage: build.evaluation_usage,
            build_observation_summary: build.observation_summary.clone(),
            restricted_build_requests: evaluated.restricted_build_requests.clone(),
            semantic_bindings,
            semantic_binding_candidates: candidates,
            build_handoff,
            policy: checked.policy,
            policy_rows: policy.rows,
            policy_commitment: policy.commitment,
        };
        Ok((
            CompletedOccurrence {
                review,
                facts: Box::new(facts),
                policy_bytes,
            },
            policy.bytes,
        ))
    }
}
