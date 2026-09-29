//! Candidate review workflow: select bindings, compile, then retain the requested product.
//!
//! All products share discovery and final checking. Each pass owns a disposable
//! sponsored session; package_pass owns dependency-order compilation and projection.

mod component_publication;
mod ledger;
mod package_pass;
mod policy;
mod session;
mod session_accounting;
#[cfg(test)]
mod tests;

use super::semantic_bindings::{candidate_semantic_binding_inputs, semantic_bindings_by_consumer};
use super::{
    CompileResolvedPackageReviewsError, CompilerIssuedPackageReviewSet,
    ConsumerScopedSemanticBindingReviewInput, ReviewedPackageProductionCandidate,
};
use crate::resolution::graph::{ExactTargetPackageSourceClosure, ResolvedPackageSourceClosure};
use crate::review::restricted_build_grants::RestrictedBuildCheckpoint;
use crate::review::timings;
use artifacts::compile_timings::counters;
use package_pass::{CompiledPackageReviews, TargetEntryDiscovery};
use session::ReviewBuildSession;
use std::path::Path;

/// Target-independent prepared package sources retained across candidate
/// reviews of one resolved source closure.
///
/// Slots follow the resolver's package positions, each keyed by the entry
/// root it was prepared for. A populated slot supplies only that package's
/// binding-independent parse frontier to a later pass or candidate: custody
/// verification, build execution, and checking always run again, and each
/// child's own root-path and source-input validation rejects a checkpoint
/// that no longer names its package's prepared sources. The store holds no
/// checked result, binding decision, review row, or build output, so nothing
/// verified can be replayed merely because source bytes match.
///
/// One store belongs to one resolved closure. Callers reviewing several exact
/// targets of the same closure share it; a differently shaped closure drops
/// stale slots rather than risk a misplaced checkpoint, and a slot whose
/// recorded entry root differs from the request's is prepared fresh.
#[derive(Default)]
pub struct CandidateSourcePreparation {
    slots: Vec<Option<PreparedPackageSource>>,
    /// Fresh preparations performed through this store; witnesses that a
    /// repeated or cross-target candidate prepared each package once.
    fresh_preparations: usize,
    /// Collect the internal stage ladder on every fresh preparation this
    /// store performs. Off by default: the ladder measures nothing until a
    /// caller asks for it, and the rows stay on the checked record until a
    /// report-facing leg prints them.
    collect_timings: bool,
}

/// One package's retained parse frontier and the entry root it belongs to.
/// The root is the exact identity the child request validates, so a same
/// count but differently resolved closure cannot consume a misplaced slot.
struct PreparedPackageSource {
    entry_root: std::path::PathBuf,
    prepared: compilation_products::PreparedCheckedSource,
}

impl CandidateSourcePreparation {
    /// An empty store; the first candidate sizes it to that closure's graph.
    pub fn new() -> Self {
        Self::default()
    }

    /// Slots pre-sized for this closure's package graph.
    pub fn for_closure(closure: &ResolvedPackageSourceClosure) -> Self {
        Self {
            slots: empty_slots(closure.graph().packages().len()),
            fresh_preparations: 0,
            collect_timings: false,
        }
    }

    /// Prepare fresh sources with the stage ladder enabled.
    pub fn collecting_timings(mut self, collect_timings: bool) -> Self {
        self.collect_timings = collect_timings;
        self
    }

    /// Size slots to this closure's package positions. A differently shaped
    /// closure starts over empty rather than risk a misplaced checkpoint.
    fn size_for(&mut self, closure: &ResolvedPackageSourceClosure) {
        let package_count = closure.graph().packages().len();
        if self.slots.len() != package_count {
            self.slots = empty_slots(package_count);
        }
    }

    /// Fresh source preparations performed through this store.
    #[cfg(test)]
    fn fresh_preparation_count(&self) -> usize {
        self.fresh_preparations
    }
}

fn empty_slots(count: usize) -> Vec<Option<PreparedPackageSource>> {
    std::iter::repeat_with(|| None).take(count).collect()
}

/// How this invocation obtains consumer-scoped semantic bindings.
/// Explicit input is checked as supplied; it does not trigger discovery or admission.
#[derive(Clone, Copy)]
pub enum SemanticBindingReview<'a> {
    /// Discover package-owned surfaces, then recheck with the proposed bindings.
    Discover,
    /// Compile once with exactly these bindings, including an explicitly empty set.
    Explicit(&'a [ConsumerScopedSemanticBindingReviewInput]),
}

/// One candidate compilation of an exact target closure: where it stages, the
/// root activation's build snapshot, and the restricted-request consent the
/// invoking operation holds. Observational review carries no checkpoint:
/// inspection never issues grants, so projected requests survive to the
/// decision document. A consuming operation (`check_project`,
/// `check_locked_sources`, `compile_project`) supplies its accepted target's
/// checkpoint, so each occurrence's projected request joins its granted
/// meaning inside the pass, before that request's own build effect executes.
/// A supplied snapshot selects only the root activation's inputs and required
/// outputs in every review pass; dependencies keep their own inventories.
#[derive(Clone, Copy)]
pub struct CandidateCompilation<'request, 'closure> {
    pub target_closure: &'request ExactTargetPackageSourceClosure<'closure>,
    pub build_root: &'request Path,
    pub root_build_snapshot: Option<&'request build_evaluation::BuildSnapshotRequest>,
    pub restricted_build_checkpoint: Option<&'request RestrictedBuildCheckpoint>,
}

impl<'request, 'closure> CandidateCompilation<'request, 'closure> {
    /// Observational compilation: no root snapshot and no granted consent.
    pub fn new(
        target_closure: &'request ExactTargetPackageSourceClosure<'closure>,
        build_root: &'request Path,
    ) -> Self {
        Self {
            target_closure,
            build_root,
            root_build_snapshot: None,
            restricted_build_checkpoint: None,
        }
    }
}

/// Compile the exact dependency closure and return its compiler-issued reviews.
/// No review escapes its disposable session. Binding discovery is not acceptance.
pub fn compile_resolved_package_reviews(
    target_closure: &ExactTargetPackageSourceClosure<'_>,
    build_root: &Path,
    bindings: SemanticBindingReview<'_>,
) -> Result<CompilerIssuedPackageReviewSet, CompileResolvedPackageReviewsError> {
    compile_resolved_package_reviews_reusing(
        CandidateCompilation::new(target_closure, build_root),
        bindings,
        &mut CandidateSourcePreparation::for_closure(target_closure.source_closure()),
    )
}

/// The same candidate review under an explicit [`CandidateCompilation`],
/// retaining binding-independent source preparation in the caller's store. A
/// command reviewing several targets of one resolved closure prepares each
/// package once; changed sources and selections still reject through the
/// ordinary custody and checkpoint checks. Inspection must not derive policy
/// from a broader input view than the invocation it is reviewing.
pub fn compile_resolved_package_reviews_reusing(
    candidate: CandidateCompilation<'_, '_>,
    bindings: SemanticBindingReview<'_>,
    preparation: &mut CandidateSourcePreparation,
) -> Result<CompilerIssuedPackageReviewSet, CompileResolvedPackageReviewsError> {
    compile_candidate(candidate, bindings, None, preparation).map(|compiled| compiled.reviews)
}

/// Retain the application root from the same final pass that produced its reviews.
/// This checked product grants no native authority.
/// A supplied snapshot selects only the root activation's inputs and required
/// outputs in every review pass; dependencies keep their own package inventories.
pub fn compile_resolved_package_candidate_for_production(
    target_closure: &ExactTargetPackageSourceClosure<'_>,
    build_root: &Path,
    bindings: SemanticBindingReview<'_>,
    root_build_snapshot: Option<&build_evaluation::BuildSnapshotRequest>,
) -> Result<ReviewedPackageProductionCandidate, CompileResolvedPackageReviewsError> {
    production_candidate_reusing(
        CandidateCompilation {
            root_build_snapshot,
            ..CandidateCompilation::new(target_closure, build_root)
        },
        bindings,
        &mut CandidateSourcePreparation::for_closure(target_closure.source_closure()),
    )
}

/// The production route retaining source preparation in the caller's store.
/// An operation that compiles several exact targets of one resolved closure
/// shares the store, so each package's binding-independent parse frontier is
/// built once while custody admission, build execution, and checking still
/// run per target context.
pub(crate) fn production_candidate_reusing(
    candidate: CandidateCompilation<'_, '_>,
    bindings: SemanticBindingReview<'_>,
    preparation: &mut CandidateSourcePreparation,
) -> Result<ReviewedPackageProductionCandidate, CompileResolvedPackageReviewsError> {
    let target_closure = candidate.target_closure;
    let closure = target_closure.source_closure();
    let root = closure.graph().root().clone();
    if closure.root_role() != package_compilation::BuildDeclarationKind::Application {
        return Err(
            CompileResolvedPackageReviewsError::InvalidProductionRootRole {
                package: root,
                role: closure.root_role(),
            },
        );
    }
    let root_path = closure
        .source_root(&root)
        .expect("validated source closure retains its root custody")
        .join("main.omg");
    let compiled = compile_candidate(candidate, bindings, Some(&root_path), preparation)?;
    let checked_root = compiled.checked_root.ok_or_else(|| {
        CompileResolvedPackageReviewsError::IdentityMismatch {
            package: root.clone(),
        }
    })?;
    Ok(ReviewedPackageProductionCandidate {
        reviews: compiled.reviews,
        root,
        root_path,
        root_role: closure.root_role(),
        target_profile: target_closure.target_profile(),
        checked_root,
    })
}

/// CHECK accepts either project role and preserves the requested entry through
/// discovery and final binding, without entering production. This
/// observational route carries no restricted-request consent; a consuming
/// check supplies its accepted target's checkpoint through
/// [`compile_candidate_for_check_reusing`].
pub(crate) fn compile_resolved_package_candidate_for_check(
    target_closure: &ExactTargetPackageSourceClosure<'_>,
    build_root: &Path,
    entry_path: &Path,
    root_build_snapshot: Option<&build_evaluation::BuildSnapshotRequest>,
    collect_timings: bool,
) -> Result<
    (compiler::CheckedCompilation, CompilerIssuedPackageReviewSet),
    CompileResolvedPackageReviewsError,
> {
    compile_candidate_for_check_reusing(
        CandidateCompilation {
            root_build_snapshot,
            ..CandidateCompilation::new(target_closure, build_root)
        },
        entry_path,
        &mut CandidateSourcePreparation::for_closure(target_closure.source_closure())
            .collecting_timings(collect_timings),
    )
}

/// The consuming check route retaining source preparation in the caller's
/// store. An operation checking several exact targets of one resolved closure
/// shares the store, so each package's binding-independent parse frontier is
/// built once while custody admission, build execution, and checking still
/// run per target context. A consuming check supplies its accepted target's
/// restricted-request checkpoint, so retained consent gates each projected
/// request's own build effect inside the pass rather than at the operation's
/// boundary after it; the store's own `collecting_timings` choice governs the
/// stage ladder.
pub(crate) fn compile_candidate_for_check_reusing(
    candidate: CandidateCompilation<'_, '_>,
    entry_path: &Path,
    preparation: &mut CandidateSourcePreparation,
) -> Result<
    (compiler::CheckedCompilation, CompilerIssuedPackageReviewSet),
    CompileResolvedPackageReviewsError,
> {
    let compiled = compile_candidate(
        candidate,
        SemanticBindingReview::Discover,
        Some(entry_path),
        preparation,
    )?;
    let CompiledPackageReviews {
        reviews,
        checked_root,
        occurrences: _,
        carried: _,
    } = compiled;
    checked_root
        .map(|checked| (*checked, reviews))
        .ok_or_else(|| CompileResolvedPackageReviewsError::IdentityMismatch {
            package: candidate
                .target_closure
                .source_closure()
                .graph()
                .root()
                .clone(),
        })
}

/// The one candidate route every review, production and check entry runs;
/// see [`CandidateCompilation`] for what the request carries.
fn compile_candidate(
    candidate: CandidateCompilation<'_, '_>,
    bindings: SemanticBindingReview<'_>,
    retained_root_entry: Option<&Path>,
    preparation: &mut CandidateSourcePreparation,
) -> Result<CompiledPackageReviews, CompileResolvedPackageReviewsError> {
    let CandidateCompilation {
        target_closure,
        build_root,
        root_build_snapshot,
        restricted_build_checkpoint,
    } = candidate;
    let _requested = timings::requested(preparation.collect_timings);
    let _stage = timings::stage("candidate_compilation");
    // The passes below check each package of the closure again, usually on
    // an identical program; retain those checks on first sight.
    let _retention = compiler::retain_repeated_checks();
    preparation.size_for(target_closure.source_closure());
    if let SemanticBindingReview::Explicit(inputs) = bindings {
        let _pass = timings::subject_stage(|| {
            format!(
                "review_pass explicit root={}",
                target_closure
                    .source_closure()
                    .graph()
                    .root()
                    .name()
                    .as_str()
            )
        });
        return note_pass_accounting("explicit", || {
            compile_pass(
                target_closure,
                build_root,
                inputs,
                retained_root_entry,
                root_build_snapshot,
                restricted_build_checkpoint,
                TargetEntryDiscovery::Disabled,
                None,
                preparation,
            )
        });
    }
    let preliminary = {
        let _pass = timings::subject_stage(|| {
            format!(
                "review_pass discovery root={}",
                target_closure
                    .source_closure()
                    .graph()
                    .root()
                    .name()
                    .as_str()
            )
        });
        note_pass_accounting("discovery", || {
            compile_pass(
                target_closure,
                build_root,
                &[],
                retained_root_entry,
                root_build_snapshot,
                restricted_build_checkpoint,
                TargetEntryDiscovery::Dependencies,
                None,
                preparation,
            )
        })?
    };
    let discovered = candidate_semantic_binding_inputs(
        &preliminary.reviews,
        target_closure.source_closure().graph().root(),
    )?;
    if discovered.is_empty() {
        return Ok(preliminary);
    }
    // Discovery cannot survive as a checked product when it proposed bindings,
    // but an occurrence whose inputs the discovered bindings leave untouched
    // is the same checked product under the same inputs: this pass carries
    // those compiles forward and only compiles what the bindings changed.
    // Custody verification and grant-consistency checks still run per pass.
    let _pass = timings::subject_stage(|| {
        format!(
            "review_pass bound root={} discovered_bindings={}",
            target_closure
                .source_closure()
                .graph()
                .root()
                .name()
                .as_str(),
            discovered.len()
        )
    });
    note_pass_accounting("bound", || {
        compile_pass(
            target_closure,
            build_root,
            &discovered,
            retained_root_entry,
            root_build_snapshot,
            restricted_build_checkpoint,
            TargetEntryDiscovery::Disabled,
            Some(&preliminary),
            preparation,
        )
    })
}

/// Run one review pass, then attribute its checked work beside the pass's
/// stage line when review timings are enabled: how many occurrences it
/// holds, how many it actually compiled, how many a prior pass's identical
/// compile carried forward untouched, how many typed frontiers a later
/// occurrence reused instead of re-assembling identical source, and what the
/// two whole-program checkpoints inside each compiled occurrence cost. The
/// preliminary row is the pre-build declaration-authority check. The
/// `*_checks` counts are requests to `lower_typed_trees`; the `*_executed`
/// counts are only the checks that actually ran, so a pass whose identical
/// frozen graphs the exact-input memo served shows requests without
/// executions, and its remaining micros name the residual
/// preliminary-versus-settled duplication rather than a repeated check.
fn note_pass_accounting(
    pass: &'static str,
    run: impl FnOnce() -> Result<CompiledPackageReviews, CompileResolvedPackageReviewsError>,
) -> Result<CompiledPackageReviews, CompileResolvedPackageReviewsError> {
    let compilations_before = counters::checked_compilation_call_count();
    let frontiers_before = counters::typed_frontier_reuse_count();
    let preliminary_before = counters::preliminary_check_totals();
    let settled_before = counters::settled_check_totals();
    let preliminary_executed_before =
        compiler::RepeatedCheckRetention::executed_preliminary_check_count();
    let settled_executed_before = compiler::RepeatedCheckRetention::executed_settled_check_count();
    let result = run()?;
    let occurrences = result.occurrences.len();
    let carried = result.carried;
    timings::note(|| {
        let (preliminary_calls, preliminary_micros) = counters::preliminary_check_totals();
        let (settled_calls, settled_micros) = counters::settled_check_totals();
        format!(
            "review_pass {pass} occurrences={occurrences} compiled={} carried={carried} frontier_reused={} preliminary_checks={} preliminary_executed={} preliminary_micros={} settled_checks={} settled_executed={} settled_micros={}",
            counters::checked_compilation_call_count() - compilations_before,
            counters::typed_frontier_reuse_count() - frontiers_before,
            preliminary_calls - preliminary_before.0,
            compiler::RepeatedCheckRetention::executed_preliminary_check_count()
                - preliminary_executed_before,
            preliminary_micros - preliminary_before.1,
            settled_calls - settled_before.0,
            compiler::RepeatedCheckRetention::executed_settled_check_count()
                - settled_executed_before,
            settled_micros - settled_before.1,
        )
    });
    Ok(result)
}

fn compile_pass(
    target_closure: &ExactTargetPackageSourceClosure<'_>,
    build_root: &Path,
    bindings: &[ConsumerScopedSemanticBindingReviewInput],
    retained_root_entry: Option<&Path>,
    root_build_snapshot: Option<&build_evaluation::BuildSnapshotRequest>,
    restricted_build_checkpoint: Option<&RestrictedBuildCheckpoint>,
    discovery: TargetEntryDiscovery,
    prior: Option<&CompiledPackageReviews>,
    preparation: &mut CandidateSourcePreparation,
) -> Result<CompiledPackageReviews, CompileResolvedPackageReviewsError> {
    let closure = target_closure.source_closure();
    let execution_profile = target::TargetProfile::host_if_supported();
    let source = crate::resolution::graph::CanonicalSourceClosureSubject::from_resolved(
        target_closure,
        Default::default(),
    )
    .map_err(|error| CompileResolvedPackageReviewsError::Projection {
        package: closure.graph().root().clone(),
        diagnostics: vec![diagnostics::Diagnostic::error(error.to_string())],
    })?;
    let roster = crate::lock::PackageOccurrenceRoster::derive(&source).map_err(|error| {
        CompileResolvedPackageReviewsError::Projection {
            package: closure.graph().root().clone(),
            diagnostics: vec![diagnostics::Diagnostic::error(error.to_string())],
        }
    })?;
    let bindings =
        semantic_bindings_by_consumer(target_closure, execution_profile, &roster, bindings)?;
    let session = ReviewBuildSession::create(build_root)?;
    let result = package_pass::compile_dependency_closure(
        target_closure,
        execution_profile,
        &roster,
        session.root(),
        session.filesystem_sponsor(),
        session.evaluation_sponsor(),
        &bindings,
        retained_root_entry,
        root_build_snapshot,
        restricted_build_checkpoint,
        discovery,
        prior,
        preparation,
    );
    let compiled = session.dispose(result)?;
    if retained_root_entry.is_some() {
        let root = closure.graph().root();
        let checked = compiled.checked_root.as_ref().ok_or_else(|| {
            CompileResolvedPackageReviewsError::IdentityMismatch {
                package: root.clone(),
            }
        })?;
        let subject = checked.package_compilation_subject();
        if subject.map(|subject| subject.root()) != Some(root.identity())
            || subject.map(|subject| subject.root_role()) != Some(closure.root_role())
            || checked.selected_target_profile() != Some(target_closure.target_profile())
        {
            return Err(CompileResolvedPackageReviewsError::IdentityMismatch {
                package: root.clone(),
            });
        }
    }
    Ok(compiled)
}
