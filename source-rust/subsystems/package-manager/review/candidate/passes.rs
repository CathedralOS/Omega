//! Resolve the review's dependency roster and discover its consumer-scoped bindings.
//! A bound pass recovers evidence against the final inputs and shares the invocation's
//! accounting with the discovery pass.

use crate::resolution::graph::CanonicalSourceClosureSubject;
use package_compilation::build_execution::BuildSnapshotRequest;
#[path = "passes_occurrences.rs"]
mod occurrences;

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
pub use occurrences::{PackageReviewExecutor, PackageReviewPass, RetainedPackageReviews};

pub(crate) use crate::build_session::PackageSourceBindings;

#[derive(Clone, Copy)]
pub(crate) struct CandidateReview<'request, 'closure> {
    pub target_closure: &'request ExactTargetPackageSourceClosure<'closure>,
    pub root_build_snapshot: Option<&'request BuildSnapshotRequest>,
}

pub(crate) fn review_resolved_packages(
    candidate: CandidateReview<'_, '_>,
    session: &mut PackageBuildSession,
    source_bindings: &mut PackageSourceBindings,
    execute: &mut PackageReviewExecutor<'_>,
) -> Result<CompilerIssuedPackageReviewSet, CompileResolvedPackageReviewsError> {
    session.require_open()?;
    let result = review_candidate(candidate, source_bindings, execute);
    let accounting = match &result {
        Ok(compiled) => compiled.source_completion_accounting(),
        Err(error) => error
            .source_completion_accounting()
            .cloned()
            .unwrap_or_default(),
    };
    session.record_source_completion(accounting);
    result.map(|compiled| compiled.reviews)
}

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
    let accounting = preliminary.source_completion_accounting();
    let discovered = candidate_semantic_binding_inputs(
        &preliminary.reviews,
        target_closure.source_closure().graph().root(),
    )
    .map_err(|error| error.with_source_completion_accounting(accounting.clone()))?;
    if discovered.is_empty() {
        return Ok(preliminary);
    }
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
    .map_err(|error| error.with_source_completion_accounting(accounting))
}

fn note_pass_accounting(
    pass: &'static str,
    run: impl FnOnce() -> Result<RetainedPackageReviews, CompileResolvedPackageReviewsError>,
) -> Result<RetainedPackageReviews, CompileResolvedPackageReviewsError> {
    let compilations_before = counters::checked_compilation_call_count();
    let frontiers_before = counters::typed_frontier_reuse_count();
    let settled_before = counters::settled_check_totals();
    let result = run()?;
    let occurrences = result.occurrences.len();
    let carried = result.carried;
    timings::note(|| {
        let (settled_calls, settled_micros) = counters::settled_check_totals();
        format!(
            "review_pass {pass} occurrences={occurrences} compiled={} carried={carried} frontier_reused={} settled_checks={} settled_micros={}",
            counters::checked_compilation_call_count() - compilations_before,
            counters::typed_frontier_reuse_count() - frontiers_before,
            settled_calls - settled_before.0,
            settled_micros - settled_before.1,
        )
    });
    Ok(result)
}

fn review_pass(
    target_closure: &ExactTargetPackageSourceClosure<'_>,
    bindings: &[ConsumerScopedSemanticBindingReviewInput],
    root_build_snapshot: Option<&BuildSnapshotRequest>,
    discovery: TargetEntryDiscovery,
    prior: Option<&RetainedPackageReviews>,
    source_bindings: &mut PackageSourceBindings,
    execute: &mut PackageReviewExecutor<'_>,
) -> Result<RetainedPackageReviews, CompileResolvedPackageReviewsError> {
    let closure = target_closure.source_closure();
    let execution_profile = target::TargetProfile::host_if_supported();
    let source = CanonicalSourceClosureSubject::from_resolved(target_closure, Default::default())
        .map_err(|error| {
        CompileResolvedPackageReviewsError::new(
            crate::review::CompileResolvedPackageReviewCause::Projection {
                package: closure.graph().root().clone(),
                diagnostics: vec![diagnostics::Diagnostic::error(error.to_string())],
            },
        )
    })?;
    let roster = crate::lock::PackageOccurrenceRoster::derive(&source).map_err(|error| {
        CompileResolvedPackageReviewsError::new(
            crate::review::CompileResolvedPackageReviewCause::Projection {
                package: closure.graph().root().clone(),
                diagnostics: vec![diagnostics::Diagnostic::error(error.to_string())],
            },
        )
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
