//! Candidate review workflow: select bindings, then compile.
//!
//! All products share discovery and final checking. Each pass owns a disposable
//! sponsored session; package_pass owns dependency-order compilation and projection.

mod ledger;
mod package_pass;
mod policy;

use super::semantic_bindings::{candidate_semantic_binding_inputs, semantic_bindings_by_consumer};
use super::{
    CompileResolvedPackageReviewsError, CompilerIssuedPackageReviewSet,
    ConsumerScopedSemanticBindingReviewInput,
};
use crate::build_session::PackageBuildSession;
use crate::resolution::graph::ExactTargetPackageSourceClosure;
use crate::review::restricted_build_grants::RestrictedBuildCheckpoint;
use crate::review::timings;
use artifacts::compile_timings::counters;
use package_pass::{CompiledPackageReviews, TargetEntryDiscovery};
use std::path::Path;

pub use crate::build_session::PackageSourcePreparation as CandidateSourcePreparation;

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
    compile_candidate(candidate, bindings, preparation).map(|compiled| compiled.reviews)
}

/// The one candidate route every review and check entry runs;
/// see [`CandidateCompilation`] for what the request carries.
fn compile_candidate(
    candidate: CandidateCompilation<'_, '_>,
    bindings: SemanticBindingReview<'_>,
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
/// preliminary row is the pre-build declaration-authority check; every
/// `*_checks` request to `lower_typed_trees` runs a check.
fn note_pass_accounting(
    pass: &'static str,
    run: impl FnOnce() -> Result<CompiledPackageReviews, CompileResolvedPackageReviewsError>,
) -> Result<CompiledPackageReviews, CompileResolvedPackageReviewsError> {
    let compilations_before = counters::checked_compilation_call_count();
    let frontiers_before = counters::typed_frontier_reuse_count();
    let preliminary_before = counters::preliminary_check_totals();
    let settled_before = counters::settled_check_totals();
    let result = run()?;
    let occurrences = result.occurrences.len();
    let carried = result.carried;
    timings::note(|| {
        let (preliminary_calls, preliminary_micros) = counters::preliminary_check_totals();
        let (settled_calls, settled_micros) = counters::settled_check_totals();
        format!(
            "review_pass {pass} occurrences={occurrences} compiled={} carried={carried} frontier_reused={} preliminary_checks={} preliminary_micros={} settled_checks={} settled_micros={}",
            counters::checked_compilation_call_count() - compilations_before,
            counters::typed_frontier_reuse_count() - frontiers_before,
            preliminary_calls - preliminary_before.0,
            preliminary_micros - preliminary_before.1,
            settled_calls - settled_before.0,
            settled_micros - settled_before.1,
        )
    });
    Ok(result)
}

fn compile_pass(
    target_closure: &ExactTargetPackageSourceClosure<'_>,
    build_root: &Path,
    bindings: &[ConsumerScopedSemanticBindingReviewInput],
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
    let session = PackageBuildSession::create(build_root)?;
    let result = package_pass::compile_dependency_closure(
        target_closure,
        execution_profile,
        &roster,
        session.root(),
        session.filesystem_sponsor(),
        session.evaluation_sponsor(),
        &bindings,
        root_build_snapshot,
        restricted_build_checkpoint,
        discovery,
        prior,
        preparation,
    );
    session.dispose(result)
}
