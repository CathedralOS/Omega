//! Admit review occurrences and join their evidence, without executing compilers.
//!
//! Acquisition and the immutable parse checkpoint remain package-owned. Build
//! and product occurrences share those inputs, not checked output: each has its
//! own target, generated sources, bindings and review. The same source-roster
//! contract drives production, lock comparison and independent reconstruction.

use crate::build_session::{PackageBuildSession, dependency_first_package_order};
use compilation_products::{CheckedCompileRequest, CompileFailure};

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
    AcceptedSemanticBinding, PackageCompilationInputError, PackageCompilationInputs,
};
use package_evidence::record::PackageReviewEvidence;
use package_evidence::record::SemanticBindingReviewCandidate;
use std::collections::BTreeMap;

use super::PackageSourceBindings;

pub struct RetainedPackageReviews {
    pub(super) reviews: CompilerIssuedPackageReviewSet,
    /// Every occurrence's compile under the exact inputs it consumed, in
    /// occurrence order. A following pass over the same closure and roster
    /// carries an occurrence forward — check, build evaluation, and review —
    /// only when it arrives at these same inputs under the same entry.
    pub(super) occurrences: Vec<RetainedOccurrenceCompile>,
    /// Occurrences this pass carried forward from `prior` without compiling:
    /// the occurrences whose complete inputs were identical to a retained
    /// earlier compile. Timing notes report it beside the compiled count so a
    /// bound pass's remaining compiles are distinguishable from its carries.
    pub(super) carried: usize,
}

/// One package occurrence's compile, retained with the complete inputs that
/// produced it. The inputs and entry path are the reuse key: they carry the
/// bindings, dependency generated-source handoffs, purpose and source custody
/// a later pass could alter, while target and build snapshot
/// are parameters the reviewing caller supplies identically to every pass.
#[derive(Debug, Clone)]
pub(super) struct RetainedOccurrenceCompile {
    /// Position in the dependency-first occurrence order.
    pub(super) position: usize,
    pub(super) purpose: DependencyPurpose,
    /// The checked entry path this occurrence's compile consumed.
    pub(super) entry: std::path::PathBuf,
    /// The complete inputs this occurrence's compile consumed, after resolved
    /// semantic bindings and dependency generated-source handoffs joined.
    pub(super) inputs: PackageCompilationInputs,
    /// The source inventory and output roster this occurrence's build
    /// evaluation captured — a compile input `inputs` does not carry.
    pub(super) build_snapshot: build_evaluation::BuildSnapshotRequest,
    /// Canonical-policy bytes this occurrence's review added to the running
    /// retained-policy total.
    pub(super) policy_canonical_bytes: usize,
    /// Index of this occurrence's review inside the pass's review vector.
    pub(super) review: usize,
    /// Opaque live source custody or source-free facts recovered from cache.
    pub(super) compilation: OccurrenceCompile,
    /// This dependency-closure pass already performed the check, even if a
    /// later component-cache miss temporarily interrupted its consumers.
    compiled_in_pass: bool,
}

/// One retained occurrence's checked compile. A compile minted in this
/// process keeps an opaque compiler source owner; a compile recovered from the
/// persistent review cache carries only the facts the carried path and the
/// component publisher consume, since a source owner's sealed
/// program storage is not serializable.
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

    fn tolerated_unsettled_service_fields(&self) -> bool {
        match self {
            Self::Live(source) => source.facts().tolerated_unsettled_service_fields,
            Self::Cached(facts) => facts.tolerated_unsettled_service_fields,
        }
    }

    /// The source-consumption check a carried compile owes before its review
    /// counts. A persisted occurrence's retained commitment was written by an
    /// earlier invocation, so re-deriving it over the stored consumed units and
    /// this invocation's live inputs is what admits the reuse. A live
    /// compiler-owned review source has nothing to establish: this invocation read
    /// those sources and holds the bytes it checked.
    fn verify_current_source_consumption(
        &self,
        inputs: &PackageCompilationInputs,
    ) -> Result<(), Vec<Diagnostic>> {
        match self {
            Self::Live(_) => Ok(()),
            Self::Cached(facts) => {
                let commitment = package_compilation::derive_source_consumption_commitment(
                    &facts.consumed_units,
                    inputs,
                )?;
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

/// Only the preliminary Discover pass may propose an already-checked
/// dependency's target entry schema. Source checkpoint reuse is independent:
/// the bound pass consumes exactly its supplied bindings.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum TargetEntryDiscovery {
    Disabled,
    Dependencies,
}

/// Application work for one policy-selected review pass. The package owner
/// admits the roster, bindings and root snapshot before any Build runs.
pub struct PackageReviewPass<'pass> {
    closure: &'pass ResolvedPackageSourceClosure,
    packages: Vec<ReviewPackage<'pass>>,
    execution_profile: Option<target::TargetProfile>,
    roster: &'pass PackageOccurrenceRoster,
    semantic_bindings_by_consumer:
        &'pass BTreeMap<(PackageKey, DependencyPurpose), Vec<AcceptedSemanticBinding>>,
    root_build_snapshot: Option<&'pass build_evaluation::BuildSnapshotRequest>,
    discovery: TargetEntryDiscovery,
    prior: Option<&'pass RetainedPackageReviews>,
}

/// The dependency scope is prepared once for all of a package's purposes
/// and reused if a demanded component interrupts the pass.
struct ReviewPackage<'pass> {
    scope: PackageCompilationScope<'pass>,
    custody: &'pass PackageSourceCustody,
    position: usize,
    purposes: Vec<(DependencyPurpose, target::TargetProfile)>,
}

/// One complete dependency execution service supplied by application Build.
/// Package management selects policy passes; it does not assemble compiler stages.
pub type PackageReviewExecutor =
    for<'pass> fn(
        PackageReviewPass<'pass>,
        &mut PackageBuildSession,
        &mut PackageSourceBindings,
    ) -> Result<RetainedPackageReviews, CompileResolvedPackageReviewsError>;

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
        root_build_snapshot: Option<&'pass build_evaluation::BuildSnapshotRequest>,
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

    /// Borrow admitted work in dependency order; the yielded token retains
    /// occurrence timing until its work or carry has completed.
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

    /// A cache-miss restart receives only already completed evidence. The
    /// application retains pending Build continuations separately.
    pub fn begin_round<'round>(
        &'round self,
        source_bindings: &'round mut PackageSourceBindings,
        fresh_components: &'round std::collections::BTreeSet<(
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

/// One pass over a dependency closure in dependency-first order. The request
/// every occurrence shares is fixed at construction; the reviews, budgets,
/// candidates and build usages accumulate as each occurrence completes, so a
/// consumer always finds its dependencies' retained reviews already here.
///
/// Each occurrence runs four phases in order: assemble its complete inputs,
/// carry a prior pass's or persisted record's identical compile forward, or
/// else compile it fresh and then project and retain the fresh review.
pub struct PackageReviewRound<'pass> {
    closure: &'pass ResolvedPackageSourceClosure,
    execution_profile: Option<target::TargetProfile>,
    semantic_bindings_by_consumer:
        &'pass BTreeMap<(PackageKey, DependencyPurpose), Vec<AcceptedSemanticBinding>>,
    root_build_snapshot: Option<&'pass build_evaluation::BuildSnapshotRequest>,
    discovery: TargetEntryDiscovery,
    prior: Option<&'pass RetainedPackageReviews>,
    source_bindings: &'pass mut PackageSourceBindings,
    /// Dependency occurrences a consumer demanded a component description
    /// from that no carried record retained: they compile fresh and their
    /// replacement records carry the published description.
    fresh_components: &'pass std::collections::BTreeSet<(
        semantic_vocabulary::PackageKeyIdentity,
        DependencyPurpose,
    )>,
    /// Completed work from interrupted attempts in this same pass. It remains
    /// usable only under the exact input comparisons of ordinary review reuse.
    recovered: &'pass [RetainedPackageReviews],
    reviews: Vec<CompilerIssuedPackageReview>,
    /// Index into `reviews` of each package position's review, by purpose slot.
    review_positions: Vec<[Option<usize>; 2]>,
    /// Each dependency's own checked compilation is retained as a component
    /// candidate: a later consumer's `Independent` selection can only settle
    /// against the description published from that compilation, and the
    /// component-closure fence refuses any substitute. Cloning shares the
    /// sealed program storage, so the same dependency can serve more than one
    /// independent consumer.
    component_candidates:
        BTreeMap<(semantic_vocabulary::PackageKeyIdentity, DependencyPurpose), OccurrenceCompile>,
    occurrences: Vec<RetainedOccurrenceCompile>,
    /// Occurrences carried forward instead of compiled.
    carried_occurrences: usize,
    retained_policy_canonical_total: usize,
    /// Target-entry schema offers from checked dependencies, by purpose. Only
    /// the discovery pass consults them to propose a consumer's binding.
    target_entry_candidates: Vec<(DependencyPurpose, SemanticBindingReviewCandidate)>,
}

/// One checked package occurrence: the package, its graph position and source
/// custody, the purpose it is checked for, and the target that purpose selects.
pub struct PackageReviewOccurrence<'occurrence> {
    key: &'occurrence PackageKey,
    scope: &'occurrence PackageCompilationScope<'occurrence>,
    _timing: Option<crate::review::timings::StageTiming>,
    custody: &'occurrence PackageSourceCustody,
    position: usize,
    purpose: DependencyPurpose,
    checked_target: target::TargetProfile,
}

/// One occurrence's assembled entry and build snapshot — with its complete
/// inputs, the reuse key a carried compile must match — plus what the fresh
/// review's projection consumes.
struct AssembledOccurrence {
    entry: std::path::PathBuf,
    build_snapshot: build_evaluation::BuildSnapshotRequest,
    /// The discovery pass's nominated target-entry binding, if one joined.
    proposed_entry: Option<SemanticBindingReviewCandidate>,
}

impl<'pass> PackageReviewRound<'pass> {
    /// Admit one occurrence, or carry an exact prior/cache result without
    /// asking the application to execute its Build again.
    pub fn admit_occurrence<'occurrence>(
        &mut self,
        occurrence: PackageReviewOccurrence<'occurrence>,
        session: &mut PackageBuildSession,
    ) -> Result<Option<PackageReviewWork<'occurrence>>, CompileResolvedPackageReviewsError>
    where
        'pass: 'occurrence,
    {
        let (inputs, assembled) = self.assemble_occurrence_inputs(&occurrence)?;
        if self.carry_prior_compile(&occurrence, &inputs, &assembled)? {
            return Ok(None);
        }
        let request = CheckedCompileRequest {
            build_execution_profile: self.execution_profile,
            package_inputs: Some(inputs),
            filesystem_sponsor: Some(session.filesystem_sponsor().clone()),
            evaluation_sponsor: Some(session.evaluation_sponsor().clone()),
            build_snapshot: Some(assembled.build_snapshot.clone()),
            collect_timings: self.source_bindings.collect_timings,
            permit_unsettled_fused_service_fields: self.discovery
                == TargetEntryDiscovery::Dependencies,
            ..CheckedCompileRequest::new(
                &assembled.entry,
                Some(occurrence.checked_target.target_name()),
            )
        };
        Ok(Some(PackageReviewWork {
            request,
            completion: PackageReviewCompletion {
                occurrence,
                assembled,
                closure: self.closure,
                execution_profile: self.execution_profile,
            },
            maximum_policy_canonical_bytes: super::policy::MAXIMUM_RETAINED_POLICY_CANONICAL_BYTES
                .saturating_sub(self.retained_policy_canonical_total),
        }))
    }

    /// Join the occurrence's purpose, semantic bindings and every required
    /// dependency's retained generated-source bundle to its scope's inputs,
    /// and select its entry and build snapshot. The complete inputs return
    /// beside the rest: a fresh compile consumes them, a carry only compares.
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
        let inputs = scope
            .compilation_inputs_reusing(&mut self.source_bindings.root_bindings[position])
            .and_then(|inputs| inputs.with_compilation_purpose(purpose))
            .map_err(
                |errors| CompileResolvedPackageReviewsError::CompilationInputs {
                    package: key.clone(),
                    errors,
                },
            )?;
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
            match candidates.as_slice() {
                [] => {}
                [candidate] => {
                    semantic_bindings.push(candidate.binding().clone());
                    proposed_entry = Some((*candidate).clone());
                }
                // The dependency catalog cannot see the consumer's Build
                // selection yet. Checked entry discovery may nominate one
                // exact schema through its selected physical requirement;
                // absent or ambiguous selection still fails closed there.
                _ => {}
            }
        }
        let inputs = inputs
            .with_semantic_role_discovery(self.discovery == TargetEntryDiscovery::Dependencies)
            .with_accepted_semantic_bindings(semantic_bindings)
            .map_err(
                |errors| CompileResolvedPackageReviewsError::CompilationInputs {
                    package: key.clone(),
                    errors,
                },
            )?;
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
                let review = dependency_review.ok_or(
                    PackageCompilationInputError::MissingGeneratedSourceBundle {
                        package: dependency.identity(),
                    },
                );
                let review = review.map_err(|error| {
                    CompileResolvedPackageReviewsError::CompilationInputs {
                        package: key.clone(),
                        errors: vec![error],
                    }
                })?;
                let custody = closure
                    .custody(dependency)
                    .ok_or(PackageCompilationInputError::ForeignGeneratedSourceBundle {
                        package: dependency.identity(),
                    })
                    .map_err(
                        |error| CompileResolvedPackageReviewsError::CompilationInputs {
                            package: key.clone(),
                            errors: vec![error],
                        },
                    )?;
                let bundle = review.generated_source_bundle();
                if review.resolution() != custody.resolution()
                    || bundle.package() != dependency.identity()
                    || bundle.source_consumption_commitment()
                        != review.source_consumption_commitment()
                {
                    return Err(CompileResolvedPackageReviewsError::CompilationInputs {
                        package: key.clone(),
                        errors: vec![
                            PackageCompilationInputError::GeneratedSourceBundleCustodyMismatch {
                                package: dependency.identity(),
                            },
                        ],
                    });
                }
                dependency_bundles.push(bundle.clone());
            }
        }
        let inputs = inputs
            .with_complete_dependency_generated_sources(dependency_bundles)
            .map_err(
                |errors| CompileResolvedPackageReviewsError::CompilationInputs {
                    package: key.clone(),
                    errors,
                },
            )?;
        let main_entry = custody.snapshot_root().join("main.omg");
        // Package review checks authored build-only packages through the compiler's
        // existing build entrance. Explicit application/check entries below retain
        // their own source requirement; no source or product exports are invented.
        let entry = if main_entry.is_file() {
            main_entry
        } else {
            custody.snapshot_root().join("build.omg")
        };
        // Every package build activation runs against a captured immutable
        // snapshot of its own sealed source custody, never the live custody
        // root: build evaluation captures the inventory from the same
        // canonical metadata index the binding validated above, materializes
        // a fresh private Source root for this occurrence, and records the
        // inventory extent in the review's build observation. Only the root
        // may receive a caller-selected source inventory and fixed output
        // roster; dependency source inventories remain their own. Explicit
        // named inputs belong only to the root invocation. Otherwise
        // the outputs a package build must complete are declared through
        // `builder.output.require`, registered during evaluation, and settled
        // against sealed staged custody before the result publishes; package
        // review has no fixed-output requirement of its own, and a roster
        // inferred here from the declaration would be a second roster source
        // that scoped execution forbids (a conditional branch omitting
        // `require` must not be reinterpreted as having checked the artifact).
        let build_snapshot = if key == closure.graph().root() {
            self.root_build_snapshot.cloned()
        } else {
            None
        }
        .unwrap_or_else(|| {
            build_evaluation::BuildSnapshotRequest::new(std::iter::empty::<Vec<u8>>())
        });
        // Every package compiles as the root of its own component view, so
        // any of them — not only the graph root — may author `Independent`
        // selections over dependencies this loop already compiled. The
        // completed Build exposes those selections before Product checking.
        // Its opaque continuation accepts their descriptions without replaying
        // Build or changing the request's source graph.
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

    /// Carry forward an identical compile from the prior pass or the
    /// persistent review cache, retaining its review exactly as a fresh one
    /// would be. Returns whether the occurrence was carried.
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
        // A pass that arrives at exactly these inputs for this occurrence
        // compiles the same program deterministically, so the prior
        // pass's compile carries its check, build evaluation, and review
        // forward untouched. The carried compile must not have exercised
        // the discovery-only tolerance this pass withholds: a result
        // settled under it is not the result this pass would produce.
        // The pass that ran this evaluation already charged the shared
        // invocation sponsor. Carrying its immutable result retains that
        // charge without evaluating or charging it again.
        // The reuse key is the occurrence's complete compile input, not a
        // binding-normalized projection of it: `typed_trees_to_checked_trees`
        // provider settlement and selected-execution settlement read every
        // accepted semantic binding — canonical host bindings mint
        // toolchain plans and authorize fused-service erasures, entry and
        // exit bindings resolve into the review's resolved-binding rows —
        // so a pass difference in any binding produces a genuinely
        // different checked product and review. Discovery mode is also an
        // explicit input: its provisional role projection cannot stand in for
        // final acceptance. Normalizing either field out of this key would
        // replay a review that never checked this pass's authority. The typed
        // frontier can still be reused independently of these later inputs.
        // A persisted review from an earlier update invocation is the
        // same carry an in-session prior offers: the record key binds
        // package identity, purpose, checked context, entry, toolchain,
        // and the complete live inputs, and the decoded record
        // re-verifies its own canonical payloads before it counts.
        // A compile that used the discovery-only tolerance carries only into
        // a discovery pass, which permits the same tolerance and so would
        // compute the same provisional result; a bound pass withholds it and
        // compiles fresh. Discovery mode is part of the inputs, so a record
        // a discovery pass stored only answers another discovery pass. The
        // graph root is never persisted: its checked compilation is the
        // invocation's product.
        let tolerance_permitted = self.discovery == TargetEntryDiscovery::Dependencies;
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
                            && (tolerance_permitted
                                || !occurrence.compilation.tolerated_unsettled_service_fields())
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
                if key == closure.graph().root()
                    || self.fresh_components.contains(&(key.identity(), purpose))
                {
                    return None;
                }
                let directory = custody.compiled_review_cache_dir()?;
                let cache = super::super::review_cache::CompiledReviewCache::open(directory)?;
                let record = cache.lookup(&super::super::review_cache::compiled_review_key(
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
                if occurrence.tolerated_unsettled_service_fields() && !tolerance_permitted {
                    return None;
                }
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
            super::policy::reserve(self.retained_policy_canonical_total, policy_canonical_bytes)
                .ok_or_else(|| {
                    CompileResolvedPackageReviewsError::RetainedPolicyCanonicalBudget {
                        package: key.clone(),
                        maximum_bytes: super::policy::MAXIMUM_RETAINED_POLICY_CANONICAL_BYTES,
                    }
                })?;
        // Consumers need the same entry-schema offer whether this
        // dependency compiled now, in a prior pass, or in a prior
        // invocation. An offer is not an accepted binding: only the
        // discovery pass may use it to propose a checked selection.
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

    /// Select an admitted dependency's retained component source or cached
    /// description. Publication and a demanded cache refresh belong to Build.
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
                diagnostics: vec![Diagnostic::error(format!(
                    "an independent provider selection names dependency package `{}`, which this closure never compiled",
                    inputs.package_name(component).unwrap_or("<unnamed>"),
                ))],
            }),
        }
    }

    /// Join compiler evidence to resolver custody and consumer nominations,
    /// charge its retained budget, then retain the occurrence and admit caching.
    pub fn retain(
        &mut self,
        completion: PackageReviewCompletion<'_>,
        product: compiler::PackageReviewProduct,
        occurrence_inputs: PackageCompilationInputs,
    ) -> Result<
        Option<super::super::review_cache::PackageReviewCacheWrite>,
        CompileResolvedPackageReviewsError,
    > {
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
            generated_source_bundle,
            policy,
            policy_canonical_bytes,
        } = evidence;
        if generated_source_bundle.package() != key.identity()
            || generated_source_bundle.target() != checked_target
        {
            return Err(CompileResolvedPackageReviewsError::IdentityMismatch {
                package: key.clone(),
            });
        }
        self.retained_policy_canonical_total =
            super::policy::reserve(self.retained_policy_canonical_total, policy_canonical_bytes)
                .ok_or_else(|| {
                    CompileResolvedPackageReviewsError::RetainedPolicyCanonicalBudget {
                        package: key.clone(),
                        maximum_bytes: super::policy::MAXIMUM_RETAINED_POLICY_CANONICAL_BYTES,
                    }
                })?;
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
            generated_source_bundle: generated_source_bundle.clone(),
            policy,
            policy_rows,
            policy_commitment,
        };
        // Persist the review under the occurrence's post-settlement
        // inputs so a later invocation arriving at exactly these inputs
        // carries it without compiling. The key includes the component
        // descriptions joined to the completed Build, so an occurrence that
        // needed those descriptions never matches an unanswered-inputs lookup —
        // the same shape the in-session carry takes.
        let mut cache_write = None;
        if key != closure.graph().root()
            && (self.discovery == TargetEntryDiscovery::Dependencies
                || !source.facts().tolerated_unsettled_service_fields)
            && let Some(directory) = custody.compiled_review_cache_dir()
            && let Some(cache) = super::super::review_cache::CompiledReviewCache::open(directory)
            && let Some(consumed_units) = source.facts().consumed_units.as_ref()
            && let Ok(target_entry_candidate) = source.target_entry_candidate()
        {
            let demanded = self.fresh_components.contains(&(key.identity(), purpose));
            if let Some(record) = super::super::review_cache::record_compiled_review(
                &review,
                super::super::review_cache::CachedOccurrenceFacts {
                    tolerated_unsettled_service_fields: source
                        .facts()
                        .tolerated_unsettled_service_fields,
                    consumed_units: consumed_units.to_vec(),
                    source_consumption_commitment,
                    component_description: None,
                    target_entry_candidate,
                },
                policy_canonical_bytes,
            ) && let Some(cache_key) = super::super::review_cache::compiled_review_key(
                key.identity(),
                purpose,
                checked_target,
                execution_profile,
                entry,
                &occurrence_inputs,
                &build_snapshot,
            ) {
                cache_write = Some(super::super::review_cache::PackageReviewCacheWrite {
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

    /// Finish root-wide policy joins and return the retained review occurrences.
    pub fn finish(mut self) -> Result<RetainedPackageReviews, CompileResolvedPackageReviewsError> {
        let root = self.closure.graph().root();
        let services = self
            .reviews
            .iter()
            .find(|review| {
                &review.key == root && review.generated_source_bundle.purpose().is_product()
            })
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
            total = super::policy::reserve(total, length).ok_or_else(|| {
                CompileResolvedPackageReviewsError::RetainedPolicyCanonicalBudget {
                    package: review.key.clone(),
                    maximum_bytes: super::policy::MAXIMUM_RETAINED_POLICY_CANONICAL_BYTES,
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

    /// Retain only completed individual reviews. An interrupted set is private
    /// reuse input, never the final root-wide review returned to the caller.
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

/// Whole-occurrence work admitted by package policy, with its completion
/// context retained alongside the compiler request.
pub struct PackageReviewWork<'occurrence> {
    pub request: CheckedCompileRequest,
    pub completion: PackageReviewCompletion<'occurrence>,
    pub maximum_policy_canonical_bytes: usize,
}

pub struct PackageReviewCompletion<'occurrence> {
    occurrence: PackageReviewOccurrence<'occurrence>,
    assembled: AssembledOccurrence,
    closure: &'occurrence ResolvedPackageSourceClosure,
    execution_profile: Option<target::TargetProfile>,
}

impl PackageReviewCompletion<'_> {
    /// Reserve staging only when Build will really execute. A resumed opaque
    /// continuation keeps its original staging and charges no second grant.
    pub fn reserve_build_directory(
        &self,
        session: &mut PackageBuildSession,
    ) -> Result<std::path::PathBuf, CompileResolvedPackageReviewsError> {
        session.create_occurrence_directory(
            self.occurrence.key,
            self.occurrence.custody.resolution(),
            self.occurrence.purpose,
        )
    }

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
        CompileResolvedPackageReviewsError::CompilationInputs {
            package: self.occurrence.key.clone(),
            errors,
        }
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
                super::policy::projection_failure(self.occurrence.key, failure)
            }
        }
    }
}

pub enum PackageReviewComponent {
    Source(compiler::PackageReviewSource),
    Description(package_compilation::IndependentComponentDescription),
}
