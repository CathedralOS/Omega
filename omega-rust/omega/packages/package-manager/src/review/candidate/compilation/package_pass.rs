//! Compile dependencies before their consumers, once per checked occurrence.
//!
//! Acquisition and the immutable parse checkpoint remain package-owned. Build
//! and product occurrences share those inputs, not checked output: each has its
//! own target, generated sources, bindings and review. The same source-roster
//! contract drives production, lock comparison and independent reconstruction.
//!
//! Named input assignments are reconciled across all incoming edges before
//! running authored code. A shared package/purpose/profile node may reuse equal
//! complete maps; unioning unequal maps would grant one requester another's
//! inputs. A separate tool package would still need capture, grant issuance and
//! checked generated-source handoff. Routing into these existing owners keeps
//! that enforcement in one execution path without making aliases new instances.

use super::session_accounting::verify_build_session_accounting;
use compilation_products::CheckedCompileRequest;

use super::super::custody::{
    dependency_first_package_order, package_build_root, verify_selected_source_custody,
};
use super::super::rows::RetainedReviewRows;
use super::super::semantic_bindings::{
    SemanticBindingReviewCandidate, candidate_hosted_exit_bindings, candidate_service_bindings,
    candidate_target_entry_binding,
};
use super::super::{
    CompileResolvedPackageReviewsError, CompilerIssuedPackageReview,
    CompilerIssuedPackageReviewSet, PackageSourceVerificationPhase,
};
use super::ledger::{
    MAXIMUM_RETAINED_ORDINARY_LEDGER_BYTES, reserve_retained_obligation_ledger_bytes,
    retained_obligation_ledger_bytes,
};
use crate::declarations::DependencyPurpose;
use crate::declarations::PackageKey;
use crate::lock::{
    PackageAcceptanceRow, PackageCheckedContext, PackageOccurrenceRoster, PackagePolicyAcceptance,
};
use crate::resolution::PackageCompilationScope;
use crate::resolution::graph::{ExactTargetPackageSourceClosure, ResolvedPackageSourceClosure};
use crate::resolution::source::PackageSourceCustody;
use crate::review::restricted_build_grants::RestrictedBuildCheckpoint;
use checked_interpreter::{BuildEvaluationSponsor, FilesystemSponsor};
use compilation_products::CheckedCompileOutcome;
use diagnostics::Diagnostic;
use package_compilation::{
    AcceptedSemanticBinding, PackageCompilationInputError, PackageCompilationInputs,
};
use package_evidence::ledger::{ReconstructedPackageReview, reconstruct_package_review};
use package_evidence::record::PackagePolicyRowKind;
use std::collections::BTreeMap;
use std::path::Path;

use super::{CandidateSourcePreparation, PreparedPackageSource};

pub(super) struct CompiledPackageReviews {
    pub(super) reviews: CompilerIssuedPackageReviewSet,
    pub(super) checked_root: Option<Box<compiler::CheckedCompilation>>,
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
/// a later pass could alter, while target, build snapshot and grant consent
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
    /// Retained-obligation bytes this occurrence's review added to the
    /// running ledger total.
    pub(super) obligation_ledger_bytes: usize,
    /// Index of this occurrence's review inside the pass's review vector.
    pub(super) review: usize,
    /// The checked compilation itself — shared storage with the component
    /// candidate or checked root this pass retained — or, for an occurrence
    /// recovered from the persistent review cache, the carried facts that
    /// stand in for it.
    pub(super) checked: OccurrenceCompile,
}

/// One retained occurrence's checked compile. A compile minted in this
/// process keeps its `CheckedCompilation`; a compile recovered from the
/// persistent review cache carries only the facts the carried path and the
/// component publisher consume, since a `CheckedCompilation`'s sealed
/// program storage is not serializable.
#[derive(Debug, Clone)]
pub(super) enum OccurrenceCompile {
    Checked(compiler::CheckedCompilation),
    Cached(Box<super::super::review_cache::CachedOccurrenceFacts>),
}

impl OccurrenceCompile {
    fn target_entry_candidate(
        &self,
        owner: &PackageKey,
        target: target::TargetProfile,
    ) -> Result<Option<SemanticBindingReviewCandidate>, CompileResolvedPackageReviewsError> {
        match self {
            Self::Checked(checked) => candidate_target_entry_binding(checked, owner, target),
            Self::Cached(facts) => Ok(facts.target_entry_candidate.clone()),
        }
    }

    fn tolerated_unsettled_service_fields(&self) -> bool {
        match self {
            Self::Checked(checked) => checked.execution().tolerated_unsettled_service_fields(),
            Self::Cached(facts) => facts.tolerated_unsettled_service_fields,
        }
    }

    /// The source-consumption re-verification a carried compile owes before
    /// its review counts. A live `CheckedCompilation` re-reads the physical
    /// sources it retained; a cached occurrence instead re-derives its
    /// retained commitment over the stored consumed units plus the live
    /// inputs — the same content identity, checked without the physical read
    /// the surrounding custody verification already performs.
    fn verify_current_source_consumption(
        &self,
        inputs: &PackageCompilationInputs,
    ) -> Result<(), Vec<Diagnostic>> {
        match self {
            Self::Checked(checked) => checked.verify_current_source_consumption(),
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
/// final and Explicit passes consume exactly their supplied bindings.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum TargetEntryDiscovery {
    Disabled,
    Dependencies,
}

fn purpose_slot(purpose: DependencyPurpose) -> usize {
    match purpose {
        DependencyPurpose::Product => 0,
        DependencyPurpose::Build => 1,
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn compile_dependency_closure(
    target_closure: &ExactTargetPackageSourceClosure<'_>,
    execution_profile: Option<target::TargetProfile>,
    roster: &PackageOccurrenceRoster,
    build_session_root: &Path,
    filesystem_sponsor: &FilesystemSponsor,
    evaluation_sponsor: &BuildEvaluationSponsor,
    semantic_bindings_by_consumer: &BTreeMap<
        (PackageKey, DependencyPurpose),
        Vec<AcceptedSemanticBinding>,
    >,
    retained_root_entry: Option<&Path>,
    root_build_snapshot: Option<&build_evaluation::BuildSnapshotRequest>,
    // A consuming compile supplies the accepted target's restricted-request
    // checkpoint so each occurrence's projected requests join its granted
    // meanings — bound to its exact checked context — before its own build
    // effect executes, before the review is retained, and before its
    // generated-source bundle hands off to a consumer.
    // `None` is audit-only observation: requests still project for review
    // material and the pass issues no grants.
    restricted_build_checkpoint: Option<&RestrictedBuildCheckpoint>,
    discovery: TargetEntryDiscovery,
    // A pass already run over this same closure and roster — the discovery
    // pass to the bound pass that follows it. Every occurrence it retained is
    // a reuse candidate for an occurrence that now arrives at exactly its
    // inputs; anything else compiles fresh.
    prior: Option<&CompiledPackageReviews>,
    preparation: &mut CandidateSourcePreparation,
) -> Result<CompiledPackageReviews, CompileResolvedPackageReviewsError> {
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
    // Reconcile the entire invocation before any authored dependency runs.
    // Shared scheduling nodes may share equal assignments, never the union of
    // grants from different incoming edges (an unassigned edge is empty).
    let dependency_inputs = reconcile_build_inputs(target_closure, roster, root_build_snapshot)
        .map_err(
            |diagnostics| CompileResolvedPackageReviewsError::Compilation {
                package: closure.graph().root().clone(),
                diagnostics,
            },
        )?;
    let mut pass = ClosurePass {
        closure,
        execution_profile,
        build_session_root,
        filesystem_sponsor,
        evaluation_sponsor,
        semantic_bindings_by_consumer,
        retained_root_entry,
        root_build_snapshot,
        restricted_build_checkpoint,
        discovery,
        prior,
        preparation,
        dependency_inputs,
        reviews: Vec::with_capacity(roster.occurrence_count()),
        review_positions: vec![[None::<usize>; 2]; closure.graph().packages().len()],
        checked_root: None,
        component_candidates: BTreeMap::new(),
        retained_obligation_ledger_total: 0,
        discarded_build_usages: Vec::new(),
        session_usages: Vec::new(),
        consumed_build_charges: Vec::new(),
        accounting_complete: true,
        occurrences: Vec::new(),
        carried_occurrences: 0,
        retained_policy_canonical_total: 0,
        target_entry_candidates: Vec::new(),
    };
    for key in dependency_first_package_order(closure) {
        let scope = PackageCompilationScope::new(closure, &key);
        verify_selected_source_custody(&scope, PackageSourceVerificationPhase::BeforeCompilation)?;
        let custody = closure
            .custody(&key)
            .expect("validated source closure retains custody for every graph package");
        let position = closure.graph().package_position(&key).ok_or_else(|| {
            CompileResolvedPackageReviewsError::IdentityMismatch {
                package: key.clone(),
            }
        })?;
        let purposes = roster.purposes(&key).ok_or_else(|| {
            CompileResolvedPackageReviewsError::IdentityMismatch {
                package: key.clone(),
            }
        })?;
        // One acquired source can produce two independently checked occurrences.
        // Every dependency package is complete before its consumer; role slots
        // retain the exact generated output and never alias equal-profile builds.
        for &purpose in purposes {
            // One line per package occurrence: the review pass repeats this
            // whole loop, so attributing a pass without attributing its
            // members cannot show WHICH package the second pass repeats.
            let _occurrence = crate::review::timings::subject_stage(|| {
                format!("package {} purpose={purpose:?}", key.name().as_str())
            });
            let checked_target = if purpose.is_product() {
                target_closure.target_profile()
            } else {
                execution_profile.ok_or_else(|| {
                    CompileResolvedPackageReviewsError::UnsupportedBuildActivation {
                        package: key.clone(),
                        reason: "build dependencies require an admitted build execution profile",
                    }
                })?
            };
            if let Err(error) = pass.compile_occurrence(&PackageOccurrence {
                key: &key,
                scope: &scope,
                custody,
                position,
                purpose,
                checked_target,
            }) {
                // A completed Build still consumed or reserved work when its
                // product failed. Existing early exits without an outcome's
                // telemetry (interpreter, continuation, or identity failures)
                // retain their pre-existing reconciliation limitation.
                if pass.accounting_complete {
                    pass.verify_accounting()?;
                }
                return Err(error);
            }
        }
    }
    pass.finish()
}

/// One pass over a dependency closure in dependency-first order. The request
/// every occurrence shares is fixed at construction; the reviews, budgets,
/// candidates and build usages accumulate as each occurrence completes, so a
/// consumer always finds its dependencies' retained reviews already here.
///
/// Each occurrence runs four phases in order: assemble its complete inputs,
/// carry a prior pass's or persisted record's identical compile forward, or
/// else compile it fresh and then project and retain the fresh review.
struct ClosurePass<'pass> {
    closure: &'pass ResolvedPackageSourceClosure,
    execution_profile: Option<target::TargetProfile>,
    build_session_root: &'pass Path,
    filesystem_sponsor: &'pass FilesystemSponsor,
    evaluation_sponsor: &'pass BuildEvaluationSponsor,
    semantic_bindings_by_consumer:
        &'pass BTreeMap<(PackageKey, DependencyPurpose), Vec<AcceptedSemanticBinding>>,
    retained_root_entry: Option<&'pass Path>,
    root_build_snapshot: Option<&'pass build_evaluation::BuildSnapshotRequest>,
    restricted_build_checkpoint: Option<&'pass RestrictedBuildCheckpoint>,
    discovery: TargetEntryDiscovery,
    prior: Option<&'pass CompiledPackageReviews>,
    preparation: &'pass mut CandidateSourcePreparation,
    /// Named build inputs reconciled across every incoming edge, by graph
    /// position and purpose slot.
    dependency_inputs: Vec<[Option<OccurrenceBuildInputs>; 2]>,
    reviews: Vec<CompilerIssuedPackageReview>,
    /// Index into `reviews` of each package position's review, by purpose slot.
    review_positions: Vec<[Option<usize>; 2]>,
    checked_root: Option<Box<compiler::CheckedCompilation>>,
    /// Each dependency's own checked compilation is retained as a component
    /// candidate: a later consumer's `Independent` selection can only settle
    /// against the description published from that compilation, and the
    /// component-closure fence refuses any substitute. Cloning shares the
    /// sealed program storage, so the same dependency can serve more than one
    /// independent consumer.
    component_candidates:
        BTreeMap<(semantic_vocabulary::PackageKeyIdentity, DependencyPurpose), OccurrenceCompile>,
    retained_obligation_ledger_total: usize,
    /// Sponsored build evaluations that ran inside a compile the
    /// component-closure fence rejected: real session consumption no retained
    /// review reports, reconciled beside the reviews at the session's end.
    discarded_build_usages: Vec<build_evaluation::BuildEvaluationUsage>,
    /// Usage reported into this session's sponsor: only evaluations this pass
    /// actually ran. A carried occurrence's evaluation was consumed and
    /// reconciled by the pass that ran it, so it reports nothing here and an
    /// entirely carried pass's sponsor accounting reads as the empty session
    /// it was.
    session_usages: Vec<Option<build_evaluation::BuildEvaluationUsage>>,
    /// Current-pass policy charges for consumed Pure values, not executions.
    consumed_build_charges: Vec<checked_interpreter::BuildEvaluationConsumptionReceipt>,
    accounting_complete: bool,
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
struct PackageOccurrence<'occurrence> {
    key: &'occurrence PackageKey,
    scope: &'occurrence PackageCompilationScope<'occurrence>,
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

impl ClosurePass<'_> {
    fn compile_occurrence(
        &mut self,
        occurrence: &PackageOccurrence<'_>,
    ) -> Result<(), CompileResolvedPackageReviewsError> {
        let (unanswered_inputs, assembled) = self.assemble_occurrence_inputs(occurrence)?;
        if self.carry_prior_compile(occurrence, &unanswered_inputs, &assembled)? {
            return Ok(());
        }
        let (checked, occurrence_inputs) =
            self.compile_fresh(occurrence, unanswered_inputs, &assembled)?;
        self.project_and_retain_review(occurrence, assembled, checked, occurrence_inputs)
    }

    /// Join the occurrence's purpose, semantic bindings and every required
    /// dependency's retained generated-source bundle to its scope's inputs,
    /// and select its entry and build snapshot. The complete inputs return
    /// beside the rest: a fresh compile consumes them, a carry only compares.
    fn assemble_occurrence_inputs(
        &self,
        occurrence: &PackageOccurrence<'_>,
    ) -> Result<(PackageCompilationInputs, AssembledOccurrence), CompileResolvedPackageReviewsError>
    {
        let PackageOccurrence {
            key,
            scope,
            custody,
            position,
            purpose,
            ..
        } = *occurrence;
        let closure = self.closure;
        let inputs = scope
            .compilation_inputs()
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
                _ => {
                    return Err(
                        CompileResolvedPackageReviewsError::AmbiguousCandidateSemanticBinding {
                            consumer: key.clone(),
                            role: candidates[0].binding().role(),
                            candidate_count: candidates.len(),
                        },
                    );
                }
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
        let default_entry = if main_entry.is_file() {
            main_entry
        } else {
            custody.snapshot_root().join("build.omg")
        };
        let entry = if key == closure.graph().root() {
            self.retained_root_entry
                .map(Path::to_path_buf)
                .unwrap_or(default_entry)
        } else {
            default_entry
        };
        // Every package build activation runs against a captured immutable
        // snapshot of its own sealed source custody, never the live custody
        // root: build evaluation captures the inventory from the same
        // canonical metadata index the binding validated above, materializes
        // a fresh private Source root for this occurrence, and records the
        // inventory extent in the review's build observation. Only the root
        // may receive a caller-selected source inventory and fixed output
        // roster; dependency source inventories remain their own. Explicit
        // named inputs are separate grants selected for this occurrence. Otherwise
        // the outputs a package build must complete are declared through
        // `builder.output.require`, registered during evaluation, and settled
        // against sealed staged custody before the result publishes; package
        // review has no fixed-output requirement of its own, and a roster
        // inferred here from the declaration would be a second roster source
        // that scoped execution forbids (a conditional branch omitting
        // `require` must not be reinterpreted as having checked the artifact).
        let build_snapshot = if key == closure.graph().root() {
            self.root_build_snapshot
                .cloned()
                .map(build_evaluation::BuildSnapshotRequest::without_dependency_inputs)
        } else {
            self.dependency_inputs[position][purpose_slot(purpose)]
                .as_ref()
                .map(|assignment| assignment.snapshot.clone())
        }
        .unwrap_or_else(|| {
            build_evaluation::BuildSnapshotRequest::new(std::iter::empty::<Vec<u8>>())
        });
        // Every package compiles as the root of its own component view, so
        // any of them — not only the graph root — may author `Independent`
        // selections over dependencies this loop already compiled. The
        // discovery output carries those selections out of a compile the
        // component-closure fence rejects; `unanswered_inputs` keeps the
        // request's graph so the retried compile can attach the published
        // descriptions without rebuilding it.
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
        occurrence: &PackageOccurrence<'_>,
        unanswered_inputs: &PackageCompilationInputs,
        assembled: &AssembledOccurrence,
    ) -> Result<bool, CompileResolvedPackageReviewsError> {
        let PackageOccurrence {
            key,
            scope,
            custody,
            position,
            purpose,
            checked_target,
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
        // Consumed evaluation usage stays on the carried review but was
        // reconciled by the pass that ran it, so none of it enters this
        // session's accounting.
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
        // Tolerated records are never stored, and the graph root is never
        // persisted — its checked compilation is the invocation's product.
        let reused = self.prior.and_then(|prior| {
            prior
                .occurrences
                .iter()
                .find(|occurrence| {
                    occurrence.position == position
                        && occurrence.purpose == purpose
                        && occurrence.entry == *entry
                        && occurrence.inputs == *unanswered_inputs
                        && occurrence.build_snapshot == *build_snapshot
                        && !occurrence.checked.tolerated_unsettled_service_fields()
                })
                .and_then(|occurrence| {
                    prior
                        .reviews
                        .reviews()
                        .get(occurrence.review)
                        .map(|review| (occurrence, review.clone()))
                })
        });
        let carried = reused
            .map(|(occurrence, review)| {
                (
                    occurrence.checked.clone(),
                    review,
                    occurrence.policy_canonical_bytes,
                    occurrence.obligation_ledger_bytes,
                )
            })
            .or_else(|| {
                if key == closure.graph().root() {
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
                ))?;
                let policy_canonical_bytes = record.policy_canonical_bytes()?;
                let obligation_ledger_bytes = record.obligation_ledger_bytes()?;
                let (review, facts) = record.into_review_and_facts(key, custody.resolution())?;
                // A persisted entailment discharge replays inert: recovery
                // keeps its obligation open by design, so a record carrying
                // one can never stand in for the fresh compile that closed it.
                if !review
                    .projection()
                    .contract_entailment_exit_membership_discharges()
                    .is_empty()
                    || !review
                        .projection()
                        .contract_entailment_assumption_discharges()
                        .is_empty()
                {
                    return None;
                }
                let occurrence = OccurrenceCompile::Cached(Box::new(facts));
                if occurrence.tolerated_unsettled_service_fields() {
                    return None;
                }
                Some((
                    occurrence,
                    review,
                    policy_canonical_bytes,
                    obligation_ledger_bytes,
                ))
            });
        let Some((compile, review, policy_canonical_bytes, obligation_ledger_bytes)) = carried
        else {
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
        self.retained_obligation_ledger_total = reserve_retained_obligation_ledger_bytes(
            self.retained_obligation_ledger_total,
            obligation_ledger_bytes,
        )
        .ok_or_else(|| {
            CompileResolvedPackageReviewsError::RetainedObligationLedgerBudget {
                package: key.clone(),
                maximum_bytes: MAXIMUM_RETAINED_ORDINARY_LEDGER_BYTES,
            }
        })?;
        reject_ungranted_restricted_requests(
            self.restricted_build_checkpoint,
            closure,
            key,
            &review,
        )?;
        // Consumers need the same entry-schema offer whether this
        // dependency compiled now, in a prior pass, or in a prior
        // invocation. An offer is not an accepted binding: only the
        // discovery pass may use it to propose a checked selection.
        if self.discovery == TargetEntryDiscovery::Dependencies {
            self.target_entry_candidates.extend(
                compile
                    .target_entry_candidate(key, checked_target)?
                    .map(|candidate| (purpose, candidate)),
            );
        }
        let review_position = self.reviews.len();
        self.review_positions[position][purpose_slot(purpose)] = Some(review_position);
        self.reviews.push(review);
        if key == closure.graph().root() {
            if self.retained_root_entry.is_some() {
                let OccurrenceCompile::Checked(checked) = &compile else {
                    return Err(CompileResolvedPackageReviewsError::Projection {
                        package: key.clone(),
                        diagnostics: vec![Diagnostic::error(
                            "the graph root's checked compilation is never persisted to the review cache",
                        )],
                    });
                };
                self.checked_root = Some(Box::new(checked.clone()));
            }
        } else {
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
            obligation_ledger_bytes,
            review: review_position,
            checked: compile,
        });
        self.carried_occurrences += 1;
        verify_selected_source_custody(scope, PackageSourceVerificationPhase::AfterCompilation)?;
        Ok(true)
    }

    /// Compile the occurrence under its assembled inputs, retrying once with
    /// published component descriptions when the component-closure fence
    /// rejects unanswered `Independent` selections. Returns the checked
    /// compilation and the exact inputs it consumed.
    fn compile_fresh(
        &mut self,
        occurrence: &PackageOccurrence<'_>,
        unanswered_inputs: PackageCompilationInputs,
        assembled: &AssembledOccurrence,
    ) -> Result<
        (compiler::CheckedCompilation, PackageCompilationInputs),
        CompileResolvedPackageReviewsError,
    > {
        let PackageOccurrence {
            key,
            scope,
            custody,
            position,
            purpose,
            checked_target,
        } = *occurrence;
        let closure = self.closure;
        let execution_profile = self.execution_profile;
        let restricted_build_checkpoint = self.restricted_build_checkpoint;
        let discovery = self.discovery;
        let target = checked_target.target_name();
        let entry = assembled.entry.as_path();
        let build_snapshot = &assembled.build_snapshot;
        let mut request = CheckedCompileRequest {
            build_execution_profile: execution_profile,
            build_dir: Some(package_build_root(
                self.build_session_root,
                key,
                custody.resolution(),
                purpose,
            )),
            package_inputs: Some(unanswered_inputs.clone()),
            filesystem_sponsor: Some(self.filesystem_sponsor.clone()),
            evaluation_sponsor: Some(self.evaluation_sponsor.clone()),
            build_snapshot: Some(build_snapshot.clone()),
            // An armed checkpoint binds the admitted activation's
            // restricted requests to this occurrence's granted meanings
            // before each request's own build effect executes.
            restricted_build_grants: restricted_build_checkpoint.map(|checkpoint| {
                Box::new(checkpoint.grants(
                    key.identity(),
                    PackageCheckedContext::new(purpose, checked_target, execution_profile),
                    closure.dependency_path(key),
                ))
                    as Box<dyn package_compilation::build_execution::RestrictedBuildGrants>
            }),
            collect_timings: self.preparation.collect_timings,
            // A discovery pass admits an entry whose `Service` fields still
            // lack their selection: this pass's whole product is the
            // nomination that supplies it.
            permit_unsettled_fused_service_fields: discovery == TargetEntryDiscovery::Dependencies,
            ..CheckedCompileRequest::new(entry, Some(target))
        };
        // A populated slot supplies only this package's binding-independent
        // parse frontier; the child still runs custody, build execution, and
        // checking, and the checkpoint is written back so a later pass or
        // candidate of this same closure prepares it once. A slot recorded
        // under another entry root belongs to a differently resolved package
        // at this position; it is replaced rather than consumed.
        request.prepared_source = match self.preparation.slots[position].take() {
            Some(slot) if slot.entry_root == *entry => Some(slot.prepared),
            _ => None,
        };
        request.retain_prepared_source = true;
        if request.prepared_source.is_none() {
            self.preparation.fresh_preparations += 1;
        }
        let outcome = compiler::compile_checked_reusing_builds(
            request,
            &self.preparation.pure_build_compilation,
        );
        self.record_build_accounting(&outcome);
        let CheckedCompileOutcome {
            result: checked,
            prepared_source: mut retained,
            independent_components: component_discovery,
            build_accounting: _,
        } = outcome;
        let (checked, occurrence_inputs) = match checked {
            Ok(checked) => (checked, unanswered_inputs),
            Err(diagnostics) => {
                let Some(inputs) = publish_independent_component_descriptions(
                    key,
                    purpose,
                    unanswered_inputs,
                    &component_discovery.selections,
                    &self.component_candidates,
                )?
                else {
                    return Err(CompileResolvedPackageReviewsError::Compilation {
                        package: key.clone(),
                        diagnostics,
                    });
                };
                // The descriptions a settled compile attached stay attached:
                // only a compile whose fence rejected unanswered selections
                // reaches here. One retry is the whole protocol — a selection
                // still unanswered after publication re-rejects at the same
                // fence, never as a fused edge.
                let request = CheckedCompileRequest {
                    build_execution_profile: execution_profile,
                    build_dir: Some(package_build_root(
                        self.build_session_root,
                        key,
                        custody.resolution(),
                        purpose,
                    )),
                    package_inputs: Some(inputs.clone()),
                    filesystem_sponsor: Some(self.filesystem_sponsor.clone()),
                    evaluation_sponsor: Some(self.evaluation_sponsor.clone()),
                    build_snapshot: Some(build_snapshot.clone()),
                    // The retried activation is the same checked
                    // occurrence: consent re-arms identically.
                    restricted_build_grants: restricted_build_checkpoint.map(|checkpoint| {
                        Box::new(checkpoint.grants(
                            key.identity(),
                            PackageCheckedContext::new(purpose, checked_target, execution_profile),
                            closure.dependency_path(key),
                        ))
                            as Box<dyn package_compilation::build_execution::RestrictedBuildGrants>
                    }),
                    collect_timings: self.preparation.collect_timings,
                    retain_prepared_source: true,
                    // The retried discovery child keeps the same
                    // settlement tolerance as the rejected attempt.
                    permit_unsettled_fused_service_fields: discovery
                        == TargetEntryDiscovery::Dependencies,
                    ..CheckedCompileRequest::new(entry, Some(target))
                };
                self.preparation.fresh_preparations += 1;
                let outcome = compiler::compile_checked_reusing_builds(
                    request,
                    &self.preparation.pure_build_compilation,
                );
                self.record_build_accounting(&outcome);
                retained = outcome.prepared_source;
                let checked = outcome.result.map_err(|diagnostics| {
                    CompileResolvedPackageReviewsError::Compilation {
                        package: key.clone(),
                        diagnostics,
                    }
                })?;
                (checked, inputs)
            }
        };
        self.preparation.slots[position] = retained.map(|prepared| PreparedPackageSource {
            entry_root: entry.to_path_buf(),
            prepared,
        });
        verify_selected_source_custody(scope, PackageSourceVerificationPhase::AfterCompilation)?;
        Ok((checked, occurrence_inputs))
    }

    /// Record the current request immediately, before any product/review error
    /// can discard it. Historical usage stays on the configuration; only this
    /// execution or this account's newly issued consumption receipt enters here.
    fn record_build_accounting(&mut self, outcome: &CheckedCompileOutcome) {
        use compilation_products::BuildEvaluationAccounting;
        match &outcome.build_accounting {
            BuildEvaluationAccounting::Executed(usage) => {
                if outcome.result.is_err() {
                    self.discarded_build_usages.push(*usage);
                } else {
                    self.session_usages.push(Some(*usage));
                }
            }
            BuildEvaluationAccounting::Consumed(receipt) => {
                self.consumed_build_charges.push(receipt.clone());
            }
            BuildEvaluationAccounting::Unreported => {
                if outcome.result.is_err() {
                    self.accounting_complete = false;
                }
            }
        }
    }

    fn verify_accounting(&self) -> Result<(), CompileResolvedPackageReviewsError> {
        verify_build_session_accounting(
            &self.session_usages,
            &self.discarded_build_usages,
            &self.consumed_build_charges,
            self.evaluation_sponsor,
        )
    }

    /// Project a freshly checked compile into its compiler-issued review,
    /// charge its retained budgets, persist it for later invocations, and
    /// retain it for this pass's consumers.
    fn project_and_retain_review(
        &mut self,
        occurrence: &PackageOccurrence<'_>,
        assembled: AssembledOccurrence,
        checked: compiler::CheckedCompilation,
        occurrence_inputs: PackageCompilationInputs,
    ) -> Result<(), CompileResolvedPackageReviewsError> {
        let PackageOccurrence {
            key,
            custody,
            position,
            purpose,
            checked_target,
            ..
        } = *occurrence;
        let closure = self.closure;
        let execution_profile = self.execution_profile;
        let AssembledOccurrence {
            entry,
            build_snapshot,
            proposed_entry,
        } = assembled;
        let entry = entry.as_path();
        checked
            .verify_current_source_consumption()
            .map_err(
                |diagnostics| CompileResolvedPackageReviewsError::SourceConsumptionDrift {
                    package: key.clone(),
                    diagnostics,
                },
            )?;
        let source_consumption_commitment = checked
            .sources()
            .source_consumption_commitment()
            .ok_or_else(
                || CompileResolvedPackageReviewsError::SourceConsumptionMissing {
                    package: key.clone(),
                },
            )?;
        let selected_build_machine_identity = checked
            .build_admission()
            .selected_build_machine_identity
            .as_deref()
            .ok_or_else(|| CompileResolvedPackageReviewsError::Projection {
                package: key.clone(),
                diagnostics: vec![Diagnostic::error(
                    "package review requires one exact selected build-machine identity",
                )],
            })?
            .to_owned();
        let build_observation_summary = checked.build().observation_summary.clone();
        let restricted_build_requests = checked.build_admission().restricted_build_requests.clone();
        let build_evaluation_usage = checked.build().evaluation_usage;

        let semantic_bindings = checked
            .execution()
            .accepted_semantic_bindings()
            .cloned()
            .collect();
        let generated_source_bundle =
            checked.package_generated_source_bundle().map_err(|error| {
                CompileResolvedPackageReviewsError::Projection {
                    package: key.clone(),
                    diagnostics: vec![Diagnostic::error(error)],
                }
            })?;
        if generated_source_bundle.package() != key.identity()
            || generated_source_bundle.source_consumption_commitment()
                != source_consumption_commitment
        {
            return Err(CompileResolvedPackageReviewsError::IdentityMismatch {
                package: key.clone(),
            });
        }
        let ReconstructedPackageReview {
            projection,
            canonical_rows,
            ledger: obligations,
            results: obligation_results,
        } = reconstruct_package_review(&checked).map_err(|diagnostics| {
            CompileResolvedPackageReviewsError::Projection {
                package: key.clone(),
                diagnostics,
            }
        })?;
        if projection.package() != key.identity() {
            return Err(CompileResolvedPackageReviewsError::IdentityMismatch {
                package: key.clone(),
            });
        }
        let (policy, policy_total) = super::policy::project(
            &checked,
            key,
            checked_target,
            self.retained_policy_canonical_total,
        )?;
        // This is a source compilation, not a link between independently
        // compiled artifacts. Psi already checks declaration visibility,
        // the selected relationship and every by-value representation demand.
        // The consumer Build owns the combined compilation's choice; a
        // dependency's historical review selection cannot constrain it.
        let policy_canonical_bytes = policy_total - self.retained_policy_canonical_total;
        self.retained_policy_canonical_total = policy_total;
        let mut semantic_binding_candidates = candidate_service_bindings(&checked, key)?;
        semantic_binding_candidates.extend(proposed_entry);
        semantic_binding_candidates.extend(candidate_hosted_exit_bindings(&checked, key)?);
        if self.discovery == TargetEntryDiscovery::Dependencies {
            self.target_entry_candidates.extend(
                candidate_target_entry_binding(&checked, key, checked_target)?
                    .map(|candidate| (purpose, candidate)),
            );
        }
        let canonical_review_bytes = projection.canonical_review_bytes().map_err(|error| {
            CompileResolvedPackageReviewsError::Encoding {
                package: key.clone(),
                error,
            }
        })?;
        let obligations_bytes =
            retained_obligation_ledger_bytes(&obligations).ok_or_else(|| {
                CompileResolvedPackageReviewsError::RetainedObligationLedgerBudget {
                    package: key.clone(),
                    maximum_bytes: MAXIMUM_RETAINED_ORDINARY_LEDGER_BYTES,
                }
            })?;
        self.retained_obligation_ledger_total = reserve_retained_obligation_ledger_bytes(
            self.retained_obligation_ledger_total,
            obligations_bytes,
        )
        .ok_or_else(|| {
            CompileResolvedPackageReviewsError::RetainedObligationLedgerBudget {
                package: key.clone(),
                maximum_bytes: MAXIMUM_RETAINED_ORDINARY_LEDGER_BYTES,
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
            projection,
            policy,
            canonical_review_bytes,
            canonical_rows: RetainedReviewRows(canonical_rows),
            obligations,
            obligation_results,
        };
        reject_ungranted_restricted_requests(
            self.restricted_build_checkpoint,
            closure,
            key,
            &review,
        )?;
        // Persist the review under the occurrence's post-settlement
        // inputs so a later invocation arriving at exactly these inputs
        // carries it without compiling. The key's inputs digest is the
        // retried compile's inputs when a retry joined independent
        // component descriptions, so an occurrence that required the
        // publication retry never matches an unanswered-inputs lookup —
        // the same shape the in-session carry takes.
        // A review carrying entailment discharges never replays faithfully:
        // recovery keeps their obligations open, so the record cannot
        // represent this compile's closed results.
        if key != closure.graph().root()
            && !checked.execution().tolerated_unsettled_service_fields()
            && review
                .projection()
                .contract_entailment_exit_membership_discharges()
                .is_empty()
            && review
                .projection()
                .contract_entailment_assumption_discharges()
                .is_empty()
            && let Some(directory) = custody.compiled_review_cache_dir()
            && let Some(cache) = super::super::review_cache::CompiledReviewCache::open(directory)
            && let Some(subject) = checked.sources().package_subject()
        {
            let component_description = super::component_publication::publish_checked_component(
                checked.clone(),
                proof_admission::AdmissionProfile::default(),
            )
            .ok();
            if let Some(record) = super::super::review_cache::record_compiled_review(
                &review,
                &checked,
                subject.consumed_units().to_vec(),
                component_description,
                policy_canonical_bytes,
                obligations_bytes,
            ) {
                cache.store(
                    &super::super::review_cache::compiled_review_key(
                        key.identity(),
                        purpose,
                        checked_target,
                        execution_profile,
                        entry,
                        &occurrence_inputs,
                        &build_snapshot,
                    ),
                    &record,
                );
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
            obligation_ledger_bytes: obligations_bytes,
            review: review_position,
            checked: OccurrenceCompile::Checked(checked.clone()),
        });
        if key == closure.graph().root() {
            if self.retained_root_entry.is_some() {
                self.checked_root = Some(Box::new(checked));
            }
        } else {
            self.component_candidates.insert(
                (key.identity(), purpose),
                OccurrenceCompile::Checked(checked),
            );
        }
        Ok(())
    }

    /// Reconcile this session's sponsored build usage and hand the retained
    /// reviews, root compilation and occurrence compiles to the caller.
    fn finish(self) -> Result<CompiledPackageReviews, CompileResolvedPackageReviewsError> {
        self.verify_accounting()?;
        Ok(CompiledPackageReviews {
            reviews: CompilerIssuedPackageReviewSet {
                reviews: self.reviews,
            },
            checked_root: self.checked_root,
            occurrences: self.occurrences,
            carried: self.carried_occurrences,
        })
    }
}

#[cfg(test)]
mod tests;

/// Join one retained review's projected restricted build requests against an
/// armed checkpoint, rejecting any request no grant covered. A consuming
/// compile joins each occurrence's projected restricted requests against its
/// retained consent here — before the review is retained and before the
/// generated-source bundle can hand off to a dependent activation — rather
/// than at the consuming operation's boundary after the whole closure
/// compiled. An ungranted occurrence rejects with its pending request
/// meanings attached, whether the review was compiled this pass or carried
/// forward under equal inputs.
fn reject_ungranted_restricted_requests(
    restricted_build_checkpoint: Option<&RestrictedBuildCheckpoint>,
    closure: &crate::resolution::graph::ResolvedPackageSourceClosure,
    key: &PackageKey,
    review: &CompilerIssuedPackageReview,
) -> Result<(), CompileResolvedPackageReviewsError> {
    let Some(checkpoint) = restricted_build_checkpoint else {
        return Ok(());
    };
    let projected = PackagePolicyAcceptance::from_policy(review.policy()).map_err(|error| {
        CompileResolvedPackageReviewsError::Projection {
            package: key.clone(),
            diagnostics: vec![Diagnostic::error(error.to_string())],
        }
    })?;
    let ungranted = checkpoint.ungranted_requests(
        review.key().identity(),
        review.checked_context(),
        closure.dependency_path(review.key()),
        projected
            .rows()
            .iter()
            .filter(|row| row.kind() == PackagePolicyRowKind::RestrictedBuildRequest)
            .map(PackageAcceptanceRow::canonical_text),
    );
    if !ungranted.is_empty() {
        return Err(
            CompileResolvedPackageReviewsError::UngrantedRestrictedBuildRequests {
                package: key.clone(),
                ungranted,
            },
        );
    }
    Ok(())
}

/// Publish the verified component descriptions a rejected compile's
/// discovered `Independent` selections name, and return the same inputs with
/// them attached. `Ok(None)` means the rejected compile authored no
/// answerable selection — its original diagnostics stand. A selection naming
/// the compiling package itself, or one whose provider carries no package
/// identity, has no dependency that could answer it. A selection naming a
/// package the closure never compiled, or whose checked compilation cannot
/// publish a description, fails the consumer's review directly instead of
/// retrying into the same fence.
fn publish_independent_component_descriptions(
    key: &PackageKey,
    purpose: DependencyPurpose,
    inputs: package_compilation::PackageCompilationInputs,
    independent_component_selections: &[compilation_products::IndependentComponentSelection],
    component_candidates: &BTreeMap<
        (semantic_vocabulary::PackageKeyIdentity, DependencyPurpose),
        OccurrenceCompile,
    >,
) -> Result<Option<package_compilation::PackageCompilationInputs>, CompileResolvedPackageReviewsError>
{
    let mut described = std::collections::BTreeSet::new();
    let mut descriptions = Vec::new();
    for selection in independent_component_selections {
        let Some(component) = selection.component else {
            continue;
        };
        if component == key.identity() || !described.insert(component) {
            continue;
        }
        let description = match component_candidates.get(&(component, purpose)) {
            Some(OccurrenceCompile::Checked(component_checked)) => {
                super::component_publication::publish_checked_component(
                    component_checked.clone(),
                    proof_admission::AdmissionProfile::default(),
                )
                .map_err(|diagnostics| {
                    CompileResolvedPackageReviewsError::Compilation {
                        package: key.clone(),
                        diagnostics,
                    }
                })?
            }
            Some(OccurrenceCompile::Cached(facts)) => {
                facts.component_description.clone().ok_or_else(|| {
                    CompileResolvedPackageReviewsError::Compilation {
                        package: key.clone(),
                        diagnostics: vec![Diagnostic::error(format!(
                            "an independent provider selection names dependency package `{}`, whose persisted review retains no component description",
                            inputs.package_name(component).unwrap_or("<unnamed>"),
                        ))],
                    }
                })?
            }
            None => {
                return Err(CompileResolvedPackageReviewsError::Compilation {
                    package: key.clone(),
                    diagnostics: vec![Diagnostic::error(format!(
                        "an independent provider selection names dependency package `{}`, which this closure never compiled",
                        inputs.package_name(component).unwrap_or("<unnamed>"),
                    ))],
                });
            }
        };
        descriptions.push(description);
    }
    if descriptions.is_empty() {
        return Ok(None);
    }
    inputs
        .with_independent_component_descriptions(descriptions)
        .map(Some)
        .map_err(
            |errors| CompileResolvedPackageReviewsError::CompilationInputs {
                package: key.clone(),
                errors,
            },
        )
}

struct OccurrenceBuildInputs {
    incoming: package_compilation::BuildDependencyOccurrence,
    snapshot: build_evaluation::BuildSnapshotRequest,
}

/// The spec's scheduling node is package/purpose/profiles, not import alias.
/// Equal edge assignments can reuse that node; incompatible complete maps
/// reject before execution rather than granting their union or multiplying
/// package identity. Product edges propagate their requester's purposes, as
/// in PackageOccurrenceRoster; build edges always select the build purpose.
fn reconcile_build_inputs(
    target_closure: &ExactTargetPackageSourceClosure<'_>,
    roster: &PackageOccurrenceRoster,
    request: Option<&build_evaluation::BuildSnapshotRequest>,
) -> Result<Vec<[Option<OccurrenceBuildInputs>; 2]>, Vec<Diagnostic>> {
    use build_evaluation::BuildSnapshotRequest;
    use package_compilation::BuildDependencyOccurrence;

    let graph = target_closure.source_closure().graph();
    let mut assigned: Vec<[Option<OccurrenceBuildInputs>; 2]> =
        graph.packages().iter().map(|_| [None, None]).collect();
    let Some(request) = request else {
        return Ok(assigned);
    };
    for (occurrence, _) in request.dependency_inputs() {
        let exists = graph.packages().iter().any(|node| {
            node.source().key().identity() == occurrence.requester()
                && node.dependencies().iter().any(|edge| {
                    edge.purpose() == occurrence.purpose()
                        && edge.alias().as_str() == occurrence.alias()
                        && edge.target().identity() == occurrence.target()
                })
        });
        if !exists {
            return Err(vec![Diagnostic::error(format!(
                "named build inputs are assigned to a dependency occurrence that does not exist: {occurrence:?}"
            ))]);
        }
    }
    let empty = BuildSnapshotRequest::new(std::iter::empty::<Vec<u8>>());
    for node in graph.packages() {
        let requester = node.source().key();
        for edge in node.dependencies() {
            let occurrence = BuildDependencyOccurrence::new(
                requester.identity(),
                edge.purpose(),
                edge.alias().as_str(),
                edge.target().identity(),
            );
            let slots = request
                .dependency_input_slots(&occurrence)
                .unwrap_or_else(|| empty.inputs());
            let position = graph
                .package_position(edge.target())
                .expect("validated graph edge target");
            for &requester_purpose in roster
                .purposes(requester)
                .expect("closed occurrence roster")
            {
                let purpose = if edge.purpose().is_product() {
                    requester_purpose
                } else {
                    DependencyPurpose::Build
                };
                let assignment = &mut assigned[position][purpose_slot(purpose)];
                if let Some(previous) = assignment {
                    if previous.snapshot.inputs() != slots {
                        return Err(vec![Diagnostic::error(format!(
                            "conflicting named build input assignments for shared {:?} activation of `{}`: {:?} and {:?}; assignments must match exactly, including unassigned inputs",
                            purpose,
                            edge.target().name().as_str(),
                            previous.incoming,
                            occurrence,
                        ))]);
                    }
                } else {
                    *assignment = Some(OccurrenceBuildInputs {
                        incoming: occurrence.clone(),
                        snapshot: empty.clone().with_inputs(
                            slots
                                .iter()
                                .map(|(name, input)| (name.clone(), input.clone())),
                        )?,
                    });
                }
            }
        }
    }
    Ok(assigned)
}
