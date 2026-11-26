//! Reuse a complete package review only when its exact source, Build inputs, target and decisions match.
//! A missing occurrence requires a new project compilation before it can become a review.

use crate::build_session::dependency_first_package_order;
use package_compilation::build_execution::BuildSnapshotRequest;
use source_completion_accounting::SourceCompletionAccounting;

use super::super::{
    CompileResolvedPackageReviewsError, CompilerIssuedPackageReview, CompilerIssuedPackageReviewSet,
};
use crate::declarations::PackageKey;
use crate::declarations::{DependencyPurpose, purpose_slot};
use crate::lock::PackageOccurrenceRoster;
use crate::resolution::PackageCompilationScope;
use crate::resolution::graph::{ExactTargetPackageSourceClosure, ResolvedPackageSourceClosure};
use crate::resolution::source::PackageSourceCustody;
use diagnostics::Diagnostic;
use package_compilation::{
    ConsumedSourceUnit, IndependentComponentDescription, PackageCompilationInputError,
    PackageCompilationInputs,
    derive_source_consumption_commitment,
};
use provider_acceptance::AcceptedSemanticBinding;
use std::collections::BTreeMap;
use std::sync::Arc;

use super::PackageSourceBindings;

#[path = "fresh_occurrence.rs"]
mod fresh;

pub(super) const MAXIMUM_RETAINED_POLICY_CANONICAL_BYTES: usize = 64 * 1024 * 1024;

pub(super) fn reserve(current: usize, additional: usize) -> Option<usize> {
    current
        .checked_add(additional)
        .filter(|total| *total <= MAXIMUM_RETAINED_POLICY_CANONICAL_BYTES)
}

pub(super) struct RetainedPackageReviews {
    pub(super) reviews: CompilerIssuedPackageReviewSet,
    pub(super) occurrences: Vec<RetainedOccurrenceCompile>,
    pub(super) carried: usize,
    accounting: SourceCompletionAccounting,
}

impl RetainedPackageReviews {
    pub fn source_completion_accounting(&self) -> SourceCompletionAccounting {
        self.accounting.clone()
    }
}

#[derive(Debug, Clone)]
pub(super) struct OccurrenceFacts {
    consumed_units: Vec<ConsumedSourceUnit>,
    component_description: Option<IndependentComponentDescription>,
}

#[derive(Debug, Clone)]
pub(super) struct RetainedOccurrenceCompile {
    pub(super) position: usize, // Position in the dependency-first occurrence order.
    pub(super) purpose: DependencyPurpose,
    pub(super) entry: std::path::PathBuf, // The checked entry path this occurrence's compile consumed.
    pub(super) inputs: PackageCompilationInputs, // Complete inputs after resolved semantic bindings and dependency handoffs joined.
    pub(super) build_snapshot: BuildSnapshotRequest, // Source inventory and output roster captured by build evaluation.
    pub(super) policy_canonical_bytes: usize, // Canonical-policy bytes added to the running retained-policy total.
    pub(super) review: usize, // Index of this occurrence's review inside the pass's review vector.
    pub(super) compilation: Arc<OccurrenceFacts>,
}

pub(super) struct PackageReviewPass<'pass> {
    closure: &'pass ResolvedPackageSourceClosure,
    packages: Vec<ReviewPackage<'pass>>,
    execution_profile: Option<target::TargetProfile>,
    roster: &'pass PackageOccurrenceRoster,
    semantic_bindings_by_consumer:
        &'pass BTreeMap<(PackageKey, DependencyPurpose), Vec<AcceptedSemanticBinding>>,
    root_build_snapshot: Option<&'pass BuildSnapshotRequest>,
    prior: Option<&'pass RetainedPackageReviews>,
}

struct ReviewPackage<'pass> {
    scope: PackageCompilationScope<'pass>,
    custody: &'pass PackageSourceCustody,
    position: usize,
    purposes: Vec<(DependencyPurpose, target::TargetProfile)>,
}

impl<'pass> PackageReviewPass<'pass> {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn admit(
        target_closure: &'pass ExactTargetPackageSourceClosure<'_>,
        execution_profile: Option<target::TargetProfile>,
        roster: &'pass PackageOccurrenceRoster,
        semantic_bindings_by_consumer: &'pass BTreeMap<
            (PackageKey, DependencyPurpose),
            Vec<AcceptedSemanticBinding>,
        >,
        root_build_snapshot: Option<&'pass BuildSnapshotRequest>,
        prior: Option<&'pass RetainedPackageReviews>,
    ) -> Result<Self, CompileResolvedPackageReviewsError> {
        let closure = target_closure.source_closure();
        if execution_profile.is_none()
            && let Some(coverage) = roster
                .coverages()
                .iter()
                .find(|coverage| coverage.purposes().contains(&DependencyPurpose::Build))
        {
            return Err(CompileResolvedPackageReviewsError::new(
                crate::review::CompileResolvedPackageReviewCause::UnsupportedBuildActivation {
                    package: coverage.package().clone(),
                    reason: "build dependencies require an admitted build execution profile",
                },
            ));
        }
        let mut packages = Vec::new();
        for key in dependency_first_package_order(closure) {
            let missing = || {
                CompileResolvedPackageReviewsError::new(
                    crate::review::CompileResolvedPackageReviewCause::IdentityMismatch {
                        package: key.clone(),
                    },
                )
            };
            let position = closure.graph().package_position(&key).ok_or_else(missing)?;
            let custody = closure.custody(&key).ok_or_else(missing)?;
            let requested = roster.purposes(&key).ok_or_else(missing)?;
            let mut purposes = Vec::with_capacity(requested.len());
            for &purpose in requested {
                let checked_target = if purpose.is_product() {
                    target_closure.target_profile()
                } else {
                    execution_profile.ok_or_else(|| CompileResolvedPackageReviewsError::new(crate::review::CompileResolvedPackageReviewCause::UnsupportedBuildActivation {
                        package: key.clone(),
                        reason: "build dependencies require an admitted build execution profile",
                    }))?
                };
                purposes.push((purpose, checked_target));
            }
            packages.push(ReviewPackage {
                scope: PackageCompilationScope::new(closure, custody.key()),
                custody,
                position,
                purposes,
            });
        }
        Ok(Self {
            closure,
            packages,
            execution_profile,
            roster,
            semantic_bindings_by_consumer,
            root_build_snapshot,
            prior,
        })
    }

    fn occurrences(&self) -> impl Iterator<Item = PackageReviewOccurrence<'_>> {
        self.packages.iter().flat_map(|package| {
            package
                .purposes
                .iter()
                .map(move |&(purpose, checked_target)| {
                    let key = package.custody.key();
                    PackageReviewOccurrence {
                        key,
                        scope: &package.scope,
                        custody: package.custody,
                        position: package.position,
                        purpose,
                        checked_target,
                        _timing: crate::review::timings::subject_stage(|| {
                            format!("package {} purpose={purpose:?}", key.name().as_str())
                        }),
                    }
                })
        })
    }

    /// Review dependencies before their consumers. A matching pass from this invocation
    /// supplies its completed review. Each other occurrence opens its original Build product.
    pub fn review(
        self,
        source_bindings: &mut PackageSourceBindings,
        builds: &crate::project::PackageReviewBuilds,
    ) -> Result<RetainedPackageReviews, CompileResolvedPackageReviewsError> {
        let mut round = self.begin_round(source_bindings);
        for occurrence in self.occurrences() {
            if let Err(error) = round.review_occurrence(occurrence, builds) {
                return Err(error.with_source_completion_accounting(round.accounting));
            }
        }
        let accounting = round.accounting.clone();
        round
            .finish_reviews()
            .map_err(|error| error.with_source_completion_accounting(accounting))
    }

    fn begin_round<'round>(
        &'round self,
        source_bindings: &'round mut PackageSourceBindings,
    ) -> PackageReviewRound<'round> {
        PackageReviewRound {
            closure: self.closure,
            execution_profile: self.execution_profile,
            semantic_bindings_by_consumer: self.semantic_bindings_by_consumer,
            root_build_snapshot: self.root_build_snapshot,
            prior: self.prior,
            source_bindings,
            reviews: Vec::with_capacity(self.roster.occurrence_count()),
            review_positions: vec![[None; 2]; self.closure.graph().packages().len()],
            occurrences: Vec::new(),
            carried_occurrences: 0,
            accounting: Default::default(),
            retained_policy_canonical_total: 0,
        }
    }
}

struct PackageReviewRound<'pass> {
    closure: &'pass ResolvedPackageSourceClosure,
    execution_profile: Option<target::TargetProfile>,
    semantic_bindings_by_consumer:
        &'pass BTreeMap<(PackageKey, DependencyPurpose), Vec<AcceptedSemanticBinding>>,
    root_build_snapshot: Option<&'pass BuildSnapshotRequest>,
    prior: Option<&'pass RetainedPackageReviews>,
    source_bindings: &'pass mut PackageSourceBindings,
    reviews: Vec<CompilerIssuedPackageReview>,
    review_positions: Vec<[Option<usize>; 2]>, // Index into `reviews` of each package position's review, by purpose slot.
    occurrences: Vec<RetainedOccurrenceCompile>,
    carried_occurrences: usize, // Occurrences carried forward instead of compiled.
    accounting: SourceCompletionAccounting,
    retained_policy_canonical_total: usize,
}

struct PackageReviewOccurrence<'occurrence> {
    key: &'occurrence PackageKey,
    scope: &'occurrence PackageCompilationScope<'occurrence>,
    _timing: Option<crate::review::timings::StageTiming>,
    custody: &'occurrence PackageSourceCustody,
    position: usize,
    purpose: DependencyPurpose,
    checked_target: target::TargetProfile,
}

impl PackageReviewOccurrence<'_> {
    fn compilation_error(
        &self,
        execution_profile: Option<target::TargetProfile>,
        failure: build_evaluation_result::CompileFailure,
    ) -> CompileResolvedPackageReviewsError {
        CompileResolvedPackageReviewsError::from_compilation(
            self.key.clone(),
            crate::lock::PackageCheckedContext::new(
                self.purpose,
                self.checked_target,
                execution_profile,
            ),
            self.scope.closure().dependency_path(self.key),
            failure,
        )
    }
}

struct CompletedOccurrence {
    review: CompilerIssuedPackageReview,
    facts: Arc<OccurrenceFacts>,
    policy_bytes: usize,
}

struct AssembledOccurrence {
    entry: std::path::PathBuf,
    build_snapshot: BuildSnapshotRequest,
}

impl<'pass> PackageReviewRound<'pass> {
    fn review_occurrence(
        &mut self,
        occurrence: PackageReviewOccurrence<'_>,
        builds: &crate::project::PackageReviewBuilds,
    ) -> Result<(), CompileResolvedPackageReviewsError> {
        let (inputs, assembled) = self.assemble_occurrence_inputs(&occurrence)?;
        let product = builds
            .product(&inputs)
            .map_err(|failure| occurrence.compilation_error(self.execution_profile, failure))?;
        if self.carry_prior_compile(&occurrence, &inputs, &assembled, &product)? {
            return Ok(());
        }
        self.retain_product(occurrence, inputs, assembled, product)
    }

    fn assemble_occurrence_inputs(
        &mut self,
        occurrence: &PackageReviewOccurrence<'_>,
    ) -> Result<(PackageCompilationInputs, AssembledOccurrence), CompileResolvedPackageReviewsError>
    {
        let PackageReviewOccurrence {
            key,
            scope,
            custody,
            position,
            purpose,
            ..
        } = *occurrence;
        let closure = self.closure;
        let refused = |errors: Vec<PackageCompilationInputError>| {
            CompileResolvedPackageReviewsError::compilation_inputs(key, errors)
        };
        let inputs = scope
            .compilation_inputs_reusing(&mut self.source_bindings.root_bindings[position])
            .and_then(|inputs| inputs.with_compilation_purpose(purpose))
            .map_err(refused)?;
        let semantic_bindings = self
            .semantic_bindings_by_consumer
            .get(&(key.clone(), purpose))
            .cloned()
            .unwrap_or_default();
        let inputs = inputs
            .with_accepted_semantic_bindings(semantic_bindings)
            .map_err(refused)?;
        let required_instances = inputs.required_dependency_source_instances();
        let mut dependency_bundles = Vec::with_capacity(required_instances.len());
        let mut component_descriptions = Vec::new();
        for dependency in scope
            .packages()
            .iter()
            .filter(|dependency| *dependency != key)
        {
            for dependency_purpose in DependencyPurpose::ALL {
                if !required_instances.contains(&(dependency.identity(), dependency_purpose)) {
                    continue;
                }
                let dependency_position =
                    closure
                        .graph()
                        .package_position(dependency)
                        .and_then(|position| {
                            self.review_positions[position][purpose_slot(dependency_purpose)]
                        });
                let dependency_review =
                    dependency_position.and_then(|position| self.reviews.get(position));
                let review =
                    dependency_review.ok_or(PackageCompilationInputError::MissingBuildHandoff {
                        package: dependency.identity(),
                    });
                let review = review.map_err(|error| {
                    CompileResolvedPackageReviewsError::new(
                        crate::review::CompileResolvedPackageReviewCause::CompilationInputs {
                            package: key.clone(),
                            errors: vec![error],
                        },
                    )
                })?;
                let custody = closure
                    .custody(dependency)
                    .ok_or(PackageCompilationInputError::ForeignBuildHandoff {
                        package: dependency.identity(),
                    })
                    .map_err(|error| {
                        CompileResolvedPackageReviewsError::new(
                            crate::review::CompileResolvedPackageReviewCause::CompilationInputs {
                                package: key.clone(),
                                errors: vec![error],
                            },
                        )
                    })?;
                let bundle = review.build_handoff();
                if review.resolution() != custody.resolution()
                    || bundle.package() != dependency.identity()
                    || bundle.source_consumption_commitment()
                        != review.source_consumption_commitment()
                {
                    return Err(CompileResolvedPackageReviewsError::new(
                        crate::review::CompileResolvedPackageReviewCause::CompilationInputs {
                            package: key.clone(),
                            errors: vec![
                                PackageCompilationInputError::BuildHandoffCustodyMismatch {
                                    package: dependency.identity(),
                                },
                            ],
                        },
                    ));
                }
                if dependency_purpose.is_product()
                    && bundle.target() == occurrence.checked_target
                    && let Some(description) = dependency_position
                        .and_then(|index| self.occurrences.get(index))
                        .and_then(|entry| entry.compilation.component_description.as_ref())
                {
                    if description.package() != dependency.identity() {
                        return Err(refused(vec![
                            PackageCompilationInputError::BuildHandoffCustodyMismatch {
                                package: dependency.identity(),
                            },
                        ]));
                    }
                    component_descriptions.push(description.clone());
                }
                dependency_bundles.push(bundle.clone());
            }
        }
        let inputs = inputs
            .with_complete_dependency_build_handoffs(dependency_bundles)
            .and_then(|inputs| {
                inputs.with_independent_component_descriptions(component_descriptions)
            })
            .map_err(|errors| {
                CompileResolvedPackageReviewsError::new(
                    crate::review::CompileResolvedPackageReviewCause::CompilationInputs {
                        package: key.clone(),
                        errors,
                    },
                )
            })?;
        let main_entry = custody.snapshot_root().join("main.omg");
        let entry = if main_entry.is_file() {
            main_entry
        } else {
            custody.snapshot_root().join("build.omg")
        };
        let build_snapshot = if key == closure.graph().root() {
            self.root_build_snapshot.cloned()
        } else {
            None
        }
        .unwrap_or_else(|| BuildSnapshotRequest::new(std::iter::empty::<Vec<u8>>()));
        Ok((
            inputs,
            AssembledOccurrence {
                entry,
                build_snapshot,
            },
        ))
    }

    fn carry_prior_compile(
        &mut self,
        occurrence: &PackageReviewOccurrence<'_>,
        unanswered_inputs: &PackageCompilationInputs,
        assembled: &AssembledOccurrence,
        product: &project_build::PlannedProduct,
    ) -> Result<bool, CompileResolvedPackageReviewsError> {
        let PackageReviewOccurrence {
            position,
            purpose,
            checked_target,
            ..
        } = *occurrence;
        let entry = assembled.entry.as_path();
        let build_snapshot = &assembled.build_snapshot;
        let carried = self.prior.and_then(|prior| {
            let occurrence = prior.occurrences.iter().find(|occurrence| {
                occurrence.position == position
                    && occurrence.purpose == purpose
                    && occurrence.entry == *entry
                    && occurrence.inputs == *unanswered_inputs
                    && occurrence.build_snapshot == *build_snapshot
            })?;
            let review = prior.reviews.reviews().get(occurrence.review)?;
            Some((occurrence, review))
        });
        let Some((previous, review)) = carried else {
            return Ok(false);
        };
        let evaluated = product.evaluated();
        // Source and snapshot identity can match even if this Build's restricted
        // grants, observations, generated source or provider decisions changed.
        if evaluated.restricted_build_requests != review.restricted_build_requests()
            || evaluated.configuration.evaluation_usage != review.build_evaluation_usage()
            || evaluated.configuration.observation_summary.as_ref()
                != review.build_observation_summary()
            || &product
                .build_handoff(checked_target)
                .map_err(|failure| occurrence.compilation_error(self.execution_profile, failure))?
                != review.build_handoff()
        {
            return Ok(false);
        }
        self.retain_occurrence(
            occurrence,
            unanswered_inputs,
            assembled,
            CompletedOccurrence {
                review: review.clone(),
                facts: Arc::clone(&previous.compilation),
                policy_bytes: previous.policy_canonical_bytes,
            },
        )?;
        self.carried_occurrences += 1;
        Ok(true)
    }

    fn retain_occurrence(
        &mut self,
        occurrence: &PackageReviewOccurrence<'_>,
        unanswered_inputs: &PackageCompilationInputs,
        assembled: &AssembledOccurrence,
        completed: CompletedOccurrence,
    ) -> Result<(), CompileResolvedPackageReviewsError> {
        let PackageReviewOccurrence {
            key,
            position,
            purpose,
            ..
        } = *occurrence;
        let CompletedOccurrence {
            review,
            facts: compile,
            policy_bytes: policy_canonical_bytes,
        } = completed;
        let entry = &assembled.entry;
        let build_snapshot = &assembled.build_snapshot;
        derive_source_consumption_commitment(&compile.consumed_units, unanswered_inputs).and_then(|commitment| {
            if commitment == review.source_consumption_commitment() {
                Ok(())
            } else {
                Err(vec![Diagnostic::error("retained review source consumption differs from the current package inputs")])
            }
        })
            .map_err(|diagnostics| {
                CompileResolvedPackageReviewsError::new(
                    crate::review::CompileResolvedPackageReviewCause::SourceConsumptionDrift {
                        package: key.clone(),
                        diagnostics,
                    },
                )
            })?;
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
        let review_position = self.reviews.len();
        self.review_positions[position][purpose_slot(purpose)] = Some(review_position);
        self.reviews.push(review);
        self.occurrences.push(RetainedOccurrenceCompile {
            position,
            purpose,
            entry: entry.to_path_buf(),
            inputs: unanswered_inputs.clone(),
            build_snapshot: build_snapshot.clone(),
            policy_canonical_bytes,
            review: review_position,
            compilation: compile,
        });
        Ok(())
    }

    fn finish_reviews(
        mut self,
    ) -> Result<RetainedPackageReviews, CompileResolvedPackageReviewsError> {
        let root = self.closure.graph().root();
        let services = self
            .reviews
            .iter()
            .find(|review| &review.key == root && review.build_handoff.purpose().is_product())
            .ok_or_else(|| {
                CompileResolvedPackageReviewsError::new(
                    crate::review::CompileResolvedPackageReviewCause::IdentityMismatch {
                        package: root.clone(),
                    },
                )
            })?
            .policy
            .service_audits()
            .to_vec();
        let mut total = 0;
        for occurrence in &mut self.occurrences {
            let review = &mut self.reviews[occurrence.review];
            let length = if review.policy.service_audits() != services {
                review.policy.apply_service_audit_policy(&services);
                let encoded =
                    super::super::review_set::encode_policy(&review.policy).map_err(|error| {
                        CompileResolvedPackageReviewsError::new(
                            crate::review::CompileResolvedPackageReviewCause::Encoding {
                                package: review.key.clone(),
                                error,
                            },
                        )
                    })?;
                review.policy_rows = encoded.rows;
                review.policy_commitment = encoded.commitment;
                encoded.bytes.len()
            } else {
                occurrence.policy_canonical_bytes
            };
            total = reserve(total, length).ok_or_else(|| {
                CompileResolvedPackageReviewsError::new(crate::review::CompileResolvedPackageReviewCause::RetainedPolicyCanonicalBudget {
                    package: review.key.clone(),
                    maximum_bytes: MAXIMUM_RETAINED_POLICY_CANONICAL_BYTES,
                })
            })?;
            occurrence.policy_canonical_bytes = length;
        }
        Ok(self.into_retained())
    }

    fn into_retained(self) -> RetainedPackageReviews {
        RetainedPackageReviews {
            reviews: CompilerIssuedPackageReviewSet {
                reviews: self.reviews,
            },
            occurrences: self.occurrences,
            carried: self.carried_occurrences,
            accounting: self.accounting,
        }
    }
}
