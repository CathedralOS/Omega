/*
    We admit review occurrences in dependency-first order and join their
    compiler-issued evidence without executing compilers directly. Acquisition
    and the immutable parse checkpoint remain package-owned, while Build and
    Product occurrences each have their own target, generated sources, semantic
    bindings, and review.

    For each occurrence in a `PackageReviewRound`, we assemble its complete
    compilation inputs (`assemble_occurrence_inputs`), carry forward an
    identical compile from a prior pass or the persistent review cache when the
    complete inputs and build snapshot match (`carry_prior_compile`), or emit a
    `PackageReviewWork` request for fresh compilation and then retain the
    resulting review (`retain`). Aggregate canonical-policy bytes across the
    closure are bounded by `MAXIMUM_RETAINED_POLICY_CANONICAL_BYTES` (64 MiB).
*/

use crate::build_session::dependency_first_package_order;
use compilation_products::CompileFailure;

use super::super::semantic_bindings::projection_error;
use super::super::{
    CompileResolvedPackageReviewsError, CompilerIssuedPackageReview, CompilerIssuedPackageReviewSet,
};
use crate::declarations::PackageKey;
use crate::declarations::{DependencyPurpose, purpose_slot};
use crate::lock::{PackageCheckedContext, PackageOccurrenceRoster};
use crate::resolution::PackageCompilationScope;
use crate::resolution::graph::{ExactTargetPackageSourceClosure, ResolvedPackageSourceClosure};
use crate::resolution::source::PackageSourceCustody;
use diagnostics::Diagnostic;
use package_compilation::{
    AcceptedSemanticBinding, IndependentComponentDescription, PackageCompilationInputError,
    PackageCompilationInputs, derive_source_consumption_commitment,
};
use package_evidence::PackageReviewProjectionError;
use package_evidence::record::PackageReviewEvidence;
use package_evidence::record::SemanticBindingReviewCandidate;
use std::collections::BTreeMap;

use super::super::review_cache::{PackageReviewCacheWrite, compiled_review_key};
use super::PackageSourceBindings;
use std::collections::BTreeSet;
use std::iter::empty;

pub(super) const MAXIMUM_RETAINED_POLICY_CANONICAL_BYTES: usize = 64 * 1024 * 1024;

pub(super) fn projection_failure(
    key: &PackageKey,
    error: PackageReviewProjectionError,
) -> CompileResolvedPackageReviewsError {
    use PackageReviewProjectionError as Error;
    match error {
        Error::SourceConsumptionMissing => {
            CompileResolvedPackageReviewsError::SourceConsumptionMissing {
                package: key.clone(),
            }
        }
        Error::Projection(diagnostics) => CompileResolvedPackageReviewsError::Projection {
            package: key.clone(),
            diagnostics,
        },
        Error::IdentityMismatch => CompileResolvedPackageReviewsError::IdentityMismatch {
            package: key.clone(),
        },
        Error::Encoding(error) => CompileResolvedPackageReviewsError::Encoding {
            package: key.clone(),
            error,
        },
        Error::PolicyBudget => CompileResolvedPackageReviewsError::RetainedPolicyCanonicalBudget {
            package: key.clone(),
            maximum_bytes: MAXIMUM_RETAINED_POLICY_CANONICAL_BYTES,
        },
        Error::SemanticBinding(error) => projection_error(key, error),
    }
}

pub(super) fn reserve(current: usize, additional: usize) -> Option<usize> {
    current
        .checked_add(additional)
        .filter(|total| *total <= MAXIMUM_RETAINED_POLICY_CANONICAL_BYTES)
}

pub struct RetainedPackageReviews {
    pub(super) reviews: CompilerIssuedPackageReviewSet,
    pub(super) occurrences: Vec<RetainedOccurrenceCompile>,
    pub(super) carried: usize,
}

#[derive(Debug, Clone)]
pub(super) struct RetainedOccurrenceCompile {
    pub(super) position: usize, // Position in the dependency-first occurrence order.
    pub(super) purpose: DependencyPurpose,
    pub(super) entry: std::path::PathBuf, // The checked entry path this occurrence's compile consumed.
    pub(super) inputs: PackageCompilationInputs, // Complete inputs after resolved semantic bindings and dependency handoffs joined.
    pub(super) build_snapshot: build_settings::BuildSnapshotRequest, // Source inventory and output roster captured by build evaluation.
    pub(super) policy_canonical_bytes: usize, // Canonical-policy bytes added to the running retained-policy total.
    pub(super) review: usize, // Index of this occurrence's review inside the pass's review vector.
    pub(super) compilation: OccurrenceCompile, // Opaque live source custody or source-free facts recovered from cache.
    compiled_in_pass: bool,
}

#[derive(Debug, Clone)]
pub(super) enum OccurrenceCompile {
    Live(compiler::PackageReviewSource),
    Cached(Box<super::super::review_cache::CachedOccurrenceFacts>),
}

impl OccurrenceCompile {
    fn target_entry_candidate(
        &self,
        owner: &PackageKey,
    ) -> Result<Option<SemanticBindingReviewCandidate>, CompileResolvedPackageReviewsError> {
        match self {
            Self::Live(source) => source
                .target_entry_candidate()
                .map_err(|error| projection_error(owner, error)),
            Self::Cached(facts) => Ok(facts.target_entry_candidate.clone()),
        }
    }

    fn verify_current_source_consumption(
        &self,
        inputs: &PackageCompilationInputs,
    ) -> Result<(), Vec<Diagnostic>> {
        match self {
            Self::Live(_) => Ok(()),
            Self::Cached(facts) => {
                let commitment =
                    derive_source_consumption_commitment(&facts.consumed_units, inputs)?;
                if commitment != facts.source_consumption_commitment {
                    return Err(vec![Diagnostic::error(
                        "a persisted review's retained source-consumption commitment does not re-derive over its retained consumed units and the live inputs",
                    )]);
                }
                Ok(())
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum TargetEntryDiscovery {
    Disabled,
    Dependencies,
}

pub struct PackageReviewPass<'pass> {
    closure: &'pass ResolvedPackageSourceClosure,
    packages: Vec<ReviewPackage<'pass>>,
    execution_profile: Option<target::TargetProfile>,
    roster: &'pass PackageOccurrenceRoster,
    semantic_bindings_by_consumer:
        &'pass BTreeMap<(PackageKey, DependencyPurpose), Vec<AcceptedSemanticBinding>>,
    root_build_snapshot: Option<&'pass build_settings::BuildSnapshotRequest>,
    discovery: TargetEntryDiscovery,
    prior: Option<&'pass RetainedPackageReviews>,
}

struct ReviewPackage<'pass> {
    scope: PackageCompilationScope<'pass>,
    custody: &'pass PackageSourceCustody,
    position: usize,
    purposes: Vec<(DependencyPurpose, target::TargetProfile)>,
}

pub type PackageReviewExecutor<'execution> = dyn for<'pass> FnMut(
        PackageReviewPass<'pass>,
        &mut PackageSourceBindings,
    ) -> Result<RetainedPackageReviews, CompileResolvedPackageReviewsError>
    + 'execution;

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
        root_build_snapshot: Option<&'pass build_settings::BuildSnapshotRequest>,
        discovery: TargetEntryDiscovery,
        prior: Option<&'pass RetainedPackageReviews>,
    ) -> Result<Self, CompileResolvedPackageReviewsError> {
        let closure = target_closure.source_closure();
        if execution_profile.is_none()
            && let Some(coverage) = roster
                .coverages()
                .iter()
                .find(|coverage| coverage.purposes().contains(&DependencyPurpose::Build))
        {
            return Err(
                CompileResolvedPackageReviewsError::UnsupportedBuildActivation {
                    package: coverage.package().clone(),
                    reason: "build dependencies require an admitted build execution profile",
                },
            );
        }
        let mut packages = Vec::new();
        for key in dependency_first_package_order(closure) {
            let missing = || CompileResolvedPackageReviewsError::IdentityMismatch {
                package: key.clone(),
            };
            let position = closure.graph().package_position(&key).ok_or_else(missing)?;
            let custody = closure.custody(&key).ok_or_else(missing)?;
            let requested = roster.purposes(&key).ok_or_else(missing)?;
            let mut purposes = Vec::with_capacity(requested.len());
            for &purpose in requested {
                let checked_target = if purpose.is_product() {
                    target_closure.target_profile()
                } else {
                    execution_profile.ok_or_else(|| CompileResolvedPackageReviewsError::UnsupportedBuildActivation {
                        package: key.clone(),
                        reason: "build dependencies require an admitted build execution profile",
                    })?
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
            discovery,
            prior,
        })
    }

    pub fn occurrences(&self) -> impl Iterator<Item = PackageReviewOccurrence<'_>> {
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

    pub fn begin_round<'round>(
        &'round self,
        source_bindings: &'round mut PackageSourceBindings,
        fresh_components: &'round BTreeSet<(
            semantic_vocabulary::PackageKeyIdentity,
            DependencyPurpose,
        )>,
        recovered: &'round [RetainedPackageReviews],
    ) -> PackageReviewRound<'round> {
        PackageReviewRound {
            closure: self.closure,
            execution_profile: self.execution_profile,
            semantic_bindings_by_consumer: self.semantic_bindings_by_consumer,
            root_build_snapshot: self.root_build_snapshot,
            discovery: self.discovery,
            prior: self.prior,
            source_bindings,
            fresh_components,
            recovered,
            reviews: Vec::with_capacity(self.roster.occurrence_count()),
            review_positions: vec![[None; 2]; self.closure.graph().packages().len()],
            component_candidates: BTreeMap::new(),
            occurrences: Vec::new(),
            carried_occurrences: 0,
            retained_policy_canonical_total: 0,
            target_entry_candidates: Vec::new(),
        }
    }
}

pub struct PackageReviewRound<'pass> {
    closure: &'pass ResolvedPackageSourceClosure,
    execution_profile: Option<target::TargetProfile>,
    semantic_bindings_by_consumer:
        &'pass BTreeMap<(PackageKey, DependencyPurpose), Vec<AcceptedSemanticBinding>>,
    root_build_snapshot: Option<&'pass build_settings::BuildSnapshotRequest>,
    discovery: TargetEntryDiscovery,
    prior: Option<&'pass RetainedPackageReviews>,
    source_bindings: &'pass mut PackageSourceBindings,
    fresh_components: &'pass BTreeSet<(semantic_vocabulary::PackageKeyIdentity, DependencyPurpose)>,
    recovered: &'pass [RetainedPackageReviews],
    reviews: Vec<CompilerIssuedPackageReview>,
    review_positions: Vec<[Option<usize>; 2]>, // Index into `reviews` of each package position's review, by purpose slot.
    component_candidates:
        BTreeMap<(semantic_vocabulary::PackageKeyIdentity, DependencyPurpose), OccurrenceCompile>,
    occurrences: Vec<RetainedOccurrenceCompile>,
    carried_occurrences: usize, // Occurrences carried forward instead of compiled.
    retained_policy_canonical_total: usize,
    target_entry_candidates: Vec<(DependencyPurpose, SemanticBindingReviewCandidate)>,
}

pub struct PackageReviewOccurrence<'occurrence> {
    key: &'occurrence PackageKey,
    scope: &'occurrence PackageCompilationScope<'occurrence>,
    _timing: Option<crate::review::timings::StageTiming>,
    custody: &'occurrence PackageSourceCustody,
    position: usize,
    purpose: DependencyPurpose,
    checked_target: target::TargetProfile,
}

struct AssembledOccurrence {
    entry: std::path::PathBuf,
    build_snapshot: build_settings::BuildSnapshotRequest,
    proposed_entry: Option<SemanticBindingReviewCandidate>,
}

impl<'pass> PackageReviewRound<'pass> {
    pub fn admit_occurrence<'occurrence>(
        &mut self,
        occurrence: PackageReviewOccurrence<'occurrence>,
    ) -> Result<Option<PackageReviewWork<'occurrence>>, CompileResolvedPackageReviewsError>
    where
        'pass: 'occurrence,
    {
        let (inputs, assembled) = self.assemble_occurrence_inputs(&occurrence)?;
        if self.carry_prior_compile(&occurrence, &inputs, &assembled)? {
            return Ok(None);
        }
        let request = PackageReviewRequest {
            inputs,
            execution: package_compilation::build_execution::BuildExecutionContract {
                profile: self.execution_profile,
                snapshot: Some(assembled.build_snapshot.clone()),
            },
            entry: assembled.entry.clone(),
            target: occurrence.checked_target,
        };
        Ok(Some(PackageReviewWork {
            request,
            completion: PackageReviewCompletion {
                occurrence,
                assembled,
                closure: self.closure,
                execution_profile: self.execution_profile,
            },
            maximum_policy_canonical_bytes: MAXIMUM_RETAINED_POLICY_CANONICAL_BYTES
                .saturating_sub(self.retained_policy_canonical_total),
        }))
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
        let mut semantic_bindings = self
            .semantic_bindings_by_consumer
            .get(&(key.clone(), purpose))
            .cloned()
            .unwrap_or_default();
        let mut proposed_entry = None;
        if self.discovery == TargetEntryDiscovery::Dependencies
            && inputs.root_role() == package_compilation::BuildDeclarationKind::Application
        {
            let candidates = self
                .target_entry_candidates
                .iter()
                .filter(|(candidate_purpose, candidate)| {
                    *candidate_purpose == purpose
                        && inputs.package_root(candidate.binding().package()).is_some()
                        && !semantic_bindings
                            .iter()
                            .any(|binding| binding.role() == candidate.binding().role())
                })
                .map(|(_, candidate)| candidate)
                .collect::<Vec<_>>();
            if let [candidate] = candidates.as_slice() {
                semantic_bindings.push(candidate.binding().clone());
                proposed_entry = Some((*candidate).clone());
            }
        }
        let inputs = inputs
            .with_semantic_role_discovery(self.discovery == TargetEntryDiscovery::Dependencies)
            .with_accepted_semantic_bindings(semantic_bindings)
            .map_err(refused)?;
        let required_instances = inputs.required_dependency_source_instances();
        let mut dependency_bundles = Vec::with_capacity(required_instances.len());
        for dependency in scope
            .packages()
            .iter()
            .filter(|dependency| *dependency != key)
        {
            for dependency_purpose in DependencyPurpose::ALL {
                if !required_instances.contains(&(dependency.identity(), dependency_purpose)) {
                    continue;
                }
                let dependency_review = closure
                    .graph()
                    .package_position(dependency)
                    .and_then(|position| {
                        self.review_positions[position][purpose_slot(dependency_purpose)]
                    })
                    .and_then(|position| self.reviews.get(position));
                let review =
                    dependency_review.ok_or(PackageCompilationInputError::MissingBuildHandoff {
                        package: dependency.identity(),
                    });
                let review = review.map_err(|error| {
                    CompileResolvedPackageReviewsError::CompilationInputs {
                        package: key.clone(),
                        errors: vec![error],
                    }
                })?;
                let custody = closure
                    .custody(dependency)
                    .ok_or(PackageCompilationInputError::ForeignBuildHandoff {
                        package: dependency.identity(),
                    })
                    .map_err(
                        |error| CompileResolvedPackageReviewsError::CompilationInputs {
                            package: key.clone(),
                            errors: vec![error],
                        },
                    )?;
                let bundle = review.build_handoff();
                if review.resolution() != custody.resolution()
                    || bundle.package() != dependency.identity()
                    || bundle.source_consumption_commitment()
                        != review.source_consumption_commitment()
                {
                    return Err(CompileResolvedPackageReviewsError::CompilationInputs {
                        package: key.clone(),
                        errors: vec![PackageCompilationInputError::BuildHandoffCustodyMismatch {
                            package: dependency.identity(),
                        }],
                    });
                }
                dependency_bundles.push(bundle.clone());
            }
        }
        let inputs = inputs
            .with_complete_dependency_build_handoffs(dependency_bundles)
            .map_err(
                |errors| CompileResolvedPackageReviewsError::CompilationInputs {
                    package: key.clone(),
                    errors,
                },
            )?;
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
        .unwrap_or_else(|| build_settings::BuildSnapshotRequest::new(empty::<Vec<u8>>()));
        let unanswered_inputs = inputs;
        Ok((
            unanswered_inputs,
            AssembledOccurrence {
                entry,
                build_snapshot,
                proposed_entry,
            },
        ))
    }

    fn carry_prior_compile(
        &mut self,
        occurrence: &PackageReviewOccurrence<'_>,
        unanswered_inputs: &PackageCompilationInputs,
        assembled: &AssembledOccurrence,
    ) -> Result<bool, CompileResolvedPackageReviewsError> {
        let PackageReviewOccurrence {
            key,
            custody,
            position,
            purpose,
            checked_target,
            ..
        } = *occurrence;
        let closure = self.closure;
        let execution_profile = self.execution_profile;
        let entry = assembled.entry.as_path();
        let build_snapshot = &assembled.build_snapshot;
        let reused = self
            .recovered
            .iter()
            .rev()
            .map(|prior| (prior, true))
            .chain(self.prior.map(|prior| (prior, false)))
            .find_map(|(prior, same_pass)| {
                prior
                    .occurrences
                    .iter()
                    .find(|occurrence| {
                        occurrence.position == position
                            && occurrence.purpose == purpose
                            && (!self.fresh_components.contains(&(key.identity(), purpose))
                                || matches!(occurrence.compilation, OccurrenceCompile::Live(_)))
                            && occurrence.entry == *entry
                            && occurrence.inputs == *unanswered_inputs
                            && occurrence.build_snapshot == *build_snapshot
                    })
                    .and_then(|occurrence| {
                        prior
                            .reviews
                            .reviews()
                            .get(occurrence.review)
                            .map(|review| (occurrence, review.clone(), same_pass))
                    })
            });
        let carried = reused
            .map(|(occurrence, review, same_pass)| {
                (
                    occurrence.compilation.clone(),
                    review,
                    occurrence.policy_canonical_bytes,
                    same_pass && occurrence.compiled_in_pass,
                )
            })
            .or_else(|| {
                if self.fresh_components.contains(&(key.identity(), purpose)) {
                    return None;
                }
                let directory = custody.compiled_review_cache_dir()?;
                let cache = super::super::review_cache::CompiledReviewCache::open(directory)?;
                let record = cache.lookup(&compiled_review_key(
                    key.identity(),
                    purpose,
                    checked_target,
                    execution_profile,
                    entry,
                    unanswered_inputs,
                    build_snapshot,
                )?)?;
                let policy_canonical_bytes = record.policy_canonical_bytes()?;
                let (review, facts) = record.into_review_and_facts(key, custody.resolution())?;
                let occurrence = OccurrenceCompile::Cached(Box::new(facts));
                Some((occurrence, review, policy_canonical_bytes, false))
            });
        let Some((compile, review, policy_canonical_bytes, compiled_in_pass)) = carried else {
            return Ok(false);
        };
        compile
            .verify_current_source_consumption(unanswered_inputs)
            .map_err(
                |diagnostics| CompileResolvedPackageReviewsError::SourceConsumptionDrift {
                    package: key.clone(),
                    diagnostics,
                },
            )?;
        self.retained_policy_canonical_total =
            reserve(self.retained_policy_canonical_total, policy_canonical_bytes).ok_or_else(
                || CompileResolvedPackageReviewsError::RetainedPolicyCanonicalBudget {
                    package: key.clone(),
                    maximum_bytes: MAXIMUM_RETAINED_POLICY_CANONICAL_BYTES,
                },
            )?;
        if self.discovery == TargetEntryDiscovery::Dependencies {
            self.target_entry_candidates.extend(
                compile
                    .target_entry_candidate(key)?
                    .map(|candidate| (purpose, candidate)),
            );
        }
        let review_position = self.reviews.len();
        self.review_positions[position][purpose_slot(purpose)] = Some(review_position);
        self.reviews.push(review);
        if key != closure.graph().root() {
            self.component_candidates
                .insert((key.identity(), purpose), compile.clone());
        }
        self.occurrences.push(RetainedOccurrenceCompile {
            position,
            purpose,
            entry: entry.to_path_buf(),
            inputs: unanswered_inputs.clone(),
            build_snapshot: build_snapshot.clone(),
            policy_canonical_bytes,
            review: review_position,
            compilation: compile,
            compiled_in_pass,
        });
        self.carried_occurrences += usize::from(!compiled_in_pass);
        Ok(true)
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
                    CompileResolvedPackageReviewsError::ComponentDescriptionNotRetained {
                        package: occurrence.key.clone(),
                        component,
                        purpose: occurrence.purpose,
                    }
                })?;
                Ok(Some(PackageReviewComponent::Description(description)))
            }
            None => Err(CompileResolvedPackageReviewsError::Compilation {
                package: occurrence.key.clone(),
                failure: compilation_products::CompileFailure::Diagnostics(vec![
                    Diagnostic::error(format!(
                        "an independent provider selection names dependency package `{}`, which this closure never compiled",
                        inputs.package_name(component).unwrap_or("<unnamed>"),
                    )),
                ]),
            }),
        }
    }

    pub fn retain(
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
            return Err(CompileResolvedPackageReviewsError::IdentityMismatch {
                package: key.clone(),
            });
        }
        self.retained_policy_canonical_total =
            reserve(self.retained_policy_canonical_total, policy_canonical_bytes).ok_or_else(
                || CompileResolvedPackageReviewsError::RetainedPolicyCanonicalBudget {
                    package: key.clone(),
                    maximum_bytes: MAXIMUM_RETAINED_POLICY_CANONICAL_BYTES,
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
                CompileResolvedPackageReviewsError::Encoding {
                    package: key.clone(),
                    error,
                }
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

    pub fn finish(mut self) -> Result<RetainedPackageReviews, CompileResolvedPackageReviewsError> {
        let root = self.closure.graph().root();
        let services = self
            .reviews
            .iter()
            .find(|review| &review.key == root && review.build_handoff.purpose().is_product())
            .ok_or_else(|| CompileResolvedPackageReviewsError::IdentityMismatch {
                package: root.clone(),
            })?
            .policy
            .service_audits()
            .to_vec();
        let mut total = 0;
        for occurrence in &mut self.occurrences {
            let review = &mut self.reviews[occurrence.review];
            let changed = review.policy.service_audits() != services;
            let length = if changed {
                review.policy.apply_service_audit_policy(&services);
                review
                    .policy
                    .canonical_bytes()
                    .map_err(|error| CompileResolvedPackageReviewsError::Encoding {
                        package: review.key.clone(),
                        error,
                    })?
                    .len()
            } else {
                occurrence.policy_canonical_bytes
            };
            total = reserve(total, length).ok_or_else(|| {
                CompileResolvedPackageReviewsError::RetainedPolicyCanonicalBudget {
                    package: review.key.clone(),
                    maximum_bytes: MAXIMUM_RETAINED_POLICY_CANONICAL_BYTES,
                }
            })?;
            occurrence.policy_canonical_bytes = length;
            if changed {
                let (rows, commitment) = super::super::review_set::policy_rows_and_commitment(
                    &review.policy,
                )
                .map_err(|error| CompileResolvedPackageReviewsError::Encoding {
                    package: review.key.clone(),
                    error,
                })?;
                review.policy_rows = rows;
                review.policy_commitment = commitment;
            }
        }
        Ok(self.into_retained())
    }

    pub fn into_retained(self) -> RetainedPackageReviews {
        RetainedPackageReviews {
            reviews: CompilerIssuedPackageReviewSet {
                reviews: self.reviews,
            },
            occurrences: self.occurrences,
            carried: self.carried_occurrences,
        }
    }
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
            compiler::PackageReviewFailure::Projection(failure) => {
                projection_failure(self.occurrence.key, failure)
            }
        }
    }
}

pub enum PackageReviewComponent {
    Source(compiler::PackageReviewSource),
    Description(IndependentComponentDescription),
}
