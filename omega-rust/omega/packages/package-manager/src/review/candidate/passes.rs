//! Select discovery/bound policy passes and ask application Build to execute them.
//!
//! All products share discovery and final checking. The top-level invocation
//! owns the sponsored session borrowed by every target, candidate and review pass;
//! `occurrences` admits dependency work and joins compiler evidence to policy.

mod occurrences;
mod policy;

use super::semantic_bindings::{candidate_semantic_binding_inputs, semantic_bindings_by_consumer};
use super::{
    CompileResolvedPackageReviewsError, CompilerIssuedPackageReviewSet,
    ConsumerScopedSemanticBindingReviewInput,
};
use crate::build_session::PackageBuildSession;
use crate::resolution::graph::ExactTargetPackageSourceClosure;
use crate::review::timings;
use artifacts::compile_timings::counters;
use occurrences::TargetEntryDiscovery;
pub use occurrences::{
    PackageReviewCompletion, PackageReviewComponent, PackageReviewExecutor,
    PackageReviewOccurrence, PackageReviewPass, PackageReviewRound, PackageReviewWork,
    RetainedPackageReviews,
};

pub(crate) use crate::build_session::PackageSourceBindings;

/// Observational review of an exact target closure and root Build inventory.
/// It projects restricted requests for decisions but never exercises grants.
/// Accepted production builds admit those grants through accepted_compilation.
#[derive(Clone, Copy)]
pub(crate) struct CandidateReview<'request, 'closure> {
    pub target_closure: &'request ExactTargetPackageSourceClosure<'closure>,
    pub root_build_snapshot: Option<&'request build_evaluation::BuildSnapshotRequest>,
}

/// Review an explicit [`CandidateReview`] with resolved source bindings.
/// The compiler session owns source preparation and exact language judgments;
/// changed sources and selections still reject through the ordinary custody
/// and checkpoint checks. Inspection must not derive policy
/// from a broader input view than the invocation it is reviewing.
/// The invocation's owner lends the same session to every target and round;
/// this operation neither reserves a new sponsor nor disposes that owner.
pub(crate) fn review_resolved_packages(
    candidate: CandidateReview<'_, '_>,
    session: &mut PackageBuildSession,
    source_bindings: &mut PackageSourceBindings,
    execute: &mut PackageReviewExecutor<'_>,
) -> Result<CompilerIssuedPackageReviewSet, CompileResolvedPackageReviewsError> {
    session.require_open()?;
    review_candidate(candidate, source_bindings, execute).map(|compiled| compiled.reviews)
}

/// The candidate route shared by package commands and read-only audit;
/// see [`CandidateReview`] for what the request carries.
fn review_candidate(
    candidate: CandidateReview<'_, '_>,
    source_bindings: &mut PackageSourceBindings,
    execute: &mut PackageReviewExecutor<'_>,
) -> Result<RetainedPackageReviews, CompileResolvedPackageReviewsError> {
    let CandidateReview {
        target_closure,
        root_build_snapshot,
    } = candidate;
    let _requested = timings::requested(source_bindings.collect_timings);
    let _stage = timings::stage("candidate_compilation");
    source_bindings.size_for(target_closure.source_closure());
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
            review_pass(
                target_closure,
                &[],
                root_build_snapshot,
                TargetEntryDiscovery::Dependencies,
                None,
                source_bindings,
                execute,
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
    // Custody verification and consumer-scoped binding checks still run per pass.
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
        review_pass(
            target_closure,
            &discovered,
            root_build_snapshot,
            TargetEntryDiscovery::Disabled,
            Some(&preliminary),
            source_bindings,
            execute,
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
    run: impl FnOnce() -> Result<RetainedPackageReviews, CompileResolvedPackageReviewsError>,
) -> Result<RetainedPackageReviews, CompileResolvedPackageReviewsError> {
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

fn review_pass(
    target_closure: &ExactTargetPackageSourceClosure<'_>,
    bindings: &[ConsumerScopedSemanticBindingReviewInput],
    root_build_snapshot: Option<&build_evaluation::BuildSnapshotRequest>,
    discovery: TargetEntryDiscovery,
    prior: Option<&RetainedPackageReviews>,
    source_bindings: &mut PackageSourceBindings,
    execute: &mut PackageReviewExecutor<'_>,
) -> Result<RetainedPackageReviews, CompileResolvedPackageReviewsError> {
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
    let plan = PackageReviewPass::admit(
        target_closure,
        execution_profile,
        &roster,
        &bindings,
        root_build_snapshot,
        discovery,
        prior,
    )?;
    execute(plan, source_bindings)
}
