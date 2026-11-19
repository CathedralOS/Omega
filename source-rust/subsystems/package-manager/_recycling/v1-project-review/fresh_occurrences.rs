pub(super) fn projection_failure(
    key: &PackageKey,
    error: PackageReviewProjectionError,
) -> CompileResolvedPackageReviewsError {
    use PackageReviewProjectionError as Error;
    match error {
        Error::SourceConsumptionMissing => CompileResolvedPackageReviewsError::new(
            crate::review::CompileResolvedPackageReviewCause::SourceConsumptionMissing {
                package: key.clone(),
            },
        ),
        Error::Projection(diagnostics) => CompileResolvedPackageReviewsError::new(
            crate::review::CompileResolvedPackageReviewCause::Projection {
                package: key.clone(),
                diagnostics,
            },
        ),
        Error::IdentityMismatch => CompileResolvedPackageReviewsError::new(
            crate::review::CompileResolvedPackageReviewCause::IdentityMismatch {
                package: key.clone(),
            },
        ),
        Error::Encoding(error) => CompileResolvedPackageReviewsError::new(
            crate::review::CompileResolvedPackageReviewCause::Encoding {
                package: key.clone(),
                error,
            },
        ),
        Error::PolicyBudget => CompileResolvedPackageReviewsError::new(
            crate::review::CompileResolvedPackageReviewCause::RetainedPolicyCanonicalBudget {
                package: key.clone(),
                maximum_bytes: MAXIMUM_RETAINED_POLICY_CANONICAL_BYTES,
            },
        ),
        Error::SemanticBinding(error) => projection_error(key, error),
    }
}

    pub fn component(
        &self,
        completion: &PackageReviewCompletion<'_>,
        inputs: &PackageCompilationInputs,
        component: semantic_vocabulary::PackageKeyIdentity,
    ) -> Result<Option<PackageReviewComponent>, CompileResolvedPackageReviewsError> {
        let occurrence = &completion.occurrence;
        if component == occurrence.key.identity() {
            return Ok(None);
        }
        match self
            .component_candidates
            .get(&(component, occurrence.purpose))
        {
            Some(OccurrenceCompile::Live(source)) => {
                Ok(Some(PackageReviewComponent::Source(source.clone())))
            }
            Some(OccurrenceCompile::Cached(facts)) => {
                let description = facts.component_description.clone().ok_or_else(|| {
                    CompileResolvedPackageReviewsError::new(crate::review::CompileResolvedPackageReviewCause::ComponentDescriptionNotRetained {
                        package: occurrence.key.clone(),
                        component,
                        purpose: occurrence.purpose,
                    })
                })?;
                Ok(Some(PackageReviewComponent::Description(description)))
            }
            None => Err(CompileResolvedPackageReviewsError::new(
                crate::review::CompileResolvedPackageReviewCause::Compilation {
                    package: occurrence.key.clone(),
                    failure: CompileFailure::Diagnostics(vec![Diagnostic::error(format!(
                        "an independent provider selection names dependency package `{}`, which this closure never compiled",
                        inputs.package_name(component).unwrap_or("<unnamed>"),
                    ))]),
                },
            )),
        }
    }

    pub fn retain(
        &mut self,
        completion: PackageReviewCompletion<'_>,
        product: compiler::PackageReviewProduct,
        occurrence_inputs: PackageCompilationInputs,
    ) -> Result<Option<PackageReviewCacheWrite>, CompileResolvedPackageReviewsError> {
        let accounting = product.source.source_completion_accounting().clone();
        self.retain_product(completion, product, occurrence_inputs)
            .map_err(|error| error.with_source_completion_accounting(accounting))
    }

    fn retain_product(
        &mut self,
        completion: PackageReviewCompletion<'_>,
        product: compiler::PackageReviewProduct,
        occurrence_inputs: PackageCompilationInputs,
    ) -> Result<Option<PackageReviewCacheWrite>, CompileResolvedPackageReviewsError> {
        let PackageReviewCompletion {
            occurrence,
            assembled,
            ..
        } = completion;
        let PackageReviewOccurrence {
            key,
            custody,
            position,
            purpose,
            checked_target,
            ..
        } = occurrence;
        let closure = self.closure;
        let execution_profile = self.execution_profile;
        let AssembledOccurrence {
            entry,
            build_snapshot,
            proposed_entry,
        } = assembled;
        let entry = entry.as_path();
        let compiler::PackageReviewProduct { evidence, source } = product;
        let PackageReviewEvidence {
            source_consumption_commitment,
            selected_build_machine_identity,
            build_observation_summary,
            restricted_build_requests,
            build_evaluation_usage,
            semantic_bindings,
            service_binding_candidates,
            hosted_exit_binding_candidates,
            build_handoff,
            policy,
            policy_canonical_bytes,
        } = evidence;
        if build_handoff.package() != key.identity() || build_handoff.target() != checked_target {
            return Err(CompileResolvedPackageReviewsError::new(
                crate::review::CompileResolvedPackageReviewCause::IdentityMismatch {
                    package: key.clone(),
                },
            ));
        }
        self.retained_policy_canonical_total =
            reserve(self.retained_policy_canonical_total, policy_canonical_bytes).ok_or_else(
                || {
                    CompileResolvedPackageReviewsError::new(
                crate::review::CompileResolvedPackageReviewCause::RetainedPolicyCanonicalBudget {
                    package: key.clone(),
                    maximum_bytes: MAXIMUM_RETAINED_POLICY_CANONICAL_BYTES,
                },
            )
                },
            )?;
        let mut semantic_binding_candidates = service_binding_candidates;
        semantic_binding_candidates.extend(proposed_entry);
        if self.discovery == TargetEntryDiscovery::Dependencies {
            for (candidate_purpose, candidate) in &self.target_entry_candidates {
                if *candidate_purpose == purpose
                    && semantic_bindings
                        .iter()
                        .any(|binding| binding == candidate.binding())
                    && !semantic_binding_candidates
                        .iter()
                        .any(|existing| existing.binding() == candidate.binding())
                {
                    semantic_binding_candidates.push(candidate.clone());
                }
            }
        }
        semantic_binding_candidates.extend(hosted_exit_binding_candidates);
        if self.discovery == TargetEntryDiscovery::Dependencies {
            self.target_entry_candidates.extend(
                source
                    .target_entry_candidate()
                    .map_err(|error| projection_error(key, error))?
                    .map(|candidate| (purpose, candidate)),
            );
        }
        let (policy_rows, policy_commitment) =
            super::super::review_set::policy_rows_and_commitment(&policy).map_err(|error| {
                CompileResolvedPackageReviewsError::new(
                    crate::review::CompileResolvedPackageReviewCause::Encoding {
                        package: key.clone(),
                        error,
                    },
                )
            })?;
        let review = CompilerIssuedPackageReview {
            key: key.clone(),
            resolution: custody.resolution().clone(),
            source_consumption_commitment,
            selected_build_machine_identity,
            build_evaluation_usage,
            build_observation_summary,
            restricted_build_requests,
            semantic_bindings,
            semantic_binding_candidates,
            build_handoff: build_handoff.clone(),
            policy,
            policy_rows,
            policy_commitment,
        };
        let mut cache_write = None;
        if let Some(directory) = custody.compiled_review_cache_dir()
            && let Some(cache) = super::super::review_cache::CompiledReviewCache::open(directory)
            && let Some(consumed_units) = source.facts().consumed_units.as_ref()
            && let Ok(target_entry_candidate) = source.target_entry_candidate()
        {
            let demanded = self.fresh_components.contains(&(key.identity(), purpose));
            if let Some(record) = super::super::review_cache::record_compiled_review(
                &review,
                super::super::review_cache::CachedOccurrenceFacts {
                    consumed_units: consumed_units.to_vec(),
                    source_consumption_commitment,
                    component_description: None,
                    target_entry_candidate,
                },
                policy_canonical_bytes,
            ) && let Some(cache_key) = compiled_review_key(
                key.identity(),
                purpose,
                checked_target,
                execution_profile,
                entry,
                &occurrence_inputs,
                &build_snapshot,
            ) {
                cache_write = Some(PackageReviewCacheWrite {
                    cache,
                    key: cache_key,
                    record,
                    component_source: demanded.then(|| source.clone()),
                });
            }
        }
        let review_position = self.reviews.len();
        self.review_positions[position][purpose_slot(purpose)] = Some(review_position);
        self.reviews.push(review);
        self.occurrences.push(RetainedOccurrenceCompile {
            position,
            purpose,
            entry: entry.to_path_buf(),
            inputs: occurrence_inputs,
            build_snapshot,
            policy_canonical_bytes,
            review: review_position,
            compilation: OccurrenceCompile::Live(source.clone()),
            compiled_in_pass: true,
        });
        if key != closure.graph().root() {
            self.component_candidates
                .insert((key.identity(), purpose), OccurrenceCompile::Live(source));
        }
        Ok(cache_write)
    }

pub struct PackageReviewWork<'occurrence> {
    pub request: PackageReviewRequest,
    pub completion: PackageReviewCompletion<'occurrence>,
    pub maximum_policy_canonical_bytes: usize,
}

pub struct PackageReviewRequest {
    pub inputs: PackageCompilationInputs,
    pub execution: package_compilation::build_execution::BuildExecutionContract,
    pub target: target::TargetProfile,
    pub entry: std::path::PathBuf,
}

pub struct PackageReviewCompletion<'occurrence> {
    occurrence: PackageReviewOccurrence<'occurrence>,
    assembled: AssembledOccurrence,
    closure: &'occurrence ResolvedPackageSourceClosure,
    execution_profile: Option<target::TargetProfile>,
}

impl PackageReviewCompletion<'_> {
    pub fn compilation_failure(
        &self,
        failure: CompileFailure,
    ) -> CompileResolvedPackageReviewsError {
        let occurrence = &self.occurrence;
        CompileResolvedPackageReviewsError::from_compilation(
            occurrence.key.clone(),
            PackageCheckedContext::new(
                occurrence.purpose,
                occurrence.checked_target,
                self.execution_profile,
            ),
            self.closure.dependency_path(occurrence.key),
            failure,
        )
    }

    pub fn input_failure(
        &self,
        errors: Vec<PackageCompilationInputError>,
    ) -> CompileResolvedPackageReviewsError {
        CompileResolvedPackageReviewsError::compilation_inputs(self.occurrence.key, errors)
    }

    pub fn review_failure(
        &self,
        failure: compiler::PackageReviewFailure,
    ) -> CompileResolvedPackageReviewsError {
        match failure {
            compiler::PackageReviewFailure::Compilation(failure) => {
                self.compilation_failure(failure)
            }
            compiler::PackageReviewFailure::Projection {
                failure,
                accounting,
            } => projection_failure(self.occurrence.key, failure)
                .with_source_completion_accounting(accounting),
        }
    }
}

pub enum PackageReviewComponent {
    Source(compiler::PackageReviewSource),
    Description(IndependentComponentDescription),
}
